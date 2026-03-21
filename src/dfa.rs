use crate::*;

use std::collections::HashMap;
use std::iter::Iterator;
use std::ops::{Add, Not, RangeInclusive};

/// A deterministic finite-state automaton.
///
/// ## Constructing
/// - [Dfa::from_range]: DFA that matches a single symbol in a range.
/// - [Dfa::from_ranges]: DFA that matches a single symbol in any of a list of ranges.
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

pub(crate) type SwitchTable<T> = (Vec<RangeInclusive<u8>>, Vec<T>);

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

    // TODO: move into SwitchTable
    fn process_switch_table<T: std::cmp::Eq>(r: &mut SwitchTable<T>) {
        // fill spaces
        for n in 0..=r.0.len() {
            let below = match n {
                0 => 0,
                _ => {
                    if *r.0[n - 1].end() == 255 {
                        continue;
                    } else {
                        r.0[n - 1].end() + 1
                    }
                }
            };
            let above = if n == r.0.len() {
                255
            } else if *r.0[n].start() == 0 {
                continue;
            } else {
                r.0[n].start() - 1
            };
            if below <= above {
                if n == r.0.len() {
                    r.0[n - 1] = *r.0[n - 1].start()..=255;
                } else {
                    r.0[n] = below..=*r.0[n].end();
                }
            }
        }
        // merge equal entries
        let mut n = 0;
        while n < r.0.len() - 1 {
            if r.1[n] == r.1[n + 1] {
                r.0[n] = *r.0[n].start()..=*r.0[n + 1].end();
                r.0.remove(n + 1);
                r.1.remove(n + 1);
            } else {
                n += 1;
            }
        }
    }

    // TODO: move into SwitchTable
    fn fill_switch_table<T: Clone + std::cmp::Eq>(r: &mut SwitchTable<T>, def: &T) {
        // fill spaces
        let mut n = 0;
        let mut last_start = 0;
        while n < r.0.len() {
            if *r.0[n].start() > last_start {
                r.0.insert(n, last_start..=r.0[n].start() - 1);
                r.1.insert(n, def.clone());
                n += 1;
            }
            last_start = r.0[n].end() + 1;
            n += 1;
        }
        if *r.0[n - 1].end() < 255 {
            r.0.push(r.0[n - 1].end() + 1..=255);
            r.1.push(def.clone());
        }
    }

    fn from_nfa_build_transitions_rec(
        trans: &mut [Option<Transition>],
        start: u16,
        mut r: SwitchTable<u16>,
    ) -> Option<u16> {
        Self::process_switch_table(&mut r);
        let l = r.0.len();
        if l == 1 {
            return Some(r.1[0]);
        }
        let mut best_range = 0..l;
        // try to find a range that leaves the same item on both sides
        for start in 1..=1 {
            for end in l / 2..=std::cmp::min(l / 2 + 1, l - 1) {
                if r.1[start - 1] == r.1[end] && end > start {
                    best_range = start..end;
                }
            }
        }
        // otherwise just choose anything
        if best_range == (0..l) {
            best_range = 0..l / 2;
        }
        // create switch tables
        let mut switch_inside = (vec![], vec![]);
        let mut switch_outside = (vec![], vec![]);
        for n in 0..l {
            if best_range.contains(&n) {
                switch_inside.0.push(r.0[n].clone());
                switch_inside.1.push(r.1[n]);
            } else {
                switch_outside.0.push(r.0[n].clone());
                switch_outside.1.push(r.1[n]);
            }
        }
        // estimate starting positions
        let inside = start + 1;
        let outside = start + switch_inside.0.len() as u16;
        // create transition
        let range = r.0[best_range.start].start()..=r.0[best_range.end - 1].end();
        let mut t = Transition {
            min: **range.start(),
            max: **range.end(),
            inside,
            outside,
            consume: false,
        };
        // call recursively
        if let Some(x) = Self::from_nfa_build_transitions_rec(trans, inside, switch_inside) {
            t.inside = x;
        }
        if let Some(x) = Self::from_nfa_build_transitions_rec(trans, outside, switch_outside) {
            t.outside = x;
        }
        trans[start as usize] = Some(t);
        None
    }

    fn from_nfa_build_transitions(
        trans: &mut [Option<Transition>],
        start: u16,
        r: SwitchTable<u16>,
    ) {
        if let Some(x) = Self::from_nfa_build_transitions_rec(trans, start, r) {
            trans[start as usize] = Some(Transition {
                min: 0,
                max: 255,
                inside: x,
                outside: REJECTING_STATE,
                consume: true,
            });
        } else {
            trans[start as usize].as_mut().unwrap().consume = true;
        }
    }

    /// Constructs a DFA from a NFA, using the [powerset construction](https://en.wikipedia.org/wiki/Powerset_construction).
    ///
    /// The resulting DFA will behave like [Nfa::run_shortest]
    /// in that it yields the shortest possible match for any input.
    /// However, it is approximately 10 times faster,
    /// and up to twice as fast as [Nfa::run].
    ///
    /// For typical NFAs, the overhead of performing the conversion
    /// with this function is worth it beyond some hundreds of bytes of input.
    /// However, specifically crafted NFAs can yield an exponential worst-case
    /// running time for the conversion.
    pub fn from_nfa(nfa: Nfa) -> Self {
        let l = nfa.transitions.len() as u16;
        // Compute a map of state sets to lists of branches
        let map = nfa.compute_powerset_map();
        let mut starting = HashMap::new();
        let mut cur_state = 0;
        let mut states = BitSet::new_with_size(l);
        starting.insert(states.clone(), REJECTING_STATE);
        states.insert(INITIAL_STATE);
        starting.insert(states.clone(), cur_state);
        cur_state += std::cmp::max(map[&states].0.len() - 1, 1) as u16;
        // Associate each state set to a state number
        for (k, _) in map.iter() {
            if !starting.contains_key(k) {
                starting.insert(k.clone(), cur_state);
                cur_state += std::cmp::max(map[k].0.len() - 1, 1) as u16;
            }
        }
        // Apply new states
        let mut map2 = HashMap::new();
        for (k, r) in map.into_iter() {
            let r2 = (
                r.0,
                r.1.into_iter()
                    .map(|x| match x {
                        Some(x) => starting[&x],
                        None => ACCEPTING_STATE,
                    })
                    .collect::<Vec<_>>(),
            );
            map2.insert(k, r2);
        }
        let mut trans: Vec<_> = (0..cur_state).map(|_| None).collect();
        // Build transitions
        for (k, r) in map2.into_iter() {
            Self::from_nfa_build_transitions(&mut trans, starting[&k], r);
        }
        // TODO: compress transitions, removing None values
        Self {
            transitions: trans
                .into_iter()
                .map(|x| {
                    x.unwrap_or(Transition {
                        min: 0,
                        max: 255,
                        inside: REJECTING_STATE,
                        outside: REJECTING_STATE,
                        consume: false,
                    })
                })
                .collect(),
        }
    }

    /// Similar to [Dfa::from_range], but matches any of a number of ranges.
    pub fn from_ranges<I: Iterator<Item = RangeInclusive<u8>>>(ranges: &mut I) -> Self {
        let ranges: Vec<_> = ranges.into_iter().collect();
        let l = ranges.len();
        let mut switch = (ranges, (0..l).map(|_| ACCEPTING_STATE).collect());
        Self::fill_switch_table(&mut switch, &REJECTING_STATE);
        Self::process_switch_table(&mut switch);
        let l = (switch.0.len() - 1) as u16;
        let mut transitions: Vec<_> = (0..l).map(|_| None).collect();
        Self::from_nfa_build_transitions(&mut transitions, 0, switch);
        // TODO: compress transitions, removing None values
        Self {
            transitions: transitions
                .into_iter()
                .map(|x| {
                    x.unwrap_or(Transition {
                        min: 0,
                        max: 255,
                        inside: REJECTING_STATE,
                        outside: REJECTING_STATE,
                        consume: false,
                    })
                })
                .collect(),
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
fn dfa_ranges_test() {
    let dfa = Dfa::from_ranges(&mut [2..=2, 4..=5].into_iter());
    assert!(!dfa.run(&mut [0].into_iter()));
    assert!(dfa.run(&mut [2].into_iter()));
    assert!(!dfa.run(&mut [3].into_iter()));
    assert!(dfa.run(&mut [4].into_iter()));
    assert!(!dfa.run(&mut [6].into_iter()));
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
