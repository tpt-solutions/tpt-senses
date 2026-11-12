# Changelog

All notable changes to this crate are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `to_sensed_observation`: converts an observation to a `SensedObservation` for `tpt-thought`
  (timestamp, source, trust class and a value map that keeps confidence, origin and clock).
- Pinned git dependency on `tpt-thought-core` at commit `8f00ec3`.
