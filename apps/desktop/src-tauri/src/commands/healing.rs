use super::{CommandError, CommandResult};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct DiagnoseArgs {
    pub source: String,
    pub kind: String,
    pub severity: u8,
    pub detail: serde_json::Value,
}

#[derive(Serialize)]
pub struct DiagnosisDto {
    pub rationale: String,
    pub confidence: f32,
    pub policy: serde_json::Value,
    pub destructive: bool,
}

#[derive(Serialize)]
pub struct HealingEventDto {
    pub id: String,
    pub at: String,
    pub source: String,
    pub kind: String,
    pub policy: String,
    pub outcome: String,
}

#[tauri::command]
pub async fn healing_diagnose(_args: DiagnoseArgs) -> CommandResult<Option<DiagnosisDto>> {
    Err(CommandError::NotImplemented(
        "healing diagnose wired in PR #5 (real healers + edge function)".into(),
    ))
}

#[tauri::command]
pub async fn healing_apply(_policy: serde_json::Value) -> CommandResult<String> {
    Err(CommandError::NotImplemented("healing_apply".into()))
}

#[tauri::command]
pub async fn healing_recent_events(_limit: u32) -> CommandResult<Vec<HealingEventDto>> {
    Ok(vec![])
}
