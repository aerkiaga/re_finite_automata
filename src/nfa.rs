use crate::dfa::SwitchTable;
use crate::*;
use std::collections::HashMap;
use std::iter::Iterator;
use std::ops::{Add, BitOr, Not, RangeInclusive};
use try_index::TryIndex;

/// A nondeterministic finite-state automaton.
///
/// ## Constructing
/// - [Nfa::from_range]: NFA that matches a single symbol in a range.
/// - [Nfa::append] (**+** *operator*): NFA that matches concatenation.
/// - [Nfa::combine] (**|** *operator*): NFA that matches either of two.
/// - [Nfa::invert] (**!** *operator*): NFA that matches the reverse.
/// - [Nfa::switch]: applies either NFA depending on match status.
/// - [Nfa::from_dfa]: converts a DFA into a NFA.
///
/// ## Matching
/// - [Nfa::run]: typical matching.
/// - [Nfa::run_shortest]: gives shortest possible match.
///
/// ## Example
/// ```rust
/// use re_finite_automata::Nfa;
///
/// // we can construct a NFA using the provided methods
/// // this matches regex `[\0-\5][\4]|[\3]`.
/// let nfa = (Nfa::from_range(0..=5) + Nfa::from_range(4..=4)) | (Nfa::from_range(3..=3));
/// let input = [3, 4, 5, 6];
/// assert_eq!(nfa.run(&input), Some(2)); // first 2 bytes of input match
///
/// let mut iter = input.into_iter(); // any iterator over u8 will do
/// assert!(nfa.run_shortest(&mut iter)); // input matches
/// assert_eq!(iter.as_slice(), [4, 5, 6]); // we can get our match from the iterator
/// // note that .run_shortest() returns the shortest match here
/// ```
#[derive(Clone)]
pub struct Nfa {
    // each transition contains states or indices into the state table
    pub(crate) transitions: Vec<Transition>,
    // first element in each state list is size
    pub(crate) states: Vec<u16>,
}

impl Nfa {
    /// Returns the number of states.
    pub fn size(&self) -> u16 {
        self.transitions.len() as u16
    }

    /// Given a state, gives the range of input symbols that will
    /// cause a transition to the "inside" state.
    pub fn range(&self, state: u16) -> RangeInclusive<u8> {
        let transition = &self.transitions[state as usize];
        transition.min..=transition.max
    }

    /// Given a raw next state value, produce a list of next states.
    pub(crate) fn translate_state<'a>(&'a self, state: &'a u16) -> &'a [u16] {
        if !state <= 1 {
            return std::slice::from_ref(state);
        }
        if state & 0x8000 != 0 {
            let index = (state & !0x8000) as usize;
            let size = self.states[index] as usize;
            &self.states[index + 1..=index + size]
        } else {
            std::slice::from_ref(state)
        }
    }

    /// Given a raw next state value, produce a list of next states.
    fn translate_state_mut<'a>(&'a mut self, state: &'a mut u16) -> &'a mut [u16] {
        if !*state <= 1 {
            return std::slice::from_mut(state);
        }
        if *state & 0x8000 != 0 {
            let index = (*state & !0x8000) as usize;
            let size = self.states[index] as usize;
            &mut self.states[index + 1..=index + size]
        } else {
            std::slice::from_mut(state)
        }
    }

    /// Given a state, returns its "inside" next states.
    pub fn inside(&self, state: u16) -> &[u16] {
        let transition = &self.transitions[state as usize];
        let r = &transition.inside;
        self.translate_state(r)
    }

    /// Given a state, returns its "outside" next states.
    pub fn outside(&self, state: u16) -> &[u16] {
        let transition = &self.transitions[state as usize];
        let r = &transition.outside;
        self.translate_state(r)
    }

    /// Given a state, returns whether to consume input upon transitioning to it.
    pub fn consumes(&self, state: u16) -> bool {
        let transition = &self.transitions[state as usize];
        transition.consume
    }

    /// Given a state and an input symbol, produces all next states.
    pub fn apply(&self, state: u16, input: u8) -> &[u16] {
        let transition = &self.transitions[state as usize];
        let r = if (transition.min..=transition.max).contains(&input) {
            &transition.inside
        } else {
            &transition.outside
        };
        self.translate_state(r)
    }

