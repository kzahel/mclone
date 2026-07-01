#![forbid(unsafe_code)]

mod block_clip;
pub mod block_facts;
pub mod block_shapes;
mod collision;

pub use block_facts::{
    BlockFluidKind, LAVA_BLOCK_STATE_ID, WATER_BLOCK_STATE_ID, block_fluid_height,
    block_fluid_kind, is_fluid, terrain_id,
};
pub use block_shapes::{block_collision_aabb, block_outline_aabbs, clip_block_outline};
pub use collision::{
    CollisionMovementResult, collide_movement, collide_movement_result,
    collision_aabb_for_feet_position, solid_block_aabbs_in,
};
