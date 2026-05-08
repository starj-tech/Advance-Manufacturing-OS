use crate::sample::TagSample;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_stream::Stream;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct BridgeId(pub String);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BridgeKind {
    OpcUa,
    Mqtt,
    Modbus,
    EthernetIp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BridgeStatus {
    Disconnected,
    Connecting,
    Connected,
    Faulted,
}

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("connection: {0}")]
    Connection(String),
    #[error("subscription: {0}")]
    Subscription(String),
    #[error("write: {0}")]
    Write(String),
    #[error("encoding: {0}")]
    Encoding(String),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

/// Stream of tag samples produced by `Bridge::subscribe`.
///
/// Pinned for object-safety so different bridge impls can return
/// heterogeneous concrete stream types.
pub type SampleStream =
    std::pin::Pin<Box<dyn Stream<Item = Result<TagSample, BridgeError>> + Send>>;

/// Abstraction over an industrial protocol endpoint.
///
/// Implementations:
///   - `aether-opcua::OpcUaBridge`
///   - `aether-mqtt::MqttBridge`
///
/// Backpressure: producers must respect bounded channels and drop-oldest
/// policy to protect the SQLite ingest path. See protocols.md.
#[async_trait::async_trait]
pub trait Bridge: Send + Sync {
    fn id(&self) -> &BridgeId;
    fn kind(&self) -> BridgeKind;
    fn status(&self) -> BridgeStatus;

    async fn connect(&self) -> Result<(), BridgeError>;
    async fn disconnect(&self) -> Result<(), BridgeError>;

    /// Subscribe to a tag mapped via `TagRegistry`. Each subscription
    /// returns its own stream.
    async fn subscribe(&self, tag: &str) -> Result<SampleStream, BridgeError>;

    /// Write a value back to a writable tag.
    async fn write(&self, tag: &str, value: f64) -> Result<(), BridgeError>;

    /// Snapshot of last-known values per tag.
    async fn snapshot(&self) -> Result<Vec<TagSample>, BridgeError>;
}
