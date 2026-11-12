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
- [x] Create git repo and `.gitignore`
- [x] Root workspace `Cargo.toml` (resolver 3, shared `[workspace.package]` metadata, shared lints)
- [x] `tpt-senses` member crate skeleton (`crates/tpt-senses` or root member — decide layout) — layout: `crates/tpt-senses`
- [x] `license = "MIT OR Apache-2.0"` in all Cargo manifests (crate inherits from workspace)
- [x] `LICENSE-MIT` (copyright TPT Solutions)
- [x] `LICENSE-APACHE` (Apache-2.0 text)
- [x] README licence section + standard contribution clause (contributions dual-licensed)
- [x] Update spec/README header: licence changed from MIT to MIT OR Apache-2.0
- [x] `README.md` (purpose, principle: measurement → observation → cognition, scope, non-goals §55)
- [x] `DESIGN.md` (derived from spec; initial version, finalise in Phase 7)
- [x] `CHANGELOG.md` (Keep a Changelog format)
- [x] `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`
- [x] Define crate features: `std`, `serde`, `cbor` done; `alloc` is always required (alloc-free subset in Phase 7); `units`, `glam`/`nalgebra` to be added with their phases
- [x] Choose and set MSRV (1.85); MSRV CI job added, not yet run on GitHub
- [x] `rustfmt.toml`, clippy config, `#![forbid(unsafe_code)]` policy (or documented unsafe exceptions), `unsafe_code = forbid` via workspace lint
- [x] `cargo-deny` config: allowed licences list, ban Apache-2.0-only / GPL / LGPL / MPL / unknown, advisories, duplicate checks (`deny.toml`; passes locally)
- [x] GitHub Actions CI: fmt, clippy, test (default / no-default / all features), docs build, MSRV, cargo-deny (workflow written; not yet run on GitHub)
- [x] CI: `no_std` build on a bare-metal target (e.g. `thumbv7em-none-eabihf`) (verified locally)
- [x] CI: WASM build check (`wasm32-unknown-unknown`) §41 (verified locally)
- [ ] CI: cargo-semver-checks / publish dry-run
- [x] Dependency audit: `futures-core`, `serde` and dev-only `serde_json` pass the licence policy (`cargo deny check`)
- [ ] Reserve crate names on crates.io (`tpt-senses` + planned sub-crates)
- [x] Create `tests/` directory and test layout (`crates/tpt-senses/tests/core_model.rs`)

## Phase 1 — Core data model (spec §58 Phase 1)
Modules: `lib`, `sensor`, `modality`, `measurement`, `observation`, `capability`, `uncertainty`, `provenance`, `timestamp`, `health` §42
- [x] `lib.rs`: `#![no_std]`, feature gating, prelude, crate docs
- [x] Identifiers: `SensorId`, `ObservationId`, `EntityId`, etc. — efficient, `no_std`-safe, no mandatory UUID dependency §50
- [x] `Timestamp` / `Duration` / `Frequency` own types, with clock-source tag §17
- [x] `Modality` enum with all listed variants + `Custom(...)`; ensure it is not an architectural bottleneck §11, §52
- [x] Capability types: `Range`, `Resolution`, `SpectralRange`, etc. §10
- [x] `SensorCapabilities` (descriptive, not biologically biased) §10
- [x] Modality-specific capability types (e.g. `VisionCapability`, `AuditoryCapability`) §5
- [x] `MeasurementValue`: scalar, vector, matrix, image, audio buffer, point cloud, spectrum, categorical, structured, custom binary §12
- [x] Zero-copy / borrowed / shared-buffer representation for large data §12, §50
- [x] `Measurement` struct §12
- [x] `Uncertainty` (confidence, std-dev, bounds, distribution) §15
- [x] Distinct types for value / confidence / precision / accuracy / uncertainty §15
- [x] `Confidence` newtype (validated range)
- [x] `Provenance` (source chain: hardware → sensor → measurement → algorithm → observation; algorithm/version/parameters) §16, §47
- [x] Provenance records real vs simulated vs recorded vs synthetic origin §30
- [x] `ObservationValue`, `Property`, `Observation` §13
- [x] Observation lifecycle states (RAW … ARCHIVED) §34
- [x] `SensorHealth` states (Online/Offline/Degraded/Calibrating/Faulted/Unknown) + diagnostics (signal quality, noise, latency, drop rate, temperature, calibration status) §31
- [x] `SensorError` type
- [x] `Sensor` trait (identity, capabilities, health, read, calibration, last_reading_at) §9 — provenance and uncertainty are per reading, carried on `Measurement`, not exposed by the sensor
- [x] Measurement → Observation conversion path (simple sensor, no pipeline) §26
- [x] Unit tests: modality, timestamps, uncertainty, provenance, observation construction §48
- [x] Doc-tests and examples (thermal example in `lib.rs`; radiation example, §44, still pending)
- [x] Verify build with `--no-default-features` (no_std, no alloc where practical) §51

