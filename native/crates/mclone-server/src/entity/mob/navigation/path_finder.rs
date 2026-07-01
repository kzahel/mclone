use mclone_core::{BlockPos, BlockStateId};
use mclone_path::{PathNeighbor, PathSearch, PathSearchQuery, manhattan_distance};

use super::{BlockPathType, WalkNodeEvaluator, path::GroundPath, path_service::PathRequest};

pub(super) use mclone_path::PathSearchLimits;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PathFinder {
    limits: PathSearchLimits,
}

impl PathFinder {
    pub(super) const fn new(limits: PathSearchLimits) -> Self {
        Self { limits }
    }

    pub(super) fn find_path<F, M>(
        self,
        request: PathRequest,
        block_state_at: &F,
        pathfinding_malus: M,
    ) -> Option<GroundPath>
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
        M: Fn(BlockPathType) -> f32 + Copy,
    {
        let max_up_step_blocks = request.max_up_step.max(1.0).floor() as i32;
        let evaluator = WalkNodeEvaluator::new_with_max_up_step(
            request.mob_width,
            request.mob_height,
            max_up_step_blocks,
        );
        let start =
            evaluator.get_start(request.start_position, block_state_at, pathfinding_malus)?;
        let max_visited_nodes = ((self.limits.max_visited_nodes as f32
            * request.max_visited_nodes_multiplier)
            .floor()
            .max(1.0)) as usize;
        let result = PathSearch::new(PathSearchLimits { max_visited_nodes }).find_path(
            PathSearchQuery::new(start, request.target_position)
                .with_follow_range(request.follow_range),
            |pos| {
                evaluator
                    .get_neighbors(pos, block_state_at, pathfinding_malus)
                    .into_iter()
                    .map(|neighbor| PathNeighbor::new(neighbor.pos, neighbor.cost_malus))
                    .collect()
            },
            |pos, target| manhattan_distance(pos, target) <= request.reach_range,
        );
        let reached = result.can_reach();
        Some(GroundPath::from_nodes(
            result.into_nodes(),
            request.target_position,
            reached,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::Vec3d;

    fn flat_ground(pos: BlockPos) -> Option<BlockStateId> {
        Some(if pos.y == 63 {
            BlockStateId(1)
        } else {
            BlockStateId(mclone_blocks::terrain_id::AIR)
        })
    }

    fn obstacle_ground(pos: BlockPos) -> Option<BlockStateId> {
        if pos.y == 63 || pos == BlockPos::new(1, 64, 0) || pos == BlockPos::new(1, 65, 0) {
            Some(BlockStateId(1))
        } else {
            Some(BlockStateId(mclone_blocks::terrain_id::AIR))
        }
    }

    fn one_block_ledge(pos: BlockPos) -> Option<BlockStateId> {
        let floor_y = if pos.x <= 0 { 63 } else { 64 };
        Some(if pos.y == floor_y {
            BlockStateId(1)
        } else {
            BlockStateId(mclone_blocks::terrain_id::AIR)
        })
    }

    fn request_to(target_position: BlockPos, max_visited_nodes_multiplier: f32) -> PathRequest {
        PathRequest {
            start_position: Vec3d::new(0.5, 64.0, 0.5),
            target_position,
            mob_width: 0.9,
            mob_height: 1.4,
            follow_range: 16.0,
            max_up_step: 0.6,
            reach_range: 0,
            max_visited_nodes_multiplier,
        }
    }

    #[test]
    fn path_finder_returns_direct_flat_path() {
        let path = PathFinder::new(PathSearchLimits {
            max_visited_nodes: 128,
        })
        .find_path(
            request_to(BlockPos::new(3, 64, 0), 1.0),
            &flat_ground,
            BlockPathType::default_malus,
        )
        .expect("flat ground should produce a path");

        assert!(path.can_reach());
        assert_eq!(path.nodes().first(), Some(&BlockPos::new(0, 64, 0)));
        assert_eq!(path.nodes().last(), Some(&BlockPos::new(3, 64, 0)));
        assert!(path.node_count() >= 2);
    }

    #[test]
    fn path_finder_routes_around_blocking_column() {
        let path = PathFinder::new(PathSearchLimits {
            max_visited_nodes: 128,
        })
        .find_path(
            request_to(BlockPos::new(2, 64, 0), 1.0),
            &obstacle_ground,
            BlockPathType::default_malus,
        )
        .expect("flat ground with one two-block obstacle should produce a path");

        assert!(path.can_reach());
        assert_eq!(path.nodes().first(), Some(&BlockPos::new(0, 64, 0)));
        assert_eq!(path.nodes().last(), Some(&BlockPos::new(2, 64, 0)));
        assert!(!path.nodes().contains(&BlockPos::new(1, 64, 0)));
        assert!(path.node_count() > 3);
    }

    #[test]
    fn path_finder_can_route_up_one_block_ledge() {
        let path = PathFinder::new(PathSearchLimits {
            max_visited_nodes: 128,
        })
        .find_path(
            request_to(BlockPos::new(2, 65, 0), 1.0),
            &one_block_ledge,
            BlockPathType::default_malus,
        )
        .expect("one-block ledge should produce a path");

        assert!(path.can_reach());
        assert!(path.nodes().contains(&BlockPos::new(1, 65, 0)));
        assert_eq!(path.nodes().last(), Some(&BlockPos::new(2, 65, 0)));
    }

    #[test]
    fn path_finder_returns_best_partial_path_when_budget_exhausts() {
        let path = PathFinder::new(PathSearchLimits {
            max_visited_nodes: 1,
        })
        .find_path(
            request_to(BlockPos::new(8, 64, 0), 1.0),
            &flat_ground,
            BlockPathType::default_malus,
        )
        .expect("bounded search should still return the best partial path");

        assert!(!path.can_reach());
        assert_eq!(path.nodes(), &[BlockPos::new(0, 64, 0)]);
    }
}
