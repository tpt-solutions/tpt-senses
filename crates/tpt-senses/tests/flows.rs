#![allow(clippy::float_cmp, reason = "tests compare exact configured values")]
//! Integration flows from spec §44. Each test plays the role of an external adapter (the
//! "tpt-cv", "tpt-audio", ROS, or detector side) and checks that the core turns its output into
//! observations that keep their provenance, uncertainty and lifecycle rules.

use tpt_senses::StandardDeviation;
use tpt_senses::prelude::*;
use tpt_senses::{
    Adapter, AdapterError, AttentionHint, CalibrationKind, CalibrationRecord, CalibrationStage,
    CalibrationState, CoordinateFrame, EventKind, Frequency, InformationNeed,
    ParameterChangeRequest, ParameterKind, Pipeline, RecordSink, Recording, Sample,
    SensorCandidate, SensorCapabilities, SensorHealth, SensoryProfile, SpatialReference,
    Uncertainty, ValidationStage, attention_sampling, information_request_for, movement_event,
    replay_through, select_sensor, threshold_event,
};

fn mono(ns: u64) -> Timestamp {
    Timestamp::new(ns, ClockSource::Monotonic)
}

fn conf(v: f64) -> Confidence {
    Confidence::new(v).unwrap()
}

// ---- Camera (§44): Camera → tpt-cv → visual detection → tpt-senses → Observation -----------

/// A detection as a vision library would report it.
struct Detection {
    label: &'static str,
    score: f64,
    camera: u64,
    frame_ns: u64,
}

struct VisionAdapter {
    next_id: u64,
}

impl Adapter for VisionAdapter {
    type Input = Detection;

    fn name(&self) -> &'static str {
        "tpt-cv-adapter"
    }

    fn adapt(&mut self, d: Detection) -> Result<Vec<Observation>, AdapterError> {
        if d.label.is_empty() {
            return Err(AdapterError::Invalid("empty label".into()));
        }
        let c = Confidence::new(d.score)
            .ok_or_else(|| AdapterError::Invalid("score out of range".into()))?;
        self.next_id += 1;
        Ok(vec![Observation::new(
            ObservationId::new(self.next_id),
            Modality::Visual,
            Property::new(format!("detected_{}", d.label)),
            ObservationValue::Boolean(true),
            c,
            mono(d.frame_ns),
            Provenance::new(Origin::Real)
                .with_step(ProvenanceStep::Sensor(SensorId::new(d.camera))),
        )])
    }
}

#[test]
fn camera_detection_becomes_observation_through_pipeline() {
    let mut adapter = VisionAdapter { next_id: 0 };
    let obs = adapter
        .adapt(Detection {
            label: "pedestrian",
            score: 0.93,
            camera: 1,
            frame_ns: 33_000_000,
        })
        .unwrap()
        .remove(0);
    let out = Pipeline::new()
        .with_stage(ValidationStage::new(conf(0.5)))
        .run(obs)
        .unwrap();
    assert_eq!(out.observation.state(), ObservationState::Validated);
    assert_eq!(out.observation.provenance().origin(), Origin::Real);
    assert_eq!(out.observation.property().as_str(), "detected_pedestrian");
}

#[test]
fn camera_adapter_rejects_bad_detections() {
    let mut adapter = VisionAdapter { next_id: 0 };
    assert!(
        adapter
            .adapt(Detection {
                label: "x",
                score: 1.5,
                camera: 1,
                frame_ns: 0
            })
            .is_err()
    );
    assert!(
        adapter
            .adapt(Detection {
                label: "",
                score: 0.5,
                camera: 1,
                frame_ns: 0
            })
            .is_err()
    );
}

// ---- Microphone (§44): Microphone → tpt-audio → features → (tpt-voice optional) → Observation

