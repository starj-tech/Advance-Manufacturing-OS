use serde::Serialize;

/// A capability identifier checked by the UI via `useCapability("...")`.
///
/// `kind` groups capabilities so the UI can render them as a coherent
/// section (Tracking, Quality, Maintenance, Compliance).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityKind {
    Tracking,
    Quality,
    Maintenance,
    Safety,
    Compliance,
    Process,
    Sustainability,
}

/// Static catalog entry. Serialize-only by design: capabilities flow
/// from the code-defined catalog out to the frontend, never the other
/// way. This lets us keep `&'static str` for zero-allocation iteration.
#[derive(Clone, Debug, Serialize)]
pub struct Capability {
    pub id: &'static str,
    pub kind: CapabilityKind,
    pub display: &'static str,
    pub description: &'static str,
}

/// Helper macro for the static profile table.
macro_rules! cap {
    ($id:expr, $kind:expr, $display:expr, $desc:expr) => {
        Capability {
            id: $id,
            kind: $kind,
            display: $display,
            description: $desc,
        }
    };
}

pub(crate) use cap;
