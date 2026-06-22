# Industrial Protocols

> Status: skeleton. Real OPC-UA / MQTT subscriptions land in PR #3.

## Why native, not browser

Browser-based factory dashboards lose: native socket access, deterministic
latency, robust reconnection, and protocol features that need binary
sockets (e.g. OPC-UA secure channel). Tauri lets us put the protocol
clients in Rust and stream samples to the renderer over IPC.

## Bridge abstraction

`crates/aether-protocols/src/bridge.rs::Bridge`:

```rust
async fn connect(&self) -> Result<()>;
async fn subscribe(&self, tag: &str) -> Result<SampleStream>;
async fn write(&self, tag: &str, value: f64) -> Result<()>;
async fn snapshot(&self) -> Result<Vec<TagSample>>;
```

Implementations:

- `aether-opcua` — `opcua` crate (locka99). Pure Rust, async, certificate
  - user-token auth.
- `aether-mqtt` — `rumqttc`. Pure Rust, MQTT 5.

A future `aether-modbus` would slot in identically.

## Tag mapping

Domain code never sees a NodeId or topic. `protocols.toml` maps domain
tags to protocol sources:

```toml
[[binding]]
tag = "press_01.temp"
source = "opcua://plc1/ns=2;s=Temperature"
unit = "celsius"
scale = 0.1
deadband = 0.5
```

Loaded into `TagRegistry` at startup.

## Backpressure

PLC bursts (10k samples/s) → bounded `tokio::mpsc` (4096) → 200ms batch
aggregator → SQLite multi-row INSERT. Drop-oldest with counter visible
to the Manager Shell. Decimated stream (10 Hz) goes to UI; raw stream
goes to DB.

## Frontend bridge

Tauri `Channel<TagSample>` over IPC, NOT a local WebSocket. Lower
latency, no port allocation, no CORS friction, native auth.

```rust
#[tauri::command]
async fn subscribe_tag(tag: String, channel: Channel<TagSample>)
    -> Result<SubId>;
```

## Mock server

`tools/opcua-mock-server` runs in dev/CI. PR #3 makes it expose
synthetic NodeIds (sine + noise) so end-to-end tests don't depend on
real PLCs.