#[test]
fn non_speech_sound_becomes_sound_event_not_speech() {
    // Audio features arrive from an audio pipeline. Speech recognition is optional, and this
    // path has none: the sound is represented as an auditory observation.
    let loudness_db = Observation::new(
        ObservationId::new(10),
        Modality::Auditory,
        Property::new("loudness_db"),
        ObservationValue::Scalar(92.0),
        conf(0.8),
        mono(500_000_000),
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(7))),
    );
    let event = threshold_event(
        &loudness_db,
        EventKind::SoundDetected,
        80.0,
        SensorId::new(7),
    )
    .expect("loud sound should be an event");
    assert_eq!(event.kind, EventKind::SoundDetected);
    assert_eq!(event.measurements, [] as [tpt_senses::Measurement; 0]);
    assert_eq!(event.modality, Modality::Auditory);
}

// ---- Agricultural robot (§44, §36): ROS sensors → tpt-senses → decision ------------------------

#[test]
fn weed_vs_not_weed_uses_active_perception_follow_up() {
    // Step 1: a visual classification with low confidence.
    let first = Observation::new(
        ObservationId::new(20),
        Modality::Visual,
        Property::new("weed_probability"),
        ObservationValue::Scalar(0.55),
        conf(0.55),
        mono(0),
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(1))),
    );

    // Step 2: the uncertainty triggers a request for a multispectral follow-up.
    let suggestion =
        ParameterChangeRequest::new(SensorId::new(2), ParameterKind::SpectralBand, 740.0)
            .with_reason("separate weed from crop canopy");
    let request = information_request_for(
        first.id(),
        first.property().clone(),
        first.confidence(),
        conf(0.8),
        vec![suggestion],
    )
    .expect("low confidence should request more information");
    assert_eq!(request.need, InformationNeed::LowConfidence);

    // Step 3: pick the multispectral sensor from the robot's sensors.
    let sensors = [
        SensorCandidate::new(
            SensorId::new(1),
            SensorCapabilities::new(vec![Modality::Visual]),
            SensorHealth::Online,
            Some(0.05),
        ),
        SensorCandidate::new(
            SensorId::new(2),
            SensorCapabilities::new(vec![Modality::Multispectral]),
            SensorHealth::Online,
            Some(0.1),
        ),
    ];
    assert_eq!(
        select_sensor(&sensors, &Modality::Multispectral),
        Some(SensorId::new(2))
    );

    // Step 4: attention raises the sampling rate of that sensor.
    let rate = attention_sampling(
        Frequency::from_hz(2.0).unwrap(),
        Frequency::from_hz(20.0).unwrap(),
        AttentionHint::new(None, conf(1.0)),
    )
    .unwrap();
    assert_eq!(rate.hz(), 20.0);

    // Step 5: the follow-up observation is sharper, and it is recorded as a separate observation
    // that does not replace the first one.
    let follow_up = Observation::new(
        ObservationId::new(21),
        Modality::Multispectral,
        Property::new("weed_probability"),
        ObservationValue::Scalar(0.92),
        conf(0.92),
        mono(100_000_000),
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(2))),
    );
    assert!(follow_up.confidence() > first.confidence());
    assert_ne!(first.id(), follow_up.id());
}

#[test]
fn agricultural_profile_capabilities_cover_the_flow() {
    let caps = SensoryProfile::AgriculturalRobot.capabilities();
    for m in [
        Modality::Visual,
        Modality::Multispectral,
        Modality::Thermal,
        Modality::Lidar,
    ] {
        assert!(caps.supports(&m));
    }
}

#[test]
fn robot_position_fix_with_frame_and_uncertainty() {
    let gps = Observation::new(
        ObservationId::new(30),
        Modality::Position,
        Property::new("robot_pose"),
        ObservationValue::Vector(vec![0.0, 0.0, 0.0]),
        conf(0.9),
        mono(0),
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(3))),
    )
    .with_location(SpatialReference::at(
        CoordinateFrame::World,
        [512.4, 88.1, 0.0],
    ))
    .with_uncertainty(
        Uncertainty::new().with_standard_deviation(StandardDeviation::new(1.5).unwrap()),
    );
    assert_eq!(
        gps.uncertainty().unwrap().standard_deviation,
        StandardDeviation::new(1.5)
    );
    assert_eq!(gps.location().unwrap().frame, CoordinateFrame::World);
}

