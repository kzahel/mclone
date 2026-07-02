use std::collections::BTreeMap;

use mclone_core::{Aabb, BlockPos, BlockStateId, Vec3d};
use mclone_path::{
    DEFAULT_PATH_HEURISTIC_WEIGHT, PathNeighbor, PathSearch, PathSearchDiagnostics,
    PathSearchLimits, PathSearchQuery,
};

use crate::{ClientRuntime, LOCAL_PLAYER_STANDING_HEIGHT, LOCAL_PLAYER_STANDING_WIDTH};

const TELEPORT_EPSILON: f64 = 1.0e-7;
const DEFAULT_MAX_DISTANCE: f64 = 8.0;
const DEFAULT_ARC_HEIGHT: f64 = 1.25;
const DEFAULT_ARC_SAMPLES: usize = 24;
const DEFAULT_CANDIDATE_RADIUS: f64 = 1.25;
const DEFAULT_CANDIDATE_SPACING: f64 = 0.5;
const DEFAULT_MIN_PROGRESS: f64 = 0.75;
const DEFAULT_MAX_STEP_UP: i32 = 1;
const DEFAULT_MAX_DROP: i32 = 3;
const DEFAULT_MAX_VISITED_NODES: usize = 512;
const DEFAULT_MARKER_HEIGHT: f64 = 1.45;
const SUPPORT_PROBE_DEPTH: f64 = 0.08;

pub trait TeleportCollisionWorld {
    fn block_state_at(&self, pos: BlockPos) -> Option<BlockStateId>;
}

impl TeleportCollisionWorld for ClientRuntime {
    fn block_state_at(&self, pos: BlockPos) -> Option<BlockStateId> {
        self.block_state_at_block_pos(pos)
    }
}

