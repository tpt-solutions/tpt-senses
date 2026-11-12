//! Phase 3 tests: perception pipeline, fusion, entity association, world state, internal sensing
//! and event detection. Includes the spec's example: camera + `LiDAR` + radar + GPS → "Vehicle #27".

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "test arithmetic on small trial indices and a seeded PRNG"
)]

use tpt_senses::StandardDeviation;
use tpt_senses::prelude::*;
use tpt_senses::{
    Algorithm, AssociationConfig, CalibrationKind, CalibrationRecord, CalibrationStage,
    CalibrationState, ConfidenceWeighted, CoordinateFrame, Duration, EntityKind, EventKind,
    FnStage, FusionConfig, FusionError, FusionStrategy, InternalMetric, InverseVariance,
    MetaSignal, Pipeline, Point3, ScalarInput, SensorCandidate, SensorCapabilities, SensorHealth,
    SpatialReference, StageOutcome, Transform, Translation, TrustLevel, Uncertainty,
    ValidationStage, WorldState, associate, fuse, fuse_presence, internal_observation,
    meta_observation, movement_event, select_sensor, threshold_event,
};

fn mono(ns: u64) -> Timestamp {
    Timestamp::new(ns, ClockSource::Monotonic)
}

fn conf(v: f64) -> Confidence {
    Confidence::new(v).unwrap()
}

fn located(
    id: u64,
    sensor: u64,
    modality: Modality,
    frame: CoordinateFrame,
    pos: Point3,
    ts: Timestamp,
    c: f64,
) -> Observation {
    Observation::new(
        ObservationId::new(id),
        modality,
        Property::new("position"),
        ObservationValue::Vector(pos.to_vec()),
        conf(c),
        ts,
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(sensor))),
    )
    .with_location(SpatialReference::at(frame, pos))
}

// ---- Pipeline -------------------------------------------------------------------------------

#[test]
fn pipeline_records_every_stage_for_explainability() {
    let mut cal = CalibrationState::new();
    cal.record(
        CalibrationRecord::new(CalibrationKind::Intrinsic, mono(0))
            .with_parameter("focal_px", 900.0),
    );

    let pipeline = Pipeline::new()
        .with_stage(ValidationStage::new(conf(0.2)))
        .with_stage(CalibrationStage::new(
            cal,
            CalibrationKind::Intrinsic,
            mono(10),
        ))
        .with_stage(FnStage::new(
            Algorithm::new("threshold-detector", "4").with_parameter("threshold", "0.5"),
            ObservationState::Processed,
            |o: Observation| StageOutcome::Pass(o),
        ));

    let input = Observation::new(
        ObservationId::new(1),
        Modality::Visual,
        Property::new("obstacle_detected"),
        ObservationValue::Boolean(true),
        conf(0.96),
        mono(5),
        Provenance::new(Origin::Real),
    );
    let out = pipeline.run(input).unwrap();

    assert_eq!(out.observation.state(), ObservationState::Processed);
    let names: Vec<&str> = out
        .trace
        .iter()
        .map(|r| r.algorithm.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["validation", "calibration-check", "threshold-detector"]
    );
    assert_eq!(out.observation.provenance().algorithms().count(), 3);
    assert_eq!(out.observation.provenance().calibrations().count(), 1);
}

#[test]
fn pipeline_stops_on_rejection_and_reports_stage() {
    let pipeline = Pipeline::new()
        .with_stage(FnStage::new(
            Algorithm::new("reject-all", "1"),
            ObservationState::Validated,
            |_| StageOutcome::Reject("sensor occluded".into()),
        ))
        .with_stage(ValidationStage::new(conf(0.0)));
    let input = Observation::new(
        ObservationId::new(2),
        Modality::Visual,
        Property::new("x"),
        ObservationValue::Boolean(false),
        conf(0.9),
        mono(0),
        Provenance::new(Origin::Real),
    );
    let err = pipeline.run(input).unwrap_err();
    assert_eq!(err.stage, "reject-all");
    assert_eq!(err.reason, "sensor occluded");
}

