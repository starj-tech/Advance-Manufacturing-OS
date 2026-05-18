//! Backpressure-aware batch aggregator.
//!
//! ## Why this exists
//! PR #3's telemetry pipeline (OPC-UA / MQTT subscriptions) can
//! produce tag samples faster than SQLite can ingest single
//! rows. The fix has two halves:
//!
//!   1. A bounded channel between producer and sink — caps
//!      memory if the sink stalls. Default 4096 entries.
//!   2. A flush strategy that batches: every N items OR every
//!      T milliseconds, whichever comes first. The combined
//!      INSERT amortizes the SQLite write-amplification.
//!
//! Producers call [`BatchHandle::send`] from any task. Returns
//! `false` if the channel is full (caller's right move is to
//! increment its own drop-counter and move on — the
//! aggregator's [`BatchHandle::dropped_count`] also tracks the
//! tally globally for ops dashboards).
//!
//! ## Why drop on full, not block
//! Telemetry is intrinsically lossy under sustained overload
//! — blocking the producer would propagate the sink's slowness
//! into the protocol layer's polling loop and corrupt sample
//! timing. Dropping with a counter is the IEC 62443 / OPC-UA
//! recommended behavior: prefer fresh samples over old, and
//! surface the loss in metrics.
//!
//! ## Lifecycle
//! The spawned background task lives as long as the handle
//! has at least one clone. When all handles drop, the channel
//! closes, the task drains the residual buffer with one final
//! flush, and exits cleanly.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Default per-channel capacity. 4096 matches the PR #3 plan
/// — at 1 KiB average sample size, ~4 MiB peak buffer, sized
/// for the cheapest industrial gateways we target.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 4096;

/// Default batch size. 256 INSERTs per transaction is the
/// sweet spot for SQLite WAL mode on typical SSDs (linear in
/// throughput up to ~256, sub-linear past 1024).
pub const DEFAULT_BATCH_MAX_ITEMS: usize = 256;

/// Default flush interval. 200 ms matches PR #3's "200 ms
/// window aggregator" guideline. Short enough that UI lag is
/// imperceptible; long enough that batching wins are real.
pub const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug)]
pub struct BatchConfig {
    pub channel_capacity: usize,
    pub batch_max_items: usize,
    pub flush_interval: Duration,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
            batch_max_items: DEFAULT_BATCH_MAX_ITEMS,
            flush_interval: DEFAULT_FLUSH_INTERVAL,
        }
    }
}

/// Producer-side handle. Cheap to clone — clones share the
/// same channel and dropped-counter, so a fleet of producers
/// converges on one aggregator.
#[derive(Clone)]
pub struct BatchHandle<T> {
    tx: mpsc::Sender<T>,
    dropped: Arc<AtomicU64>,
}

