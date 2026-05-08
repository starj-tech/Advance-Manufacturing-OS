use super::{CommandError, CommandResult};
use serde::Serialize;

#[derive(Serialize)]
pub struct SyncStatus {
    pub online: bool,
    pub outbox_size: u32,
    pub last_pulled_hlc: Option<String>,
    pub last_pushed_hlc: Option<String>,
}

#[tauri::command]
pub async fn push_outbox() -> CommandResult<u32> {
    Err(CommandError::NotImplemented(
        "outbox push wired in PR #2 (sync engine)".into(),
    ))
}

#[tauri::command]
pub async fn pull_changes() -> CommandResult<u32> {
    Err(CommandError::NotImplemented("pull_changes".into()))
}

#[tauri::command]
pub async fn sync_status() -> CommandResult<SyncStatus> {
    Ok(SyncStatus {
        online: false,
        outbox_size: 0,
        last_pulled_hlc: None,
        last_pushed_hlc: None,
    })
}
