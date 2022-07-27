
/// An array of bits
#[derive(Debug, Clone)]
pub struct BitSet {
    /// The buffer
    pub longs: Vec<u64>,
}

impl BitSet {
    /// Creates a new 
    pub fn new() -> Self {
        Self {
            longs: Vec::new(),
        }
    }

    /// Sets the bit at the `nth` position to the given value,
    /// allocates more space if needed
    pub fn set_bit(&mut self, nth: usize, value: bool) {
        let idx = nth / 64;
        let offset = nth % 64;
        
        while idx >= self.longs.len() {
            self.longs.push(0);
        }

        if value {
            self.longs[idx] |= 1 << offset;
        }
        else {
            self.longs[idx] &= !(1 << offset);
        }
    }

    /// Get the bit at the given position
    pub fn get_bit(&self, nth: usize) -> bool {
        let idx = nth / 64;
        let offset = nth % 64;

        if idx >= self.longs.len() {
            return false;
        }

        (self.longs[idx] & 1 << offset) == 0
    }

    /// Removes any useless longs in the buffer
    pub fn compactify(&mut self) {
        while self.longs.last() == Some(&0) {
            self.longs.pop();
        }
    }
}

impl Default for BitSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::BitSet;

    #[test]
    fn bitset_test() {
        let mut bs = BitSet::new();

        assert_eq!(bs.get_bit(0), false);
        assert_eq!(bs.get_bit(1), false);
        assert_eq!(bs.longs.len(), 0);

        bs.set_bit(0, true);

        assert_eq!(bs.get_bit(0), true);
        assert_eq!(bs.get_bit(1), false);
        assert_eq!(bs.longs.as_slice(), &[1]);

        bs.set_bit(0, false);

        assert_eq!(bs.get_bit(0), false);
        assert_eq!(bs.get_bit(1), false);
        assert_eq!(bs.longs.as_slice(), &[0]);

        bs.compactify();

        assert_eq!(bs.get_bit(0), false);
        assert_eq!(bs.get_bit(1), false);
        assert_eq!(bs.longs.len(), 0);

        bs.set_bit(1, true);
        bs.set_bit(2, true);

        assert_eq!(bs.get_bit(0), false);
        assert_eq!(bs.get_bit(1), true);
        assert_eq!(bs.get_bit(2), true);
        assert_eq!(bs.get_bit(3), false);
        assert_eq!(bs.longs.as_slice(), &[0b110]);

        bs.set_bit(2, false);
        bs.set_bit(3, true);

        assert_eq!(bs.get_bit(0), false);
        assert_eq!(bs.get_bit(1), true);
        assert_eq!(bs.get_bit(2), false);
        assert_eq!(bs.get_bit(3), true);
        assert_eq!(bs.longs.as_slice(), &[0b1010]);

        (0..100)
            .for_each(|i| bs.set_bit(i, i % 4 == 0));

        assert_eq!(bs.longs.len(), 2);

        (0..300)
            .for_each(|i| bs.set_bit(i, i % 4 == 0));

        assert_eq!(bs.longs.len(), 5);
        assert!(
            (0..300).all(|i| bs.get_bit(i) == (i % 4 == 0))
        );

        (0..300)
            .for_each(|i| bs.set_bit(i, false));

        assert_eq!(bs.longs.len(), 5);
        assert!(
            (0..300).all(|i| !bs.get_bit(i))
        );

        bs.compactify();

        assert_eq!(bs.longs.len(), 0);
    }
}
