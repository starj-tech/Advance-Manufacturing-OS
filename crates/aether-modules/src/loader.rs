//! Module loading lifecycle.
//!
//! Real implementation in PR #4:
//!   - Fetch bundle + signature from Supabase Storage / module registry
//!   - Verify signature via `verify::verify_bundle`
//!   - Cache to `<app_data>/aether/<tenant_id>/modules/<id>/<ver>/`
//!   - Hand off to TS-side runtime which spawns a Web Worker

use crate::manifest::Manifest;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("verify: {0}")]
    Verify(#[from] crate::verify::VerifyError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

#[derive(Debug, Clone)]
pub struct LoadedModule {
    pub manifest: Manifest,
    pub install_path: std::path::PathBuf,
}

pub async fn load(_manifest_toml: &str) -> Result<LoadedModule, LoadError> {
    Err(LoadError::NotImplemented("module loader wired in PR #4"))
}
