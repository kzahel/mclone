use mclone_core::{Aabb, BlockPos, BlockStateId, Vec3d};

use crate::block_collision_aabbs;

pub const COLLISION_EPSILON: f64 = 1.0e-7;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CollisionMovementResult {
    pub requested: Vec3d,
    pub traveled: Vec3d,
    pub horizontal_collision: bool,
    pub vertical_collision: bool,
    pub on_ground: bool,
}

pub fn collision_aabb_for_feet_position(position: Vec3d, width: f64, height: f64) -> Aabb {
    let half_width = width / 2.0;
    Aabb::new(
        position.x - half_width,
        position.y,
        position.z - half_width,
        position.x + half_width,
        position.y + height,
        position.z + half_width,
    )
}

pub fn collide_movement<F>(block_state_at: F, bounding_box: Aabb, movement: Vec3d) -> Vec3d
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    if !bounding_box.is_finite() || !movement.is_finite() || movement.length_sqr() == 0.0 {
        return Vec3d::ZERO;
    }

    let solid_blocks = solid_block_aabbs_in(block_state_at, bounding_box.expand_towards(movement));
    collide_with_aabbs(bounding_box, movement, &solid_blocks)
}

pub fn collide_movement_result(requested: Vec3d, traveled: Vec3d) -> CollisionMovementResult {
    CollisionMovementResult {
        requested,
        traveled,
        horizontal_collision: !nearly_equal(requested.x, traveled.x)
            || !nearly_equal(requested.z, traveled.z),
        vertical_collision: !nearly_equal(requested.y, traveled.y),
        on_ground: !nearly_equal(requested.y, traveled.y) && requested.y < 0.0,
    }
}

pub fn solid_block_aabbs_in<F>(block_state_at: F, area: Aabb) -> Vec<Aabb>
where
    F: Fn(BlockPos) -> Option<BlockStateId>,
{
    if !area.is_finite() {
        return Vec::new();
    }

    let min_x = area.min_x.floor() as i32;
    let min_y = area.min_y.floor() as i32;
    let min_z = area.min_z.floor() as i32;
    let max_x = area.max_x.floor() as i32;
    let max_y = area.max_y.floor() as i32;
    let max_z = area.max_z.floor() as i32;
    let mut solids = Vec::new();
    for y in min_y..=max_y {
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                let pos = BlockPos::new(x, y, z);
                let Some(block_state) = block_state_at(pos) else {
                    continue;
                };
                for block_box in block_collision_aabbs(block_state, pos) {
                    if block_box.intersects(area) {
                        solids.push(block_box);
                    }
                }
            }
        }
    }
    solids
}

