# TPT-Senses — Project Todo

**Org:** TPT Solutions · **Language:** Rust (edition 2024, stable) · **License:** MIT OR Apache-2.0 · **Spec:** `spec.txt`

Legend: `[ ]` todo · `[x]` done · **(EXT)** = blocked on another TPT repo / external system · `§n` = spec section

## Key decisions
- [x] Dual licensed: MIT OR Apache-2.0
- [x] No Apache-only dependencies (allow MIT, MIT/Apache dual, BSD, ISC, Zlib, Unlicense)
- [x] Cargo workspace from the start; `tpt-senses` is the first member
- [x] Core is `no_std` + `alloc`, with a `std` feature
- [x] Streams are runtime-agnostic (`futures-core`); no tokio in core
- [x] Optional `serde` feature; JSON + CBOR adapters tested
- [x] Own minimal Timestamp / Duration / Frequency / Id types; units and linear-algebra interop behind optional features
- [x] CI: GitHub Actions; stable Rust; edition 2024; MSRV defined and tested

---

## Phase 0 — Project setup & licensing
- [ ] Create git repo and `.gitignore`
- [ ] Root workspace `Cargo.toml` (resolver 3, shared `[workspace.package]` metadata, shared lints)
- [ ] `tpt-senses` member crate skeleton (`crates/tpt-senses` or root member — decide layout)
- [ ] `license = "MIT OR Apache-2.0"` in all Cargo manifests
- [ ] `LICENSE-MIT` (copyright TPT Solutions)
- [ ] `LICENSE-APACHE` (Apache-2.0 text)
- [ ] README licence section + standard contribution clause (contributions dual-licensed)
- [ ] Update spec/README header: licence changed from MIT to MIT OR Apache-2.0
- [ ] `README.md` (purpose, principle: measurement → observation → cognition, scope, non-goals §55)
- [ ] `DESIGN.md` (derived from spec)
- [ ] `CHANGELOG.md` (Keep a Changelog format)
- [ ] `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`
- [ ] Define crate features: `std`, `alloc`, `serde`, `json`, `cbor`, `units`, `glam`/`nalgebra` interop (as needed)
- [ ] Choose and set MSRV; add MSRV CI job
- [ ] `rustfmt.toml`, clippy config, `#![forbid(unsafe_code)]` policy (or documented unsafe exceptions)
- [ ] `cargo-deny` config: allowed licences list, ban Apache-2.0-only / GPL / LGPL / MPL / unknown, advisories, duplicate checks
- [ ] GitHub Actions CI: fmt, clippy, test (std + alloc + all features), docs build, cargo-deny
- [ ] CI: `no_std` build on a bare-metal target (e.g. `thumbv7em-none-eabihf`)
- [ ] CI: WASM build check (`wasm32-unknown-unknown`) §41
- [ ] CI: cargo-semver-checks / publish dry-run
- [ ] Dependency audit: confirm every dependency (`futures-core`, `serde`, CBOR/JSON crates, etc.) passes the licence policy
- [ ] Reserve crate names on crates.io (`tpt-senses` + planned sub-crates)
- [ ] Create `tests/` directory and test layout

