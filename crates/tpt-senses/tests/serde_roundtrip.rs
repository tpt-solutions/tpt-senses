//! Serialization tests (feature `serde`). Round-trips must preserve every field, and
//! deserialization must reject values that the in-memory constructors would reject.
#![cfg(feature = "serde")]

use serde_json::{Value, json};
use std::sync::Arc;
use tpt_senses::prelude::*;

use tpt_senses::{
    Algorithm, AuthenticationStatus, Bounds, CalibrationKind, CalibrationRecord, CoordinateFrame,
    Frequency, Measurement, MeasurementValue, SourceIdentity, SpatialReference, TrustLevel,
    TrustMetadata, Uncertainty,
};

fn mono(ns: u64) -> Timestamp {
    Timestamp::new(ns, ClockSource::Monotonic)
}

fn full_observation() -> Observation {
    let mut provenance = Provenance::new(Origin::Real)
        .with_step(ProvenanceStep::Hardware {
            device: "cam-0".into(),
        })
        .with_step(ProvenanceStep::Sensor(SensorId::new(7)))
        .with_step(ProvenanceStep::Algorithm(
            Algorithm::new("detector", "v4").with_parameter("threshold", "0.5"),
        ));
    provenance.push(ProvenanceStep::Calibration(
        CalibrationRecord::new(CalibrationKind::Intrinsic, mono(1))
            .with_parameter("focal_px", 800.0),
    ));

    Observation::new(
        ObservationId::new(42),
        Modality::Custom("gravitational".into()),
        Property::new("vehicle_position"),
        ObservationValue::Vector(vec![1.0, 2.0, 0.5]),
        Confidence::new(0.87).unwrap(),
        mono(1_000),
        provenance,
    )
    .with_subject(EntityId::new(27))
    .with_uncertainty(Uncertainty {
        standard_deviation: Some(0.2),
        bounds: Bounds::new(0.5, 1.5),
        ..Default::default()
    })
    .with_location(SpatialReference::at(
        CoordinateFrame::World,
        [1.0, 2.0, 0.5],
    ))
    .with_trust(TrustMetadata {
        source: Some(SourceIdentity::new("robot-7")),
        authentication: AuthenticationStatus::Authenticated,
        level: TrustLevel::Verified,
        ..TrustMetadata::unverified()
    })
}

#[test]
fn observation_round_trips_through_json() {
    let original = full_observation();
    let text = serde_json::to_string(&original).unwrap();
    let back: Observation = serde_json::from_str(&text).unwrap();
    assert_eq!(back, original);
}

#[test]
fn observation_round_trip_keeps_provenance_chain_in_order() {
    let original = full_observation();
    let back: Observation =
        serde_json::from_value(serde_json::to_value(&original).unwrap()).unwrap();
    assert_eq!(back.provenance().chain(), original.provenance().chain());
    assert_eq!(back.provenance().calibrations().count(), 1);
}

fn serialized() -> Value {
    serde_json::to_value(full_observation()).unwrap()
}

#[test]
fn confidence_outside_unit_interval_is_rejected() {
    let mut v = serialized();
    v["confidence"] = json!(7.0);
    assert!(serde_json::from_value::<Observation>(v).is_err());
}

#[test]
fn negative_standard_deviation_is_rejected() {
    let mut v = serialized();
    v["uncertainty"]["standard_deviation"] = json!(-1.0);
    assert!(serde_json::from_value::<Observation>(v).is_err());
}

#[test]
fn inverted_bounds_are_rejected() {
    let mut v = serialized();
    v["uncertainty"]["bounds"] = json!({ "lower": 3.0, "upper": 1.0 });
    assert!(serde_json::from_value::<Observation>(v).is_err());
}

#[test]
fn negative_precision_is_rejected() {
    let mut v = serialized();
    v["uncertainty"]["precision"] = json!(-0.1);
    assert!(serde_json::from_value::<Observation>(v).is_err());
}

#[test]
fn unknown_lifecycle_state_is_rejected() {
    let mut v = serialized();
    v["state"] = json!("Blessed");
    assert!(serde_json::from_value::<Observation>(v).is_err());
}

#[test]
fn identifiers_and_timestamps_round_trip() {
    let id = ObservationId::new(9);
    let back: ObservationId = serde_json::from_str(&serde_json::to_string(&id).unwrap()).unwrap();
    assert_eq!(back, id);

    let ts = Timestamp::new(123, ClockSource::Hardware);
    let back: Timestamp = serde_json::from_str(&serde_json::to_string(&ts).unwrap()).unwrap();
    assert_eq!(back, ts);
}

#[test]
fn valid_uncertainty_round_trips_on_its_own() {
    let u = Uncertainty {
        standard_deviation: Some(0.0),
        bounds: Bounds::new(-1.0, 1.0),
        ..Default::default()
    };
    let back: Uncertainty = serde_json::from_str(&serde_json::to_string(&u).unwrap()).unwrap();
    assert_eq!(back, u);
}

