//! Adapter from `tpt-senses` observations to `tpt-eve` facts (spec §16, §47, §59).
//!
//! `tpt-eve` reasons over symbolic triples. This crate turns each observation into one
//! [`tpt_eve_core::Fact`] and keeps the evidence attached to it:
//!
//! * **Confidence** carries over from the observation.
//! * **Source** is [`FactSource::Extracted`]. A sensor reading is not hand-authored
//!   (`Asserted`) and is not derived by inference, so it is recorded as evidence that
//!   was extracted from the world, with its confidence.
//! * **Uncertainty and the full provenance chain do not fit in a `Fact`.** They stay on the
//!   observation. Use `tpt_senses::explain` when a verifier needs them.
//!
//! Observations with no single-triple form (vectors) are skipped, not approximated.
//!
//! The mapping is:
//!
//! | Observation part | Fact part |
//! |---|---|
//! | entity, or `unattributed` when there is none | subject (`Symbol`, `entity:<n>`) |
//! | property name | predicate (`Symbol`) |
//! | `Boolean(b)` | object `Symbol("true")` or `Symbol("false")` |
//! | `Scalar(x)` | object `Text(x)` |
//! | `Categorical(label)` | object `Symbol(label)` |
//! | `Vector(_)` | not converted |
//! | confidence | `confidence` (as `f32`) |

use tpt_eve_core::{Fact, FactSource, Value};
use tpt_senses::{Observation, ObservationSink, ObservationValue};

/// Subject used when an observation is not about a known entity.
pub const UNATTRIBUTED: &str = "unattributed";

/// Converts one observation to a fact, or `None` if it has no single-triple form.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    reason = "tpt-eve stores confidence as f32; the value is already within [0, 1]"
)]
pub fn observation_to_fact(observation: &Observation) -> Option<Fact> {
    let subject = match observation.subject() {
        Some(entity) => Value::Symbol(format!("entity:{}", entity.get())),
        None => Value::Symbol(UNATTRIBUTED.into()),
    };
    let predicate = Value::Symbol(observation.property().as_str().into());
    let object = match observation.value() {
        ObservationValue::Boolean(true) => Value::Symbol("true".into()),
        ObservationValue::Boolean(false) => Value::Symbol("false".into()),
        ObservationValue::Scalar(x) => Value::Text(x.to_string()),
        ObservationValue::Categorical(label) => Value::Symbol(label.clone()),
        // Vectors have no single-triple form. Unknown future variants are not guessed at either.
        _ => return None,
    };
    let confidence = observation.confidence().get() as f32;

    Some(Fact {
        subject,
        predicate,
        object,
        source: FactSource::Extracted { confidence },
        confidence,
    })
}

/// An [`ObservationSink`] that collects facts for `tpt-eve`.
///
/// Observations with no fact form are dropped and counted in [`skipped`](Self::skipped).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EveFactSink {
    facts: Vec<Fact>,
    skipped: usize,
}

impl EveFactSink {
    /// An empty sink.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            facts: Vec::new(),
            skipped: 0,
        }
    }

    /// Facts collected so far, in arrival order.
    #[must_use]
    pub fn facts(&self) -> &[Fact] {
        &self.facts
    }

    /// Number of observations that had no fact form.
    #[must_use]
    pub const fn skipped(&self) -> usize {
        self.skipped
    }

    /// Takes the collected facts, leaving the sink empty.
    pub fn take_facts(&mut self) -> Vec<Fact> {
        core::mem::take(&mut self.facts)
    }
}

impl ObservationSink for EveFactSink {
    fn accept(&mut self, observation: Observation) {
        match observation_to_fact(&observation) {
            Some(fact) => self.facts.push(fact),
            None => self.skipped += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_senses::prelude::*;
    use tpt_senses::{EntityId, Property};

    fn observation(value: ObservationValue, confidence: f64) -> Observation {
        Observation::new(
            ObservationId::new(1),
            Modality::Visual,
            Property::new("ripeness"),
            value,
            Confidence::new(confidence).unwrap(),
            Timestamp::new(0, ClockSource::Monotonic),
            Provenance::new(Origin::Real),
        )
    }

    #[test]
    fn scalar_becomes_text_object_with_extracted_source() {
        let fact = observation_to_fact(&observation(ObservationValue::Scalar(0.75), 0.8)).unwrap();
        assert_eq!(fact.subject, Value::Symbol(UNATTRIBUTED.into()));
        assert_eq!(fact.predicate, Value::Symbol("ripeness".into()));
        assert_eq!(fact.object, Value::Text("0.75".into()));
        assert_eq!(fact.source, FactSource::Extracted { confidence: 0.8 });
        assert!((fact.confidence - 0.8).abs() < 1e-6);
    }

    #[test]
    fn boolean_and_categorical_map_to_symbols() {
        let yes = observation_to_fact(&observation(ObservationValue::Boolean(true), 1.0)).unwrap();
        assert_eq!(yes.object, Value::Symbol("true".into()));
        let no = observation_to_fact(&observation(ObservationValue::Boolean(false), 1.0)).unwrap();
        assert_eq!(no.object, Value::Symbol("false".into()));
        let label = observation_to_fact(&observation(
            ObservationValue::Categorical("fungal_stress".into()),
            0.5,
        ))
        .unwrap();
        assert_eq!(label.object, Value::Symbol("fungal_stress".into()));
    }

    #[test]
    fn subject_comes_from_the_entity_when_known() {
        let observation =
            observation(ObservationValue::Boolean(true), 1.0).with_subject(EntityId::new(27));
        let fact = observation_to_fact(&observation).unwrap();
        assert_eq!(fact.subject, Value::Symbol("entity:27".into()));
    }

    #[test]
    fn vectors_have_no_single_triple_form() {
        let observation = observation(ObservationValue::Vector(vec![1.0, 2.0]), 1.0);
        assert!(observation_to_fact(&observation).is_none());
    }

    #[test]
    fn sink_collects_facts_and_counts_skips() {
        let mut sink = EveFactSink::new();
        sink.accept(observation(ObservationValue::Scalar(1.0), 0.9));
        sink.accept(observation(ObservationValue::Vector(vec![1.0]), 0.9));
        sink.accept(observation(ObservationValue::Boolean(true), 0.9));
        assert_eq!(sink.facts().len(), 2);
        assert_eq!(sink.skipped(), 1);

        let taken = sink.take_facts();
        assert_eq!(taken.len(), 2);
        assert!(sink.facts().is_empty());
    }

    #[test]
    fn duplicate_readings_share_a_triple() {
        // Two readings of the same triple are duplicates to tpt-eve's working memory,
        // whatever their confidence.
        let a = observation_to_fact(&observation(ObservationValue::Boolean(true), 0.6)).unwrap();
        let b = observation_to_fact(&observation(ObservationValue::Boolean(true), 0.9)).unwrap();
        assert!(a.same_triple(&b));
    }
}
