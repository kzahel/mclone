use mclone_core::BlockStateId;

/// Current generated terrain ids are identity-mapped to `BlockStateId`.
/// Keep this table narrow until client gameplay consumes the full block-state
/// registry instead of the terrain-MVP raw id lane.
pub(crate) mod terrain_id {
    pub(crate) const AIR: u32 = 0;
    pub(crate) const WATER: u32 = 2;
    pub(crate) const SNOW: u32 = 8;
    pub(crate) const LAVA: u32 = 9;
    pub(crate) const GRASS: u32 = 43;
    pub(crate) const DANDELION: u32 = 44;
    pub(crate) const POPPY: u32 = 45;
    pub(crate) const FERN: u32 = 50;
    pub(crate) const DEAD_BUSH: u32 = 51;
    pub(crate) const LARGE_FERN_LOWER: u32 = 68;
    pub(crate) const LARGE_FERN_UPPER: u32 = 69;
    pub(crate) const GLOW_LICHEN: u32 = 70;
    pub(crate) const CAVE_AIR: u32 = 71;
    pub(crate) const WATER_LEVEL_1: u32 = 72;
    pub(crate) const WATER_LEVEL_8: u32 = 79;
    pub(crate) const LAVA_LEVEL_1: u32 = 80;
    pub(crate) const LAVA_LEVEL_8: u32 = 87;
    #[cfg(test)]
    pub(crate) const DRIPSTONE_BLOCK: u32 = 89;
    pub(crate) const POINTED_DRIPSTONE: u32 = 90;
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
