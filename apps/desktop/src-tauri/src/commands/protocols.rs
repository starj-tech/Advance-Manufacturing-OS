use super::{CommandError, CommandResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct BridgeInfo {
    pub id: String,
    pub kind: String, // "opcua" | "mqtt"
    pub status: String,
}

#[derive(Deserialize)]
pub struct SubscribeArgs {
    pub bridge_id: String,
    pub tag: String,
}

#[tauri::command]
pub async fn list_bridges() -> CommandResult<Vec<BridgeInfo>> {
    // Placeholder; aether-protocols::registry will populate this.
    Ok(vec![])
}

#[tauri::command]
pub async fn subscribe_tag(_args: SubscribeArgs) -> CommandResult<String> {
    Err(CommandError::NotImplemented(
        "tag subscription wired in PR #3 (industrial protocols)".into(),
    ))
}

#[tauri::command]
pub async fn unsubscribe_tag(_subscription_id: String) -> CommandResult<()> {
    Ok(())
}
