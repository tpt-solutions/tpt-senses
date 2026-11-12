//! Temporal model and alignment (spec §17).
//!
//! The rule of this module: asynchronous data is never silently treated as simultaneous. Every
//! operation that combines timestamps returns `None` or `Err(ClockMismatch)` when clocks differ,
//! and simultaneity is always an explicit tolerance check.

use crate::stream::Sample;
use crate::{ClockMismatch, Duration, Observation, Timestamp};

/// The timestamps that describe one piece of data as it moves through the system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct TimingInfo {
    /// When the sensor acquired the data.
    pub acquired: Timestamp,
    /// When processing of the data finished, if it has been processed.
    pub processed: Option<Timestamp>,
}

impl TimingInfo {
    /// Timing for freshly acquired data, not yet processed.
    #[must_use]
    pub const fn acquired(acquired: Timestamp) -> Self {
        Self {
            acquired,
            processed: None,
        }
    }

    /// Time from acquisition to the end of processing.
    ///
    /// Returns `None` if not yet processed, on mismatched clocks, or if processing is recorded as
    /// finishing before acquisition.
    #[must_use]
    pub fn latency(&self) -> Option<Duration> {
        self.processed?.duration_since(self.acquired)
    }
}

/// Observations timestamped in `[start, end)`, in their stored order.
///
/// This is a read-only view for reflection and review. It stores nothing. Observations on a
/// different clock from `start` are never selected, because their position in the interval is
/// unknown.
#[must_use]
pub fn observations_between(
    observations: &[Observation],
    start: Timestamp,
    end: Timestamp,
) -> alloc::vec::Vec<&Observation> {
    observations
        .iter()
        .filter(|o| {
            let ts = o.timestamp();
            ts.is_same_clock(start) && ts.nanos() >= start.nanos() && ts.nanos() < end.nanos()
        })
        .collect()
}

/// Whether two timestamps are simultaneous within `tolerance`.
///
/// Returns `None` if the timestamps come from different clocks: their simultaneity cannot be
/// decided, and it must not be assumed.
#[must_use]
pub fn simultaneous(a: Timestamp, b: Timestamp, tolerance: Duration) -> Option<bool> {
    let gap = if a >= b {
        a.duration_since(b)?
    } else {
        b.duration_since(a)?
    };
    Some(gap <= tolerance)
}

/// How old a timestamp is relative to "now".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    /// Within the allowed age.
    Fresh,
    /// Older than the allowed age.
    Stale,
    /// Timestamped after "now", which indicates a bad clock or corrupted timestamp.
    FromFuture,
}

/// Classifies `timestamp` against `now`. Returns `None` if the clocks differ.
#[must_use]
pub fn freshness(timestamp: Timestamp, now: Timestamp, max_age: Duration) -> Option<Freshness> {
    if timestamp > now {
        return timestamp
            .is_same_clock(now)
            .then_some(Freshness::FromFuture);
    }
    let age = now.duration_since(timestamp)?;
    Some(if age > max_age {
        Freshness::Stale
    } else {
        Freshness::Fresh
    })
}

fn check_clock(samples: &[Sample<f64>], at: Timestamp) -> Result<(), ClockMismatch> {
    match samples.iter().find(|s| !s.timestamp.is_same_clock(at)) {
        Some(s) => Err(ClockMismatch {
            expected: at.clock(),
            found: s.timestamp.clock(),
        }),
        None => Ok(()),
    }
}

/// Returns the sample closest in time to `at`, if it is within `tolerance`.
///
/// # Errors
///
/// Returns [`ClockMismatch`] if any sample is on a different clock from `at`.
pub fn align_nearest(
    samples: &[Sample<f64>],
    at: Timestamp,
    tolerance: Duration,
) -> Result<Option<Sample<f64>>, ClockMismatch> {
    check_clock(samples, at)?;
    let best = samples
        .iter()
        .filter_map(|s| {
            let gap = if s.timestamp >= at {
                s.timestamp.duration_since(at)
            } else {
                at.duration_since(s.timestamp)
            }?;
            Some((gap, s))
        })
        .min_by_key(|(gap, _)| *gap);
    Ok(best
        .filter(|(gap, _)| *gap <= tolerance)
        .map(|(_, s)| s.clone()))
}

/// Linearly interpolates the value at `at` from the two samples that bracket it.
///
/// Returns `Ok(None)` if `at` is not between a sample at or before it and one at or after it.
///
/// # Errors
///
/// Returns [`ClockMismatch`] if any sample is on a different clock from `at`.
#[allow(
    clippy::cast_precision_loss,
    reason = "nanosecond gaps are interpolated as f64; precision loss is acceptable here"
)]
pub fn align_linear(samples: &[Sample<f64>], at: Timestamp) -> Result<Option<f64>, ClockMismatch> {
    check_clock(samples, at)?;
    let before = samples
        .iter()
        .filter(|s| s.timestamp <= at)
        .max_by_key(|s| s.timestamp.nanos());
    let after = samples
        .iter()
        .filter(|s| s.timestamp >= at)
        .min_by_key(|s| s.timestamp.nanos());
    let (Some(a), Some(b)) = (before, after) else {
        return Ok(None);
    };
    if a.timestamp == b.timestamp {
        return Ok(Some(a.value));
    }
    let span = match b.timestamp.duration_since(a.timestamp) {
        Some(d) => d.as_nanos() as f64,
        None => return Ok(None),
    };
    let offset = match at.duration_since(a.timestamp) {
        Some(d) => d.as_nanos() as f64,
        None => return Ok(None),
    };
    Ok(Some(a.value + (b.value - a.value) * (offset / span)))
}

