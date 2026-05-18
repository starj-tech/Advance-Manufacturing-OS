//! Cross-crate integration tests for the Phase-2 hardware stack.
//!
//! This crate intentionally has no library content — its purpose
//! is to be a workspace member that can dev-depend on every
//! Phase-2 crate at once, hosting integration tests under
//! `tests/` that wouldn't fit cleanly inside any single crate
//! (each one would either need a circular dep or wouldn't get to
//! cross-cut the trait surfaces uniformly).
//!
//! See `tests/phase2_e2e.rs` for the scenarios.
