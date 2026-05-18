//! Conflict resolution — pure decision function on top of
//! [`super::reconcile::ConflictPolicy`].
//!
//! ## Why this is its own module
//! The `Reconciler` trait abstracts the *transport* (push to
//! server, pull since cursor); the actual question of "given
//! two divergent versions of the same row, which one wins?"
//! is a pure-function decision against the policy. Splitting
//! it lets the engine call `resolve()` without depending on
//! any transport, and lets property tests exercise the
//! convergence invariant without spinning up a mock server.
//!
//! ## Convergence
//! The headline correctness property: applying the same
//! resolver to (local, remote) on both sides of a network
//! partition yields the SAME final state on both sides. For
//! LWW this means symmetric ordering on HLC; for
//! reject-and-surface this means both sides agree to surface;
//! for server-transition this means both sides agree to defer
//! to the server. Property tests pin all three.

use crate::outbox::OutboxEntry;
use crate::reconcile::ConflictPolicy;
use std::cmp::Ordering;

/// Outcome of resolving a conflict between a local and a remote
/// version of the same `(entity, entity_id)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConflictOutcome {
    /// Local version wins — applier should keep local and
    /// discard remote.
    PreferLocal,
    /// Remote version wins — applier should overwrite local
    /// with remote.
    PreferRemote,
    /// Both versions need to be merged via Automerge (BOM /
    /// SOP CRDT documents). The engine forwards both payloads
    /// to `automerge_merge::merge_documents`.
    NeedsAutomergeMerge,
    /// Server-side state-transition function decides. Engine
    /// re-issues the local op as a `wo_transition(id, from,
    /// to)` RPC; rejection is the server's job.
    NeedsServerTransition,
    /// No automatic decision possible (counter / financial
    /// entries). Engine surfaces this to the UI for human
    /// resolution and refuses to silently apply either side.
    RequireUserAction,
}

impl ConflictOutcome {
    /// Whether this outcome lets the engine continue
    /// automatically. `RequireUserAction` is the only outcome
    /// that blocks; everything else has a deterministic next
    /// step.
    pub fn is_automatic(&self) -> bool {
        !matches!(self, ConflictOutcome::RequireUserAction)
    }
}

