# Changelog

All notable changes to this crate are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this crate
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Adapter from `tpt-audio` level readings to `tpt-senses` auditory observations, with peak and RMS
  levels in linear amplitude.
- `LevelMirror` mirrors the audio engine's `Meter`, read from commit `7268ccb`.
