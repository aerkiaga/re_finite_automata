#![feature(int_lowest_highest_one)]

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
