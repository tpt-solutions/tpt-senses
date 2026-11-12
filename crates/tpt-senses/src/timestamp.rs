//! Time with an explicit clock source (spec §17).
//!
//! Timestamps from different clocks are never considered comparable: ordering between them
//! yields `None`, so asynchronous measurements cannot silently be treated as simultaneous.

use core::cmp::Ordering;

/// The clock that produced a timestamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ClockSource {
    /// Monotonic clock: never goes backwards, arbitrary epoch.
    Monotonic,
    /// Wall-clock time (may jump when NTP adjusts it).
    Wall,
    /// Clock provided by the sensor hardware.
    Hardware,
    /// Simulation clock.
    Simulated,
    /// Clock source not known.
    Unknown,
}

/// A point in time, stored as nanoseconds since the epoch of its clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Timestamp {
    nanos: u64,
    clock: ClockSource,
}

impl Timestamp {
    /// Creates a timestamp from nanoseconds since the epoch of `clock`.
    #[must_use]
    pub const fn new(nanos: u64, clock: ClockSource) -> Self {
        Self { nanos, clock }
    }

    /// Nanoseconds since the epoch of this timestamp's clock.
    #[must_use]
    pub const fn nanos(self) -> u64 {
        self.nanos
    }

    /// The clock that produced this timestamp.
    #[must_use]
    pub const fn clock(self) -> ClockSource {
        self.clock
    }

    /// Returns `true` if both timestamps come from the same clock source.
    #[must_use]
    pub fn is_same_clock(self, other: Self) -> bool {
        self.clock == other.clock
    }

    /// Elapsed time since `earlier`.
    ///
    /// Returns `None` if the clocks differ or if `earlier` is later than `self`.
    #[must_use]
    pub fn duration_since(self, earlier: Self) -> Option<Duration> {
        if !self.is_same_clock(earlier) {
            return None;
        }
        self.nanos
            .checked_sub(earlier.nanos)
            .map(Duration::from_nanos)
    }
}

impl Timestamp {
    /// Moves the timestamp later by `delta`, saturating at `u64::MAX` nanoseconds.
    #[must_use]
    pub const fn saturating_add(self, delta: Duration) -> Self {
        Self {
            nanos: self.nanos.saturating_add(delta.nanos),
            clock: self.clock,
        }
    }

    /// Moves the timestamp earlier by `delta`, saturating at zero.
    #[must_use]
    pub const fn saturating_sub(self, delta: Duration) -> Self {
        Self {
            nanos: self.nanos.saturating_sub(delta.nanos),
            clock: self.clock,
        }
    }
}

/// Returned when two timestamps from different clocks are combined where a single clock is
/// required.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockMismatch {
    /// Clock the operation expected.
    pub expected: ClockSource,
    /// Clock that was supplied.
    pub found: ClockSource,
}

impl core::fmt::Display for ClockMismatch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "clock mismatch: expected {:?}, found {:?}",
            self.expected, self.found
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ClockMismatch {}

/// Timestamps are ordered only when they share a clock; otherwise they are incomparable.
impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.clock != other.clock {
            return None;
        }
        Some(self.nanos.cmp(&other.nanos))
    }
}

/// A non-negative span of time, in nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Duration {
    nanos: u64,
}

impl Duration {
    /// A zero-length duration.
    pub const ZERO: Self = Self { nanos: 0 };

    /// Creates a duration from nanoseconds.
    #[must_use]
    pub const fn from_nanos(nanos: u64) -> Self {
        Self { nanos }
    }

    /// Creates a duration from milliseconds, saturating on overflow.
    #[must_use]
    pub const fn from_millis(millis: u64) -> Self {
        Self {
            nanos: millis.saturating_mul(1_000_000),
        }
    }

    /// Creates a duration from seconds, saturating on overflow.
    #[must_use]
    pub const fn from_secs(secs: u64) -> Self {
        Self {
            nanos: secs.saturating_mul(1_000_000_000),
        }
    }

    /// Total nanoseconds.
    #[must_use]
    pub const fn as_nanos(self) -> u64 {
        self.nanos
    }

    /// Total seconds as a floating-point value.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "f64 is the intended public float view"
    )]
    pub fn as_secs_f64(self) -> f64 {
        self.nanos as f64 / 1_000_000_000.0
    }
}

/// A finite, strictly positive rate in hertz.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Frequency {
    hz: f64,
}

impl Frequency {
    /// Creates a frequency. Returns `None` unless `hz` is finite and greater than zero.
    #[must_use]
    pub fn from_hz(hz: f64) -> Option<Self> {
        (hz.is_finite() && hz > 0.0).then_some(Self { hz })
    }

    /// The frequency in hertz.
    #[must_use]
    pub const fn hz(self) -> f64 {
        self.hz
    }

    /// The period between samples. Sub-nanosecond periods round down to zero.
    #[must_use]
    pub fn period(self) -> Duration {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "period is finite and positive; truncation is documented"
        )]
        let nanos = (1_000_000_000.0 / self.hz) as u64;
        Duration::from_nanos(nanos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_clock_timestamps_are_incomparable() {
        let a = Timestamp::new(10, ClockSource::Monotonic);
        let b = Timestamp::new(10, ClockSource::Wall);
        assert_eq!(a.partial_cmp(&b), None);
        assert_eq!(a.duration_since(b), None);
        assert_ne!(a, b);
    }

    #[test]
    fn same_clock_duration_is_computed() {
        let a = Timestamp::new(1_500, ClockSource::Monotonic);
        let b = Timestamp::new(1_000, ClockSource::Monotonic);
        assert_eq!(a.duration_since(b), Some(Duration::from_nanos(500)));
        assert_eq!(b.duration_since(a), None);
        assert!(a > b);
    }

    #[test]
    fn duration_constructors() {
        assert_eq!(Duration::from_millis(2).as_nanos(), 2_000_000);
        assert!((Duration::from_secs(1).as_secs_f64() - 1.0).abs() < 1e-12);
        assert_eq!(Duration::from_secs(u64::MAX).as_nanos(), u64::MAX);
    }

    #[test]
    fn frequency_rejects_invalid_values() {
        assert!(Frequency::from_hz(0.0).is_none());
        assert!(Frequency::from_hz(-1.0).is_none());
        assert!(Frequency::from_hz(f64::NAN).is_none());
        assert!(Frequency::from_hz(f64::INFINITY).is_none());
        assert_eq!(
            Frequency::from_hz(10.0).unwrap().period(),
            Duration::from_millis(100)
        );
    }
}