## Phase 2 — Streams, events, spatial, temporal, calibration (spec §58 Phase 2)
Modules: `stream`, `event`, `spatial`, `temporal`, `calibration`
- [x] `SensorStream<T>` on `futures-core::Stream`, runtime-agnostic §27
- [x] Bounded buffering + configurable backpressure policy §27, §50
- [x] Cancellation support
- [x] Ordering guarantees + out-of-order handling
- [x] Dropped-sample reporting
- [x] Rate limiting and filtering combinators
- [x] Sync (non-async) polling alternative for embedded / `no_std`
- [x] Event model: `ObjectAppeared`, `ObjectDisappeared`, `ObjectMoved`, `CollisionDetected`, `TemperatureExceeded`, `SoundDetected`, `PressureChanged`, `RadiationSpike`, `SensorFailure`, `CalibrationChanged`, `EnvironmentChanged`, + custom §21
- [x] Event fields: timestamp, source, entity, modality, measurements, confidence, provenance §21
- [x] Spatial reference / coordinate-frame types (world, robot, body, camera, pixel) §18
- [x] Transform trait/interface so external systems (ROS TF) can plug in without a hard dependency §18
- [x] Temporal model: acquisition / processing timestamps, latency, ordering, clock source §17 (sensor-clock and observation timestamps not yet modelled)
- [x] Temporal alignment (interpolation / nearest-sample / windowing) across mixed-rate sensors §17
- [x] Never silently treat asynchronous measurements as simultaneous — enforce in API and test §17
- [x] Calibration metadata: intrinsic, extrinsic, temperature, zero-point, time, alignment §32
- [x] Calibration state included in provenance §32
- [x] Security & trust metadata: source identity, authentication status, integrity status, trust level §33
- [x] Ensure an observation is never automatically treated as a fact §33, §49
- [x] Fault tests: stale data, bad timestamps, out-of-order messages, calibration failure §48 (sensor noise only as a diagnostics field; no noise-injection test yet)
- [x] Tests for streams (backpressure, drop reporting, cancellation)

## Phase 3 — Fusion, entity association, world state (spec §58 Phase 3)
Modules: `fusion`, `entity`, state/world-state module, perception pipeline
- [x] Perception pipeline abstraction with optional stages: validation → calibration → normalisation → feature extraction → classification → association → fusion → observation → world state §26 — stages are pluggable (`Stage` trait); built-in validation and calibration; fusion and association are separate functions, not stages yet
- [x] Pipeline stages expose input / algorithm+version / parameters / output / uncertainty / provenance §47 — trace gives algorithm, input/output confidence and state; stage uncertainty is not yet in the trace
- [x] Fusion API combining multiple measurements/observations into one observation §19
- [x] Fusion preserves underlying evidence and never erases provenance §19
- [x] Baseline fusion strategies (e.g. confidence-weighted, Bayesian-style); pluggable custom strategies
- [x] Sensor disagreement handling and reporting §48
- [x] Entity model (`EntityId`, entity types) §20
- [x] Entity association (many sensors → one entity) with confidence + provenance §20
- [x] Multimodal evidence grouping (visual + auditory + LiDAR + thermal → entity) §38
- [x] World-state representation (robot, nearby entities, environment, robot body) §22
- [x] State derived from observations, never an independent source of truth §22
- [x] Internal sensing: battery, CPU/GPU, memory pressure, temperature, motor temperature, joint position/torque, network quality, sensor/actuator health §23 — modelled as `InternalMetric`; no live sources (those come from the integrating system)
- [x] Interface for higher-level internal observations (prediction error, uncertainty, contradiction, attention demand, goal conflict) without owning cognitive state §23
- [x] Event detection from observation changes §21 — threshold crossing and movement; no general change detector yet
- [x] Fusion tests (sensor A + sensor B → fused observation) §48
- [x] Integration tests (sensor → measurement → observation) §48
- [x] Example: camera + LiDAR + radar + GPS → "Vehicle #27" §19

