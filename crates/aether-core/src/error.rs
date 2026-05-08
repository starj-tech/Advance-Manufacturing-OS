use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization: {0}")]
    Serialize(String),

    #[error("crypto: {0}")]
    Crypto(String),

    #[error("storage: {0}")]
    Storage(String),

    #[error("network: {0}")]
    Network(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialize(err.to_string())
    }
}