/// Mean of all samples within `half_width` of `centre`. Returns `Ok(None)` if none qualify.
///
/// # Errors
///
/// Returns [`ClockMismatch`] if any sample is on a different clock from `centre`.
#[allow(
    clippy::cast_precision_loss,
    reason = "sample counts are far below 2^52; the mean is an f64 by definition"
)]
pub fn window_mean(
    samples: &[Sample<f64>],
    centre: Timestamp,
    half_width: Duration,
) -> Result<Option<f64>, ClockMismatch> {
    check_clock(samples, centre)?;
    let in_window: alloc::vec::Vec<f64> = samples
        .iter()
        .filter(|s| {
            let gap = if s.timestamp >= centre {
                s.timestamp.duration_since(centre)
            } else {
                centre.duration_since(s.timestamp)
            };
            gap.is_some_and(|g| g <= half_width)
        })
        .map(|s| s.value)
        .collect();
    if in_window.is_empty() {
        return Ok(None);
    }
    Ok(Some(in_window.iter().sum::<f64>() / in_window.len() as f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClockSource;

    fn t(ns: u64) -> Timestamp {
        Timestamp::new(ns, ClockSource::Monotonic)
    }

    fn s(ns: u64, value: f64) -> Sample<f64> {
        Sample {
            timestamp: t(ns),
            value,
        }
    }

    #[test]
    fn latency_is_processing_minus_acquisition() {
        let mut timing = TimingInfo::acquired(t(100));
        assert_eq!(timing.latency(), None);
        timing.processed = Some(t(250));
        assert_eq!(timing.latency(), Some(Duration::from_nanos(150)));
        timing.processed = Some(t(50));
        assert_eq!(timing.latency(), None);
    }

    #[test]
    fn simultaneity_requires_same_clock() {
        let wall = Timestamp::new(100, ClockSource::Wall);
        assert_eq!(simultaneous(t(100), wall, Duration::from_secs(1)), None);
        assert_eq!(
            simultaneous(t(100), t(105), Duration::from_nanos(5)),
            Some(true)
        );
        assert_eq!(
            simultaneous(t(100), t(106), Duration::from_nanos(5)),
            Some(false)
        );
        assert_eq!(
            simultaneous(t(106), t(100), Duration::from_nanos(5)),
            Some(false)
        );
    }

    #[test]
    fn freshness_detects_stale_and_future_data() {
        let now = t(1_000);
        let age = Duration::from_nanos(100);
        assert_eq!(freshness(t(950), now, age), Some(Freshness::Fresh));
        assert_eq!(freshness(t(100), now, age), Some(Freshness::Stale));
        assert_eq!(freshness(t(2_000), now, age), Some(Freshness::FromFuture));
        assert_eq!(
            freshness(Timestamp::new(0, ClockSource::Wall), now, age),
            None
        );
    }

    #[test]
    fn nearest_respects_tolerance() {
        let samples = [s(0, 1.0), s(100, 2.0), s(200, 3.0)];
        let hit = align_nearest(&samples, t(120), Duration::from_nanos(30)).unwrap();
        assert_eq!(hit.map(|x| x.value), Some(2.0));
        let miss = align_nearest(&samples, t(150), Duration::from_nanos(10)).unwrap();
        assert!(miss.is_none());
    }

    #[test]
    fn linear_interpolates_between_brackets() {
        let samples = [s(0, 0.0), s(100, 10.0)];
        assert_eq!(align_linear(&samples, t(25)).unwrap(), Some(2.5));
        assert_eq!(align_linear(&samples, t(100)).unwrap(), Some(10.0));
        assert_eq!(align_linear(&samples, t(150)).unwrap(), None);
    }

    #[test]
    fn window_mean_averages_inside_window() {
        let samples = [s(0, 1.0), s(50, 3.0), s(500, 100.0)];
        assert_eq!(
            window_mean(&samples, t(40), Duration::from_nanos(60)).unwrap(),
            Some(2.0)
        );
        assert_eq!(
            window_mean(&samples, t(1_000), Duration::from_nanos(1)).unwrap(),
            None
        );
    }

    #[test]
    fn alignment_refuses_mixed_clocks() {
        let samples = [Sample {
            timestamp: Timestamp::new(0, ClockSource::Wall),
            value: 1.0,
        }];
        assert!(align_linear(&samples, t(0)).is_err());
        assert!(align_nearest(&samples, t(0), Duration::from_secs(1)).is_err());
        assert!(window_mean(&samples, t(0), Duration::from_secs(1)).is_err());
    }
}
