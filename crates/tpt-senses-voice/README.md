# tpt-senses-voice

An adapter from `tpt-voice` output to [`tpt-senses`](https://crates.io/crates/tpt-senses)
auditory observations.

Speech recognition is an optional part of hearing. This crate represents what the speech stack
reports, namely word timings and speaker turns, as auditory observations. It does not define
hearing, and it does not produce observations for non-speech sounds. Those come from
[`tpt-senses-audio`](https://crates.io/crates/tpt-senses-audio).

## Inputs

- `WordMirror`: a recognised word with start and end times in seconds and a recogniser confidence
  from 0 to 1.
- `SpeakerMirror`: a speaker turn from diarization.

The speaker's display name is deliberately not mirrored. It is personal data, and the speaker
identifier is enough for the observation.

## Why mirrors?

`tpt-voice` is not published to crates.io, and a crates.io package cannot depend on a git source.
The mirrors were read from commit `acd3307`:

- `WordMirror`: `tpt-av-voice-utils/src/transcript.rs`, `WordTimestamp`.
- `SpeakerMirror`: `tpt-av-voice-diarize/src/diarizer.rs`, `SpeakerSegment`.

When the source changes, update the mirror and its tests together.

## Usage

Add the crate with `cargo add tpt-senses-voice`, then use the `Adapter` implementation to convert
the mirror types into observations.

## Minimum supported Rust version

1.85 (edition 2024).

## Licence

Licensed under either of Apache License, Version 2.0 or MIT license, at your option.
