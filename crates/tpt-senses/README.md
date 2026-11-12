# tpt-senses

A general-purpose sensory and perception foundation for TPT's cognitive, robotic and autonomous
systems.

> **Sensors produce measurements. Perception produces observations. Cognition reasons about
> observations.**

`tpt-senses` gives every sensor (camera, LiDAR, microphone, radiation detector, ROS topic,
simulator, or internal system monitor) one common representation of what was observed, where,
when, by what, with what confidence, and under what conditions.

## What it provides

- **Measurements and observations**: `Measurement` / `MeasurementValue` for raw output, and
  `Observation` for a semantically meaningful interpretation of it.
- **Uncertainty and confidence**: explicit, non-interchangeable types for confidence, standard
  deviation and bounds.
- **Provenance**: the traceable chain from hardware to observation, including whether the data is
  real, simulated, recorded or synthetic.
- **Time**: `Timestamp`, `Duration` and `Frequency` with an explicit clock source, plus temporal
  alignment and simultaneity checks, so asynchronous data is never silently treated as simultaneous.
- **Sensors**: the `Sensor` trait, `SensorCapabilities` and `SensorHealth` with diagnostics.
- **Streams**: `SensorStream` on `futures-core`, runtime-agnostic, with bounded buffering,
  backpressure, drop reporting and cancellation.
- **Perception**: a pluggable `Pipeline` of stages, multi-sensor `fuse` with pluggable strategies,
  entity association, world state, and threshold and movement event detectors.
- **Recording and replay**: a storage-agnostic `RecordSink`, an in-memory `Recording`, and
  deterministic `replay_through`.
- **Adapters**: the `Adapter` trait for connecting external sources. TPT-specific adapters live in
  separate crates.
- **Profiles**: `SensoryProfile` describes sensing capabilities for human, robot, drone and other
  platforms. These are engineering descriptions, not biological models.

## Features

| Feature | Default | Purpose                                                                 |
|---------|---------|-------------------------------------------------------------------------|
| `std`   | yes     | `std::error::Error` integration.                                        |
| `serde` | no      | Serialization of the observation model, with validated deserialization. |
| `cbor`  | no      | CBOR encoding through `cbor4ii`. Implies `serde`.                       |

The core is `no_std` and requires `alloc`. Build without default features for a `no_std` target:

```sh
cargo build --no-default-features
```

## Example

```rust
use tpt_senses::prelude::*;

let sensor = SensorId::new(1);
let provenance = Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(sensor));
let measurement = Measurement::new(
    sensor,
    Modality::Thermal,
    MeasurementValue::Scalar(37.4),
    Timestamp::new(1_000_000, ClockSource::Monotonic),
    provenance,
);

let observation = Observation::from_measurement(
    &measurement,
    ObservationId::new(1),
    Property::new("temperature_state"),
    ObservationValue::Categorical("normal".into()),
    Confidence::new(0.95).expect("confidence in [0, 1]"),
);
```

## Non-goals

`tpt-senses` is not a computer vision framework, an audio codec, a speech recognition framework, a
ROS replacement, an AI model or reasoning engine, a hardware-driver collection, or a world model.
It is the interface between measurements and cognition. Drivers, ROS and computer-vision
algorithms sit below it and connect through adapters.

## Minimum supported Rust version

1.85 (edition 2024).

## Licence

Licensed under either of Apache License, Version 2.0 or MIT license, at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this crate by you shall be dual licensed as above, without any additional terms or conditions.
