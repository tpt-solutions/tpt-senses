//! Sensor health and diagnostics (spec §31). Sensors are entities that need monitoring, and
//! the cognitive layer should be able to reason about *why* confidence is low.

use crate::{Duration, StandardDeviation};

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

/// A ratio in the closed range `[0.0, 1.0]`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Ratio(f64);

impl Ratio {
    /// Creates a ratio. Returns `None` unless `value` is finite and within `[0.0, 1.0]`.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && (0.0..=1.0).contains(&value)).then_some(Self(value))
    }

    /// The ratio.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// A temperature in degrees Celsius. Finite by construction.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Celsius(f64);

impl Celsius {
    /// Creates a temperature. Returns `None` unless `value` is finite.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self(value))
    }

    /// The temperature in degrees Celsius.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Quantitative diagnostics. Every field is optional because sensors report different subsets.
///
/// Build with [`Diagnostics::new`] and the `with_*` methods.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Diagnostics {
    /// Signal quality on a `[0, 1]` scale, where `1.0` is best.
    pub signal_quality: Option<Ratio>,
    /// Noise level, as a standard deviation in the measurement's unit.
    pub noise: Option<StandardDeviation>,
    /// End-to-end latency.
    pub latency: Option<Duration>,
    /// Fraction of samples dropped, in `[0.0, 1.0]`.
    pub drop_rate: Option<Ratio>,
    /// Sensor temperature in degrees Celsius.
    pub temperature: Option<Celsius>,
    /// Calibration status.
    pub calibration: Option<CalibrationStatus>,
}

impl Diagnostics {
    /// No diagnostics reported. Same as `Default::default()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the signal quality (builder style).
    #[must_use]
    pub const fn with_signal_quality(mut self, value: Ratio) -> Self {
        self.signal_quality = Some(value);
        self
    }

    /// Sets the noise level (builder style).
    #[must_use]
    pub const fn with_noise(mut self, value: StandardDeviation) -> Self {
        self.noise = Some(value);
        self
    }

    /// Sets the latency (builder style).
    #[must_use]
    pub const fn with_latency(mut self, value: Duration) -> Self {
        self.latency = Some(value);
        self
    }

    /// Sets the drop rate (builder style).
    #[must_use]
    pub const fn with_drop_rate(mut self, value: Ratio) -> Self {
        self.drop_rate = Some(value);
        self
    }

    /// Sets the temperature (builder style).
    #[must_use]
    pub const fn with_temperature(mut self, value: Celsius) -> Self {
        self.temperature = Some(value);
        self
    }

    /// Sets the calibration status (builder style).
    #[must_use]
    pub const fn with_calibration(mut self, value: CalibrationStatus) -> Self {
        self.calibration = Some(value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratios_and_temperatures_are_validated() {
        assert!(Ratio::new(0.0).is_some() && Ratio::new(1.0).is_some());
        assert!(Ratio::new(1.01).is_none() && Ratio::new(f64::NAN).is_none());
        assert!(Celsius::new(-40.0).is_some());
        assert!(Celsius::new(f64::INFINITY).is_none());
    }

    #[test]
    fn operational_states() {
        assert!(SensorHealth::Online.is_operational());
        assert!(SensorHealth::Degraded.is_operational());
        assert!(!SensorHealth::Calibrating.is_operational());
        assert!(!SensorHealth::Faulted.is_operational());
        assert_eq!(SensorHealth::default(), SensorHealth::Unknown);
    }
}