// ---- Measurements ---------------------------------------------------------------------------

fn measurement(value: MeasurementValue) -> Measurement {
    Measurement::new(
        SensorId::new(3),
        Modality::Visual,
        value,
        mono(5),
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(3))),
    )
}

fn measurement_json(value: MeasurementValue) -> Value {
    serde_json::to_value(measurement(value)).unwrap()
}

#[test]
fn every_measurement_variant_round_trips() {
    let image = MeasurementValue::image(2, 2, 3, Arc::from(vec![7u8; 12])).unwrap();
    let matrix = MeasurementValue::matrix(2, 2, vec![1.0, 0.0, 0.0, 1.0]).unwrap();
    let audio = MeasurementValue::audio(
        Frequency::from_hz(16_000.0).unwrap(),
        2,
        Arc::from(vec![0.0f32, 0.1, 0.2, 0.3]),
    )
    .unwrap();
    let spectrum = MeasurementValue::spectrum(vec![450.0, 550.0], vec![0.2, 0.9]).unwrap();
    let nested = MeasurementValue::Structured(vec![
        ("gain".into(), MeasurementValue::Scalar(2.0)),
        ("bands".into(), spectrum.clone()),
        ("label".into(), MeasurementValue::Categorical("crop".into())),
    ]);
    let cloud = MeasurementValue::PointCloud(Arc::from(vec![[0.0f32, 1.0, 2.0]]));
    let binary = MeasurementValue::Binary {
        media_type: "application/x-custom".into(),
        data: Arc::from(vec![1u8, 2, 3]),
    };
    for value in [
        MeasurementValue::Scalar(1.5),
        MeasurementValue::Vector(vec![1.0, 2.0]),
        matrix,
        image,
        audio,
        spectrum,
        nested,
        cloud,
        binary,
    ] {
        let m = measurement(value);
        let text = serde_json::to_string(&m).unwrap();
        let back: Measurement = serde_json::from_str(&text).unwrap();
        assert_eq!(back, m);
    }
}

#[test]
fn matrix_with_wrong_element_count_is_rejected() {
    let mut v = measurement_json(MeasurementValue::matrix(2, 2, vec![1.0; 4]).unwrap());
    v["value"]["Matrix"]["rows"] = json!(3);
    assert!(serde_json::from_value::<Measurement>(v).is_err());
}

#[test]
fn image_with_wrong_byte_count_is_rejected() {
    let mut v =
        measurement_json(MeasurementValue::image(2, 2, 3, Arc::from(vec![0u8; 12])).unwrap());
    v["value"]["Image"]["width"] = json!(5);
    assert!(serde_json::from_value::<Measurement>(v).is_err());
}

#[test]
fn audio_that_is_not_whole_frames_is_rejected() {
    let mut v = measurement_json(
        MeasurementValue::audio(
            Frequency::from_hz(8_000.0).unwrap(),
            2,
            Arc::from(vec![0.0f32; 4]),
        )
        .unwrap(),
    );
    v["value"]["Audio"]["samples"] = json!([0.0, 0.1, 0.2]);
    assert!(serde_json::from_value::<Measurement>(v).is_err());
}

#[test]
fn audio_with_zero_channels_or_bad_rate_is_rejected() {
    let base = measurement_json(
        MeasurementValue::audio(
            Frequency::from_hz(8_000.0).unwrap(),
            1,
            Arc::from(vec![0.0f32; 4]),
        )
        .unwrap(),
    );
    let mut zero_channels = base.clone();
    zero_channels["value"]["Audio"]["channels"] = json!(0);
    assert!(serde_json::from_value::<Measurement>(zero_channels).is_err());

    let mut bad_rate = base;
    bad_rate["value"]["Audio"]["sample_rate_hz"] = json!(-8.0);
    assert!(serde_json::from_value::<Measurement>(bad_rate).is_err());
}

#[test]
fn spectrum_with_mismatched_lengths_is_rejected() {
    let mut v =
        measurement_json(MeasurementValue::spectrum(vec![1.0, 2.0], vec![3.0, 4.0]).unwrap());
    v["value"]["Spectrum"]["intensities"] = json!([3.0]);
    assert!(serde_json::from_value::<Measurement>(v).is_err());
}

#[test]
fn invalid_value_nested_inside_structure_is_rejected() {
    let nested = MeasurementValue::Structured(vec![(
        "grid".into(),
        MeasurementValue::matrix(1, 1, vec![0.5]).unwrap(),
    )]);
    let mut v = measurement_json(nested);
    v["value"]["Structured"][0][1]["Matrix"]["cols"] = json!(4);
    assert!(serde_json::from_value::<Measurement>(v).is_err());
}
