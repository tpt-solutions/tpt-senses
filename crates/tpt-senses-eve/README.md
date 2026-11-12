# tpt-senses-eve

Adapter from `tpt-senses` observations to `tpt-eve` facts.

`tpt-eve` reasons over subject–predicate–object facts. This crate turns each observation into one
`tpt_eve_core::Fact`, keeping its confidence and recording its source as
`FactSource::Extracted`. Uncertainty and the full provenance chain stay on the observation; use
`tpt_senses::explain` when a verifier needs them.

- `observation_to_fact`: converts one observation. Vectors have no single-triple form and return `None`.
- `EveFactSink`: an `ObservationSink` that collects facts and counts the observations it skipped.

This crate depends on `tpt-eve-core` (crates.io, MIT OR Apache-2.0). It does not depend on the
`tpt-eve` engine.

## Licence

Licensed under either of [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE) at your option.