## Phase 4 — Adapters for existing TPT projects (spec §58 Phase 4)
Consume capabilities from existing repos rather than duplicating them §7. All items depend on those repos' APIs.
- [x] Define the generic adapter trait(s) in core (measurement/feature in → observation out), no dependency on other TPT repos
- [x] Decide adapter packaging (feature flags vs separate crates, e.g. `tpt-senses-vision`, `tpt-senses-audio`) §43: separate crates for TPT-dependent adapters, decided in DESIGN.md; none created yet
- [ ] **(EXT)** `tpt-cv` adapter: visual features/detections → visual observations
- [ ] **(EXT)** `tpt-audio` adapter: audio data/features → auditory observations
- [ ] **(EXT)** `tpt-voice` adapter: speech recognition as an optional auditory component; hearing is not defined by it; non-speech sounds represented §7
- [ ] **(EXT)** `tpt-teleop-agri` adapter: common representation for robot datafeeds §7
- [ ] Verify the licences of every TPT dependency are compatible with MIT OR Apache-2.0 and the dependency policy
- [x] Verify dependency direction: adapters point upward; core never depends on hardware ecosystems §54
- [x] Adapter integration tests and examples (camera, microphone flows §44): mock adapters in `tests/flows.rs`; real adapters are (EXT)

## Phase 5 — External integrations (spec §58 Phase 5)
- [ ] **ROS 2 integration:** `tpt-senses-ros2` — deferred by decision; `rclrs` is Apache-2.0 only, needs a crate-scoped licence exception §8, §41
  - [ ] **(EXT)** Topic adapters: camera, LiDAR, IMU, GPS, force, custom
  - [ ] **(EXT)** ROS TF → spatial reference bridge §18
  - [ ] Decide whether `tpt-senses-ros` (ROS 1) is justified §8
  - [ ] Check the ROS 2 Rust client licence (e.g. rclrs — Apache-2.0 only?) against the dependency policy; resolve conflicts or isolate/skip
- [ ] **Simulation:** `tpt-senses-sim` — deferred by decision; `Sensor` trait and `Origin::Simulated` already give simulators the same interface §30
- [ ] **Record/replay:** `tpt-senses-record` — deferred by decision until a consumer exists; in-core `Recording` with seek, range and replay is done §29
  - [x] Storage-format-agnostic trait in core: `RecordSink` (on-disk format not yet); concrete format only in the record crate §29
  - [ ] Choose compression crate(s) that meet licence policy
- [x] Deterministic replay: same recorded stream → same pipeline → same observations §28: `replay_through`
- [x] Replay tests (determinism where the pipeline is designed to be) §48
- [x] Serialization adapters: `serde` (JSON via dev tests) and `cbor` (`cbor4ii`, MIT) done, with validated deserialization for observations and measurements §39
- [x] Serialization round-trip tests: JSON and CBOR, including rejection of invalid values
- [x] Defer / log decision: MessagePack, Arrow-compatible structures, custom binary §39: recorded in DESIGN.md
- [x] Optional ML hook crate `tpt-senses-ml` only if justified; core stays ML/LLM/cloud-free §40, §43: not created; core stays ML-free (DESIGN.md)
- [x] Review: only create each crate when implementation complexity justifies it §8, §43

