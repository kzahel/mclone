use mclone_blocks::{BlockFluidKind, block_fluid_height, block_fluid_kind};
use mclone_core::{Aabb, BlockPos, BlockStateId, Vec3d};
use mclone_protocol::{PLAYER_STANDING_HEIGHT, PLAYER_STANDING_WIDTH};

const PLAYER_FLUID_CONTACT_EPSILON: f64 = 0.001;

pub(crate) fn player_body_touches_lava(
    feet_position: Vec3d,
    mut block_state_at: impl FnMut(BlockPos) -> Option<BlockStateId>,
) -> bool {
    let body = mclone_blocks::collision_aabb_for_feet_position(
        feet_position,
        PLAYER_STANDING_WIDTH,
        PLAYER_STANDING_HEIGHT,
    );
    let body = Aabb::new(
        body.min_x + PLAYER_FLUID_CONTACT_EPSILON,
        body.min_y + PLAYER_FLUID_CONTACT_EPSILON,
        body.min_z + PLAYER_FLUID_CONTACT_EPSILON,
        body.max_x - PLAYER_FLUID_CONTACT_EPSILON,
        body.max_y - PLAYER_FLUID_CONTACT_EPSILON,
        body.max_z - PLAYER_FLUID_CONTACT_EPSILON,
    );
    let min_x = body.min_x.floor() as i32;
    let min_y = body.min_y.floor() as i32;
    let min_z = body.min_z.floor() as i32;
    let max_x = body.max_x.floor() as i32;
    let max_y = body.max_y.floor() as i32;
    let max_z = body.max_z.floor() as i32;
    for y in min_y..=max_y {
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                let pos = BlockPos::new(x, y, z);
                let Some(state) = block_state_at(pos) else {
                    continue;
                };
                if block_fluid_kind(state) != BlockFluidKind::Lava {
                    continue;
                }
                let Some(height) = block_fluid_height(state) else {
                    continue;
                };
                let fluid = Aabb::new(
                    f64::from(x),
                    f64::from(y),
                    f64::from(z),
                    f64::from(x + 1),
                    f64::from(y) + f64::from(height),
                    f64::from(z + 1),
                );
                if body.intersects(fluid) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_blocks::{LAVA_BLOCK_STATE_ID, terrain_id};

    fn state_at_lava_origin(pos: BlockPos, state: BlockStateId) -> Option<BlockStateId> {
        (pos == BlockPos::new(0, 64, 0)).then_some(state)
    }

    #[test]
    fn body_overlap_detects_source_and_flowing_lava() {
        let feet = Vec3d::new(0.5, 64.0, 0.5);
        assert!(player_body_touches_lava(feet, |pos| {
            state_at_lava_origin(pos, LAVA_BLOCK_STATE_ID)
        }));
        for raw in terrain_id::LAVA_LEVEL_1..=terrain_id::LAVA_LEVEL_8 {
            assert!(player_body_touches_lava(feet, |pos| {
                state_at_lava_origin(pos, BlockStateId(raw))
            }));
        }
    }

    #[test]
    fn adjacent_face_and_missing_data_do_not_invent_lava_contact() {
        let merely_touching = Vec3d::new(1.3, 64.0, 0.5);
        assert!(!player_body_touches_lava(merely_touching, |pos| {
            state_at_lava_origin(pos, LAVA_BLOCK_STATE_ID)
        }));
        assert!(!player_body_touches_lava(
            Vec3d::new(0.5, 64.0, 0.5),
            |_| None
        ));
    }
}