    /// Runs the state machine on some input and returns whether the input is accepted.
    /// Matches the shortest possible input, using breadth-first search.
    ///
    /// This function is about 6 times slower than [Nfa::run],
    /// but guarantees the shortest possible match,
    /// and may be faster for some inputs.
    /// Match length may be extracted from the iterator.
    pub fn run_shortest<I: Iterator<Item = u8>>(&self, input: &mut I) -> bool {
        let l = self.transitions.len() as u16;
        let mut states_a = BitSet::new_with_size(l);
        let mut states_b = BitSet::new_with_size(l);
        let mut states_c = BitSet::new_with_size(l);
        states_a.insert(INITIAL_STATE);
        for symbol in input.by_ref() {
            if states_a.is_empty() {
                return false;
            }
            while let Some(state) = states_a.iter_next_remove() {
                if !states_c.contains(state) {
                    let new_states = self.apply(state, symbol);
                    for new_state in new_states {
                        if (!new_state) <= 1 {
                            if *new_state == ACCEPTING_STATE {
                                return true;
                            }
                        } else if self.consumes(*new_state) {
                            states_b.insert(*new_state);
                        } else {
                            states_a.insert(*new_state);
                        }
                    }
                    states_c.insert(state);
                }
            }
            std::mem::swap(&mut states_a, &mut states_b);
            states_c.drain();
        }
        false
    }

    /// Runs the state machine on some input and returns whether the input is accepted,
    /// and if so the number of bytes parsed.
    /// Respects matching priorities, using depth-first search.
    ///
    /// This function is usually much faster than [Nfa::run_shortest].
    pub fn run<I: TryIndex<usize, Output = u8> + ?Sized>(&self, input: &I) -> Option<usize> {
        let mut index = 0;
        let mut state = INITIAL_STATE;
        let mut stack = vec![];
        let mut symbol = 0;
        loop {
            if (!state) <= 1 {
                if state == ACCEPTING_STATE {
                    return Some(index);
                } else {
                    (index, state, symbol) = stack.pop()?;
                    continue;
                }
            }
            if self.consumes(state) {
                symbol = match input.try_index(index) {
                    Some(x) => *x,
                    None => {
                        (index, state, symbol) = stack.pop()?;
                        continue;
                    }
                };
                index = index.wrapping_add(1);
            }
            let new_states = self.apply(state, symbol);
            for n in (1..new_states.len()).rev() {
                stack.push((index, new_states[n], symbol));
            }
            state = new_states[0];
        }
    }

    /// Creates a new NFA that matches a single symbol within a range.
    pub fn from_range(range: RangeInclusive<u8>) -> Self {
        Nfa::from_dfa(Dfa::from_range(range))
    }

