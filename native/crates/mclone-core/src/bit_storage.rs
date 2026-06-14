#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitStorage {
    bits: u8,
    size: usize,
    values_per_word: usize,
    mask: u64,
    data: Vec<u64>,
}

impl BitStorage {
    pub fn new(bits: u8, size: usize) -> Self {
        let values_per_word = validate_bits(bits);
        let word_count = size.div_ceil(values_per_word);
        Self {
            bits,
            size,
            values_per_word,
            mask: (1_u64 << bits) - 1,
            data: vec![0; word_count],
        }
    }

    pub fn from_raw(bits: u8, size: usize, data: Vec<u64>) -> Self {
        let values_per_word = validate_bits(bits);
        let expected_word_count = size.div_ceil(values_per_word);
        assert_eq!(
            data.len(),
            expected_word_count,
            "bit-storage data length {} != expected {expected_word_count} for bits={bits}, size={size}",
            data.len()
        );
        Self {
            bits,
            size,
            values_per_word,
            mask: (1_u64 << bits) - 1,
            data,
        }
    }

    pub const fn bits(&self) -> u8 {
        self.bits
    }

    pub const fn size(&self) -> usize {
        self.size
    }

    pub fn raw(&self) -> &[u64] {
        &self.data
    }

    pub fn into_raw(self) -> Vec<u64> {
        self.data
    }

    pub fn get(&self, index: usize) -> u32 {
        self.validate_index(index);
        let word_index = index / self.values_per_word;
        let bit_index = (index - word_index * self.values_per_word) * self.bits as usize;
        ((self.data[word_index] >> bit_index) & self.mask) as u32
    }

    pub fn set(&mut self, index: usize, value: u32) {
        self.validate_index(index);
        self.validate_value(value);
        let word_index = index / self.values_per_word;
        let bit_index = (index - word_index * self.values_per_word) * self.bits as usize;
        let cleared = self.data[word_index] & !(self.mask << bit_index);
        self.data[word_index] = cleared | ((value as u64 & self.mask) << bit_index);
    }

    pub fn get_and_set(&mut self, index: usize, value: u32) -> u32 {
        let previous = self.get(index);
        self.set(index, value);
        previous
    }

    pub fn get_all(&self) -> Vec<u32> {
        let mut values = Vec::with_capacity(self.size);
        for word in &self.data {
            let mut remaining = *word;
            for _ in 0..self.values_per_word {
                if values.len() >= self.size {
                    return values;
                }
                values.push((remaining & self.mask) as u32);
                remaining >>= self.bits;
            }
        }
        values
    }

    fn validate_index(&self, index: usize) {
        assert!(
            index < self.size,
            "bit-storage index {index} out of bounds for size {}",
            self.size
        );
    }

    fn validate_value(&self, value: u32) {
        assert!(
            value as u64 <= self.mask,
            "bit-storage value {value} out of bounds for {} bits",
            self.bits
        );
    }
}

pub fn palette_bits_for(palette_size: usize) -> u8 {
    assert!(
        palette_size >= 1,
        "palette_bits_for requires a positive palette size, got {palette_size}"
    );
    if palette_size <= 1 {
        return 1;
    }
    (usize::BITS - (palette_size - 1).leading_zeros()) as u8
}

pub fn local_palette_bits_for(palette_size: usize) -> u8 {
    palette_bits_for(palette_size).max(4)
}

fn validate_bits(bits: u8) -> usize {
    assert!(
        (1..=32).contains(&bits),
        "BitStorage bits must be in [1, 32], got {bits}"
    );
    64 / bits as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_palette_bits() {
        assert_eq!(palette_bits_for(1), 1);
        assert_eq!(palette_bits_for(2), 1);
        assert_eq!(palette_bits_for(3), 2);
        assert_eq!(palette_bits_for(4), 2);
        assert_eq!(palette_bits_for(5), 3);
        assert_eq!(local_palette_bits_for(1), 4);
        assert_eq!(local_palette_bits_for(16), 4);
        assert_eq!(local_palette_bits_for(17), 5);
    }

    #[test]
    fn bit_storage_roundtrips_4096_entries() {
        let mut storage = BitStorage::new(5, 4096);
        for index in 0..storage.size() {
            storage.set(index, (index % 17) as u32);
        }

        assert_eq!(storage.get(0), 0);
        assert_eq!(storage.get(1), 1);
        assert_eq!(storage.get(16), 16);
        assert_eq!(storage.get(17), 0);

        let raw = storage.raw().to_vec();
        let restored = BitStorage::from_raw(5, 4096, raw);
        assert_eq!(restored.get_all(), storage.get_all());
    }
}
