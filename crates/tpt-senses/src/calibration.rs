//! Calibration metadata (spec §32). Calibration is explicit and part of provenance.

use alloc::string::String;
use alloc::vec::Vec;

use crate::Timestamp;

/// The kind of calibration applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CalibrationKind {
    /// Sensor-internal parameters (focal length, distortion, ...).
    Intrinsic,
    /// Pose of the sensor relative to another frame.
    Extrinsic,
    /// Correction as a function of temperature.
    Temperature,
    /// Offset at zero input.
    ZeroPoint,
    /// Clock offset / drift correction.
    Time,
    /// Alignment with other sensors.
    Alignment,
}

/// One calibration applied to a sensor at a point in time.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CalibrationRecord {
    /// What was calibrated.
    pub kind: CalibrationKind,
    /// When the calibration was performed.
    pub performed_at: Timestamp,
    /// When the calibration stops being valid, if known.
    pub valid_until: Option<Timestamp>,
    /// Named parameter values, e.g. `("zero_point", 0.02)`.
    pub parameters: Vec<(String, f64)>,
    /// Whether the calibration procedure succeeded. A failed calibration must not be used.
    pub succeeded: bool,
}

impl CalibrationRecord {
    /// Creates a successful calibration record with no parameters and no expiry.
    #[must_use]
    pub const fn new(kind: CalibrationKind, performed_at: Timestamp) -> Self {
        Self {
            kind,
            performed_at,
            valid_until: None,
            parameters: Vec::new(),
            succeeded: true,
        }
    }

    /// Sets the expiry time (builder style).
    #[must_use]
    pub const fn valid_until(mut self, until: Timestamp) -> Self {
        self.valid_until = Some(until);
        self
    }

    /// Adds a named parameter (builder style).
    #[must_use]
    pub fn with_parameter(mut self, name: impl Into<String>, value: f64) -> Self {
        self.parameters.push((name.into(), value));
        self
    }

    /// Marks the calibration as failed (builder style).
    #[must_use]
    pub const fn failed(mut self) -> Self {
        self.succeeded = false;
        self
    }

    /// Looks up a parameter by name.
    #[must_use]
    pub fn parameter(&self, name: &str) -> Option<f64> {
        self.parameters
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
    }

    /// Whether this calibration can be relied on at `at`.
    ///
    /// Returns `None` if `at` is on a different clock, because validity cannot be decided.
    #[must_use]
    pub fn is_usable_at(&self, at: Timestamp) -> Option<bool> {
        if !self.performed_at.is_same_clock(at) {
            return None;
        }
        let started = self.performed_at <= at;
        let not_expired = match self.valid_until {
            Some(until) => at <= until,
            None => true,
        };
        Some(self.succeeded && started && not_expired)
    }
}

/// The calibration history of one sensor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CalibrationState {
    records: Vec<CalibrationRecord>,
}

impl CalibrationState {
    /// Creates an empty state (nothing calibrated).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Records a calibration.
    pub fn record(&mut self, record: CalibrationRecord) {
        self.records.push(record);
    }

    /// All recorded calibrations, oldest first.
    #[must_use]
    pub fn records(&self) -> &[CalibrationRecord] {
        &self.records
    }

    /// The most recent calibration of `kind` performed on the same clock at or before `at`,
    /// provided it is usable at `at`.
    ///
    /// If the most recent calibration of that kind has failed or expired, the result is `None`:
    /// an older successful calibration does not silently stand in for a newer failed one.
    #[must_use]
    pub fn usable(&self, kind: CalibrationKind, at: Timestamp) -> Option<&CalibrationRecord> {
        let latest = self
            .records
            .iter()
            .filter(|r| r.kind == kind && r.performed_at.is_same_clock(at) && r.performed_at <= at)
            .max_by(|a, b| {
                a.performed_at
                    .partial_cmp(&b.performed_at)
                    .unwrap_or(core::cmp::Ordering::Equal)
            })?;
        (latest.is_usable_at(at) == Some(true)).then_some(latest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClockSource;

    fn t(n: u64) -> Timestamp {
        Timestamp::new(n, ClockSource::Monotonic)
    }

    #[test]
    fn expiry_is_enforced() {
        let rec = CalibrationRecord::new(CalibrationKind::Temperature, t(10)).valid_until(t(20));
        assert_eq!(rec.is_usable_at(t(15)), Some(true));
        assert_eq!(rec.is_usable_at(t(25)), Some(false));
        assert_eq!(rec.is_usable_at(t(5)), Some(false));
    }

    #[test]
    fn different_clock_is_undecidable() {
        let rec = CalibrationRecord::new(CalibrationKind::Time, t(10));
        let wall = Timestamp::new(10, ClockSource::Wall);
        assert_eq!(rec.is_usable_at(wall), None);
    }

    #[test]
    fn failed_calibration_is_not_usable() {
        let rec = CalibrationRecord::new(CalibrationKind::ZeroPoint, t(1)).failed();
        assert_eq!(rec.is_usable_at(t(2)), Some(false));
    }

    #[test]
    fn state_returns_latest_usable_record() {
        let mut state = CalibrationState::new();
        state.record(
            CalibrationRecord::new(CalibrationKind::ZeroPoint, t(1)).with_parameter("zero", 0.1),
        );
        state.record(
            CalibrationRecord::new(CalibrationKind::ZeroPoint, t(5)).with_parameter("zero", 0.2),
        );
        let usable = state.usable(CalibrationKind::ZeroPoint, t(6)).unwrap();
        assert_eq!(usable.parameter("zero"), Some(0.2));
        assert!(state.usable(CalibrationKind::Intrinsic, t(6)).is_none());
    }

    #[test]
    fn newer_failed_calibration_blocks_use() {
        let mut state = CalibrationState::new();
        state.record(CalibrationRecord::new(CalibrationKind::Extrinsic, t(1)));
        state.record(CalibrationRecord::new(CalibrationKind::Extrinsic, t(5)).failed());
        assert!(state.usable(CalibrationKind::Extrinsic, t(6)).is_none());
    }
}
