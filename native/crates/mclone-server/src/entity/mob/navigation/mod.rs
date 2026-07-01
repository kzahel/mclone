mod path;
mod path_finder;
mod path_service;
mod random_pos;
mod walk_node_evaluator;

use mclone_core::{BlockPos, BlockStateId, Vec3d};

pub(crate) use random_pos::{default_random_pos, land_random_pos};
pub(crate) use walk_node_evaluator::{BlockPathType, WalkNodeEvaluator};

use path::GroundPath;
use path_service::{ImmediatePathService, PathRequest};

const DEFAULT_FOLLOW_RANGE_BLOCKS: f32 = 16.0;
const DEFAULT_REACH_RANGE_BLOCKS: i32 = 1;
const DEFAULT_MAX_VISITED_NODES_MULTIPLIER: f32 = 1.0;
const MAX_DISTANCE_TO_WAYPOINT_WIDE_FACTOR: f32 = 0.5;
const MAX_DISTANCE_TO_WAYPOINT_NARROW_BASE: f32 = 0.75;
const STUCK_CHECK_INTERVAL_TICKS: i32 = 100;
const STUCK_MIN_DISTANCE_SQR: f64 = 2.25;
const TARGET_VERTICAL_SEARCH: i32 = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NavigationMoveTarget {
    pub(crate) position: Vec3d,
    pub(crate) speed_modifier: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GroundPathNavigation {
    path: Option<GroundPath>,
    speed_modifier: f64,
    tick: i32,
    last_stuck_check: i32,
    last_stuck_check_pos: Vec3d,
    max_distance_to_waypoint: f32,
    is_stuck: bool,
}

impl Default for GroundPathNavigation {
    fn default() -> Self {
        Self {
            path: None,
            speed_modifier: 0.0,
            tick: 0,
            last_stuck_check: 0,
            last_stuck_check_pos: Vec3d::ZERO,
            max_distance_to_waypoint: MAX_DISTANCE_TO_WAYPOINT_NARROW_BASE,
            is_stuck: false,
        }
    }
}

impl GroundPathNavigation {
    pub(crate) fn is_done(&self) -> bool {
        self.path.as_ref().is_none_or(GroundPath::is_done)
    }

    pub(crate) fn is_in_progress(&self) -> bool {
        !self.is_done()
    }

    pub(crate) fn is_stuck(&self) -> bool {
        self.is_stuck
    }

    pub(crate) fn is_stable_destination<F>(&self, block_state_at: &F, pos: BlockPos) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        WalkNodeEvaluator::is_stable_destination(block_state_at, pos)
    }

    pub(crate) fn target_pos(&self) -> Option<BlockPos> {
        self.path.as_ref().map(GroundPath::target)
    }

    pub(crate) fn stop(&mut self) {
        self.path = None;
    }

    pub(crate) fn move_to<F>(
        &mut self,
        mob_position: Vec3d,
        target: Vec3d,
        speed_modifier: f64,
        mob_width: f32,
        mob_height: f32,
        block_state_at: &F,
        pathfinding_malus: impl Fn(BlockPathType) -> f32 + Copy,
    ) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        let Some(path) = self.create_path(
            mob_position,
            target,
            mob_width,
            mob_height,
            block_state_at,
            pathfinding_malus,
        ) else {
            self.stop();
            return false;
        };

        if path.is_done() {
            return false;
        }

        self.speed_modifier = speed_modifier;
        self.last_stuck_check = self.tick;
        self.last_stuck_check_pos = mob_position;
        self.is_stuck = false;
        self.path = Some(path);
        true
    }

    pub(crate) fn tick<F>(
        &mut self,
        mob_position: Vec3d,
        mob_width: f32,
        on_ground: bool,
        block_state_at: &F,
    ) -> Option<NavigationMoveTarget>
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        self.tick += 1;
        if self.is_done() {
            return None;
        }

        if on_ground {
            self.follow_the_path(mob_position, mob_width);
        }

        let path = self.path.as_ref()?;
        if path.is_done() {
            return None;
        }

        let next = path.next_entity_pos(mob_width);
        let next_block = BlockPos::containing(next);
        let y = if WalkNodeEvaluator::is_open(block_state_at, next_block.below()) {
            next.y
        } else {
            WalkNodeEvaluator::floor_level(block_state_at, next_block)
        };
        Some(NavigationMoveTarget {
            position: Vec3d::new(next.x, y, next.z),
            speed_modifier: self.speed_modifier,
        })
    }

    fn create_path<F>(
        &self,
        mob_position: Vec3d,
        target: Vec3d,
        mob_width: f32,
        mob_height: f32,
        block_state_at: &F,
        pathfinding_malus: impl Fn(BlockPathType) -> f32 + Copy,
    ) -> Option<GroundPath>
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        let target_pos = adjust_ground_target(BlockPos::containing(target), block_state_at)?;
        ImmediatePathService::find_path(
            PathRequest {
                start_position: mob_position,
                target_position: target_pos,
                mob_width,
                mob_height,
                follow_range: DEFAULT_FOLLOW_RANGE_BLOCKS,
                reach_range: DEFAULT_REACH_RANGE_BLOCKS,
                max_visited_nodes_multiplier: DEFAULT_MAX_VISITED_NODES_MULTIPLIER,
            },
            block_state_at,
            pathfinding_malus,
        )
    }

    fn follow_the_path(&mut self, mob_position: Vec3d, mob_width: f32) {
        let Some(path) = &mut self.path else {
            return;
        };

        self.max_distance_to_waypoint = if mob_width > MAX_DISTANCE_TO_WAYPOINT_NARROW_BASE {
            mob_width * MAX_DISTANCE_TO_WAYPOINT_WIDE_FACTOR
        } else {
            MAX_DISTANCE_TO_WAYPOINT_NARROW_BASE - mob_width * MAX_DISTANCE_TO_WAYPOINT_WIDE_FACTOR
        };

        if let Some(next_node) = path.next_node_pos() {
            let dx = (mob_position.x - (next_node.x as f64 + 0.5)).abs();
            let dy = (mob_position.y - next_node.y as f64).abs();
            let dz = (mob_position.z - (next_node.z as f64 + 0.5)).abs();
            if dx < f64::from(self.max_distance_to_waypoint)
                && dz < f64::from(self.max_distance_to_waypoint)
                && dy < 1.0
            {
                path.advance();
            }
        }

        self.do_stuck_detection(mob_position);
    }

    fn do_stuck_detection(&mut self, mob_position: Vec3d) {
        if self.tick - self.last_stuck_check <= STUCK_CHECK_INTERVAL_TICKS {
            return;
        }
        if mob_position.distance_to_sqr(self.last_stuck_check_pos) < STUCK_MIN_DISTANCE_SQR {
            self.is_stuck = true;
            self.stop();
        } else {
            self.is_stuck = false;
        }
        self.last_stuck_check = self.tick;
        self.last_stuck_check_pos = mob_position;
    }
}

