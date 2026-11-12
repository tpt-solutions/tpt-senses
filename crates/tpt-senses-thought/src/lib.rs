//! Adapter from `tpt-senses` observations to `tpt-thought` observation input (spec §46, §59).
//!
//! `tpt-thought` reasons over [`StructuredValue`]s stamped with a source and a trust class. This
//! crate turns one [`tpt_senses::Observation`] into a [`SensedObservation`] holding exactly that.
//! It does not build a whole `tpt_thought_loop::Input`: the question, goal, constraints and
//! candidate plans belong to the cognitive caller, not to sensing.
//!
//! Mapping:
//!
//! | Observation | `tpt-thought` |
//! |---|---|
//! | timestamp (nanoseconds) | `at` (`Timestamp`, nanoseconds, clock stated in the value) |
//! | source identity, else first sensor id | `source` |
//! | authentication and trust level | `trust` (`SourceTrust`) |
//! | property, modality, value, confidence, origin, clock, entity, standard deviation | `observation` map |
//!
//! The observation map keeps the evidence (confidence, origin, clock, uncertainty) visible to
//! cognition. Nothing here marks the value as a fact.

use std::collections::BTreeMap;

use tpt_senses::{AuthenticationStatus, Observation, ObservationValue, Origin, TrustLevel};
use tpt_thought_core::{SourceTrust, StructuredValue, Timestamp};

/// One observation, shaped for `tpt-thought`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct SensedObservation {
    /// When the observation was made, in nanoseconds on the observation's clock.
    pub at: Timestamp,
    /// Where it came from: a source identity, or `sensor:<id>`, or `unknown`.
    pub source: String,
    /// Trust class of the source.
    pub trust: SourceTrust,
    /// The observation's content and evidence.
    pub observation: StructuredValue,
}

/// Converts one observation for `tpt-thought`.
#[must_use]
pub fn to_sensed_observation(observation: &Observation) -> SensedObservation {
    let metadata = observation.trust();
    let source = metadata
        .source
        .as_ref()
        .map(|identity| identity.as_str().to_owned())
        .or_else(|| {
            observation
                .provenance()
                .sensors()
                .next()
                .map(|sensor| format!("sensor:{}", sensor.get()))
        })
        .unwrap_or_else(|| "unknown".to_owned());

    let trust = match (metadata.level, metadata.authentication) {
        (TrustLevel::Verified, _) => SourceTrust::Trusted,
        (_, AuthenticationStatus::Authenticated) => SourceTrust::Authenticated,
        _ => SourceTrust::Untrusted,
    };

    let timestamp = observation.timestamp();
    let at = Timestamp(i64::try_from(timestamp.nanos()).unwrap_or(i64::MAX));

    let mut fields = BTreeMap::new();
    fields.insert(
        "property".to_owned(),
        StructuredValue::Text(observation.property().as_str().to_owned()),
    );
    fields.insert(
        "modality".to_owned(),
        StructuredValue::Text(observation.modality().name().to_owned()),
    );
    fields.insert("value".to_owned(), value_to_structured(observation.value()));
    fields.insert(
        "confidence".to_owned(),
        StructuredValue::Float(observation.confidence().get()),
    );
    fields.insert(
        "origin".to_owned(),
        StructuredValue::Text(origin_name(observation.provenance().origin()).to_owned()),
    );
    fields.insert(
        "clock".to_owned(),
        StructuredValue::Text(format!("{:?}", timestamp.clock())),
    );
    if let Some(entity) = observation.subject() {
        fields.insert(
            "entity".to_owned(),
            StructuredValue::Text(format!("entity:{}", entity.get())),
        );
    }
    if let Some(sd) = observation
        .uncertainty()
        .and_then(|uncertainty| uncertainty.standard_deviation)
    {
        fields.insert(
            "standard_deviation".to_owned(),
            StructuredValue::Float(sd.get()),
        );
    }

    SensedObservation {
        at,
        source,
        trust,
        observation: StructuredValue::Map(fields),
    }
}

fn value_to_structured(value: &ObservationValue) -> StructuredValue {
    match value {
        ObservationValue::Boolean(b) => StructuredValue::Bool(*b),
        ObservationValue::Scalar(x) => StructuredValue::Float(*x),
        ObservationValue::Categorical(label) => StructuredValue::Text(label.clone()),
        ObservationValue::Vector(values) => {
            StructuredValue::List(values.iter().map(|x| StructuredValue::Float(*x)).collect())
        }
        // Unknown future variants are carried as absent rather than guessed at.
        _ => StructuredValue::Null,
    }
}

