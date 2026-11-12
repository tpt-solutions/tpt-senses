# tpt-senses-cv

An adapter from `tpt-cv` to [`tpt-senses`](https://crates.io/crates/tpt-senses).

`tpt-cv` produces geometry: keypoints, optical-flow tracks and camera poses. This crate turns that
output into `Observation`s with a confidence, provenance and a spatial reference. Interpretation is
left to the layers above.

Confidence is a policy choice, not something `tpt-cv` reports. It comes from `CvConfig`, so the
integrator decides how much each kind of measurement is worth:

- `keypoint_confidence`: FAST responses are scores, not probabilities.
- `flow_confidence`: each successful optical-flow track.
- `pose_confidence`: each camera pose estimate.

## Dependencies

This crate depends on the published `tpt-cv-feature` and `tpt-cv-geo` 0.1 crates. The `tpt-cv`
release it was written against is recorded in the `TPT_CV_VERSION` constant. Tests also use
`tpt-cv-core`, and run real detection and tracking end to end.

## Usage

Add the crate with `cargo add tpt-senses-cv`. Create a `CvConfig` with the sensor's `SensorId` and
the three confidence values, then pass `tpt-cv` output to `CvAdapter`.

## Minimum supported Rust version

1.85 (edition 2024).

## Licence

Licensed under either of Apache License, Version 2.0 or MIT license, at your option.
