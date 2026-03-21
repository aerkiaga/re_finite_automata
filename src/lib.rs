#![feature(int_lowest_highest_one)]

//! A crate for constructing and simulating finite-state automata,
//! with a focus on regex matches on byte arrays.
//!
//! This crate provides two types of finite automata:
//! - **Deterministic FAs** ([Dfa]).
//! - **Nondeterministic FAs** ([Nfa]).
//!
//! The methods implemented for each allow to:
//! - Construct them using primitive composition.
//! - Run them to check if an input matches.
//! - Implement custom matching code on top of them.
//! - Convert either type of FA into the other.

/// The initial state for a finite-state automaton.
pub const INITIAL_STATE: u16 = 0;

/// The accepting state for a finite-state automaton.
pub const ACCEPTING_STATE: u16 = 0xfffe;

/// The rejecting state for a finite-state automaton.
pub const REJECTING_STATE: u16 = 0xffff;

/// One transition rule in a finite-state automaton.
///
/// If the input is within the (inclusive) range,
/// the "inside" state is taken, otherwise the "outside" state.
/// May consume input before the transition.
#[derive(Clone)]
struct Transition {
    min: u8,
    max: u8,
    inside: u16,
    outside: u16,
    consume: bool,
}

mod dfa;
pub use dfa::Dfa;

mod bitset;
pub(crate) use bitset::BitSet;

mod nfa;
pub use nfa::Nfa;
