use super::{CommandError, CommandResult};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct UnlockArgs {
    pub user_id: String,
    pub machine_id: String,
}

#[derive(Serialize)]
pub struct UnlockVerdict {
    pub allowed: bool,
    pub reason: String,
    pub missing_certs: Vec<String>,
    pub expired_certs: Vec<String>,
}

#[derive(Serialize)]
pub struct CertSummary {
    pub code: String,
    pub issued_at: String,
    pub expires_at: Option<String>,
    pub revoked: bool,
}

#[tauri::command]
pub async fn interlock_request_unlock(_args: UnlockArgs) -> CommandResult<UnlockVerdict> {
    Err(CommandError::NotImplemented(
        "interlock_request_unlock wired in PR #5 with the protocol bridges".into(),
    ))
}

#[tauri::command]
pub async fn interlock_user_certifications(_user_id: String) -> CommandResult<Vec<CertSummary>> {
    Ok(vec![])
}

#[tauri::command]
pub async fn interlock_machine_required_certs(_machine_id: String) -> CommandResult<Vec<String>> {
    Ok(vec![])
}
