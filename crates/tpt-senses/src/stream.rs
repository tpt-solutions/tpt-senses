//! Streams of samples (spec §27): bounded buffering, backpressure, drop reporting, cancellation,
//! ordering and rate limiting.
//!
//! The sample types here are runtime-agnostic. [`SensorStream`] is built on
//! `futures_core::Stream`, and [`BoundedQueue`] is a synchronous alternative for polling loops
//! and `no_std` targets.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::num::NonZeroUsize;
use core::pin::Pin;
use core::task::{Context, Poll};

use futures_core::Stream;

use crate::{ClockMismatch, ClockSource, Duration, Timestamp};

/// A value tagged with the time it describes.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample<T> {
    /// When the value was acquired.
    pub timestamp: Timestamp,
    /// The value.
    pub value: T,
}

impl<T> Sample<T> {
    /// Creates a sample.
    #[must_use]
    pub const fn new(timestamp: Timestamp, value: T) -> Self {
        Self { timestamp, value }
    }

    /// Transforms the value, keeping the timestamp.
    #[must_use]
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Sample<U> {
        Sample {
            timestamp: self.timestamp,
            value: f(self.value),
        }
    }
}

/// What a bounded buffer does when it is full.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum BackpressurePolicy {
    /// Evict the oldest item to make room. Favours fresh data (the usual choice for sensors).
    #[default]
    DropOldest,
    /// Discard the incoming item. Favours continuity of the existing backlog.
    DropNewest,
}

/// The result of pushing an item into a [`BoundedQueue`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushOutcome {
    /// Stored without loss.
    Accepted,
    /// Stored; the oldest item was evicted to make room.
    DroppedOldest,
    /// Not stored; the incoming item was discarded.
    DroppedNewest,
}

/// A fixed-capacity FIFO with an explicit overflow policy and drop accounting.
///
/// This is the synchronous core of the streaming path: it needs no runtime and no allocator
/// beyond `alloc`.
#[derive(Clone, Debug)]
pub struct BoundedQueue<T> {
    items: VecDeque<T>,
    capacity: NonZeroUsize,
    policy: BackpressurePolicy,
    dropped_total: u64,
    dropped_since_take: u64,
}

impl<T> BoundedQueue<T> {
    /// Creates an empty queue.
    #[must_use]
    pub fn new(capacity: NonZeroUsize, policy: BackpressurePolicy) -> Self {
        Self {
            items: VecDeque::with_capacity(capacity.get()),
            capacity,
            policy,
            dropped_total: 0,
            dropped_since_take: 0,
        }
    }

    /// Pushes an item, applying the overflow policy when full.
    pub fn push(&mut self, item: T) -> PushOutcome {
        if self.items.len() < self.capacity.get() {
            self.items.push_back(item);
            return PushOutcome::Accepted;
        }
        self.dropped_total += 1;
        self.dropped_since_take += 1;
        match self.policy {
            BackpressurePolicy::DropOldest => {
                self.items.pop_front();
                self.items.push_back(item);
                PushOutcome::DroppedOldest
            }
            BackpressurePolicy::DropNewest => PushOutcome::DroppedNewest,
        }
    }

    /// Removes and returns the oldest item, if any.
    pub fn try_pop(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    /// Discards all buffered items. Drop counters are not changed.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Number of buffered items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if no items are buffered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Maximum number of buffered items.
    #[must_use]
    pub const fn capacity(&self) -> NonZeroUsize {
        self.capacity
    }

    /// The overflow policy.
    #[must_use]
    pub const fn policy(&self) -> BackpressurePolicy {
        self.policy
    }

    /// Total items dropped since creation.
    #[must_use]
    pub const fn dropped_total(&self) -> u64 {
        self.dropped_total
    }

    /// Items dropped since the last call to this method, then resets that count.
    ///
    /// Use this for periodic drop reports, so that each report covers a distinct interval.
    pub fn take_dropped(&mut self) -> u64 {
        core::mem::take(&mut self.dropped_since_take)
    }
}

/// A [`futures_core::Stream`] adapter that buffers items from an inner stream.
///
/// Each poll pulls up to `budget` items from the inner stream into a [`BoundedQueue`] (so the
/// inner stream can outpace the consumer), then yields one buffered item. Overflow follows the
/// [`BackpressurePolicy`] and is counted, never silent.
///
/// [`SensorStream::cancel`] ends the stream immediately and discards buffered items.
pub struct SensorStream<S: Stream> {
    inner: Option<S>,
    queue: BoundedQueue<S::Item>,
    budget: usize,
    cancelled: bool,
}

impl<S: Stream> SensorStream<S> {
    /// Wraps `inner` with a buffer of `capacity` items. The per-poll pull budget defaults to
    /// `capacity`.
    #[must_use]
    pub fn new(inner: S, capacity: NonZeroUsize, policy: BackpressurePolicy) -> Self {
        Self {
            inner: Some(inner),
            queue: BoundedQueue::new(capacity, policy),
            budget: capacity.get(),
            cancelled: false,
        }
    }

