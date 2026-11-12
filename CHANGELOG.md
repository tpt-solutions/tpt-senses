# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Cargo workspace with `tpt-senses` as the first member; `resolver = "3"`, edition 2024,
  MSRV 1.85, dual-licensed MIT OR Apache-2.0.
- Core data model: identifiers, `Timestamp` / `Duration` / `Frequency` with clock sources,
  `Modality` (with `Custom`), `MeasurementValue`, `Measurement`, `Confidence`, `Uncertainty`,
  `Provenance`, `Observation` with lifecycle states, `SensorHealth` and `Diagnostics`,
  `SensorCapabilities`, `SensorError` and the `Sensor` trait.
- `std` feature (default) for `std::error::Error` integration; `no_std` + `alloc` otherwise.
- Unit and integration tests for the core model.
- Phase 2: `SensorStream` (futures-core), `BoundedQueue` with backpressure and drop counts,
  `ReorderBuffer`, `RateLimiter`, cancellation; events; spatial references and `Transform`;
  `TimingInfo`, alignment (nearest, linear, window), freshness and simultaneity checks;
  calibration records and state (recorded in provenance); trust metadata on observations.
- Phase 3: perception `Pipeline` with `Stage`, `ValidationStage`, `CalibrationStage`, `FnStage`
  and per-stage traces; `fuse` with `ConfidenceWeighted` and `InverseVariance` and a pluggable
  `FusionStrategy`; disagreement reporting; entity `associate`; `WorldState::derive`; internal
  sensing (`InternalMetric`, `MetaSignal`); threshold and movement event detectors.
- Phase 4–6: `Adapter` trait; storage-agnostic `RecordSink`, in-memory `Recording` with seek and
  deterministic `replay_through`; parameter change types, information requests, attention-based
  sampling and sensor selection; `SensoryProfile`.
- Integration tests for the spec §44 flows (camera, microphone, agricultural robot, radiation).
- Optional `serde` feature: serialization of the observation model, with validated deserialization
  for types that carry invariants.
- `Sensor::calibration()` and `Sensor::last_reading_at()` with defaults.
- `observations_between()`: a read-only, clock-aware time window over observations.
- Seeded sensor-noise fault test.

### Changed
- `ReorderBuffer::new` takes a capacity. Samples arriving while it is full are discarded and
  counted (`Ingest::Overflowed`, `overflow_dropped()`). This bounds memory under a flood.