    fn iter_transitions(&mut self) -> NfaTransitionIterator<'_> {
        NfaTransitionIterator {
            nfa: self,
            state: 0,
            inside: true,
        }
    }

    fn rebase_states_array(&mut self, offset: u16) {
        for transition in self.iter_transitions() {
            if !*transition > 1 && *transition & 0x8000 != 0 {
                *transition += offset;
            }
        }
    }

    fn iter_states(&mut self) -> NfaStateIterator<'_> {
        NfaStateIterator {
            nfa: self,
            state: 0,
            inside: true,
            index: 0,
        }
    }

    fn rebase_transition_states(&mut self, offset: u16) {
        for state in self.iter_states() {
            if !*state > 1 {
                *state += offset;
            }
        }
    }

    fn replace_state(&mut self, old: u16, new: u16) {
        for state in self.iter_states() {
            if *state == old {
                *state = new;
            }
        }
    }

    /// Creates a new NFA that matches the concatenation of two NFAs.
    pub fn append(mut self, mut other: Self) -> Self {
        let toffset = self.transitions.len() as u16;
        let soffset = self.states.len() as u16;
        self.replace_state(ACCEPTING_STATE, toffset);
        other.rebase_transition_states(toffset);
        other.rebase_states_array(soffset);
        self.transitions.append(&mut other.transitions);
        self.states.append(&mut other.states);
        self
    }

    /// Creates a new NFA that matches either of two NFAs.
    pub fn combine(mut self, mut other: Self) -> Self {
        let toffset = self.transitions.len() as u16;
        let soffset = self.states.len() as u16;
        let mut r = Nfa {
            transitions: vec![Transition {
                min: 0,
                max: 255,
                inside: 0x8000,
                outside: 0,
                consume: true,
            }],
            states: vec![2, 1, toffset + 1],
        };
        self.transitions[0].consume = false;
        other.transitions[0].consume = false;
        self.rebase_transition_states(1);
        self.rebase_states_array(3);
        other.rebase_transition_states(toffset + 1);
        other.rebase_states_array(soffset + 3);
        r.transitions.append(&mut self.transitions);
        r.states.append(&mut self.states);
        r.transitions.append(&mut other.transitions);
        r.states.append(&mut other.states);
        r
    }

    /// Creates a new NFA with opposite matching behavior.
    pub fn invert(mut self) -> Self {
        for state in self.iter_states() {
            if *state == ACCEPTING_STATE {
                *state = REJECTING_STATE;
            } else if *state == REJECTING_STATE {
                *state = ACCEPTING_STATE;
            }
        }
        self
    }

    /// Creates a new NFA that matches zero or more times, as many as possible.
    pub fn repeat_greedy(mut self) -> Self {
        let mut r = Nfa {
            transitions: vec![Transition {
                min: 0,
                max: 255,
                inside: 0x8000,
                outside: 0,
                consume: false,
            }],
            states: vec![2, 1, ACCEPTING_STATE],
        };
        self.rebase_transition_states(1);
        self.rebase_states_array(3);
        self.replace_state(ACCEPTING_STATE, 0);
        r.transitions.append(&mut self.transitions);
        r.states.append(&mut self.states);
        r
    }

    /// Creates a new NFA that matches zero or more times, as few as possible.
    pub fn repeat_lazy(mut self) -> Self {
        let mut r = Nfa {
            transitions: vec![Transition {
                min: 0,
                max: 255,
                inside: 0x8000,
                outside: 0,
                consume: false,
            }],
            states: vec![2, ACCEPTING_STATE, 1],
        };
        self.rebase_transition_states(1);
        self.rebase_states_array(3);
        self.replace_state(ACCEPTING_STATE, 0);
        r.transitions.append(&mut self.transitions);
        r.states.append(&mut self.states);
        r
    }

    /// Creates a new NFA that matches either depending on result of current NFA.
    /// Will remove first consumed input from following NFAs.
    pub fn switch(mut self, mut accept: Self, mut reject: Self) -> Self {
        let toffset = self.transitions.len() as u16;
        let toffset2 = accept.transitions.len() as u16;
        let soffset = self.states.len() as u16;
        let soffset2 = accept.states.len() as u16;
        self.replace_state(ACCEPTING_STATE, toffset);
        self.replace_state(REJECTING_STATE, toffset + toffset2);
        accept.transitions[0].consume = false;
        reject.transitions[0].consume = false;
        accept.rebase_transition_states(toffset);
        reject.rebase_transition_states(toffset + toffset2);
        accept.rebase_states_array(soffset);
        reject.rebase_states_array(soffset + soffset2);
        self.transitions.append(&mut accept.transitions);
        self.transitions.append(&mut reject.transitions);
        self
    }

    /// Converts a DFA into a NFA. This operation is constant-time and fast,
    /// unlike the opposite conversion (see [Dfa::from_nfa]).
    pub fn from_dfa(dfa: Dfa) -> Self {
        Self {
            transitions: dfa.transitions,
            states: vec![],
        }
    }

    pub(crate) fn explore_transitions(
        &self,
        mut states_a: BitSet,
        mut states_b: BitSet,
        symbol: u8,
    ) -> (Option<BitSet>, u8) {
        let mut max = 255;
        states_b.drain();
        while let Some(state) = states_a.iter_next_remove() {
            let trans = &self.transitions[state as usize];
            let new_states = if symbol < trans.min {
                max = std::cmp::min(max, trans.min - 1);
                self.translate_state(&trans.outside)
            } else if symbol > trans.max {
                self.translate_state(&trans.outside)
            } else {
                max = std::cmp::min(max, trans.max);
                self.translate_state(&trans.inside)
            };
            for new_state in new_states {
                if (!new_state) <= 1 {
                    if *new_state == ACCEPTING_STATE {
                        return (None, max);
                    }
                } else if self.consumes(*new_state) {
                    states_b.insert(*new_state);
                } else {
                    states_a.insert(*new_state);
                }
            }
        }
        (Some(states_b), max)
    }

    pub(crate) fn compute_powerset_map(&self) -> HashMap<BitSet, SwitchTable<Option<BitSet>>> {
        let l = self.transitions.len() as u16;
        let mut map = HashMap::new();
        let mut pending = vec![];
        let mut states = BitSet::new_with_size(l);
        states.insert(INITIAL_STATE);
        pending.push(states);
        while let Some(states) = pending.pop() {
            let mut ranges: Vec<RangeInclusive<u8>> = vec![];
            let mut last_states: Vec<Option<BitSet>> = vec![];
            let mut states_b = BitSet::new_with_size(l);
            let mut symbol = 0;
            loop {
                let (next_states, max) = self.explore_transitions(states.clone(), states_b, symbol);
                let mut new = true;
                if ranges.last().is_some()
                    && let Some(next) = last_states.last()
                    && *next == next_states
                {
                    *ranges.last_mut().unwrap() = *ranges.last().unwrap().start()..=max;
                    new = false;
                }
                if new {
                    last_states.push(next_states.clone());
                    ranges.push(symbol..=max);
                    match next_states {
                        Some(next) if !map.contains_key(&next) && !next.clone().is_empty() => {
                            pending.push(next.clone());
                        }
                        _ => {}
                    }
                    states_b = BitSet::new_with_size(l);
                } else {
                    states_b = next_states.unwrap_or_else(|| BitSet::new_with_size(l));
                }
                if max == 255 {
                    break;
                }
                symbol = max + 1;
            }
            map.insert(states.clone(), (ranges, last_states));
        }
        map
    }
}

