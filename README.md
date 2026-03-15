# re_finite_automata

A crate for constructing and simulating finite-state automata,
with a focus on regex matches on byte arrays.

## Features

- Deterministic (DFA) and nondeterministic (NFA) finite-state automata.
- Generic matching implementations as well as methods for implementing custom ones.
- Primitive operators for constructing NFAs.

## ToDo

- Primitive operators for constructing DFAs.
- Conversion of NFAs into DFAs.
- DFA optimization.

## Testing

```shell
cargo test
cargo llvm-cov
cargo mutants
```