    /// Sets how many items may be pulled from the inner stream per poll (builder style).
    #[must_use]
    pub fn with_budget(mut self, budget: NonZeroUsize) -> Self {
        self.budget = budget.get();
        self
    }

    /// Total items dropped by the buffer since creation.
    #[must_use]
    pub const fn dropped_total(&self) -> u64 {
        self.queue.dropped_total()
    }

    /// Items dropped since the last call, then resets that count.
    pub fn take_dropped(&mut self) -> u64 {
        self.queue.take_dropped()
    }

    /// Cancels the stream: buffered items are discarded and no further items are produced.
    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.inner = None;
        self.queue.clear();
    }

    /// Whether [`SensorStream::cancel`] has been called.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

/// Items must be `Unpin`, which holds for all sample payloads in this crate.
impl<S> Stream for SensorStream<S>
where
    S: Stream + Unpin,
    S::Item: Unpin,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.cancelled {
            return Poll::Ready(None);
        }

        let mut pulled = 0;
        while let Some(inner) = this.inner.as_mut() {
            if pulled >= this.budget {
                break;
            }
            match Pin::new(inner).poll_next(cx) {
                Poll::Ready(Some(item)) => {
                    pulled += 1;
                    this.queue.push(item);
                }
                Poll::Ready(None) => this.inner = None,
                Poll::Pending => break,
            }
        }

        if let Some(item) = this.queue.try_pop() {
            return Poll::Ready(Some(item));
        }
        if this.inner.is_none() {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}

/// Returned by [`ReorderBuffer::push`] for a sample that was accepted into the buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ingest {
    /// Held until it can be emitted in timestamp order.
    Buffered,
    /// Arrived after a later point had already been emitted. Discarded and counted.
    Late,
}

/// Restores timestamp order for samples that arrive slightly out of order.
///
/// Samples are held until the watermark (`highest timestamp seen - lateness`) passes them. A
/// sample that arrives after its position has already been emitted is discarded and counted, so
/// downstream consumers always see a non-decreasing sequence.
///
/// All samples must come from one clock. A sample from another clock is rejected with
/// [`ClockMismatch`], because its order relative to the others is unknown.
#[derive(Clone, Debug)]
pub struct ReorderBuffer<T> {
    lateness: Duration,
    clock: Option<ClockSource>,
    pending: Vec<Sample<T>>,
    highest_seen: Option<Timestamp>,
    last_emitted: Option<Timestamp>,
    late_dropped: u64,
}

impl<T> ReorderBuffer<T> {
    /// Creates a buffer that tolerates samples up to `lateness` behind the newest one.
    #[must_use]
    pub const fn new(lateness: Duration) -> Self {
        Self {
            lateness,
            clock: None,
            pending: Vec::new(),
            highest_seen: None,
            last_emitted: None,
            late_dropped: 0,
        }
    }

    /// Adds a sample.
    ///
    /// # Errors
    ///
    /// Returns [`ClockMismatch`] if the sample's clock differs from earlier samples.
    pub fn push(&mut self, sample: Sample<T>) -> Result<Ingest, ClockMismatch> {
        let clock = sample.timestamp.clock();
        match self.clock {
            Some(expected) if expected != clock => {
                return Err(ClockMismatch {
                    expected,
                    found: clock,
                });
            }
            Some(_) => {}
            None => self.clock = Some(clock),
        }

        if let Some(emitted) = self.last_emitted {
            if sample.timestamp < emitted {
                self.late_dropped += 1;
                return Ok(Ingest::Late);
            }
        }

        let timestamp = sample.timestamp;
        let nanos = timestamp.nanos();
        let position = self
            .pending
            .partition_point(|s| s.timestamp.nanos() <= nanos);
        self.pending.insert(position, sample);

        if self.highest_seen.is_none_or(|high| nanos > high.nanos()) {
            self.highest_seen = Some(timestamp);
        }
        Ok(Ingest::Buffered)
    }

    /// Emits every sample at or before the watermark, in timestamp order.
    pub fn pop_ready(&mut self) -> Vec<Sample<T>> {
        let Some(high) = self.highest_seen else {
            return Vec::new();
        };
        let watermark = high.saturating_sub(self.lateness).nanos();
        let count = self
            .pending
            .partition_point(|s| s.timestamp.nanos() <= watermark);
        self.emit(count)
    }

    /// Emits everything still held, in timestamp order. Use at end of stream.
    pub fn flush(&mut self) -> Vec<Sample<T>> {
        let count = self.pending.len();
        self.emit(count)
    }

    fn emit(&mut self, count: usize) -> Vec<Sample<T>> {
        if count == 0 {
            return Vec::new();
        }
        let out: Vec<Sample<T>> = self.pending.drain(..count).collect();
        self.last_emitted = out.last().map(|s| s.timestamp);
        out
    }

