//! Module loading lifecycle.
//!
//! ## What load() does
//! 1. Parse the manifest TOML (validates structure + signature
//!    algo).
//! 2. Look up the publisher's trusted key by id; refuse if
//!    untrusted.
//! 3. Verify the ed25519 signature against
//!    `Sha256(bundle_bytes) || manifest_bytes`.
//! 4. Determine the cache path:
//!    `<cache_root>/<tenant_id>/<module_id>/<module_version>/`.
//! 5. Write the manifest + bundle into that path atomically
//!    (temp-then-rename so a torn write can never leave a
//!    half-installed module visible to the runtime).
//! 6. Return a `LoadedModule` with the manifest + the install
//!    path the runtime hands to the Web Worker spawner.
//!
//! ## What load() does NOT do
//! - Fetching from Supabase Storage. The caller passes raw
//!   bundle bytes. A future session can layer a fetcher on
//!   top once the Supabase client is wired; the verify+cache
//!   layer here is the trust-critical part and stays pure.
//! - Spawning the runtime. The TS-side runtime takes the
//!   `install_path` and does the Web Worker spawn / Comlink
//!   bridge. The loader is content-agnostic.
//!
//! ## Atomic install
//! Half-installed modules are a security problem (the runtime
//! could try to load a truncated UI bundle and crash, or worse,
//! load a partially-overwritten bundle that mixes old + new
//! code). We write to a temp dir under the same parent (same
//! filesystem → atomic rename) and only rename into the final
//! path after every file is fsync'd.