impl Add for Nfa {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        self.append(other)
    }
}

impl BitOr for Nfa {
    type Output = Self;

    fn bitor(self, other: Self) -> Self::Output {
        self.combine(other)
    }
}

impl Not for Nfa {
    type Output = Self;

    fn not(self) -> Self::Output {
        self.invert()
    }
}

struct NfaTransitionIterator<'a> {
    nfa: &'a mut Nfa,
    state: u16,
    inside: bool,
}

impl<'a> Iterator for NfaTransitionIterator<'a> {
    type Item = &'a mut u16;

    fn next(&mut self) -> Option<Self::Item> {
        let trans: &'a mut Vec<Transition> = unsafe { &mut *(&mut self.nfa.transitions as *mut _) };
        let r = if self.state as usize >= trans.len() {
            None
        } else if self.inside {
            Some(&mut trans[self.state as usize].inside)
        } else {
            Some(&mut trans[self.state as usize].outside)
        };
        if !self.inside {
            self.state += 1;
        }
        self.inside = !self.inside;
        r
    }
}

struct NfaStateIterator<'a> {
    nfa: &'a mut Nfa,
    state: u16,
    inside: bool,
    index: u16,
}

impl<'a> Iterator for NfaStateIterator<'a> {
    type Item = &'a mut u16;

    fn next(&mut self) -> Option<Self::Item> {
        let trans: &'a mut Vec<Transition> = unsafe { &mut *(&mut self.nfa.transitions as *mut _) };
        let mut r = if self.state as usize >= trans.len() {
            return None;
        } else if self.inside {
            &mut trans[self.state as usize].inside
        } else {
            &mut trans[self.state as usize].outside
        };
        if *r & 0x8000 != 0 {
            let v = self.nfa.translate_state_mut(r);
            let l = v.len();
            r = &mut v[self.index as usize];
            self.index += 1;
            if self.index as usize >= l {
                self.index = 0;
            }
        }
        if self.index == 0 {
            if !self.inside {
                self.state += 1;
            }
            self.inside = !self.inside;
        }
        Some(unsafe { &mut *(r as *mut _) })
    }
}

