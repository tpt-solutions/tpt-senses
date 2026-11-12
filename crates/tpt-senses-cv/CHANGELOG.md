# Changelog

All notable changes to this crate are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this crate
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Adapter from `tpt-cv` to `tpt-senses` observations: keypoints, optical-flow tracks and camera
  poses, each with a confidence taken from `CvConfig`.
- `TPT_CV_VERSION` constant recording the `tpt-cv` release the adapter targets (`0.1.0`).
- End-to-end tests that run real detection and tracking through `tpt-cv`.
