use super::{CommandError, CommandResult};
use serde::Serialize;

#[derive(Serialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub commit: Option<&'static str>,
}

#[derive(Serialize)]
pub struct RuntimeStatus {
    pub online: bool,
    pub sync_lag_ms: u64,
    pub bridges: Vec<String>,
}

#[tauri::command]
pub fn get_app_info() -> CommandResult<AppInfo> {
    Ok(AppInfo {
        name: "AETHER-OS",
        version: env!("CARGO_PKG_VERSION"),
        commit: option_env!("VERGEN_GIT_SHA"),
    })
}

#[tauri::command]
pub fn get_runtime_status() -> CommandResult<RuntimeStatus> {
    // Placeholder: a future PR wires this to aether-sync, aether-protocols.
    let _ = CommandError::Internal("placeholder".into());
    Ok(RuntimeStatus {
        online: false,
        sync_lag_ms: 0,
        bridges: vec![],
    })
}
