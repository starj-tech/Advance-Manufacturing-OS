//! IPC command surface exposed to the React frontend.
//!
//! All Rust-side command handlers live under this module. Each submodule
//! corresponds to one architectural concern (system, auth, sync, protocols,
//! modules, safety) so the boundary between frontend and Rust stays explicit.

pub mod auth;
pub mod discovery;
pub mod healing;
pub mod hedging;
pub mod i18n;
pub mod interlock;
pub mod modules;
pub mod protocols;
pub mod safety;
pub mod sync;
pub mod system;

use serde::Serialize;

#[derive(thiserror::Error, Debug, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum CommandError {
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("internal: {0}")]
    Internal(String),
}

pub type CommandResult<T> = Result<T, CommandError>;
