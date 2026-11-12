# tpt-senses-thought

Adapter from `tpt-senses` observations to `tpt-thought` observation input.

`tpt-thought` reasons over structured values with a source and a trust class.
`to_sensed_observation` turns one `tpt_senses::Observation` into a `SensedObservation` with a
timestamp, a source, a `SourceTrust` and a value map. The map keeps the evidence visible:
confidence, origin, clock, entity and standard deviation.

The adapter does not build a `tpt_thought_loop::Input`. The question, goal, constraints and
candidate plans belong to the cognitive caller.

## Pinned dependency

`tpt-thought-core` is taken from git at a fixed commit (see `Cargo.toml`). `tpt-thought` is not on
crates.io, so this crate cannot be published while it has a git dependency. Update the pin
deliberately, and re-run the tests after each change.

## Licence

Licensed under either of [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE) at your option.
