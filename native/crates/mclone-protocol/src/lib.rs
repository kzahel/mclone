#![forbid(unsafe_code)]

use mclone_core::ChunkPos;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkInterest {
    pub center: ChunkPos,
    pub radius_chunks: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_interest_is_data_only() {
        let interest = ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 8,
        };
        assert_eq!(interest.radius_chunks, 8);
    }
}