#[test]
fn nfa_run_test() {
    let mut nfa = Nfa {
        transitions: vec![
            Transition {
                min: 0,
                max: 255,
                inside: 0x8000,
                outside: 0,
                consume: true,
            },
            Transition {
                min: 5,
                max: 5,
                inside: ACCEPTING_STATE,
                outside: 2,
                consume: false,
            },
            Transition {
                min: 6,
                max: 8,
                inside: 3,
                outside: REJECTING_STATE,
                consume: false,
            },
            Transition {
                min: 6,
                max: 8,
                inside: ACCEPTING_STATE,
                outside: REJECTING_STATE,
                consume: true,
            },
            Transition {
                min: 15,
                max: 15,
                inside: ACCEPTING_STATE,
                outside: REJECTING_STATE,
                consume: true,
            },
        ],
        states: vec![2, 1, 4],
    };
    assert_eq!(nfa.size(), 5);
    assert_eq!(nfa.range(2), 6..=8);
    assert_eq!(nfa.inside(2), [3]);
    assert_eq!(nfa.outside(1), [2]);
    assert_eq!(nfa.consumes(1), false);
    assert_eq!(nfa.apply(2, 7), [3]);
    assert_eq!(nfa.translate_state(&0x8000), [1, 4]);
    assert_eq!(nfa.translate_state_mut(&mut 0x8000), [1, 4]);
    assert_eq!(nfa.translate_state_mut(&mut 2), [2]);
    assert!(nfa.run_shortest(&mut [5, 9].into_iter()));
    assert_eq!(nfa.run(&[5, 9]), Some(1));
    assert!(nfa.run_shortest(&mut [7, 8].into_iter()));
    assert_eq!(nfa.run(&[7, 8]), Some(2));
    assert!(!nfa.run_shortest(&mut [9, 7].into_iter()));
    assert_eq!(nfa.run(&[9, 7]), None);
    assert!(nfa.run_shortest(&mut [9, 15].into_iter()));
    assert_eq!(nfa.run(&[9, 15]), Some(2));
}

#[test]
fn nfa_add_test() {
    let nfa1 = Nfa::from_range(4..=5);
    let nfa2 = Nfa::from_range(6..=6);
    let nfa = nfa1 + nfa2;
    assert!(nfa.run_shortest(&mut [4, 6].into_iter()));
    assert!(!nfa.run_shortest(&mut [4, 5].into_iter()));
    assert!(!nfa.run_shortest(&mut [6, 6].into_iter()));
    assert!(nfa.run(&[4, 6]).is_some());
    assert!(!nfa.run(&[4, 5]).is_some());
    assert!(!nfa.run(&[6, 6]).is_some());
}

#[test]
fn nfa_not_test() {
    let nfa1 = Nfa::from_range(4..=5);
    let nfa2 = Nfa::from_range(6..=6);
    let nfa = !(nfa1 + nfa2);
    assert!(!nfa.run_shortest(&mut [4, 6].into_iter()));
    assert!(nfa.run_shortest(&mut [4, 5].into_iter()));
    assert!(nfa.run_shortest(&mut [6, 6].into_iter()));
    assert!(!nfa.run(&[4, 6]).is_some());
    assert!(nfa.run(&[4, 5]).is_some());
    assert!(nfa.run(&[6, 6]).is_some());
}

#[test]
fn nfa_or_test() {
    let nfa1 = Nfa::from_range(4..=5);
    let nfa2 = Nfa::from_range(6..=6);
    let nfa = nfa1 | nfa2;
    assert!(nfa.run_shortest(&mut [4].into_iter()));
    assert!(nfa.run_shortest(&mut [6].into_iter()));
    assert!(!nfa.run_shortest(&mut [7].into_iter()));
    assert!(nfa.run(&[4]).is_some());
    assert!(nfa.run(&[6]).is_some());
    assert!(!nfa.run(&[7]).is_some());
}

#[test]
fn nfa_compound_test() {
    let nfa0 = Nfa::from_range(0..=0);
    let nfa1 = Nfa::from_range(1..=1);
    let nfa = ((nfa0.clone() | nfa1.clone()) + nfa0.clone() + nfa1.clone()) | (nfa0 + nfa1);
    assert!(nfa.run_shortest(&mut [0, 0, 1].into_iter()));
    assert!(nfa.run_shortest(&mut [0, 1].into_iter()));
    assert!(nfa.run_shortest(&mut [1, 0, 1].into_iter()));
    assert!(!nfa.run_shortest(&mut [1, 0, 0].into_iter()));
    assert!(nfa.run_shortest(&mut [0, 1, 0].into_iter()));
    assert!(!nfa.run_shortest(&mut [1, 0].into_iter()));
    assert!(nfa.run(&[0, 0, 1]).is_some());
    assert!(nfa.run(&[0, 1]).is_some());
    assert!(nfa.run(&[1, 0, 1]).is_some());
    assert!(!nfa.run(&[1, 0, 0]).is_some());
    assert!(nfa.run(&[0, 1, 0]).is_some());
    assert!(!nfa.run(&[1, 0]).is_some());
}

