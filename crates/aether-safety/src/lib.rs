//! Safety subsystem for the Employee shell.
//!
//! - SOS pipeline (Tier 0–4): hardware → mDNS → MQTT local → Supabase
//!   Realtime → external escalation. See `docs/architecture/safety.md`.
//! - Geofencing: weighted GPS + Wi-Fi BSSID + BLE evidence.
//! - Glove-friendly UI guidelines documented in `safety.md`.

pub mod geofence;
pub mod sos;

pub use geofence::{Evidence, GeofenceEvaluator, Verdict};
pub use sos::{SosBroadcaster, SosError, SosEvent, SosTier};
