//! Circuit breaker for downstream sinks.
//!
//! ## What this is for
//! V23's `BatchHandle` drops samples when the channel fills.
//! That's the right behavior under transient backpressure
//! (sink temporarily slow). It's the WRONG behavior under
//! persistent failure (sink permanently down) — silently
//! losing every sample without alerting the healing layer.
//!
//! The circuit breaker bridges the two:
//!
//!   * **Closed** — normal operation. Calls flow through.
//!     Failures increment a counter.
//!   * **Open** — N consecutive failures observed. Calls
//!     fail immediately without invoking the sink (saves the
//!     sink from being hammered). A timer counts down.
//!   * **HalfOpen** — timer elapsed. Next call probes the
//!     sink; success → Closed; failure → Open again with the
//!     timer reset.
//!
//! The state transitions are exactly the classic Hystrix /
//! Resilience4j pattern. This crate implements them as a
//! thin pure-Rust primitive callers wrap around their sink
//! function. The healing layer (PR #5) subscribes to
//! `breaker.state_changes()` and emits a symptom when state
//! transitions to Open, so a stuck sink becomes a healable
//! fault automatically.
//!
//! ## Why pure (no async dependency)
//! The breaker decides which path to take; it doesn't await
//! anything itself. Callers feed it `record_success` /
//! `record_failure` after their (sync or async) sink call.
//! That keeps the trait surface narrow and lets sync sinks
//! use it too.

use std::sync::atomic::{AtomicI64, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Default consecutive-failure threshold before opening.
/// Five misses is "the sink is consistently broken, not just
/// a transient hiccup."
pub const DEFAULT_FAILURE_THRESHOLD: u32 = 5;

/// Default cool-down before the breaker probes again. 30 s
/// balances "give the sink time to recover" against "don't
/// drop ten minutes of samples before noticing it came back."
pub const DEFAULT_COOLDOWN: Duration = Duration::from_secs(30);

/// State enum mapped to a single AtomicU8 for lock-free reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BreakerState {
    Closed = 0,
    Open = 1,
    HalfOpen = 2,
}

impl BreakerState {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => BreakerState::Closed,
            1 => BreakerState::Open,
            2 => BreakerState::HalfOpen,
            // Unreachable — only this module mutates the
            // backing atomic, and only with the above three
            // values. Fall back to Closed rather than panic
            // so a corrupted state bit can't take down the
            // process.
            _ => BreakerState::Closed,
        }
    }

    pub fn slug(&self) -> &'static str {
        match self {
            BreakerState::Closed => "closed",
            BreakerState::Open => "open",
            BreakerState::HalfOpen => "half-open",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BreakerConfig {
    pub failure_threshold: u32,
    pub cooldown: Duration,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: DEFAULT_FAILURE_THRESHOLD,
            cooldown: DEFAULT_COOLDOWN,
        }
    }
}

/// What the caller asks the breaker before invoking the sink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Decision {
    /// Sink call may proceed.
    Proceed,
    /// Breaker open — caller MUST skip the sink call. This is
    /// the only path where the breaker overrides the caller;
    /// honoring it is what saves a broken sink from being
    /// hammered.
    SkipShortCircuit,
    /// Breaker half-open — caller MAY probe the sink. Caller
    /// reports back via `record_success` / `record_failure`
    /// and the breaker transitions accordingly.
    Probe,
}

/// Lock-free circuit breaker. Cheap to clone (Arc'd
/// atomics inside); shared across the producer fleet so
/// every producer sees the same state.
#[derive(Clone)]
pub struct CircuitBreaker {
    inner: Arc<BreakerInner>,
    config: BreakerConfig,
}

struct BreakerInner {
    state: AtomicU8,
    consecutive_failures: AtomicU32,
    /// Wall-time (nanos since `BASE_INSTANT`) when the
    /// current Open window started. `i64::MIN` when not Open.
    opened_at_nanos: AtomicI64,
    /// Total count of state transitions — observable for
    /// dashboards and tests.
    transition_count: AtomicU64,
}