// ---- Fusion ---------------------------------------------------------------------------------

fn range_reading(
    id: u64,
    sensor: u64,
    value: f64,
    sigma: Option<f64>,
    ts: Timestamp,
) -> Observation {
    let mut o = Observation::new(
        ObservationId::new(id),
        Modality::Depth,
        Property::new("range_m"),
        ObservationValue::Scalar(value),
        conf(0.9),
        ts,
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(sensor))),
    );
    if let Some(s) = sigma {
        if let Some(sd) = StandardDeviation::new(s) {
            o = o.with_uncertainty(Uncertainty::new().with_standard_deviation(sd));
        }
    }
    o
}

#[test]
fn fusion_of_two_sensors_preserves_both_in_provenance() {
    let cfg = FusionConfig {
        time_tolerance: Duration::from_millis(5),
        disagreement_threshold: 1.0,
    };
    let lidar = range_reading(10, 1, 25.00, Some(0.02), mono(0));
    let radar = range_reading(11, 2, 25.30, Some(0.5), mono(1_000_000));
    let fused = fuse(
        &[lidar, radar],
        ObservationId::new(100),
        &InverseVariance,
        &cfg,
    )
    .unwrap();
    let ObservationValue::Scalar(v) = fused.observation.value() else {
        panic!()
    };
    assert!((v - 25.0).abs() < 0.05, "lidar should dominate, got {v}");
    assert_eq!(
        fused.observation.provenance().sensors().collect::<Vec<_>>(),
        vec![SensorId::new(1), SensorId::new(2)]
    );
    assert!(fused.disagreement.is_none());
}

#[test]
fn fusion_fault_sensor_disagreement_names_the_outlier() {
    let cfg = FusionConfig {
        time_tolerance: Duration::from_millis(5),
        disagreement_threshold: 2.0,
    };
    let a = range_reading(20, 1, 10.0, None, mono(0));
    let b = range_reading(21, 2, 10.1, None, mono(0));
    let c = range_reading(22, 3, 17.0, None, mono(0)); // faulty sensor
    let fused = fuse(
        &[a, b, c],
        ObservationId::new(101),
        &ConfidenceWeighted,
        &cfg,
    )
    .unwrap();
    let d = fused.disagreement.expect("disagreement must be reported");
    assert_eq!(d.outlier_sensors, vec![SensorId::new(3)]);
}

#[test]
fn fusion_refuses_inputs_that_are_not_simultaneous() {
    let cfg = FusionConfig {
        time_tolerance: Duration::from_millis(1),
        disagreement_threshold: 1.0,
    };
    let a = range_reading(30, 1, 5.0, None, mono(0));
    let b = range_reading(31, 2, 5.0, None, mono(500_000_000));
    assert_eq!(
        fuse(&[a, b], ObservationId::new(102), &ConfidenceWeighted, &cfg).unwrap_err(),
        FusionError::NotSimultaneous
    );
}

struct Clamped;
impl FusionStrategy for Clamped {
    fn name(&self) -> &'static str {
        "clamped-mean"
    }
    fn combine(&self, inputs: &[ScalarInput]) -> Option<(f64, Option<f64>)> {
        #[allow(clippy::cast_precision_loss, reason = "test input counts are tiny")]
        let mean = inputs.iter().map(|i| i.value).sum::<f64>() / inputs.len() as f64;
        Some((mean.clamp(0.0, 100.0), None))
    }
}

#[test]
fn custom_fusion_strategy_is_used_and_recorded() {
    let cfg = FusionConfig {
        time_tolerance: Duration::from_millis(1),
        disagreement_threshold: 1000.0,
    };
    let inputs = [
        range_reading(40, 1, 150.0, None, mono(0)),
        range_reading(41, 2, 130.0, None, mono(0)),
    ];
    let fused = fuse(&inputs, ObservationId::new(103), &Clamped, &cfg).unwrap();
    assert_eq!(fused.observation.value(), &ObservationValue::Scalar(100.0));
    assert!(
        fused
            .observation
            .provenance()
            .algorithms()
            .any(|a| a.name == "clamped-mean")
    );
}

