use mclone_core::{BlockPos, BlockStateId, Vec3d};

use super::{
    BlockPathType,
    path::GroundPath,
    path_finder::{PathFinder, PathSearchLimits},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PathRequest {
    pub(super) start_position: Vec3d,
    pub(super) target_position: BlockPos,
    pub(super) mob_width: f32,
    pub(super) mob_height: f32,
    pub(super) follow_range: f32,
    pub(super) max_up_step: f64,
    pub(super) reach_range: i32,
    pub(super) max_visited_nodes_multiplier: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ImmediatePathService;

impl ImmediatePathService {
    pub(super) fn find_path<F, M>(
        request: PathRequest,
        block_state_at: &F,
        pathfinding_malus: M,
    ) -> Option<GroundPath>
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
        M: Fn(BlockPathType) -> f32 + Copy,
    {
        let max_visited_nodes = (request.follow_range * 16.0).floor().max(1.0) as usize;
        PathFinder::new(PathSearchLimits { max_visited_nodes }).find_path(
            request,
            block_state_at,
            pathfinding_malus,
        )
    }
}
