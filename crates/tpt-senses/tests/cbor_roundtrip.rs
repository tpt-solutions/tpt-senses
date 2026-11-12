//! CBOR tests (feature `cbor`). CBOR and JSON must agree on the model, and CBOR decoding must
//! apply the same validation as JSON decoding.
#![cfg(feature = "cbor")]

use std::sync::Arc;

use serde_json::{Value, json};
use tpt_senses::StandardDeviation;
use tpt_senses::prelude::*;
use tpt_senses::{
    Bounds, Confidence, Frequency, Measurement, MeasurementValue, Uncertainty, from_cbor, to_cbor,
};

fn mono(ns: u64) -> Timestamp {
    Timestamp::new(ns, ClockSource::Monotonic)
}

fn observation() -> Observation {
    Observation::new(
        ObservationId::new(5),
        Modality::Thermal,
        Property::new("temperature_c"),
        ObservationValue::Scalar(37.4),
        Confidence::new(0.9).unwrap(),
        mono(10),
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(2))),
    )
    .with_uncertainty(
        Uncertainty::new()
            .with_standard_deviation(StandardDeviation::new(0.1).unwrap())
            .with_bounds(Bounds::new(37.0, 37.8).unwrap()),
    )
}

#[test]
fn observation_round_trips_through_cbor() {
    let original = observation();
    let bytes = to_cbor(&original).unwrap();
    let back: Observation = from_cbor(&bytes).unwrap();
    assert_eq!(back, original);
}

#[test]
fn measurement_with_shared_buffers_round_trips_through_cbor() {
    let m = Measurement::new(
        SensorId::new(4),
        Modality::Visual,
        MeasurementValue::audio(
            Frequency::from_hz(16_000.0).unwrap(),
            2,
            Arc::from(vec![0.0f32, 0.5, -0.5, 0.25]),
        )
        .unwrap(),
        mono(0),
        Provenance::new(Origin::Recorded),
    );
    let back: Measurement = from_cbor(&to_cbor(&m).unwrap()).unwrap();
    assert_eq!(back, m);
}

#[test]
fn encoding_is_deterministic() {
    let o = observation();
    assert_eq!(to_cbor(&o).unwrap(), to_cbor(&o).unwrap());
}

#[test]
fn cbor_and_json_decode_to_the_same_value() {
    let o = observation();
    let from_json: Observation = serde_json::from_value(serde_json::to_value(&o).unwrap()).unwrap();
    let from_bin: Observation = from_cbor(&to_cbor(&o).unwrap()).unwrap();
    assert_eq!(from_json, from_bin);
}

/// Encodes a mutated JSON document as CBOR, so the CBOR decoder sees the malformed input.
fn cbor_of(mut v: Value, edit: impl FnOnce(&mut Value)) -> Vec<u8> {
    edit(&mut v);
    to_cbor(&v).unwrap()
}

#[test]
fn cbor_rejects_out_of_range_confidence() {
    let base = serde_json::to_value(observation()).unwrap();
    let bytes = cbor_of(base, |v| {
        v["confidence"] = json!(7.0);
    });
    assert!(from_cbor::<Observation>(&bytes).is_err());
}

#[test]
fn cbor_rejects_negative_standard_deviation() {
    let base = serde_json::to_value(observation()).unwrap();
    let bytes = cbor_of(base, |v| {
        v["uncertainty"]["standard_deviation"] = json!(-1.0);
    });
    assert!(from_cbor::<Observation>(&bytes).is_err());
}

#[test]
fn cbor_rejects_inverted_bounds() {
    let base = serde_json::to_value(observation()).unwrap();
    let bytes = cbor_of(base, |v| {
        v["uncertainty"]["bounds"] = json!({ "lower": 2.0, "upper": 1.0 });
    });
    assert!(from_cbor::<Observation>(&bytes).is_err());
}

#[test]
fn cbor_rejects_malformed_bytes() {
    assert!(from_cbor::<Observation>(&[0xff, 0x00, 0x13]).is_err());
    let err = from_cbor::<Confidence>(&[0xff]).unwrap_err();
    assert!(!err.message().is_empty());
}

#[test]
fn confidence_alone_round_trips() {
    let c = Confidence::new(0.25).unwrap();
    let back: Confidence = from_cbor(&to_cbor(&c).unwrap()).unwrap();
    assert_eq!(back, c);
    assert!(from_cbor::<Confidence>(&to_cbor(&2.0_f64).unwrap()).is_err());
}
