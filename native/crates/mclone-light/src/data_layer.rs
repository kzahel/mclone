use std::fmt;

use mclone_core::LIGHT_DATA_LAYER_BYTE_COUNT;

pub const DATA_LAYER_SIZE: usize = LIGHT_DATA_LAYER_BYTE_COUNT;
pub const DATA_LAYER_VALUE_COUNT: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataLayer {
    data: Option<Box<[u8; DATA_LAYER_SIZE]>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataLayerError {
    InvalidByteLength { actual: usize },
}

impl fmt::Display for DataLayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidByteLength { actual } => write!(
                f,
                "DataLayer should be {DATA_LAYER_SIZE} bytes, got {actual}"
            ),
        }
    }
}

impl std::error::Error for DataLayerError {}

impl Default for DataLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl DataLayer {
    pub fn new() -> Self {
        Self { data: None }
    }

    pub fn from_bytes(bytes: [u8; DATA_LAYER_SIZE]) -> Self {
        Self {
            data: Some(Box::new(bytes)),
        }
    }

    pub fn from_vec(bytes: Vec<u8>) -> Result<Self, DataLayerError> {
        let actual = bytes.len();
        let bytes: [u8; DATA_LAYER_SIZE] = bytes
            .try_into()
            .map_err(|_| DataLayerError::InvalidByteLength { actual })?;
        Ok(Self::from_bytes(bytes))
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        self.get_index(data_layer_index(x, y, z))
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, value: u8) {
        self.set_index(data_layer_index(x, y, z), value);
    }

    pub fn get_index(&self, index: usize) -> u8 {
        assert!(
            index < DATA_LAYER_VALUE_COUNT,
            "DataLayer index {index} out of bounds"
        );
        let Some(data) = &self.data else {
            return 0;
        };
        let byte = data[index >> 1];
        let shift = 4 * (index & 1);
        (byte >> shift) & 15
    }

    pub fn set_index(&mut self, index: usize, value: u8) {
        assert!(
            index < DATA_LAYER_VALUE_COUNT,
            "DataLayer index {index} out of bounds"
        );
        let data = self
            .data
            .get_or_insert_with(|| Box::new([0; DATA_LAYER_SIZE]));
        let byte_index = index >> 1;
        let shift = 4 * (index & 1);
        let mask = !(15 << shift);
        data[byte_index] = (data[byte_index] & mask) | ((value & 15) << shift);
    }

    pub fn get_data(&mut self) -> &[u8; DATA_LAYER_SIZE] {
        self.data
            .get_or_insert_with(|| Box::new([0; DATA_LAYER_SIZE]))
    }

    pub fn get_data_mut(&mut self) -> &mut [u8; DATA_LAYER_SIZE] {
        self.data
            .get_or_insert_with(|| Box::new([0; DATA_LAYER_SIZE]))
    }

    pub fn as_bytes(&self) -> Option<&[u8; DATA_LAYER_SIZE]> {
        self.data.as_deref()
    }

    pub fn into_bytes(self) -> Option<Vec<u8>> {
        self.data.map(|data| Vec::from(*data))
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_none()
    }
}

pub fn data_layer_index(x: i32, y: i32, z: i32) -> usize {
    assert!((0..16).contains(&x), "DataLayer x {x} out of bounds");
    assert!((0..16).contains(&y), "DataLayer y {y} out of bounds");
    assert!((0..16).contains(&z), "DataLayer z {z} out of bounds");
    ((y << 8) | (z << 4) | x) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_layer_empty_reads_zero_and_allocates_lazily() {
        let mut layer = DataLayer::new();

        assert!(layer.is_empty());
        assert_eq!(layer.get(15, 15, 15), 0);
        assert!(layer.is_empty());
        assert_eq!(layer.get_data().len(), DATA_LAYER_SIZE);
        assert!(!layer.is_empty());
    }

    #[test]
    fn data_layer_mut_data_allocates_and_allows_bulk_fill() {
        let mut layer = DataLayer::new();

        layer.get_data_mut().fill(0xFF);

        assert!(!layer.is_empty());
        assert_eq!(layer.get(0, 0, 0), 15);
        assert_eq!(layer.get(15, 15, 15), 15);
    }

    #[test]
    fn data_layer_uses_java_index_and_nibble_order() {
        let mut layer = DataLayer::new();

        layer.set(0, 0, 0, 0x0A);
        layer.set(1, 0, 0, 0x03);
        layer.set(15, 15, 15, 0x0F);

        let data = layer.as_bytes().unwrap();
        assert_eq!(data[0], 0x3A);
        assert_eq!(data_layer_index(0, 1, 0), 256);
        assert_eq!(data_layer_index(15, 15, 15), 4095);
        assert_eq!(layer.get(0, 0, 0), 0x0A);
        assert_eq!(layer.get(1, 0, 0), 0x03);
        assert_eq!(layer.get(15, 15, 15), 0x0F);
    }

    #[test]
    fn data_layer_copy_is_independent() {
        let mut original = DataLayer::new();
        original.set(2, 3, 4, 7);
        let mut copied = original.clone();

        copied.set(2, 3, 4, 12);

        assert_eq!(original.get(2, 3, 4), 7);
        assert_eq!(copied.get(2, 3, 4), 12);
    }
}
