
#[derive(Clone)]
pub struct BitBuffer {
    bits_per_entry: u64,
    entries_per_long: u64,
    entries: usize,
    mask: u64,
    longs: Vec<u64>,
}

impl BitBuffer {
    pub fn create(bits_per_entry: u8, entries: usize) -> BitBuffer {
        let entries_per_long = 64 / bits_per_entry as u64;
        // Rounding up div
        let longs_len = (entries + entries_per_long as usize - 1) / entries_per_long as usize;
        let longs = vec![0; longs_len];
        BitBuffer {
            bits_per_entry: bits_per_entry as u64,
            longs,
            entries,
            entries_per_long,
            mask: (1 << bits_per_entry) - 1,
        }
    }

    fn load(entries: usize, bits_per_entry: u8, longs: Vec<u64>) -> BitBuffer {
        let entries_per_long = 64 / bits_per_entry as u64;
        BitBuffer {
            bits_per_entry: bits_per_entry as u64,
            longs,
            entries,
            entries_per_long,
            mask: (1 << bits_per_entry) - 1,
        }
    }

    pub fn get_entry(&self, word_idx: usize) -> u32 {
        // Find the set of indices.
        let arr_idx = word_idx / self.entries_per_long as usize;
        let sub_idx =
            (word_idx as u64 - arr_idx as u64 * self.entries_per_long) * self.bits_per_entry;
        // Find the word.
        let word = (self.longs[arr_idx] >> sub_idx) & self.mask;
        word as u32
    }

    pub fn set_entry(&mut self, word_idx: usize, word: u32) {
        // Find the set of indices.
        let arr_idx = word_idx / self.entries_per_long as usize;
        let sub_idx =
            (word_idx as u64 - arr_idx as u64 * self.entries_per_long) * self.bits_per_entry;
        // Set the word.
        let mask = !(self.mask << sub_idx);
        self.longs[arr_idx] = (self.longs[arr_idx] & mask) | ((word as u64) << sub_idx);
    }
}