// ---- Entity association: "Vehicle #27" ------------------------------------------------------

#[test]
fn camera_lidar_radar_gps_resolve_to_vehicle_27() {
    // Camera reports in its own frame; a fixed transform brings it to world coordinates.
    let camera_to_world = Translation::new(
        CoordinateFrame::Camera,
        CoordinateFrame::World,
        [100.0, 50.0, 0.0],
    );
    let camera_obs = located(
        1,
        1,
        Modality::Visual,
        CoordinateFrame::Camera,
        [-89.8, -49.9, 0.0],
        mono(0),
        0.8,
    );
    let camera_world = {
        let r = camera_to_world
            .transform_reference(camera_obs.location().unwrap())
            .unwrap();
        camera_obs.clone().with_location(r)
    };

    let lidar = located(
        2,
        2,
        Modality::Lidar,
        CoordinateFrame::World,
        [10.1, 0.05, 0.0],
        mono(2_000_000),
        0.95,
    );
    let radar = located(
        3,
        3,
        Modality::Radar,
        CoordinateFrame::World,
        [10.3, -0.1, 0.0],
        mono(4_000_000),
        0.7,
    );
    let gps = located(
        4,
        4,
        Modality::Position,
        CoordinateFrame::World,
        [9.9, 0.0, 0.0],
        mono(6_000_000),
        0.6,
    );
    let unrelated = located(
        5,
        5,
        Modality::Lidar,
        CoordinateFrame::World,
        [500.0, 0.0, 0.0],
        mono(0),
        0.9,
    );

    let observations = [camera_world, lidar, radar, gps, unrelated];
    let cfg = AssociationConfig {
        max_distance: 2.0,
        time_tolerance: Duration::from_millis(10),
    };
    let entities = associate(&observations, &EntityKind::Vehicle, EntityId::new(27), &cfg);

    let vehicle = entities
        .iter()
        .find(|e| e.members.len() > 1)
        .expect("vehicle group");
    assert_eq!(vehicle.entity, EntityId::new(27));
    assert_eq!(vehicle.members.len(), 4);
    assert_eq!(vehicle.modalities.len(), 4);
    assert_eq!(vehicle.sensors().len(), 4);
    // Combined confidence is higher than any single source.
    assert!(vehicle.confidence.get() > 0.95);
    // The distant observation is its own entity, not part of Vehicle #27.
    assert!(
        entities
            .iter()
            .any(|e| e.members == vec![ObservationId::new(5)])
    );
}

// ---- World state ----------------------------------------------------------------------------

#[test]
fn world_state_is_derived_from_observations() {
    let vehicle_obs = located(
        1,
        1,
        Modality::Lidar,
        CoordinateFrame::World,
        [1.0, 2.0, 0.0],
        mono(0),
        0.9,
    )
    .with_subject(EntityId::new(27));
    let temp = internal_observation(
        ObservationId::new(2),
        InternalMetric::BatteryLevel,
        0.6,
        conf(1.0),
        mono(0),
        Provenance::new(Origin::Real),
    );
    let air = Observation::new(
        ObservationId::new(3),
        Modality::Environmental,
        Property::new("humidity"),
        ObservationValue::Scalar(0.4),
        conf(0.8),
        mono(0),
        Provenance::new(Origin::Real),
    );
    let pose = Observation::new(
        ObservationId::new(4),
        Modality::Position,
        Property::new("robot_pose"),
        ObservationValue::Vector(vec![0.0, 0.0, 0.0]),
        conf(0.9),
        mono(0),
        Provenance::new(Origin::Real),
    )
    .with_location(SpatialReference::at(
        CoordinateFrame::World,
        [3.0, 4.0, 0.0],
    ));
    let meta = meta_observation(
        ObservationId::new(5),
        MetaSignal::Contradiction,
        0.5,
        conf(0.5),
        mono(0),
        Provenance::new(Origin::Unknown),
    );

    let observations = [vehicle_obs, temp, air, pose, meta];
    let cfg = AssociationConfig {
        max_distance: 2.0,
        time_tolerance: Duration::from_millis(1),
    };
    let assoc = associate(&observations, &EntityKind::Vehicle, EntityId::new(27), &cfg);
    let world = WorldState::derive(&observations, &assoc);

    assert_eq!(world.entities().len(), assoc.len());
    assert_eq!(world.body().len(), 1);
    assert_eq!(world.environment().len(), 1);
    assert_eq!(world.robot_pose().unwrap().position, Some([3.0, 4.0, 0.0]));
    // Meta-signals are for cognition and do not appear in the world state.
    assert!(
        world
            .body()
            .iter()
            .all(|o| o.property().as_str() != "meta.contradiction")
    );
}

