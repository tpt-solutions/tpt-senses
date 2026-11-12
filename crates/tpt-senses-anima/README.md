# tpt-senses-anima

Adapter from `tpt-senses` observations to `tpt-anima` memory nodes.

`observation_to_memory_node` builds a `tpt_anima_sse::MemoryNode` for one observation:

- **Payload:** the observation, CBOR-encoded (`application/cbor`). It decodes back to the same
  observation with `tpt_senses::from_cbor`.
- **Modality:** `Visual` and `Auditory` map to `Visual` and `Audio`. Other modalities are `Abstract`.
- **Embedding:** produced by a caller-supplied `Embedder`. `tpt-anima`'s own embedders are demo
  pipelines, so this crate does not choose one.

The caller also supplies the memory ID and creator agent ID, since `tpt-anima` owns its identifier
scheme.

## Pinned dependency

`tpt-anima-sse` is taken from git at a fixed commit (see `Cargo.toml`). `tpt-anima` is not on
crates.io, so this crate cannot be published while it has a git dependency.

## Licence

Licensed under either of [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE) at your option.
