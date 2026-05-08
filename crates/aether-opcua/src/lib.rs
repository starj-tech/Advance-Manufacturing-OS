//! OPC-UA client wrapper.
//!
//! Backed by the `opcua` crate (locka99) — pure Rust, async-friendly,
//! supports certificate + user-token auth. The `Bridge` impl below is a
//! skeleton that compiles against the trait but returns
//! `BridgeError::NotImplemented`. Real subscription pipeline ships in PR #3.

use aether_protocols::{
    Bridge, BridgeError, BridgeId, BridgeKind, BridgeStatus, SampleStream, TagSample,
};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct OpcUaConfig {
    pub endpoint: String,
    pub application_uri: String,
    pub session_timeout_ms: u32,
    pub publish_interval_ms: u64,
}

impl Default for OpcUaConfig {
    fn default() -> Self {
        Self {
            endpoint: "opc.tcp://127.0.0.1:4840".into(),
            application_uri: "urn:aether-os:client".into(),
            session_timeout_ms: 60_000,
            publish_interval_ms: 200,
        }
    }
}

pub struct OpcUaBridge {
    id: BridgeId,
    config: OpcUaConfig,
    status: Arc<RwLock<BridgeStatus>>,
}

impl OpcUaBridge {
    pub fn new(id: impl Into<String>, config: OpcUaConfig) -> Self {
        Self {
            id: BridgeId(id.into()),
            config,
            status: Arc::new(RwLock::new(BridgeStatus::Disconnected)),
        }
    }

    pub fn config(&self) -> &OpcUaConfig {
        &self.config
    }
}

#[async_trait::async_trait]
impl Bridge for OpcUaBridge {
    fn id(&self) -> &BridgeId {
        &self.id
    }

    fn kind(&self) -> BridgeKind {
        BridgeKind::OpcUa
    }

    fn status(&self) -> BridgeStatus {
        match self.status.try_read() {
            Ok(g) => *g,
            Err(_) => BridgeStatus::Disconnected,
        }
    }

    async fn connect(&self) -> Result<(), BridgeError> {
        Err(BridgeError::NotImplemented(
            "OpcUaBridge::connect — wired in PR #3",
        ))
    }

    async fn disconnect(&self) -> Result<(), BridgeError> {
        *self.status.write().await = BridgeStatus::Disconnected;
        Ok(())
    }

    async fn subscribe(&self, _tag: &str) -> Result<SampleStream, BridgeError> {
        Err(BridgeError::NotImplemented("OpcUaBridge::subscribe"))
    }

    async fn write(&self, _tag: &str, _value: f64) -> Result<(), BridgeError> {
        Err(BridgeError::NotImplemented("OpcUaBridge::write"))
    }

    async fn snapshot(&self) -> Result<Vec<TagSample>, BridgeError> {
        Ok(vec![])
    }
}
