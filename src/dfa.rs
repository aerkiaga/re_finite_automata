use crate::*;

use std::collections::HashMap;
use std::iter::Iterator;
use std::ops::{Add, Not, RangeInclusive};

/// A deterministic finite-state automaton.
///
/// ## Constructing
/// - [Dfa::from_range]: DFA that matches a single symbol in a range.
/// - [Dfa::append] (**+** *operator*): DFA that matches concatenation.
/// - [Dfa::invert] (**!** *operator*): DFA that matches the reverse.
/// - [Dfa::switch]: applies either DFA depending on match status.
/// - [Dfa::from_nfa]: converts a NFA into a DFA.
///
/// ## Matching
/// - [Dfa::run]: matches input.
///
/// ## Example
/// ```rust
/// use re_finite_automata::Dfa;
///
/// // we can construct a DFA using the provided methods
/// // this matches regex `[\0-\5][\4]`.
/// let dfa = Dfa::from_range(0..=5) + Dfa::from_range(4..=4);
/// let input = [3, 4, 5, 6];
/// let mut iter = input.into_iter(); // any iterator over u8 will do
/// assert!(dfa.run(&mut iter)); // input matches
/// assert_eq!(iter.as_slice(), [5, 6]); // we can get our match from the iterator
/// ```
#[derive(Clone)]
pub struct Dfa {
    // each transition contains new states
    pub(crate) transitions: Vec<Transition>,
}

type MapItem = (Vec<RangeInclusive<u8>>, Vec<Option<BitSet>>);

impl Dfa {
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

    /// Given a state, returns its "inside" next state.
    pub fn inside(&self, state: u16) -> u16 {
        let transition = &self.transitions[state as usize];
        transition.inside
    }

    /// Given a state, returns its "outside" next state.
    pub fn outside(&self, state: u16) -> u16 {
        let transition = &self.transitions[state as usize];
        transition.outside
    }

    /// Given a state, returns whether to consume input upon transitioning to it.
    pub fn consumes(&self, state: u16) -> bool {
        let transition = &self.transitions[state as usize];
        transition.consume
    }

    /// Given a state and an input symbol, produces the next state.
    pub fn apply(&self, state: u16, input: u8) -> u16 {
        let transition = &self.transitions[state as usize];
        if (transition.min..=transition.max).contains(&input) {
            transition.inside
        } else {
            transition.outside
        }
    }

    /// Runs the state machine on some input and returns whether the input is accepted.
    pub fn run<I: Iterator<Item = u8>>(&self, input: &mut I) -> bool {
        let mut state = INITIAL_STATE;
        let mut symbol = 0;
        loop {
            let transition = &self.transitions[state as usize];
            if transition.consume {
                // TODO: avoid accessing transition twice
                symbol = match input.next() {
                    Some(x) => x,
                    None => return false,
                }
            }
            state = if (transition.min..=transition.max).contains(&symbol) {
                transition.inside
            } else {
                transition.outside
            };
            if (!state) <= 1 {
                return state == ACCEPTING_STATE;
            }
        }
    }

    /// Creates a new DFA that matches a single symbol within a range.
    pub fn from_range(range: RangeInclusive<u8>) -> Self {
        Dfa {
            transitions: vec![Transition {
                min: *range.start(),
                max: *range.end(),
                inside: ACCEPTING_STATE,
                outside: REJECTING_STATE,
                consume: true,
            }],
        }
    }

