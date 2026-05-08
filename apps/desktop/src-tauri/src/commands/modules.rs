use super::{CommandError, CommandResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ModuleSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub shells: Vec<String>,
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct InstallArgs {
    pub module_id: String,
    pub version: String,
}

#[tauri::command]
pub async fn list_installed() -> CommandResult<Vec<ModuleSummary>> {
    Ok(vec![])
}

#[tauri::command]
pub async fn install_module(_args: InstallArgs) -> CommandResult<()> {
    Err(CommandError::NotImplemented(
        "module install wired in PR #4 (module runtime)".into(),
    ))
}

#[tauri::command]
pub async fn uninstall_module(_module_id: String) -> CommandResult<()> {
    Err(CommandError::NotImplemented("uninstall_module".into()))
}
