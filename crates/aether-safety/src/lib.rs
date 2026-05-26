//! Safety subsystem for the Employee shell.
//!
//! - SOS pipeline (Tier 0–4): hardware → mDNS → MQTT local → Supabase
//!   Realtime → external escalation. See `docs/architecture/safety.md`.
//! - Geofencing: weighted GPS + Wi-Fi BSSID + BLE evidence.
//! - Glove-friendly UI guidelines documented in `safety.md`.

pub mod geofence;
pub mod interlock;
pub mod sos;
pub mod sos_lifecycle;

pub use geofence::{Evidence, GeofenceEvaluator, Verdict as GeofenceVerdict};
pub use interlock::{
    evaluate as evaluate_interlock, Certification, InterlockController, InterlockError,
    UnlockRequest, Verdict as InterlockVerdict,
};
pub use sos::{SosBroadcaster, SosError, SosEvent, SosTier};
pub use sos_lifecycle::{SosLifecycleError, SosStatus, SosTransition};
