# tpt-senses-audio

An adapter that turns `tpt-audio` level readings into
[`tpt-senses`](https://crates.io/crates/tpt-senses) auditory observations.

`LevelMirror` carries the peak and RMS levels a channel's meter reports. Both are linear amplitude;
the meter does not report decibels, so any conversion is the caller's choice. Each reading becomes
an auditory `Observation` with its channel, timestamp and provenance.

For speech, see [`tpt-senses-voice`](https://crates.io/crates/tpt-senses-voice). Non-speech sound
is handled here.

## Why mirrors?

`tpt-audio` is not published to crates.io, and a crates.io package cannot depend on a git source.
`LevelMirror` mirrors `Meter::peak()` and `Meter::rms()` in `tpt-av-audio-core`, read from commit
`7268ccb`. When the source changes, update the mirror and its tests together.

## Usage

Add the crate with `cargo add tpt-senses-audio`, then use the `Adapter` implementation to convert
`AudioInput` values into observations.

## Minimum supported Rust version

1.85 (edition 2024).

## Licence

Licensed under either of Apache License, Version 2.0 or MIT license, at your option.
