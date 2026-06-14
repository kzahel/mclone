#![forbid(unsafe_code)]

pub const TARGET_MINECRAFT_VERSION: &str = "1.17.1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct BlockStateId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_target_minecraft_version() {
        assert_eq!(TARGET_MINECRAFT_VERSION, "1.17.1");
    }
}
