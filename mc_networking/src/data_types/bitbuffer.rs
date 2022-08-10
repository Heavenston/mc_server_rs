#[derive(Clone, Debug)]
pub struct BitBuffer {
    entry_bit_size: u64,
    entries_per_long: u64,
    mask: u64,
    longs: Vec<i64>,
}

impl BitBuffer {
    pub fn create(entry_bit_size: u8, size: usize) -> BitBuffer {
        let entries_per_long = 64 / entry_bit_size as u64;
        // Rounding up div
        let longs_len = (size + entries_per_long as usize - 1) / entries_per_long as usize;
        let longs = vec![0; longs_len];
        BitBuffer {
            entry_bit_size: entry_bit_size as u64,
            longs,
            entries_per_long,
            mask: (1 << entry_bit_size) - 1,
        }
    }

    pub fn load(bits_per_entry: u8, longs: Vec<i64>) -> BitBuffer {
        let entries_per_long = 64 / bits_per_entry as u64;
        BitBuffer {
            entry_bit_size: bits_per_entry as u64,
            longs,
            entries_per_long,
            mask: (1 << bits_per_entry) - 1,
        }
    }

    pub fn get_entry(&self, word_idx: usize) -> u32 {
        // Find the set of indices.
        let arr_idx = word_idx / self.entries_per_long as usize;
        let sub_idx =
            (word_idx as u64 - arr_idx as u64 * self.entries_per_long) * self.entry_bit_size;
        // Find the word.
        let word = ((self.longs[arr_idx] as u64) >> sub_idx) & self.mask;
        word as u32
    }

    pub fn set_entry(&mut self, word_idx: usize, word: u32) {
        // Find the set of indices.
        let arr_idx = word_idx / self.entries_per_long as usize;
        let sub_idx =
            (word_idx as u64 - arr_idx as u64 * self.entries_per_long) * self.entry_bit_size;
        // Set the word.
        let mask = !(self.mask << sub_idx);
        self.longs[arr_idx] =
            (((self.longs[arr_idx] as u64) & mask) | ((word as u64) << sub_idx)) as i64;
    }

    pub fn into_buffer(self) -> Vec<i64> {
        self.longs
    }
}

#[cfg(test)]
mod test {
    use super::BitBuffer;

    #[test]
    fn test_bit_buffer() {
        let bit_size = 3;
        let entries = 30u32;
        let modulo = 2u32.pow(bit_size as u32);

        let mut bf = BitBuffer::create(bit_size, entries as usize);
        for (i, v) in (0..entries).enumerate() {
            bf.set_entry(i, v % modulo);
        }
        assert_eq!(bf.get_entry(0), 0);
        assert_eq!(bf.get_entry(1), 1);
        assert!((0..entries).enumerate().all(|(i, v)| bf.get_entry(i) == v % modulo));
    }
}
