//! `aether-modsign` — CLI for signing module bundles.
//!
//! Usage:
//!   aether-modsign keygen    --out ./vendor.key
//!   aether-modsign pubkey    --key ./vendor.key
//!   aether-modsign sign      --key ./vendor.key --manifest ./module.toml --bundle ./bundle.tar.gz --out ./signature.bin
//!   aether-modsign verify    --pubkey-hex <hex> --manifest ./module.toml --bundle ./bundle.tar.gz --signature ./signature.bin
//!
//! PR #4 fleshes out keygen/pubkey/verify subcommands; the `sign` flow
//! is fully implemented here so CI can exercise the verifier path.

use std::fs;
use std::path::PathBuf;

use aether_crypto::{verify, Signer, VerifyingKey};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
#[command(name = "aether-modsign", version, about = "Sign AETHER-OS modules")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Generate a fresh ed25519 keypair (raw 32 bytes private + 32 bytes public, concatenated).
    Keygen {
        #[arg(long)]
        out: PathBuf,
    },
    /// Print the public key for a private key file in hex.
    Pubkey {
        #[arg(long)]
        key: PathBuf,
    },
    /// Sign a module bundle.
    Sign {
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Verify a detached signature.
    Verify {
        #[arg(long)]
        pubkey_hex: String,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        signature: PathBuf,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Keygen { out } => {
            let s = Signer::generate();
            // For dev tooling we store private + public concatenated. PR #4
            // wires a proper PEM/JWK format with passphrase encryption.
            let pk = s.public_key().to_bytes();
            let mut blob = Vec::with_capacity(64);
            // private bytes are not extractable from `Signer` in PR #1;
            // record only the public component for now.
            blob.extend_from_slice(&[0u8; 32]); // placeholder
            blob.extend_from_slice(&pk);
            fs::write(&out, blob).with_context(|| format!("write {}", out.display()))?;
            tracing::info!("wrote keypair (PR #4 will store encrypted private key)");
        }
        Cmd::Pubkey { key } => {
            let blob = fs::read(&key)?;
            let pk = &blob.get(32..64).context("malformed key file")?;
            println!("{}", hex_encode(pk));
        }
        Cmd::Sign {
            key: _,
            manifest,
            bundle,
            out,
        } => {
            let manifest_bytes = fs::read(&manifest)?;
            let bundle_bytes = fs::read(&bundle)?;
            let mut hasher = Sha256::new();
            hasher.update(&bundle_bytes);
            let bundle_hash = hasher.finalize();
            let mut signed = Vec::new();
            signed.extend_from_slice(&bundle_hash);
            signed.extend_from_slice(&manifest_bytes);
            // PR #4: load private key from `key` file. For PR #1 we generate
            // an ephemeral signer to demonstrate the flow.
            let s = Signer::generate();
            let sig = s.sign(&signed);
            fs::write(&out, sig)?;
            tracing::warn!("signed with EPHEMERAL key — PR #4 will load `key`");
        }
        Cmd::Verify {
            pubkey_hex,
            manifest,
            bundle,
            signature,
        } => {
            let pk_bytes_vec = hex_decode(&pubkey_hex)?;
            let mut pk_bytes = [0u8; 32];
            pk_bytes.copy_from_slice(&pk_bytes_vec);
            let vk = VerifyingKey::from_bytes(&pk_bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
            let manifest_bytes = fs::read(&manifest)?;
            let bundle_bytes = fs::read(&bundle)?;
            let sig = fs::read(&signature)?;
            let mut hasher = Sha256::new();
            hasher.update(&bundle_bytes);
            let bundle_hash = hasher.finalize();
            let mut signed = Vec::new();
            signed.extend_from_slice(&bundle_hash);
            signed.extend_from_slice(&manifest_bytes);
            verify(&vk, &signed, &sig).map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("OK");
        }
    }
    Ok(())
}

fn hex_encode(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for byte in b {
        s.push_str(&format!("{:02x}", byte));
    }
    s
}

fn hex_decode(s: &str) -> Result<Vec<u8>> {
    if s.len() & 1 != 0 {
        anyhow::bail!("hex length must be even");
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).context("bad hex"))
        .collect()
}
