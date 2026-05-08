//! Application-wide state container.

use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::RwLock;

#[derive(thiserror::Error, Debug)]
pub enum StateError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("db: {0}")]
    Db(#[from] aether_core::Error),
}

#[derive(Clone)]
pub struct AppState {
    pub session: Arc<RwLock<Option<Session>>>,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub tenant_id: String,
    pub user_id: String,
    pub primary_role: String,
}

impl AppState {
    pub async fn initialize(handle: &AppHandle) -> Result<(), StateError> {
        let state = Self {
            session: Arc::new(RwLock::new(None)),
        };
        handle.manage(state);
        tracing::info!("AppState initialized");
        Ok(())
    }
}
