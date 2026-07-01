use mclone_core::{BlockPos, BlockStateId, Vec3d};
use mclone_worldgen::prng::SimpleRandomSource;

use super::{BlockPathType, GroundPathNavigation, WalkNodeEvaluator};
use crate::game_mode::JAVA_OVERWORLD_MAX_BUILD_HEIGHT;

const JAVA_OVERWORLD_MIN_BUILD_HEIGHT: i32 = 0;
const RANDOM_POS_ATTEMPTS: usize = 10;

pub(crate) fn default_random_pos<F, M>(
    position: Vec3d,
    horizontal_range: i32,
    vertical_range: i32,
    random: &mut SimpleRandomSource,
    navigation: &GroundPathNavigation,
    block_state_at: &F,
    pathfinding_malus: M,
) -> Option<Vec3d>
where
    F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    M: Fn(BlockPathType) -> f32 + Copy,
{
    generate_random_pos(|| {
        let direction = generate_random_direction(random, horizontal_range, vertical_range);
        let candidate = generate_random_pos_toward_direction(position, direction);
        (!is_outside_limits(candidate)
            && navigation.is_stable_destination(block_state_at, candidate)
            && !WalkNodeEvaluator::has_malus(block_state_at, pathfinding_malus, candidate))
        .then_some(candidate)
    })
}

pub(crate) fn land_random_pos<F, M>(
    position: Vec3d,
    horizontal_range: i32,
    vertical_range: i32,
    random: &mut SimpleRandomSource,
    navigation: &GroundPathNavigation,
    block_state_at: &F,
    pathfinding_malus: M,
) -> Option<Vec3d>
where
    F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
    M: Fn(BlockPathType) -> f32 + Copy,
{
    generate_random_pos(|| {
        let direction = generate_random_direction(random, horizontal_range, vertical_range);
        let candidate = generate_random_pos_toward_direction(position, direction);
        if is_outside_limits(candidate)
            || !navigation.is_stable_destination(block_state_at, candidate)
        {
            return None;
        }
        let candidate = move_pos_up_out_of_solid(candidate, block_state_at);
        (!WalkNodeEvaluator::is_water(block_state_at, candidate)
            && !WalkNodeEvaluator::has_malus(block_state_at, pathfinding_malus, candidate))
        .then_some(candidate)
    })
}

fn generate_random_pos(mut supplier: impl FnMut() -> Option<BlockPos>) -> Option<Vec3d> {
    let mut best: Option<BlockPos> = None;
    let mut best_weight = f64::NEG_INFINITY;
    for _ in 0..RANDOM_POS_ATTEMPTS {
        if let Some(candidate) = supplier() {
            let weight = 0.0;
            if weight > best_weight {
                best_weight = weight;
                best = Some(candidate);
            }
        }
    }
    best.map(at_bottom_center_of)
}

fn generate_random_direction(
    random: &mut SimpleRandomSource,
    horizontal_range: i32,
    vertical_range: i32,
) -> BlockPos {
    let x = random.next_int_bound(2 * horizontal_range + 1) - horizontal_range;
    let y = random.next_int_bound(2 * vertical_range + 1) - vertical_range;
    let z = random.next_int_bound(2 * horizontal_range + 1) - horizontal_range;
    BlockPos::new(x, y, z)
}

fn generate_random_pos_toward_direction(position: Vec3d, direction: BlockPos) -> BlockPos {
    BlockPos::containing(Vec3d::new(
        position.x + direction.x as f64,
        position.y + direction.y as f64,
        position.z + direction.z as f64,
    ))
}

fn move_pos_up_out_of_solid<F>(mut pos: BlockPos, block_state_at: &F) -> BlockPos
where
    F: Fn(BlockPos) -> Option<BlockStateId> + ?Sized,
{
    if !WalkNodeEvaluator::is_solid(block_state_at, pos) {
        return pos;
    }
    pos = pos.offset(0, 1, 0);
    while pos.y < JAVA_OVERWORLD_MAX_BUILD_HEIGHT
        && WalkNodeEvaluator::is_solid(block_state_at, pos)
    {
        pos = pos.offset(0, 1, 0);
    }
    pos
}

fn is_outside_limits(pos: BlockPos) -> bool {
    pos.y < JAVA_OVERWORLD_MIN_BUILD_HEIGHT || pos.y > JAVA_OVERWORLD_MAX_BUILD_HEIGHT
}

fn at_bottom_center_of(pos: BlockPos) -> Vec3d {
    Vec3d::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5)
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

    fn water_pool(pos: BlockPos) -> Option<BlockStateId> {
        if pos.y == 63 {
            Some(BlockStateId(1))
        } else if pos == BlockPos::new(8, 64, 8) {
            Some(mclone_blocks::WATER_BLOCK_STATE_ID)
        } else {
            Some(BlockStateId(mclone_blocks::terrain_id::AIR))
        }
    }

    #[test]
    fn land_random_pos_rejects_candidates_without_stable_floor() {
        let mut random = SimpleRandomSource::new(1);
        let navigation = GroundPathNavigation::default();

        assert_eq!(
            land_random_pos(
                Vec3d::new(8.5, 64.0, 8.5),
                10,
                7,
                &mut random,
                &navigation,
                &no_blocks,
                BlockPathType::default_malus,
            ),
            None
        );
    }

    #[test]
    fn land_random_pos_returns_bottom_center_of_stable_target() {
        let mut random = SimpleRandomSource::new(1);
        let navigation = GroundPathNavigation::default();

        let target = land_random_pos(
            Vec3d::new(8.5, 64.0, 8.5),
            10,
            0,
            &mut random,
            &navigation,
            &flat_ground,
            BlockPathType::default_malus,
        )
        .expect("flat ground should offer a target");

        assert_eq!(target.y, 64.0);
        assert_eq!(target.x - target.x.floor(), 0.5);
        assert_eq!(target.z - target.z.floor(), 0.5);
    }

    #[test]
    fn land_random_pos_rejects_water_even_when_malus_is_zero() {
        let mut random = SimpleRandomSource::new(149);
        let navigation = GroundPathNavigation::default();
        let target = land_random_pos(
            Vec3d::new(8.5, 64.0, 8.5),
            0,
            0,
            &mut random,
            &navigation,
            &water_pool,
            |_| 0.0,
        );

        assert_eq!(target, None);
    }
}
