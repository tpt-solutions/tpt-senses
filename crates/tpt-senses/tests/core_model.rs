//! Integration tests for the core data model: sensor → measurement → observation, plus
//! the cross-cutting rules from the spec (provenance, uncertainty, no silent fact-making).

use tpt_senses::StandardDeviation;
use tpt_senses::prelude::*;
use tpt_senses::{
    Algorithm, Bounds, Distribution, Duration, Frequency, Range, Resolution, SensorCapabilities,
    SensorError, SensorHealth, Uncertainty,
};

/// A minimal simulated sensor that returns a scripted sequence of values.
struct ScriptedSensor {
    id: SensorId,
    caps: SensorCapabilities,
    script: Vec<Result<f64, SensorError>>,
    next: usize,
}

impl Sensor for ScriptedSensor {
    type Measurement = Measurement;

    fn identity(&self) -> SensorId {
        self.id
    }

    fn capabilities(&self) -> &SensorCapabilities {
        &self.caps
    }

    fn health(&self) -> SensorHealth {
        SensorHealth::Degraded
    }

    fn read(&mut self) -> Result<Measurement, SensorError> {
        let item = self
            .script
            .get(self.next)
            .cloned()
            .unwrap_or(Err(SensorError::NotReady));
        self.next += 1;
        let value = item?;
        Ok(Measurement::new(
            self.id,
            Modality::Thermal,
            MeasurementValue::Scalar(value),
            Timestamp::new(self.next as u64 * 1_000, ClockSource::Hardware),
            Provenance::new(Origin::Simulated).with_step(ProvenanceStep::Sensor(self.id)),
        ))
    }
}

fn thermal_sensor(script: Vec<Result<f64, SensorError>>) -> ScriptedSensor {
    ScriptedSensor {
        id: SensorId::new(42),
        caps: SensorCapabilities::new(vec![Modality::Thermal]),
        script,
        next: 0,
    }
}

#[test]
fn sensor_to_observation_pipeline_preserves_provenance() {
    let mut sensor = thermal_sensor(vec![Ok(37.4)]);
    assert_eq!(sensor.health(), SensorHealth::Degraded);
    assert!(sensor.health().is_operational());

    let measurement = sensor.read().expect("scripted reading");
    assert_eq!(measurement.sensor, sensor.identity());
    assert_eq!(measurement.value, MeasurementValue::Scalar(37.4));

    let obs = Observation::from_measurement(
        &measurement,
        ObservationId::new(1),
        Property::new("temperature_state"),
        ObservationValue::Categorical("normal".into()),
        Confidence::new(0.87).unwrap(),
    )
    .with_subject(EntityId::new(1847));

    assert_eq!(obs.subject(), Some(EntityId::new(1847)));
    assert_eq!(obs.provenance().origin(), Origin::Simulated);
    let sensors: Vec<_> = obs.provenance().sensors().collect();
    assert_eq!(sensors, vec![SensorId::new(42), SensorId::new(42)]);
}

#[test]
fn sensor_errors_propagate() {
    let mut sensor = thermal_sensor(vec![Err(SensorError::Timeout)]);
    assert_eq!(sensor.read().unwrap_err(), SensorError::Timeout);
    // Past the script the sensor reports NotReady.
    assert_eq!(sensor.read().unwrap_err(), SensorError::NotReady);
}

#[test]
fn measurement_uncertainty_is_carried_into_observation() {
    let uncertainty = Uncertainty::new()
        .with_standard_deviation(StandardDeviation::new(0.1).unwrap())
        .with_bounds(Bounds::new(37.0, 37.8).unwrap())
        .with_distribution(Distribution::Gaussian);

    let measurement = Measurement::new(
        SensorId::new(5),
        Modality::Thermal,
        MeasurementValue::Scalar(37.4),
        Timestamp::new(0, ClockSource::Monotonic),
        Provenance::new(Origin::Real),
    )
    .with_uncertainty(uncertainty.clone());

    let obs = Observation::from_measurement(
        &measurement,
        ObservationId::new(2),
        Property::new("temperature"),
        ObservationValue::Scalar(37.4),
        Confidence::new(0.5).unwrap(),
    );
    assert_eq!(obs.uncertainty(), Some(&uncertainty));
}

#[test]
fn observations_start_as_evidence_not_fact() {
    let obs = Observation::new(
        ObservationId::new(3),
        Modality::Visual,
        Property::new("obstacle_detected"),
        ObservationValue::Boolean(true),
        Confidence::new(0.96).unwrap(),
        Timestamp::new(0, ClockSource::Monotonic),
        Provenance::new(Origin::Real)
            .with_step(ProvenanceStep::Algorithm(Algorithm::new("detector", "v4")))
            .with_step(ProvenanceStep::Algorithm(Algorithm::new("fusion", "v2"))),
    );
    assert_eq!(obs.state(), ObservationState::Raw);
    assert_eq!(obs.provenance().algorithms().count(), 2);
}

#[test]
fn timestamps_from_different_clocks_are_never_simultaneous() {
    let hw = Timestamp::new(1_000, ClockSource::Hardware);
    let wall = Timestamp::new(1_000, ClockSource::Wall);
    assert_ne!(hw, wall);
    assert!(hw.partial_cmp(&wall).is_none());
    assert!(hw.partial_cmp(&wall).is_none() && hw != wall);
}

#[test]
fn value_types_reject_invalid_input() {
    assert!(Confidence::new(1.5).is_none());
    assert!(Range::new(2.0, 1.0).is_none());
    assert!(Resolution::new(0.0).is_none());
    assert!(Frequency::from_hz(0.0).is_none());
    assert_eq!(Duration::from_millis(5).as_nanos(), 5_000_000);
}

#[test]
fn custom_modalities_work_end_to_end() {
    let caps = SensorCapabilities::new(vec![Modality::Custom("gravitational".into())]);
    assert!(caps.supports(&Modality::Custom("gravitational".into())));

    let m = Measurement::new(
        SensorId::new(9),
        Modality::Custom("gravitational".into()),
        MeasurementValue::Vector(vec![0.0, 0.0, -9.81]),
        Timestamp::new(0, ClockSource::Simulated),
        Provenance::new(Origin::Simulated),
    );
    let obs = Observation::from_measurement(
        &m,
        ObservationId::new(4),
        Property::new("gravity_vector"),
        ObservationValue::Vector(vec![0.0, 0.0, -9.81]),
        Confidence::new(1.0).unwrap(),
    );
    assert!(obs.modality().is_custom());
    assert_eq!(obs.modality().name(), "gravitational");
}

#[test]
fn lifecycle_rejects_backwards_transitions() {
    let mut obs = Observation::new(
        ObservationId::new(5),
        Modality::Acceleration,
        Property::new("shock"),
        ObservationValue::Scalar(12.0),
        Confidence::new(0.7).unwrap(),
        Timestamp::new(0, ClockSource::Monotonic),
        Provenance::new(Origin::Real),
    );
    obs.advance_to(ObservationState::Validated).unwrap();
    obs.advance_to(ObservationState::Archived).unwrap();
    let err = obs.advance_to(ObservationState::Raw).unwrap_err();
    assert_eq!(err.to, ObservationState::Raw);
    assert_eq!(obs.state(), ObservationState::Archived);
}
