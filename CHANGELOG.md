# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- A new depth-first `Nfa::run` method.
- `from_range`, `append` and `invert` for creating `Dfa` objects.
- `switch` for creating `Dfa` and `Nfa` objects.
- `repeat_greedy` and `repeat_lazy` for creating `Nfa` objects.
- Several new tests and benchmarks.

### Changed

- Made methods take mutable references to iterators instead of iterators.
- Renamed `Nfa::run` to `Nfa::run_shortest` and added new `Nfa::run`.
- Made `Nfa::run_shortest` about 3x faster.
- Made `Dfa::run` about 25% faster.

## [0.1.0] - 2026-03-15

### Added

- Deterministic and nondeterministic finite-state automata types.
- Methods for inspecting automata.
- Methods for running automata on byte iterators.
- `from_range`, `append`, `combine` and `invert` for creating `Nfa` objects.
- Operator overrides for the above.
- Various tests and benchmarks.