// ---- Internal sensing -----------------------------------------------------------------------

#[test]
fn internal_metrics_use_the_same_observation_model() {
    let o = internal_observation(
        ObservationId::new(9),
        InternalMetric::MotorTemperature,
        71.5,
        conf(1.0),
        mono(0),
        Provenance::new(Origin::Real),
    );
    assert_eq!(o.modality(), &Modality::Thermal);
    assert_eq!(
        InternalMetric::from_property(o.property()),
        Some(InternalMetric::MotorTemperature)
    );
    // Internal observations start untrusted like any other observation.
    assert_eq!(o.trust().level, TrustLevel::Unknown);
}

// ---- Event detection ------------------------------------------------------------------------

#[test]
fn threshold_crossing_becomes_event_with_evidence() {
    let temp = Observation::new(
        ObservationId::new(50),
        Modality::Thermal,
        Property::new("temperature_c"),
        ObservationValue::Scalar(81.0),
        conf(0.9),
        mono(7),
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(8))),
    )
    .with_subject(EntityId::new(3));
    let ev = threshold_event(
        &temp,
        EventKind::TemperatureExceeded,
        75.0,
        SensorId::new(8),
    )
    .unwrap();
    assert_eq!(ev.kind, EventKind::TemperatureExceeded);
    assert_eq!(ev.entity, Some(EntityId::new(3)));
    assert_eq!(ev.confidence, conf(0.9));
    assert!(
        threshold_event(
            &temp,
            EventKind::TemperatureExceeded,
            90.0,
            SensorId::new(8)
        )
        .is_none()
    );
}

#[test]
fn movement_is_detected_between_located_observations() {
    let before = located(
        60,
        1,
        Modality::Position,
        CoordinateFrame::World,
        [0.0, 0.0, 0.0],
        mono(0),
        0.9,
    )
    .with_subject(EntityId::new(27));
    let after = located(
        61,
        1,
        Modality::Position,
        CoordinateFrame::World,
        [3.0, 4.0, 0.0],
        mono(1_000),
        0.5,
    )
    .with_subject(EntityId::new(27));
    let ev = movement_event(&before, &after, 1.0, SensorId::new(1)).unwrap();
    assert_eq!(ev.kind, EventKind::ObjectMoved);
    assert_eq!(
        ev.confidence,
        conf(0.5),
        "movement is only as reliable as its weaker end"
    );
    assert!(movement_event(&before, &after, 10.0, SensorId::new(1)).is_none());
}

#[test]
fn movement_is_not_inferred_across_frames() {
    let a = located(
        70,
        1,
        Modality::Position,
        CoordinateFrame::Camera,
        [0.0, 0.0, 0.0],
        mono(0),
        0.9,
    );
    let b = located(
        71,
        1,
        Modality::Position,
        CoordinateFrame::World,
        [9.0, 9.0, 9.0],
        mono(1),
        0.9,
    );
    assert!(movement_event(&a, &b, 0.1, SensorId::new(1)).is_none());
}

// ---- Uncertainty-aware pipeline stage -------------------------------------------------------

