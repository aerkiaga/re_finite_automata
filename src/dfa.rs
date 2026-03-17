use crate::*;

/// A deterministic finite-state automaton.
#[derive(Clone)]
pub struct Dfa {
    // each transition contains new states
    transitions: Vec<Transition>,
}

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
    pub fn run<I: Iterator<Item = u8>>(&self, mut input: I) -> bool {
        let mut state = INITIAL_STATE;
        let mut symbol = 0;
        loop {
            if self.consumes(state) {
                symbol = match input.next() {
                    Some(x) => x,
                    None => return false,
                }
            }
            state = self.apply(state, symbol);
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
    pub fn append(&mut self, mut other: Self) {
        let toffset = self.transitions.len() as u16;
        self.replace_state(ACCEPTING_STATE, toffset);
        other.rebase_transition_states(toffset);
        self.transitions.append(&mut other.transitions);
    }

    /// Creates a new DFA with opposite matching behavior.
    pub fn invert(&mut self) {
        for state in self.iter_transitions() {
            if *state == ACCEPTING_STATE {
                *state = REJECTING_STATE;
            } else if *state == REJECTING_STATE {
                *state = ACCEPTING_STATE;
            }
        }
    }
}

impl Add for Dfa {
    type Output = Self;

    fn add(mut self, other: Self) -> Self::Output {
        self.append(other);
        self
    }
}

impl Not for Dfa {
    type Output = Self;

    fn not(mut self) -> Self::Output {
        self.invert();
        self
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
    assert!(dfa.run([5, 9].into_iter()));
    assert!(dfa.run([7, 8].into_iter()));
    assert!(!dfa.run([9, 7].into_iter()));
}

#[test]
fn dfa_add_test() {
    let dfa1 = Dfa::from_range(4..=5);
    let dfa2 = Dfa::from_range(6..=6);
    let dfa = dfa1 + dfa2;
    assert!(dfa.run([4, 6].into_iter()));
    assert!(!dfa.run([4, 5].into_iter()));
    assert!(!dfa.run([6, 6].into_iter()));
}

#[test]
fn dfa_not_test() {
    let dfa1 = Dfa::from_range(4..=5);
    let dfa2 = Dfa::from_range(6..=6);
    let dfa = !(dfa1 + dfa2);
    assert!(!dfa.run([4, 6].into_iter()));
    assert!(dfa.run([4, 5].into_iter()));
    assert!(dfa.run([6, 6].into_iter()));
}

#[test]
fn dfa_compound_test() {
    let dfa0 = Dfa::from_range(0..=1);
    let dfa1 = Dfa::from_range(0..=0);
    let dfa2 = Dfa::from_range(1..=1);
    let dfa = dfa0 + (dfa1 + dfa2);
    assert!(dfa.run([0, 0, 1].into_iter()));
    assert!(!dfa.run([0, 1].into_iter()));
    assert!(dfa.run([1, 0, 1].into_iter()));
    assert!(!dfa.run([1, 0, 0].into_iter()));
    assert!(!dfa.run([0, 1, 0].into_iter()));
    assert!(!dfa.run([1, 0].into_iter()));
}
