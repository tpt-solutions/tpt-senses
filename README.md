# tpt-senses

A general-purpose sensory and perception foundation for TPT's cognitive, robotic, autonomous and
intelligent systems.

> **Sensors produce measurements. Perception produces observations. Cognition reasons about
> observations.**

`tpt-senses` gives every sensor — camera, LiDAR, microphone, radiation detector, ROS topic,
simulator, or internal system monitor — one common representation of *what was observed, where,
when, by what, with what confidence, and under what conditions*.

## Status

Early development (`0.1.0`, not yet published). The core data model, streams, temporal alignment,
calibration and trust metadata, the perception pipeline, fusion, entity association, world state,
events, and the adapter / record-replay contracts are implemented and tested. Serialization and
external (`tpt-*`) integrations are planned. See [`todo.md`](todo.md) for the phased
plan and [`spec.txt`](spec.txt) for the design specification.

## Scope

- Measurements, observations, uncertainty, provenance, timestamps and clock sources
- Extensible modality model, including custom modalities
- Sensor capabilities and health diagnostics
- The `Sensor` trait every sensor implements
- `no_std` + `alloc` core, with an optional `std` feature

## Non-goals

`tpt-senses` is **not**:

- a computer vision framework
- an audio codec library
- a speech recognition framework
- a ROS replacement or a robotics operating system
- an AI model, an LLM interface, or a cognitive reasoning engine
- a hardware-driver collection
- a five-senses simulation or a human sensory emulator
- a world model

It is the sensory/perceptual interface between measurements and cognition. Hardware drivers,
ROS, and computer-vision algorithms sit *below* it and are integrated through adapters.

## Usage

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

## Building and testing

```sh
cargo test --workspace
cargo test --workspace --no-default-features   # no_std build
```

Minimum supported Rust version: **1.85** (edition 2024).

## Licence

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT license](LICENSE-MIT)

at your option.

## Contributing

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this work by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.

See [CONTRIBUTING.md](CONTRIBUTING.md).
