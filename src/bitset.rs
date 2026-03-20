use smallvec::SmallVec;

/// An optimized set of u16 using a bitmap.
#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct BitSet {
    buckets: SmallVec<[u64; 1]>,
    lowest: u16,
}

impl BitSet {
    pub(crate) fn new_with_size(size: u16) -> Self {
        Self {
            buckets: (0..size.div_ceil(64)).map(|_| 0).collect(),
            lowest: size + 63,
        }
    }

    pub(crate) fn drain(&mut self) {
        for element in self.buckets.iter_mut() {
            *element = 0;
        }
    }

    pub(crate) fn insert(&mut self, value: u16) {
        if value < self.lowest {
            self.lowest = value;
        }
        let index = value / 64;
        let bit = value % 64;
        self.buckets[index as usize] |= 1 << bit;
    }

    pub(crate) fn contains(&mut self, value: u16) -> bool {
        let index = value / 64;
        let bit = value % 64;
        self.buckets[index as usize] & (1 << bit) != 0
    }

    pub(crate) fn iter_next_remove(&mut self) -> Option<u16> {
        let index = self.lowest / 64;
        if index as usize >= self.buckets.len() {
            return None;
        }
        let bit = self.lowest % 64;
        if self.buckets[index as usize] & (1 << bit) != 0 {
            self.buckets[index as usize] &= !(1 << bit);
            return Some(self.lowest);
        }
        for index in index..self.buckets.len() as u16 {
            if let Some(bit) = self.buckets[index as usize].lowest_one() {
                let r = index * 64 + bit as u16;
                self.lowest = r;
                self.buckets[index as usize] &= !(1 << bit);
                return Some(r);
            }
        }
        self.lowest = self.buckets.len() as u16 * 64;
        None
    }

    pub(crate) fn is_empty(&mut self) -> bool {
        let index = self.lowest / 64;
        if index as usize >= self.buckets.len() {
            return true;
        }
        let bit = self.lowest % 64;
        if self.buckets[index as usize] & (1 << bit) != 0 {
            return false;
        }
        for index in index..self.buckets.len() as u16 {
            if self.buckets[index as usize] != 0 {
                self.lowest = index * 64;
                return false;
            }
        }
        self.lowest = self.buckets.len() as u16 * 64;
        true
    }
}

#[test]
fn bitset_test() {
    let mut set = BitSet::new_with_size(200);
    assert!(set.is_empty());
    set.insert(140);
    assert!(!set.is_empty());
    assert!((0..200).all(|n| set.contains(n) == (n == 140)));
    set.insert(150);
    assert!(!set.is_empty());
    assert!((0..200).all(|n| set.contains(n) == (n == 140 || n == 150)));
    set.insert(40);
    assert!(!set.is_empty());
    assert!((0..200).all(|n| set.contains(n) == (n == 40 || n == 140 || n == 150)));
    assert_eq!(set.iter_next_remove(), Some(40));
    assert!(!set.is_empty());
    assert!((0..200).all(|n| set.contains(n) == (n == 140 || n == 150)));
    assert_eq!(set.iter_next_remove(), Some(140));
    assert!(!set.is_empty());
    assert!((0..200).all(|n| set.contains(n) == (n == 150)));
    assert_eq!(set.iter_next_remove(), Some(150));
    assert!(set.is_empty());
    assert_eq!(set.iter_next_remove(), None);
    assert!(set.is_empty());
}

#[test]
fn bitset_drain_test() {
    let mut set = BitSet::new_with_size(200);
    assert!(set.is_empty());
    set.insert(140);
    assert!(!set.is_empty());
    assert!((0..200).all(|n| set.contains(n) == (n == 140)));
    set.insert(150);
    assert!(!set.is_empty());
    assert!((0..200).all(|n| set.contains(n) == (n == 140 || n == 150)));
    set.insert(40);
    assert!(!set.is_empty());
    set.drain();
    assert!(set.is_empty());
}

#[test]
fn bitset_all_test() {
    let mut set = BitSet::new_with_size(64 * 100);
    assert!(set.is_empty());
    for n in 0..64 * 100 {
        set.insert(n);
        assert!(set.contains(n));
        assert!(!set.is_empty());
    }
    for n in 0..64 * 100 {
        assert!(!set.is_empty());
        assert_eq!(set.iter_next_remove(), Some(n));
    }
    assert!(set.is_empty());
}
