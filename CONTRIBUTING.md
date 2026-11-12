# Contributing to tpt-senses

Thanks for your interest in `tpt-senses`. Contributions are accepted through **issues only**: bug
reports, feature requests and questions. Please do not open pull requests.

## Reporting a bug or requesting a feature

Open an issue and include enough detail to act on it:

- For bugs: what you did, what you expected, what happened, and your Rust version and platform.
- For feature requests: the problem you are trying to solve and why the current behaviour is not enough.

## Design rules

Read [`DESIGN.md`](DESIGN.md) and [`spec.txt`](spec.txt) first. Feature requests should respect:

- The core must stay free of ROS, ML/LLM, cloud and hardware-driver dependencies.
- Measurements and observations must keep uncertainty and provenance.
- An observation is evidence, never automatically a fact.

## Licence

`tpt-senses` is dual-licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE). Unless you
explicitly state otherwise, any contribution you intentionally submit for inclusion is licensed
under the same terms, without any additional conditions.