fn collide_with_aabbs(mut bounding_box: Aabb, movement: Vec3d, solids: &[Aabb]) -> Vec3d {
    let mut x = movement.x;
    let mut y = movement.y;
    let mut z = movement.z;

    if y != 0.0 {
        y = clip_axis(Axis::Y, bounding_box, solids, y);
        if y != 0.0 {
            bounding_box = bounding_box.move_by(Vec3d::new(0.0, y, 0.0));
        }
    }

    let z_first = x.abs() < z.abs();
    if z_first && z != 0.0 {
        z = clip_axis(Axis::Z, bounding_box, solids, z);
        if z != 0.0 {
            bounding_box = bounding_box.move_by(Vec3d::new(0.0, 0.0, z));
        }
    }

    if x != 0.0 {
        x = clip_axis(Axis::X, bounding_box, solids, x);
        if !z_first && x != 0.0 {
            bounding_box = bounding_box.move_by(Vec3d::new(x, 0.0, 0.0));
        }
    }

    if !z_first && z != 0.0 {
        z = clip_axis(Axis::Z, bounding_box, solids, z);
    }

    Vec3d::new(x, y, z)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Axis {
    X,
    Y,
    Z,
}

fn clip_axis(axis: Axis, bounding_box: Aabb, solids: &[Aabb], mut delta: f64) -> f64 {
    if delta.abs() < COLLISION_EPSILON {
        return 0.0;
    }

    for solid in solids {
        if !overlaps_other_axes(axis, bounding_box, *solid) {
            continue;
        }
        if delta > 0.0 {
            let distance = min_axis(axis, *solid) - max_axis(axis, bounding_box);
            if distance >= -COLLISION_EPSILON && distance < delta {
                delta = distance.max(0.0);
            }
        } else {
            let distance = max_axis(axis, *solid) - min_axis(axis, bounding_box);
            if distance <= COLLISION_EPSILON && distance > delta {
                delta = distance.min(0.0);
            }
        }
    }
    delta
}

fn overlaps_other_axes(axis: Axis, a: Aabb, b: Aabb) -> bool {
    match axis {
        Axis::X => {
            ranges_overlap(a.min_y, a.max_y, b.min_y, b.max_y)
                && ranges_overlap(a.min_z, a.max_z, b.min_z, b.max_z)
        }
        Axis::Y => {
            ranges_overlap(a.min_x, a.max_x, b.min_x, b.max_x)
                && ranges_overlap(a.min_z, a.max_z, b.min_z, b.max_z)
        }
        Axis::Z => {
            ranges_overlap(a.min_x, a.max_x, b.min_x, b.max_x)
                && ranges_overlap(a.min_y, a.max_y, b.min_y, b.max_y)
        }
    }
}

fn ranges_overlap(a_min: f64, a_max: f64, b_min: f64, b_max: f64) -> bool {
    a_min < b_max && a_max > b_min
}

fn min_axis(axis: Axis, aabb: Aabb) -> f64 {
    match axis {
        Axis::X => aabb.min_x,
        Axis::Y => aabb.min_y,
        Axis::Z => aabb.min_z,
    }
}

fn max_axis(axis: Axis, aabb: Aabb) -> f64 {
    match axis {
        Axis::X => aabb.max_x,
        Axis::Y => aabb.max_y,
        Axis::Z => aabb.max_z,
    }
}

fn nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() < COLLISION_EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain_id;

    fn block_at(pos: BlockPos) -> Option<BlockStateId> {
        (pos == BlockPos::ZERO).then_some(BlockStateId(1))
    }

    #[test]
    fn colliding_movement_stops_at_full_block_wall() {
        let box_ = collision_aabb_for_feet_position(Vec3d::new(-1.0, 0.0, 0.5), 0.6, 1.8);
        let traveled = collide_movement(block_at, box_, Vec3d::new(2.0, 0.0, 0.0));
        let result = collide_movement_result(Vec3d::new(2.0, 0.0, 0.0), traveled);

        assert_eq!(traveled, Vec3d::new(0.7, 0.0, 0.0));
        assert!(result.horizontal_collision);
        assert!(!result.vertical_collision);
        assert!(!result.on_ground);
    }

    #[test]
    fn colliding_movement_lands_on_full_block() {
        let box_ = collision_aabb_for_feet_position(Vec3d::new(0.5, 2.0, 0.5), 0.6, 1.8);
        let requested = Vec3d::new(0.0, -3.0, 0.0);
        let traveled = collide_movement(block_at, box_, requested);
        let result = collide_movement_result(requested, traveled);

        assert_eq!(traveled, Vec3d::new(0.0, -1.0, 0.0));
        assert!(!result.horizontal_collision);
        assert!(result.vertical_collision);
        assert!(result.on_ground);
    }

    #[test]
    fn collision_ignores_air_and_empty_collision_blocks() {
        let blocks = |pos: BlockPos| {
            if pos == BlockPos::new(0, 0, 0) {
                Some(BlockStateId(terrain_id::AIR))
            } else if pos == BlockPos::new(1, 0, 0) {
                Some(BlockStateId(terrain_id::GRASS))
            } else {
                None
            }
        };
        let box_ = collision_aabb_for_feet_position(Vec3d::new(-1.0, 0.0, 0.5), 0.6, 1.8);
        let requested = Vec3d::new(2.0, 0.0, 0.0);

        assert_eq!(collide_movement(blocks, box_, requested), requested);
    }
}
