//! Capability gating.
//!
//! A module's manifest declares the capabilities it wants
//! (`[permissions] read / write / network`). Two checks sit on top:
//!
//!  1. **Load-time policy check** ([`CapabilityPolicy::validate`]): the
//!     tenant defines the maximum capability surface any module may
//!     request. A signed bundle from a trusted publisher is *still*
//!     refused if its manifest asks for more than the tenant allows —
//!     signing proves authorship, not authorization. Defense in depth.
//!
//!  2. **Runtime request gate** ([`CapabilityGate`]): once installed, the
//!     module makes individual calls (`read('telemetry')`,
//!     `fetch('https://vendor/api')`). The gate answers allow/deny for
//!     each against the manifest's grants — this is what the TS Comlink
//!     bridge / WASI host bindings consult on every call.
//!
//! Read/write resources match exactly. Network matches by origin prefix
//! at a path boundary, so a grant for `https://v.example.com` allows
//! `https://v.example.com/api` but not `https://v.example.com.evil.com`.

use crate::manifest::Manifest;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityKind {
    Read,
    Write,
    Network,
}

/// A single runtime capability request from a running module.
#[derive(Clone, Copy, Debug)]
pub struct CapabilityRequest<'a> {
    pub kind: CapabilityKind,
    /// Resource name (`telemetry`, `work_orders`) for read/write, or the
    /// full request URL for network.
    pub resource: &'a str,
}

impl<'a> CapabilityRequest<'a> {
    pub fn read(resource: &'a str) -> Self {
        Self {
            kind: CapabilityKind::Read,
            resource,
        }
    }
    pub fn write(resource: &'a str) -> Self {
        Self {
            kind: CapabilityKind::Write,
            resource,
        }
    }
    pub fn network(url: &'a str) -> Self {
        Self {
            kind: CapabilityKind::Network,
            resource: url,
        }
    }
}

/// `true` if `url` is within the granted `origin` at a path boundary.
fn origin_allows(origin: &str, url: &str) -> bool {
    url == origin || url.starts_with(&format!("{origin}/"))
}

/// Runtime gate built from a module's granted permissions. Holds the
/// allowlists and answers per-call allow/deny.
#[derive(Clone, Debug)]
pub struct CapabilityGate {
    read: Vec<String>,
    write: Vec<String>,
    network: Vec<String>,
}

impl CapabilityGate {
    pub fn from_manifest(manifest: &Manifest) -> Self {
        let p = &manifest.permissions;
        Self {
            read: p.read.clone(),
            write: p.write.clone(),
            network: p.network.clone(),
        }
    }