fn origin_name(origin: Origin) -> &'static str {
    match origin {
        Origin::Real => "real",
        Origin::Simulated => "simulated",
        Origin::Recorded => "recorded",
        Origin::Synthetic => "synthetic",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_senses::prelude::*;
    use tpt_senses::{
        EntityId, Property, StandardDeviation, Timestamp as SenseTimestamp, TrustMetadata,
    };

    fn observation(value: ObservationValue) -> Observation {
        Observation::new(
            ObservationId::new(1),
            Modality::Auditory,
            Property::new("sound_level_db"),
            value,
            Confidence::new(0.7).unwrap(),
            SenseTimestamp::new(42, ClockSource::Monotonic),
            Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(5))),
        )
    }

    fn field<'a>(sensed: &'a SensedObservation, key: &str) -> &'a StructuredValue {
        match &sensed.observation {
            StructuredValue::Map(map) => &map[key],
            other => panic!("expected a map, got {other:?}"),
        }
    }

    #[test]
    fn scalar_keeps_value_confidence_and_origin() {
        let sensed = to_sensed_observation(&observation(ObservationValue::Scalar(61.5)));
        assert_eq!(field(&sensed, "value"), &StructuredValue::Float(61.5));
        assert_eq!(field(&sensed, "confidence"), &StructuredValue::Float(0.7));
        assert_eq!(
            field(&sensed, "origin"),
            &StructuredValue::Text("real".into())
        );
        assert_eq!(
            field(&sensed, "modality"),
            &StructuredValue::Text("auditory".into())
        );
        assert_eq!(sensed.at, Timestamp(42));
    }

    #[test]
    fn values_map_to_structured_variants() {
        assert_eq!(
            value_to_structured(&ObservationValue::Boolean(true)),
            StructuredValue::Bool(true)
        );
        assert_eq!(
            value_to_structured(&ObservationValue::Categorical("alarm".into())),
            StructuredValue::Text("alarm".into())
        );
        assert_eq!(
            value_to_structured(&ObservationValue::Vector(vec![1.0, 2.0])),
            StructuredValue::List(vec![
                StructuredValue::Float(1.0),
                StructuredValue::Float(2.0)
            ])
        );
    }

    #[test]
    fn source_falls_back_to_sensor_id_then_unknown() {
        let sensed = to_sensed_observation(&observation(ObservationValue::Scalar(1.0)));
        assert_eq!(sensed.source, "sensor:5");

        let bare = Observation::new(
            ObservationId::new(2),
            Modality::Visual,
            Property::new("p"),
            ObservationValue::Scalar(1.0),
            Confidence::ZERO,
            SenseTimestamp::new(0, ClockSource::Monotonic),
            Provenance::new(Origin::Unknown),
        );
        assert_eq!(to_sensed_observation(&bare).source, "unknown");
    }

    #[test]
    fn trust_follows_level_then_authentication() {
        let verified = observation(ObservationValue::Scalar(1.0)).with_trust(
            TrustMetadata::unverified()
                .with_level(TrustLevel::Verified)
                .with_authentication(AuthenticationStatus::Authenticated),
        );
        assert_eq!(to_sensed_observation(&verified).trust, SourceTrust::Trusted);

        let authenticated = observation(ObservationValue::Scalar(1.0)).with_trust(
            TrustMetadata::unverified().with_authentication(AuthenticationStatus::Authenticated),
        );
        assert_eq!(
            to_sensed_observation(&authenticated).trust,
            SourceTrust::Authenticated
        );

        let unverified = observation(ObservationValue::Scalar(1.0));
        assert_eq!(
            to_sensed_observation(&unverified).trust,
            SourceTrust::Untrusted
        );
    }

    #[test]
    fn entity_and_uncertainty_are_included_when_present() {
        let observation = observation(ObservationValue::Scalar(1.0))
            .with_subject(EntityId::new(27))
            .with_uncertainty(
                Uncertainty::new().with_standard_deviation(StandardDeviation::new(0.5).unwrap()),
            );
        let sensed = to_sensed_observation(&observation);
        assert_eq!(
            field(&sensed, "entity"),
            &StructuredValue::Text("entity:27".into())
        );
        assert_eq!(
            field(&sensed, "standard_deviation"),
            &StructuredValue::Float(0.5)
        );
    }

    #[test]
    fn property_name_is_kept() {
        let sensed = to_sensed_observation(&observation(ObservationValue::Boolean(false)));
        assert_eq!(
            field(&sensed, "property"),
            &StructuredValue::Text(Property::new("sound_level_db").as_str().into())
        );
    }
}
