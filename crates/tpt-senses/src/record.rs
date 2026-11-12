//! Recording and replay (spec §28, §29).
//!
//! The core defines a storage-agnostic [`RecordSink`] and an in-memory [`Recording`]. Concrete
//! on-disk formats belong in a separate record crate. A recording enforces one clock and
//! non-decreasing time, so a replay is always a well-defined sequence.

use alloc::vec::Vec;
use core::fmt;

use crate::{ClockMismatch, ClockSource, Sample, Timestamp};

/// Why a sample could not be recorded.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordError {
    /// The sample is on a different clock from the recording.
    ClockMismatch(ClockMismatch),
    /// The sample is earlier than the last recorded sample.
    OutOfOrder {
        /// Timestamp of the last recorded sample, in nanoseconds.
        last_nanos: u64,
        /// Timestamp of the rejected sample, in nanoseconds.
        found_nanos: u64,
    },
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClockMismatch(m) => write!(f, "{m}"),
            Self::OutOfOrder {
                last_nanos,
                found_nanos,
            } => {
                write!(
                    f,
                    "sample at {found_nanos} ns is earlier than last at {last_nanos} ns"
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RecordError {}

/// Anything that accepts a stream of samples: memory, a file, a network log.
pub trait RecordSink<T> {
    /// Appends one sample.
    ///
    /// # Errors
    ///
    /// Returns [`RecordError`] if the sample violates the sink's ordering or clock rules.
    fn write(&mut self, sample: Sample<T>) -> Result<(), RecordError>;
}

/// An in-memory, time-ordered, single-clock recording.
#[derive(Clone, Debug, PartialEq)]
pub struct Recording<T> {
    clock: Option<ClockSource>,
    samples: Vec<Sample<T>>,
}

impl<T> Default for Recording<T> {
    fn default() -> Self {
        Self {
            clock: None,
            samples: Vec::new(),
        }
    }
}

impl<T> Recording<T> {
    /// An empty recording.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of samples.
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Returns `true` if the recording holds no samples.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// All samples, in time order.
    #[must_use]
    pub fn samples(&self) -> &[Sample<T>] {
        &self.samples
    }

    /// The clock this recording uses, once the first sample has been written.
    #[must_use]
    pub const fn clock(&self) -> Option<ClockSource> {
        self.clock
    }

    /// Index of the first sample at or after `at`. Equals `len()` if none.
    #[must_use]
    pub fn seek(&self, at: Timestamp) -> usize {
        self.samples
            .partition_point(|s| s.timestamp.nanos() < at.nanos())
    }

    /// Samples in the half-open interval `[start, end)`.
    #[must_use]
    pub fn range(&self, start: Timestamp, end: Timestamp) -> &[Sample<T>] {
        let from = self.seek(start);
        let to = self.seek(end).max(from);
        &self.samples[from..to]
    }

    /// Iterates over the samples in order, for replay.
    pub fn replay(&self) -> impl Iterator<Item = &Sample<T>> + '_ {
        self.samples.iter()
    }
}

impl<T> RecordSink<T> for Recording<T> {
    fn write(&mut self, sample: Sample<T>) -> Result<(), RecordError> {
        let clock = sample.timestamp.clock();
        match self.clock {
            Some(expected) if expected != clock => {
                return Err(RecordError::ClockMismatch(ClockMismatch {
                    expected,
                    found: clock,
                }));
            }
            Some(_) => {}
            None => self.clock = Some(clock),
        }
        if let Some(last) = self.samples.last() {
            if sample.timestamp.nanos() < last.timestamp.nanos() {
                return Err(RecordError::OutOfOrder {
                    last_nanos: last.timestamp.nanos(),
                    found_nanos: sample.timestamp.nanos(),
                });
            }
        }
        self.samples.push(sample);
        Ok(())
    }
}

/// Replays a recording through a deterministic function, collecting the outputs.
///
/// Determinism is a property of `process`: with the same recording and the same function, the
/// output is the same every time (spec §28). This function adds no randomness or wall-clock
/// reads of its own.
pub fn replay_through<T, U, F>(recording: &Recording<T>, process: F) -> Vec<U>
where
    F: FnMut(&Sample<T>) -> Option<U>,
{
    recording.replay().filter_map(process).collect()
}

#[cfg(test)]
#[allow(
    clippy::cast_precision_loss,
    reason = "test sample values are small integers"
)]
mod tests {
    use super::*;

    fn t(ns: u64) -> Timestamp {
        Timestamp::new(ns, ClockSource::Hardware)
    }

    #[test]
    fn recording_enforces_order_and_clock() {
        let mut rec = Recording::new();
        rec.write(Sample::new(t(10), 1)).unwrap();
        assert!(matches!(
            rec.write(Sample::new(t(5), 2)),
            Err(RecordError::OutOfOrder { .. })
        ));
        let wall = Timestamp::new(20, ClockSource::Wall);
        assert!(matches!(
            rec.write(Sample::new(wall, 3)),
            Err(RecordError::ClockMismatch(_))
        ));
        assert_eq!(rec.len(), 1);
    }

    #[test]
    fn seek_and_range_select_by_time() {
        let mut rec = Recording::new();
        for ns in [0u64, 10, 20, 30] {
            rec.write(Sample::new(t(ns), ns)).unwrap();
        }
        assert_eq!(rec.seek(t(15)), 2);
        let window: Vec<u64> = rec.range(t(10), t(30)).iter().map(|s| s.value).collect();
        assert_eq!(window, [10, 20]);
    }

    #[test]
    fn replay_is_deterministic() {
        let mut rec = Recording::new();
        for ns in 0..5u64 {
            rec.write(Sample::new(t(ns), ns as f64)).unwrap();
        }
        let run = |rec: &Recording<f64>| {
            replay_through(rec, |s| (s.value >= 2.0).then_some(s.value * 10.0))
        };
        assert_eq!(run(&rec), run(&rec));
        assert_eq!(run(&rec), [20.0, 30.0, 40.0]);
    }
}