## Phase 1 — Core data model (spec §58 Phase 1)
Modules: `lib`, `sensor`, `modality`, `measurement`, `observation`, `capability`, `uncertainty`, `provenance`, `timestamp`, `health` §42
- [ ] `lib.rs`: `#![no_std]`, feature gating, prelude, crate docs
- [ ] Identifiers: `SensorId`, `ObservationId`, `EntityId`, etc. — efficient, `no_std`-safe, no mandatory UUID dependency §50
- [ ] `Timestamp` / `Duration` / `Frequency` own types, with clock-source tag §17
- [ ] `Modality` enum with all listed variants + `Custom(...)`; ensure it is not an architectural bottleneck §11, §52
- [ ] Capability types: `Range`, `Resolution`, `SpectralRange`, etc. §10
- [ ] `SensorCapabilities` (descriptive, not biologically biased) §10
- [ ] Modality-specific capability types (e.g. `VisionCapability`, `AuditoryCapability`) §5
- [ ] `MeasurementValue`: scalar, vector, matrix, image, audio buffer, point cloud, spectrum, categorical, structured, custom binary §12
- [ ] Zero-copy / borrowed / shared-buffer representation for large data §12, §50
- [ ] `Measurement` struct §12
- [ ] `Uncertainty` (confidence, std-dev, bounds, distribution) §15
- [ ] Distinct types for value / confidence / precision / accuracy / uncertainty §15
- [ ] `Confidence` newtype (validated range)
- [ ] `Provenance` (source chain: hardware → sensor → measurement → algorithm → observation; algorithm/version/parameters) §16, §47
- [ ] Provenance records real vs simulated vs recorded vs synthetic origin §30
- [ ] `ObservationValue`, `Property`, `Observation` §13
- [ ] Observation lifecycle states (RAW … ARCHIVED) §34
- [ ] `SensorHealth` states (Online/Offline/Degraded/Calibrating/Faulted/Unknown) + diagnostics (signal quality, noise, latency, drop rate, temperature, calibration status) §31
- [ ] `SensorError` type
- [ ] `Sensor` trait (identity, capabilities, read, + timestamp/calibration/health/provenance/uncertainty exposure) §9
- [ ] Measurement → Observation conversion path (simple sensor, no pipeline) §26
- [ ] Unit tests: modality, timestamps, uncertainty, provenance, observation construction §48
- [ ] Doc-tests and examples (e.g. radiation sensor with no human equivalent §44)
- [ ] Verify build with `--no-default-features` (no_std, no alloc where practical) §51

## Phase 2 — Streams, events, spatial, temporal, calibration (spec §58 Phase 2)
Modules: `stream`, `event`, `spatial`, `temporal`, `calibration`
- [ ] `SensorStream<T>` on `futures-core::Stream`, runtime-agnostic §27
- [ ] Bounded buffering + configurable backpressure policy §27, §50
- [ ] Cancellation support
- [ ] Ordering guarantees + out-of-order handling
- [ ] Dropped-sample reporting
- [ ] Rate limiting and filtering combinators
- [ ] Sync (non-async) polling alternative for embedded / `no_std`
- [ ] Event model: `ObjectAppeared`, `ObjectDisappeared`, `ObjectMoved`, `CollisionDetected`, `TemperatureExceeded`, `SoundDetected`, `PressureChanged`, `RadiationSpike`, `SensorFailure`, `CalibrationChanged`, `EnvironmentChanged`, + custom §21
- [ ] Event fields: timestamp, source, entity, modality, measurements, confidence, provenance §21
- [ ] Spatial reference / coordinate-frame types (world, robot, body, camera, pixel) §18
- [ ] Transform trait/interface so external systems (ROS TF) can plug in without a hard dependency §18
- [ ] Temporal model: sensor / acquisition / processing / observation timestamps, latency, ordering, clock source §17
- [ ] Temporal alignment (interpolation / nearest-sample / windowing) across mixed-rate sensors §17
- [ ] Never silently treat asynchronous measurements as simultaneous — enforce in API and test §17
- [ ] Calibration metadata: intrinsic, extrinsic, temperature, zero-point, time, alignment §32
- [ ] Calibration state included in provenance §32
- [ ] Security & trust metadata: source identity, authentication status, integrity status, trust level §33
- [ ] Ensure an observation is never automatically treated as a fact §33, §49
- [ ] Fault tests: stale data, bad timestamps, out-of-order messages, sensor noise, calibration failure §48
- [ ] Tests for streams (backpressure, drop reporting, cancellation)

