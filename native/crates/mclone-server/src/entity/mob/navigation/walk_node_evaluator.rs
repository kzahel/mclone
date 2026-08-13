use mclone_blocks::{BlockFluidKind, block_collision_aabb, block_fluid_kind, terrain_id};
use mclone_core::{BlockPos, BlockStateId, Vec3d};

const DEFAULT_MAX_UP_STEP_BLOCKS: i32 = 1;
const MAX_FLOOR_STEP_HEIGHT: f64 = 1.125;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum BlockPathType {
    Blocked,
    Fence,
    Open,
    Walkable,
    Lava,
    Water,
}

impl BlockPathType {
    pub(crate) const fn default_malus(self) -> f32 {
        match self {
            Self::Blocked | Self::Fence | Self::Lava => -1.0,
            Self::Open | Self::Walkable => 0.0,
            Self::Water => 8.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PathNeighbor {
    pub(super) pos: BlockPos,
    pub(super) path_type: BlockPathType,
    pub(super) cost_malus: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WalkNodeEvaluator {
    entity_width_blocks: i32,
    entity_height_blocks: i32,
    entity_depth_blocks: i32,
    max_up_step_blocks: i32,
}

impl WalkNodeEvaluator {
    pub(super) fn new(entity_width: f32, entity_height: f32) -> Self {
        Self::new_with_max_up_step(entity_width, entity_height, DEFAULT_MAX_UP_STEP_BLOCKS)
    }

    pub(super) fn new_with_max_up_step(
        entity_width: f32,
        entity_height: f32,
        max_up_step_blocks: i32,
    ) -> Self {
        Self {
            entity_width_blocks: (entity_width + 1.0).floor().max(1.0) as i32,
            entity_height_blocks: (entity_height + 1.0).floor().max(1.0) as i32,
            entity_depth_blocks: (entity_width + 1.0).floor().max(1.0) as i32,
            max_up_step_blocks: max_up_step_blocks.max(0),
        }
    }

    pub(super) fn get_start<F, M>(
        &self,
        position: Vec3d,
        block_state_at: &F,
        pathfinding_malus: M,
    ) -> Option<BlockPos>
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
        M: Fn(BlockPathType) -> f32 + Copy,
    {
        let mut pos = BlockPos::new(
            position.x.floor() as i32,
            (position.y + 0.5).floor() as i32,
            position.z.floor() as i32,
        );

        for _ in 0..16 {
            if self
                .accepted_at(block_state_at, pathfinding_malus, pos)
                .is_some()
            {
                return Some(pos);
            }
            pos = pos.below();
        }
        None
    }

    pub(super) fn get_neighbors<F, M>(
        &self,
        pos: BlockPos,
        block_state_at: &F,
        pathfinding_malus: M,
    ) -> Vec<PathNeighbor>
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
        M: Fn(BlockPathType) -> f32 + Copy,
    {
        let south = self.find_accepted_node(block_state_at, pathfinding_malus, pos, 0, 1);
        let west = self.find_accepted_node(block_state_at, pathfinding_malus, pos, -1, 0);
        let east = self.find_accepted_node(block_state_at, pathfinding_malus, pos, 1, 0);
        let north = self.find_accepted_node(block_state_at, pathfinding_malus, pos, 0, -1);

        let mut neighbors = Vec::with_capacity(8);
        push_neighbor(&mut neighbors, south);
        push_neighbor(&mut neighbors, west);
        push_neighbor(&mut neighbors, east);
        push_neighbor(&mut neighbors, north);

        let northwest = self.find_accepted_node(block_state_at, pathfinding_malus, pos, -1, -1);
        if is_diagonal_valid(pos, west, north, northwest) {
            push_neighbor(&mut neighbors, northwest);
        }
        let northeast = self.find_accepted_node(block_state_at, pathfinding_malus, pos, 1, -1);
        if is_diagonal_valid(pos, east, north, northeast) {
            push_neighbor(&mut neighbors, northeast);
        }
        let southwest = self.find_accepted_node(block_state_at, pathfinding_malus, pos, -1, 1);
        if is_diagonal_valid(pos, west, south, southwest) {
            push_neighbor(&mut neighbors, southwest);
        }
        let southeast = self.find_accepted_node(block_state_at, pathfinding_malus, pos, 1, 1);
        if is_diagonal_valid(pos, east, south, southeast) {
            push_neighbor(&mut neighbors, southeast);
        }

        neighbors
    }

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
                BlockPathType::Blocked | BlockPathType::Fence => BlockPathType::Walkable,
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
            terrain_id::OAK_FENCE_STATE_START..=terrain_id::OAK_FENCE_STATE_END => {
                BlockPathType::Fence
            }
            terrain_id::OAK_FENCE_GATE_STATE_START..=terrain_id::OAK_FENCE_GATE_STATE_END
                if (state.0 - terrain_id::OAK_FENCE_GATE_STATE_START) & 4 == 0 =>
            {
                BlockPathType::Fence
            }
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

    fn find_accepted_node<F, M>(
        &self,
        block_state_at: &F,
        pathfinding_malus: M,
        from: BlockPos,
        dx: i32,
        dz: i32,
    ) -> Option<PathNeighbor>
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
        M: Fn(BlockPathType) -> f32 + Copy,
    {
        let same_y = from.offset(dx, 0, dz);
        if let Some(neighbor) = self.accepted_at(block_state_at, pathfinding_malus, same_y) {
            return Some(neighbor);
        }

        let current_floor = Self::floor_level(block_state_at, from);
        for step in 1..=self.max_up_step_blocks {
            let stepped = same_y.offset(0, step, 0);
            if let Some(neighbor) = self.accepted_at(block_state_at, pathfinding_malus, stepped) {
                let next_floor = Self::floor_level(block_state_at, stepped);
                if next_floor - current_floor <= MAX_FLOOR_STEP_HEIGHT {
                    return Some(neighbor);
                }
            }
        }

        self.accepted_at(block_state_at, pathfinding_malus, same_y.below())
    }

    fn accepted_at<F, M>(
        &self,
        block_state_at: &F,
        pathfinding_malus: M,
        pos: BlockPos,
    ) -> Option<PathNeighbor>
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
        M: Fn(BlockPathType) -> f32 + Copy,
    {
        let path_type = Self::get_block_path_type_static(block_state_at, pos);
        let cost_malus = pathfinding_malus(path_type);
        if path_type != BlockPathType::Walkable || !(0.0..8.0).contains(&cost_malus) {
            return None;
        }

        for dx in 0..self.entity_width_blocks {
            for dz in 0..self.entity_depth_blocks {
                for dy in 0..self.entity_height_blocks {
                    if !Self::is_open(block_state_at, pos.offset(dx, dy, dz)) {
                        return None;
                    }
                }
                if !Self::is_stable_destination(block_state_at, pos.offset(dx, 0, dz)) {
                    return None;
                }
            }
        }

        Some(PathNeighbor {
            pos,
            path_type,
            cost_malus,
        })
    }
}

fn push_neighbor(neighbors: &mut Vec<PathNeighbor>, neighbor: Option<PathNeighbor>) {
    if let Some(neighbor) = neighbor {
        neighbors.push(neighbor);
    }
}

fn is_diagonal_valid(
    current: BlockPos,
    first_cardinal: Option<PathNeighbor>,
    second_cardinal: Option<PathNeighbor>,
    diagonal: Option<PathNeighbor>,
) -> bool {
    let (Some(first_cardinal), Some(second_cardinal), Some(diagonal)) =
        (first_cardinal, second_cardinal, diagonal)
    else {
        return false;
    };
    diagonal.cost_malus >= 0.0
        && first_cardinal.pos.y <= current.y
        && second_cardinal.pos.y <= current.y
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

    #[test]
    fn fence_and_closed_gate_are_fence_nodes_but_open_gate_is_open() {
        let at = |pos: BlockPos| {
            Some(match pos.x {
                0 => state(terrain_id::OAK_FENCE_STATE_START),
                1 => state(terrain_id::OAK_FENCE_GATE_STATE_START),
                2 => state(terrain_id::OAK_FENCE_GATE_STATE_START + 4),
                _ => state(terrain_id::AIR),
            })
        };

        assert_eq!(
            WalkNodeEvaluator::get_block_path_type_static(&at, BlockPos::new(0, 64, 0)),
            BlockPathType::Fence
        );
        assert_eq!(
            WalkNodeEvaluator::get_block_path_type_static(&at, BlockPos::new(1, 64, 0)),
            BlockPathType::Fence
        );
        assert_eq!(
            WalkNodeEvaluator::get_block_path_type_static(&at, BlockPos::new(2, 64, 0)),
            BlockPathType::Open
        );
        assert_eq!(BlockPathType::Fence.default_malus(), -1.0);
    }

    #[test]
    fn start_uses_on_ground_y_and_entity_clearance() {
        let evaluator = WalkNodeEvaluator::new(0.9, 1.4);

        assert_eq!(
            evaluator.get_start(
                Vec3d::new(0.5, 64.0, 0.5),
                &terrain,
                BlockPathType::default_malus
            ),
            Some(BlockPos::new(0, 64, 0))
        );
    }

    #[test]
    fn neighbors_include_one_block_drop_but_not_unsupported_air() {
        fn stepped_ground(pos: BlockPos) -> Option<BlockStateId> {
            let floor_y = if pos.x <= 0 { 63 } else { 62 };
            Some(if pos.y == floor_y {
                state(1)
            } else {
                state(terrain_id::AIR)
            })
        }

        let evaluator = WalkNodeEvaluator::new(0.9, 1.4);
        let neighbors = evaluator.get_neighbors(
            BlockPos::new(0, 64, 0),
            &stepped_ground,
            BlockPathType::default_malus,
        );

        assert!(
            neighbors
                .iter()
                .any(|node| node.pos == BlockPos::new(1, 63, 0))
        );
        assert!(
            !neighbors
                .iter()
                .any(|node| node.pos == BlockPos::new(1, 64, 0))
        );
    }

    #[test]
    fn neighbors_include_one_block_step_up_with_headroom() {
        fn ledge_ground(pos: BlockPos) -> Option<BlockStateId> {
            let floor_y = if pos.x <= 0 { 63 } else { 64 };
            Some(if pos.y == floor_y {
                state(1)
            } else {
                state(terrain_id::AIR)
            })
        }

        let evaluator = WalkNodeEvaluator::new(0.9, 1.4);
        let neighbors = evaluator.get_neighbors(
            BlockPos::new(0, 64, 0),
            &ledge_ground,
            BlockPathType::default_malus,
        );

        assert!(
            neighbors
                .iter()
                .any(|node| node.pos == BlockPos::new(1, 65, 0))
        );
    }

    #[test]
    fn neighbors_reject_step_up_without_headroom() {
        fn low_ceiling_ledge(pos: BlockPos) -> Option<BlockStateId> {
            let floor_y = if pos.x <= 0 { 63 } else { 64 };
            Some(if pos.y == floor_y || pos == BlockPos::new(1, 66, 0) {
                state(1)
            } else {
                state(terrain_id::AIR)
            })
        }

        let evaluator = WalkNodeEvaluator::new(0.9, 1.4);
        let neighbors = evaluator.get_neighbors(
            BlockPos::new(0, 64, 0),
            &low_ceiling_ledge,
            BlockPathType::default_malus,
        );

        assert!(
            !neighbors
                .iter()
                .any(|node| node.pos == BlockPos::new(1, 65, 0))
        );
    }
}
