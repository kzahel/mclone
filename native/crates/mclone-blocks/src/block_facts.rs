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
    pub const CACTUS: u32 = 105;
    pub const SUGAR_CANE: u32 = 106;
    pub const SEAGRASS: u32 = 107;
    pub const TALL_SEAGRASS_LOWER: u32 = 108;
    pub const TALL_SEAGRASS_UPPER: u32 = 109;
    pub const KELP: u32 = 110;
    pub const KELP_PLANT: u32 = 111;
    pub const TUBE_CORAL_BLOCK: u32 = 112;
    pub const BRAIN_CORAL_BLOCK: u32 = 113;
    pub const BUBBLE_CORAL_BLOCK: u32 = 114;
    pub const FIRE_CORAL_BLOCK: u32 = 115;
    pub const HORN_CORAL_BLOCK: u32 = 116;
    pub const SEA_PICKLE_1: u32 = 117;
    pub const SEA_PICKLE_2: u32 = 118;
    pub const SEA_PICKLE_3: u32 = 119;
    pub const SEA_PICKLE_4: u32 = 120;
    pub const DARK_OAK_LOG: u32 = 121;
    pub const DARK_OAK_LEAVES: u32 = 122;
    pub const BROWN_MUSHROOM_BLOCK: u32 = 123;
    pub const RED_MUSHROOM_BLOCK: u32 = 124;
    pub const MUSHROOM_STEM: u32 = 125;
    pub const ACACIA_LOG: u32 = 126;
    pub const ACACIA_LEAVES: u32 = 127;
    pub const JUNGLE_LOG: u32 = 128;
    pub const JUNGLE_LEAVES: u32 = 129;
    pub const BAMBOO: u32 = 130;
    pub const LILY_PAD: u32 = 131;
    pub const SWEET_BERRY_BUSH: u32 = 132;
    pub const ALLIUM: u32 = 133;
    pub const AZURE_BLUET: u32 = 134;
    pub const RED_TULIP: u32 = 135;
    pub const ORANGE_TULIP: u32 = 136;
    pub const WHITE_TULIP: u32 = 137;
    pub const PINK_TULIP: u32 = 138;
    pub const OXEYE_DAISY: u32 = 139;
    pub const CORNFLOWER: u32 = 140;
    pub const LILY_OF_THE_VALLEY: u32 = 141;
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
