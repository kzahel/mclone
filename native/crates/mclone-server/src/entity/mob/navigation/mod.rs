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

const DEFAULT_REACH_RANGE_BLOCKS: i32 = 1;
const DEFAULT_MAX_VISITED_NODES_MULTIPLIER: f32 = 1.0;
const MAX_TIME_RECOMPUTE_TICKS: i32 = 20;
const MAX_DISTANCE_TO_WAYPOINT_WIDE_FACTOR: f32 = 0.5;
const MAX_DISTANCE_TO_WAYPOINT_NARROW_BASE: f32 = 0.75;
const STUCK_CHECK_INTERVAL_TICKS: i32 = 100;
const STUCK_MIN_DISTANCE_SQR: f64 = 2.25;
const NAVIGATION_TICK_MILLIS: u64 = 50;
const TIMEOUT_LIMIT_MULTIPLIER: f64 = 3.0;
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
    timeout_cached_node: Option<BlockPos>,
    timeout_timer_millis: u64,
    last_timeout_check_tick: i32,
    timeout_limit_millis: f64,
    max_distance_to_waypoint: f32,
    has_delayed_recomputation: bool,
    time_last_recompute: i32,
    target_pos: Option<BlockPos>,
    reach_range: i32,
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
            timeout_cached_node: None,
            timeout_timer_millis: 0,
            last_timeout_check_tick: 0,
            timeout_limit_millis: 0.0,
            max_distance_to_waypoint: MAX_DISTANCE_TO_WAYPOINT_NARROW_BASE,
            has_delayed_recomputation: false,
            time_last_recompute: 0,
            target_pos: None,
            reach_range: DEFAULT_REACH_RANGE_BLOCKS,
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

    pub(crate) fn has_delayed_recomputation(&self) -> bool {
        self.has_delayed_recomputation
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

    pub(crate) fn path_reaches_target(&self) -> bool {
        self.path.as_ref().is_some_and(GroundPath::can_reach)
    }

    pub(crate) fn stop(&mut self) {
        self.path = None;
    }

    pub(crate) fn recompute_path_around(
        &mut self,
        changed_pos: BlockPos,
        mob_position: Vec3d,
    ) -> bool {
        let Some(path) = &self.path else {
            return false;
        };
        if path.is_done() || path.node_count() == 0 {
            return false;
        }
        let Some(end_node) = path.end_node_pos() else {
            return false;
        };

        let midpoint = Vec3d::new(
            (end_node.x as f64 + mob_position.x) * 0.5,
            (end_node.y as f64 + mob_position.y) * 0.5,
            (end_node.z as f64 + mob_position.z) * 0.5,
        );
        let remaining_nodes = path.remaining_node_count() as f64;
        if !block_pos_closer_than(changed_pos, midpoint, remaining_nodes) {
            return false;
        }

        self.has_delayed_recomputation = true;
        true
    }

    pub(crate) fn recompute_path<F>(
        &mut self,
        mob_position: Vec3d,
        mob_width: f32,
        mob_height: f32,
        follow_range: f32,
        max_up_step: f64,
        block_state_at: &F,
        pathfinding_malus: impl Fn(BlockPathType) -> f32 + Copy,
    ) where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        if self.tick - self.time_last_recompute <= MAX_TIME_RECOMPUTE_TICKS {
            self.has_delayed_recomputation = true;
            return;
        }

        let Some(target_pos) = self.target_pos else {
            self.has_delayed_recomputation = false;
            return;
        };

        self.path = self.create_path_to_block(
            mob_position,
            target_pos,
            mob_width,
            mob_height,
            follow_range,
            max_up_step,
            self.reach_range,
            block_state_at,
            pathfinding_malus,
        );
        self.time_last_recompute = self.tick;
        self.has_delayed_recomputation = false;
        self.reset_stuck_timeout();
    }

    pub(crate) fn move_to<F>(
        &mut self,
        mob_position: Vec3d,
        target: Vec3d,
        speed_modifier: f64,
        mob_width: f32,
        mob_height: f32,
        follow_range: f32,
        max_up_step: f64,
        block_state_at: &F,
        pathfinding_malus: impl Fn(BlockPathType) -> f32 + Copy,
    ) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        self.move_to_with_reach_range(
            mob_position,
            target,
            speed_modifier,
            mob_width,
            mob_height,
            follow_range,
            max_up_step,
            DEFAULT_REACH_RANGE_BLOCKS,
            block_state_at,
            pathfinding_malus,
        )
    }

    pub(crate) fn move_to_with_reach_range<F>(
        &mut self,
        mob_position: Vec3d,
        target: Vec3d,
        speed_modifier: f64,
        mob_width: f32,
        mob_height: f32,
        follow_range: f32,
        max_up_step: f64,
        reach_range: i32,
        block_state_at: &F,
        pathfinding_malus: impl Fn(BlockPathType) -> f32 + Copy,
    ) -> bool
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        let Some(mut path) = self.create_path(
            mob_position,
            target,
            mob_width,
            mob_height,
            follow_range,
            max_up_step,
            reach_range,
            block_state_at,
            pathfinding_malus,
        ) else {
            self.stop();
            return false;
        };

        self.trim_path(&mut path, block_state_at);
        if path.is_done() {
            return false;
        }

        self.speed_modifier = speed_modifier;
        self.last_stuck_check = self.tick;
        self.last_stuck_check_pos = mob_position;
        self.target_pos = Some(path.target());
        self.reach_range = reach_range;
        self.reset_stuck_timeout();
        self.path = Some(path);
        true
    }

    pub(crate) fn tick<F>(
        &mut self,
        mob_position: Vec3d,
        mob_width: f32,
        mob_height: f32,
        on_ground: bool,
        follow_range: f32,
        max_up_step: f64,
        movement_speed: f64,
        block_state_at: &F,
        pathfinding_malus: impl Fn(BlockPathType) -> f32 + Copy,
    ) -> Option<NavigationMoveTarget>
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        self.tick += 1;
        if self.has_delayed_recomputation {
            self.recompute_path(
                mob_position,
                mob_width,
                mob_height,
                follow_range,
                max_up_step,
                block_state_at,
                pathfinding_malus,
            );
        }

        if self.is_done() {
            return None;
        }

        if on_ground {
            self.follow_the_path(mob_position, mob_width, movement_speed);
        } else {
            self.advance_if_falling_past_waypoint(mob_position, mob_width);
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
        follow_range: f32,
        max_up_step: f64,
        reach_range: i32,
        block_state_at: &F,
        pathfinding_malus: impl Fn(BlockPathType) -> f32 + Copy,
    ) -> Option<GroundPath>
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        let target_pos = adjust_ground_target(BlockPos::containing(target), block_state_at)?;
        self.create_path_to_block(
            mob_position,
            target_pos,
            mob_width,
            mob_height,
            follow_range,
            max_up_step,
            reach_range,
            block_state_at,
            pathfinding_malus,
        )
    }

    fn trim_path<F>(&self, path: &mut GroundPath, block_state_at: &F)
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        trim_path_with(path, |pos| is_cauldron_path_trim_block(block_state_at, pos));
    }

    fn create_path_to_block<F>(
        &self,
        mob_position: Vec3d,
        target_pos: BlockPos,
        mob_width: f32,
        mob_height: f32,
        follow_range: f32,
        max_up_step: f64,
        reach_range: i32,
        block_state_at: &F,
        pathfinding_malus: impl Fn(BlockPathType) -> f32 + Copy,
    ) -> Option<GroundPath>
    where
        F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    {
        ImmediatePathService::find_path(
            PathRequest {
                start_position: mob_position,
                target_position: target_pos,
                mob_width,
                mob_height,
                follow_range,
                max_up_step,
                reach_range,
                max_visited_nodes_multiplier: DEFAULT_MAX_VISITED_NODES_MULTIPLIER,
            },
            block_state_at,
            pathfinding_malus,
        )
    }

    fn follow_the_path(&mut self, mob_position: Vec3d, mob_width: f32, movement_speed: f64) {
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
            } else if should_target_next_node_in_direction(path, mob_position) {
                path.advance();
            }
        }

        self.do_stuck_detection(mob_position, movement_speed);
    }

    fn advance_if_falling_past_waypoint(&mut self, mob_position: Vec3d, mob_width: f32) {
        let Some(path) = &mut self.path else {
            return;
        };
        if path.is_done() {
            return;
        }

        let next = path.next_entity_pos(mob_width);
        if mob_position.y > next.y
            && floor_to_i32(mob_position.x) == floor_to_i32(next.x)
            && floor_to_i32(mob_position.z) == floor_to_i32(next.z)
        {
            path.advance();
        }
    }

    fn do_stuck_detection(&mut self, mob_position: Vec3d, movement_speed: f64) {
        if self.tick - self.last_stuck_check <= STUCK_CHECK_INTERVAL_TICKS {
        } else {
            if mob_position.distance_to_sqr(self.last_stuck_check_pos) < STUCK_MIN_DISTANCE_SQR {
                self.is_stuck = true;
                self.stop();
            } else {
                self.is_stuck = false;
            }
            self.last_stuck_check = self.tick;
            self.last_stuck_check_pos = mob_position;
        }

        let Some(path) = &self.path else {
            return;
        };
        if path.is_done() {
            return;
        }

        let Some(next_node) = path.next_node_pos() else {
            return;
        };
        if Some(next_node) == self.timeout_cached_node {
            let elapsed_ticks = (self.tick - self.last_timeout_check_tick).max(0) as u64;
            self.timeout_timer_millis = self
                .timeout_timer_millis
                .saturating_add(elapsed_ticks.saturating_mul(NAVIGATION_TICK_MILLIS));
        } else {
            self.timeout_cached_node = Some(next_node);
            let distance = mob_position
                .distance_to_sqr(bottom_center(next_node))
                .sqrt();
            let speed = movement_speed * self.speed_modifier;
            self.timeout_limit_millis = if speed > 0.0 {
                distance / speed * 1000.0
            } else {
                0.0
            };
        }

        if self.timeout_limit_millis > 0.0
            && self.timeout_timer_millis as f64
                > self.timeout_limit_millis * TIMEOUT_LIMIT_MULTIPLIER
        {
            self.timeout_path();
        }

        self.last_timeout_check_tick = self.tick;
    }

    fn timeout_path(&mut self) {
        self.reset_stuck_timeout();
        self.stop();
    }

    fn reset_stuck_timeout(&mut self) {
        self.timeout_cached_node = None;
        self.timeout_timer_millis = 0;
        self.timeout_limit_millis = 0.0;
        self.last_timeout_check_tick = self.tick;
        self.is_stuck = false;
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

fn trim_path_with(path: &mut GroundPath, mut is_cauldron: impl FnMut(BlockPos) -> bool) {
    for index in 0..path.node_count() {
        let Some(node) = path.node_pos(index) else {
            continue;
        };
        let next = path.node_pos(index + 1);
        if !is_cauldron(node) {
            continue;
        }

        path.replace_node(index, node.offset(0, 1, 0));
        if let Some(next) = next {
            if node.y >= next.y {
                path.replace_node(index + 1, next.offset(0, 1, 0));
            }
        }
    }
}

fn is_cauldron_path_trim_block<F>(block_state_at: &F, pos: BlockPos) -> bool
where
    F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
{
    let _ = block_state_at(pos);
    // Terrain MVP does not register cauldron block states yet. Keep the Java
    // trim hook in place so adding cauldron facts later is a data extension.
    false
}

fn block_pos_closer_than(pos: BlockPos, target: Vec3d, distance: f64) -> bool {
    block_pos_center(pos).distance_to_sqr(target) < distance * distance
}

fn should_target_next_node_in_direction(path: &GroundPath, mob_position: Vec3d) -> bool {
    let next_index = path.next_node_index();
    if next_index + 1 >= path.node_count() {
        return false;
    }

    let Some(next_node) = path.next_node_pos() else {
        return false;
    };
    let next_center = bottom_center(next_node);
    if mob_position.distance_to_sqr(next_center) >= 4.0 {
        return false;
    }

    let Some(following_node) = path.node_pos(next_index + 1) else {
        return false;
    };
    let following_center = bottom_center(following_node);
    let path_direction = following_center.subtract(next_center);
    let mob_direction = mob_position.subtract(next_center);
    dot(path_direction, mob_direction) > 0.0
}

fn bottom_center(pos: BlockPos) -> Vec3d {
    Vec3d::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5)
}

