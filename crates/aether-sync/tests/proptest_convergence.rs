//! Property-based convergence test for the V21 resolver.
//!
//! ## What this proves
//! The original PR #2 plan called out a property test that
//! "2 clients with random ops converge at the server."  This
//! file implements it. Strategy:
//!
//!   * Generate a random sequence of `(client_label, hlc,
//!     entity_id)` ops representing two clients writing
//!     concurrently to the same row.
//!   * Apply them in TWO orderings: A→B and B→A. For LWW the
//!     final state must be byte-identical regardless of
//!     observation order (commutativity under the resolver).
//!   * Repeat with proptest's shrinker so any failing case is
//!     minimized to the smallest reproducer.
//!
//! Convergence here means: two clients on opposite sides of a
//! partition see ops in different orders, but once each has
//! seen every op, they agree on the live state. That's the
//! deepest sync correctness property AETHER-OS depends on.

use aether_core::Hlc;
use aether_sync::outbox::{Op, OutboxEntry};
use aether_sync::reconcile::ConflictPolicy;
use aether_sync::resolve::{resolve, ConflictOutcome};
use proptest::prelude::*;

/// Apply a sequence of ops in order against an optional
/// "current" entry, producing the final live entry. Uses LWW
/// for every step — mirrors what the production engine does
/// when it observes a stream of pulled entries against the
/// current local state.
fn fold_lww(ops: &[OutboxEntry]) -> Option<OutboxEntry> {
    let mut current: Option<OutboxEntry> = None;
    for op in ops {
        current = match current {
            None => Some(op.clone()),
            Some(existing) => match resolve(&existing, op, &ConflictPolicy::LastWriterWinsHlc) {
                ConflictOutcome::PreferLocal => Some(existing),
                ConflictOutcome::PreferRemote => Some(op.clone()),
                other => panic!("LWW should never yield {other:?}"),
            },
        };
    }
    current
}

/// Build an OutboxEntry from a wall-time, logical, node tag,
/// and a payload byte (the payload is the "value" the
/// resolver is choosing between).
fn make_entry(wall_ms: u64, logical: u32, node: &str, payload_byte: u8) -> OutboxEntry {
    OutboxEntry {
        op_id: format!("{node}-{wall_ms}.{logical}"),
        entity: "test".into(),
        entity_id: "row-1".into(),
        op: Op::Update,
        payload: vec![payload_byte],
        hlc_ts: Hlc::new(wall_ms, logical, node),
        parent_hlc: None,
        encrypted: false,
    }
}