/// Resolve a conflict deterministically. Pure function — same
/// inputs always yield the same output. The convergence
/// property test relies on this being symmetric for LWW (swap
/// local/remote → swap PreferLocal/PreferRemote, never both
/// say "prefer mine").
pub fn resolve(
    local: &OutboxEntry,
    remote: &OutboxEntry,
    policy: &ConflictPolicy,
) -> ConflictOutcome {
    match policy {
        ConflictPolicy::LastWriterWinsHlc => match local.hlc_ts.cmp(&remote.hlc_ts) {
            // Local HLC is greater → local wrote later → local
            // wins. Equal HLCs (same wall + logical + node)
            // means the same op was observed twice — pick
            // local because it's already applied here; the
            // remote is a re-delivery.
            Ordering::Greater | Ordering::Equal => ConflictOutcome::PreferLocal,
            Ordering::Less => ConflictOutcome::PreferRemote,
        },
        ConflictPolicy::AutomergeMerge => ConflictOutcome::NeedsAutomergeMerge,
        ConflictPolicy::ServerTransition => ConflictOutcome::NeedsServerTransition,
        ConflictPolicy::RejectAndSurface => ConflictOutcome::RequireUserAction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outbox::Op;
    use aether_core::Hlc;

    fn entry(entity: &str, hlc: Hlc) -> OutboxEntry {
        OutboxEntry {
            op_id: format!("op-{}-{}", entity, hlc),
            entity: entity.into(),
            entity_id: "row-1".into(),
            op: Op::Update,
            payload: vec![],
            hlc_ts: hlc,
            parent_hlc: None,
            encrypted: false,
        }
    }

    #[test]
    fn lww_picks_greater_hlc() {
        let earlier = entry("x", Hlc::new(10, 0, "a"));
        let later = entry("x", Hlc::new(20, 0, "a"));
        assert_eq!(
            resolve(&later, &earlier, &ConflictPolicy::LastWriterWinsHlc),
            ConflictOutcome::PreferLocal
        );
        assert_eq!(
            resolve(&earlier, &later, &ConflictPolicy::LastWriterWinsHlc),
            ConflictOutcome::PreferRemote
        );
    }

    #[test]
    fn lww_equal_hlc_prefers_local_as_dedupe() {
        // Same op observed twice (a re-delivery from the server)
        // — keep what's already applied locally. This is the
        // idempotency contract: applying the same op twice
        // must not double-count.
        let a = entry("x", Hlc::new(10, 5, "a"));
        let b = entry("x", Hlc::new(10, 5, "a"));
        assert_eq!(
            resolve(&a, &b, &ConflictPolicy::LastWriterWinsHlc),
            ConflictOutcome::PreferLocal
        );
    }

    #[test]
    fn lww_breaks_ties_by_logical_then_node() {
        // HLC ordering is (wall_ms, logical, node). When wall_ms
        // ties, logical breaks the tie; when both tie, node
        // string lex order breaks. This is the canonical HLC
        // total order and the resolver must respect it.
        let same_wall_a = entry("x", Hlc::new(10, 0, "a"));
        let same_wall_b = entry("x", Hlc::new(10, 1, "a"));
        // Logical 1 > 0, so b wins.
        assert_eq!(
            resolve(
                &same_wall_a,
                &same_wall_b,
                &ConflictPolicy::LastWriterWinsHlc
            ),
            ConflictOutcome::PreferRemote
        );

        let same_all_a = entry("x", Hlc::new(10, 0, "alpha"));
        let same_all_b = entry("x", Hlc::new(10, 0, "beta"));
        // Node "beta" > "alpha", so b wins.
        assert_eq!(
            resolve(&same_all_a, &same_all_b, &ConflictPolicy::LastWriterWinsHlc),
            ConflictOutcome::PreferRemote
        );
    }

    #[test]
    fn automerge_policy_defers_to_merge() {
        let a = entry("boms", Hlc::new(10, 0, "a"));
        let b = entry("boms", Hlc::new(20, 0, "a"));
        // Doesn't matter which HLC is bigger — automerge
        // doesn't pick a winner, it merges both.
        assert_eq!(
            resolve(&a, &b, &ConflictPolicy::AutomergeMerge),
            ConflictOutcome::NeedsAutomergeMerge
        );
        assert_eq!(
            resolve(&b, &a, &ConflictPolicy::AutomergeMerge),
            ConflictOutcome::NeedsAutomergeMerge
        );
    }

    #[test]
    fn server_transition_defers_to_server() {
        let a = entry("work_orders", Hlc::new(10, 0, "a"));
        let b = entry("work_orders", Hlc::new(20, 0, "a"));
        assert_eq!(
            resolve(&a, &b, &ConflictPolicy::ServerTransition),
            ConflictOutcome::NeedsServerTransition
        );
    }

    #[test]
    fn reject_and_surface_blocks_automatic_progress() {
        // Inventory / finance: the only outcome that returns
        // `is_automatic() == false`. Engine surfaces to UI.
        let a = entry("materials", Hlc::new(10, 0, "a"));
        let b = entry("materials", Hlc::new(20, 0, "a"));
        let outcome = resolve(&a, &b, &ConflictPolicy::RejectAndSurface);
        assert_eq!(outcome, ConflictOutcome::RequireUserAction);
        assert!(!outcome.is_automatic());
    }

    #[test]
    fn every_non_user_outcome_is_automatic() {
        // The is_automatic predicate is what the engine branches
        // on. Pin that only RequireUserAction blocks.
        assert!(ConflictOutcome::PreferLocal.is_automatic());
        assert!(ConflictOutcome::PreferRemote.is_automatic());
        assert!(ConflictOutcome::NeedsAutomergeMerge.is_automatic());
        assert!(ConflictOutcome::NeedsServerTransition.is_automatic());
        assert!(!ConflictOutcome::RequireUserAction.is_automatic());
    }

    // ---------- convergence properties ----------

    /// Headline convergence test: for LWW, swapping local and
    /// remote yields the inverse outcome (PreferLocal ↔
    /// PreferRemote, Equal stays at PreferLocal because of the
    /// dedupe contract). This means both sides of a partition
    /// — A sees (a, b), B sees (b, a) — end up agreeing on
    /// which version wins.
    #[test]
    fn lww_resolution_is_symmetric_in_swap() {
        for (wa, wb) in [(10, 20), (20, 10), (5, 5), (100, 99)] {
            let a = entry("x", Hlc::new(wa, 0, "a"));
            let b = entry("x", Hlc::new(wb, 0, "b"));
            let ab = resolve(&a, &b, &ConflictPolicy::LastWriterWinsHlc);
            let ba = resolve(&b, &a, &ConflictPolicy::LastWriterWinsHlc);
            match (ab, ba) {
                (ConflictOutcome::PreferLocal, ConflictOutcome::PreferRemote)
                | (ConflictOutcome::PreferRemote, ConflictOutcome::PreferLocal) => {
                    // Symmetric for strict ordering.
                }
                (ConflictOutcome::PreferLocal, ConflictOutcome::PreferLocal) => {
                    // Equal HLC + node — both sides "prefer local"
                    // which is the dedupe path; both sides keep
                    // what they have and converge because the
                    // entries are byte-identical (same HLC means
                    // same op).
                    assert_eq!(a.hlc_ts, b.hlc_ts);
                }
                (l, r) => panic!("non-symmetric LWW outcome: {l:?} vs {r:?}"),
            }
        }
    }

    /// Convergence for non-LWW policies: swapping local/remote
    /// yields the SAME outcome, because none of those policies
    /// pick a "winner" based on a side-asymmetric comparison.
    #[test]
    fn non_lww_policies_are_swap_invariant() {
        let a = entry("x", Hlc::new(10, 0, "a"));
        let b = entry("x", Hlc::new(20, 0, "b"));
        for policy in [
            ConflictPolicy::AutomergeMerge,
            ConflictPolicy::ServerTransition,
            ConflictPolicy::RejectAndSurface,
        ] {
            let ab = resolve(&a, &b, &policy);
            let ba = resolve(&b, &a, &policy);
            assert_eq!(ab, ba, "non-LWW policy {policy:?} must be swap-invariant");
        }
    }

    #[test]
    fn three_way_chain_converges_to_latest_under_lww() {
        // Simulates client C receiving A's op, then B's op
        // arriving later. Apply A then B: result is B (later
        // HLC wins). Apply B then A: result is still B (A
        // would lose against the already-applied B).
        let a = entry("x", Hlc::new(10, 0, "a"));
        let b = entry("x", Hlc::new(20, 0, "b"));

        // Path 1: A → B
        let step1 = resolve(&a, &b, &ConflictPolicy::LastWriterWinsHlc);
        // After step1, B is now "local" (assuming step1 said
        // PreferRemote). Apply A as the new incoming.
        let local_after_step1 = if step1 == ConflictOutcome::PreferRemote {
            &b
        } else {
            &a
        };
        let step2 = resolve(local_after_step1, &a, &ConflictPolicy::LastWriterWinsHlc);
        assert_eq!(step2, ConflictOutcome::PreferLocal); // B stays

        // Path 2: B → A directly.
        let path2 = resolve(&b, &a, &ConflictPolicy::LastWriterWinsHlc);
        assert_eq!(path2, ConflictOutcome::PreferLocal); // B stays

        // Both paths land on "the row that ends up live is B"
        // — convergence.
    }
}