#[test]
fn negative_uncertainty_cannot_reach_the_pipeline() {
    // A negative standard deviation is refused when the value is built, so there is nothing
    // invalid left for a validation stage to reject.
    assert!(StandardDeviation::new(-1.0).is_none());
    let reading = range_reading(80, 1, 1.0, Some(-1.0), mono(0));
    assert!(
        reading
            .uncertainty()
            .and_then(|u| u.standard_deviation)
            .is_none()
    );
    let pipeline = Pipeline::new().with_stage(ValidationStage::new(conf(0.0)));
    assert!(pipeline.run(reading).is_ok());
}

// ---- Sensor noise (fault injection, seeded for reproducibility) -----------------------------

/// Deterministic pseudo-random numbers (xorshift64), so the fault test is reproducible.
struct Rng(u64);

impl Rng {
    fn next_unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Approximately normal noise with the given standard deviation (sum of 12 uniforms).
    fn gaussian(&mut self, sigma: f64) -> f64 {
        let s: f64 = (0..12).map(|_| self.next_unit()).sum();
        (s - 6.0) * sigma
    }
}

#[test]
fn fusing_noisy_sensors_beats_the_noisy_imprecise_one() {
    let truth = 25.0;
    let (sigma_precise, sigma_coarse) = (0.05, 0.8);
    let cfg = FusionConfig {
        time_tolerance: Duration::from_millis(1),
        disagreement_threshold: 100.0,
    };
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);

    let (mut err_coarse, mut err_fused) = (0.0, 0.0);
    let trials = 300;
    for trial in 0..trials {
        let ts = mono(trial as u64 * 1_000_000);
        let precise = range_reading(
            1_000 + trial as u64 * 2,
            1,
            truth + rng.gaussian(sigma_precise),
            Some(sigma_precise),
            ts,
        );
        let coarse = range_reading(
            1_001 + trial as u64 * 2,
            2,
            truth + rng.gaussian(sigma_coarse),
            Some(sigma_coarse),
            ts,
        );
        let fused = fuse(
            &[precise, coarse.clone()],
            ObservationId::new(trial as u64),
            &InverseVariance,
            &cfg,
        )
        .unwrap();
        let ObservationValue::Scalar(v) = fused.observation.value() else {
            panic!()
        };
        let ObservationValue::Scalar(c) = coarse.value() else {
            panic!()
        };
        err_coarse += (c - truth).abs();
        err_fused += (v - truth).abs();
        // Fusion never reports more uncertainty than the best input.
        let sd = fused
            .observation
            .uncertainty()
            .unwrap()
            .standard_deviation
            .unwrap()
            .get();
        assert!(sd <= sigma_precise + 1e-12);
    }
    assert!(
        err_fused < err_coarse,
        "fused error {err_fused} should beat coarse-only error {err_coarse}"
    );
}

// ---- Multimodal presence fusion (§38) -------------------------------------------------------

fn detection(
    id: u64,
    modality: Modality,
    sensor: u64,
    claim: bool,
    c: f64,
    ts: Timestamp,
) -> Observation {
    Observation::new(
        ObservationId::new(id),
        modality,
        Property::new("person_present"),
        ObservationValue::Boolean(claim),
        conf(c),
        ts,
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(sensor))),
    )
}

fn presence_cfg() -> FusionConfig {
    FusionConfig {
        time_tolerance: Duration::from_millis(5),
        disagreement_threshold: 0.5,
    }
}

#[test]
fn agreeing_modalities_raise_confidence_above_either_alone() {
    let visual = detection(1, Modality::Visual, 1, true, 0.7, mono(0));
    let thermal = detection(2, Modality::Thermal, 2, true, 0.6, mono(1_000_000));
    let fused = fuse_presence(
        &[visual.clone(), thermal.clone()],
        ObservationId::new(9),
        &presence_cfg(),
    )
    .unwrap();
    assert_eq!(fused.observation.value(), &ObservationValue::Boolean(true));
    assert!(fused.observation.confidence().get() > 0.7);
    assert!(fused.disagreement.is_none());
    assert_eq!(
        fused.observation.modality(),
        &Modality::Custom("multimodal".into())
    );
    assert_eq!(fused.observation.provenance().sensors().count(), 2);
}

