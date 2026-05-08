use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManifestModule {
    pub id: String,
    pub name: String,
    pub version: String,
    pub shells: Vec<String>,
    #[serde(default)]
    pub min_aether: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub ui: String,
    #[serde(default)]
    pub backend: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ManifestPermissions {
    #[serde(default)]
    pub read: Vec<String>,
    #[serde(default)]
    pub write: Vec<String>,
    #[serde(default)]
    pub network: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManifestSig {
    pub algorithm: String,
    pub public_key_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub module: ManifestModule,
    pub entry: ManifestEntry,
    #[serde(default)]
    pub permissions: ManifestPermissions,
    #[serde(default)]
    pub dependencies: std::collections::BTreeMap<String, String>,
    pub signature: ManifestSig,
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid: {0}")]
    Invalid(String),
}

impl Manifest {
    pub fn from_toml(input: &str) -> Result<Self, ManifestError> {
        let m: Self = toml::from_str(input)?;
        if m.signature.algorithm != "ed25519" {
            return Err(ManifestError::Invalid(format!(
                "unsupported signature algorithm: {}",
                m.signature.algorithm
            )));
        }
        if m.module.shells.is_empty() {
            return Err(ManifestError::Invalid(
                "module.shells must be non-empty".into(),
            ));
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[module]
id = "com.aether.qc-spc"
name = "Statistical Process Control"
version = "1.4.2"
shells = ["manager", "executive"]
min_aether = "0.3.0"

[entry]
ui = "ui/index.js"
backend = "backend.wasm"

[permissions]
read = ["telemetry", "work_orders"]
write = ["spc_charts"]
network = ["https://qc-vendor.example.com"]

[dependencies]
"@aether/ui-kit" = "^1.0.0"

[signature]
algorithm = "ed25519"
public_key_id = "vendor-acme-2026"
"#;

    #[test]
    fn parses_valid_manifest() {
        let m = Manifest::from_toml(SAMPLE).unwrap();
        assert_eq!(m.module.id, "com.aether.qc-spc");
        assert_eq!(m.module.shells, vec!["manager", "executive"]);
        assert_eq!(m.entry.ui, "ui/index.js");
        assert_eq!(m.permissions.read.len(), 2);
    }

    #[test]
    fn rejects_unknown_signature_algo() {
        let bad = SAMPLE.replace("ed25519", "rsa-pkcs1");
        assert!(matches!(
            Manifest::from_toml(&bad),
            Err(ManifestError::Invalid(_))
        ));
    }
}