fn adjust_ground_target<F>(mut target: BlockPos, block_state_at: &F) -> Option<BlockPos>
where
    F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
{
    if WalkNodeEvaluator::is_open(block_state_at, target) {
        for _ in 0..TARGET_VERTICAL_SEARCH {
            if WalkNodeEvaluator::is_stable_destination(block_state_at, target) {
                return Some(target);
            }
            target = target.below();
        }
        return None;
    }

    for _ in 0..TARGET_VERTICAL_SEARCH {
        target = target.offset(0, 1, 0);
        if WalkNodeEvaluator::is_open(block_state_at, target)
            && WalkNodeEvaluator::is_stable_destination(block_state_at, target)
        {
            return Some(target);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::BlockStateId;

    fn flat_ground(pos: BlockPos) -> Option<BlockStateId> {
        Some(if pos.y == 63 {
            BlockStateId(1)
        } else {
            BlockStateId(mclone_blocks::terrain_id::AIR)
        })
    }

    fn no_blocks(_pos: BlockPos) -> Option<BlockStateId> {
        None
    }

    fn pathfinding_malus(path_type: BlockPathType) -> f32 {
        path_type.default_malus()
    }

    #[test]
    fn ground_navigation_adjusts_air_target_down_to_stable_floor() {
        let mut navigation = GroundPathNavigation::default();

        assert!(navigation.move_to(
            Vec3d::new(8.5, 64.0, 8.5),
            Vec3d::new(10.5, 70.0, 10.5),
            1.0,
            0.9,
            1.4,
            &flat_ground,
            pathfinding_malus,
        ));

        assert_eq!(navigation.target_pos(), Some(BlockPos::new(10, 64, 10)));
    }

    #[test]
    fn ground_navigation_rejects_target_without_floor_support() {
        let mut navigation = GroundPathNavigation::default();

        assert!(!navigation.move_to(
            Vec3d::new(8.5, 64.0, 8.5),
            Vec3d::new(10.5, 70.0, 10.5),
            1.0,
            0.9,
            1.4,
            &no_blocks,
            pathfinding_malus,
        ));
        assert!(navigation.is_done());
    }

    #[test]
    fn ground_navigation_ticks_next_waypoint_into_move_target() {
        let mut navigation = GroundPathNavigation::default();
        navigation.move_to(
            Vec3d::new(8.5, 64.0, 8.5),
            Vec3d::new(10.5, 64.0, 10.5),
            1.0,
            0.9,
            1.4,
            &flat_ground,
            pathfinding_malus,
        );

        let target = navigation
            .tick(Vec3d::new(8.5, 64.0, 8.5), 0.9, true, &flat_ground)
            .expect("path should feed a move target");

        assert!(target.position.distance_to_sqr(Vec3d::new(8.5, 64.0, 8.5)) > 0.0);
        assert_eq!(target.speed_modifier, 1.0);
    }
}
