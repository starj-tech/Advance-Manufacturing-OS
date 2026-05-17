//! Newtype wrappers around UUID for compile-time safety.
//!
//! Using `TenantId(Uuid)` instead of bare `Uuid` everywhere prevents the
//! classic bug of passing a `user_id` where a `tenant_id` is expected.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! id_newtype {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            pub fn from_uuid(u: Uuid) -> Self {
                Self(u)
            }

            pub fn into_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

id_newtype!(
    TenantId,
    "Identifier for a tenant (factory / customer org)."
);
id_newtype!(UserId, "Identifier for a user account.");
id_newtype!(MachineId, "Identifier for a manufacturing machine.");
id_newtype!(WorkOrderId, "Identifier for a work order.");
id_newtype!(MaterialId, "Identifier for a material/SKU.");
id_newtype!(ModuleId, "Identifier for an installed module.");
id_newtype!(
    CameraId,
    "Identifier for a machine-vision camera (inspection station, line-side, mobile)."
);
id_newtype!(
    RobotId,
    "Identifier for a controllable robot (arm, AGV, AMR)."
);
id_newtype!(
    ActuatorId,
    "Identifier for any controllable hardware actuator — superset of camera/robot."
);
id_newtype!(
    ScannerId,
    "Identifier for an auto-id scanner (barcode / QR / RFID / NFC)."
);
id_newtype!(
    ControllerId,
    "Identifier for a PLC / PAC / CNC / DCS controller."
);
id_newtype!(
    HmiId,
    "Identifier for an HMI surface (touchscreen / rugged tablet / smart glasses / pendant)."
);
id_newtype!(
    GatewayId,
    "Identifier for an IIoT gateway / edge server / protocol bridge."
);

/// Generic ULID-style entity identifier serialized as a string.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(pub String);

impl EntityId {
    pub fn new() -> Self {
        Self(Uuid::now_v7().to_string())
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
