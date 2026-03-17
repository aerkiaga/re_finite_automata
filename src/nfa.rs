use crate::*;

/// A nondeterministic finite-state automaton.
#[derive(Clone)]
pub struct Nfa {
    // each transition contains states or indices into the state table
    transitions: Vec<Transition>,
    // first element in each state list is size
    states: Vec<u16>,
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
    fn translate_state<'a>(&'a self, state: &'a u16) -> &'a [u16] {
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
    pub fn run<I: Iterator<Item = u8>>(&self, input: I) -> bool {
        let l = self.transitions.len() as u16;
        let mut states_a = BitSet::new_with_size(l);
        let mut states_b = BitSet::new_with_size(l);
        let mut states_c = BitSet::new_with_size(l);
        states_a.insert(INITIAL_STATE);
        for symbol in input {
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
                }
                states_c.insert(state);
            }
            std::mem::swap(&mut states_a, &mut states_b);
            states_c.drain();
        }
        false
    }

    /// Creates a new NFA that matches a single symbol within a range.
    pub fn from_range(range: RangeInclusive<u8>) -> Self {
        Nfa {
            transitions: vec![Transition {
                min: *range.start(),
                max: *range.end(),
                inside: ACCEPTING_STATE,
                outside: REJECTING_STATE,
                consume: true,
            }],
            states: vec![],
        }
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
    pub fn append(&mut self, mut other: Self) {
        let toffset = self.transitions.len() as u16;
        let soffset = self.states.len() as u16;
        self.replace_state(ACCEPTING_STATE, toffset);
        other.rebase_transition_states(toffset);
        other.rebase_states_array(soffset);
        self.transitions.append(&mut other.transitions);
        self.states.append(&mut other.states);
    }

    /// Creates a new NFA that matches either of two NFAs.
    pub fn combine(&mut self, mut other: Self) {
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
        *self = r;
    }

    /// Creates a new NFA with opposite matching behavior.
    pub fn invert(&mut self) {
        for state in self.iter_states() {
            if *state == ACCEPTING_STATE {
                *state = REJECTING_STATE;
            } else if *state == REJECTING_STATE {
                *state = ACCEPTING_STATE;
            }
        }
    }
}

impl Add for Nfa {
    type Output = Self;

    fn add(mut self, other: Self) -> Self::Output {
        self.append(other);
        self
    }
}

impl BitOr for Nfa {
    type Output = Self;

    fn bitor(mut self, other: Self) -> Self::Output {
        self.combine(other);
        self
    }
}

impl Not for Nfa {
    type Output = Self;

    fn not(mut self) -> Self::Output {
        self.invert();
        self
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
    assert!(nfa.run([5, 9].into_iter()));
    assert!(nfa.run([7, 8].into_iter()));
    assert!(!nfa.run([9, 7].into_iter()));
    assert!(nfa.run([9, 15].into_iter()));
}

#[test]
fn nfa_add_test() {
    let nfa1 = Nfa::from_range(4..=5);
    let nfa2 = Nfa::from_range(6..=6);
    let nfa = nfa1 + nfa2;
    assert!(nfa.run([4, 6].into_iter()));
    assert!(!nfa.run([4, 5].into_iter()));
    assert!(!nfa.run([6, 6].into_iter()));
}

#[test]
fn nfa_not_test() {
    let nfa1 = Nfa::from_range(4..=5);
    let nfa2 = Nfa::from_range(6..=6);
    let nfa = !(nfa1 + nfa2);
    assert!(!nfa.run([4, 6].into_iter()));
    assert!(nfa.run([4, 5].into_iter()));
    assert!(nfa.run([6, 6].into_iter()));
}

#[test]
fn nfa_or_test() {
    let nfa1 = Nfa::from_range(4..=5);
    let nfa2 = Nfa::from_range(6..=6);
    let nfa = nfa1 | nfa2;
    assert!(nfa.run([4].into_iter()));
    assert!(nfa.run([6].into_iter()));
    assert!(!nfa.run([7].into_iter()));
}

#[test]
fn nfa_compound_test() {
    let nfa0 = Nfa::from_range(0..=0);
    let nfa1 = Nfa::from_range(1..=1);
    let nfa = ((nfa0.clone() | nfa1.clone()) + nfa0.clone() + nfa1.clone()) | (nfa0 + nfa1);
    assert!(nfa.run([0, 0, 1].into_iter()));
    assert!(nfa.run([0, 1].into_iter()));
    assert!(nfa.run([1, 0, 1].into_iter()));
    assert!(!nfa.run([1, 0, 0].into_iter()));
    assert!(nfa.run([0, 1, 0].into_iter()));
    assert!(!nfa.run([1, 0].into_iter()));
}
