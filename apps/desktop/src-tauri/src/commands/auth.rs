use super::{CommandError, CommandResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct PasskeyChallenge {
    pub challenge: String,
    pub rp_id: String,
}

#[derive(Deserialize)]
pub struct PasskeyAttestation {
    pub credential_id: String,
    pub client_data_json: String,
    pub attestation_object: String,
}

#[derive(Deserialize)]
pub struct PasskeyAssertion {
    pub credential_id: String,
    pub client_data_json: String,
    pub authenticator_data: String,
    pub signature: String,
}

#[derive(Serialize)]
pub struct SessionToken {
    pub access_token: String,
    pub refresh_token: String,
    pub primary_role: String,
}

#[tauri::command]
pub async fn passkey_register_start(_email: String) -> CommandResult<PasskeyChallenge> {
    Err(CommandError::NotImplemented(
        "passkey registration handled by Supabase Edge Function in next PR".into(),
    ))
}

#[tauri::command]
pub async fn passkey_register_finish(
    _attestation: PasskeyAttestation,
) -> CommandResult<SessionToken> {
    Err(CommandError::NotImplemented("passkey_register_finish".into()))
}

#[tauri::command]
pub async fn passkey_login_start(_email: String) -> CommandResult<PasskeyChallenge> {
    Err(CommandError::NotImplemented("passkey_login_start".into()))
}

#[tauri::command]
pub async fn passkey_login_finish(_assertion: PasskeyAssertion) -> CommandResult<SessionToken> {
    Err(CommandError::NotImplemented("passkey_login_finish".into()))
}

#[tauri::command]
pub async fn session_lock() -> CommandResult<()> {
    // In a full implementation this zeroizes the in-memory master key
    // and clears any decrypted caches.
    tracing::info!("session lock requested");
    Ok(())
}
