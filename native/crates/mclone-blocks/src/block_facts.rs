use mclone_core::BlockStateId;

/// Current generated terrain ids are identity-mapped to `BlockStateId`.
/// Keep this table narrow until client/server gameplay consumes the full
/// block-state registry instead of the terrain-MVP raw id lane.
pub mod terrain_id {
    pub const AIR: u32 = 0;
    pub const WATER: u32 = 2;
    pub const SNOW: u32 = 8;
    pub const LAVA: u32 = 9;
    pub const GRASS: u32 = 43;
    pub const DANDELION: u32 = 44;
    pub const POPPY: u32 = 45;
    pub const FERN: u32 = 50;
    pub const DEAD_BUSH: u32 = 51;
    pub const LARGE_FERN_LOWER: u32 = 68;
    pub const LARGE_FERN_UPPER: u32 = 69;
    pub const GLOW_LICHEN: u32 = 70;
    pub const CAVE_AIR: u32 = 71;
    pub const WATER_LEVEL_1: u32 = 72;
    pub const WATER_LEVEL_8: u32 = 79;
    pub const LAVA_LEVEL_1: u32 = 80;
    pub const LAVA_LEVEL_8: u32 = 87;
    #[cfg(test)]
    pub const DRIPSTONE_BLOCK: u32 = 89;
    pub const POINTED_DRIPSTONE: u32 = 90;
    pub const TORCH: u32 = 100;
    pub const WALL_TORCH_NORTH: u32 = 101;
    pub const WALL_TORCH_EAST: u32 = 102;
    pub const WALL_TORCH_SOUTH: u32 = 103;
    pub const WALL_TORCH_WEST: u32 = 104;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BlockFluidKind {
    #[default]
    None,
    Water,
    Lava,
}

pub const WATER_BLOCK_STATE_ID: BlockStateId = BlockStateId(terrain_id::WATER);
pub const LAVA_BLOCK_STATE_ID: BlockStateId = BlockStateId(terrain_id::LAVA);

pub fn block_fluid_kind(state: BlockStateId) -> BlockFluidKind {
    match state.0 {
        terrain_id::WATER => BlockFluidKind::Water,
        terrain_id::LAVA => BlockFluidKind::Lava,
        id if (terrain_id::WATER_LEVEL_1..=terrain_id::WATER_LEVEL_8).contains(&id) => {
            BlockFluidKind::Water
        }
        id if (terrain_id::LAVA_LEVEL_1..=terrain_id::LAVA_LEVEL_8).contains(&id) => {
            BlockFluidKind::Lava
        }
        _ => BlockFluidKind::None,
    }
}

pub fn is_fluid(state: BlockStateId) -> bool {
    block_fluid_kind(state) != BlockFluidKind::None
}

pub fn block_fluid_height(state: BlockStateId) -> Option<f32> {
    match block_fluid_kind(state) {
        BlockFluidKind::Water | BlockFluidKind::Lava => Some(1.0),
        BlockFluidKind::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(id: u32) -> BlockStateId {
        BlockStateId(id)
    }

    #[test]
    fn classifies_source_and_level_fluids() {
        assert_eq!(
            block_fluid_kind(state(terrain_id::WATER)),
            BlockFluidKind::Water
        );
        assert_eq!(
            block_fluid_kind(state(terrain_id::WATER_LEVEL_1)),
            BlockFluidKind::Water
        );
        assert_eq!(
            block_fluid_kind(state(terrain_id::WATER_LEVEL_8)),
            BlockFluidKind::Water
        );
        assert_eq!(
            block_fluid_kind(state(terrain_id::LAVA)),
            BlockFluidKind::Lava
        );
        assert_eq!(
            block_fluid_kind(state(terrain_id::LAVA_LEVEL_1)),
            BlockFluidKind::Lava
        );
        assert_eq!(
            block_fluid_kind(state(terrain_id::LAVA_LEVEL_8)),
            BlockFluidKind::Lava
        );
        assert_eq!(
            block_fluid_kind(state(terrain_id::AIR)),
            BlockFluidKind::None
        );
    }

    #[test]
    fn current_fluid_height_is_full_block_for_terrain_mvp_ids() {
        assert_eq!(block_fluid_height(state(terrain_id::WATER)), Some(1.0));
        assert_eq!(
            block_fluid_height(state(terrain_id::WATER_LEVEL_8)),
            Some(1.0)
        );
        assert_eq!(block_fluid_height(state(terrain_id::AIR)), None);
    }
}