## Phase 3 — Fusion, entity association, world state (spec §58 Phase 3)
Modules: `fusion`, `entity`, state/world-state module, perception pipeline
- [ ] Perception pipeline abstraction with optional stages: validation → calibration → normalisation → feature extraction → classification → association → fusion → observation → world state §26
- [ ] Pipeline stages expose input / algorithm+version / parameters / output / uncertainty / provenance §47
- [ ] Fusion API combining multiple measurements/observations into one observation §19
- [ ] Fusion preserves underlying evidence and never erases provenance §19
- [ ] Baseline fusion strategies (e.g. confidence-weighted, Bayesian-style); pluggable custom strategies
- [ ] Sensor disagreement handling and reporting §48
- [ ] Entity model (`EntityId`, entity types) §20
- [ ] Entity association (many sensors → one entity) with confidence + provenance §20
- [ ] Multimodal evidence grouping (visual + auditory + LiDAR + thermal → entity) §38
- [ ] World-state representation (robot, nearby entities, environment, robot body) §22
- [ ] State derived from observations, never an independent source of truth §22
- [ ] Internal sensing: battery, CPU/GPU, memory pressure, temperature, motor temperature, joint position/torque, network quality, sensor/actuator health §23
- [ ] Interface for higher-level internal observations (prediction error, uncertainty, contradiction, attention demand, goal conflict) without owning cognitive state §23
- [ ] Event detection from observation changes §21
- [ ] Fusion tests (sensor A + sensor B → fused observation) §48
- [ ] Integration tests (sensor → measurement → observation) §48
- [ ] Example: camera + LiDAR + radar + GPS → "Vehicle #27" §19

## Phase 4 — Adapters for existing TPT projects (spec §58 Phase 4)
Consume capabilities from existing repos rather than duplicating them §7. All items depend on those repos' APIs.
- [ ] Define the generic adapter trait(s) in core (measurement/feature in → observation out), no dependency on other TPT repos
- [ ] Decide adapter packaging (feature flags vs separate crates, e.g. `tpt-senses-vision`, `tpt-senses-audio`) §43
- [ ] **(EXT)** `tpt-cv` adapter: visual features/detections → visual observations
- [ ] **(EXT)** `tpt-audio` adapter: audio data/features → auditory observations
- [ ] **(EXT)** `tpt-voice` adapter: speech recognition as an optional auditory component; hearing is not defined by it; non-speech sounds represented §7
- [ ] **(EXT)** `tpt-teleop-agri` adapter: common representation for robot datafeeds §7
- [ ] Verify the licences of every TPT dependency are compatible with MIT OR Apache-2.0 and the dependency policy
- [ ] Verify dependency direction: adapters point upward; core never depends on hardware ecosystems §54
- [ ] Adapter integration tests and examples (camera, microphone flows §44)

## Phase 5 — External integrations (spec §58 Phase 5)
- [ ] **ROS 2 integration:** `tpt-senses-ros2` crate (separate; core stays ROS-free) §8, §41
  - [ ] **(EXT)** Topic adapters: camera, LiDAR, IMU, GPS, force, custom
  - [ ] **(EXT)** ROS TF → spatial reference bridge §18
  - [ ] Decide whether `tpt-senses-ros` (ROS 1) is justified §8
  - [ ] Check the ROS 2 Rust client licence (e.g. rclrs — Apache-2.0 only?) against the dependency policy; resolve conflicts or isolate/skip
- [ ] **Simulation:** `tpt-senses-sim` — virtual sensors using the same interface; provenance exposes simulated origin §30
- [ ] **Record/replay:** `tpt-senses-record` — record, compress, index, replay, seek, timestamp, filter §29
  - [ ] Storage-format-agnostic trait in core; concrete format only in the record crate §29
  - [ ] Choose compression crate(s) that meet licence policy