use std::sync::atomic::AtomicU32;
use std::sync::OnceLock;

/// Anchor instant the breaker uses for relative timestamps.
/// `Instant` isn't directly atomic-safe; we encode timestamps
/// as nanos since this anchor.
fn anchor_instant() -> Instant {
    static ANCHOR: OnceLock<Instant> = OnceLock::new();
    *ANCHOR.get_or_init(Instant::now)
}

fn now_nanos() -> i64 {
    anchor_instant().elapsed().as_nanos() as i64
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self::with_config(BreakerConfig::default())
    }

    pub fn with_config(config: BreakerConfig) -> Self {
        Self {
            inner: Arc::new(BreakerInner {
                state: AtomicU8::new(BreakerState::Closed as u8),
                consecutive_failures: AtomicU32::new(0),
                opened_at_nanos: AtomicI64::new(i64::MIN),
                transition_count: AtomicU64::new(0),
            }),
            config,
        }
    }

    /// Read the current state without modifying it. Lock-free.
    pub fn state(&self) -> BreakerState {
        BreakerState::from_u8(self.inner.state.load(Ordering::Acquire))
    }

    /// Decide whether the caller may invoke the sink. May
    /// transition Open→HalfOpen as a side effect if the
    /// cooldown has elapsed (this is the "probe time" path).
    pub fn decide(&self) -> Decision {
        loop {
            let cur = BreakerState::from_u8(self.inner.state.load(Ordering::Acquire));
            match cur {
                BreakerState::Closed => return Decision::Proceed,
                BreakerState::HalfOpen => return Decision::Probe,
                BreakerState::Open => {
                    let opened_at = self.inner.opened_at_nanos.load(Ordering::Acquire);
                    if opened_at == i64::MIN {
                        // Defensive: state is Open but the
                        // timestamp wasn't recorded. Treat as
                        // ready-to-probe.
                        if self.transition_to(BreakerState::Open, BreakerState::HalfOpen) {
                            return Decision::Probe;
                        }
                        continue;
                    }
                    let elapsed_nanos = now_nanos() - opened_at;
                    if elapsed_nanos < 0
                        || (elapsed_nanos as u128) < self.config.cooldown.as_nanos()
                    {
                        return Decision::SkipShortCircuit;
                    }
                    // Cooldown elapsed — try to promote to
                    // HalfOpen. Compare-exchange so two
                    // concurrent callers don't double-probe.
                    if self.transition_to(BreakerState::Open, BreakerState::HalfOpen) {
                        return Decision::Probe;
                    }
                    // Lost the race — re-read state and decide
                    // again.
                    continue;
                }
            }
        }
    }

    /// Caller reports the sink call succeeded. In Closed
    /// state this resets the failure counter; in HalfOpen
    /// it transitions back to Closed.
    pub fn record_success(&self) {
        let cur = BreakerState::from_u8(self.inner.state.load(Ordering::Acquire));
        match cur {
            BreakerState::Closed => {
                self.inner.consecutive_failures.store(0, Ordering::Release);
            }
            BreakerState::HalfOpen => {
                self.transition_to(BreakerState::HalfOpen, BreakerState::Closed);
                self.inner.consecutive_failures.store(0, Ordering::Release);
                self.inner
                    .opened_at_nanos
                    .store(i64::MIN, Ordering::Release);
            }
            BreakerState::Open => {
                // Shouldn't happen — caller would have been
                // short-circuited. Defensive: still reset.
                self.inner.consecutive_failures.store(0, Ordering::Release);
            }
        }
    }

    /// Caller reports the sink call failed. Increments the
    /// consecutive-failure counter; trips to Open if the
    /// threshold is reached. In HalfOpen state, a single
    /// failure re-opens the breaker.
    pub fn record_failure(&self) {
        let cur = BreakerState::from_u8(self.inner.state.load(Ordering::Acquire));
        match cur {
            BreakerState::Closed => {
                let new_fails = self
                    .inner
                    .consecutive_failures
                    .fetch_add(1, Ordering::AcqRel)
                    + 1;
                if new_fails >= self.config.failure_threshold
                    && self.transition_to(BreakerState::Closed, BreakerState::Open)
                {
                    self.inner
                        .opened_at_nanos
                        .store(now_nanos(), Ordering::Release);
                }
            }
            BreakerState::HalfOpen => {
                // The probe failed — back to Open. Reset the
                // cooldown timer so we don't probe immediately.
                if self.transition_to(BreakerState::HalfOpen, BreakerState::Open) {
                    self.inner
                        .opened_at_nanos
                        .store(now_nanos(), Ordering::Release);
                }
            }
            BreakerState::Open => {
                // Caller shouldn't be reporting — ignore.
            }
        }
    }

    /// Observable count of state transitions. Dashboards
    /// expose this as a counter.
    pub fn transition_count(&self) -> u64 {
        self.inner.transition_count.load(Ordering::Acquire)
    }

    /// Observable consecutive-failure count (in Closed
    /// state; meaningless in Open/HalfOpen but cheap to
    /// expose).
    pub fn consecutive_failures(&self) -> u32 {
        self.inner.consecutive_failures.load(Ordering::Acquire)
    }

    /// Force the breaker to a specific state. Used by
    /// healing-layer "operator manually reset" flows. Bumps
    /// the transition counter so the change shows up in
    /// dashboards.
    pub fn force_state(&self, new_state: BreakerState) {
        let old = self.inner.state.swap(new_state as u8, Ordering::AcqRel);
        if old != new_state as u8 {
            self.inner.transition_count.fetch_add(1, Ordering::AcqRel);
            if new_state == BreakerState::Closed {
                self.inner.consecutive_failures.store(0, Ordering::Release);
                self.inner
                    .opened_at_nanos
                    .store(i64::MIN, Ordering::Release);
            } else if new_state == BreakerState::Open {
                self.inner
                    .opened_at_nanos
                    .store(now_nanos(), Ordering::Release);
            }
        }
    }

    /// Atomic state transition. Returns true if the CAS
    /// succeeded (we won the race), false otherwise.
    fn transition_to(&self, from: BreakerState, to: BreakerState) -> bool {
        let ok = self
            .inner
            .state
            .compare_exchange(from as u8, to as u8, Ordering::AcqRel, Ordering::Acquire)
            .is_ok();
        if ok {
            self.inner.transition_count.fetch_add(1, Ordering::AcqRel);
        }
        ok
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_config() -> BreakerConfig {
        BreakerConfig {
            failure_threshold: 3,
            cooldown: Duration::from_millis(50),
        }
    }

    #[test]
    fn fresh_breaker_starts_closed() {
        let b = CircuitBreaker::with_config(small_config());
        assert_eq!(b.state(), BreakerState::Closed);
        assert_eq!(b.decide(), Decision::Proceed);
        assert_eq!(b.consecutive_failures(), 0);
    }

    #[test]
    fn n_failures_trip_to_open() {
        let b = CircuitBreaker::with_config(small_config());
        for _ in 0..3 {
            b.record_failure();
        }
        assert_eq!(b.state(), BreakerState::Open);
        assert_eq!(b.decide(), Decision::SkipShortCircuit);
    }

    #[test]
    fn one_success_resets_consecutive_failures_in_closed_state() {
        // The failure counter is CONSECUTIVE — a success
        // resets it. Pin this so a transient failure pair
        // followed by a recovery doesn't trip later.
        let b = CircuitBreaker::with_config(small_config());
        b.record_failure();
        b.record_failure();
        assert_eq!(b.consecutive_failures(), 2);
        b.record_success();
        assert_eq!(b.consecutive_failures(), 0);
        b.record_failure();
        // Counter is back to 1, NOT 3 — not yet tripped.
        assert_eq!(b.state(), BreakerState::Closed);
    }

    #[test]
    fn open_breaker_transitions_to_half_open_after_cooldown() {
        let b = CircuitBreaker::with_config(small_config());
        for _ in 0..3 {
            b.record_failure();
        }
        assert_eq!(b.state(), BreakerState::Open);
        std::thread::sleep(Duration::from_millis(80));
        // Next decision after cooldown promotes to HalfOpen.
        assert_eq!(b.decide(), Decision::Probe);
        assert_eq!(b.state(), BreakerState::HalfOpen);
    }

    #[test]
    fn successful_probe_in_half_open_closes_breaker() {
        let b = CircuitBreaker::with_config(small_config());
        for _ in 0..3 {
            b.record_failure();
        }
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(b.decide(), Decision::Probe);
        b.record_success();
        assert_eq!(b.state(), BreakerState::Closed);
        // Failure counter reset on close.
        assert_eq!(b.consecutive_failures(), 0);
    }

    #[test]
    fn failed_probe_in_half_open_reopens_breaker() {
        let b = CircuitBreaker::with_config(small_config());
        for _ in 0..3 {
            b.record_failure();
        }
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(b.decide(), Decision::Probe);
        b.record_failure();
        assert_eq!(b.state(), BreakerState::Open);
        // Immediate decision returns SkipShortCircuit (the
        // cooldown timer was reset on the re-open).
        assert_eq!(b.decide(), Decision::SkipShortCircuit);
    }

    #[test]
    fn force_state_reset_clears_counters() {
        let b = CircuitBreaker::with_config(small_config());
        for _ in 0..3 {
            b.record_failure();
        }
        assert_eq!(b.state(), BreakerState::Open);
        b.force_state(BreakerState::Closed);
        assert_eq!(b.state(), BreakerState::Closed);
        assert_eq!(b.consecutive_failures(), 0);
    }

    #[test]
    fn transition_count_increments_per_state_change() {
        let b = CircuitBreaker::with_config(small_config());
        let before = b.transition_count();
        for _ in 0..3 {
            b.record_failure();
        }
        // One transition: Closed → Open.
        assert_eq!(b.transition_count(), before + 1);
    }

    #[test]
    fn breaker_clone_shares_state() {
        // The breaker is shared across the producer fleet.
        // A failure observed via one handle MUST be visible
        // via every clone.
        let b1 = CircuitBreaker::with_config(small_config());
        let b2 = b1.clone();
        for _ in 0..3 {
            b1.record_failure();
        }
        assert_eq!(b2.state(), BreakerState::Open);
    }

    #[test]
    fn state_slug_kebab_case() {
        assert_eq!(BreakerState::Closed.slug(), "closed");
        assert_eq!(BreakerState::Open.slug(), "open");
        assert_eq!(BreakerState::HalfOpen.slug(), "half-open");
    }

    #[test]
    fn decide_in_open_state_before_cooldown_returns_short_circuit() {
        let b = CircuitBreaker::with_config(BreakerConfig {
            failure_threshold: 1,
            cooldown: Duration::from_secs(60),
        });
        b.record_failure();
        assert_eq!(b.state(), BreakerState::Open);
        // Cooldown is 60s — well past test runtime.
        assert_eq!(b.decide(), Decision::SkipShortCircuit);
        assert_eq!(b.decide(), Decision::SkipShortCircuit);
    }

    #[test]
    fn consecutive_failures_capped_by_threshold_then_trips() {
        // Pin the boundary: failure_threshold = 3 means
        // exactly 3 failures trip (not 2, not 4).
        let b = CircuitBreaker::with_config(small_config());
        b.record_failure();
        b.record_failure();
        assert_eq!(b.state(), BreakerState::Closed);
        b.record_failure(); // 3rd
        assert_eq!(b.state(), BreakerState::Open);
    }
}
