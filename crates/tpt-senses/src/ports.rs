//! Boundaries to cognition (spec §35, §46, §59).
//!
//! `tpt-anima` and `tpt-thought` sit on the other side of these traits. This crate defines the
//! boundary only. It never depends on either project, so the cognitive layer can be replaced
//! without touching sensing, and sensing can be swapped without touching cognition.
//!
//! * [`ObservationSink`]: observations, events and state flow out to the cognitive layer.
//! * [`AttentionSource`]: attention hints flow in. They change sampling only, never truth.

use alloc::vec::Vec;

use crate::{AttentionHint, Observation};

/// Receives observations from sensing. Implement this to hand observations to a cognitive layer.
///
/// Implementations must keep each observation as evidence. Accepting one must not mark it as a
/// fact, and must not discard its provenance.
pub trait ObservationSink {
    /// Accepts one observation.
    fn accept(&mut self, observation: Observation);
}

impl ObservationSink for Vec<Observation> {
    fn accept(&mut self, observation: Observation) {
        self.push(observation);
    }
}

/// Supplies attention hints from the cognitive layer.
pub trait AttentionSource {
    /// Takes every hint received since the last call. Returns an empty list if there are none.
    fn drain_hints(&mut self) -> Vec<AttentionHint>;
}

impl AttentionSource for Vec<AttentionHint> {
    fn drain_hints(&mut self) -> Vec<AttentionHint> {
        core::mem::take(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClockSource, Confidence, Modality, ObservationId, ObservationValue, Origin, Property,
        Provenance, Timestamp,
    };

    fn observation(id: u64) -> Observation {
        Observation::new(
            ObservationId::new(id),
            Modality::Visual,
            Property::new("presence"),
            ObservationValue::Scalar(1.0),
            Confidence::new(0.5).unwrap(),
            Timestamp::new(0, ClockSource::Monotonic),
            Provenance::new(Origin::Real),
        )
    }

    #[test]
    fn sink_keeps_observations_in_order_and_unchanged() {
        let mut sink: Vec<Observation> = Vec::new();
        sink.accept(observation(1));
        sink.accept(observation(2));
        assert_eq!(sink.len(), 2);
        assert_eq!(sink[0].id(), ObservationId::new(1));
        assert_eq!(sink[0].state(), crate::ObservationState::Raw);
    }

    #[test]
    fn attention_source_drains_once() {
        let mut hints = alloc::vec![AttentionHint::new(None, Confidence::ONE)];
        assert_eq!(hints.drain_hints().len(), 1);
        assert!(hints.drain_hints().is_empty());
    }
}
