//! Provenance: "Why do we believe this?" (spec §16, §30).

use alloc::string::String;
use alloc::vec::Vec;

use crate::{CalibrationRecord, SensorId};

/// Where the data ultimately came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Origin {
    /// Produced by real hardware observing the physical world.
    Real,
    /// Produced by a simulator.
    Simulated,
    /// Replayed from a recording.
    Recorded,
    /// Synthesised (e.g. generated data, augmentation).
    Synthetic,
    /// Origin not known.
    Unknown,
}

/// A processing algorithm and the parameters it was run with.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Algorithm {
    /// Algorithm name, e.g. `detector`.
    pub name: String,
    /// Algorithm version, e.g. `v4`.
    pub version: String,
    /// Key/value parameters the algorithm was configured with.
    pub parameters: Vec<(String, String)>,
}

impl Algorithm {
    /// Creates an algorithm record with no parameters.
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            parameters: Vec::new(),
        }
    }

    /// Adds a configuration parameter.
    #[must_use]
    pub fn with_parameter(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.parameters.push((key.into(), value.into()));
        self
    }
}

/// One link in the chain from hardware to observation.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProvenanceStep {
    /// A physical device, identified by a free-form name or serial.
    Hardware {
        /// Device identifier.
        device: String,
    },
    /// A sensor that produced the data.
    Sensor(SensorId),
    /// A processing algorithm applied to the data.
    Algorithm(Algorithm),
    /// A calibration in effect when the data was produced (spec §32).
    Calibration(CalibrationRecord),
}

/// The full, ordered source chain of a measurement or observation.
///
/// # Example
///
/// ```
/// use tpt_senses::{Algorithm, Origin, Provenance, ProvenanceStep, SensorId};
///
/// let provenance = Provenance::new(Origin::Real)
///     .with_step(ProvenanceStep::Hardware { device: "camera-0".into() })
///     .with_step(ProvenanceStep::Sensor(SensorId::new(1)))
///     .with_step(ProvenanceStep::Algorithm(Algorithm::new("detector", "2.1")));
///
/// assert_eq!(provenance.origin(), Origin::Real);
/// assert_eq!(provenance.sensors().collect::<Vec<_>>(), [SensorId::new(1)]);
/// assert_eq!(provenance.algorithms().count(), 1);
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Provenance {
    origin: Origin,
    chain: Vec<ProvenanceStep>,
}

impl Provenance {
    /// Creates an empty provenance record with the given origin.
    #[must_use]
    pub const fn new(origin: Origin) -> Self {
        Self {
            origin,
            chain: Vec::new(),
        }
    }

    /// Appends a step to the chain (builder style).
    #[must_use]
    pub fn with_step(mut self, step: ProvenanceStep) -> Self {
        self.chain.push(step);
        self
    }

    /// Appends a step to the chain in place.
    pub fn push(&mut self, step: ProvenanceStep) {
        self.chain.push(step);
    }

    /// The origin of the data.
    #[must_use]
    pub const fn origin(&self) -> Origin {
        self.origin
    }

    /// The ordered chain of provenance steps, oldest first.
    #[must_use]
    pub fn chain(&self) -> &[ProvenanceStep] {
        &self.chain
    }

    /// Iterates over the sensors in the chain.
    pub fn sensors(&self) -> impl Iterator<Item = SensorId> + '_ {
        self.chain.iter().filter_map(|step| match step {
            ProvenanceStep::Sensor(id) => Some(*id),
            _ => None,
        })
    }

    /// The calibration records in the chain.
    pub fn calibrations(&self) -> impl Iterator<Item = &CalibrationRecord> + '_ {
        self.chain.iter().filter_map(|step| match step {
            ProvenanceStep::Calibration(record) => Some(record),
            _ => None,
        })
    }

    /// Iterates over the algorithms in the chain.
    pub fn algorithms(&self) -> impl Iterator<Item = &Algorithm> + '_ {
        self.chain.iter().filter_map(|step| match step {
            ProvenanceStep::Algorithm(algorithm) => Some(algorithm),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_preserves_order_and_filters() {
        let sensor = SensorId::new(7);
        let provenance = Provenance::new(Origin::Simulated)
            .with_step(ProvenanceStep::Hardware {
                device: "cam-0".into(),
            })
            .with_step(ProvenanceStep::Sensor(sensor))
            .with_step(ProvenanceStep::Algorithm(
                Algorithm::new("detector", "v4").with_parameter("threshold", "0.5"),
            ));

        assert_eq!(provenance.origin(), Origin::Simulated);
        assert_eq!(provenance.chain().len(), 3);
        assert_eq!(provenance.sensors().collect::<Vec<_>>(), [sensor]);
        let algo = provenance.algorithms().next().unwrap();
        assert_eq!(algo.name, "detector");
        assert_eq!(algo.parameters[0], ("threshold".into(), "0.5".into()));
    }
}