#[test]
fn stronger_modality_wins_a_disagreement_and_it_is_reported() {
    // Visual says present with 0.9; thermal says absent with 0.6.
    let visual = detection(3, Modality::Visual, 1, true, 0.9, mono(0));
    let thermal = detection(4, Modality::Thermal, 2, false, 0.6, mono(0));
    let fused = fuse_presence(&[visual, thermal], ObservationId::new(10), &presence_cfg()).unwrap();
    assert_eq!(fused.observation.value(), &ObservationValue::Boolean(true));
    let d = fused.disagreement.expect("contradiction must be reported");
    assert!(d.spread > 0.5);
}

#[test]
fn presence_fusion_refuses_asynchronous_and_non_boolean_inputs() {
    let a = detection(5, Modality::Visual, 1, true, 0.8, mono(0));
    let late = detection(6, Modality::Thermal, 2, true, 0.8, mono(900_000_000));
    assert_eq!(
        fuse_presence(&[a.clone(), late], ObservationId::new(11), &presence_cfg()).unwrap_err(),
        FusionError::NotSimultaneous
    );
    let scalar = Observation::new(
        ObservationId::new(7),
        Modality::Thermal,
        Property::new("person_present"),
        ObservationValue::Scalar(1.0),
        conf(0.8),
        mono(0),
        Provenance::new(Origin::Real),
    );
    assert_eq!(
        fuse_presence(&[a, scalar], ObservationId::new(12), &presence_cfg()).unwrap_err(),
        FusionError::NotScalar
    );
}

#[test]
fn certain_contradictory_evidence_stays_finite() {
    // Extreme confidences must not produce NaN or infinite odds.
    let yes = detection(8, Modality::Visual, 1, true, 1.0, mono(0));
    let no = detection(9, Modality::Thermal, 2, false, 1.0, mono(0));
    let fused = fuse_presence(&[yes, no], ObservationId::new(13), &presence_cfg()).unwrap();
    assert!(fused.observation.confidence().get().is_finite());
}

// ---- Missing and faulted sensors (fault injection) -----------------------------------------

#[test]
fn faulted_sensors_are_skipped_when_selecting_a_source() {
    let caps = SensorCapabilities::new(vec![Modality::Thermal]);
    let candidates = [
        SensorCandidate {
            sensor: SensorId::new(1),
            capabilities: caps.clone(),
            health: SensorHealth::Faulted,
            noise: Some(0.01),
        },
        SensorCandidate {
            sensor: SensorId::new(2),
            capabilities: caps.clone(),
            health: SensorHealth::Offline,
            noise: Some(0.01),
        },
    ];
    assert_eq!(select_sensor(&candidates, &Modality::Thermal), None);
}

#[test]
fn fusion_still_works_when_one_of_two_sensors_is_missing() {
    let cfg = FusionConfig {
        time_tolerance: Duration::from_millis(1),
        disagreement_threshold: 1.0,
    };
    let surviving = range_reading(60, 1, 12.0, Some(0.2), mono(0));
    let fused = fuse(&[surviving], ObservationId::new(14), &InverseVariance, &cfg).unwrap();
    assert_eq!(
        fused.observation.provenance().sensors().collect::<Vec<_>>(),
        vec![SensorId::new(1)]
    );
    assert!(fused.disagreement.is_none());
}

#[test]
fn a_pipeline_with_no_calibration_stops_rather_than_passing_data_through() {
    let pipeline = Pipeline::new().with_stage(CalibrationStage::new(
        CalibrationState::new(),
        CalibrationKind::Intrinsic,
        mono(10),
    ));
    let obs = detection(10, Modality::Visual, 1, true, 0.9, mono(10));
    let err = pipeline.run(obs).unwrap_err();
    assert!(err.reason.contains("no usable"));
}

#[test]
fn world_state_is_empty_when_nothing_was_observed() {
    let world = WorldState::derive(&[], &[]);
    assert!(world.entities().is_empty());
    assert!(world.environment().is_empty());
    assert!(world.body().is_empty());
    assert!(world.robot_pose().is_none());
}
