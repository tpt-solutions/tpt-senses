//! The `Sensor` abstraction (spec §9).

use alloc::string::String;
use core::fmt;

use crate::{SensorCapabilities, SensorHealth, SensorId};

/// Errors a sensor can report while reading.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SensorError {
    /// The sensor is not ready to produce data yet.
    NotReady,
    /// No data arrived within the expected time.
    Timeout,
    /// The connection to the sensor was lost.
    Disconnected,
    /// The sensor reported a fault.
    Faulted(String),
    /// Data was received but failed validation.
    InvalidData(String),
}

impl fmt::Display for SensorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotReady => f.write_str("sensor not ready"),
            Self::Timeout => f.write_str("sensor read timed out"),
            Self::Disconnected => f.write_str("sensor disconnected"),
            Self::Faulted(reason) => write!(f, "sensor faulted: {reason}"),
            Self::InvalidData(reason) => write!(f, "invalid sensor data: {reason}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SensorError {}

/// Anything that produces measurements: a camera, a `LiDAR`, a simulated sensor, or an internal
/// system monitor. Implementations must not assume a particular modality or runtime.
pub trait Sensor {
    /// The type of measurement this sensor produces.
    type Measurement;

    /// Stable identity of this sensor.
    fn identity(&self) -> SensorId;

    /// Descriptive capabilities of this sensor.
    fn capabilities(&self) -> &SensorCapabilities;

    /// Current health. Defaults to [`SensorHealth::Unknown`] for sensors that do not report it.
    fn health(&self) -> SensorHealth {
        SensorHealth::Unknown
    }

    /// Takes one reading.
    ///
    /// # Errors
    ///
    /// Returns [`SensorError`] if no valid measurement can be produced.
    fn read(&mut self) -> Result<Self::Measurement, SensorError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn errors_display_their_cause() {
        assert_eq!(SensorError::Timeout.to_string(), "sensor read timed out");
        assert_eq!(
            SensorError::Faulted("lens obstructed".into()).to_string(),
            "sensor faulted: lens obstructed"
        );
    }
}
