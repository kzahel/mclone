use mclone_core::ChunkPos;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LightChunkKey {
    pub pos: ChunkPos,
    pub revision: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_inputs_are_revisioned() {
        let key = LightChunkKey {
            pos: ChunkPos::new(2, -3),
            revision: 7,
        };
        assert_eq!(key.revision, 7);
    }
}
