//! Hybrid Logical Clock — wall-clock + logical counter for sync ordering.
//!
//! Wire format: `<wall_millis>.<logical_counter>.<node_id>` so HLC values
//! can be compared lexicographically when treated as strings, and parsed
//! back when needed by the sync engine.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Hlc {
    pub wall_ms: u64,
    pub logical: u32,
    pub node: String,
}

impl Hlc {
    pub fn new(wall_ms: u64, logical: u32, node: impl Into<String>) -> Self {
        Self {
            wall_ms,
            logical,
            node: node.into(),
        }
    }

    /// Parse from canonical wire format `wall.logical.node`.
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.splitn(3, '.');
        let wall = parts.next()?.parse().ok()?;
        let logical = parts.next()?.parse().ok()?;
        let node = parts.next()?.to_string();
        Some(Self {
            wall_ms: wall,
            logical,
            node,
        })
    }
}

impl fmt::Display for Hlc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.wall_ms, self.logical, self.node)
    }
}

impl Ord for Hlc {
    fn cmp(&self, other: &Self) -> Ordering {
        self.wall_ms
            .cmp(&other.wall_ms)
            .then(self.logical.cmp(&other.logical))
            .then(self.node.cmp(&other.node))
    }
}

impl PartialOrd for Hlc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip() {
        let h = Hlc::new(1700000000000, 42, "node-a");
        let s = h.to_string();
        let parsed = Hlc::parse(&s).unwrap();
        assert_eq!(parsed, h);
    }

    #[test]
    fn ordering_walltime_then_logical_then_node() {
        let a = Hlc::new(10, 0, "a");
        let b = Hlc::new(10, 1, "a");
        let c = Hlc::new(10, 1, "b");
        let d = Hlc::new(11, 0, "a");
        assert!(a < b);
        assert!(b < c);
        assert!(c < d);
    }

    #[test]
    fn rejects_malformed() {
        assert!(Hlc::parse("garbage").is_none());
        assert!(Hlc::parse("10.x.node").is_none());
    }
}