    fn iter_transitions(&mut self) -> DfaTransitionIterator<'_> {
        DfaTransitionIterator {
            dfa: self,
            state: 0,
            inside: true,
        }
    }

    fn rebase_transition_states(&mut self, offset: u16) {
        for state in self.iter_transitions() {
            if !*state > 1 {
                *state += offset;
            }
        }
    }

    fn replace_state(&mut self, old: u16, new: u16) {
        for state in self.iter_transitions() {
            if *state == old {
                *state = new;
            }
        }
    }

    /// Creates a new NFA that matches the concatenation of two NFAs.
    pub fn append(mut self, mut other: Self) -> Self {
        let toffset = self.transitions.len() as u16;
        self.replace_state(ACCEPTING_STATE, toffset);
        other.rebase_transition_states(toffset);
        self.transitions.append(&mut other.transitions);
        self
    }

    /// Creates a new DFA with opposite matching behavior.
    pub fn invert(mut self) -> Self {
        for state in self.iter_transitions() {
            if *state == ACCEPTING_STATE {
                *state = REJECTING_STATE;
            } else if *state == REJECTING_STATE {
                *state = ACCEPTING_STATE;
            }
        }
        self
    }

    /// Creates a new DFA that matches either depending on result of current DFA.
    /// Will remove first consumed input from following DFAs.
    pub fn switch(mut self, mut accept: Self, mut reject: Self) -> Self {
        let toffset = self.transitions.len() as u16;
        let toffset2 = accept.transitions.len() as u16;
        self.replace_state(ACCEPTING_STATE, toffset);
        self.replace_state(REJECTING_STATE, toffset + toffset2);
        accept.transitions[0].consume = false;
        reject.transitions[0].consume = false;
        accept.rebase_transition_states(toffset);
        reject.rebase_transition_states(toffset + toffset2);
        self.transitions.append(&mut accept.transitions);
        self.transitions.append(&mut reject.transitions);
        self
    }

    // TODO: move to module nfa
    fn apply_nfa(
        nfa: &Nfa,
        mut states_a: BitSet,
        mut states_b: BitSet,
        symbol: u8,
    ) -> (Option<BitSet>, u8) {
        let mut max = 255;
        states_b.drain();
        while let Some(state) = states_a.iter_next_remove() {
            let trans = &nfa.transitions[state as usize];
            let new_states = if symbol < trans.min {
                max = std::cmp::min(max, trans.min - 1);
                nfa.translate_state(&trans.outside)
            } else if symbol > trans.max {
                nfa.translate_state(&trans.outside)
            } else {
                max = std::cmp::min(max, trans.max);
                nfa.translate_state(&trans.inside)
            };
            for new_state in new_states {
                if (!new_state) <= 1 {
                    if *new_state == ACCEPTING_STATE {
                        return (None, max);
                    }
                } else if nfa.consumes(*new_state) {
                    states_b.insert(*new_state);
                } else {
                    states_a.insert(*new_state);
                }
            }
        }
        (Some(states_b), max)
    }

    // TODO: move to module nfa
    fn from_nfa_compute_map(nfa: Nfa) -> HashMap<BitSet, MapItem> {
        let l = nfa.transitions.len() as u16;
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
                let (next_states, max) = Self::apply_nfa(&nfa, states.clone(), states_b, symbol);
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

    fn from_nfa_build_transitions(
        trans: &mut [Option<Transition>],
        k: BitSet,
        r: MapItem,
        starting: &HashMap<BitSet, usize>,
    ) {
        let mut s = starting[&k];
        // TODO: handle more gracefully
        for n in 0..std::cmp::max(r.0.len() - 1, 1) {
            let range = &r.0[n];
            let inside_states = &r.1[n];
            let inside = match inside_states {
                Some(states) => {
                    if states.clone().is_empty() {
                        REJECTING_STATE
                    } else {
                        starting[states] as u16
                    }
                }
                None => ACCEPTING_STATE,
            };
            let outside = (s + 1) as u16;
            let t = Transition {
                min: *range.start(),
                max: *range.end(),
                inside,
                outside,
                consume: false,
            };
            trans[s] = Some(t);
            s = outside as usize;
        }
        if r.0.len() > 1 {
            let inside_states = &r.1.last().unwrap();
            trans[s - 1].as_mut().unwrap().outside = match inside_states {
                Some(states) => {
                    if states.clone().is_empty() {
                        REJECTING_STATE
                    } else {
                        starting[states] as u16
                    }
                }
                None => ACCEPTING_STATE,
            };
            if r.0.len() == 3
                && trans[s - 2].as_ref().unwrap().inside == trans[s - 1].as_ref().unwrap().outside
            {
                *trans[s - 2].as_mut().unwrap() = trans[s - 1].as_ref().unwrap().clone();
            }
        }
        trans[starting[&k]].as_mut().unwrap().consume = true;
    }

    /// Constructs a DFA from a NFA, using the [powerset construction](https://en.wikipedia.org/wiki/Powerset_construction).
    ///
    /// The resulting DFA will behave like [Nfa::run_shortest]
    /// in that it yields the shortest possible match for any input.
    /// However, it is approximately 10 times faster,
    /// and up to twice as fast as [Nfa::run].
    ///
    /// For typical NFAs, the overhead of performing the conversion
    /// with this function is worth it beyond several hundreds of bytes of input.
    /// However, specifically crafted NFAs can yield an exponential worst-case
    /// running time for the conversion.
    pub fn from_nfa(nfa: Nfa) -> Self {
        let l = nfa.transitions.len() as u16;
        // Compute a map of state sets to lists of branches
        let map = Self::from_nfa_compute_map(nfa);
        let mut starting = HashMap::new();
        let mut cur_state = 0;
        let mut states = BitSet::new_with_size(l);
        states.insert(INITIAL_STATE);
        starting.insert(states.clone(), cur_state);
        cur_state += std::cmp::max(map[&states].0.len() - 1, 1);
        // Associate each state set to a state number
        for (k, _) in map.iter() {
            if !starting.contains_key(k) {
                starting.insert(k.clone(), cur_state);
                cur_state += std::cmp::max(map[k].0.len() - 1, 1);
            }
        }
        let mut trans: Vec<_> = (0..cur_state).map(|_| None).collect();
        // Build transitions
        for (k, r) in map.into_iter() {
            Self::from_nfa_build_transitions(&mut trans, k, r, &starting);
        }
        Dfa {
            transitions: trans.into_iter().map(|x| x.unwrap()).collect(),
        }
    }
}

