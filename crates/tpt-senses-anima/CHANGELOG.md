# Changelog

All notable changes to this crate are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `observation_to_memory_node`: builds a `tpt-anima` `MemoryNode` from an observation, with the
  observation as a CBOR payload and a caller-supplied `Embedder`.
- `sensory_modality`: maps `Visual` and `Auditory` to `tpt-anima` modalities; others are `Abstract`.
- Pinned git dependency on `tpt-anima-sse` at commit `d02d628`.
