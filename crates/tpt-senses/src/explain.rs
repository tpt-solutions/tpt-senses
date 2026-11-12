//! Verification hook: "why do we believe this?" (spec §16, §47).
//!
//! [`explain`] restates an observation's provenance chain as typed data, so a verifier (such as
//! `tpt-eve`) can inspect every source and transformation behind it. It only reads the
//! observation. It never changes the observation and never decides whether the observation is
//! true.

use alloc::string::String;
use alloc::vec::Vec;

use crate::{
    Algorithm, CalibrationRecord, Confidence, Observation, ObservationId, ObservationState, Origin,
    ProvenanceStep, SensorId, TrustLevel,
};

/// An inspectable account of where an observation came from.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Explanation {
    /// The observation this explains.
    pub observation: ObservationId,
    /// Whether the data is real, simulated, recorded, synthetic or of unknown origin.
    pub origin: Origin,
    /// Hardware devices named in the chain, in order.
    pub hardware: Vec<String>,
    /// Sensors in the chain, in order.
    pub sensors: Vec<SensorId>,
    /// Algorithms applied, in order, with their parameters.
    pub algorithms: Vec<Algorithm>,
    /// Calibrations in effect, in order.
    pub calibrations: Vec<CalibrationRecord>,
    /// Number of steps in the full chain, including ones not listed above.
    pub steps: usize,
    /// The observation's confidence. Describes the producer's belief, not truth.
    pub confidence: Confidence,
    /// Whether any uncertainty was recorded.
    pub has_uncertainty: bool,
    /// The trust level assigned to the source.
    pub trust: TrustLevel,
    /// The lifecycle state at the time of explanation.
    pub state: ObservationState,
}

/// Explains an observation from its provenance chain, trust and uncertainty.
#[must_use]
pub fn explain(observation: &Observation) -> Explanation {
    let chain = observation.provenance().chain();
    let mut hardware = Vec::new();
    let mut calibrations = Vec::new();
    let mut algorithms = Vec::new();
    for step in chain {
        match step {
            ProvenanceStep::Hardware { device } => hardware.push(device.clone()),
            ProvenanceStep::Calibration(record) => calibrations.push(record.clone()),
            ProvenanceStep::Algorithm(algorithm) => algorithms.push(algorithm.clone()),
            ProvenanceStep::Sensor(_) => {}
        }
    }

    Explanation {
        observation: observation.id(),
        origin: observation.provenance().origin(),
        hardware,
        sensors: observation.provenance().sensors().collect(),
        algorithms,
        calibrations,
        steps: chain.len(),
        confidence: observation.confidence(),
        has_uncertainty: observation.uncertainty().is_some(),
        trust: observation.trust().level,
        state: observation.state(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClockSource, Modality, ObservationValue, Property, Provenance, StandardDeviation,
        Timestamp, Uncertainty,
    };

    #[test]
    fn explains_chain_in_order() {
        let provenance = Provenance::new(Origin::Simulated)
            .with_step(ProvenanceStep::Hardware {
                device: "cam-0".into(),
            })
            .with_step(ProvenanceStep::Sensor(SensorId::new(4)))
            .with_step(ProvenanceStep::Algorithm(Algorithm::new("detector", "2")));
        let observation = Observation::new(
            ObservationId::new(9),
            Modality::Visual,
            Property::new("presence"),
            ObservationValue::Scalar(1.0),
            Confidence::new(0.6).unwrap(),
            Timestamp::new(0, ClockSource::Monotonic),
            provenance,
        )
        .with_uncertainty(
            Uncertainty::new().with_standard_deviation(StandardDeviation::new(0.1).unwrap()),
        );

        let explanation = explain(&observation);
        assert_eq!(explanation.observation, ObservationId::new(9));
        assert_eq!(explanation.origin, Origin::Simulated);
        assert_eq!(explanation.hardware, ["cam-0"]);
        assert_eq!(explanation.sensors, [SensorId::new(4)]);
        assert_eq!(explanation.algorithms.len(), 1);
        assert_eq!(explanation.steps, 3);
        assert!(explanation.has_uncertainty);
        assert_eq!(explanation.state, ObservationState::Raw);
    }

    #[test]
    fn explains_observation_without_chain() {
        let observation = Observation::new(
            ObservationId::new(1),
            Modality::Visual,
            Property::new("presence"),
            ObservationValue::Scalar(0.0),
            Confidence::ZERO,
            Timestamp::new(0, ClockSource::Monotonic),
            Provenance::new(Origin::Unknown),
        );
        let explanation = explain(&observation);
        assert_eq!(explanation.steps, 0);
        assert!(explanation.sensors.is_empty());
        assert!(!explanation.has_uncertainty);
    }
}