// ---- Radiation sensor (§44): no human-equivalent sense is required ----------------------------

#[test]
fn radiation_spike_is_an_event_with_no_human_analogue() {
    let dose = Observation::new(
        ObservationId::new(40),
        Modality::Radiation,
        Property::new("dose_rate_usv_h"),
        ObservationValue::Scalar(4.2),
        conf(0.99),
        mono(9),
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(9))),
    );
    let spike = threshold_event(&dose, EventKind::RadiationSpike, 1.0, SensorId::new(9)).unwrap();
    assert_eq!(spike.kind, EventKind::RadiationSpike);
    assert_eq!(spike.modality, Modality::Radiation);
    assert!(
        SensoryProfile::Spacecraft
            .capabilities()
            .supports(&Modality::Radiation)
    );
}

// ---- Calibration gates the whole chain --------------------------------------------------------

#[test]
fn uncalibrated_radiation_reading_is_not_passed_on() {
    let mut cal = CalibrationState::new();
    cal.record(CalibrationRecord::new(CalibrationKind::ZeroPoint, mono(0)).failed());
    let pipeline = Pipeline::new().with_stage(CalibrationStage::new(
        cal,
        CalibrationKind::ZeroPoint,
        mono(5),
    ));
    let reading = Observation::new(
        ObservationId::new(41),
        Modality::Radiation,
        Property::new("dose_rate_usv_h"),
        ObservationValue::Scalar(0.1),
        conf(0.9),
        mono(5),
        Provenance::new(Origin::Real),
    );
    assert!(pipeline.run(reading).is_err());
}

// ---- Replay: same recording, same pipeline, same observations (§28) --------------------------

#[test]
fn replayed_recording_yields_identical_observations() {
    let mut recording = Recording::new();
    for (i, v) in [0.2, 0.9, 0.4, 0.95].into_iter().enumerate() {
        recording
            .write(Sample::new(mono(i as u64 * 1_000_000), v))
            .unwrap();
    }
    let process = |s: &Sample<f64>| {
        let c = Confidence::new(s.value)?;
        Some(Observation::new(
            ObservationId::new(s.timestamp.nanos()),
            Modality::Visual,
            Property::new("presence"),
            ObservationValue::Boolean(true),
            c,
            s.timestamp,
            Provenance::new(Origin::Recorded),
        ))
    };
    let first = replay_through(&recording, process);
    let second = replay_through(&recording, process);
    assert_eq!(first, second);
    assert_eq!(first.len(), 4);
}

#[test]
fn movement_between_replayed_fixes_is_detected() {
    let a = Observation::new(
        ObservationId::new(50),
        Modality::Position,
        Property::new("robot_pose"),
        ObservationValue::Vector(vec![0.0; 3]),
        conf(0.9),
        mono(0),
        Provenance::new(Origin::Recorded),
    )
    .with_location(SpatialReference::at(
        CoordinateFrame::World,
        [0.0, 0.0, 0.0],
    ));
    let b = Observation::new(
        ObservationId::new(51),
        Modality::Position,
        Property::new("robot_pose"),
        ObservationValue::Vector(vec![0.0; 3]),
        conf(0.9),
        mono(1_000_000_000),
        Provenance::new(Origin::Recorded),
    )
    .with_location(SpatialReference::at(
        CoordinateFrame::World,
        [0.0, 2.0, 0.0],
    ));
    let ev = movement_event(&a, &b, 1.0, SensorId::new(3)).unwrap();
    assert_eq!(ev.kind, EventKind::ObjectMoved);
}
