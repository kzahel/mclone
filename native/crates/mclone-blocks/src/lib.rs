#![forbid(unsafe_code)]

mod block_clip;
pub mod block_facts;
pub mod block_shapes;
mod collision;

pub use block_facts::{
    BlockFluidKind, DEFAULT_BLOCK_FRICTION, DEFAULT_BLOCK_JUMP_FACTOR, DEFAULT_BLOCK_SPEED_FACTOR,
    LAVA_BLOCK_STATE_ID, WATER_BLOCK_STATE_ID, block_fluid_height, block_fluid_kind,
    block_friction, block_jump_factor, block_speed_factor, is_fluid, terrain_id,
};
pub use block_shapes::{
    block_collision_aabb, block_collision_aabbs, block_outline_aabbs, clip_block_outline,
};
pub use collision::{
    CollisionMovementResult, collide_movement, collide_movement_result,
    collision_aabb_for_feet_position, solid_block_aabbs_in,
};