    /// Decide a single request. Read/write match exactly; network matches
    /// by origin prefix at a path boundary.
    pub fn allows(&self, req: &CapabilityRequest) -> bool {
        match req.kind {
            CapabilityKind::Read => self.read.iter().any(|r| r == req.resource),
            CapabilityKind::Write => self.write.iter().any(|r| r == req.resource),
            CapabilityKind::Network => self
                .network
                .iter()
                .any(|origin| origin_allows(origin, req.resource)),
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum CapabilityError {
    #[error("module requests {kind:?} capability '{resource}' outside tenant policy")]
    OutsidePolicy {
        kind: CapabilityKind,
        resource: String,
    },
}

/// Tenant-defined maximum capability surface. A module manifest may
/// request a subset of this; anything beyond is refused at load time.
#[derive(Clone, Debug, Default)]
pub struct CapabilityPolicy {
    pub read: Vec<String>,
    pub write: Vec<String>,
    pub network: Vec<String>,
}

impl CapabilityPolicy {
    /// Validate that every capability the manifest requests is permitted
    /// by this policy. Read/write/network all require exact membership —
    /// the policy is an explicit allowlist, not a prefix rule.
    pub fn validate(&self, manifest: &Manifest) -> Result<(), CapabilityError> {
        let p = &manifest.permissions;
        for r in &p.read {
            if !self.read.contains(r) {
                return Err(CapabilityError::OutsidePolicy {
                    kind: CapabilityKind::Read,
                    resource: r.clone(),
                });
            }
        }
        for w in &p.write {
            if !self.write.contains(w) {
                return Err(CapabilityError::OutsidePolicy {
                    kind: CapabilityKind::Write,
                    resource: w.clone(),
                });
            }
        }
        for n in &p.network {
            if !self.network.contains(n) {
                return Err(CapabilityError::OutsidePolicy {
                    kind: CapabilityKind::Network,
                    resource: n.clone(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"
[module]
id = "com.aether.qc"
name = "QC"
version = "1.0.0"
shells = ["manager"]

[entry]
ui = "ui/index.js"

[permissions]
read = ["telemetry", "work_orders"]
write = ["spc_charts"]
network = ["https://vendor.example.com"]

[signature]
algorithm = "ed25519"
public_key_id = "p"
"#;

    fn manifest() -> Manifest {
        Manifest::from_toml(MANIFEST).unwrap()
    }

    #[test]
    fn gate_allows_granted_read_denies_others() {
        let g = CapabilityGate::from_manifest(&manifest());
        assert!(g.allows(&CapabilityRequest::read("telemetry")));
        assert!(g.allows(&CapabilityRequest::read("work_orders")));
        assert!(!g.allows(&CapabilityRequest::read("certifications")));
    }

    #[test]
    fn gate_separates_read_and_write() {
        let g = CapabilityGate::from_manifest(&manifest());
        // spc_charts is writable but not in the read grant.
        assert!(g.allows(&CapabilityRequest::write("spc_charts")));
        assert!(!g.allows(&CapabilityRequest::read("spc_charts")));
        assert!(!g.allows(&CapabilityRequest::write("telemetry")));
    }

    #[test]
    fn gate_network_matches_origin_at_path_boundary() {
        let g = CapabilityGate::from_manifest(&manifest());
        assert!(g.allows(&CapabilityRequest::network("https://vendor.example.com")));
        assert!(g.allows(&CapabilityRequest::network(
            "https://vendor.example.com/api/v1"
        )));
        // Suffix-attack origin must not match.
        assert!(!g.allows(&CapabilityRequest::network(
            "https://vendor.example.com.evil.com"
        )));
        assert!(!g.allows(&CapabilityRequest::network("https://other.example.com")));
    }

    #[test]
    fn empty_grants_deny_everything() {
        let m = Manifest::from_toml(
            r#"
[module]
id = "x"
name = "x"
version = "1.0.0"
shells = ["manager"]
[entry]
ui = "u"
[signature]
algorithm = "ed25519"
public_key_id = "p"
"#,
        )
        .unwrap();
        let g = CapabilityGate::from_manifest(&m);
        assert!(!g.allows(&CapabilityRequest::read("telemetry")));
        assert!(!g.allows(&CapabilityRequest::network("https://x")));
    }

    #[test]
    fn policy_accepts_a_subset_request() {
        let policy = CapabilityPolicy {
            read: vec!["telemetry".into(), "work_orders".into(), "extra".into()],
            write: vec!["spc_charts".into()],
            network: vec!["https://vendor.example.com".into()],
        };
        assert!(policy.validate(&manifest()).is_ok());
    }

    #[test]
    fn policy_refuses_a_read_outside_it() {
        let policy = CapabilityPolicy {
            read: vec!["telemetry".into()], // missing work_orders
            write: vec!["spc_charts".into()],
            network: vec!["https://vendor.example.com".into()],
        };
        assert_eq!(
            policy.validate(&manifest()),
            Err(CapabilityError::OutsidePolicy {
                kind: CapabilityKind::Read,
                resource: "work_orders".into(),
            })
        );
    }

    #[test]
    fn policy_refuses_an_unlisted_network_origin() {
        let policy = CapabilityPolicy {
            read: vec!["telemetry".into(), "work_orders".into()],
            write: vec!["spc_charts".into()],
            network: vec![], // no network allowed
        };
        assert!(matches!(
            policy.validate(&manifest()),
            Err(CapabilityError::OutsidePolicy {
                kind: CapabilityKind::Network,
                ..
            })
        ));
    }
}
