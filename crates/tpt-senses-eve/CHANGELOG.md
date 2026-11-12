# Changelog

All notable changes to this crate are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `observation_to_fact`: converts an observation to a `tpt_eve_core::Fact`. Confidence carries over;
  the source is `FactSource::Extracted`; vectors are skipped.
- `EveFactSink`: an `ObservationSink` that collects facts for `tpt-eve` and counts skipped
  observations.
- Depends on `tpt-eve-core` 0.1.0 from crates.io.
