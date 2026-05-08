//! MQTT client wrapper, backed by `rumqttc`. PR #3 lands the real impl;
//! this crate ships the trait skeleton.

use aether_protocols::{
    Bridge, BridgeError, BridgeId, BridgeKind, BridgeStatus, SampleStream, TagSample,
};

#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub client_id: String,
    pub keep_alive_secs: u16,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 1883,
            client_id: "aether-os".into(),
            keep_alive_secs: 30,
        }
    }
}

pub struct MqttBridge {
    id: BridgeId,
    config: MqttConfig,
}

impl MqttBridge {
    pub fn new(id: impl Into<String>, config: MqttConfig) -> Self {
        Self {
            id: BridgeId(id.into()),
            config,
        }
    }

    pub fn config(&self) -> &MqttConfig {
        &self.config
    }
}

#[async_trait::async_trait]
impl Bridge for MqttBridge {
    fn id(&self) -> &BridgeId {
        &self.id
    }

    fn kind(&self) -> BridgeKind {
        BridgeKind::Mqtt
    }

    fn status(&self) -> BridgeStatus {
        BridgeStatus::Disconnected
    }

    async fn connect(&self) -> Result<(), BridgeError> {
        Err(BridgeError::NotImplemented(
            "MqttBridge::connect — wired in PR #3",
        ))
    }

    async fn disconnect(&self) -> Result<(), BridgeError> {
        Ok(())
    }

    async fn subscribe(&self, _tag: &str) -> Result<SampleStream, BridgeError> {
        Err(BridgeError::NotImplemented("MqttBridge::subscribe"))
    }

    async fn write(&self, _tag: &str, _value: f64) -> Result<(), BridgeError> {
        Err(BridgeError::NotImplemented("MqttBridge::write"))
    }

    async fn snapshot(&self) -> Result<Vec<TagSample>, BridgeError> {
        Ok(vec![])
    }
}
