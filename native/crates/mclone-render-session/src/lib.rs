#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Context, Result, bail};
use glam::{Quat, Vec3};
use mclone_assets::ActorFigureId;
use mclone_client::{
    ActorPresentation, ActorPresentationId, ActorPresentationKind, BlockInteractionTarget,
    ClientInteractionController, ClientRuntime, CollisionMovementResult, FlyingMovementStep,
    HAND_PUSH_DEFAULT_HAND_RADIUS, HandPushLocomotionController, HandPushMovementStep,
    HandPushPose, LOCAL_PLAYER_STANDING_EYE_HEIGHT, LOCAL_PLAYER_STANDING_HEIGHT,
    LOCAL_PLAYER_TICKS_PER_SECOND, LocalPlayerController, LocalPlayerDimensions, LocalPlayerPose,
    NoClipMovementStep, PlayerInputKey, ThrusterHandInput, ThrusterMovementStep, ThrusterTuning,
    WalkingMovementStep,
};
use mclone_core::{
    AIR_BLOCK_STATE_ID, Aabb, AxisTopology, BlockHitResult, BlockPos, BlockStateId,
    CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkPos, ChunkSnapshot, HorizontalTopology,
    PackedChunkSection, PackedLightSection, SECTION_HEIGHT, Vec3d, block_to_chunk_coord,
    block_to_section_coord, chunk_block_coord, chunk_middle_block_coord,
};
use mclone_mesh::{
    GrassPatch, RenderSectionKey, TexturedChunkMeshInput, TexturedChunkVertex, TexturedMeshCatalog,
    TexturedRenderSectionBuildOptions, TexturedRenderSectionBuildReport, TexturedRenderSectionMesh,
    TexturedRenderSectionMetadata, TexturedVisibleChunkMesh, VisibilityGraphBuildStats,
    VisibilitySet, build_textured_render_sections_for_section_set_with_stats_and_options,
    build_textured_render_sections_with_stats, quad_face_count_from_indices,
};
use mclone_protocol::{
    ClientCommand, ClientEphemeralMessage, EntityKind, ItemKind, PlayerPositionUpdate,
    SectionBlockUpdate, ServerUpdate,
};
use mclone_render::chunk::{ChunkCamera, PerspectiveRenderPose};
use mclone_render::entity::{ActorInstance, ActorInstanceId};
use mclone_render::gui::WorldGuiLine;

const PACKED_BUILD_REPORT_MAGIC: &[u8; 8] = b"MCRSBR3\0";
pub const LANDING_MIN_IMPACT_SPEED: f64 = 0.5;
const THIRD_PERSON_CAMERA_DISTANCE: f64 = 4.0;

mod camera;
mod compile_queue;
mod dirty;
mod flat_surface_layout;
mod mesh_inputs;
mod ready_plan;
mod resident_tile;
mod section_cache;
mod server_updates;
mod session;
mod upload;
mod view;

pub use camera::*;
pub use compile_queue::*;
pub use dirty::*;
pub use flat_surface_layout::*;
pub use mesh_inputs::*;
pub use ready_plan::*;
pub use resident_tile::*;
pub use section_cache::*;
pub use server_updates::*;
pub use session::*;
pub use upload::*;
pub use view::*;

#[cfg(test)]
pub(crate) use camera::{ENGINE_DEBUG_HAND_SPHERE_SEGMENTS, hand_push_emulation_direction};

#[cfg(test)]
mod tests;