- [ ] Deterministic replay: same recorded stream → same pipeline → same observations §28
- [ ] Replay tests (determinism where the pipeline is designed to be) §48
- [ ] Serialization adapters: `serde` feature, JSON, CBOR; keep Rust-native types canonical §39
- [ ] Serialization round-trip tests (JSON, CBOR)
- [ ] Defer / log decision: MessagePack, Arrow-compatible structures, custom binary §39
- [ ] Optional ML hook crate `tpt-senses-ml` only if justified; core stays ML/LLM/cloud-free §40, §43
- [ ] Review: only create each crate when implementation complexity justifies it §8, §43

## Phase 6 — Advanced perception (spec §58 Phase 6)
- [ ] Controllable sensor parameters model: zoom, focus, exposure, sampling frequency, direction, resolution, spectral band, gain, range §37
- [ ] Parameter-change request/response types (actual control stays in the device layer) §37
- [ ] Active-perception loop types: uncertainty → request additional information → sensor action → new observation §36
- [ ] Example: weed vs not-weed with a multispectral follow-up §36
- [ ] Attention-aware sensing interface (attention changes sensing rates/resolution) §35
- [ ] **(EXT)** `tpt-anima` interface: observations/events/state out; attention hints in §35
- [ ] **(EXT)** `tpt-thought` consumption interface; confirm no hardware knowledge needed §46, §59
- [ ] **(EXT)** `tpt-eve` verification hooks: inspectable transformations, "why do we believe this?" §16, §47
- [ ] Adaptive sensor selection (choose sensors by capability, health, uncertainty)
- [ ] Advanced multimodal fusion across modalities §38
- [ ] Sensory history export for reflection/sleep processing, without owning memory §46
- [ ] Confirm safety-critical decision-making stays outside the sensory layer §49

## Phase 7 — Profiles, hardening, docs & release
- [ ] `profile` module: `SensoryProfile` (Human, Robot, Drone, AgriculturalRobot, IndustrialRobot, Spacecraft, Custom) §24
- [ ] Profiles defined as capabilities, with a "conceptual not biological" disclaimer §5, §25
- [ ] Biological-inspiration reference profiles documented as engineering inspiration only §25
- [ ] Custom modality extensibility tests (e.g. gravitational, novel chemical signature) §52
- [ ] Full test suite: unit, integration, fusion, replay, fault §48
- [ ] Fault tests: missing sensors, sensor disagreement, etc. §48
- [ ] Benchmarks: high-frequency streams, allocation counts, zero-copy large data (video/audio/point clouds/hyperspectral) §50
- [ ] Embedded checks: `no_std` on target, binary size, alloc-free subset §51
- [ ] WASM build verification §41
- [ ] Rustdoc complete with examples; docs.rs config
- [ ] DESIGN.md finalised; relationship diagrams §6, §54
- [ ] Integration example for each of the spec §44 flows (camera, microphone, agri robot, radiation)
- [ ] Final dependency/licence audit (`cargo deny check`) 
- [ ] Security review: untrusted sensor data, trust metadata §33
- [ ] Semver / API review before 0.1.0
- [ ] Publish `tpt-senses` 0.1.0 to crates.io
- [ ] Tag release and write CHANGELOG entry

---

## Success criteria checklist (spec §59)
- [ ] 1. A new sensor can be integrated without modifying `tpt-thought`
- [ ] 2. A new modality can be introduced without redesigning the core
- [ ] 3. ROS can provide measurements without becoming a core dependency
- [ ] 4. Existing TPT media repos contribute perception without being absorbed into `tpt-senses`
- [ ] 5. Observations retain uncertainty and provenance
- [ ] 6. Multiple sensors can be fused into a common observation
- [ ] 7. Real and simulated sensors use the same cognitive interface
- [ ] 8. Internal system sensors use the same observation model
- [ ] 9. Sensory capabilities beyond human perception are supported
- [ ] 10. `tpt-thought` can reason over observations without knowing the hardware

## Open items / to confirm later
- [ ] Final workspace directory layout (root crate vs `crates/` folder)
- [ ] MSRV value
- [ ] Which `tpt-*` repos are public/available and their licences
- [ ] Whether the ROS 2 Rust bindings' licence passes the dependency policy
