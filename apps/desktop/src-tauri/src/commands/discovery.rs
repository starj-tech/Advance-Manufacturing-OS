use super::{CommandError, CommandResult};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ScanArgs {
    pub cidr: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Serialize)]
pub struct ScanProgress {
    pub hosts_total: u32,
    pub hosts_completed: u32,
    pub devices_found: u32,
}

#[derive(Serialize)]
pub struct DiscoveredDeviceDto {
    pub fingerprint: String,
    pub host: String,
    pub port: u16,
    pub probe: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub suggested_bindings_count: usize,
}

#[tauri::command]
pub async fn discovery_scan(_args: ScanArgs) -> CommandResult<Vec<DiscoveredDeviceDto>> {
    Err(CommandError::NotImplemented(
        "discovery scan wired in PR #3 with the protocol bridges".into(),
    ))
}

#[tauri::command]
pub async fn discovery_save_device(_fingerprint: String) -> CommandResult<()> {
    Err(CommandError::NotImplemented("discovery_save_device".into()))
}