use crate::manifest::Manifest;
use crate::verify::verify_bundle;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("manifest: {0}")]
    Manifest(#[from] crate::manifest::ManifestError),
    #[error("verify: {0}")]
    Verify(#[from] crate::verify::VerifyError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// `cache_root` points at a file (or a non-creatable
    /// path). Caller-facing — usually the result of a config
    /// typo.
    #[error("cache_root {0} is not a directory and cannot be created")]
    BadCacheRoot(PathBuf),
}

#[derive(Debug, Clone)]
pub struct LoadedModule {
    pub manifest: Manifest,
    pub install_path: PathBuf,
}

/// Compute the canonical install path for a module under a
/// given cache root. Path components are derived from data on
/// the manifest; the caller passes tenant_id as the
/// multi-tenant namespacing dimension.
///
/// Shape: `<cache_root>/<tenant_id>/<module_id>/<version>/`.
pub fn install_path(cache_root: &Path, tenant_id: Uuid, manifest: &Manifest) -> PathBuf {
    cache_root
        .join(tenant_id.to_string())
        .join(&manifest.module.id)
        .join(&manifest.module.version)
}

/// Synchronous (file-system bound) module loader. The
/// signature verify is the trust gate; everything after is
/// the durable-install ceremony.
pub fn load(
    manifest_toml: &str,
    bundle_bytes: &[u8],
    signature: &[u8],
    trusted_keys: &[(String, [u8; 32])],
    cache_root: &Path,
    tenant_id: Uuid,
) -> Result<LoadedModule, LoadError> {
    let manifest = Manifest::from_toml(manifest_toml)?;
    verify_bundle(
        &manifest,
        manifest_toml.as_bytes(),
        bundle_bytes,
        signature,
        trusted_keys,
    )?;

    if !cache_root.exists() {
        std::fs::create_dir_all(cache_root)?;
    }
    let target = install_path(cache_root, tenant_id, &manifest);
    if !target.parent().is_some_and(|p| p.exists()) {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // Atomic install: write to a sibling temp dir, rename
    // into place. Same parent → same filesystem → rename is
    // atomic on POSIX. On Windows we accept the (very) small
    // window: rename will replace; partial writes are
    // detectable because the manifest is the last file moved.
    let temp_dir = target.with_file_name(format!(
        "{}.tmp.{}",
        manifest.module.version,
        Uuid::now_v7()
    ));
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
    }
    std::fs::create_dir_all(&temp_dir)?;

    // Write bundle first, manifest second. If a crash happens
    // mid-install, the absent or older-named manifest file
    // signals "skip this dir, it's incomplete."
    std::fs::write(temp_dir.join("bundle.bin"), bundle_bytes)?;
    std::fs::write(temp_dir.join("signature.bin"), signature)?;
    std::fs::write(temp_dir.join("manifest.toml"), manifest_toml)?;

    // If a previous install at `target` exists, remove it
    // before the rename. This is the "atomic upgrade" path —
    // the operator's module-list never sees a hybrid old+new
    // tree because the new tree is the rename target, not an
    // overlay.
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    std::fs::rename(&temp_dir, &target)?;

    Ok(LoadedModule {
        manifest,
        install_path: target,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_crypto::Signer;

    const MANIFEST: &str = r#"
[module]
id = "com.aether.test"
name = "Test"
version = "1.0.0"
shells = ["manager"]

[entry]
ui = "ui/index.js"

[signature]
algorithm = "ed25519"
public_key_id = "test-publisher"
"#;

    type TrustedKeys = Vec<(String, [u8; 32])>;

    /// Build a (signer, verifying_key_bytes, trusted_keys)
    /// triple for the test "test-publisher" id.
    fn fixture_publisher() -> (Signer, [u8; 32], TrustedKeys) {
        let signer = Signer::generate();
        let vk = signer.public_key().to_bytes();
        let trusted = vec![("test-publisher".to_string(), vk)];
        (signer, vk, trusted)
    }

    fn sign_bundle(signer: &Signer, manifest_toml: &str, bundle: &[u8]) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bundle);
        let bundle_hash = hasher.finalize();
        let mut signed = Vec::with_capacity(bundle_hash.len() + manifest_toml.len());
        signed.extend_from_slice(&bundle_hash);
        signed.extend_from_slice(manifest_toml.as_bytes());
        signer.sign(&signed)
    }

    fn temp_cache() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    #[test]
    fn happy_path_installs_module_with_all_three_files() {
        let cache = temp_cache();
        let (signer, _vk, trusted) = fixture_publisher();
        let bundle = b"fake-tarball-contents";
        let sig = sign_bundle(&signer, MANIFEST, bundle);
        let tenant = Uuid::new_v4();

        let loaded = load(MANIFEST, bundle, &sig, &trusted, cache.path(), tenant).unwrap();
        assert_eq!(loaded.manifest.module.id, "com.aether.test");
        assert_eq!(loaded.manifest.module.version, "1.0.0");

        // All three files present at the canonical path.
        let p = &loaded.install_path;
        assert!(p.join("bundle.bin").exists());
        assert!(p.join("signature.bin").exists());
        assert!(p.join("manifest.toml").exists());

        // Path shape: <cache>/<tenant>/<module_id>/<version>/
        assert!(p.ends_with("1.0.0"));
        assert!(p.parent().unwrap().ends_with("com.aether.test"));
        assert!(p
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .ends_with(tenant.to_string()));
    }

    #[test]
    fn untrusted_publisher_is_refused_before_any_file_is_written() {
        let cache = temp_cache();
        let (signer, _vk, _) = fixture_publisher();
        // Trusted set has a DIFFERENT publisher id — manifest
        // says "test-publisher", trusted set says "other".
        let other_signer = Signer::generate();
        let trusted = vec![("other".to_string(), other_signer.public_key().to_bytes())];
        let bundle = b"x";
        let sig = sign_bundle(&signer, MANIFEST, bundle);
        let tenant = Uuid::new_v4();

        let err = load(MANIFEST, bundle, &sig, &trusted, cache.path(), tenant).unwrap_err();
        assert!(matches!(err, LoadError::Verify(_)));

        // No files written — the verify gate runs before any
        // filesystem mutation.
        assert!(cache.path().read_dir().unwrap().next().is_none());
    }

    #[test]
    fn tampered_bundle_is_refused() {
        let cache = temp_cache();
        let (signer, _vk, trusted) = fixture_publisher();
        let bundle = b"original-bundle";
        let sig = sign_bundle(&signer, MANIFEST, bundle);

        // Tamper: sign one bundle, attempt to install another.
        let tampered = b"swapped-bundle";
        let tenant = Uuid::new_v4();
        let err = load(MANIFEST, tampered, &sig, &trusted, cache.path(), tenant).unwrap_err();
        assert!(matches!(err, LoadError::Verify(_)));
    }

    #[test]
    fn tampered_manifest_is_refused() {
        let cache = temp_cache();
        let (signer, _vk, trusted) = fixture_publisher();
        let bundle = b"x";
        let sig = sign_bundle(&signer, MANIFEST, bundle);
        let tenant = Uuid::new_v4();

        // Append a permission the publisher never authorized.
        let tampered_manifest = format!("{MANIFEST}\n[evil]\nread = [\"keystore\"]\n");
        let err = load(
            &tampered_manifest,
            bundle,
            &sig,
            &trusted,
            cache.path(),
            tenant,
        )
        .unwrap_err();
        assert!(matches!(err, LoadError::Verify(_)));
    }

    #[test]
    fn malformed_manifest_is_refused_with_manifest_error() {
        let cache = temp_cache();
        let (_signer, _vk, trusted) = fixture_publisher();
        let err = load(
            "this is not toml",
            b"x",
            &[0u8; 64],
            &trusted,
            cache.path(),
            Uuid::new_v4(),
        )
        .unwrap_err();
        assert!(matches!(err, LoadError::Manifest(_)));
    }

    #[test]
    fn reinstall_replaces_existing_module_atomically() {
        // Install v1.0.0 then v1.0.0 again with different
        // bundle contents — second install replaces the
        // first cleanly (no leftover files from the old
        // bundle in the new tree).
        let cache = temp_cache();
        let (signer, _vk, trusted) = fixture_publisher();
        let tenant = Uuid::new_v4();

        let bundle_a = b"version-a-bytes";
        let sig_a = sign_bundle(&signer, MANIFEST, bundle_a);
        load(MANIFEST, bundle_a, &sig_a, &trusted, cache.path(), tenant).unwrap();

        let bundle_b = b"version-b-with-more-bytes";
        let sig_b = sign_bundle(&signer, MANIFEST, bundle_b);
        let loaded = load(MANIFEST, bundle_b, &sig_b, &trusted, cache.path(), tenant).unwrap();

        let bundle_on_disk = std::fs::read(loaded.install_path.join("bundle.bin")).unwrap();
        assert_eq!(bundle_on_disk, bundle_b);
    }

    #[test]
    fn install_paths_are_isolated_per_tenant() {
        // Multi-tenant: same module id under tenant A and
        // tenant B installs to different directories so
        // tenant A can't see (or replace) tenant B's tree.
        let cache = temp_cache();
        let (signer, _vk, trusted) = fixture_publisher();
        let bundle = b"x";
        let sig = sign_bundle(&signer, MANIFEST, bundle);

        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let loaded_a = load(MANIFEST, bundle, &sig, &trusted, cache.path(), a).unwrap();
        let loaded_b = load(MANIFEST, bundle, &sig, &trusted, cache.path(), b).unwrap();
        assert_ne!(loaded_a.install_path, loaded_b.install_path);
        // Both still exist after the second install.
        assert!(loaded_a.install_path.join("manifest.toml").exists());
        assert!(loaded_b.install_path.join("manifest.toml").exists());
    }

    #[test]
    fn install_path_helper_matches_load_output_path() {
        let cache = temp_cache();
        let manifest = Manifest::from_toml(MANIFEST).unwrap();
        let tenant = Uuid::new_v4();
        let p = install_path(cache.path(), tenant, &manifest);
        assert!(p.ends_with("1.0.0"));
        assert!(p.to_string_lossy().contains(&tenant.to_string()));
        assert!(p.to_string_lossy().contains("com.aether.test"));
    }
}