fn block_pos_center(pos: BlockPos) -> Vec3d {
    Vec3d::new(pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5)
}

fn dot(first: Vec3d, second: Vec3d) -> f64 {
    first.x * second.x + first.y * second.y + first.z * second.z
}

fn floor_to_i32(value: f64) -> i32 {
    value.floor() as i32
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

    fn follow_range() -> f32 {
        16.0
    }

    fn max_up_step() -> f64 {
        0.6
    }

    fn tick_navigation(
        navigation: &mut GroundPathNavigation,
        mob_position: Vec3d,
        on_ground: bool,
    ) -> Option<NavigationMoveTarget> {
        navigation.tick(
            mob_position,
            0.9,
            1.4,
            on_ground,
            follow_range(),
            max_up_step(),
            0.2,
            &flat_ground,
            pathfinding_malus,
        )
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
            follow_range(),
            max_up_step(),
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
            follow_range(),
            max_up_step(),
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
            follow_range(),
            max_up_step(),
            &flat_ground,
            pathfinding_malus,
        );

        let target = navigation
            .tick(
                Vec3d::new(8.5, 64.0, 8.5),
                0.9,
                1.4,
                true,
                follow_range(),
                max_up_step(),
                0.2,
                &flat_ground,
                pathfinding_malus,
            )
            .expect("path should feed a move target");

        assert!(target.position.distance_to_sqr(Vec3d::new(8.5, 64.0, 8.5)) > 0.0);
        assert_eq!(target.speed_modifier, 1.0);
    }

    #[test]
    fn ground_navigation_delays_recompute_until_java_gate_allows() {
        let mut navigation = GroundPathNavigation::default();
        let mob_position = Vec3d::new(8.5, 64.0, 8.5);
        assert!(navigation.move_to(
            mob_position,
            Vec3d::new(12.5, 64.0, 8.5),
            1.0,
            0.9,
            1.4,
            follow_range(),
            max_up_step(),
            &flat_ground,
            pathfinding_malus,
        ));

        navigation.recompute_path(
            mob_position,
            0.9,
            1.4,
            follow_range(),
            max_up_step(),
            &flat_ground,
            pathfinding_malus,
        );
        assert!(navigation.has_delayed_recomputation());

        for _ in 0..20 {
            let _ = tick_navigation(&mut navigation, mob_position, false);
        }
        assert!(navigation.has_delayed_recomputation());

        let _ = tick_navigation(&mut navigation, mob_position, false);
        assert!(!navigation.has_delayed_recomputation());
        assert_eq!(navigation.target_pos(), Some(BlockPos::new(12, 64, 8)));
        assert!(navigation.is_in_progress());
    }

    #[test]
    fn ground_navigation_timeout_cached_node_stops_stalled_path() {
        let mut navigation = GroundPathNavigation::default();
        let mob_position = Vec3d::new(8.5, 64.0, 8.5);
        assert!(navigation.move_to(
            mob_position,
            Vec3d::new(12.5, 64.0, 8.5),
            1.0,
            0.9,
            1.4,
            follow_range(),
            max_up_step(),
            &flat_ground,
            pathfinding_malus,
        ));

        for _ in 0..12 {
            let _ = navigation.tick(
                mob_position,
                0.9,
                1.4,
                true,
                follow_range(),
                max_up_step(),
                10.0,
                &flat_ground,
                pathfinding_malus,
            );
        }

        assert!(navigation.is_done());
        assert!(!navigation.is_stuck());
    }

    #[test]
    fn ground_navigation_advances_when_falling_past_next_waypoint() {
        let mut navigation = GroundPathNavigation::default();
        navigation.path = Some(GroundPath::from_nodes(
            vec![BlockPos::new(8, 64, 8), BlockPos::new(8, 64, 9)],
            BlockPos::new(8, 64, 9),
            true,
        ));
        navigation.speed_modifier = 1.0;

        let target = tick_navigation(&mut navigation, Vec3d::new(8.5, 65.0, 8.5), false)
            .expect("second waypoint should remain after falling past first");

        assert_eq!(
            navigation.path.as_ref().and_then(GroundPath::next_node_pos),
            Some(BlockPos::new(8, 64, 9))
        );
        assert_eq!(target.position, Vec3d::new(8.5, 64.0, 9.5));
    }

    #[test]
    fn ground_navigation_block_change_marks_near_remaining_path_for_recompute() {
        let mut navigation = GroundPathNavigation::default();
        navigation.path = Some(GroundPath::from_nodes(
            vec![
                BlockPos::new(0, 64, 0),
                BlockPos::new(1, 64, 0),
                BlockPos::new(2, 64, 0),
                BlockPos::new(3, 64, 0),
                BlockPos::new(4, 64, 0),
            ],
            BlockPos::new(4, 64, 0),
            true,
        ));

        assert!(
            !navigation
                .recompute_path_around(BlockPos::new(40, 64, 40), Vec3d::new(0.5, 64.0, 0.5),)
        );
        assert!(!navigation.has_delayed_recomputation());

        assert!(
            navigation.recompute_path_around(BlockPos::new(2, 64, 0), Vec3d::new(0.5, 64.0, 0.5),)
        );
        assert!(navigation.has_delayed_recomputation());
    }

    #[test]
    fn ground_navigation_block_change_uses_remaining_path_distance() {
        let mut navigation = GroundPathNavigation::default();
        let mut path = GroundPath::from_nodes(
            vec![
                BlockPos::new(0, 64, 0),
                BlockPos::new(1, 64, 0),
                BlockPos::new(2, 64, 0),
                BlockPos::new(3, 64, 0),
                BlockPos::new(4, 64, 0),
            ],
            BlockPos::new(4, 64, 0),
            true,
        );
        path.advance();
        path.advance();
        path.advance();
        path.advance();
        navigation.path = Some(path);

        assert!(
            !navigation.recompute_path_around(BlockPos::new(0, 64, 0), Vec3d::new(0.5, 64.0, 0.5),)
        );
        assert!(!navigation.has_delayed_recomputation());
    }

    #[test]
    fn ground_navigation_trim_path_hook_raises_cauldron_nodes() {
        let mut path = GroundPath::from_nodes(
            vec![
                BlockPos::new(0, 64, 0),
                BlockPos::new(1, 64, 0),
                BlockPos::new(2, 63, 0),
            ],
            BlockPos::new(2, 63, 0),
            true,
        );

        trim_path_with(&mut path, |pos| pos == BlockPos::new(1, 64, 0));

        assert_eq!(
            path.nodes(),
            &[
                BlockPos::new(0, 64, 0),
                BlockPos::new(1, 65, 0),
                BlockPos::new(2, 64, 0),
            ]
        );
    }
}
