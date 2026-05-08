//! Zero-config IoT auto-discovery.
//!
//! "Plug a PLC into the network, hit Scan, see live data within minutes
//! instead of weeks." This crate orchestrates a set of `DiscoveryProbe`
//! implementations against a CIDR range and surfaces:
//!
//!   - Endpoints reachable on each protocol (OPC-UA, MQTT, Modbus,
//!     EtherNet/IP)
//!   - Vendor metadata (where the protocol exposes it)
//!   - Suggested tag bindings the user can accept with one click
//!
//! ## Trust boundary
//! Discovery is read-only. Probes never write to PLCs and never leave the
//! local network — anything sent to Supabase is the user's explicit
//! "save discovered device" action.
//!
//! ## Skeleton scope
//! This PR ships the probe trait surface, the orchestrator, and the
//! result types. Real network probing wires up in PR #3 alongside the
//! OPC-UA / MQTT bridges.

pub mod probe;
pub mod result;
pub mod scanner;
pub mod suggestion;

pub use probe::{
    DiscoveryProbe, EthernetIpProbe, ModbusProbe, MqttProbe, OpcUaProbe, ProbeError, ProbeKind,
};
pub use result::{DiscoveredDevice, VendorMetadata};
pub use scanner::{ScanRequest, Scanner};
pub use suggestion::{Confidence, SuggestedBinding};
