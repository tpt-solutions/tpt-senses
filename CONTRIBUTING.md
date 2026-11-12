# Contributing to tpt-senses

Thanks for your interest in contributing.

## Licence of contributions

`tpt-senses` is dual-licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE). Unless you
explicitly state otherwise, any contribution you intentionally submit for inclusion is licensed
under the same terms, without any additional conditions.

Only submit code you have the right to contribute. Dependencies must satisfy the licence policy in
`deny.toml`.

## Before you open a pull request

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --no-default-features
```

## Design rules

Read [`DESIGN.md`](DESIGN.md) and [`spec.txt`](spec.txt) first. In particular:

- The core must stay free of ROS, ML/LLM, cloud and hardware-driver dependencies.
- Measurements and observations must keep uncertainty and provenance.
- An observation is evidence, never automatically a fact.
- Add tests for every new validated type and every new invariant.
- Keep new public items documented; the crate builds with `missing_docs` as a warning.

## Changelog

Add an entry under `[Unreleased]` in [`CHANGELOG.md`](CHANGELOG.md) for user-visible changes.
