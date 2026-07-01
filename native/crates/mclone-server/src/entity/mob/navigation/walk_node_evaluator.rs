use mclone_blocks::{BlockFluidKind, block_collision_aabb, block_fluid_kind, terrain_id};
use mclone_core::{BlockPos, BlockStateId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum BlockPathType {
    Blocked,
    Open,
    Walkable,
    Lava,
    Water,
}

impl BlockPathType {
    pub(crate) const fn default_malus(self) -> f32 {
        match self {
            Self::Blocked | Self::Lava => -1.0,
            Self::Open | Self::Walkable => 0.0,
            Self::Water => 8.0,
        }
    }
}

pub(crate) struct WalkNodeEvaluator;

impl WalkNodeEvaluator {
    pub(crate) fn get_block_path_type_static<F>(block_state_at: &F, pos: BlockPos) -> BlockPathType
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        let mut path_type = Self::get_block_path_type_raw(block_state_at, pos);
        if path_type == BlockPathType::Open {
            let below = Self::get_block_path_type_raw(block_state_at, pos.below());
            path_type = match below {
                BlockPathType::Walkable
                | BlockPathType::Open
                | BlockPathType::Water
                | BlockPathType::Lava => BlockPathType::Open,
                BlockPathType::Blocked => BlockPathType::Walkable,
            };
        }
        path_type
    }

    pub(crate) fn is_open<F>(block_state_at: &F, pos: BlockPos) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        matches!(
            Self::get_block_path_type_raw(block_state_at, pos),
            BlockPathType::Open
        )
    }

    pub(crate) fn is_stable_destination<F>(block_state_at: &F, pos: BlockPos) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        block_state_at(pos.below())
            .and_then(|state| block_collision_aabb(state, pos.below()))
            .is_some()
    }

    pub(crate) fn floor_level<F>(block_state_at: &F, pos: BlockPos) -> f64
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        let below = pos.below();
        block_state_at(below)
            .and_then(|state| block_collision_aabb(state, below))
            .map_or(below.y as f64, |shape| shape.max_y)
    }

    pub(crate) fn has_malus<F, M>(block_state_at: &F, pathfinding_malus: M, pos: BlockPos) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
        M: Fn(BlockPathType) -> f32,
    {
        pathfinding_malus(Self::get_block_path_type_static(block_state_at, pos)) != 0.0
    }

    pub(crate) fn is_water<F>(block_state_at: &F, pos: BlockPos) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        block_state_at(pos).is_some_and(|state| block_fluid_kind(state) == BlockFluidKind::Water)
    }

    pub(crate) fn is_solid<F>(block_state_at: &F, pos: BlockPos) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        block_state_at(pos)
            .and_then(|state| block_collision_aabb(state, pos))
            .is_some()
    }

    fn get_block_path_type_raw<F>(block_state_at: &F, pos: BlockPos) -> BlockPathType
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        let Some(state) = block_state_at(pos) else {
            return BlockPathType::Blocked;
        };

        match state.0 {
            terrain_id::AIR | terrain_id::CAVE_AIR => BlockPathType::Open,
            _ => match block_fluid_kind(state) {
                BlockFluidKind::Water => BlockPathType::Water,
                BlockFluidKind::Lava => BlockPathType::Lava,
                BlockFluidKind::None => {
                    if block_collision_aabb(state, pos).is_some() {
                        BlockPathType::Blocked
                    } else {
                        BlockPathType::Open
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(id: u32) -> BlockStateId {
        BlockStateId(id)
    }

    fn terrain(pos: BlockPos) -> Option<BlockStateId> {
        match pos {
            BlockPos { x: 0, y: 63, z: 0 } => Some(state(1)),
            BlockPos { x: 1, y: 64, z: 0 } => Some(state(terrain_id::WATER)),
            BlockPos { x: 2, y: 64, z: 0 } => Some(state(terrain_id::GRASS)),
            _ => Some(state(terrain_id::AIR)),
        }
    }

    #[test]
    fn air_above_solid_is_walkable() {
        assert_eq!(
            WalkNodeEvaluator::get_block_path_type_static(&terrain, BlockPos::new(0, 64, 0)),
            BlockPathType::Walkable
        );
        assert!(WalkNodeEvaluator::is_stable_destination(
            &terrain,
            BlockPos::new(0, 64, 0)
        ));
    }

    #[test]
    fn open_air_without_support_stays_open() {
        assert_eq!(
            WalkNodeEvaluator::get_block_path_type_static(&terrain, BlockPos::new(4, 80, 0)),
            BlockPathType::Open
        );
        assert!(!WalkNodeEvaluator::is_stable_destination(
            &terrain,
            BlockPos::new(4, 80, 0)
        ));
    }

    #[test]
    fn fluids_and_no_collision_plants_are_classified_for_land_navigation() {
        assert_eq!(
            WalkNodeEvaluator::get_block_path_type_static(&terrain, BlockPos::new(1, 64, 0)),
            BlockPathType::Water
        );
        assert_eq!(
            WalkNodeEvaluator::get_block_path_type_static(&terrain, BlockPos::new(2, 64, 0)),
            BlockPathType::Open
        );
    }
}
