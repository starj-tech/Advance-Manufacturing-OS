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

#[derive(Serialize)]
pub struct ManagerHealingCard {
    pub at: String,
    /// Plain-language headline ("Sync queue caught up by itself").
    pub title: String,
    /// One-paragraph plain-language explanation tailored for managers.
    pub plain_summary: String,
    /// "ok" | "watch" | "needs-you"
    pub urgency: String,
    /// True if a follow-up action requires the manager's confirmation.
    pub requires_confirmation: bool,
}

/// Manager-facing healing summary. Translates raw symptoms +
/// HealingPolicy into plain language so the manager doesn't need to
/// call IT support to understand what just happened.
///
/// Real impl pulls recent events from `audit_log` + healing ledger and
/// renders them through a small templating layer (PR #5). This skeleton
/// returns demo cards so the UI surface is exercised end-to-end.
#[tauri::command]
pub async fn healing_manager_summary() -> CommandResult<Vec<ManagerHealingCard>> {
    Ok(vec![
        ManagerHealingCard {
            at: "2026-05-08 04:12".into(),
            title: "Sync queue caught up by itself".into(),
            plain_summary: "Your factory was offline for 18 minutes around dawn. AETHER-OS held \
                            120 events locally and reconciled them automatically when the link came \
                            back. No data loss; no action needed.".into(),
            urgency: "ok".into(),
            requires_confirmation: false,
        },
        ManagerHealingCard {
            at: "2026-05-07 22:48".into(),
            title: "Press-01 OPC-UA reconnect storm — paused".into(),
            plain_summary: "PRESS-01's PLC dropped its OPC-UA session repeatedly. AETHER-OS \
                            paused new subscriptions for 2 minutes to let the controller stabilize, \
                            then resumed. If this repeats, ask maintenance to check the network \
                            cable to the cabinet.".into(),
            urgency: "watch".into(),
            requires_confirmation: false,
        },
        ManagerHealingCard {
            at: "2026-05-07 14:03".into(),
            title: "Telemetry storage at 90% — confirm trim?".into(),
            plain_summary: "Your local telemetry buffer was 90% full. AETHER-OS can keep the \
                            most recent 48 hours of raw readings and downsample older data to \
                            1-minute averages. Approve once and we'll handle this automatically \
                            from now on.".into(),
            urgency: "needs-you".into(),
            requires_confirmation: true,
        },
    ])
}
