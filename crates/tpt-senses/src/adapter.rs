//! Adapter contract (spec §7, §54). An adapter turns data from an existing system (a vision
//! pipeline, an audio pipeline, a robot telemetry feed) into observations.
//!
//! The core defines only the trait. Adapters for specific TPT repositories live outside the core,
//! so the core never depends on them and dependencies point upward, from adapter to core.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::Observation;

/// Why an adapter could not produce observations from an input.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AdapterError {
    /// The input kind is not one this adapter understands.
    Unsupported(String),
    /// The input was recognised but is malformed or inconsistent.
    Invalid(String),
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "unsupported input: {what}"),
            Self::Invalid(why) => write!(f, "invalid input: {why}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AdapterError {}

/// Converts an external representation into observations.
///
/// `Input` is whatever the external system provides: a detection list, a feature vector, a
/// telemetry record. Implementations must not assume a runtime, and must not invent observations
/// the input does not support.
pub trait Adapter {
    /// The external input type.
    type Input;

    /// Stable name of the adapter, recorded in provenance by the caller.
    fn name(&self) -> &'static str;

    /// Produces zero or more observations from one input.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError`] if the input cannot be converted.
    fn adapt(&mut self, input: Self::Input) -> Result<Vec<Observation>, AdapterError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClockSource, Confidence, Modality, ObservationId, ObservationValue, Origin, Property,
        Provenance, ProvenanceStep, SensorId, Timestamp,
    };

    /// A reading from an external system, as an adapter would receive it.
    struct ExternalReading {
        sensor: u64,
        celsius: f64,
        at_ns: u64,
    }

    /// Example adapter: external thermometer readings → thermal observations.
    struct ThermometerAdapter {
        next_id: u64,
    }

    impl Adapter for ThermometerAdapter {
        type Input = ExternalReading;

        fn name(&self) -> &'static str {
            "thermometer-adapter"
        }

        fn adapt(&mut self, input: ExternalReading) -> Result<Vec<Observation>, AdapterError> {
            if !input.celsius.is_finite() {
                return Err(AdapterError::Invalid("non-finite temperature".into()));
            }
            if input.sensor == 0 {
                return Err(AdapterError::Unsupported("sensor 0 is reserved".into()));
            }
            let id = ObservationId::new(self.next_id);
            self.next_id += 1;
            let obs = Observation::new(
                id,
                Modality::Thermal,
                Property::new("temperature_c"),
                ObservationValue::Scalar(input.celsius),
                Confidence::new(0.9).unwrap_or(Confidence::ONE),
                Timestamp::new(input.at_ns, ClockSource::Hardware),
                Provenance::new(Origin::Real)
                    .with_step(ProvenanceStep::Sensor(SensorId::new(input.sensor))),
            );
            Ok(alloc::vec![obs])
        }
    }

    fn reading(sensor: u64, celsius: f64) -> ExternalReading {
        ExternalReading {
            sensor,
            celsius,
            at_ns: 1,
        }
    }

    #[test]
    fn adapter_converts_input_to_observations() {
        let mut a = ThermometerAdapter { next_id: 1 };
        let out = a.adapt(reading(4, 21.5)).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].modality(), &Modality::Thermal);
        assert_eq!(a.name(), "thermometer-adapter");
    }

    #[test]
    fn adapter_reports_unsupported_and_invalid_input() {
        let mut a = ThermometerAdapter { next_id: 1 };
        assert!(matches!(
            a.adapt(reading(0, 1.0)),
            Err(AdapterError::Unsupported(_))
        ));
        assert!(matches!(
            a.adapt(reading(4, f64::NAN)),
            Err(AdapterError::Invalid(_))
        ));
    }
}
