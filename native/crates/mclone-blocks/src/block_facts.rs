use mclone_core::BlockStateId;

/// Current generated terrain ids are identity-mapped to `BlockStateId`.
/// Keep this table narrow until client/server gameplay consumes the full
/// block-state registry instead of the terrain-MVP raw id lane.
pub mod terrain_id {
    pub const AIR: u32 = 0;
    pub const WATER: u32 = 2;
    pub const SNOW: u32 = 8;
    pub const LAVA: u32 = 9;
    pub const PACKED_ICE: u32 = 35;
    pub const ICE: u32 = 39;
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
    pub const LILAC_LOWER: u32 = 142;
    pub const LILAC_UPPER: u32 = 143;
    pub const ROSE_BUSH_LOWER: u32 = 144;
    pub const ROSE_BUSH_UPPER: u32 = 145;
    pub const PEONY_LOWER: u32 = 146;
    pub const PEONY_UPPER: u32 = 147;
    pub const SUNFLOWER_LOWER: u32 = 148;
    pub const SUNFLOWER_UPPER: u32 = 149;
    pub const BLUE_ORCHID: u32 = 150;
    pub const BROWN_MUSHROOM: u32 = 151;
    pub const RED_MUSHROOM: u32 = 152;
    pub const MELON: u32 = 155;
    pub const VINE_EAST: u32 = 156;
    pub const VINE: u32 = VINE_EAST;
    pub const TALL_GRASS_LOWER: u32 = 157;
    pub const TALL_GRASS_UPPER: u32 = 158;
    pub const VINE_UP: u32 = 159;
    pub const VINE_NORTH: u32 = 160;
    pub const VINE_SOUTH: u32 = 161;
    pub const VINE_WEST: u32 = 162;
    pub const COCOA_AGE0_NORTH: u32 = 163;
    pub const COCOA_AGE0_EAST: u32 = 164;
    pub const COCOA_AGE0_SOUTH: u32 = 165;
    pub const COCOA_AGE0_WEST: u32 = 166;
    pub const COCOA_AGE1_NORTH: u32 = 167;
    pub const COCOA_AGE1_EAST: u32 = 168;
    pub const COCOA_AGE1_SOUTH: u32 = 169;
    pub const COCOA_AGE1_WEST: u32 = 170;
    pub const COCOA_AGE2_NORTH: u32 = 171;
    pub const COCOA_AGE2_EAST: u32 = 172;
    pub const COCOA_AGE2_SOUTH: u32 = 173;
    pub const COCOA_AGE2_WEST: u32 = 174;
    pub const BAMBOO_TOP_SMALL: u32 = 175;
    pub const BAMBOO_TOP_LARGE: u32 = 176;
    pub const BAMBOO_FINAL_LARGE: u32 = 177;
    pub const MOSSY_COBBLESTONE: u32 = 178;
    pub const TUBE_CORAL: u32 = 179;
    pub const BRAIN_CORAL: u32 = 180;
    pub const BUBBLE_CORAL: u32 = 181;
    pub const FIRE_CORAL: u32 = 182;
    pub const HORN_CORAL: u32 = 183;
    pub const TUBE_CORAL_FAN: u32 = 184;
    pub const BRAIN_CORAL_FAN: u32 = 185;
    pub const BUBBLE_CORAL_FAN: u32 = 186;
    pub const FIRE_CORAL_FAN: u32 = 187;
    pub const HORN_CORAL_FAN: u32 = 188;
    pub const TUBE_CORAL_WALL_FAN_NORTH: u32 = 189;
    pub const TUBE_CORAL_WALL_FAN_EAST: u32 = 190;
    pub const TUBE_CORAL_WALL_FAN_SOUTH: u32 = 191;
    pub const TUBE_CORAL_WALL_FAN_WEST: u32 = 192;
    pub const BRAIN_CORAL_WALL_FAN_NORTH: u32 = 193;
    pub const BRAIN_CORAL_WALL_FAN_EAST: u32 = 194;
    pub const BRAIN_CORAL_WALL_FAN_SOUTH: u32 = 195;
    pub const BRAIN_CORAL_WALL_FAN_WEST: u32 = 196;
    pub const BUBBLE_CORAL_WALL_FAN_NORTH: u32 = 197;
    pub const BUBBLE_CORAL_WALL_FAN_EAST: u32 = 198;
    pub const BUBBLE_CORAL_WALL_FAN_SOUTH: u32 = 199;
    pub const BUBBLE_CORAL_WALL_FAN_WEST: u32 = 200;
    pub const FIRE_CORAL_WALL_FAN_NORTH: u32 = 201;
    pub const FIRE_CORAL_WALL_FAN_EAST: u32 = 202;
    pub const FIRE_CORAL_WALL_FAN_SOUTH: u32 = 203;
    pub const FIRE_CORAL_WALL_FAN_WEST: u32 = 204;
    pub const HORN_CORAL_WALL_FAN_NORTH: u32 = 205;
    pub const HORN_CORAL_WALL_FAN_EAST: u32 = 206;
    pub const HORN_CORAL_WALL_FAN_SOUTH: u32 = 207;
    pub const HORN_CORAL_WALL_FAN_WEST: u32 = 208;
    pub const GLASS: u32 = 214;
    pub const SPRUCE_STAIRS_NORTH: u32 = 215;
    pub const SPRUCE_STAIRS_EAST: u32 = 216;
    pub const SPRUCE_STAIRS_SOUTH: u32 = 217;
    pub const SPRUCE_STAIRS_WEST: u32 = 218;
    pub const SPRUCE_SLAB_BOTTOM: u32 = 219;
    pub const SPRUCE_SLAB_TOP: u32 = 220;
    pub const FARMLAND_MOISTURE_0: u32 = 221;
    pub const FARMLAND_MOISTURE_7: u32 = 228;
    pub const WHEAT_AGE_0: u32 = 229;
    pub const WHEAT_AGE_7: u32 = 236;
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
pub const DEFAULT_BLOCK_FRICTION: f32 = 0.6;
pub const DEFAULT_BLOCK_SPEED_FACTOR: f32 = 1.0;
pub const DEFAULT_BLOCK_JUMP_FACTOR: f32 = 1.0;

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

pub fn block_friction(state: BlockStateId) -> f32 {
    match state.0 {
        terrain_id::ICE | terrain_id::PACKED_ICE => 0.98,
        _ => DEFAULT_BLOCK_FRICTION,
    }
}

pub fn block_speed_factor(_state: BlockStateId) -> f32 {
    DEFAULT_BLOCK_SPEED_FACTOR
}

pub fn block_jump_factor(_state: BlockStateId) -> f32 {
    DEFAULT_BLOCK_JUMP_FACTOR
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

    #[test]
    fn java_movement_defaults_apply_to_most_terrain_mvp_blocks() {
        assert_eq!(
            block_friction(state(terrain_id::AIR)),
            DEFAULT_BLOCK_FRICTION
        );
        assert_eq!(
            block_friction(state(terrain_id::GRASS)),
            DEFAULT_BLOCK_FRICTION
        );
        assert_eq!(
            block_speed_factor(state(terrain_id::GRASS)),
            DEFAULT_BLOCK_SPEED_FACTOR
        );
        assert_eq!(
            block_jump_factor(state(terrain_id::GRASS)),
            DEFAULT_BLOCK_JUMP_FACTOR
        );
    }

    #[test]
    fn ice_blocks_use_java_high_friction() {
        assert_eq!(block_friction(state(terrain_id::ICE)), 0.98);
        assert_eq!(block_friction(state(terrain_id::PACKED_ICE)), 0.98);
    }
}
