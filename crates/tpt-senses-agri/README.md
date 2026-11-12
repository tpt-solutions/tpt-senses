# tpt-senses-agri

An adapter that turns `tpt-teleop-agri` field data into
[`tpt-senses`](https://crates.io/crates/tpt-senses) observations.

It accepts:

- **GNSS fixes** (`GnssFixMirror`): position in WGS-84, with horizontal precision recorded as the
  observation's uncertainty. A precision of `0.0` means unknown.
- **Wind** (`WindMirror`), **soil** (`SoilMirror`) and **rain** (`RainMirror`) samples.

Each input becomes an `Observation` that carries its sensor identity, timestamp and provenance.

## Why mirrors?

`tpt-teleop-agri` is not published to crates.io, and a crates.io package cannot depend on a git
source. The input types here are therefore mirrors of the source structs. Each mirror names the
file and struct it was read from. The mirrors were read from `tpt-teleop-agri` commit `1c469ff`.
When the source changes, update the mirror and its tests together.

## Usage

Add the crate with `cargo add tpt-senses-agri`. Use the `Adapter` implementation from this crate
to convert a mirror value into observations, then feed those observations into a `tpt-senses`
pipeline or world state.

## Minimum supported Rust version

1.85 (edition 2024).

## Licence

Licensed under either of Apache License, Version 2.0 or MIT license, at your option.
