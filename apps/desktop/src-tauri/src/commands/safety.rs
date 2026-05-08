use super::{CommandError, CommandResult};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct SosArgs {
    pub user_id: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub accuracy_m: Option<u32>,
    pub note: Option<String>,
}

#[derive(Serialize)]
pub struct SosAck {
    pub event_id: String,
    pub local_broadcast: bool,
    pub mqtt_published: bool,
    pub cloud_persisted: bool,
}

#[derive(Deserialize)]
pub struct GeofenceQuery {
    pub fence_id: String,
    pub lat: f64,
    pub lng: f64,
    pub accuracy_m: u32,
    pub bssids: Vec<String>,
}

#[derive(Serialize)]
pub struct GeofenceVerdict {
    pub inside: bool,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

#[tauri::command]
pub async fn trigger_sos(_args: SosArgs) -> CommandResult<SosAck> {
    Err(CommandError::NotImplemented(
        "SOS pipeline wired in PR #5 (safety subsystem)".into(),
    ))
}

#[tauri::command]
pub async fn geofence_evaluate(_query: GeofenceQuery) -> CommandResult<GeofenceVerdict> {
    Ok(GeofenceVerdict {
        inside: true,
        confidence: 0.0,
        evidence: vec!["placeholder".into()],
    })
}
