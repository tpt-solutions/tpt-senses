//! Sensor health and diagnostics (spec §31). Sensors are entities that need monitoring, and
//! the cognitive layer should be able to reason about *why* confidence is low.

use crate::Duration;

/// Operational state of a sensor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SensorHealth {
    /// Working normally.
    Online,
    /// Not reachable or not producing data.
    Offline,
    /// Producing data with reduced quality (e.g. obstructed lens, high noise).
    Degraded,
    /// Calibration in progress; data not yet trustworthy.
    Calibrating,
    /// Failed; no usable data.
    Faulted,
    /// State not known.
    #[default]
    Unknown,
}

impl SensorHealth {
    /// Returns `true` if the sensor is producing usable data (possibly degraded).
    #[must_use]
    pub const fn is_operational(self) -> bool {
        matches!(self, Self::Online | Self::Degraded)
    }
}

/// Calibration status as reported in diagnostics. Full calibration metadata is a later phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CalibrationStatus {
    /// Calibration is current.
    Calibrated,
    /// Calibration is running.
    Calibrating,
    /// Calibration is out of date or failed and needs to be run.
    Required,
    /// Status not known.
    Unknown,
}

/// Quantitative diagnostics. Every field is optional because sensors report different subsets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diagnostics {
    /// Signal quality on a sensor-defined scale, where `1.0` is best.
    pub signal_quality: Option<f64>,
    /// Noise level, in the measurement's unit.
    pub noise: Option<f64>,
    /// End-to-end latency.
    pub latency: Option<Duration>,
    /// Fraction of samples dropped, in `[0.0, 1.0]`.
    pub drop_rate: Option<f64>,
    /// Sensor temperature in degrees Celsius.
    pub temperature_c: Option<f64>,
    /// Calibration status.
    pub calibration: Option<CalibrationStatus>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operational_states() {
        assert!(SensorHealth::Online.is_operational());
        assert!(SensorHealth::Degraded.is_operational());
        assert!(!SensorHealth::Calibrating.is_operational());
        assert!(!SensorHealth::Faulted.is_operational());
        assert_eq!(SensorHealth::default(), SensorHealth::Unknown);
    }
}