    /// Samples discarded for arriving after their position had been emitted.
    #[must_use]
    pub const fn late_dropped(&self) -> u64 {
        self.late_dropped
    }

    /// Samples currently held.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

/// Limits a stream of timestamps to at most one accepted sample per `min_interval`.
#[derive(Clone, Debug, PartialEq)]
pub struct RateLimiter {
    min_interval: Duration,
    last_accepted: Option<Timestamp>,
}

impl RateLimiter {
    /// Creates a limiter that accepts at most one sample per `min_interval`.
    #[must_use]
    pub const fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            last_accepted: None,
        }
    }

    /// Returns `true` if a sample at `at` should be kept, and records it if so.
    ///
    /// Out-of-order samples (earlier than the last accepted one) are rejected.
    ///
    /// # Errors
    ///
    /// Returns [`ClockMismatch`] if `at` is on a different clock from earlier accepted samples.
    pub fn allow(&mut self, at: Timestamp) -> Result<bool, ClockMismatch> {
        if let Some(last) = self.last_accepted {
            if !last.is_same_clock(at) {
                return Err(ClockMismatch {
                    expected: last.clock(),
                    found: at.clock(),
                });
            }
            match at.duration_since(last) {
                Some(gap) if gap >= self.min_interval => {}
                _ => return Ok(false),
            }
        }
        self.last_accepted = Some(at);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(ns: u64) -> Timestamp {
        Timestamp::new(ns, ClockSource::Monotonic)
    }

    #[test]
    fn drop_oldest_keeps_newest_and_counts() {
        let mut q = BoundedQueue::new(
            NonZeroUsize::new(2).unwrap(),
            BackpressurePolicy::DropOldest,
        );
        assert_eq!(q.push(1), PushOutcome::Accepted);
        assert_eq!(q.push(2), PushOutcome::Accepted);
        assert_eq!(q.push(3), PushOutcome::DroppedOldest);
        assert_eq!(q.try_pop(), Some(2));
        assert_eq!(q.try_pop(), Some(3));
        assert_eq!(q.dropped_total(), 1);
    }

    #[test]
    fn drop_newest_keeps_backlog() {
        let mut q = BoundedQueue::new(
            NonZeroUsize::new(1).unwrap(),
            BackpressurePolicy::DropNewest,
        );
        q.push(1);
        assert_eq!(q.push(2), PushOutcome::DroppedNewest);
        assert_eq!(q.try_pop(), Some(1));
        assert_eq!(q.try_pop(), None);
    }

    #[test]
    fn take_dropped_reports_intervals() {
        let mut q = BoundedQueue::new(
            NonZeroUsize::new(1).unwrap(),
            BackpressurePolicy::DropOldest,
        );
        q.push(1);
        q.push(2);
        q.push(3);
        assert_eq!(q.take_dropped(), 2);
        assert_eq!(q.take_dropped(), 0);
        assert_eq!(q.dropped_total(), 2);
    }

    #[test]
    fn reorder_emits_in_timestamp_order() {
        let mut rb = ReorderBuffer::new(Duration::from_nanos(100));
        for ns in [10u64, 30, 20, 40, 200] {
            assert_eq!(rb.push(Sample::new(t(ns), ns)).unwrap(), Ingest::Buffered);
        }
        let out: Vec<u64> = rb.pop_ready().into_iter().map(|s| s.value).collect();
        assert_eq!(out, [10, 20, 30, 40]);
        let rest: Vec<u64> = rb.flush().into_iter().map(|s| s.value).collect();
        assert_eq!(rest, [200]);
    }

    #[test]
    fn reorder_discards_and_counts_late_samples() {
        let mut rb = ReorderBuffer::new(Duration::from_nanos(10));
        rb.push(Sample::new(t(100), 'a')).unwrap();
        rb.push(Sample::new(t(200), 'b')).unwrap();
        assert_eq!(rb.pop_ready().len(), 1); // emits 'a'; watermark is 190
        assert_eq!(rb.push(Sample::new(t(50), 'c')).unwrap(), Ingest::Late);
        assert_eq!(rb.late_dropped(), 1);
    }

    #[test]
    fn reorder_rejects_mixed_clocks() {
        let mut rb = ReorderBuffer::new(Duration::from_nanos(10));
        rb.push(Sample::new(t(1), ())).unwrap();
        let wall = Timestamp::new(2, ClockSource::Wall);
        assert!(rb.push(Sample::new(wall, ())).is_err());
    }

    #[test]
    fn rate_limiter_enforces_interval() {
        let mut rl = RateLimiter::new(Duration::from_nanos(100));
        assert_eq!(rl.allow(t(0)), Ok(true));
        assert_eq!(rl.allow(t(50)), Ok(false));
        assert_eq!(rl.allow(t(100)), Ok(true));
        assert_eq!(rl.allow(t(90)), Ok(false));
        assert!(rl.allow(Timestamp::new(300, ClockSource::Wall)).is_err());
    }
}
