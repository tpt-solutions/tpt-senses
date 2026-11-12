# Changelog

All notable changes to this crate are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this crate
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Adapter from `tpt-teleop-agri` field data to `tpt-senses` observations: GNSS fixes (with
  horizontal precision recorded as uncertainty), wind, soil and rain samples.
- Input types are mirrors of the `tpt-teleop-agri` structs, read from commit `1c469ff`.
