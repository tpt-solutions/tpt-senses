# Changelog

All notable changes to this crate are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this crate
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Adapter from `tpt-voice` output to `tpt-senses` auditory observations: word timings and speaker
  turns. Non-speech sound is left to `tpt-senses-audio`.
- Input types are mirrors of the `tpt-voice` structs, read from commit `acd3307`. The speaker's
  display name is not carried.