impl<F> TeleportCollisionWorld for F
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    fn block_state_at(&self, pos: BlockPos) -> Option<BlockStateId> {
        self(pos)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeleportIntent {
    pub start_feet: Vec3d,
    pub aim_origin: Vec3d,
    pub aim_direction: Vec3d,
    pub current_yaw_degrees: f64,
}

impl TeleportIntent {
    pub const fn new(
        start_feet: Vec3d,
        aim_origin: Vec3d,
        aim_direction: Vec3d,
        current_yaw_degrees: f64,
    ) -> Self {
        Self {
            start_feet,
            aim_origin,
            aim_direction,
            current_yaw_degrees,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeleportConfig {
    pub max_distance: f64,
    pub arc_height: f64,
    pub arc_samples: usize,
    pub candidate_radius: f64,
    pub candidate_spacing: f64,
    pub min_candidate_progress: f64,
    pub body_width: f64,
    pub body_height: f64,
    pub max_step_up_blocks: i32,
    pub max_drop_blocks: i32,
    pub max_visited_nodes: usize,
    pub marker_height: f64,
}

impl Default for TeleportConfig {
    fn default() -> Self {
        Self {
            max_distance: DEFAULT_MAX_DISTANCE,
            arc_height: DEFAULT_ARC_HEIGHT,
            arc_samples: DEFAULT_ARC_SAMPLES,
            candidate_radius: DEFAULT_CANDIDATE_RADIUS,
            candidate_spacing: DEFAULT_CANDIDATE_SPACING,
            min_candidate_progress: DEFAULT_MIN_PROGRESS,
            body_width: LOCAL_PLAYER_STANDING_WIDTH,
            body_height: LOCAL_PLAYER_STANDING_HEIGHT,
            max_step_up_blocks: DEFAULT_MAX_STEP_UP,
            max_drop_blocks: DEFAULT_MAX_DROP,
            max_visited_nodes: DEFAULT_MAX_VISITED_NODES,
            marker_height: DEFAULT_MARKER_HEIGHT,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeleportValidityReason {
    Valid,
    InvalidIntent,
    StartUnloaded,
    StartBlocked,
    StartUnsupported,
    NoCandidate,
    NoReachableCandidate,
}

impl TeleportValidityReason {
    pub const fn is_valid(self) -> bool {
        matches!(self, Self::Valid)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeleportResolverDiagnostics {
    pub candidate_count: usize,
    pub searched_candidates: usize,
    pub path: Option<PathSearchDiagnostics>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeleportPreview {
    pub validity: TeleportValidityReason,
    pub target_feet: Option<Vec3d>,
    pub target_yaw_degrees: f64,
    pub marker_dot: Option<Vec3d>,
    pub coarse_target: Option<BlockPos>,
    pub path: Vec<BlockPos>,
    pub arc_points: Vec<Vec3d>,
    pub diagnostics: TeleportResolverDiagnostics,
}

impl TeleportPreview {
    fn invalid(
        validity: TeleportValidityReason,
        target_yaw_degrees: f64,
        arc_points: Vec<Vec3d>,
        diagnostics: TeleportResolverDiagnostics,
    ) -> Self {
        Self {
            validity,
            target_feet: None,
            target_yaw_degrees,
            marker_dot: None,
            coarse_target: None,
            path: Vec::new(),
            arc_points,
            diagnostics,
        }
    }

    pub const fn is_valid(&self) -> bool {
        self.validity.is_valid()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SearchBasis {
    start_feet: Vec3d,
    origin: Vec3d,
    forward: Vec3d,
    right: Vec3d,
    aim_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Candidate {
    node: BlockPos,
    feet: Vec3d,
    progress: f64,
    lateral_distance: f64,
    vertical_delta: i32,
    score: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StandFailure {
    Unloaded,
    Blocked,
    Unsupported,
}

pub fn resolve_teleport_preview<W>(
    world: &W,
    intent: TeleportIntent,
    config: TeleportConfig,
) -> TeleportPreview
where
    W: TeleportCollisionWorld + ?Sized,
{
    let config = sanitize_config(config);
    let Some(basis) = SearchBasis::new(intent) else {
        return TeleportPreview::invalid(
            TeleportValidityReason::InvalidIntent,
            intent.current_yaw_degrees,
            Vec::new(),
            TeleportResolverDiagnostics {
                candidate_count: 0,
                searched_candidates: 0,
                path: None,
            },
        );
    };

    let arc_points = build_intent_arc(basis, config);
    let target_yaw_degrees = yaw_degrees_from_forward(basis.forward);
    let start_node = BlockPos::containing(intent.start_feet);
    if let Err(reason) = validate_standing_pose(world, intent.start_feet, config) {
        return TeleportPreview::invalid(
            start_failure_reason(reason),
            target_yaw_degrees,
            arc_points,
            TeleportResolverDiagnostics {
                candidate_count: 0,
                searched_candidates: 0,
                path: None,
            },
        );
    }

    let candidates = collect_candidates(world, basis, config);
    if candidates.is_empty() {
        return TeleportPreview::invalid(
            TeleportValidityReason::NoCandidate,
            target_yaw_degrees,
            arc_points,
            TeleportResolverDiagnostics {
                candidate_count: 0,
                searched_candidates: 0,
                path: None,
            },
        );
    }

    let search = PathSearch::new(PathSearchLimits {
        max_visited_nodes: config.max_visited_nodes,
    });
    let mut diagnostics = TeleportResolverDiagnostics {
        candidate_count: candidates.len(),
        searched_candidates: 0,
        path: None,
    };

    for candidate in candidates.iter().rev() {
        diagnostics.searched_candidates += 1;
        let path = search.find_path(
            PathSearchQuery::new(start_node, candidate.node)
                .with_follow_range(config.max_distance as f32 + config.max_drop_blocks as f32 + 2.0)
                .with_heuristic_weight(DEFAULT_PATH_HEURISTIC_WEIGHT),
            |node| path_neighbors(world, basis, config, node),
            |pos, target| pos == target,
        );
        diagnostics.path = Some(path.diagnostics());

        let Some(&resolved_node) = path.nodes().last() else {
            continue;
        };
        let resolved_progress = node_progress(basis, resolved_node);
        if resolved_progress < config.min_candidate_progress {
            continue;
        }
        let Some(resolved_feet) = refine_feet_in_node(world, resolved_node, candidate.feet, config)
        else {
            continue;
        };

        let marker_dot = marker_dot_for(resolved_feet, resolved_progress, basis, config);
        return TeleportPreview {
            validity: TeleportValidityReason::Valid,
            target_feet: Some(resolved_feet),
            target_yaw_degrees,
            marker_dot: Some(marker_dot),
            coarse_target: Some(resolved_node),
            path: path.into_nodes(),
            arc_points,
            diagnostics,
        };
    }

    TeleportPreview::invalid(
        TeleportValidityReason::NoReachableCandidate,
        target_yaw_degrees,
        arc_points,
        diagnostics,
    )
}

impl SearchBasis {
    fn new(intent: TeleportIntent) -> Option<Self> {
        if !intent.start_feet.is_finite()
            || !intent.aim_origin.is_finite()
            || !intent.aim_direction.is_finite()
        {
            return None;
        }
        let aim = normalize_or_zero(intent.aim_direction);
        let horizontal = Vec3d::new(aim.x, 0.0, aim.z);
        let forward = normalize_or_zero(horizontal);
        if forward == Vec3d::ZERO {
            return None;
        }
        let right = Vec3d::new(forward.z, 0.0, -forward.x);
        Some(Self {
            start_feet: intent.start_feet,
            origin: intent.aim_origin,
            forward,
            right,
            aim_y: aim.y.clamp(-0.75, 0.75),
        })
    }
}

fn sanitize_config(mut config: TeleportConfig) -> TeleportConfig {
    if !config.max_distance.is_finite() || config.max_distance <= 0.0 {
        config.max_distance = DEFAULT_MAX_DISTANCE;
    }
    if !config.arc_height.is_finite() || config.arc_height < 0.0 {
        config.arc_height = DEFAULT_ARC_HEIGHT;
    }
    config.arc_samples = config.arc_samples.max(2);
    if !config.candidate_radius.is_finite() || config.candidate_radius < 0.0 {
        config.candidate_radius = DEFAULT_CANDIDATE_RADIUS;
    }
    if !config.candidate_spacing.is_finite() || config.candidate_spacing <= 0.0 {
        config.candidate_spacing = DEFAULT_CANDIDATE_SPACING;
    }
    if !config.min_candidate_progress.is_finite() || config.min_candidate_progress < 0.0 {
        config.min_candidate_progress = DEFAULT_MIN_PROGRESS;
    }
    if !config.body_width.is_finite() || config.body_width <= 0.0 {
        config.body_width = LOCAL_PLAYER_STANDING_WIDTH;
    }
    if !config.body_height.is_finite() || config.body_height <= 0.0 {
        config.body_height = LOCAL_PLAYER_STANDING_HEIGHT;
    }
    config.max_step_up_blocks = config.max_step_up_blocks.max(0);
    config.max_drop_blocks = config.max_drop_blocks.max(0);
    config.max_visited_nodes = config.max_visited_nodes.max(1);
    if !config.marker_height.is_finite() || config.marker_height <= 0.0 {
        config.marker_height = DEFAULT_MARKER_HEIGHT;
    }
    config
}

fn build_intent_arc(basis: SearchBasis, config: TeleportConfig) -> Vec<Vec3d> {
    let mut points = Vec::with_capacity(config.arc_samples + 1);
    for index in 0..=config.arc_samples {
        let t = index as f64 / config.arc_samples as f64;
        points.push(arc_point_at_t(basis, config, t));
    }
    points
}

fn arc_point_at_t(basis: SearchBasis, config: TeleportConfig, t: f64) -> Vec3d {
    let progress = config.max_distance * t;
    let arc_lift = config.arc_height * 4.0 * t * (1.0 - t);
    basis
        .origin
        .add(basis.forward.scale(progress))
        .add(Vec3d::new(
            0.0,
            basis.aim_y * config.arc_height * t + arc_lift,
            0.0,
        ))
}

fn collect_candidates<W>(world: &W, basis: SearchBasis, config: TeleportConfig) -> Vec<Candidate>
where
    W: TeleportCollisionWorld + ?Sized,
{
    let mut by_node: BTreeMap<BlockPos, Candidate> = BTreeMap::new();
    let lateral_steps = (config.candidate_radius / config.candidate_spacing).ceil() as i32;
    let start_y = BlockPos::containing(basis.start_feet).y;

    for sample in 1..=config.arc_samples {
        let t = sample as f64 / config.arc_samples as f64;
        let arc = arc_point_at_t(basis, config, t);
        let progress = config.max_distance * t;
        if progress < config.min_candidate_progress {
            continue;
        }
        for lateral_index in -lateral_steps..=lateral_steps {
            let lateral_offset = lateral_index as f64 * config.candidate_spacing;
            if lateral_offset.abs() > config.candidate_radius + TELEPORT_EPSILON {
                continue;
            }
            let preferred = arc.add(basis.right.scale(lateral_offset));
            for foot_y in (start_y - config.max_drop_blocks)..=(start_y + config.max_step_up_blocks)
            {
                let preferred_feet = Vec3d::new(preferred.x, foot_y as f64, preferred.z);
                let node = BlockPos::containing(preferred_feet);
                if !node_inside_search_window(basis, config, node) {
                    continue;
                }
                let Some(feet) = refine_feet_in_node(world, node, preferred_feet, config) else {
                    continue;
                };
                let progress = progress_for_feet(basis, feet);
                if progress < config.min_candidate_progress {
                    continue;
                }
                let lateral_distance = lateral_distance_for_feet(basis, feet);
                let vertical_delta = node.y - start_y;
                let score = candidate_score(progress, lateral_distance, vertical_delta);
                let candidate = Candidate {
                    node,
                    feet,
                    progress,
                    lateral_distance,
                    vertical_delta,
                    score,
                };
                by_node
                    .entry(node)
                    .and_modify(|existing| {
                        if candidate.score > existing.score {
                            *existing = candidate;
                        }
                    })
                    .or_insert(candidate);
            }
        }
    }

    let mut candidates = by_node.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.score
            .total_cmp(&right.score)
            .then_with(|| left.progress.total_cmp(&right.progress))
            .then_with(|| right.lateral_distance.total_cmp(&left.lateral_distance))
            .then_with(|| right.vertical_delta.cmp(&left.vertical_delta))
            .then_with(|| right.node.cmp(&left.node))
    });
    candidates
}

fn path_neighbors<W>(
    world: &W,
    basis: SearchBasis,
    config: TeleportConfig,
    node: BlockPos,
) -> Vec<PathNeighbor>
where
    W: TeleportCollisionWorld + ?Sized,
{
    let mut neighbors = Vec::with_capacity(12);
    for (dx, dz) in [(1, 0), (0, -1), (0, 1), (-1, 0)] {
        let x = node.x + dx;
        let z = node.z + dz;
        for y in (node.y - config.max_drop_blocks)..=(node.y + config.max_step_up_blocks) {
            let next = BlockPos::new(x, y, z);
            if !node_inside_search_window(basis, config, next) {
                continue;
            }
            let preferred = node_center(next);
            if validate_standing_pose(world, preferred, config).is_err() {
                continue;
            }
            let vertical_cost = (next.y - node.y).abs() as f32 * 0.5;
            neighbors.push(PathNeighbor::new(next, vertical_cost));
        }
    }
    neighbors
}

fn node_inside_search_window(basis: SearchBasis, config: TeleportConfig, node: BlockPos) -> bool {
    let progress = node_progress(basis, node);
    let lateral = node_lateral_distance(basis, node);
    progress >= -0.5
        && progress <= config.max_distance + 0.75
        && lateral <= config.candidate_radius + 0.75
        && node.y >= BlockPos::containing(basis.start_feet).y - config.max_drop_blocks
        && node.y <= BlockPos::containing(basis.start_feet).y + config.max_step_up_blocks
}

fn validate_standing_pose<W>(
    world: &W,
    feet: Vec3d,
    config: TeleportConfig,
) -> Result<(), StandFailure>
where
    W: TeleportCollisionWorld + ?Sized,
{
    if !feet.is_finite() {
        return Err(StandFailure::Unloaded);
    }
    let body = player_body_aabb(feet, config);
    if !area_loaded(world, body) {
        return Err(StandFailure::Unloaded);
    }
    if !mclone_blocks::solid_block_aabbs_in(|pos| world.block_state_at(pos), body).is_empty() {
        return Err(StandFailure::Blocked);
    }

    let support = support_probe(body, feet.y);
    if !area_loaded(world, support) {
        return Err(StandFailure::Unloaded);
    }
    if mclone_blocks::solid_block_aabbs_in(|pos| world.block_state_at(pos), support).is_empty() {
        return Err(StandFailure::Unsupported);
    }

    Ok(())
}

fn refine_feet_in_node<W>(
    world: &W,
    node: BlockPos,
    preferred_feet: Vec3d,
    config: TeleportConfig,
) -> Option<Vec3d>
where
    W: TeleportCollisionWorld + ?Sized,
{
    let preferred_x = preferred_feet.x.clamp(
        node.x as f64 + TELEPORT_EPSILON,
        node.x as f64 + 1.0 - TELEPORT_EPSILON,
    );
    let preferred_z = preferred_feet.z.clamp(
        node.z as f64 + TELEPORT_EPSILON,
        node.z as f64 + 1.0 - TELEPORT_EPSILON,
    );
    let center = node_center(node);
    let mut probes = vec![Vec3d::new(preferred_x, node.y as f64, preferred_z), center];
    for radius in [0.15, 0.3] {
        for (dx, dz) in [
            (radius, 0.0),
            (-radius, 0.0),
            (0.0, radius),
            (0.0, -radius),
            (radius, radius),
            (radius, -radius),
            (-radius, radius),
            (-radius, -radius),
        ] {
            probes.push(Vec3d::new(
                (preferred_x + dx).clamp(
                    node.x as f64 + TELEPORT_EPSILON,
                    node.x as f64 + 1.0 - TELEPORT_EPSILON,
                ),
                node.y as f64,
                (preferred_z + dz).clamp(
                    node.z as f64 + TELEPORT_EPSILON,
                    node.z as f64 + 1.0 - TELEPORT_EPSILON,
                ),
            ));
        }
    }

    probes.sort_by(|left, right| {
        left.distance_to_sqr(preferred_feet)
            .total_cmp(&right.distance_to_sqr(preferred_feet))
    });
    probes.dedup_by(|left, right| left.distance_to_sqr(*right) < 1.0e-12);
    probes
        .into_iter()
        .find(|feet| validate_standing_pose(world, *feet, config).is_ok())
}

fn area_loaded<W>(world: &W, area: Aabb) -> bool
where
    W: TeleportCollisionWorld + ?Sized,
{
    if !area.is_finite() {
        return false;
    }
    let min_x = area.min_x.floor() as i32;
    let min_y = area.min_y.floor() as i32;
    let min_z = area.min_z.floor() as i32;
    let max_x = area.max_x.floor() as i32;
    let max_y = area.max_y.floor() as i32;
    let max_z = area.max_z.floor() as i32;
    for y in min_y..=max_y {
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                if world.block_state_at(BlockPos::new(x, y, z)).is_none() {
                    return false;
                }
            }
        }
    }
    true
}

fn player_body_aabb(feet: Vec3d, config: TeleportConfig) -> Aabb {
    mclone_blocks::collision_aabb_for_feet_position(feet, config.body_width, config.body_height)
}

fn support_probe(body: Aabb, feet_y: f64) -> Aabb {
    Aabb::new(
        body.min_x + TELEPORT_EPSILON,
        feet_y - SUPPORT_PROBE_DEPTH,
        body.min_z + TELEPORT_EPSILON,
        body.max_x - TELEPORT_EPSILON,
        feet_y,
        body.max_z - TELEPORT_EPSILON,
    )
}

fn candidate_score(progress: f64, lateral_distance: f64, vertical_delta: i32) -> f64 {
    progress * 10.0 - lateral_distance * 2.0 - (vertical_delta.abs() as f64 * 0.75)
}

fn marker_dot_for(feet: Vec3d, progress: f64, basis: SearchBasis, config: TeleportConfig) -> Vec3d {
    let t = (progress / config.max_distance).clamp(0.0, 1.0);
    let arc_y = arc_point_at_t(basis, config, t).y;
    Vec3d::new(feet.x, arc_y.max(feet.y + config.marker_height), feet.z)
}

fn progress_for_feet(basis: SearchBasis, feet: Vec3d) -> f64 {
    let delta = feet.subtract(basis.start_feet);
    dot_xz(delta, basis.forward)
}

fn lateral_distance_for_feet(basis: SearchBasis, feet: Vec3d) -> f64 {
    dot_xz(feet.subtract(basis.origin), basis.right).abs()
}

fn node_progress(basis: SearchBasis, node: BlockPos) -> f64 {
    progress_for_feet(basis, node_center(node))
}

fn node_lateral_distance(basis: SearchBasis, node: BlockPos) -> f64 {
    lateral_distance_for_feet(basis, node_center(node))
}

fn node_center(node: BlockPos) -> Vec3d {
    Vec3d::new(node.x as f64 + 0.5, node.y as f64, node.z as f64 + 0.5)
}

fn dot_xz(left: Vec3d, right: Vec3d) -> f64 {
    left.x * right.x + left.z * right.z
}

fn yaw_degrees_from_forward(forward: Vec3d) -> f64 {
    -forward.x.atan2(forward.z).to_degrees()
}

fn start_failure_reason(reason: StandFailure) -> TeleportValidityReason {
    match reason {
        StandFailure::Unloaded => TeleportValidityReason::StartUnloaded,
        StandFailure::Blocked => TeleportValidityReason::StartBlocked,
        StandFailure::Unsupported => TeleportValidityReason::StartUnsupported,
    }
}

fn normalize_or_zero(value: Vec3d) -> Vec3d {
    let len_sqr = value.length_sqr();
    if len_sqr <= TELEPORT_EPSILON * TELEPORT_EPSILON {
        Vec3d::ZERO
    } else {
        value.scale(1.0 / len_sqr.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::AIR_BLOCK_STATE_ID;

    const SOLID: BlockStateId = BlockStateId(7);

    #[derive(Clone, Debug)]
    struct TestWorld {
        min: BlockPos,
        max: BlockPos,
        blocks: BTreeMap<BlockPos, BlockStateId>,
    }

    impl TestWorld {
        fn loaded(min: BlockPos, max: BlockPos) -> Self {
            Self {
                min,
                max,
                blocks: BTreeMap::new(),
            }
        }

        fn with_flat_floor(
            x: std::ops::RangeInclusive<i32>,
            z: std::ops::RangeInclusive<i32>,
        ) -> Self {
            let min = BlockPos::new(*x.start(), 0, *z.start());
            let max = BlockPos::new(*x.end(), 5, *z.end());
            let mut world = Self::loaded(min, max);
            for x in x {
                for z in z.clone() {
                    world.set_solid(BlockPos::new(x, 0, z));
                }
            }
            world
        }

        fn set_solid(&mut self, pos: BlockPos) {
            self.blocks.insert(pos, SOLID);
        }

        fn fill_wall(
            &mut self,
            x: i32,
            z: std::ops::RangeInclusive<i32>,
            y: std::ops::RangeInclusive<i32>,
        ) {
            for z in z {
                for y in y.clone() {
                    self.set_solid(BlockPos::new(x, y, z));
                }
            }
        }
    }

    impl TeleportCollisionWorld for TestWorld {
        fn block_state_at(&self, pos: BlockPos) -> Option<BlockStateId> {
            if pos.x < self.min.x
                || pos.x > self.max.x
                || pos.y < self.min.y
                || pos.y > self.max.y
                || pos.z < self.min.z
                || pos.z > self.max.z
            {
                return None;
            }
            Some(*self.blocks.get(&pos).unwrap_or(&AIR_BLOCK_STATE_ID))
        }
    }

    fn config(max_distance: f64) -> TeleportConfig {
        TeleportConfig {
            max_distance,
            arc_height: 1.0,
            arc_samples: 20,
            candidate_radius: 1.25,
            candidate_spacing: 0.5,
            min_candidate_progress: 0.75,
            max_visited_nodes: 256,
            ..Default::default()
        }
    }

    fn forward_intent(max_distance: f64) -> (TeleportIntent, TeleportConfig) {
        (
            TeleportIntent::new(
                Vec3d::new(0.5, 1.0, 0.5),
                Vec3d::new(0.5, 2.2, 0.5),
                Vec3d::new(1.0, 0.0, 0.0),
                -90.0,
            ),
            config(max_distance),
        )
    }

    fn assert_approx_eq(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1.0e-9,
            "expected {actual} to be approximately {expected}"
        );
    }

    #[test]
    fn wall_aim_lands_before_obstruction() {
        let mut world = TestWorld::with_flat_floor(-2..=8, -3..=3);
        world.fill_wall(3, -2..=2, 1..=3);
        let (intent, config) = forward_intent(6.0);

        let preview = resolve_teleport_preview(&world, intent, config);

        assert_eq!(preview.validity, TeleportValidityReason::Valid);
        let feet = preview.target_feet.expect("target feet");
        assert!(feet.x <= 2.7, "feet should stop before the wall: {feet:?}");
        assert_eq!(preview.coarse_target, Some(BlockPos::new(2, 1, 0)));
        let marker = preview.marker_dot.expect("marker dot");
        assert_approx_eq(marker.x, feet.x);
        assert_approx_eq(marker.z, feet.z);
    }

    #[test]
    fn low_opening_is_treated_as_blocked_for_player_body() {
        let mut world = TestWorld::with_flat_floor(-2..=8, -3..=3);
        world.fill_wall(3, -2..=2, 1..=3);
        world.blocks.remove(&BlockPos::new(3, 1, 0));
        let (intent, config) = forward_intent(6.0);

        let preview = resolve_teleport_preview(&world, intent, config);

        assert_eq!(preview.validity, TeleportValidityReason::Valid);
        let feet = preview.target_feet.expect("target feet");
        assert!(
            feet.x <= 2.7,
            "feet should not pass through a one-block-high opening: {feet:?}"
        );
        assert_eq!(preview.coarse_target, Some(BlockPos::new(2, 1, 0)));
    }

    #[test]
    fn one_block_up_landing_is_valid_with_support_and_headroom() {
        let mut world = TestWorld::with_flat_floor(-1..=3, -2..=2);
        world.set_solid(BlockPos::new(2, 1, 0));
        let (intent, config) = forward_intent(2.2);

        let preview = resolve_teleport_preview(&world, intent, config);

        assert_eq!(preview.validity, TeleportValidityReason::Valid);
        let feet = preview.target_feet.expect("target feet");
        assert_eq!(preview.coarse_target, Some(BlockPos::new(2, 2, 0)));
        assert_approx_eq(feet.y, 2.0);
    }

    #[test]
    fn unloaded_window_without_forward_candidate_is_invalid() {
        let world = TestWorld::with_flat_floor(0..=0, 0..=0);
        let (intent, config) = forward_intent(5.0);

        let preview = resolve_teleport_preview(&world, intent, config);

        assert_eq!(preview.validity, TeleportValidityReason::NoCandidate);
        assert!(!preview.is_valid());
        assert_eq!(preview.target_feet, None);
    }

    #[test]
    fn refined_feet_pose_can_be_off_block_center() {
        let world = TestWorld::with_flat_floor(-1..=5, -2..=2);
        let intent = TeleportIntent::new(
            Vec3d::new(0.5, 1.0, 0.5),
            Vec3d::new(0.5, 2.2, 0.2),
            Vec3d::new(1.0, 0.0, -0.05),
            -90.0,
        );

        let preview = resolve_teleport_preview(&world, intent, config(3.0));

        assert_eq!(preview.validity, TeleportValidityReason::Valid);
        let feet = preview.target_feet.expect("target feet");
        let block_center_z = feet.z.floor() + 0.5;
        assert!(
            (feet.z - block_center_z).abs() > 0.1,
            "target feet should keep the continuous preferred placement: {feet:?}"
        );
    }
}