impl<T> BatchHandle<T> {
    /// Attempt to enqueue. Returns `true` on success, `false`
    /// if the channel is full (item is dropped and the
    /// internal counter is incremented). Non-blocking.
    pub fn send(&self, item: T) -> bool {
        match self.tx.try_send(item) {
            Ok(()) => true,
            Err(_) => {
                // Both Full and Closed surface as Err. We
                // treat both as "drop and count" — Closed
                // means the consumer task exited, which is
                // worth flagging via the same metric so the
                // ops dashboard sees the lost samples.
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    /// Cumulative samples dropped since the handle was
    /// created. Reads aren't synchronized — eventually
    /// consistent across producers — which is the right
    /// semantic for an ops metric.
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Whether the underlying channel is closed (consumer
    /// task has exited). After close, every `send` increments
    /// `dropped_count` and returns false.
    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}

/// Spawn a background batcher and return the producer handle.
///
/// The `flush` closure is invoked once per batch (size-
/// triggered or time-triggered, whichever fires first). It
/// receives the accumulated `Vec<T>` and runs to completion
/// before the batcher resumes draining. If `flush` is slow,
/// the channel fills and producers start seeing
/// `send` return false — which is the intended backpressure
/// signal.
pub fn spawn_batcher<T, F, Fut>(cfg: BatchConfig, flush: F) -> BatchHandle<T>
where
    T: Send + 'static,
    F: Fn(Vec<T>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (tx, mut rx) = mpsc::channel::<T>(cfg.channel_capacity);
    let dropped = Arc::new(AtomicU64::new(0));

    tokio::spawn(async move {
        let mut buffer: Vec<T> = Vec::with_capacity(cfg.batch_max_items);
        loop {
            // Race: receive next item against the flush
            // deadline. Whichever fires first decides the
            // next action.
            let next = tokio::time::timeout(cfg.flush_interval, rx.recv()).await;
            match next {
                Ok(Some(item)) => {
                    buffer.push(item);
                    // Drain anything that's already queued
                    // without waiting — keeps the buffer
                    // tight under a producer burst.
                    while buffer.len() < cfg.batch_max_items {
                        match rx.try_recv() {
                            Ok(more) => buffer.push(more),
                            Err(_) => break,
                        }
                    }
                    if buffer.len() >= cfg.batch_max_items {
                        let drained =
                            std::mem::replace(&mut buffer, Vec::with_capacity(cfg.batch_max_items));
                        flush(drained).await;
                    }
                }
                Ok(None) => {
                    // Channel closed by all senders dropping.
                    // Final drain.
                    if !buffer.is_empty() {
                        flush(buffer).await;
                    }
                    return;
                }
                Err(_) => {
                    // Timeout fired. Flush whatever we have;
                    // if buffer is empty, skip the flush
                    // (don't churn the sink).
                    if !buffer.is_empty() {
                        let drained =
                            std::mem::replace(&mut buffer, Vec::with_capacity(cfg.batch_max_items));
                        flush(drained).await;
                    }
                }
            }
        }
    });

    BatchHandle { tx, dropped }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Instant;

    /// Test sink: shared Vec accumulating every batch the
    /// flush closure observes. Wrapped in Arc<Mutex<>> so the
    /// closure can capture and the test can read.
    type Sink = Arc<Mutex<Vec<Vec<u32>>>>;

    fn fresh_sink() -> Sink {
        Arc::new(Mutex::new(Vec::new()))
    }

    fn flush_into(
        sink: Sink,
    ) -> impl Fn(Vec<u32>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        move |batch: Vec<u32>| {
            let sink = sink.clone();
            Box::pin(async move {
                sink.lock().unwrap().push(batch);
            })
        }
    }

    fn small_config() -> BatchConfig {
        BatchConfig {
            channel_capacity: 32,
            batch_max_items: 4,
            flush_interval: Duration::from_millis(50),
        }
    }

    #[tokio::test]
    async fn size_triggered_flush_fires_at_batch_max_items() {
        // Push exactly batch_max_items quickly — the size
        // trigger fires before the timer does.
        let sink = fresh_sink();
        let handle = spawn_batcher(small_config(), flush_into(sink.clone()));

        for i in 0..4 {
            assert!(handle.send(i));
        }
        // Wait long enough for the spawn'd task to drain +
        // flush, but well under the 50 ms timer.
        tokio::time::sleep(Duration::from_millis(20)).await;
        let batches = sink.lock().unwrap().clone();
        assert_eq!(batches.len(), 1, "size trigger should produce one batch");
        assert_eq!(batches[0], vec![0, 1, 2, 3]);
    }

    #[tokio::test]
    async fn time_triggered_flush_fires_at_interval_with_partial_batch() {
        // Push fewer than batch_max_items, then wait past the
        // flush interval. Timer fires the flush.
        let sink = fresh_sink();
        let handle = spawn_batcher(small_config(), flush_into(sink.clone()));

        handle.send(7);
        handle.send(8);
        tokio::time::sleep(Duration::from_millis(120)).await;
        let batches = sink.lock().unwrap().clone();
        assert_eq!(batches.len(), 1, "time trigger should produce one batch");
        assert_eq!(batches[0], vec![7, 8]);
    }

    #[tokio::test]
    async fn empty_timeout_does_not_invoke_flush() {
        // No items sent: timer fires but flush isn't called
        // (sink stays empty). Pins that we don't churn the
        // sink under a quiet producer.
        let sink = fresh_sink();
        let _handle: BatchHandle<u32> = spawn_batcher(small_config(), flush_into(sink.clone()));
        tokio::time::sleep(Duration::from_millis(180)).await;
        assert!(sink.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn channel_full_drops_with_counter_incremented() {
        // Stall the flush so the channel fills, then count
        // drops.
        let sink = fresh_sink();
        let sink_clone = sink.clone();
        let cfg = BatchConfig {
            channel_capacity: 4,
            batch_max_items: 4,
            // Long timer so it doesn't drain.
            flush_interval: Duration::from_secs(60),
        };
        let handle = spawn_batcher(cfg, move |batch: Vec<u32>| {
            let sink = sink_clone.clone();
            Box::pin(async move {
                // Pretend the sink is slow.
                tokio::time::sleep(Duration::from_secs(60)).await;
                sink.lock().unwrap().push(batch);
            })
        });

        let mut accepted = 0;
        let mut dropped = 0;
        for i in 0..32 {
            if handle.send(i) {
                accepted += 1;
            } else {
                dropped += 1;
            }
        }
        // Some accepted (up to channel cap), the rest dropped.
        assert!(accepted >= 1);
        assert!(dropped >= 1);
        assert_eq!(handle.dropped_count(), dropped as u64);
    }

    #[tokio::test]
    async fn handle_clone_shares_dropped_counter() {
        // Many producers, one aggregator — the dropped tally
        // is global across clones.
        let sink = fresh_sink();
        let cfg = BatchConfig {
            channel_capacity: 2,
            batch_max_items: 4,
            flush_interval: Duration::from_secs(60),
        };
        let handle = spawn_batcher(cfg, |_: Vec<u32>| {
            Box::pin(tokio::time::sleep(Duration::from_secs(60)))
        });
        let cloned = handle.clone();

        // Fill via the clone, then drop more via original.
        for i in 0..10 {
            cloned.send(i);
        }
        for i in 10..20 {
            handle.send(i);
        }

        // Both handles report the same global tally.
        let a = handle.dropped_count();
        let b = cloned.dropped_count();
        assert_eq!(a, b);
        assert!(a > 0, "some drops should have occurred under tight cap");

        let _ = sink;
    }

    #[tokio::test]
    async fn final_drain_flushes_remaining_buffer_on_close() {
        // Dropping every handle closes the channel; the task
        // does one last flush before exiting. Pin the
        // residual-flush semantic so a graceful shutdown
        // doesn't lose pending samples.
        let sink = fresh_sink();
        let cfg = BatchConfig {
            channel_capacity: 32,
            batch_max_items: 16, // higher than what we'll send
            flush_interval: Duration::from_secs(60), // never fires
        };
        let handle = spawn_batcher(cfg, flush_into(sink.clone()));
        handle.send(1);
        handle.send(2);
        handle.send(3);

        // Drop the sole handle → channel close → final flush.
        drop(handle);
        tokio::time::sleep(Duration::from_millis(50)).await;

        let batches = sink.lock().unwrap().clone();
        assert_eq!(batches.len(), 1, "final drain produces one batch");
        assert_eq!(batches[0], vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn ordering_preserved_within_a_batch() {
        // The FIFO contract — batches reflect producer
        // ordering. Critical for telemetry where sample
        // sequence is meaningful (rising/falling edges).
        let sink = fresh_sink();
        let handle = spawn_batcher(small_config(), flush_into(sink.clone()));

        for i in 0..4u32 {
            handle.send(i * 10);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
        let batches = sink.lock().unwrap().clone();
        assert_eq!(batches[0], vec![0, 10, 20, 30]);
    }

    #[tokio::test]
    async fn flush_interval_is_approximately_observed() {
        // Sanity-check the timer fires roughly when configured
        // — not exactly (tokio time isn't real-time), but
        // within an order of magnitude so a regression to
        // 2-second flush gets caught.
        let sink = fresh_sink();
        let cfg = BatchConfig {
            channel_capacity: 32,
            batch_max_items: 1024,
            flush_interval: Duration::from_millis(100),
        };
        let handle = spawn_batcher(cfg, flush_into(sink.clone()));
        handle.send(1);
        let start = Instant::now();
        loop {
            if !sink.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
            if start.elapsed() > Duration::from_secs(1) {
                panic!("flush did not fire within 1s budget");
            }
        }
        // Should fire within ~100 ms ± slack.
        assert!(start.elapsed() < Duration::from_millis(500));
    }
}