#[test]
fn nfa_switch_test() {
    let nfa0 = Nfa::from_range(0..=1);
    let nfa1 = Nfa::from_range(0..=0) + Nfa::from_range(2..=2);
    let nfa2 = Nfa::from_range(2..=2) + Nfa::from_range(0..=0);
    let nfa = nfa0.switch(nfa1, nfa2);
    assert!(nfa.run_shortest(&mut [0, 2].into_iter()));
    assert!(!nfa.run_shortest(&mut [0, 1].into_iter()));
    assert!(!nfa.run_shortest(&mut [1].into_iter()));
    assert!(nfa.run_shortest(&mut [2, 0].into_iter()));
    assert!(!nfa.run_shortest(&mut [2, 1].into_iter()));
    assert!(!nfa.run_shortest(&mut [3].into_iter()));
    assert_eq!(nfa.run(&[0, 2]), Some(2));
    assert_eq!(nfa.run(&[0, 1]), None);
    assert_eq!(nfa.run(&[1]), None);
    assert_eq!(nfa.run(&[2, 0]), Some(2));
    assert_eq!(nfa.run(&[2, 1]), None);
    assert_eq!(nfa.run(&[3]), None);
}

#[test]
fn nfa_out_test() {
    let nfa = Nfa::from_range(0..=0) + Nfa::from_range(0..=0);
    assert!(!nfa.run_shortest(&mut [0].into_iter()));
    assert_eq!(nfa.run(&[0]), None);
}

#[test]
fn nfa_repeat_greedy_test() {
    let nfa0 = Nfa::from_range(0..=0);
    let nfa1 = Nfa::from_range(1..=1);
    let nfa = nfa0 + nfa1.clone().repeat_greedy() + nfa1;
    assert_eq!(nfa.run(&[0, 1]), Some(2));
    assert_eq!(nfa.run(&[0, 1, 1]), Some(3));
    assert_eq!(nfa.run(&[0, 1, 0, 1]), Some(2));
    assert_eq!(nfa.run(&[0, 0, 1]), None);
}

#[test]
fn nfa_repeat_lazy_test() {
    let nfa0 = Nfa::from_range(0..=0);
    let nfa1 = Nfa::from_range(1..=1);
    let nfa = nfa0 + nfa1.clone().repeat_lazy() + nfa1;
    assert_eq!(nfa.run(&[0, 1]), Some(2));
    assert_eq!(nfa.run(&[0, 1, 1]), Some(2));
    assert_eq!(nfa.run(&[0, 1, 0, 1]), Some(2));
    assert_eq!(nfa.run(&[0, 0, 1]), None);
}

#[test]
fn nfa_dfa_add_test() {
    let dfa1 = Dfa::from_range(4..=5);
    let dfa2 = Dfa::from_range(6..=6);
    let dfa = dfa1 + dfa2;
    let nfa = Nfa::from_dfa(dfa);
    assert!(nfa.run_shortest(&mut [4, 6].into_iter()));
    assert!(!nfa.run_shortest(&mut [4, 5].into_iter()));
    assert!(!nfa.run_shortest(&mut [6, 6].into_iter()));
    assert!(nfa.run(&[4, 6]).is_some());
    assert!(!nfa.run(&[4, 5]).is_some());
    assert!(!nfa.run(&[6, 6]).is_some());
}

#[test]
fn nfa_dfa_not_test() {
    let dfa1 = Dfa::from_range(4..=5);
    let dfa2 = Dfa::from_range(6..=6);
    let dfa = !(dfa1 + dfa2);
    let nfa = Nfa::from_dfa(dfa);
    assert!(!nfa.run_shortest(&mut [4, 6].into_iter()));
    assert!(nfa.run_shortest(&mut [4, 5].into_iter()));
    assert!(nfa.run_shortest(&mut [6, 6].into_iter()));
    assert!(!nfa.run(&[4, 6]).is_some());
    assert!(nfa.run(&[4, 5]).is_some());
    assert!(nfa.run(&[6, 6]).is_some());
}