impl Add for Dfa {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        self.append(other)
    }
}

impl Not for Dfa {
    type Output = Self;

    fn not(self) -> Self::Output {
        self.invert()
    }
}

struct DfaTransitionIterator<'a> {
    dfa: &'a mut Dfa,
    state: u16,
    inside: bool,
}

impl<'a> Iterator for DfaTransitionIterator<'a> {
    type Item = &'a mut u16;

    fn next(&mut self) -> Option<Self::Item> {
        let trans: &'a mut Vec<Transition> = unsafe { &mut *(&mut self.dfa.transitions as *mut _) };
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

#[test]
fn dfa_run_test() {
    let dfa = Dfa {
        transitions: vec![
            Transition {
                min: 5,
                max: 5,
                inside: ACCEPTING_STATE,
                outside: 1,
                consume: true,
            },
            Transition {
                min: 6,
                max: 8,
                inside: 2,
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
        ],
    };
    assert_eq!(dfa.size(), 3);
    assert_eq!(dfa.range(1), 6..=8);
    assert_eq!(dfa.inside(1), 2);
    assert_eq!(dfa.outside(0), 1);
    assert_eq!(dfa.outside(1), REJECTING_STATE);
    assert_eq!(dfa.consumes(0), true);
    assert_eq!(dfa.apply(1, 7), 2);
    assert!(dfa.run(&mut [5, 9].into_iter()));
    assert!(dfa.run(&mut [7, 8].into_iter()));
    assert!(!dfa.run(&mut [9, 7].into_iter()));
}

#[test]
fn dfa_add_test() {
    let dfa1 = Dfa::from_range(4..=5);
    let dfa2 = Dfa::from_range(6..=6);
    let dfa = dfa1 + dfa2;
    assert!(dfa.run(&mut [4, 6].into_iter()));
    assert!(!dfa.run(&mut [4, 5].into_iter()));
    assert!(!dfa.run(&mut [6, 6].into_iter()));
}

#[test]
fn dfa_not_test() {
    let dfa1 = Dfa::from_range(4..=5);
    let dfa2 = Dfa::from_range(6..=6);
    let dfa = !(dfa1 + dfa2);
    assert!(!dfa.run(&mut [4, 6].into_iter()));
    assert!(dfa.run(&mut [4, 5].into_iter()));
    assert!(dfa.run(&mut [6, 6].into_iter()));
}

#[test]
fn dfa_compound_test() {
    let dfa0 = Dfa::from_range(0..=1);
    let dfa1 = Dfa::from_range(0..=0);
    let dfa2 = Dfa::from_range(1..=1);
    let dfa = dfa0 + (dfa1 + dfa2);
    assert!(dfa.run(&mut [0, 0, 1].into_iter()));
    assert!(!dfa.run(&mut [0, 1].into_iter()));
    assert!(dfa.run(&mut [1, 0, 1].into_iter()));
    assert!(!dfa.run(&mut [1, 0, 0].into_iter()));
    assert!(!dfa.run(&mut [0, 1, 0].into_iter()));
    assert!(!dfa.run(&mut [1, 0].into_iter()));
}

#[test]
fn dfa_switch_test() {
    let dfa0 = Dfa::from_range(0..=1);
    let dfa1 = Dfa::from_range(0..=0) + Dfa::from_range(2..=2);
    let dfa2 = Dfa::from_range(2..=2) + Dfa::from_range(0..=0);
    let dfa = dfa0.switch(dfa1, dfa2);
    assert!(dfa.run(&mut [0, 2].into_iter()));
    assert!(!dfa.run(&mut [0, 1].into_iter()));
    assert!(!dfa.run(&mut [1].into_iter()));
    assert!(dfa.run(&mut [2, 0].into_iter()));
    assert!(!dfa.run(&mut [2, 1].into_iter()));
    assert!(!dfa.run(&mut [3].into_iter()));
}

#[test]
fn dfa_nfa_run_test() {
    let nfa = Nfa {
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
    let dfa = Dfa::from_nfa(nfa);
    assert!(dfa.run(&mut [5, 9].into_iter()));
    assert!(dfa.run(&mut [7, 8].into_iter()));
    assert!(!dfa.run(&mut [9, 7].into_iter()));
    assert!(dfa.run(&mut [9, 15].into_iter()));
}

#[test]
fn dfa_nfa_add_test() {
    let nfa1 = Nfa::from_range(4..=5);
    let nfa2 = Nfa::from_range(6..=6);
    let nfa = nfa1 + nfa2;
    let dfa = Dfa::from_nfa(nfa);
    assert!(dfa.run(&mut [4, 6].into_iter()));
    assert!(!dfa.run(&mut [4, 5].into_iter()));
    assert!(!dfa.run(&mut [6, 6].into_iter()));
}

#[test]
fn dfa_nfa_not_test() {
    let nfa1 = Nfa::from_range(4..=5);
    let nfa2 = Nfa::from_range(6..=6);
    let nfa = !(nfa1 + nfa2);
    let dfa = Dfa::from_nfa(nfa);
    assert!(!dfa.run(&mut [4, 6].into_iter()));
    assert!(dfa.run(&mut [4, 5].into_iter()));
    assert!(dfa.run(&mut [6, 6].into_iter()));
}

#[test]
fn dfa_nfa_or_test() {
    let nfa1 = Nfa::from_range(4..=5);
    let nfa2 = Nfa::from_range(6..=6);
    let nfa = nfa1 | nfa2;
    let dfa = Dfa::from_nfa(nfa);
    assert!(dfa.run(&mut [4].into_iter()));
    assert!(dfa.run(&mut [6].into_iter()));
    assert!(!dfa.run(&mut [7].into_iter()));
}

#[test]
fn dfa_nfa_compound_test() {
    let nfa0 = Nfa::from_range(0..=0);
    let nfa1 = Nfa::from_range(1..=1);
    let nfa = ((nfa0.clone() | nfa1.clone()) + nfa0.clone() + nfa1.clone()) | (nfa0 + nfa1);
    let dfa = Dfa::from_nfa(nfa);
    assert!(dfa.run(&mut [0, 0, 1].into_iter()));
    assert!(dfa.run(&mut [0, 1].into_iter()));
    assert!(dfa.run(&mut [1, 0, 1].into_iter()));
    assert!(!dfa.run(&mut [1, 0, 0].into_iter()));
    assert!(dfa.run(&mut [0, 1, 0].into_iter()));
    assert!(!dfa.run(&mut [1, 0].into_iter()));
}

#[test]
fn dfa_nfa_switch_test() {
    let nfa0 = Nfa::from_range(0..=1);
    let nfa1 = Nfa::from_range(0..=0) + Nfa::from_range(2..=2);
    let nfa2 = Nfa::from_range(2..=2) + Nfa::from_range(0..=0);
    let nfa = nfa0.switch(nfa1, nfa2);
    let dfa = Dfa::from_nfa(nfa);
    assert!(dfa.run(&mut [0, 2].into_iter()));
    assert!(!dfa.run(&mut [0, 1].into_iter()));
    assert!(!dfa.run(&mut [1].into_iter()));
    assert!(dfa.run(&mut [2, 0].into_iter()));
    assert!(!dfa.run(&mut [2, 1].into_iter()));
    assert!(!dfa.run(&mut [3].into_iter()));
}

#[test]
fn dfa_nfa_out_test() {
    let nfa = Nfa::from_range(0..=0) + Nfa::from_range(0..=0);
    let dfa = Dfa::from_nfa(nfa);
    assert!(!dfa.run(&mut [0].into_iter()));
}

#[test]
fn dfa_nfa_repeat_greedy_test() {
    let nfa0 = Nfa::from_range(0..=0);
    let nfa1 = Nfa::from_range(1..=1);
    let nfa = nfa0 + nfa1.clone().repeat_greedy() + nfa1;
    let dfa = Dfa::from_nfa(nfa);
    assert!(dfa.run(&mut [0, 1].into_iter()));
    assert!(dfa.run(&mut [0, 1, 1].into_iter()));
    assert!(dfa.run(&mut [0, 1, 0, 1].into_iter()));
    assert!(!dfa.run(&mut [0, 0, 1].into_iter()));
}

#[test]
fn dfa_nfa_repeat_lazy_test() {
    let nfa0 = Nfa::from_range(0..=0);
    let nfa1 = Nfa::from_range(1..=1);
    let nfa = nfa0 + nfa1.clone().repeat_greedy() + nfa1;
    let dfa = Dfa::from_nfa(nfa);
    assert!(dfa.run(&mut [0, 1].into_iter()));
    assert!(dfa.run(&mut [0, 1, 1].into_iter()));
    assert!(dfa.run(&mut [0, 1, 0, 1].into_iter()));
    assert!(!dfa.run(&mut [0, 0, 1].into_iter()));
}
