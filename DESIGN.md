# tpt-senses — Design

This document records the design decisions behind the crate. The full specification is in
[`spec.txt`](spec.txt); section references (§n) point into it. [`todo.md`](todo.md) tracks what is
done.

## Layered model (§3, §6)

```
REALITY → SENSOR → measurement → PERCEPTION → observation → tpt-senses → world state → cognition
```

- **Measurement**: raw or minimally processed sensor output, with timestamp and provenance.
- **Observation**: a semantic interpretation of one or more measurements, with confidence.
- **Cognition** reasons about observations. It never needs to know the hardware.

## Module map

| Area | Module(s) | Spec |
|---|---|---|
| Identity | `id` | §50 |
| Time | `timestamp`, `temporal` | §17 |
| Modality | `modality` | §11, §52 |
| Values | `measurement` | §12 |
| Quality | `uncertainty`, `trust` | §15, §33 |
| Lineage | `provenance`, `calibration` | §16, §30, §32 |
| Observations | `observation` | §13, §14, §34 |
| Health | `health` | §31 |
| Capability | `capability`, `profile` | §5, §10, §24, §25 |
| Sensor | `sensor`, `adapter` | §9, §7, §54 |
| Streams | `stream`, `record` | §27, §28, §29 |
| Space | `spatial` | §18 |
| Events | `event` | §21 |
| Perception | `pipeline`, `fusion`, `entity`, `world` | §19, §20, §22, §26 |
| Internal sensing | `internal` | §23 |
| Control | `control` | §35–§37 |

## Core types

| Concern | Types | Notes |
|---|---|---|
| Identity | `SensorId`, `MeasurementId`, `ObservationId`, `EntityId` | `u64` newtypes; no UUID dependency |
| Time | `Timestamp`, `Duration`, `Frequency`, `ClockSource`, `TimingInfo` | Timestamps from different clocks are *incomparable* (`partial_cmp` → `None`) |
| Modality | `Modality` | Extensible; `Custom(..)` for modalities not yet defined (§11) |
| Values | `MeasurementValue`, `SharedBuffer` | Scalar, vector, matrix, image, audio, point cloud, spectrum, categorical, structured, binary |
| Quality | `Confidence`, `Precision`, `Accuracy`, `Bounds`, `Uncertainty` | Separate types; validated constructors (§15) |
| Trust | `TrustMetadata`, `SourceIdentity`, `TrustLevel` | Default is unverified (§33) |
| Lineage | `Provenance`, `ProvenanceStep`, `Algorithm`, `Origin`, `CalibrationRecord`, `CalibrationState` | Ordered chain from hardware to observation (§16, §32) |
| Health | `SensorHealth`, `Diagnostics`, `CalibrationStatus` | Sensors are monitored entities (§31) |
| Capability | `SensorCapabilities`, `Range`, `Resolution`, `VisionCapability`, `AuditoryCapability` | Descriptive, not biological (§10) |
| Behaviour | `Sensor` trait, `SensorError`, `Adapter` trait | `identity`, `capabilities`, `health`, `read` (§9) |

## Invariants

1. **Values are validated at construction.** `Confidence` is in `[0, 1]`; `Bounds` is ordered;
   matrices, images and spectra check their buffer lengths. Constructors return `Option`.
2. **Observations are evidence, not fact.** New observations start in `ObservationState::Raw` and
   `TrustMetadata::unverified()`. The lifecycle (§34) only moves forward, and stages may be
   skipped. Nothing in the crate marks an observation as true.
3. **Asynchronous data is never silently simultaneous.** Cross-clock timestamp comparison is
   undefined (`None`). `simultaneous`, `fuse`, `associate`, `ReorderBuffer`, `Recording` and
   `RateLimiter` all refuse mixed clocks or require an explicit tolerance.
4. **Provenance is never erased.** Fusion concatenates every input's chain and appends the
   strategy. Association does the same for member observations. Pipeline stages append
   themselves. Calibrations are recorded as provenance steps.
5. **Large payloads are shared, not copied.** Buffers are `Arc<[T]>` (`SharedBuffer`), so cloning
   a measurement is cheap.
6. **Disagreement is reported.** `fuse` returns a `Disagreement` with the outlier sensors when
   inputs spread beyond a threshold. Outliers are measured against the **median**, so a single
   faulty sensor cannot pull the reference towards itself.
7. **Failed or expired calibration blocks use.** `CalibrationState::usable` returns `None` when
   the most recent calibration of a kind has failed or expired. `CalibrationStage` rejects the
   observation in that case.

## Design choices

- **Streams are runtime-agnostic.** `SensorStream` implements `futures_core::Stream` and needs no
  executor. Overflow follows an explicit `BackpressurePolicy` and is counted, never silent.
  `BoundedQueue` is the synchronous equivalent for polling loops and `no_std` targets.
- **`alloc` is always required.** `Vec`, `String` and `Arc` appear in the core types. The
  `std` feature only adds `std::error::Error` impls. An alloc-free subset is a later goal.
- **No `libm`.** Floating-point helpers that `no_std` lacks are implemented in the crate (`math`).
- **Fusion is pluggable, provenance-preserving, and explicit about its assumptions.**
  `ConfidenceWeighted` and `InverseVariance` are built in. Combined confidence uses noisy-OR,
  which assumes independent evidence; that assumption is recorded in the fused provenance.
- **Pipelines explain themselves.** Each `StageRecord` gives the algorithm, input and output
  confidence and state. A rejection returns the stage name, the reason, and the trace so far.
- **World state is derived, not stored.** `WorldState::derive` is the only constructor and its
  fields are read-only. Meta-signals are excluded: they belong to cognition.

## Adapters and packaging (§7, §43, §54)

- The core defines the `Adapter` trait (external input → observations). It has no dependency on
  any TPT repository or hardware ecosystem.
- **Packaging decision:** adapters that depend on other TPT repositories (`tpt-cv`, `tpt-audio`,
  `tpt-voice`, `tpt-teleop-agri`) will be separate crates. Feature flags were not chosen, so that
  the core's dependency list stays small and each adapter's licence can be audited on its own. No
  adapter crate exists yet, because each needs the external repository's API.
- Dependencies point upward: adapters depend on the core, never the reverse.

## Safety boundary (§49, §35)

Safety-critical decisions stay outside the sensory layer. `tpt-senses` reports confidence,
uncertainty, provenance, disagreement and calibration status. It does not decide whether an
action is safe. Attention hints and information requests change *how* the system senses, never
*what counts as true*.

## Deliberately deferred

- Serialization (`serde`, JSON, CBOR) and an on-disk record format in a separate crate (§29, §39).
  CBOR crates need a licence check against `deny.toml` before they are added.
- Simulation sources, ROS 2 integration, and all `(EXT)` adapters (Phases 4–6).
- Benchmarks, an alloc-free subset, binary-size checks, and profile-specific fault tests (Phase 7).
- Meta-signal interpretation and anything that owns cognitive state (belongs to `tpt-thought`).
