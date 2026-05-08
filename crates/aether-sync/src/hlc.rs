use aether_core::Hlc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Generates monotonic HLC values for a single node.
///
/// On each call, advances `wall_ms` to max(now, prev_wall) and bumps
/// `logical` if the wall clock did not advance, ensuring strict
/// monotonicity even under clock skew.
pub struct HlcGenerator {
    node: String,
    state: Mutex<(u64, u32)>,
}

impl HlcGenerator {
    pub fn new(node: impl Into<String>) -> Self {
        Self {
            node: node.into(),
            state: Mutex::new((0, 0)),
        }
    }

    pub fn next(&self) -> Hlc {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let mut g = self.state.lock().expect("HlcGenerator mutex poisoned");
        if now_ms > g.0 {
            g.0 = now_ms;
            g.1 = 0;
        } else {
            g.1 = g.1.saturating_add(1);
        }
        Hlc::new(g.0, g.1, self.node.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_strictly_monotonic_values() {
        let g = HlcGenerator::new("node-a");
        let a = g.next();
        let b = g.next();
        let c = g.next();
        assert!(a < b);
        assert!(b < c);
    }
}