## Phase 6 — Advanced perception (spec §58 Phase 6)
- [x] Controllable sensor parameters model: zoom, focus, exposure, sampling frequency, direction, resolution, spectral band, gain, range §37
- [x] Parameter-change request/response types (actual control stays in the device layer) §37
- [x] Active-perception loop types: uncertainty → request additional information → sensor action → new observation §36
- [x] Example: weed vs not-weed with a multispectral follow-up §36
- [x] Attention-aware sensing interface (attention changes sensing rates/resolution) §35
- [ ] **(EXT)** `tpt-anima` interface: observations/events/state out; attention hints in §35
- [ ] **(EXT)** `tpt-thought` consumption interface; confirm no hardware knowledge needed §46, §59
- [ ] **(EXT)** `tpt-eve` verification hooks: inspectable transformations, "why do we believe this?" §16, §47
- [x] Adaptive sensor selection (choose sensors by capability, health, uncertainty)
- [x] Advanced multimodal fusion across modalities §38: `fuse_presence` pools detections from any modalities; general continuous value fusion across modalities is not yet built
- [x] Sensory history export for reflection/sleep processing, without owning memory §46: `observations_between` is a read-only, clock-aware window
- [x] Confirm safety-critical decision-making stays outside the sensory layer §49: see DESIGN.md safety boundary

## Phase 7 — Profiles, hardening, docs & release
- [x] `profile` module: `SensoryProfile` (Human, Robot, Drone, AgriculturalRobot, IndustrialRobot, Spacecraft, Custom) §24
- [x] Profiles defined as capabilities, with a "conceptual not biological" disclaimer §5, §25
- [x] Biological-inspiration reference profiles documented as engineering inspiration only §25
- [x] Custom modality extensibility tests (e.g. gravitational, novel chemical signature) §52
- [x] Full test suite: unit, integration, fusion, replay, fault §48: benchmarks not included
- [x] Fault tests: missing sensors, sensor disagreement, sensor noise, and calibration failure §48
- [ ] Benchmarks: criterion harness in `benches/streams.rs` (queue, reorder, fusion, pipeline, zero-copy clone). Allocation counts still not measured (needs an unsafe GlobalAlloc, which the workspace forbids); criterion is pinned to 0.7 for MSRV 1.85
- [ ] Embedded checks: `no_std` on bare metal verified; binary size and an alloc-free subset not yet done §51
- [x] WASM build verification §41
- [ ] Rustdoc complete with examples; docs.rs config (metadata set; examples still to add to key types)
- [x] DESIGN.md finalised for the current scope, with relationship diagram and API review; revisit at release
- [x] Integration example for each of the spec §44 flows (camera, microphone, agri robot, radiation)
- [x] Final dependency/licence audit (`cargo deny check`) for the current dependency set; repeat before release
- [ ] Security review: untrusted sensor data, trust metadata §33 — self-review in DESIGN.md; deterministic fuzz-style and property tests added; an independent review and coverage-guided fuzzing are still needed
- [ ] Semver / API review before 0.1.0 — API decisions taken and recorded in DESIGN.md (non_exhaustive plain data, validated diagnostics); owner sign-off still open
- [ ] Publish `tpt-senses` 0.1.0 to crates.io
- [ ] Tag release and write CHANGELOG entry

---

## Success criteria checklist (spec §59)
- [ ] 1. A new sensor can be integrated without modifying `tpt-thought`
- [ ] 2. A new modality can be introduced without redesigning the core
- [ ] 3. ROS can provide measurements without becoming a core dependency
- [ ] 4. Existing TPT media repos contribute perception without being absorbed into `tpt-senses`
- [x] 5. Observations retain uncertainty and provenance
- [x] 6. Multiple sensors can be fused into a common observation
- [x] 7. Real and simulated sensors use the same cognitive interface
- [x] 8. Internal system sensors use the same observation model
- [x] 9. Sensory capabilities beyond human perception are supported
- [ ] 10. `tpt-thought` can reason over observations without knowing the hardware

## Open items / to confirm later
- [x] Final workspace directory layout (root crate vs `crates/` folder)
- [x] MSRV value (1.85)
- [ ] Which `tpt-*` repos are public/available and their licences
- [ ] Whether the ROS 2 Rust bindings' licence passes the dependency policy
