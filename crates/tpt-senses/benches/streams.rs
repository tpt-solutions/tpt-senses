#![allow(missing_docs, reason = "criterion macros generate undocumented items")]
#![allow(
    clippy::cast_precision_loss,
    reason = "benchmark setup values are small"
)]
//! Core-path benchmarks: bounded buffering, reordering, fusion and the pipeline.
//! Run with `cargo bench -p tpt-senses`.

use std::hint::black_box;
use std::num::NonZeroUsize;

use criterion::{Criterion, criterion_group, criterion_main};
use tpt_senses::prelude::*;
use tpt_senses::{
    BackpressurePolicy, BoundedQueue, CalibrationKind, CalibrationRecord, CalibrationStage,
    CalibrationState, Duration, FusionConfig, InverseVariance, Measurement, MeasurementValue,
    Pipeline, ReorderBuffer, Sample, SharedBuffer, StandardDeviation, Uncertainty, ValidationStage,
    fuse,
};

fn mono(ns: u64) -> Timestamp {
    Timestamp::new(ns, ClockSource::Monotonic)
}

fn bounded_queue(c: &mut Criterion) {
    c.bench_function("bounded_queue/push_pop_1024", |b| {
        let mut q = BoundedQueue::new(
            NonZeroUsize::new(1024).unwrap(),
            BackpressurePolicy::DropOldest,
        );
        let mut i = 0u64;
        b.iter(|| {
            q.push(black_box(i));
            i = i.wrapping_add(1);
            black_box(q.try_pop());
        });
    });
}

fn reorder_buffer(c: &mut Criterion) {
    c.bench_function("reorder_buffer/push_pop_ready_jitter", |b| {
        let mut rb = ReorderBuffer::new(Duration::from_nanos(64), NonZeroUsize::new(256).unwrap());
        let mut i = 0u64;
        b.iter(|| {
            let jitter = (i * 2_654_435_761) % 32;
            let _ = rb.push(Sample::new(mono(i * 10 + 64 - jitter), i));
            black_box(rb.pop_ready());
            i = i.wrapping_add(1);
        });
    });
}

fn fusion(c: &mut Criterion) {
    let make = |id: u64, value: f64| {
        Observation::new(
            ObservationId::new(id),
            Modality::Depth,
            Property::new("range_m"),
            ObservationValue::Scalar(value),
            Confidence::new(0.9).unwrap(),
            mono(0),
            Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(id))),
        )
        .with_uncertainty(
            Uncertainty::new()
                .with_standard_deviation(StandardDeviation::new(0.1 * id as f64).unwrap()),
        )
    };
    let inputs = [make(1, 10.0), make(2, 10.2), make(3, 9.9)];
    let config = FusionConfig::new(Duration::from_millis(1), 1.0);
    c.bench_function("fusion/inverse_variance_three_inputs", |b| {
        b.iter(|| {
            black_box(fuse(
                black_box(&inputs),
                ObservationId::new(9),
                &InverseVariance,
                &config,
            ))
        });
    });
}

fn pipeline(c: &mut Criterion) {
    let mut cal = CalibrationState::new();
    cal.record(CalibrationRecord::new(
        CalibrationKind::Temperature,
        mono(0),
    ));
    let pipeline = Pipeline::new()
        .with_stage(ValidationStage::new(Confidence::new(0.5).unwrap()))
        .with_stage(CalibrationStage::new(
            cal,
            CalibrationKind::Temperature,
            mono(10),
        ));
    let observation = Observation::new(
        ObservationId::new(1),
        Modality::Thermal,
        Property::new("temperature_c"),
        ObservationValue::Scalar(37.0),
        Confidence::new(0.9).unwrap(),
        mono(10),
        Provenance::new(Origin::Real),
    );
    c.bench_function("pipeline/validate_then_calibrate", |b| {
        b.iter(|| black_box(pipeline.run(black_box(observation.clone()))));
    });
}

fn zero_copy(c: &mut Criterion) {
    // A 1 MiB frame. Cloning a measurement shares the buffer, so the cost should not depend on size.
    let frame: SharedBuffer<u8> = std::sync::Arc::from(vec![7u8; 1 << 20]);
    let value = MeasurementValue::image(1024, 1024, 1, frame).unwrap();
    let measurement = Measurement::new(
        SensorId::new(1),
        Modality::Visual,
        value,
        mono(0),
        Provenance::new(Origin::Real),
    );
    c.bench_function("measurement/clone_1mib_frame_shared", |b| {
        b.iter(|| black_box(measurement.clone()));
    });
}

criterion_group!(
    benches,
    bounded_queue,
    reorder_buffer,
    fusion,
    pipeline,
    zero_copy
);
criterion_main!(benches);