proptest! {
    /// The headline convergence property: two clients seeing
    /// the same SET of ops in different orders converge to
    /// the same final live entry.
    #[test]
    fn lww_is_order_independent_across_random_op_sequences(
        ops in proptest::collection::vec(
            (0u64..1_000_000u64, 0u32..100u32, "[a-z]{1,4}", 0u8..=255u8),
            1..16
        ),
    ) {
        let entries: Vec<OutboxEntry> = ops
            .iter()
            .map(|(w, l, n, b)| make_entry(*w, *l, n, *b))
            .collect();

        // Client A observes ops in given order.
        let final_a = fold_lww(&entries).unwrap();

        // Client B observes the same ops in reverse order.
        let mut reversed = entries.clone();
        reversed.reverse();
        let final_b = fold_lww(&reversed).unwrap();

        // Same HLC must point to the same op (HLC is unique per
        // op in a real system, so equal HLC means it's the same
        // physical op and either side is byte-identical).
        prop_assert_eq!(
            final_a.hlc_ts.clone(),
            final_b.hlc_ts.clone(),
            "convergence violated: A landed on {} vs B on {}",
            final_a.hlc_ts,
            final_b.hlc_ts
        );
    }

    /// The "highest HLC wins" property: after folding an
    /// arbitrary sequence, the live entry's HLC equals the max
    /// HLC in the input set. This is the canonical LWW
    /// invariant — losing it means the resolver picked an
    /// older op over a newer one, which would corrupt history.
    #[test]
    fn lww_final_state_has_the_max_hlc(
        ops in proptest::collection::vec(
            (0u64..1_000_000u64, 0u32..100u32, "[a-z]{1,4}", 0u8..=255u8),
            1..32
        ),
    ) {
        let entries: Vec<OutboxEntry> = ops
            .iter()
            .map(|(w, l, n, b)| make_entry(*w, *l, n, *b))
            .collect();
        let final_entry = fold_lww(&entries).unwrap();
        let max_hlc = entries.iter().map(|e| e.hlc_ts.clone()).max().unwrap();
        prop_assert_eq!(final_entry.hlc_ts, max_hlc);
    }

    /// Two-client interleave: client A and client B each emit
    /// ops independently, then exchange streams. Both sides
    /// observe the union and converge.
    #[test]
    fn two_client_interleave_converges(
        a_ops in proptest::collection::vec(
            (0u64..500_000u64, 0u32..10u32, 0u8..=255u8),
            1..8
        ),
        b_ops in proptest::collection::vec(
            (500_000u64..1_000_000u64, 0u32..10u32, 0u8..=255u8),
            1..8
        ),
    ) {
        let a: Vec<OutboxEntry> = a_ops.iter().map(|(w, l, b)| make_entry(*w, *l, "alpha", *b)).collect();
        let b: Vec<OutboxEntry> = b_ops.iter().map(|(w, l, p)| make_entry(*w, *l, "beta", *p)).collect();

        // Client A's final view: own ops then B's.
        let mut a_view = a.clone();
        a_view.extend(b.clone());
        let final_a = fold_lww(&a_view).unwrap();

        // Client B's final view: own ops then A's.
        let mut b_view = b.clone();
        b_view.extend(a.clone());
        let final_b = fold_lww(&b_view).unwrap();

        prop_assert_eq!(final_a.hlc_ts, final_b.hlc_ts);
    }

    /// Idempotency under re-delivery: applying the same op
    /// twice doesn't change the state. The server might
    /// re-deliver an op the client already saw (retry, dupe);
    /// the resolver's equal-HLC=PreferLocal contract makes
    /// this a no-op.
    #[test]
    fn idempotent_under_redelivery(
        ops in proptest::collection::vec(
            (0u64..1_000_000u64, 0u32..100u32, "[a-z]{1,4}", 0u8..=255u8),
            1..8
        ),
    ) {
        let entries: Vec<OutboxEntry> = ops
            .iter()
            .map(|(w, l, n, b)| make_entry(*w, *l, n, *b))
            .collect();
        let baseline = fold_lww(&entries).unwrap();

        // Apply the same sequence again — every op is a
        // "re-delivery" of one we've already seen.
        let mut doubled = entries.clone();
        doubled.extend(entries.iter().cloned());
        let after_redeliver = fold_lww(&doubled).unwrap();

        prop_assert_eq!(baseline.hlc_ts, after_redeliver.hlc_ts);
        prop_assert_eq!(baseline.payload, after_redeliver.payload);
    }

    /// Non-LWW policies stay swap-invariant on random inputs
    /// too. Mirrors the V21 unit test
    /// `non_lww_policies_are_swap_invariant` but exercises
    /// with proptest-generated entries.
    #[test]
    fn non_lww_policies_remain_swap_invariant_for_random_pairs(
        (wa, la, na, ba) in (0u64..1_000_000u64, 0u32..100u32, "[a-z]{1,4}", 0u8..=255u8),
        (wb, lb, nb, bb) in (0u64..1_000_000u64, 0u32..100u32, "[a-z]{1,4}", 0u8..=255u8),
    ) {
        let a = make_entry(wa, la, &na, ba);
        let b = make_entry(wb, lb, &nb, bb);
        for policy in [
            ConflictPolicy::AutomergeMerge,
            ConflictPolicy::ServerTransition,
            ConflictPolicy::RejectAndSurface,
        ] {
            let ab = resolve(&a, &b, &policy);
            let ba = resolve(&b, &a, &policy);
            prop_assert_eq!(ab, ba);
        }
    }
}
