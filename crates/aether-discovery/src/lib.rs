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
//! ## Implementation status
//! Phase 1: TCP-knock per protocol (port-open evidence + protocol-kind
//! tagging) + CIDR-iterating Scanner with concurrency limit + fingerprint
//! dedup. Phase 2 (PR #3): vendor-fingerprint exchanges layered on top
//! (OPC-UA `find_servers`, Modbus 0x2B/0x0E Read Device Identification,
//! CIP List Identity).

pub mod cidr;
pub mod probe;
pub mod result;
pub mod scanner;
pub mod suggestion;

pub use cidr::{expand_cidr, CidrError, MAX_HOSTS};
pub use probe::{
    DiscoveryProbe, EthernetIpProbe, GatewayProbe, HmiProbe, ModbusProbe, MqttProbe, OpcUaProbe,
    ProbeError, ProbeKind, RobotProbe, ScannerProbe, VisionProbe, DEFAULT_PROBE_TIMEOUT,
};
pub use result::{DiscoveredDevice, VendorMetadata};
pub use scanner::{ScanError, ScanRequest, Scanner};
pub use suggestion::{Confidence, SuggestedBinding};
