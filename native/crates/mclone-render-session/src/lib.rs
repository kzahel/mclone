#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_client::{
    ActorPresentation, ActorPresentationKind, BlockInteractionTarget, ClientInteractionController,
    ClientRuntime, HAND_PUSH_DEFAULT_HAND_RADIUS, HandPushLocomotionController,
    HandPushMovementStep, HandPushPose, LOCAL_PLAYER_STANDING_EYE_HEIGHT,
    LOCAL_PLAYER_STANDING_HEIGHT, LOCAL_PLAYER_TICKS_PER_SECOND, LocalPlayerController,
    LocalPlayerPose, NoClipMovementStep, PlayerInputKey, WalkingMovementStep,
};
use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockHitResult, BlockPos, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkPos,
    ChunkSnapshot, PackedLightSection, SECTION_HEIGHT, Vec3d, block_to_chunk_coord,
    block_to_section_coord, chunk_block_coord, chunk_middle_block_coord,
};
use mclone_mesh::{
    RenderSectionKey, TexturedChunkMeshInput, TexturedChunkVertex, TexturedMeshCatalog,
    TexturedRenderSectionBuildReport, TexturedRenderSectionMesh, TexturedVisibleChunkMesh,
    VisibilityGraphBuildStats, VisibilitySet,
    build_textured_render_sections_for_section_set_with_stats,
    build_textured_render_sections_with_stats, quad_face_count_from_indices,
};
use mclone_protocol::{
    ClientCommand, EntityKind, PlayerPositionUpdate, SectionBlockUpdate, ServerUpdate,
};
use mclone_render::entity::ActorInstance;

const PACKED_BUILD_REPORT_MAGIC: &[u8; 8] = b"MCRSBR1\0";
pub const LANDING_MIN_IMPACT_SPEED: f64 = 0.5;
const THIRD_PERSON_CAMERA_DISTANCE: f64 = 4.0;

pub fn build_client_textured_sections(
    client: &ClientRuntime,
    catalog: &TexturedMeshCatalog,
) -> Result<TexturedRenderSectionBuildReport> {
    let chunks = mesh_chunks_from_client(client)?;
    let inputs = textured_mesh_inputs(&chunks);
    build_textured_render_sections_with_stats(&inputs, catalog)
        .context("failed to build textured sections")
}

pub fn build_render_sections_from_snapshots<S: std::borrow::Borrow<ChunkSnapshot>>(
    snapshots: &[S],
    catalog: &TexturedMeshCatalog,
    target_sections: &BTreeSet<RenderSectionKey>,
) -> Result<TexturedRenderSectionBuildReport> {
    // Generic over `Borrow<ChunkSnapshot>` so a caller holding owned snapshots
    // (`&[ChunkSnapshot]`, desktop + the web full-view helpers) and one holding borrowed
    // snapshots (`&[&ChunkSnapshot]`, the web worker's resident mirror, 067 Stage 4) both
    // build sections without forcing the latter to deep-clone its resident map every compile.
    let chunks = snapshots
        .iter()
        .map(|snapshot| snapshot_mesh_block_state_ids(snapshot.borrow()))
        .collect::<Result<Vec<_>>>()?;
    let inputs = textured_mesh_inputs(&chunks);
    build_textured_render_sections_for_section_set_with_stats(&inputs, catalog, target_sections)
        .context("failed to build queued textured render sections")
}

pub fn mesh_chunks_from_client(client: &ClientRuntime) -> Result<Vec<MeshChunkBlocks>> {
    client
        .chunk_snapshots()
        .map(snapshot_mesh_block_state_ids)
        .collect::<Result<Vec<_>>>()
}

pub fn textured_mesh_inputs(chunks: &[MeshChunkBlocks]) -> Vec<TexturedChunkMeshInput<'_>> {
    chunks
        .iter()
        .map(|chunk| {
            TexturedChunkMeshInput::new(
                chunk.chunk_x,
                chunk.chunk_z,
                chunk.min_y,
                chunk.height,
                &chunk.blocks,
            )
            .with_light_sections(&chunk.light_sections)
        })
        .collect()
}

pub fn actor_instances_from_presentations(
    presentations: &[ActorPresentation],
    client: &ClientRuntime,
) -> Vec<ActorInstance> {
    presentations
        .iter()
        .map(|actor| {
            let packed_light =
                client.packed_light_at_world_or_fullbright(actor_light_probe_block_pos(actor));
            match actor.kind {
                ActorPresentationKind::RemotePlayer => ActorInstance::remote_player(
                    glam_vec3_from_vec3d(actor.feet_position),
                    actor.y_rot_degrees,
                )
                .with_packed_light(packed_light),
                ActorPresentationKind::Entity(EntityKind::Cow) => ActorInstance::cow_model(
                    glam_vec3_from_vec3d(actor.feet_position),
                    actor.y_rot_degrees,
                    actor.width,
                    actor.height,
                )
                .with_packed_light(packed_light),
                ActorPresentationKind::Entity(EntityKind::Chicken) => {
                    ActorInstance::chicken_placeholder(
                        glam_vec3_from_vec3d(actor.feet_position),
                        actor.y_rot_degrees,
                        actor.width,
                        actor.height,
                    )
                    .with_packed_light(packed_light)
                }
                ActorPresentationKind::Entity(EntityKind::DebugCube) => ActorInstance::debug_cube(
                    glam_vec3_from_vec3d(actor.feet_position),
                    actor.y_rot_degrees,
                    actor.width,
                    actor.height,
                )
                .with_packed_light(packed_light),
            }
        })
        .collect()
}

pub fn local_player_actor_instance(
    camera: &EngineCameraController,
    client: &ClientRuntime,
) -> ActorInstance {
    let pose = camera.player().pose();
    let packed_light = client.packed_light_at_world_or_fullbright(BlockPos::containing(
        pose.position
            .add(Vec3d::new(0.0, LOCAL_PLAYER_STANDING_HEIGHT * 0.5, 0.0)),
    ));
    ActorInstance::local_player(
        glam_vec3_from_vec3d(pose.position),
        pose.y_rot_degrees as f32,
    )
    .with_packed_light(packed_light)
}

pub fn actor_light_probe_block_pos(actor: &ActorPresentation) -> BlockPos {
    BlockPos::containing(actor.feet_position.add(Vec3d::new(
        0.0,
        actor_light_probe_height(actor),
        0.0,
    )))
}

pub fn actor_light_probe_height(actor: &ActorPresentation) -> f64 {
    match actor.kind {
        ActorPresentationKind::RemotePlayer => LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        ActorPresentationKind::Entity(EntityKind::Cow) => 1.3,
        ActorPresentationKind::Entity(EntityKind::Chicken) => f64::from(actor.height) * 0.92,
        ActorPresentationKind::Entity(EntityKind::DebugCube) => f64::from(actor.height) * 0.5,
    }
}

pub fn glam_vec3_from_vec3d(value: Vec3d) -> Vec3 {
    Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

#[derive(Clone, Debug)]
pub struct MeshChunkBlocks {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub height: i32,
    pub blocks: Vec<mclone_core::BlockStateId>,
    pub light_sections: Vec<PackedLightSection>,
}

pub fn snapshot_mesh_block_state_ids(snapshot: &ChunkSnapshot) -> Result<MeshChunkBlocks> {
    if snapshot.height <= 0 || snapshot.height % SECTION_HEIGHT != 0 {
        bail!(
            "chunk snapshot {:?} has invalid height {}",
            snapshot.pos,
            snapshot.height
        );
    }
    let expected_len = snapshot.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
    let mut blocks = vec![AIR_BLOCK_STATE_ID; expected_len];
    let min_section_y = snapshot.min_y / SECTION_HEIGHT;
    let section_count = snapshot.height / SECTION_HEIGHT;

    for section in &snapshot.sections {
        let section_offset = section.section_y - min_section_y;
        if !(0..section_count).contains(&section_offset) {
            bail!(
                "chunk snapshot {:?} contains section {} outside {}..{}",
                snapshot.pos,
                section.section_y,
                min_section_y,
                min_section_y + section_count - 1
            );
        }
        let unpacked = section.unpack_block_state_ids();
        if unpacked.len() != CHUNK_SECTION_VOLUME {
            bail!(
                "chunk snapshot {:?} section {} unpacked to {} blocks",
                snapshot.pos,
                section.section_y,
                unpacked.len()
            );
        }
        let start = section_offset as usize * CHUNK_SECTION_VOLUME;
        for (index, state_id) in unpacked.into_iter().enumerate() {
            blocks[start + index] = state_id;
        }
    }

    Ok(MeshChunkBlocks {
        chunk_x: snapshot.pos.x,
        chunk_z: snapshot.pos.z,
        min_y: snapshot.min_y,
        height: snapshot.height,
        blocks,
        light_sections: snapshot.light_sections.clone(),
    })
}

#[derive(Clone, Debug, Default)]
pub struct CachedTexturedRenderSections {
    sections: BTreeMap<RenderSectionKey, TexturedRenderSectionMesh>,
}

#[derive(Clone, Debug, Default)]
pub struct RenderSectionCacheUpdate {
    pub rebuilt_sections: Vec<TexturedRenderSectionMesh>,
    pub removed_section_keys: BTreeSet<RenderSectionKey>,
    pub rebuilt_vertex_count: u32,
    pub rebuilt_index_count: u32,
    pub neighbor_ready_section_count: usize,
    pub near_exception_section_count: usize,
    pub deferred_section_count: usize,
    pub submitted_compile_section_count: usize,
    pub completed_compile_section_count: usize,
    pub stale_compile_section_count: usize,
    pub pending_compile_jobs: usize,
    pub visibility_graph_stats: VisibilityGraphBuildStats,
}

impl RenderSectionCacheUpdate {
    pub fn rebuilt_section_count(&self) -> usize {
        self.rebuilt_sections.len()
    }

    pub fn removed_section_count(&self) -> usize {
        self.removed_section_keys.len()
    }

    pub fn rebuilt_face_count(&self) -> u32 {
        quad_face_count_from_indices(self.rebuilt_index_count)
    }

    pub fn merge(&mut self, other: Self) {
        self.rebuilt_sections.extend(other.rebuilt_sections);
        self.removed_section_keys.extend(other.removed_section_keys);
        self.rebuilt_vertex_count += other.rebuilt_vertex_count;
        self.rebuilt_index_count += other.rebuilt_index_count;
        self.neighbor_ready_section_count += other.neighbor_ready_section_count;
        self.near_exception_section_count += other.near_exception_section_count;
        self.deferred_section_count += other.deferred_section_count;
        self.submitted_compile_section_count += other.submitted_compile_section_count;
        self.completed_compile_section_count += other.completed_compile_section_count;
        self.stale_compile_section_count += other.stale_compile_section_count;
        self.pending_compile_jobs = other.pending_compile_jobs;
        self.visibility_graph_stats.build_count += other.visibility_graph_stats.build_count;
        self.visibility_graph_stats.total_ms += other.visibility_graph_stats.total_ms;
        self.visibility_graph_stats.worst_ms = self
            .visibility_graph_stats
            .worst_ms
            .max(other.visibility_graph_stats.worst_ms);
    }
}

#[derive(Clone, Debug)]
pub struct RenderSectionCompileRequest {
    pub target_sections: BTreeSet<RenderSectionKey>,
    pub section_revisions: BTreeMap<RenderSectionKey, u64>,
    pub snapshots: Vec<ChunkSnapshot>,
}

#[derive(Debug)]
pub struct RenderSectionCompileResult {
    pub target_sections: BTreeSet<RenderSectionKey>,
    pub section_revisions: BTreeMap<RenderSectionKey, u64>,
    pub result: std::result::Result<TexturedRenderSectionBuildReport, String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderSectionDirtyState {
    pub dirty_chunks: BTreeSet<ChunkPos>,
    pub dirty_sections: BTreeSet<RenderSectionKey>,
    pub inflight_sections: BTreeSet<RenderSectionKey>,
    section_revisions: BTreeMap<RenderSectionKey, u64>,
}

impl RenderSectionDirtyState {
    pub fn is_empty(&self) -> bool {
        self.dirty_chunks.is_empty()
            && self.dirty_sections.is_empty()
            && self.inflight_sections.is_empty()
    }

    pub fn dirty_work_is_empty(&self) -> bool {
        self.dirty_chunks.is_empty() && self.dirty_sections.is_empty()
    }

    pub fn mark_chunk_dirty(
        &mut self,
        pos: ChunkPos,
        known_section_keys: impl IntoIterator<Item = RenderSectionKey>,
    ) {
        for key in known_section_keys {
            self.bump_section_revision(key);
        }
        self.dirty_chunks.insert(pos);
    }

    pub fn mark_section_dirty(&mut self, key: RenderSectionKey) {
        self.bump_section_revision(key);
        self.dirty_sections.insert(key);
    }

    pub fn bump_section_revision(&mut self, key: RenderSectionKey) {
        let revision = self.section_revisions.entry(key).or_default();
        *revision = revision.wrapping_add(1);
    }

    pub fn bump_section_revisions(&mut self, keys: impl IntoIterator<Item = RenderSectionKey>) {
        for key in keys {
            self.bump_section_revision(key);
        }
    }

    pub fn section_revision(&self, key: RenderSectionKey) -> u64 {
        self.section_revisions
            .get(&key)
            .copied()
            .unwrap_or_default()
    }

    pub fn build_compile_request(
        &self,
        target_sections: BTreeSet<RenderSectionKey>,
        snapshots: Vec<ChunkSnapshot>,
    ) -> RenderSectionCompileRequest {
        let section_revisions = target_sections
            .iter()
            .map(|key| (*key, self.section_revision(*key)))
            .collect();
        RenderSectionCompileRequest {
            target_sections,
            section_revisions,
            snapshots,
        }
    }

    pub fn mark_compile_submitted(&mut self, target_sections: &BTreeSet<RenderSectionKey>) {
        self.inflight_sections
            .extend(target_sections.iter().copied());
    }

    pub fn accept_completed_compile_result(
        &mut self,
        completed: &RenderSectionCompileResult,
    ) -> RenderSectionCompileAcceptance {
        for key in &completed.target_sections {
            self.inflight_sections.remove(key);
        }
        completed.partition_by_revision(|key| self.section_revision(key))
    }

    pub fn discard_stale_dirty_work(
        &mut self,
        stale_chunks: &BTreeSet<ChunkPos>,
        stale_sections: &BTreeSet<RenderSectionKey>,
    ) {
        for pos in stale_chunks {
            self.dirty_chunks.remove(pos);
        }
        for key in stale_sections {
            self.dirty_sections.remove(key);
        }
    }

    pub fn discard_removed_dirty_work(
        &mut self,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_sections: &BTreeSet<RenderSectionKey>,
    ) {
        for pos in removal_chunks {
            self.dirty_chunks.remove(pos);
            self.dirty_sections
                .retain(|key| render_section_chunk_pos(*key) != *pos);
            self.inflight_sections
                .retain(|key| render_section_chunk_pos(*key) != *pos);
        }
        for key in removal_sections {
            self.dirty_sections.remove(key);
            self.inflight_sections.remove(key);
        }
    }

    pub fn apply_ready_plan(&mut self, plan: &RenderSectionReadyPlan) {
        for pos in &plan.budgeted_loaded_chunks {
            self.dirty_chunks.remove(pos);
        }
        for key in &plan.ready_section_keys {
            self.dirty_sections.remove(key);
        }
        self.dirty_sections
            .extend(plan.deferred_section_keys.iter().copied());
    }

    pub fn build_ready_plan_compile_request(
        &self,
        plan: &RenderSectionReadyPlan,
        snapshots: Vec<ChunkSnapshot>,
    ) -> Option<RenderSectionCompileRequest> {
        if plan.ready_section_keys.is_empty() {
            return None;
        }
        Some(self.build_compile_request(plan.ready_section_keys.clone(), snapshots))
    }

    pub fn accept_ready_plan_compile_submission(&mut self, plan: &RenderSectionReadyPlan) {
        self.mark_compile_submitted(&plan.ready_section_keys);
        self.apply_ready_plan(plan);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionDirtyWork {
    pub stale_dirty_chunks: BTreeSet<ChunkPos>,
    pub loaded_dirty_chunks: BTreeSet<ChunkPos>,
    pub removal_dirty_chunks: BTreeSet<ChunkPos>,
    pub stale_dirty_sections: BTreeSet<RenderSectionKey>,
    pub loaded_dirty_sections_by_chunk: BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    pub removal_dirty_sections: BTreeSet<RenderSectionKey>,
}

impl RenderSectionDirtyWork {
    pub fn has_removals(&self) -> bool {
        !self.removal_dirty_chunks.is_empty() || !self.removal_dirty_sections.is_empty()
    }
}

pub fn classify_render_section_dirty_work(
    dirty: &RenderSectionDirtyState,
    mut chunk_is_loaded: impl FnMut(ChunkPos) -> bool,
    mut chunk_is_cached: impl FnMut(ChunkPos) -> bool,
    mut section_is_loaded: impl FnMut(RenderSectionKey) -> bool,
    mut section_is_cached: impl FnMut(RenderSectionKey) -> bool,
) -> RenderSectionDirtyWork {
    let mut work = RenderSectionDirtyWork::default();

    for pos in dirty.dirty_chunks.iter().copied() {
        if chunk_is_loaded(pos) {
            work.loaded_dirty_chunks.insert(pos);
        } else if chunk_is_cached(pos) {
            work.removal_dirty_chunks.insert(pos);
        } else {
            work.stale_dirty_chunks.insert(pos);
        }
    }

    for key in dirty.dirty_sections.iter().copied() {
        if section_is_loaded(key) {
            work.loaded_dirty_sections_by_chunk
                .entry(render_section_chunk_pos(key))
                .or_default()
                .insert(key);
        } else if section_is_cached(key) {
            work.removal_dirty_sections.insert(key);
        } else {
            work.stale_dirty_sections.insert(key);
        }
    }

    work
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderSectionNeighborReadiness {
    ReadyWithNeighbors,
    ReadyNearCamera,
    DeferredMissingNeighbors,
}

impl RenderSectionNeighborReadiness {
    pub const fn is_ready(self) -> bool {
        matches!(self, Self::ReadyWithNeighbors | Self::ReadyNearCamera)
    }

    pub const fn is_near_exception(self) -> bool {
        matches!(self, Self::ReadyNearCamera)
    }
}

/// Horizontal distance (squared) below which a render section is treated as ready even
/// without fully loaded neighbors. Native keeps an exact client snapshot radius (unlike
/// Java's overfetched client chunk cache), so this near-camera exception keeps the
/// boundary ring meshing while flying above an otherwise loaded column.
const RENDER_NEIGHBOR_READY_DISTANCE_SQ: f32 = 24.0 * 24.0;

/// Shared camera-distance neighbor readiness used by both the desktop and web streaming
/// loops (067 Stage 3). Sections near the camera are always ready; farther sections wait
/// until their four horizontal neighbors have snapshots so culling/AO/light is correct.
pub fn render_section_neighbor_readiness(
    client: &ClientRuntime,
    key: RenderSectionKey,
    camera_position: Vec3,
) -> RenderSectionNeighborReadiness {
    if render_section_horizontal_distance_sq(key, camera_position)
        <= RENDER_NEIGHBOR_READY_DISTANCE_SQ
    {
        return RenderSectionNeighborReadiness::ReadyNearCamera;
    }
    if has_horizontal_neighbor_snapshots(client, ChunkPos::new(key.chunk_x, key.chunk_z)) {
        RenderSectionNeighborReadiness::ReadyWithNeighbors
    } else {
        RenderSectionNeighborReadiness::DeferredMissingNeighbors
    }
}

/// World-space center of a render section, used for distance ordering/readiness.
pub fn render_section_center(key: RenderSectionKey) -> Vec3 {
    Vec3::new(
        chunk_middle_block_coord(key.chunk_x) as f32,
        (key.section_y * SECTION_HEIGHT) as f32 + SECTION_HEIGHT as f32 * 0.5,
        chunk_middle_block_coord(key.chunk_z) as f32,
    )
}

fn render_section_distance_sq(key: RenderSectionKey, camera_position: Vec3) -> f32 {
    render_section_center(key).distance_squared(camera_position)
}

fn render_section_horizontal_distance_sq(key: RenderSectionKey, camera_position: Vec3) -> f32 {
    let center = render_section_center(key);
    let dx = center.x - camera_position.x;
    let dz = center.z - camera_position.z;
    dx * dx + dz * dz
}

fn render_chunk_distance_sq(pos: ChunkPos, camera_position: Vec3) -> f32 {
    let center = Vec3::new(
        chunk_middle_block_coord(pos.x) as f32,
        camera_position.y,
        chunk_middle_block_coord(pos.z) as f32,
    );
    center.distance_squared(camera_position)
}

fn has_horizontal_neighbor_snapshots(client: &ClientRuntime, pos: ChunkPos) -> bool {
    [
        ChunkPos::new(pos.x - 1, pos.z),
        ChunkPos::new(pos.x + 1, pos.z),
        ChunkPos::new(pos.x, pos.z - 1),
        ChunkPos::new(pos.x, pos.z + 1),
    ]
    .into_iter()
    .all(|neighbor| client.chunk_snapshot(neighbor).is_some())
}

/// Distance-sort loaded dirty chunk positions nearest-first (camera-relative), with a
/// stable coordinate tiebreak. Shared by the desktop and web streaming loops so both
/// platforms compile the nearest pending chunk first.
pub fn sort_chunk_positions_by_distance(
    positions: impl IntoIterator<Item = ChunkPos>,
    camera_position: Vec3,
) -> Vec<ChunkPos> {
    let mut positions = positions.into_iter().collect::<Vec<_>>();
    positions.sort_by(|left, right| {
        render_chunk_distance_sq(*left, camera_position)
            .total_cmp(&render_chunk_distance_sq(*right, camera_position))
            .then_with(|| left.x.cmp(&right.x))
            .then_with(|| left.z.cmp(&right.z))
    });
    positions
}

/// Distance-sort dirty-section chunks by their nearest dirty section, nearest-first.
pub fn sort_dirty_section_chunks_by_distance(
    sections_by_chunk: &BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    camera_position: Vec3,
) -> Vec<ChunkPos> {
    let mut positions = sections_by_chunk.keys().copied().collect::<Vec<_>>();
    positions.sort_by(|left, right| {
        dirty_section_chunk_distance_sq(sections_by_chunk, *left, camera_position)
            .total_cmp(&dirty_section_chunk_distance_sq(
                sections_by_chunk,
                *right,
                camera_position,
            ))
            .then_with(|| left.x.cmp(&right.x))
            .then_with(|| left.z.cmp(&right.z))
    });
    positions
}

fn dirty_section_chunk_distance_sq(
    sections_by_chunk: &BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    pos: ChunkPos,
    camera_position: Vec3,
) -> f32 {
    sections_by_chunk
        .get(&pos)
        .and_then(|keys| {
            keys.iter()
                .map(|key| render_section_distance_sq(*key, camera_position))
                .min_by(f32::total_cmp)
        })
        .unwrap_or_else(|| render_chunk_distance_sq(pos, camera_position))
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionReadyPlan {
    pub budgeted_loaded_chunks: BTreeSet<ChunkPos>,
    pub budgeted_dirty_section_chunks: BTreeSet<ChunkPos>,
    pub ready_section_keys: BTreeSet<RenderSectionKey>,
    pub deferred_section_keys: BTreeSet<RenderSectionKey>,
    pub near_exception_section_count: usize,
    pub deferred_section_count: usize,
}

pub fn plan_ready_render_sections(
    sorted_loaded_dirty_chunks: impl IntoIterator<Item = ChunkPos>,
    sorted_dirty_section_chunks: impl IntoIterator<Item = ChunkPos>,
    loaded_dirty_sections_by_chunk: &BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    chunk_budget: usize,
    mut section_keys_for_chunk: impl FnMut(ChunkPos) -> Vec<RenderSectionKey>,
    inflight_sections: &BTreeSet<RenderSectionKey>,
    mut section_readiness: impl FnMut(RenderSectionKey) -> RenderSectionNeighborReadiness,
) -> RenderSectionReadyPlan {
    let mut plan = RenderSectionReadyPlan::default();
    if chunk_budget == 0 {
        return plan;
    }

    let mut budgeted_chunk_count = 0usize;
    for pos in sorted_loaded_dirty_chunks {
        let ready_before = plan.ready_section_keys.len();
        for key in section_keys_for_chunk(pos) {
            plan_ready_render_section_key(
                &mut plan,
                key,
                inflight_sections,
                &mut section_readiness,
            );
        }
        if plan.ready_section_keys.len() > ready_before {
            plan.budgeted_loaded_chunks.insert(pos);
            budgeted_chunk_count += 1;
            if budgeted_chunk_count >= chunk_budget {
                break;
            }
        }
    }

    if budgeted_chunk_count >= chunk_budget {
        return plan;
    }

    for pos in sorted_dirty_section_chunks {
        if plan.budgeted_loaded_chunks.contains(&pos) {
            continue;
        }
        if let Some(keys) = loaded_dirty_sections_by_chunk.get(&pos) {
            let ready_before = plan.ready_section_keys.len();
            for key in keys {
                plan_ready_render_section_key(
                    &mut plan,
                    *key,
                    inflight_sections,
                    &mut section_readiness,
                );
            }
            if plan.ready_section_keys.len() > ready_before {
                plan.budgeted_dirty_section_chunks.insert(pos);
                budgeted_chunk_count += 1;
                if budgeted_chunk_count >= chunk_budget {
                    break;
                }
            }
        }
    }

    plan
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderSectionSyncPlan {
    pub dirty_work: RenderSectionDirtyWork,
    pub ready_plan: RenderSectionReadyPlan,
}

impl RenderSectionSyncPlan {
    pub fn has_removals(&self) -> bool {
        self.dirty_work.has_removals()
    }

    pub fn ready_update(&self, submitted_compile_section_count: usize) -> RenderSectionCacheUpdate {
        RenderSectionCacheUpdate {
            deferred_section_count: self.ready_plan.deferred_section_count,
            near_exception_section_count: self.ready_plan.near_exception_section_count,
            submitted_compile_section_count,
            ..RenderSectionCacheUpdate::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderSectionRemovalMode {
    ApplyImmediately,
    Defer,
}

#[derive(Clone, Debug)]
pub struct RenderSectionSyncUpdate {
    pub sync_plan: RenderSectionSyncPlan,
    pub cache_update: RenderSectionCacheUpdate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedRenderViewCompile<T> {
    pub center: ChunkPos,
    pub force: bool,
    pub metadata: T,
}

impl<T> QueuedRenderViewCompile<T> {
    pub fn new(center: ChunkPos, force: bool, metadata: T) -> Self {
        Self {
            center,
            force,
            metadata,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenderViewCompileQueueDecision<T> {
    Start(QueuedRenderViewCompile<T>),
    Queued(QueuedRenderViewCompile<T>),
    Skipped,
    Idle,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RenderViewCompileQueue<T> {
    queued: Option<QueuedRenderViewCompile<T>>,
}

impl<T: Clone> RenderViewCompileQueue<T> {
    pub fn request(
        &mut self,
        request: QueuedRenderViewCompile<T>,
        loaded_center: Option<ChunkPos>,
        compile_busy: bool,
    ) -> RenderViewCompileQueueDecision<T> {
        if !request.force && Some(request.center) == loaded_center {
            self.queued = None;
            return RenderViewCompileQueueDecision::Skipped;
        }
        if compile_busy {
            self.merge(request);
            return self
                .queued
                .clone()
                .map(RenderViewCompileQueueDecision::Queued)
                .unwrap_or(RenderViewCompileQueueDecision::Idle);
        }
        self.queued = None;
        RenderViewCompileQueueDecision::Start(request)
    }

    pub fn take_next(
        &mut self,
        loaded_center: Option<ChunkPos>,
        compile_busy: bool,
    ) -> RenderViewCompileQueueDecision<T> {
        if compile_busy {
            return self
                .queued
                .clone()
                .map(RenderViewCompileQueueDecision::Queued)
                .unwrap_or(RenderViewCompileQueueDecision::Idle);
        }
        let Some(request) = self.queued.take() else {
            return RenderViewCompileQueueDecision::Idle;
        };
        if !request.force && Some(request.center) == loaded_center {
            RenderViewCompileQueueDecision::Skipped
        } else {
            RenderViewCompileQueueDecision::Start(request)
        }
    }

    pub fn queued(&self) -> Option<&QueuedRenderViewCompile<T>> {
        self.queued.as_ref()
    }

    pub fn has_queued(&self) -> bool {
        self.queued.is_some()
    }

    fn merge(&mut self, request: QueuedRenderViewCompile<T>) {
        let Some(previous) = self.queued.take() else {
            self.queued = Some(request);
            return;
        };
        self.queued = Some(QueuedRenderViewCompile {
            center: request.center,
            force: previous.force || request.force,
            metadata: request.metadata,
        });
    }
}

pub fn prepare_render_section_sync_plan(
    dirty: &mut RenderSectionDirtyState,
    dirty_work: RenderSectionDirtyWork,
    sorted_loaded_dirty_chunks: impl IntoIterator<Item = ChunkPos>,
    sorted_dirty_section_chunks: impl IntoIterator<Item = ChunkPos>,
    chunk_budget: usize,
    section_keys_for_chunk: impl FnMut(ChunkPos) -> Vec<RenderSectionKey>,
    section_readiness: impl FnMut(RenderSectionKey) -> RenderSectionNeighborReadiness,
) -> RenderSectionSyncPlan {
    dirty.discard_stale_dirty_work(
        &dirty_work.stale_dirty_chunks,
        &dirty_work.stale_dirty_sections,
    );
    let ready_plan = plan_ready_render_sections(
        sorted_loaded_dirty_chunks,
        sorted_dirty_section_chunks,
        &dirty_work.loaded_dirty_sections_by_chunk,
        chunk_budget,
        section_keys_for_chunk,
        &dirty.inflight_sections,
        section_readiness,
    );
    RenderSectionSyncPlan {
        dirty_work,
        ready_plan,
    }
}

#[derive(Debug)]
pub struct RenderSectionCompileFinish {
    pub acceptance_report: RenderSectionCompileAcceptanceReport,
    pub accepted_sections: BTreeSet<RenderSectionKey>,
    pub stale_sections: BTreeSet<RenderSectionKey>,
    pub build_report: Option<TexturedRenderSectionBuildReport>,
}

#[derive(Debug)]
pub struct RenderSectionFinishedCompileUpdate {
    pub acceptance_report: RenderSectionCompileAcceptanceReport,
    pub accepted_sections: BTreeSet<RenderSectionKey>,
    pub stale_sections: BTreeSet<RenderSectionKey>,
    pub cache_update: RenderSectionCacheUpdate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderSectionCompileSubmission<T> {
    pub submitted_section_count: usize,
    pub output: T,
}

#[derive(Clone, Debug)]
pub struct RenderSectionReadyWorkSubmission<T> {
    pub cache_update: RenderSectionCacheUpdate,
    pub submission: Option<RenderSectionCompileSubmission<T>>,
}

pub fn finish_render_section_compile_result(
    dirty: &mut RenderSectionDirtyState,
    completed: RenderSectionCompileResult,
    request_id: u32,
) -> Result<RenderSectionCompileFinish> {
    let acceptance = dirty.accept_completed_compile_result(&completed);
    let acceptance_report =
        RenderSectionCompileAcceptanceReport::from_acceptance(request_id, &acceptance);
    let accepted_sections = acceptance.accepted_sections;
    let stale_sections = acceptance.stale_sections;
    let build_report = if accepted_sections.is_empty() {
        None
    } else {
        Some(completed.result.map_err(anyhow::Error::msg)?)
    };
    Ok(RenderSectionCompileFinish {
        acceptance_report,
        accepted_sections,
        stale_sections,
        build_report,
    })
}

pub const ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND: f64 = 32.0;
pub const ENGINE_CAMERA_MIN_SPEED_BLOCKS_PER_SECOND: f64 = 2.0;
pub const ENGINE_CAMERA_MAX_SPEED_BLOCKS_PER_SECOND: f64 = 256.0;
/// Fly-speed range exposed to the in-game menu, expressed as a multiplier of the
/// base speed. The range is symmetric in log space so a 1.0x multiplier sits at
/// the slider's midpoint.
pub const ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER: f64 = 0.125;
pub const ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER: f64 = 8.0;
pub const ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER: f64 = 1.0;
pub const ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER: f64 = 0.125;
pub const ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER: f64 = 8.0;
pub const ENGINE_CAMERA_MOUSE_SENSITIVITY: f64 = 0.0035;
pub const ENGINE_CAMERA_SPAWN_Y: f64 = 104.0;
pub const ENGINE_CAMERA_SPAWN_YAW_RADIANS: f64 = 0.55;
pub const ENGINE_CAMERA_SPAWN_PITCH_RADIANS: f64 = -0.35;
const ENGINE_HAND_PUSH_EMULATION_CYCLE_HZ: f64 = 1.8;
const ENGINE_HAND_PUSH_EMULATION_HAND_SPACING: f64 = 0.34;
const ENGINE_HAND_PUSH_EMULATION_FORWARD_REACH: f64 = 0.32;
const ENGINE_HAND_PUSH_EMULATION_STROKE: f64 = 0.36;
const ENGINE_HAND_PUSH_EMULATION_LIFT: f64 = 0.16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineCameraInput {
    pub dt_seconds: f64,
    pub mouse_delta_x: f64,
    pub mouse_delta_y: f64,
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub descend: bool,
    pub shift: bool,
    pub sprint: bool,
    pub movement_impulse: Option<EngineCameraMovementImpulse>,
    pub movement_yaw_radians: Option<f64>,
    pub hand_push: Option<EngineHandPushInput>,
    pub hand_push_emulation: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineCameraMovementImpulse {
    pub left: f32,
    pub forward: f32,
}

impl EngineCameraMovementImpulse {
    pub fn new(left: f32, forward: f32) -> Self {
        Self { left, forward }
    }

    fn as_player_impulse(self) -> (f32, f32) {
        (self.left, self.forward)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineHandPushInput {
    pub head_position: Vec3d,
    pub left_hand_position: Vec3d,
    pub right_hand_position: Vec3d,
}

impl EngineHandPushInput {
    pub fn new(
        head_position: Vec3d,
        left_hand_position: Vec3d,
        right_hand_position: Vec3d,
    ) -> Self {
        Self {
            head_position,
            left_hand_position,
            right_hand_position,
        }
    }

    fn pose(self) -> HandPushPose {
        HandPushPose {
            head_position: self.head_position,
            left_hand_position: self.left_hand_position,
            right_hand_position: self.right_hand_position,
        }
    }
}

impl Default for EngineCameraInput {
    fn default() -> Self {
        Self {
            dt_seconds: 0.0,
            mouse_delta_x: 0.0,
            mouse_delta_y: 0.0,
            forward: false,
            backward: false,
            left: false,
            right: false,
            jump: false,
            descend: false,
            shift: false,
            sprint: false,
            movement_impulse: None,
            movement_yaw_radians: None,
            hand_push: None,
            hand_push_emulation: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EngineCameraMovementMode {
    #[default]
    Walking,
    NoClip,
    HandPush,
}

impl EngineCameraMovementMode {
    pub const fn toggled(self) -> Self {
        match self {
            Self::Walking => Self::NoClip,
            Self::NoClip => Self::HandPush,
            Self::HandPush => Self::Walking,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Walking => "WALK",
            Self::NoClip => "NOCLIP",
            Self::HandPush => "HAND",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EngineCameraViewMode {
    #[default]
    FirstPerson,
    ThirdPersonBack,
}

impl EngineCameraViewMode {
    pub const fn toggled(self) -> Self {
        match self {
            Self::FirstPerson => Self::ThirdPersonBack,
            Self::ThirdPersonBack => Self::FirstPerson,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::FirstPerson => "FIRST_PERSON",
            Self::ThirdPersonBack => "THIRD_PERSON",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "first" | "first-person" | "first_person" | "1p" => Some(Self::FirstPerson),
            "third" | "third-person" | "third_person" | "third-person-back"
            | "third_person_back" | "3p" => Some(Self::ThirdPersonBack),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineCameraSnapshot {
    pub eye: Vec3d,
    pub yaw_radians: f64,
    pub pitch_radians: f64,
    pub speed_blocks_per_second: f64,
    pub chunk_pos: ChunkPos,
}

impl EngineCameraSnapshot {
    pub fn from_eye_pose(
        eye: Vec3d,
        yaw_radians: f64,
        pitch_radians: f64,
        speed_blocks_per_second: f64,
    ) -> Self {
        Self {
            eye,
            yaw_radians,
            pitch_radians,
            speed_blocks_per_second,
            chunk_pos: ChunkPos::new(
                block_to_chunk_coord(eye.x.floor() as i32),
                block_to_chunk_coord(eye.z.floor() as i32),
            ),
        }
    }

    pub fn from_player(player: &LocalPlayerController, speed_blocks_per_second: f64) -> Self {
        let pose = player.pose();
        Self {
            eye: pose.eye_position(),
            yaw_radians: pose.native_yaw_radians(),
            pitch_radians: pose.native_pitch_radians(),
            speed_blocks_per_second,
            chunk_pos: pose.chunk_pos(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineCameraFrameState {
    pub camera: EngineCameraSnapshot,
    pub movement_mode: EngineCameraMovementMode,
    pub view_mode: EngineCameraViewMode,
    pub on_ground: bool,
    pub horizontal_collision: bool,
    pub vertical_collision: bool,
    pub selected_hotbar_slot: u8,
}

impl EngineCameraFrameState {
    pub fn from_player(
        player: &LocalPlayerController,
        movement_mode: EngineCameraMovementMode,
        view_mode: EngineCameraViewMode,
        speed_blocks_per_second: f64,
        selected_hotbar_slot: u8,
    ) -> Self {
        Self {
            camera: EngineCameraSnapshot::from_player(player, speed_blocks_per_second),
            movement_mode,
            view_mode,
            on_ground: player.on_ground(),
            horizontal_collision: player.horizontal_collision(),
            vertical_collision: player.vertical_collision(),
            selected_hotbar_slot,
        }
    }

    pub const fn movement_mode_label(&self) -> &'static str {
        self.movement_mode.label()
    }

    pub const fn view_mode_label(&self) -> &'static str {
        self.view_mode.label()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnginePoseSyncCommandKind {
    Movement,
    CorrectionResync,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnginePoseSyncCommand {
    pub kind: EnginePoseSyncCommandKind,
    pub command: ClientCommand,
    pub camera: EngineCameraSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnginePoseCorrectionAcceptance {
    pub update: PlayerPositionUpdate,
    pub accept_command: ClientCommand,
    pub feet_position: Vec3d,
    pub camera: EngineCameraSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineRenderCamera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_y_radians: f32,
    pub z_near: f32,
    pub z_far: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandingEvent {
    pub impact_speed: f64,
    pub position: Vec3d,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EngineCameraController {
    player: LocalPlayerController,
    hand_push: HandPushLocomotionController,
    movement_mode: EngineCameraMovementMode,
    view_mode: EngineCameraViewMode,
    speed_blocks_per_second: f64,
    movement_speed_multiplier: f64,
    landing_events: Vec<LandingEvent>,
    hand_push_emulation_phase: f64,
}

impl EngineCameraController {
    pub fn spawn_for_chunk(center: ChunkPos) -> Self {
        let eye = Vec3d::new(
            f64::from(chunk_middle_block_coord(center.x)),
            ENGINE_CAMERA_SPAWN_Y,
            f64::from(chunk_middle_block_coord(center.z)),
        );
        Self::from_eye_pose(
            eye,
            ENGINE_CAMERA_SPAWN_YAW_RADIANS,
            ENGINE_CAMERA_SPAWN_PITCH_RADIANS,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        )
    }

    pub fn from_eye_pose(
        eye: Vec3d,
        yaw_radians: f64,
        pitch_radians: f64,
        speed_blocks_per_second: f64,
    ) -> Self {
        let mut player = LocalPlayerController::new();
        player.set_pose(LocalPlayerPose::from_eye_position(
            eye,
            -yaw_radians.to_degrees(),
            -pitch_radians.to_degrees(),
            LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        ));
        Self {
            player,
            hand_push: HandPushLocomotionController::default(),
            movement_mode: EngineCameraMovementMode::Walking,
            view_mode: EngineCameraViewMode::FirstPerson,
            speed_blocks_per_second: clamp_camera_speed(speed_blocks_per_second),
            movement_speed_multiplier: ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER,
            landing_events: Vec::new(),
            hand_push_emulation_phase: 0.0,
        }
    }

    pub const fn player(&self) -> &LocalPlayerController {
        &self.player
    }

    pub fn set_eye_pose(&mut self, eye: Vec3d, yaw_radians: f64, pitch_radians: f64) {
        self.player.set_pose(LocalPlayerPose::from_eye_position(
            eye,
            -yaw_radians.to_degrees(),
            -pitch_radians.to_degrees(),
            LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        ));
    }

    pub const fn movement_mode(&self) -> EngineCameraMovementMode {
        self.movement_mode
    }

    pub const fn view_mode(&self) -> EngineCameraViewMode {
        self.view_mode
    }

    pub fn set_view_mode(&mut self, view_mode: EngineCameraViewMode) {
        self.view_mode = view_mode;
    }

    pub fn toggle_view_mode(&mut self) -> EngineCameraViewMode {
        self.set_view_mode(self.view_mode.toggled());
        self.view_mode
    }

    pub fn set_movement_mode(&mut self, movement_mode: EngineCameraMovementMode) {
        if self.movement_mode != movement_mode {
            self.player.clear_delta_movement();
            self.hand_push.reset();
            self.hand_push_emulation_phase = 0.0;
        }
        self.movement_mode = movement_mode;
    }

    pub fn toggle_movement_mode(&mut self) -> EngineCameraMovementMode {
        self.set_movement_mode(self.movement_mode.toggled());
        self.movement_mode
    }

    pub const fn on_ground(&self) -> bool {
        self.player.on_ground()
    }

    pub const fn horizontal_collision(&self) -> bool {
        self.player.horizontal_collision()
    }

    pub const fn vertical_collision(&self) -> bool {
        self.player.vertical_collision()
    }

    pub fn next_move_player_command(&mut self) -> Option<ClientCommand> {
        self.player.next_move_player_command()
    }

    pub fn pos_rot_move_player_command(&mut self) -> ClientCommand {
        self.player.pos_rot_move_player_command()
    }

    pub fn take_landing_events(&mut self) -> Vec<LandingEvent> {
        std::mem::take(&mut self.landing_events)
    }

    pub fn apply_player_position_update(&mut self, update: PlayerPositionUpdate) -> ClientCommand {
        self.player.apply_player_position_update(update)
    }

    pub fn next_pose_sync_command(&mut self) -> Option<EnginePoseSyncCommand> {
        let command = self.next_move_player_command()?;
        Some(EnginePoseSyncCommand {
            kind: EnginePoseSyncCommandKind::Movement,
            command,
            camera: self.snapshot(),
        })
    }

    pub fn accept_position_update(
        &mut self,
        update: PlayerPositionUpdate,
    ) -> EnginePoseCorrectionAcceptance {
        let accept_command = self.apply_player_position_update(update);
        let feet_position = self.player.pose().position;
        EnginePoseCorrectionAcceptance {
            update,
            accept_command,
            feet_position,
            camera: self.snapshot(),
        }
    }

    pub fn corrected_pose_sync_command(&mut self) -> EnginePoseSyncCommand {
        let command = self.pos_rot_move_player_command();
        EnginePoseSyncCommand {
            kind: EnginePoseSyncCommandKind::CorrectionResync,
            command,
            camera: self.snapshot(),
        }
    }

    pub const fn speed_blocks_per_second(&self) -> f64 {
        self.speed_blocks_per_second
    }

    pub fn set_speed_blocks_per_second(&mut self, speed_blocks_per_second: f64) {
        self.speed_blocks_per_second = clamp_camera_speed(speed_blocks_per_second);
    }

    /// Current fly speed expressed as a multiplier of the base speed.
    pub fn fly_speed_multiplier(&self) -> f64 {
        self.speed_blocks_per_second / ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND
    }

    /// Set the fly speed from a multiplier of the base speed. The resulting
    /// speed is clamped to the engine's absolute speed limits.
    pub fn set_fly_speed_multiplier(&mut self, multiplier: f64) {
        if !multiplier.is_finite() {
            return;
        }
        self.set_speed_blocks_per_second(multiplier * ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND);
    }

    /// Current walking movement speed multiplier.
    pub const fn movement_speed_multiplier(&self) -> f64 {
        self.movement_speed_multiplier
    }

    pub fn set_movement_speed_multiplier(&mut self, multiplier: f64) {
        self.movement_speed_multiplier = clamp_movement_speed_multiplier(multiplier);
    }

    pub fn adjust_speed(&mut self, wheel_amount: f64) {
        if !wheel_amount.is_finite() {
            return;
        }
        self.set_speed_blocks_per_second(Self::adjusted_speed_blocks_per_second(
            self.speed_blocks_per_second,
            wheel_amount,
        ));
    }

    pub fn adjusted_speed_blocks_per_second(
        speed_blocks_per_second: f64,
        wheel_amount: f64,
    ) -> f64 {
        if !wheel_amount.is_finite() {
            return clamp_camera_speed(speed_blocks_per_second);
        }
        let multiplier = (1.0 + wheel_amount * 0.18).clamp(0.5, 1.8);
        clamp_camera_speed(speed_blocks_per_second * multiplier)
    }

    pub fn snapshot(&self) -> EngineCameraSnapshot {
        EngineCameraSnapshot::from_player(&self.player, self.speed_blocks_per_second)
    }

    pub fn frame_state(&self, interaction: &ClientInteractionController) -> EngineCameraFrameState {
        EngineCameraFrameState::from_player(
            &self.player,
            self.movement_mode,
            self.view_mode,
            self.speed_blocks_per_second,
            interaction.selected_hotbar_slot(),
        )
    }

    pub fn pick_block(
        &self,
        client: &ClientRuntime,
        interaction: &ClientInteractionController,
    ) -> BlockHitResult {
        let pose = self.player.pose();
        interaction.pick_block(client, pose.eye_position(), pose.view_vector())
    }

    pub fn target_block(
        &self,
        client: &ClientRuntime,
        interaction: &ClientInteractionController,
    ) -> Option<BlockInteractionTarget> {
        let pose = self.player.pose();
        interaction.target_block(client, pose.eye_position(), pose.view_vector())
    }

    pub fn set_key(&mut self, key: PlayerInputKey, down: bool) {
        self.player.set_key(key, down);
    }

    pub fn clear_keys(&mut self) {
        self.player.clear_keys();
    }

    pub fn turn_mouse_delta(&mut self, mouse_delta_x: f64, mouse_delta_y: f64) {
        Self::turn_player_mouse_delta(&mut self.player, mouse_delta_x, mouse_delta_y);
    }

    pub fn turn_player_mouse_delta(
        player: &mut LocalPlayerController,
        mouse_delta_x: f64,
        mouse_delta_y: f64,
    ) {
        if !mouse_delta_x.is_finite() || !mouse_delta_y.is_finite() {
            return;
        }
        player.turn_native_radians(
            -mouse_delta_x * ENGINE_CAMERA_MOUSE_SENSITIVITY,
            -mouse_delta_y * ENGINE_CAMERA_MOUSE_SENSITIVITY,
        );
    }

    pub fn apply_input(&mut self, input: EngineCameraInput) -> EngineCameraSnapshot {
        self.apply_key_input(input);
        let _ = self.tick_no_clip(input);
        self.snapshot()
    }

    pub fn apply_movement_input(
        &mut self,
        client: &ClientRuntime,
        input: EngineCameraInput,
    ) -> EngineCameraSnapshot {
        self.apply_key_input(input);
        match self.movement_mode {
            EngineCameraMovementMode::Walking => {
                let _ = self.tick_walking(client, input);
            }
            EngineCameraMovementMode::NoClip => {
                let _ = self.tick_no_clip(input);
            }
            EngineCameraMovementMode::HandPush => {
                let _ = self.tick_hand_push(client, input);
            }
        }
        self.snapshot()
    }

    pub fn tick_movement(&mut self, client: &ClientRuntime, dt_seconds: f64) -> bool {
        let input = EngineCameraInput {
            dt_seconds,
            ..EngineCameraInput::default()
        };
        match self.movement_mode {
            EngineCameraMovementMode::Walking => self.tick_walking(client, input),
            EngineCameraMovementMode::NoClip => self.tick_no_clip(input),
            EngineCameraMovementMode::HandPush => self.tick_hand_push(client, input),
        }
    }

    pub fn probe_ground(&mut self, client: &ClientRuntime, distance: f64) {
        if self.movement_mode == EngineCameraMovementMode::NoClip
            || !distance.is_finite()
            || distance <= 0.0
        {
            return;
        }
        self.player
            .move_colliding(client, Vec3d::new(0.0, -distance, 0.0));
    }

    fn apply_key_input(&mut self, input: EngineCameraInput) {
        self.set_key(PlayerInputKey::Forward, input.forward);
        self.set_key(PlayerInputKey::Backward, input.backward);
        self.set_key(PlayerInputKey::Left, input.left);
        self.set_key(PlayerInputKey::Right, input.right);
        self.set_key(PlayerInputKey::Jump, input.jump);
        self.set_key(PlayerInputKey::Descend, input.descend);
        self.set_key(PlayerInputKey::Shift, input.shift);
        self.set_key(PlayerInputKey::Sprint, input.sprint);
        self.turn_mouse_delta(input.mouse_delta_x, input.mouse_delta_y);
    }

    fn tick_no_clip(&mut self, input: EngineCameraInput) -> bool {
        let dt_seconds = input.dt_seconds.clamp(0.0, 0.1);
        Self::tick_player_no_clip(
            &mut self.player,
            self.speed_blocks_per_second,
            dt_seconds,
            input
                .movement_impulse
                .map(EngineCameraMovementImpulse::as_player_impulse),
            input.movement_yaw_radians,
        )
        .is_some()
    }

    pub fn tick_player_no_clip(
        player: &mut LocalPlayerController,
        speed_blocks_per_second: f64,
        dt_seconds: f64,
        movement_impulse: Option<(f32, f32)>,
        movement_yaw_radians: Option<f64>,
    ) -> Option<Vec3d> {
        let pose = player.pose();
        let movement_yaw = finite_movement_yaw(movement_yaw_radians);
        let yaw_radians = movement_yaw.unwrap_or_else(|| pose.native_yaw_radians());
        let pitch_radians = if movement_yaw.is_some() {
            0.0
        } else {
            pose.native_pitch_radians()
        };
        player.tick_no_clip_movement_with_impulse(
            NoClipMovementStep {
                yaw_radians,
                pitch_radians,
                speed_blocks_per_second: clamp_camera_speed(speed_blocks_per_second),
                dt_seconds,
                descending: false,
                sprinting: false,
            },
            movement_impulse,
        )
    }

    fn tick_walking(&mut self, client: &ClientRuntime, input: EngineCameraInput) -> bool {
        let pose = self.player.pose();
        let dt_seconds = input.dt_seconds.clamp(0.0, 0.1);
        let y_rot_degrees = finite_movement_yaw(input.movement_yaw_radians)
            .map(|yaw| -yaw.to_degrees())
            .unwrap_or(pose.y_rot_degrees);
        let was_on_ground = self.player.on_ground();
        let pre_move_fall_speed = (-self.player.delta_movement().y).max(0.0);
        let moved = self
            .player
            .tick_walking_movement_with_impulse(
                client,
                WalkingMovementStep {
                    y_rot_degrees,
                    speed_multiplier: self.movement_speed_multiplier,
                    dt_seconds,
                },
                input
                    .movement_impulse
                    .map(EngineCameraMovementImpulse::as_player_impulse),
            )
            .is_some();
        if moved && !was_on_ground && self.player.on_ground() {
            let impact_speed = pre_move_fall_speed * LOCAL_PLAYER_TICKS_PER_SECOND;
            if impact_speed >= LANDING_MIN_IMPACT_SPEED {
                self.landing_events.push(LandingEvent {
                    impact_speed,
                    position: self.player.pose().position,
                });
            }
        }
        moved
    }

    fn tick_hand_push(&mut self, client: &ClientRuntime, input: EngineCameraInput) -> bool {
        let dt_seconds = input.dt_seconds.clamp(0.0, 0.1);
        if dt_seconds <= 0.0 {
            return false;
        }

        let was_on_ground = self.player.on_ground();
        let pre_move_fall_speed = (-self.player.delta_movement().y).max(0.0);
        let hand_input = input.hand_push.or_else(|| {
            input
                .hand_push_emulation
                .then(|| self.emulated_hand_push_input(input, dt_seconds))
                .flatten()
        });
        let mut moved_by_hand = false;
        if let Some(hand_input) = hand_input {
            if let Some(result) = self.hand_push.tick(
                client,
                &mut self.player,
                HandPushMovementStep {
                    dt_seconds,
                    pose: hand_input.pose(),
                },
            ) {
                moved_by_hand = result.body_movement.length_sqr() > 1.0e-12;
            }
        } else {
            self.hand_push.reset();
        }

        let pose = self.player.pose();
        let y_rot_degrees = finite_movement_yaw(input.movement_yaw_radians)
            .map(|yaw| -yaw.to_degrees())
            .unwrap_or(pose.y_rot_degrees);
        let moved_by_physics = self
            .player
            .tick_walking_movement_with_impulse(
                client,
                WalkingMovementStep {
                    y_rot_degrees,
                    speed_multiplier: self.movement_speed_multiplier,
                    dt_seconds,
                },
                Some((0.0, 0.0)),
            )
            .is_some();
        if (moved_by_hand || moved_by_physics) && !was_on_ground && self.player.on_ground() {
            let impact_speed = pre_move_fall_speed * LOCAL_PLAYER_TICKS_PER_SECOND;
            if impact_speed >= LANDING_MIN_IMPACT_SPEED {
                self.landing_events.push(LandingEvent {
                    impact_speed,
                    position: self.player.pose().position,
                });
            }
        }
        moved_by_hand || moved_by_physics
    }

    fn emulated_hand_push_input(
        &mut self,
        input: EngineCameraInput,
        dt_seconds: f64,
    ) -> Option<EngineHandPushInput> {
        let pose = self.player.pose();
        let yaw_radians = finite_movement_yaw(input.movement_yaw_radians)
            .unwrap_or_else(|| pose.native_yaw_radians());
        let direction = hand_push_emulation_direction(input, yaw_radians);
        if direction == Vec3d::ZERO {
            return None;
        }
        self.hand_push_emulation_phase = (self.hand_push_emulation_phase
            + dt_seconds * std::f64::consts::TAU * ENGINE_HAND_PUSH_EMULATION_CYCLE_HZ)
            % std::f64::consts::TAU;
        let right = horizontal_right_from_yaw(yaw_radians);
        let feet = pose.position;
        let hand_y = feet.y + HAND_PUSH_DEFAULT_HAND_RADIUS * 0.5;
        let base = Vec3d::new(feet.x, hand_y, feet.z)
            .add(direction.scale(ENGINE_HAND_PUSH_EMULATION_FORWARD_REACH));
        let left_phase = self.hand_push_emulation_phase;
        let right_phase = self.hand_push_emulation_phase + std::f64::consts::PI;
        let hand_at_phase = |side: f64, phase: f64| {
            base.add(right.scale(side * ENGINE_HAND_PUSH_EMULATION_HAND_SPACING))
                .add(direction.scale(-phase.sin() * ENGINE_HAND_PUSH_EMULATION_STROKE))
                .add(Vec3d::new(
                    0.0,
                    phase.cos().max(0.0) * ENGINE_HAND_PUSH_EMULATION_LIFT,
                    0.0,
                ))
        };
        let head = pose.eye_position();
        Some(EngineHandPushInput::new(
            head,
            hand_at_phase(-1.0, left_phase),
            hand_at_phase(1.0, right_phase),
        ))
    }

    pub fn render_camera(&self, render_distance: u32) -> EngineRenderCamera {
        render_camera_from_snapshot_with_view_mode(self.snapshot(), self.view_mode, render_distance)
    }
}

pub fn render_camera_from_snapshot(
    snapshot: EngineCameraSnapshot,
    render_distance: u32,
) -> EngineRenderCamera {
    render_camera_from_snapshot_with_view_mode(
        snapshot,
        EngineCameraViewMode::FirstPerson,
        render_distance,
    )
}

pub fn render_camera_from_snapshot_with_view_mode(
    snapshot: EngineCameraSnapshot,
    view_mode: EngineCameraViewMode,
    render_distance: u32,
) -> EngineRenderCamera {
    let forward = view_forward(snapshot.yaw_radians, snapshot.pitch_radians);
    let eye = match view_mode {
        EngineCameraViewMode::FirstPerson => snapshot.eye,
        EngineCameraViewMode::ThirdPersonBack => snapshot
            .eye
            .add(forward.scale(-THIRD_PERSON_CAMERA_DISTANCE)),
    };
    let target = eye.add(forward);
    EngineRenderCamera {
        eye: vec3d_to_f32_array(eye),
        target: vec3d_to_f32_array(target),
        up: [0.0, 1.0, 0.0],
        fov_y_radians: 64.0_f32.to_radians(),
        z_near: 0.05,
        z_far: 700.0 + render_distance as f32 * 128.0,
    }
}

fn clamp_camera_speed(speed_blocks_per_second: f64) -> f64 {
    if speed_blocks_per_second.is_finite() {
        speed_blocks_per_second.clamp(
            ENGINE_CAMERA_MIN_SPEED_BLOCKS_PER_SECOND,
            ENGINE_CAMERA_MAX_SPEED_BLOCKS_PER_SECOND,
        )
    } else {
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND
    }
}

fn clamp_movement_speed_multiplier(multiplier: f64) -> f64 {
    if multiplier.is_finite() {
        multiplier.clamp(
            ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER,
            ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER,
        )
    } else {
        ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER
    }
}

fn finite_movement_yaw(yaw_radians: Option<f64>) -> Option<f64> {
    yaw_radians.filter(|yaw| yaw.is_finite())
}

fn hand_push_emulation_direction(input: EngineCameraInput, yaw_radians: f64) -> Vec3d {
    if !yaw_radians.is_finite() {
        return Vec3d::ZERO;
    }
    let (left_impulse, forward_impulse) = input
        .movement_impulse
        .map(|impulse| (f64::from(impulse.left), f64::from(impulse.forward)))
        .unwrap_or_else(|| {
            (
                axis(input.left, input.right) as f64,
                axis(input.forward, input.backward) as f64,
            )
        });
    if left_impulse.abs() <= 1.0e-5 && forward_impulse.abs() <= 1.0e-5 {
        return Vec3d::ZERO;
    }
    normalize_vec3d_or_zero(
        horizontal_forward_from_yaw(yaw_radians)
            .scale(forward_impulse)
            .add(horizontal_right_from_yaw(yaw_radians).scale(-left_impulse)),
    )
}

fn horizontal_forward_from_yaw(yaw_radians: f64) -> Vec3d {
    Vec3d::new(yaw_radians.sin(), 0.0, yaw_radians.cos())
}

fn horizontal_right_from_yaw(yaw_radians: f64) -> Vec3d {
    Vec3d::new(yaw_radians.cos(), 0.0, -yaw_radians.sin())
}

fn axis(positive: bool, negative: bool) -> f32 {
    let positive = positive as i32;
    let negative = negative as i32;
    (positive - negative) as f32
}

fn view_forward(yaw_radians: f64, pitch_radians: f64) -> Vec3d {
    let yaw_sin = yaw_radians.sin();
    let yaw_cos = yaw_radians.cos();
    let pitch_sin = pitch_radians.sin();
    let pitch_cos = pitch_radians.cos();
    Vec3d::new(yaw_sin * pitch_cos, pitch_sin, yaw_cos * pitch_cos)
}

fn normalize_vec3d_or_zero(value: Vec3d) -> Vec3d {
    let len_sqr = value.length_sqr();
    if len_sqr <= 1.0e-12 {
        Vec3d::ZERO
    } else {
        value.scale(1.0 / len_sqr.sqrt())
    }
}

fn vec3d_to_f32_array(value: Vec3d) -> [f32; 3] {
    [value.x as f32, value.y as f32, value.z as f32]
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EngineServerUpdateReport {
    pub changed: bool,
    pub updates: usize,
    pub snapshot_updates: usize,
    pub section_block_updates: usize,
    pub unload_updates: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EngineServerUpdateDirtyPolicy {
    pub dirty_chunk_snapshots: bool,
    pub dirty_chunk_unloads: bool,
    pub dirty_section_block_updates: bool,
}

impl EngineServerUpdateDirtyPolicy {
    pub const ALL: Self = Self {
        dirty_chunk_snapshots: true,
        dirty_chunk_unloads: true,
        dirty_section_block_updates: true,
    };

    pub const SECTION_BLOCK_UPDATES_ONLY: Self = Self {
        dirty_chunk_snapshots: false,
        dirty_chunk_unloads: false,
        dirty_section_block_updates: true,
    };
}

impl EngineServerUpdateReport {
    pub fn classify(updates: &[ServerUpdate]) -> Self {
        let mut report = Self {
            updates: updates.len(),
            ..Self::default()
        };
        for update in updates {
            match update {
                ServerUpdate::ChunkSnapshot(_) => {
                    report.changed = true;
                    report.snapshot_updates += 1;
                }
                ServerUpdate::ChunkUnload { .. } => {
                    report.changed = true;
                    report.unload_updates += 1;
                }
                ServerUpdate::SectionBlockUpdates { .. } => {
                    report.changed = true;
                    report.section_block_updates += 1;
                }
                ServerUpdate::PlayerPosition(_) => {
                    report.changed = true;
                }
                ServerUpdate::TimeUpdate { .. }
                | ServerUpdate::RemotePlayerAdd(_)
                | ServerUpdate::RemotePlayerUpdate(_)
                | ServerUpdate::RemotePlayerRemove { .. }
                | ServerUpdate::EntitySnapshot(_)
                | ServerUpdate::EntityUpdate(_)
                | ServerUpdate::EntityRemove { .. } => {}
            }
        }
        report
    }
}

pub fn render_dirty_section_keys_for_block_update(
    pos: ChunkPos,
    section_y: i32,
    update: &SectionBlockUpdate,
) -> BTreeSet<RenderSectionKey> {
    let world_x = chunk_block_coord(pos.x, update.local_x as i32);
    let world_y = section_y * SECTION_HEIGHT + update.local_y as i32;
    let world_z = chunk_block_coord(pos.z, update.local_z as i32);
    let mut keys = BTreeSet::new();
    for z in world_z - 1..=world_z + 1 {
        for x in world_x - 1..=world_x + 1 {
            for y in world_y - 1..=world_y + 1 {
                keys.insert(RenderSectionKey::new(
                    block_to_chunk_coord(x),
                    block_to_section_coord(y),
                    block_to_chunk_coord(z),
                ));
            }
        }
    }
    keys
}

#[derive(Clone, Debug, Default)]
pub struct RenderSectionSession {
    cache: CachedTexturedRenderSections,
    dirty: RenderSectionDirtyState,
}

#[derive(Debug)]
pub struct EngineRenderSession {
    client: ClientRuntime,
    render_session: RenderSectionSession,
}

impl EngineRenderSession {
    pub fn new(client: ClientRuntime) -> Self {
        Self {
            client,
            render_session: RenderSectionSession::default(),
        }
    }

    pub const fn client(&self) -> &ClientRuntime {
        &self.client
    }

    pub const fn client_mut(&mut self) -> &mut ClientRuntime {
        &mut self.client
    }

    pub const fn render_session(&self) -> &RenderSectionSession {
        &self.render_session
    }

    pub const fn render_session_mut(&mut self) -> &mut RenderSectionSession {
        &mut self.render_session
    }

    pub fn mark_chunk_neighborhood_dirty(&mut self, pos: ChunkPos) -> usize {
        let client = &self.client;
        self.render_session
            .mark_chunk_neighborhood_dirty_with_loaded_sections(pos, |dirty_pos| {
                client
                    .chunk_snapshot(dirty_pos)
                    .map(render_section_keys_for_snapshot)
                    .unwrap_or_default()
            })
    }

    pub fn mark_section_dirty(&mut self, key: RenderSectionKey) {
        self.render_session.mark_section_dirty(key);
    }

    pub fn mark_server_update_render_dirty(
        &mut self,
        updates: &[ServerUpdate],
    ) -> EngineServerUpdateReport {
        self.mark_server_update_render_dirty_with_policy(
            updates,
            EngineServerUpdateDirtyPolicy::ALL,
        )
    }

    pub fn mark_server_update_render_dirty_with_policy(
        &mut self,
        updates: &[ServerUpdate],
        policy: EngineServerUpdateDirtyPolicy,
    ) -> EngineServerUpdateReport {
        let report = EngineServerUpdateReport::classify(updates);
        for update in updates {
            match update {
                ServerUpdate::ChunkSnapshot(snapshot) => {
                    if policy.dirty_chunk_snapshots {
                        self.mark_chunk_neighborhood_dirty(snapshot.pos);
                    }
                }
                ServerUpdate::ChunkUnload { pos } => {
                    if policy.dirty_chunk_unloads {
                        self.mark_chunk_neighborhood_dirty(*pos);
                    }
                }
                ServerUpdate::SectionBlockUpdates {
                    pos,
                    section_y,
                    updates,
                } => {
                    if policy.dirty_section_block_updates {
                        for update in updates {
                            for key in
                                render_dirty_section_keys_for_block_update(*pos, *section_y, update)
                            {
                                self.mark_section_dirty(key);
                            }
                        }
                    }
                }
                ServerUpdate::TimeUpdate { .. } => {}
                ServerUpdate::PlayerPosition(_) => {}
                ServerUpdate::RemotePlayerAdd(_)
                | ServerUpdate::RemotePlayerUpdate(_)
                | ServerUpdate::RemotePlayerRemove { .. }
                | ServerUpdate::EntitySnapshot(_)
                | ServerUpdate::EntityUpdate(_)
                | ServerUpdate::EntityRemove { .. } => {}
            }
        }
        report
    }

    pub fn apply_server_updates(&mut self, updates: Vec<ServerUpdate>) -> EngineServerUpdateReport {
        let report = self.mark_server_update_render_dirty(&updates);
        self.client.apply_updates(updates);
        report
    }

    pub fn apply_server_updates_with_dirty_policy(
        &mut self,
        updates: Vec<ServerUpdate>,
        policy: EngineServerUpdateDirtyPolicy,
    ) -> EngineServerUpdateReport {
        let report = self.mark_server_update_render_dirty_with_policy(&updates, policy);
        self.client.apply_updates(updates);
        report
    }

    pub fn clear_client_replica_and_mark_render_dirty(&mut self) -> Vec<ChunkPos> {
        let stale_chunks = self.client.loaded_chunk_positions().collect::<Vec<_>>();
        self.client.clear_server_replica();
        for pos in stale_chunks.iter().copied() {
            self.mark_chunk_neighborhood_dirty(pos);
        }
        stale_chunks
    }

    pub fn mark_loaded_chunks_dirty_when_cache_empty(&mut self) -> bool {
        if !self.render_session.cache_is_empty()
            || self.client.loaded_chunk_count() == 0
            || !self.render_session.dirty_is_empty()
        {
            return false;
        }
        let loaded = self
            .client
            .chunk_snapshots()
            .map(|snapshot| snapshot.pos)
            .collect::<Vec<_>>();
        for pos in loaded {
            self.mark_chunk_neighborhood_dirty(pos);
        }
        true
    }

    pub fn apply_loaded_view_sync<I>(
        &mut self,
        previous_chunks: &BTreeSet<ChunkPos>,
        current_loaded_chunks: BTreeSet<ChunkPos>,
        loaded_section_keys_for_chunk: impl FnMut(ChunkPos) -> I,
    ) -> RenderSectionViewSync
    where
        I: IntoIterator<Item = RenderSectionKey>,
    {
        self.render_session.apply_loaded_view_sync(
            previous_chunks,
            current_loaded_chunks,
            loaded_section_keys_for_chunk,
        )
    }

    pub fn apply_current_loaded_view_sync(
        &mut self,
        previous_chunks: &BTreeSet<ChunkPos>,
    ) -> RenderSectionViewSync {
        let current_loaded_chunks = self
            .client
            .loaded_chunk_positions()
            .collect::<BTreeSet<_>>();
        let client = &self.client;
        self.render_session
            .apply_loaded_view_sync(previous_chunks, current_loaded_chunks, |pos| {
                client
                    .chunk_snapshot(pos)
                    .map(render_section_keys_for_snapshot)
                    .unwrap_or_default()
            })
    }

    pub fn drain_completed_compile_updates(
        &mut self,
        completed_results: impl IntoIterator<Item = RenderSectionCompileResult>,
        request_id_for_result: impl FnMut(&RenderSectionCompileResult) -> u32,
        pending_compile_jobs: usize,
    ) -> Result<RenderSectionCacheUpdate> {
        let client = &self.client;
        self.render_session.drain_completed_compile_updates(
            completed_results,
            request_id_for_result,
            pending_compile_jobs,
            |key| {
                let pos = render_section_chunk_pos(key);
                client
                    .chunk_snapshot(pos)
                    .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
            },
        )
    }

    pub fn prepare_sync_update(
        &mut self,
        order_loaded_dirty_chunks: impl FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        order_dirty_section_chunks: impl FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        chunk_budget: usize,
        section_readiness: impl FnMut(
            &ClientRuntime,
            RenderSectionKey,
        ) -> RenderSectionNeighborReadiness,
        removal_mode: RenderSectionRemovalMode,
    ) -> RenderSectionSyncUpdate {
        let client = &self.client;
        self.render_session.prepare_sync_update(
            |pos| client.chunk_snapshot(pos).is_some(),
            |key| {
                let pos = render_section_chunk_pos(key);
                client
                    .chunk_snapshot(pos)
                    .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
            },
            order_loaded_dirty_chunks,
            order_dirty_section_chunks,
            chunk_budget,
            |pos| {
                client
                    .chunk_snapshot(pos)
                    .map(render_section_keys_for_snapshot)
                    .unwrap_or_default()
            },
            {
                let mut section_readiness = section_readiness;
                move |key| section_readiness(client, key)
            },
            removal_mode,
        )
    }

    pub fn submit_prepared_sync_plan<T, E>(
        &mut self,
        sync_plan: &RenderSectionSyncPlan,
        snapshots: Vec<ChunkSnapshot>,
        submit: impl FnOnce(
            &RenderSectionSyncPlan,
            RenderSectionCompileRequest,
        ) -> std::result::Result<T, E>,
    ) -> std::result::Result<RenderSectionReadyWorkSubmission<T>, E> {
        self.render_session
            .submit_prepared_sync_plan(sync_plan, snapshots, submit)
    }

    pub fn finish_compile_update(
        &mut self,
        completed: RenderSectionCompileResult,
        request_id: u32,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_sections: &BTreeSet<RenderSectionKey>,
    ) -> Result<RenderSectionFinishedCompileUpdate> {
        let client = &self.client;
        self.render_session.finish_compile_update(
            completed,
            request_id,
            removal_chunks,
            removal_sections,
            |key| {
                let pos = render_section_chunk_pos(key);
                client
                    .chunk_snapshot(pos)
                    .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
            },
        )
    }

    /// Drive one budgeted increment of the shared render-section streaming loop over
    /// `compiler` (067 Stage 3). This is the single compile loop both platforms run:
    ///
    /// 1. drain the compiler's completed jobs and merge them into the resident cache,
    /// 2. seed dirty work when the cache is empty (first frame after a view loads),
    /// 3. plan a `chunk_budget`-bounded ready increment using the caller's ordering,
    ///    readiness, and removal-mode policy, then
    /// 4. submit that increment through `compiler` — but only when no job is already in
    ///    flight, so exactly one compile runs at a time.
    ///
    /// Desktop drives it at budget 1 over an OS-thread `mpsc` worker; web drives it at
    /// budget 1 over the resident `SharedArrayBuffer` ring. With budget 1 each job is
    /// tiny, so the current cache stays visible and movement fills progressively instead
    /// of stalling on a whole-view mega-job. The transport (move vs. arena) lives behind
    /// the [`RenderSectionCompiler`] trait; this loop is identical on both platforms.
    pub fn sync_render_sections_with_budget<C, OrderChunks, OrderSections, Ready, Snapshots>(
        &mut self,
        compiler: &mut C,
        chunk_budget: usize,
        order_loaded_dirty_chunks: OrderChunks,
        order_dirty_section_chunks: OrderSections,
        section_readiness: Ready,
        removal_mode: RenderSectionRemovalMode,
        snapshots_for_submit: Snapshots,
    ) -> Result<RenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
        OrderChunks: FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        OrderSections: FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        Ready: FnMut(&ClientRuntime, RenderSectionKey) -> RenderSectionNeighborReadiness,
        // The snapshot step receives the compiler as well as the client so a platform that
        // keeps a worker-side snapshot mirror (web) can diff the live snapshots against its
        // own shadow and return only the changed columns, instead of deep-cloning every
        // loaded column each frame. Desktop ignores the compiler and clones all columns (the
        // owned `Vec` is moved over `mpsc` to the worker thread, so the clone is load-bearing).
        Snapshots: FnOnce(&ClientRuntime, &mut C) -> Vec<ChunkSnapshot>,
    {
        let completed_results = compiler.try_recv_completed()?;
        let pending_compile_jobs = compiler.pending_job_count();
        let mut report =
            self.drain_completed_compile_updates(completed_results, |_| 0, pending_compile_jobs)?;

        self.mark_loaded_chunks_dirty_when_cache_empty();

        if self.render_session().dirty_work_is_empty() {
            report.pending_compile_jobs = compiler.pending_job_count();
            return Ok(report);
        }

        let sync_update = self.prepare_sync_update(
            order_loaded_dirty_chunks,
            order_dirty_section_chunks,
            chunk_budget,
            section_readiness,
            removal_mode,
        );
        report.merge(sync_update.cache_update);

        if chunk_budget == 0 || compiler.pending_job_count() > 0 {
            report.pending_compile_jobs = compiler.pending_job_count();
            return Ok(report);
        }

        let sync_plan = sync_update.sync_plan;
        let snapshots = snapshots_for_submit(self.client(), compiler);
        let submission_update =
            self.submit_prepared_sync_plan(&sync_plan, snapshots, |_sync_plan, request| {
                compiler.submit(request)
            })?;
        report.merge(submission_update.cache_update);
        report.pending_compile_jobs = compiler.pending_job_count();
        Ok(report)
    }

    /// True when there is render work the streaming loop could still make progress on:
    /// a job in flight, or dirty work whose sections are ready to compile. Drives the
    /// "pump to idle" loops (desktop `sync_all_render_sections`, the web deterministic
    /// smoke) without re-deriving readiness in the caller.
    pub fn has_pending_render_work(
        &self,
        compiler_pending_job_count: usize,
        mut section_readiness: impl FnMut(
            &ClientRuntime,
            RenderSectionKey,
        ) -> RenderSectionNeighborReadiness,
    ) -> bool {
        if compiler_pending_job_count > 0 {
            return true;
        }
        let client = &self.client;
        self.render_session
            .has_ready_pending_dirty_work(client, |key| section_readiness(client, key))
    }

    pub fn sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.render_session.sections()
    }
}

impl RenderSectionSession {
    pub fn cache_is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn dirty_is_empty(&self) -> bool {
        self.dirty.is_empty()
    }

    pub fn dirty_work_is_empty(&self) -> bool {
        self.dirty.dirty_work_is_empty()
    }

    pub fn dirty(&self) -> &RenderSectionDirtyState {
        &self.dirty
    }

    pub fn dirty_mut(&mut self) -> &mut RenderSectionDirtyState {
        &mut self.dirty
    }

    /// Whether any dirty chunk/section is ready to make compile progress this frame —
    /// either a cached chunk/section that is no longer loaded (removal work) or a loaded,
    /// not-in-flight section whose neighbors are ready. Mirrors the planning gate so the
    /// "pump to idle" loops can stop exactly when the streaming loop would idle.
    pub fn has_ready_pending_dirty_work(
        &self,
        client: &ClientRuntime,
        mut section_readiness: impl FnMut(RenderSectionKey) -> RenderSectionNeighborReadiness,
    ) -> bool {
        let ready_chunk = self.dirty.dirty_chunks.iter().copied().any(|pos| {
            if self.contains_chunk(pos) && client.chunk_snapshot(pos).is_none() {
                return true;
            }
            let Some(snapshot) = client.chunk_snapshot(pos) else {
                return false;
            };
            render_section_keys_for_snapshot(snapshot)
                .into_iter()
                .any(|key| {
                    !self.dirty.inflight_sections.contains(&key)
                        && section_readiness(key).is_ready()
                })
        });
        if ready_chunk {
            return true;
        }
        self.dirty.dirty_sections.iter().copied().any(|key| {
            if self.dirty.inflight_sections.contains(&key) {
                return false;
            }
            let pos = render_section_chunk_pos(key);
            if self.contains_section(key) && client.chunk_snapshot(pos).is_none() {
                return true;
            }
            client
                .chunk_snapshot(pos)
                .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
                && section_readiness(key).is_ready()
        })
    }

    pub fn mark_chunk_dirty(
        &mut self,
        pos: ChunkPos,
        known_section_keys: impl IntoIterator<Item = RenderSectionKey>,
    ) {
        self.dirty.mark_chunk_dirty(pos, known_section_keys);
    }

    pub fn mark_chunk_dirty_with_loaded_sections(
        &mut self,
        pos: ChunkPos,
        loaded_section_keys: impl IntoIterator<Item = RenderSectionKey>,
    ) {
        let known_section_keys = self.known_section_keys_for_chunk(pos, loaded_section_keys);
        self.mark_chunk_dirty(pos, known_section_keys);
    }

    pub fn mark_chunks_dirty_with_loaded_sections<I>(
        &mut self,
        chunks: impl IntoIterator<Item = ChunkPos>,
        mut loaded_section_keys_for_chunk: impl FnMut(ChunkPos) -> I,
    ) -> usize
    where
        I: IntoIterator<Item = RenderSectionKey>,
    {
        let chunks = chunks.into_iter().collect::<BTreeSet<_>>();
        let marked_chunk_count = chunks.len();
        for pos in chunks {
            self.mark_chunk_dirty_with_loaded_sections(pos, loaded_section_keys_for_chunk(pos));
        }
        marked_chunk_count
    }

    pub fn mark_chunk_neighborhood_dirty_with_loaded_sections<I>(
        &mut self,
        pos: ChunkPos,
        loaded_section_keys_for_chunk: impl FnMut(ChunkPos) -> I,
    ) -> usize
    where
        I: IntoIterator<Item = RenderSectionKey>,
    {
        self.mark_chunks_dirty_with_loaded_sections(
            render_dirty_chunk_neighborhood(pos),
            loaded_section_keys_for_chunk,
        )
    }

    pub fn mark_view_sync_dirty_with_loaded_sections<I>(
        &mut self,
        sync: &RenderSectionViewSync,
        loaded_section_keys_for_chunk: impl FnMut(ChunkPos) -> I,
    ) -> usize
    where
        I: IntoIterator<Item = RenderSectionKey>,
    {
        self.mark_chunks_dirty_with_loaded_sections(
            sync.dirty_chunks
                .iter()
                .chain(sync.removal_chunks.iter())
                .copied(),
            loaded_section_keys_for_chunk,
        )
    }

    pub fn apply_loaded_view_sync<I>(
        &mut self,
        previous_chunks: &BTreeSet<ChunkPos>,
        current_loaded_chunks: BTreeSet<ChunkPos>,
        loaded_section_keys_for_chunk: impl FnMut(ChunkPos) -> I,
    ) -> RenderSectionViewSync
    where
        I: IntoIterator<Item = RenderSectionKey>,
    {
        let sync =
            RenderSectionViewSync::from_loaded_chunks(previous_chunks, current_loaded_chunks);
        self.mark_view_sync_dirty_with_loaded_sections(&sync, loaded_section_keys_for_chunk);
        sync
    }

    pub fn mark_section_dirty(&mut self, key: RenderSectionKey) {
        self.dirty.mark_section_dirty(key);
    }

    pub fn sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.cache.sections()
    }

    pub fn section_keys(&self) -> impl Iterator<Item = RenderSectionKey> + '_ {
        self.cache.section_keys()
    }

    pub fn contains_chunk(&self, pos: ChunkPos) -> bool {
        self.cache.contains_chunk(pos)
    }

    pub fn contains_section(&self, key: RenderSectionKey) -> bool {
        self.cache.contains_section(key)
    }

    pub fn known_section_keys_for_chunk(
        &self,
        pos: ChunkPos,
        loaded_section_keys: impl IntoIterator<Item = RenderSectionKey>,
    ) -> BTreeSet<RenderSectionKey> {
        let mut keys = BTreeSet::new();
        keys.extend(loaded_section_keys);
        keys.extend(
            self.cache
                .section_keys()
                .filter(|key| render_section_chunk_pos(*key) == pos),
        );
        keys.extend(
            self.dirty
                .dirty_sections
                .iter()
                .copied()
                .filter(|key| render_section_chunk_pos(*key) == pos),
        );
        keys.extend(
            self.dirty
                .inflight_sections
                .iter()
                .copied()
                .filter(|key| render_section_chunk_pos(*key) == pos),
        );
        keys
    }

    pub fn classify_dirty_work(
        &self,
        chunk_is_loaded: impl FnMut(ChunkPos) -> bool,
        chunk_is_cached: impl FnMut(ChunkPos) -> bool,
        section_is_loaded: impl FnMut(RenderSectionKey) -> bool,
        section_is_cached: impl FnMut(RenderSectionKey) -> bool,
    ) -> RenderSectionDirtyWork {
        classify_render_section_dirty_work(
            &self.dirty,
            chunk_is_loaded,
            chunk_is_cached,
            section_is_loaded,
            section_is_cached,
        )
    }

    pub fn prepare_sync_update(
        &mut self,
        chunk_is_loaded: impl FnMut(ChunkPos) -> bool,
        section_is_loaded: impl FnMut(RenderSectionKey) -> bool,
        order_loaded_dirty_chunks: impl FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        order_dirty_section_chunks: impl FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        chunk_budget: usize,
        section_keys_for_chunk: impl FnMut(ChunkPos) -> Vec<RenderSectionKey>,
        section_readiness: impl FnMut(RenderSectionKey) -> RenderSectionNeighborReadiness,
        removal_mode: RenderSectionRemovalMode,
    ) -> RenderSectionSyncUpdate {
        let dirty_work = classify_render_section_dirty_work(
            &self.dirty,
            chunk_is_loaded,
            |pos| self.cache.contains_chunk(pos),
            section_is_loaded,
            |key| self.cache.contains_section(key),
        );
        let sorted_loaded_dirty_chunks = order_loaded_dirty_chunks(&dirty_work);
        let sorted_dirty_section_chunks = order_dirty_section_chunks(&dirty_work);
        let sync_plan = self.prepare_sync_plan(
            dirty_work,
            sorted_loaded_dirty_chunks,
            sorted_dirty_section_chunks,
            chunk_budget,
            section_keys_for_chunk,
            section_readiness,
        );
        let mut cache_update = RenderSectionCacheUpdate::default();
        if removal_mode == RenderSectionRemovalMode::ApplyImmediately && sync_plan.has_removals() {
            cache_update.merge(self.apply_removals(
                &sync_plan.dirty_work.removal_dirty_chunks,
                &sync_plan.dirty_work.removal_dirty_sections,
            ));
        }
        RenderSectionSyncUpdate {
            sync_plan,
            cache_update,
        }
    }

    pub fn prepare_sync_plan(
        &mut self,
        dirty_work: RenderSectionDirtyWork,
        sorted_loaded_dirty_chunks: impl IntoIterator<Item = ChunkPos>,
        sorted_dirty_section_chunks: impl IntoIterator<Item = ChunkPos>,
        chunk_budget: usize,
        section_keys_for_chunk: impl FnMut(ChunkPos) -> Vec<RenderSectionKey>,
        section_readiness: impl FnMut(RenderSectionKey) -> RenderSectionNeighborReadiness,
    ) -> RenderSectionSyncPlan {
        prepare_render_section_sync_plan(
            &mut self.dirty,
            dirty_work,
            sorted_loaded_dirty_chunks,
            sorted_dirty_section_chunks,
            chunk_budget,
            section_keys_for_chunk,
            section_readiness,
        )
    }

    pub fn apply_ready_plan(&mut self, plan: &RenderSectionReadyPlan) {
        self.dirty.apply_ready_plan(plan);
    }

    pub fn build_ready_plan_compile_request(
        &self,
        plan: &RenderSectionReadyPlan,
        snapshots: Vec<ChunkSnapshot>,
    ) -> Option<RenderSectionCompileRequest> {
        self.dirty.build_ready_plan_compile_request(plan, snapshots)
    }

    pub fn accept_ready_plan_compile_submission(&mut self, plan: &RenderSectionReadyPlan) {
        self.dirty.accept_ready_plan_compile_submission(plan);
    }

    pub fn submit_ready_plan_compile_request<T, E>(
        &mut self,
        plan: &RenderSectionReadyPlan,
        snapshots: Vec<ChunkSnapshot>,
        submit: impl FnOnce(RenderSectionCompileRequest) -> std::result::Result<T, E>,
    ) -> std::result::Result<Option<RenderSectionCompileSubmission<T>>, E> {
        let Some(request) = self.build_ready_plan_compile_request(plan, snapshots) else {
            return Ok(None);
        };
        let submitted_section_count = request.target_sections.len();
        let output = submit(request)?;
        self.accept_ready_plan_compile_submission(plan);
        Ok(Some(RenderSectionCompileSubmission {
            submitted_section_count,
            output,
        }))
    }

    pub fn submit_prepared_sync_plan<T, E>(
        &mut self,
        sync_plan: &RenderSectionSyncPlan,
        snapshots: Vec<ChunkSnapshot>,
        submit: impl FnOnce(
            &RenderSectionSyncPlan,
            RenderSectionCompileRequest,
        ) -> std::result::Result<T, E>,
    ) -> std::result::Result<RenderSectionReadyWorkSubmission<T>, E> {
        let mut cache_update = RenderSectionCacheUpdate::default();
        let Some(request) = self.build_ready_plan_compile_request(&sync_plan.ready_plan, snapshots)
        else {
            self.apply_ready_plan(&sync_plan.ready_plan);
            cache_update.merge(sync_plan.ready_update(0));
            return Ok(RenderSectionReadyWorkSubmission {
                cache_update,
                submission: None,
            });
        };
        let submitted_section_count = request.target_sections.len();
        let output = submit(sync_plan, request)?;
        self.accept_ready_plan_compile_submission(&sync_plan.ready_plan);
        cache_update.merge(sync_plan.ready_update(submitted_section_count));
        Ok(RenderSectionReadyWorkSubmission {
            cache_update,
            submission: Some(RenderSectionCompileSubmission {
                submitted_section_count,
                output,
            }),
        })
    }

    pub fn finish_compile_result(
        &mut self,
        completed: RenderSectionCompileResult,
        request_id: u32,
    ) -> Result<RenderSectionCompileFinish> {
        finish_render_section_compile_result(&mut self.dirty, completed, request_id)
    }

    pub fn finish_compile_update(
        &mut self,
        completed: RenderSectionCompileResult,
        request_id: u32,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_sections: &BTreeSet<RenderSectionKey>,
        section_is_loaded: impl FnMut(RenderSectionKey) -> bool,
    ) -> Result<RenderSectionFinishedCompileUpdate> {
        let RenderSectionCompileFinish {
            acceptance_report,
            accepted_sections,
            stale_sections,
            build_report,
        } = self.finish_compile_result(completed, request_id)?;
        let mut cache_update = RenderSectionCacheUpdate::default();
        if !stale_sections.is_empty() {
            cache_update.stale_compile_section_count += stale_sections.len();
            self.requeue_stale_sections(stale_sections.clone(), section_is_loaded);
        }
        if let Some(build_report) = build_report {
            cache_update.merge(self.apply_finished_compile_report(
                &accepted_sections,
                build_report,
                removal_chunks,
                removal_sections,
            ));
        } else if !removal_chunks.is_empty() || !removal_sections.is_empty() {
            cache_update.merge(self.apply_removals(removal_chunks, removal_sections));
        }
        Ok(RenderSectionFinishedCompileUpdate {
            acceptance_report,
            accepted_sections,
            stale_sections,
            cache_update,
        })
    }

    pub fn drain_completed_compile_updates(
        &mut self,
        completed_results: impl IntoIterator<Item = RenderSectionCompileResult>,
        mut request_id_for_result: impl FnMut(&RenderSectionCompileResult) -> u32,
        pending_compile_jobs: usize,
        mut section_is_loaded: impl FnMut(RenderSectionKey) -> bool,
    ) -> Result<RenderSectionCacheUpdate> {
        let mut update = RenderSectionCacheUpdate::default();
        for completed in completed_results {
            let request_id = request_id_for_result(&completed);
            let finished = self.finish_compile_update(
                completed,
                request_id,
                &BTreeSet::new(),
                &BTreeSet::new(),
                |key| section_is_loaded(key),
            )?;
            update.merge(finished.cache_update);
        }
        update.pending_compile_jobs = pending_compile_jobs;
        Ok(update)
    }

    pub fn apply_finished_compile_report(
        &mut self,
        accepted_sections: &BTreeSet<RenderSectionKey>,
        build_report: TexturedRenderSectionBuildReport,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_sections: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        let update = self.cache.apply_build_report(
            accepted_sections,
            build_report,
            removal_chunks,
            removal_sections,
        );
        self.dirty
            .discard_removed_dirty_work(removal_chunks, removal_sections);
        update
    }

    pub fn apply_removals(
        &mut self,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_sections: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        let update = self.cache.remove_sections(removal_chunks, removal_sections);
        self.dirty
            .discard_removed_dirty_work(removal_chunks, removal_sections);
        update
    }

    pub fn requeue_stale_sections(
        &mut self,
        target_sections: BTreeSet<RenderSectionKey>,
        mut section_is_loaded: impl FnMut(RenderSectionKey) -> bool,
    ) -> usize {
        let mut requeued = 0;
        for key in target_sections {
            if section_is_loaded(key) || self.cache.contains_section(key) {
                self.dirty.dirty_sections.insert(key);
                requeued += 1;
            }
        }
        requeued
    }
}

fn plan_ready_render_section_key(
    plan: &mut RenderSectionReadyPlan,
    key: RenderSectionKey,
    inflight_sections: &BTreeSet<RenderSectionKey>,
    section_readiness: &mut impl FnMut(RenderSectionKey) -> RenderSectionNeighborReadiness,
) {
    if inflight_sections.contains(&key) {
        plan.deferred_section_keys.insert(key);
        return;
    }

    let readiness = section_readiness(key);
    if readiness.is_ready() {
        if readiness.is_near_exception() {
            plan.near_exception_section_count += 1;
        }
        plan.ready_section_keys.insert(key);
    } else {
        plan.deferred_section_count += 1;
        plan.deferred_section_keys.insert(key);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderSectionViewSync {
    pub current_loaded_chunks: BTreeSet<ChunkPos>,
    pub dirty_chunks: BTreeSet<ChunkPos>,
    pub removal_chunks: BTreeSet<ChunkPos>,
}

impl RenderSectionViewSync {
    pub fn from_loaded_chunks(
        previous_chunks: &BTreeSet<ChunkPos>,
        current_loaded_chunks: BTreeSet<ChunkPos>,
    ) -> Self {
        let dirty_chunks = dirty_chunk_positions(previous_chunks, &current_loaded_chunks);
        let removal_chunks = previous_chunks
            .difference(&current_loaded_chunks)
            .copied()
            .collect::<BTreeSet<_>>();
        Self {
            current_loaded_chunks,
            dirty_chunks,
            removal_chunks,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderSectionPendingCompileRequest<C> {
    pub request_id: u32,
    pub context: C,
    pub target_sections: BTreeSet<RenderSectionKey>,
    pub section_revisions: BTreeMap<RenderSectionKey, u64>,
}

impl<C> RenderSectionPendingCompileRequest<C> {
    pub fn into_compile_result(
        self,
        result: std::result::Result<TexturedRenderSectionBuildReport, String>,
    ) -> (C, RenderSectionCompileResult) {
        (
            self.context,
            RenderSectionCompileResult {
                target_sections: self.target_sections,
                section_revisions: self.section_revisions,
                result,
            },
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionCompileRequestInfo {
    pub request_id: u32,
    pub submitted_section_count: usize,
    pub pending_compile_jobs: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionCompileAcceptanceReport {
    pub request_id: u32,
    pub submitted_section_count: usize,
    pub accepted_section_count: usize,
    pub stale_section_count: usize,
}

impl RenderSectionCompileAcceptanceReport {
    pub fn from_acceptance(request_id: u32, acceptance: &RenderSectionCompileAcceptance) -> Self {
        Self {
            request_id,
            submitted_section_count: acceptance.accepted_section_count()
                + acceptance.stale_section_count(),
            accepted_section_count: acceptance.accepted_section_count(),
            stale_section_count: acceptance.stale_section_count(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderSectionCompileRequestState<C> {
    next_request_id: u32,
    pending_requests: BTreeMap<u32, RenderSectionPendingCompileRequest<C>>,
    section_revisions: BTreeMap<RenderSectionKey, u64>,
}

impl<C> Default for RenderSectionCompileRequestState<C> {
    fn default() -> Self {
        Self {
            next_request_id: 1,
            pending_requests: BTreeMap::new(),
            section_revisions: BTreeMap::new(),
        }
    }
}

impl<C> RenderSectionCompileRequestState<C> {
    pub fn pending_request_count(&self) -> usize {
        self.pending_requests.len()
    }

    pub fn has_pending_requests(&self) -> bool {
        !self.pending_requests.is_empty()
    }

    pub fn begin_request(
        &mut self,
        context: C,
        target_sections: BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCompileRequestInfo {
        self.bump_section_revisions(&target_sections);
        let section_revisions = target_sections
            .iter()
            .map(|key| (*key, self.section_revision(*key)))
            .collect::<BTreeMap<_, _>>();
        self.begin_compile_request(
            context,
            RenderSectionCompileRequest {
                target_sections,
                section_revisions,
                snapshots: Vec::new(),
            },
        )
    }

    pub fn begin_compile_request(
        &mut self,
        context: C,
        request: RenderSectionCompileRequest,
    ) -> RenderSectionCompileRequestInfo {
        let request_id = self.take_next_request_id();
        let submitted_section_count = request.target_sections.len();
        self.pending_requests.insert(
            request_id,
            RenderSectionPendingCompileRequest {
                request_id,
                context,
                target_sections: request.target_sections,
                section_revisions: request.section_revisions,
            },
        );
        RenderSectionCompileRequestInfo {
            request_id,
            submitted_section_count,
            pending_compile_jobs: self.pending_request_count(),
        }
    }

    pub fn remove_pending_request(
        &mut self,
        request_id: u32,
    ) -> Option<RenderSectionPendingCompileRequest<C>> {
        self.pending_requests.remove(&request_id)
    }

    pub fn bump_section_revisions(&mut self, keys: &BTreeSet<RenderSectionKey>) {
        for key in keys {
            let revision = self.section_revisions.entry(*key).or_default();
            *revision = revision.wrapping_add(1);
        }
    }

    pub fn section_revision(&self, key: RenderSectionKey) -> u64 {
        self.section_revisions
            .get(&key)
            .copied()
            .unwrap_or_default()
    }

    fn take_next_request_id(&mut self) -> u32 {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        request_id
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PackedRenderSectionBuildReportSummary {
    pub section_count: usize,
    pub non_empty_section_count: usize,
    pub vertex_count: u32,
    pub index_count: u32,
    pub visibility_graph_stats: VisibilityGraphBuildStats,
}

impl PackedRenderSectionBuildReportSummary {
    pub fn face_count(self) -> u32 {
        quad_face_count_from_indices(self.index_count)
    }
}

pub fn summarize_textured_render_section_build_report(
    report: &TexturedRenderSectionBuildReport,
) -> PackedRenderSectionBuildReportSummary {
    let mut summary = PackedRenderSectionBuildReportSummary {
        section_count: report.sections.len(),
        visibility_graph_stats: report.visibility_graph,
        ..PackedRenderSectionBuildReportSummary::default()
    };
    for section in &report.sections {
        if section.is_empty() {
            continue;
        }
        let stats = section.stats();
        summary.non_empty_section_count += 1;
        summary.vertex_count += stats.vertex_count;
        summary.index_count += stats.index_count;
    }
    summary
}

pub fn encode_textured_render_section_build_report(
    report: &TexturedRenderSectionBuildReport,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(PACKED_BUILD_REPORT_MAGIC);
    write_u64(&mut out, report.visibility_graph.build_count as u64);
    write_f64(&mut out, report.visibility_graph.total_ms);
    write_f64(&mut out, report.visibility_graph.worst_ms);
    write_u32(&mut out, report.sections.len() as u32);
    for section in &report.sections {
        write_i32(&mut out, section.key.chunk_x);
        write_i32(&mut out, section.key.section_y);
        write_i32(&mut out, section.key.chunk_z);
        write_u64(&mut out, section.visibility.bits());
        write_u32(&mut out, section.mesh.vertices.len() as u32);
        write_u32(&mut out, section.mesh.indices.len() as u32);
        write_u32(&mut out, section.mesh.opaque_index_count());
        for vertex in &section.mesh.vertices {
            for value in vertex.position {
                write_f32(&mut out, value);
            }
            for value in vertex.uv {
                write_f32(&mut out, value);
            }
            for value in vertex.color {
                write_f32(&mut out, value);
            }
            write_u32(&mut out, vertex.packed_light);
        }
        for index in &section.mesh.indices {
            write_u32(&mut out, *index);
        }
    }
    out
}

pub fn decode_textured_render_section_build_report(
    bytes: &[u8],
) -> Result<TexturedRenderSectionBuildReport> {
    let mut reader = PackedReportReader::new(bytes)?;
    let build_count =
        usize::try_from(reader.read_u64()?).context("visibility graph build count is too large")?;
    let total_ms = reader.read_f64()?;
    let worst_ms = reader.read_f64()?;
    let section_count = reader.read_u32()? as usize;
    let mut sections = Vec::with_capacity(section_count);
    for _ in 0..section_count {
        let key = RenderSectionKey::new(reader.read_i32()?, reader.read_i32()?, reader.read_i32()?);
        let visibility = VisibilitySet::from_bits(reader.read_u64()?);
        let vertex_count = reader.read_u32()? as usize;
        let index_count = reader.read_u32()? as usize;
        let opaque_index_count = reader.read_u32()?;
        let mut vertices = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            let position = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
            let uv = [reader.read_f32()?, reader.read_f32()?];
            let color = [
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
            ];
            let packed_light = reader.read_u32()?;
            vertices.push(TexturedChunkVertex {
                position,
                uv,
                color,
                packed_light,
            });
        }
        let mut indices = Vec::with_capacity(index_count);
        for _ in 0..index_count {
            indices.push(reader.read_u32()?);
        }
        sections.push(TexturedRenderSectionMesh {
            key,
            mesh: TexturedVisibleChunkMesh {
                vertices,
                indices,
                opaque_index_count,
            },
            visibility,
        });
    }
    reader.finish()?;

    Ok(TexturedRenderSectionBuildReport {
        sections,
        visibility_graph: VisibilityGraphBuildStats {
            build_count,
            total_ms,
            worst_ms,
        },
    })
}

pub trait RenderSectionCompiler {
    fn submit(&mut self, request: RenderSectionCompileRequest) -> Result<()>;

    fn try_recv_completed(&mut self) -> Result<Vec<RenderSectionCompileResult>>;

    fn pending_job_count(&self) -> usize;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionCompileAcceptance {
    pub accepted_sections: BTreeSet<RenderSectionKey>,
    pub stale_sections: BTreeSet<RenderSectionKey>,
}

impl RenderSectionCompileAcceptance {
    pub fn accepted_section_count(&self) -> usize {
        self.accepted_sections.len()
    }

    pub fn stale_section_count(&self) -> usize {
        self.stale_sections.len()
    }
}

impl RenderSectionCompileResult {
    pub fn partition_by_revision(
        &self,
        mut current_revision: impl FnMut(RenderSectionKey) -> u64,
    ) -> RenderSectionCompileAcceptance {
        let mut accepted_sections = BTreeSet::new();
        let mut stale_sections = BTreeSet::new();
        for key in &self.target_sections {
            let submitted_revision = self.section_revisions.get(key).copied().unwrap_or_default();
            if current_revision(*key) == submitted_revision {
                accepted_sections.insert(*key);
            } else {
                stale_sections.insert(*key);
            }
        }

        RenderSectionCompileAcceptance {
            accepted_sections,
            stale_sections,
        }
    }
}

pub fn render_section_chunk_pos(key: RenderSectionKey) -> ChunkPos {
    ChunkPos::new(key.chunk_x, key.chunk_z)
}

pub fn render_section_keys_for_snapshot(snapshot: &ChunkSnapshot) -> Vec<RenderSectionKey> {
    let min_section_y = snapshot.min_y.div_euclid(SECTION_HEIGHT);
    let section_count = snapshot.height / SECTION_HEIGHT;
    (0..section_count)
        .map(|offset| RenderSectionKey::new(snapshot.pos.x, min_section_y + offset, snapshot.pos.z))
        .collect()
}

pub fn snapshot_contains_render_section(snapshot: &ChunkSnapshot, key: RenderSectionKey) -> bool {
    snapshot.pos == render_section_chunk_pos(key)
        && key.section_y * SECTION_HEIGHT >= snapshot.min_y
        && key.section_y * SECTION_HEIGHT < snapshot.min_y + snapshot.height
}

pub fn ready_section_keys_for_report(
    report: &TexturedRenderSectionBuildReport,
    dirty_chunks: &BTreeSet<ChunkPos>,
) -> BTreeSet<RenderSectionKey> {
    report
        .sections
        .iter()
        .map(|section| section.key)
        .filter(|key| dirty_chunks.contains(&render_section_chunk_pos(*key)))
        .collect()
}

pub fn target_section_keys_for_dirty_chunks<'a>(
    snapshots: impl IntoIterator<Item = &'a ChunkSnapshot>,
    dirty_chunks: &BTreeSet<ChunkPos>,
) -> BTreeSet<RenderSectionKey> {
    snapshots
        .into_iter()
        .filter(|snapshot| dirty_chunks.contains(&snapshot.pos))
        .flat_map(render_section_keys_for_snapshot)
        .collect()
}

pub fn dirty_chunk_positions(
    previous_chunks: &BTreeSet<ChunkPos>,
    current_chunks: &BTreeSet<ChunkPos>,
) -> BTreeSet<ChunkPos> {
    if previous_chunks.is_empty() {
        return current_chunks.clone();
    }
    let added_chunks = current_chunks
        .difference(previous_chunks)
        .copied()
        .collect::<BTreeSet<_>>();
    let removed_chunks = previous_chunks
        .difference(current_chunks)
        .copied()
        .collect::<BTreeSet<_>>();
    current_chunks
        .iter()
        .copied()
        .filter(|pos| added_chunks.contains(pos) || has_removed_neighbor(*pos, &removed_chunks))
        .collect()
}

pub fn render_dirty_chunk_neighborhood(pos: ChunkPos) -> [ChunkPos; 5] {
    [
        pos,
        ChunkPos::new(pos.x - 1, pos.z),
        ChunkPos::new(pos.x + 1, pos.z),
        ChunkPos::new(pos.x, pos.z - 1),
        ChunkPos::new(pos.x, pos.z + 1),
    ]
}

fn has_removed_neighbor(pos: ChunkPos, removed_chunks: &BTreeSet<ChunkPos>) -> bool {
    [
        ChunkPos::new(pos.x - 1, pos.z),
        ChunkPos::new(pos.x + 1, pos.z),
        ChunkPos::new(pos.x, pos.z - 1),
        ChunkPos::new(pos.x, pos.z + 1),
    ]
    .into_iter()
    .any(|neighbor| removed_chunks.contains(&neighbor))
}

fn write_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_f64(out: &mut Vec<u8>, value: f64) {
    out.extend_from_slice(&value.to_le_bytes());
}

struct PackedReportReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> PackedReportReader<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self> {
        if !bytes.starts_with(PACKED_BUILD_REPORT_MAGIC) {
            bail!("packed render section build report has invalid magic/version");
        }
        Ok(Self {
            bytes,
            offset: PACKED_BUILD_REPORT_MAGIC.len(),
        })
    }

    fn finish(self) -> Result<()> {
        if self.offset != self.bytes.len() {
            bail!(
                "packed render section build report has {} trailing bytes",
                self.bytes.len() - self.offset
            );
        }
        Ok(())
    }

    fn read_i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.read_array()?))
    }

    fn read_u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }

    fn read_u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }

    fn read_f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.read_array()?))
    }

    fn read_f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.read_array()?))
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.offset + N;
        if end > self.bytes.len() {
            bail!("packed render section build report ended unexpectedly");
        }
        let mut out = [0_u8; N];
        out.copy_from_slice(&self.bytes[self.offset..end]);
        self.offset = end;
        Ok(out)
    }
}

impl CachedTexturedRenderSections {
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    pub fn contains_chunk(&self, pos: ChunkPos) -> bool {
        self.sections
            .keys()
            .any(|key| key.chunk_x == pos.x && key.chunk_z == pos.z)
    }

    pub fn contains_section(&self, key: RenderSectionKey) -> bool {
        self.sections.contains_key(&key)
    }

    pub fn sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.sections.values().cloned().collect()
    }

    pub fn section_keys(&self) -> impl Iterator<Item = RenderSectionKey> + '_ {
        self.sections.keys().copied()
    }

    pub fn apply_build_report(
        &mut self,
        ready_section_keys: &BTreeSet<RenderSectionKey>,
        rebuilt_report: TexturedRenderSectionBuildReport,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_section_keys: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        let rebuilt_keys = rebuilt_report
            .sections
            .iter()
            .map(|section| section.key)
            .collect::<BTreeSet<_>>();
        let rebuilt_sections = rebuilt_report
            .sections
            .into_iter()
            .filter(|section| ready_section_keys.contains(&section.key))
            .collect::<Vec<_>>();
        let old_ready_keys = self
            .sections
            .keys()
            .copied()
            .filter(|key| ready_section_keys.contains(key))
            .collect::<BTreeSet<_>>();
        let removal_keys = self
            .sections
            .keys()
            .copied()
            .filter(|key| removal_chunks.contains(&ChunkPos::new(key.chunk_x, key.chunk_z)))
            .collect::<BTreeSet<_>>();
        let mut removed_section_keys = old_ready_keys
            .difference(&rebuilt_keys)
            .copied()
            .collect::<BTreeSet<_>>();
        removed_section_keys.extend(removal_keys);
        removed_section_keys.extend(removal_section_keys.iter().copied());

        for key in &removed_section_keys {
            self.sections.remove(key);
        }

        let mut report = RenderSectionCacheUpdate {
            rebuilt_sections,
            removed_section_keys,
            rebuilt_vertex_count: 0,
            rebuilt_index_count: 0,
            visibility_graph_stats: rebuilt_report.visibility_graph,
            neighbor_ready_section_count: ready_section_keys.len(),
            completed_compile_section_count: ready_section_keys.len(),
            ..RenderSectionCacheUpdate::default()
        };
        for section in &report.rebuilt_sections {
            let stats = section.stats();
            report.rebuilt_vertex_count += stats.vertex_count;
            report.rebuilt_index_count += stats.index_count;
            self.sections.insert(section.key, section.clone());
        }
        report
    }

    pub fn remove_sections(
        &mut self,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_section_keys: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        self.apply_build_report(
            &BTreeSet::new(),
            TexturedRenderSectionBuildReport::default(),
            removal_chunks,
            removal_section_keys,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use mclone_core::{
        AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision,
        ChunkStatus, HitResultType, chunk_section_index,
    };
    use mclone_mesh::{
        TexturedChunkVertex, TexturedRenderSectionBuildReport, TexturedVisibleChunkMesh,
        VisibilityGraphBuildStats, VisibilitySet,
    };
    use mclone_protocol::{SectionBlockUpdate, ServerUpdate};

    use super::*;

    fn test_build_report(
        keys: impl IntoIterator<Item = RenderSectionKey>,
    ) -> TexturedRenderSectionBuildReport {
        TexturedRenderSectionBuildReport {
            sections: keys
                .into_iter()
                .map(|key| TexturedRenderSectionMesh {
                    key,
                    mesh: TexturedVisibleChunkMesh::default(),
                    visibility: VisibilitySet::all_visible(),
                })
                .collect(),
            visibility_graph: VisibilityGraphBuildStats::default(),
        }
    }

    fn empty_test_snapshot(pos: ChunkPos, min_y: i32, height: i32) -> ChunkSnapshot {
        let section_count =
            usize::try_from(height / SECTION_HEIGHT).expect("test height must fit usize");
        let block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * section_count];
        ChunkSnapshot::from_block_state_ids(
            pos,
            ChunkStatus::Surface,
            ChunkRevision(1),
            min_y,
            height,
            &block_state_ids,
        )
    }

    fn test_snapshot_with_block(
        pos: ChunkPos,
        block: BlockPos,
        state: BlockStateId,
    ) -> ChunkSnapshot {
        assert_eq!(block.chunk_pos(), pos);
        assert!((0..SECTION_HEIGHT).contains(&block.y));
        let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        block_state_ids[chunk_section_index(block.x, block.y, block.z)] = state;
        ChunkSnapshot::from_block_state_ids(
            pos,
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            SECTION_HEIGHT,
            &block_state_ids,
        )
    }

    #[test]
    fn snapshot_mesh_block_state_ids_rehydrates_omitted_air_sections() {
        let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2];
        block_state_ids[CHUNK_SECTION_VOLUME] = BlockStateId(1);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            32,
            &block_state_ids,
        );

        let mesh_blocks = snapshot_mesh_block_state_ids(&snapshot).unwrap();

        assert_eq!(mesh_blocks.blocks.len(), CHUNK_SECTION_VOLUME * 2);
        assert_eq!(mesh_blocks.blocks[0], AIR_BLOCK_STATE_ID);
        assert_eq!(mesh_blocks.blocks[CHUNK_SECTION_VOLUME], BlockStateId(1));
    }

    #[test]
    fn render_section_view_sync_marks_added_chunks_and_removed_neighbors_dirty() {
        let previous_chunks = BTreeSet::from([
            ChunkPos::new(0, 0),
            ChunkPos::new(1, 0),
            ChunkPos::new(2, 0),
        ]);
        let current_chunks = BTreeSet::from([
            ChunkPos::new(1, 0),
            ChunkPos::new(2, 0),
            ChunkPos::new(3, 0),
        ]);

        let sync = RenderSectionViewSync::from_loaded_chunks(&previous_chunks, current_chunks);

        assert_eq!(
            sync.dirty_chunks,
            BTreeSet::from([ChunkPos::new(1, 0), ChunkPos::new(3, 0)])
        );
        assert_eq!(sync.removal_chunks, BTreeSet::from([ChunkPos::new(0, 0)]));
    }

    #[test]
    fn render_section_session_marks_chunk_neighborhood_dirty_with_loaded_sections() {
        let center = ChunkPos::new(5, -3);
        let east = ChunkPos::new(6, -3);
        let center_key = RenderSectionKey::new(5, 4, -3);
        let east_key = RenderSectionKey::new(6, 4, -3);
        let mut session = RenderSectionSession::default();

        let marked = session.mark_chunk_neighborhood_dirty_with_loaded_sections(center, |pos| {
            if pos == center {
                vec![center_key]
            } else if pos == east {
                vec![east_key]
            } else {
                Vec::new()
            }
        });

        assert_eq!(marked, 5);
        assert_eq!(
            session.dirty().dirty_chunks,
            BTreeSet::from(render_dirty_chunk_neighborhood(center))
        );
        assert_eq!(session.dirty().section_revision(center_key), 1);
        assert_eq!(session.dirty().section_revision(east_key), 1);
    }

    #[test]
    fn block_delta_dirty_sections_cross_chunk_and_section_boundaries() {
        let keys = render_dirty_section_keys_for_block_update(
            ChunkPos::new(0, 0),
            0,
            &SectionBlockUpdate {
                local_x: 0,
                local_y: 0,
                local_z: 15,
                block_state: BlockStateId(42),
            },
        );

        assert_eq!(
            keys,
            BTreeSet::from([
                RenderSectionKey::new(-1, -1, 0),
                RenderSectionKey::new(-1, -1, 1),
                RenderSectionKey::new(-1, 0, 0),
                RenderSectionKey::new(-1, 0, 1),
                RenderSectionKey::new(0, -1, 0),
                RenderSectionKey::new(0, -1, 1),
                RenderSectionKey::new(0, 0, 0),
                RenderSectionKey::new(0, 0, 1),
            ])
        );
    }

    #[test]
    fn engine_camera_controller_moves_no_clip_and_crosses_chunk_boundary() {
        let mut camera = EngineCameraController::from_eye_pose(
            Vec3d::new(15.5, 96.0, 8.0),
            std::f64::consts::FRAC_PI_2,
            0.0,
            32.0,
        );

        let snapshot = camera.apply_input(EngineCameraInput {
            dt_seconds: 0.1,
            forward: true,
            ..EngineCameraInput::default()
        });

        assert!(snapshot.eye.x > 16.0);
        assert_eq!(snapshot.chunk_pos, ChunkPos::new(1, 0));
    }

    #[test]
    fn engine_camera_controller_accepts_analog_movement_impulse() {
        let mut camera =
            EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
        let before = camera.snapshot();

        let after = camera.apply_input(EngineCameraInput {
            dt_seconds: 0.1,
            movement_impulse: Some(EngineCameraMovementImpulse::new(0.0, 0.5)),
            ..EngineCameraInput::default()
        });

        assert!(after.eye.z > before.eye.z);
        assert!((after.eye.z - before.eye.z) < ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND * 0.1);
    }

    #[test]
    fn engine_camera_controller_applies_mouse_look_and_sprint_boost() {
        let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));
        let before = camera.snapshot();

        let after = camera.apply_input(EngineCameraInput {
            dt_seconds: 0.1,
            mouse_delta_x: 20.0,
            mouse_delta_y: -10.0,
            forward: true,
            sprint: true,
            ..EngineCameraInput::default()
        });

        assert!((after.yaw_radians - before.yaw_radians).abs() > 1.0e-6);
        assert!((after.pitch_radians - before.pitch_radians).abs() > 1.0e-6);
        assert!(
            after.eye.subtract(before.eye).length_sqr()
                > (ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND * 0.1).powi(2)
        );
    }

    #[test]
    fn engine_camera_controller_toggles_movement_mode() {
        let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));

        assert_eq!(camera.movement_mode(), EngineCameraMovementMode::Walking);
        assert_eq!(
            camera.toggle_movement_mode(),
            EngineCameraMovementMode::NoClip
        );
        assert_eq!(
            camera.toggle_movement_mode(),
            EngineCameraMovementMode::HandPush
        );
        assert_eq!(
            camera.toggle_movement_mode(),
            EngineCameraMovementMode::Walking
        );
        assert_eq!(EngineCameraMovementMode::Walking.label(), "WALK");
        assert_eq!(EngineCameraMovementMode::NoClip.label(), "NOCLIP");
        assert_eq!(EngineCameraMovementMode::HandPush.label(), "HAND");
    }

    #[test]
    fn hand_push_emulation_direction_uses_camera_yaw() {
        let forward = hand_push_emulation_direction(
            EngineCameraInput {
                forward: true,
                hand_push_emulation: true,
                ..EngineCameraInput::default()
            },
            0.0,
        );
        assert!(forward.z > 0.99);
        assert!(forward.x.abs() < 1.0e-6);

        let right = hand_push_emulation_direction(
            EngineCameraInput {
                right: true,
                hand_push_emulation: true,
                ..EngineCameraInput::default()
            },
            0.0,
        );
        assert!(right.x > 0.99);
        assert!(right.z.abs() < 1.0e-6);
    }

    #[test]
    fn engine_camera_snapshot_from_eye_pose_floors_negative_chunks() {
        let snapshot =
            EngineCameraSnapshot::from_eye_pose(Vec3d::new(-16.01, 91.0, -0.01), 0.2, -0.1, 24.0);

        assert_eq!(snapshot.eye, Vec3d::new(-16.01, 91.0, -0.01));
        assert_eq!(snapshot.yaw_radians, 0.2);
        assert_eq!(snapshot.pitch_radians, -0.1);
        assert_eq!(snapshot.speed_blocks_per_second, 24.0);
        assert_eq!(snapshot.chunk_pos, ChunkPos::new(-2, -1));
    }

    #[test]
    fn engine_camera_frame_state_reports_controller_and_interaction_state() {
        let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));
        let mut interaction = ClientInteractionController::new();
        camera.toggle_movement_mode();
        assert!(interaction.select_hotbar_slot(4));

        let state = camera.frame_state(&interaction);

        assert_eq!(state.camera, camera.snapshot());
        assert_eq!(state.movement_mode, EngineCameraMovementMode::NoClip);
        assert_eq!(state.view_mode, EngineCameraViewMode::FirstPerson);
        assert_eq!(state.movement_mode_label(), "NOCLIP");
        assert_eq!(state.view_mode_label(), "FIRST_PERSON");
        assert_eq!(state.on_ground, camera.on_ground());
        assert_eq!(state.horizontal_collision, camera.horizontal_collision());
        assert_eq!(state.vertical_collision, camera.vertical_collision());
        assert_eq!(state.selected_hotbar_slot, 4);
    }

    #[test]
    fn engine_camera_controller_sets_explicit_eye_pose() {
        let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));
        let eye = Vec3d::new(16.25, 72.0, -0.5);

        camera.set_eye_pose(eye, 0.25, -0.125);
        let snapshot = camera.snapshot();

        assert_eq!(snapshot.eye, eye);
        assert_eq!(snapshot.chunk_pos, ChunkPos::new(1, -1));
        assert!((snapshot.yaw_radians - 0.25).abs() < 1.0e-12);
        assert!((snapshot.pitch_radians + 0.125).abs() < 1.0e-12);
    }

    #[test]
    fn engine_camera_controller_ticks_current_no_clip_keys() {
        let mut camera =
            EngineCameraController::from_eye_pose(Vec3d::new(15.5, 96.0, 8.0), 0.0, 0.0, 32.0);
        let client = ClientRuntime::local_integrated();
        camera.set_movement_mode(EngineCameraMovementMode::NoClip);
        camera.set_key(PlayerInputKey::Forward, true);

        let moved = camera.tick_movement(&client, 0.1);

        assert!(moved);
        assert!(camera.snapshot().eye.z > 8.0);
    }

    #[test]
    fn engine_camera_controller_reports_pose_sync_command() {
        let mut camera =
            EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
        camera.turn_mouse_delta(5.0, 0.0);

        let report = camera
            .next_pose_sync_command()
            .expect("rotation should produce pose sync");

        assert_eq!(report.kind, EnginePoseSyncCommandKind::Movement);
        assert!(matches!(report.command, ClientCommand::MovePlayer(_)));
        assert_eq!(report.camera, camera.snapshot());
        assert!(camera.next_pose_sync_command().is_none());
    }

    #[test]
    fn engine_camera_controller_reports_correction_acceptance_and_resync() {
        let mut camera =
            EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
        let update = PlayerPositionUpdate {
            position: Vec3d::new(4.0, 70.0, -3.0),
            y_rot_degrees: 90.0,
            x_rot_degrees: 30.0,
            relative: mclone_protocol::PlayerPositionRelativeFlags::ABSOLUTE,
            teleport_id: 42,
            dismount_vehicle: false,
        };

        let accepted = camera.accept_position_update(update);

        assert_eq!(accepted.update, update);
        assert_eq!(accepted.feet_position, update.position);
        assert_eq!(accepted.camera, camera.snapshot());
        assert!(matches!(
            accepted.accept_command,
            ClientCommand::AcceptTeleport(mclone_protocol::AcceptTeleportCommand { id: 42 })
        ));

        let resync = camera.corrected_pose_sync_command();

        assert_eq!(resync.kind, EnginePoseSyncCommandKind::CorrectionResync);
        assert!(matches!(resync.command, ClientCommand::MovePlayer(_)));
        assert_eq!(resync.camera, camera.snapshot());
        assert!(camera.next_pose_sync_command().is_none());
    }

    #[test]
    fn engine_camera_controller_can_tick_walking_path() {
        let mut camera =
            EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
        let client = ClientRuntime::local_integrated();
        let before = camera.snapshot();

        let after = camera.apply_movement_input(
            &client,
            EngineCameraInput {
                dt_seconds: 0.05,
                forward: true,
                ..EngineCameraInput::default()
            },
        );

        assert_eq!(camera.movement_mode(), EngineCameraMovementMode::Walking);
        assert!(after.eye.z > before.eye.z);
        assert!(camera.player().delta_movement().y < 0.0);
    }

    #[test]
    fn engine_camera_controller_applies_walking_speed_multiplier() {
        let client = ClientRuntime::local_integrated();
        let input = EngineCameraInput {
            dt_seconds: 0.05,
            forward: true,
            ..EngineCameraInput::default()
        };
        let mut normal =
            EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
        let mut faster =
            EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
        faster.set_movement_speed_multiplier(2.0);

        let normal_before = normal.snapshot();
        let fast_before = faster.snapshot();
        let normal_after = normal.apply_movement_input(&client, input);
        let fast_after = faster.apply_movement_input(&client, input);

        assert_eq!(normal.movement_speed_multiplier(), 1.0);
        assert_eq!(faster.movement_speed_multiplier(), 2.0);
        assert!(fast_after.eye.z - fast_before.eye.z > normal_after.eye.z - normal_before.eye.z);

        faster.set_movement_speed_multiplier(20.0);
        assert_eq!(
            faster.movement_speed_multiplier(),
            ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER
        );
        faster.set_movement_speed_multiplier(f64::NAN);
        assert_eq!(
            faster.movement_speed_multiplier(),
            ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER
        );
    }

    #[test]
    fn engine_camera_movement_yaw_override_walks_without_turning_view() {
        let mut camera =
            EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), 0.0, 0.0, 32.0);
        let client = ClientRuntime::local_integrated();
        let before = camera.snapshot();

        let after = camera.apply_movement_input(
            &client,
            EngineCameraInput {
                dt_seconds: 0.05,
                movement_impulse: Some(EngineCameraMovementImpulse::new(0.0, 1.0)),
                movement_yaw_radians: Some(std::f64::consts::FRAC_PI_2),
                ..EngineCameraInput::default()
            },
        );

        assert!(after.eye.x > before.eye.x);
        assert!((after.eye.z - before.eye.z).abs() < 1.0e-6);
        assert!((after.yaw_radians - before.yaw_radians).abs() < 1.0e-12);
        assert!((after.pitch_radians - before.pitch_radians).abs() < 1.0e-12);
    }

    #[test]
    fn engine_camera_controller_queues_landing_event_once_on_airborne_to_ground() {
        let mut client = ClientRuntime::local_integrated();
        client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
            ChunkPos::new(0, 0),
            BlockPos::new(0, 0, 0),
            BlockStateId(7),
        )));
        let mut camera =
            EngineCameraController::from_eye_pose(Vec3d::new(0.5, 2.63, 0.5), 0.0, 0.0, 32.0);
        let tick_dt = 1.0 / LOCAL_PLAYER_TICKS_PER_SECOND;

        assert!(camera.tick_movement(&client, tick_dt));
        assert!(camera.take_landing_events().is_empty());

        assert!(camera.tick_movement(&client, tick_dt));
        let events = camera.take_landing_events();

        assert_eq!(events.len(), 1);
        assert!(events[0].impact_speed >= LANDING_MIN_IMPACT_SPEED);
        assert!((events[0].position.y - 1.0).abs() < 1.0e-9);
        assert!(camera.take_landing_events().is_empty());

        assert!(camera.tick_movement(&client, tick_dt));
        assert!(camera.take_landing_events().is_empty());
    }

    #[test]
    fn engine_camera_controller_probe_ground_does_not_queue_landing_event() {
        let mut client = ClientRuntime::local_integrated();
        client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
            ChunkPos::new(0, 0),
            BlockPos::new(0, 0, 0),
            BlockStateId(7),
        )));
        let mut camera =
            EngineCameraController::from_eye_pose(Vec3d::new(0.5, 2.62, 0.5), 0.0, 0.0, 32.0);

        camera.probe_ground(&client, 0.02);

        assert!(camera.on_ground());
        assert!(camera.take_landing_events().is_empty());
    }

    #[test]
    fn engine_camera_walking_forward_tracks_crosshair_view_direction() {
        // Guards the reported "in portrait I don't walk toward the crosshair":
        // walking forward must move along the same horizontal direction the
        // render camera / crosshair points, for every yaw. The math is
        // orientation-independent, so a regression here would be a real vector
        // bug rather than a touch-feel issue.
        let client = ClientRuntime::local_integrated();
        for yaw in [0.0_f64, 0.6, 1.5, 2.4, 3.1, -0.9, -2.2] {
            let mut camera =
                EngineCameraController::from_eye_pose(Vec3d::new(8.0, 96.0, 8.0), yaw, -0.35, 32.0);
            let before = camera.snapshot();

            let render = render_camera_from_snapshot(before, 1);
            let view_dx = f64::from(render.target[0]) - f64::from(render.eye[0]);
            let view_dz = f64::from(render.target[2]) - f64::from(render.eye[2]);
            let view_len = view_dx.hypot(view_dz);
            assert!(view_len > 1.0e-9, "degenerate view direction at yaw {yaw}");

            let after = camera.apply_movement_input(
                &client,
                EngineCameraInput {
                    dt_seconds: 0.05,
                    movement_impulse: Some(EngineCameraMovementImpulse::new(0.0, 1.0)),
                    ..EngineCameraInput::default()
                },
            );
            let move_dx = after.eye.x - before.eye.x;
            let move_dz = after.eye.z - before.eye.z;
            let move_len = move_dx.hypot(move_dz);
            assert!(move_len > 1.0e-6, "no horizontal movement at yaw {yaw}");

            let dot = (move_dx * view_dx + move_dz * view_dz) / (move_len * view_len);
            assert!(
                dot > 0.9999,
                "forward walk diverged from crosshair at yaw {yaw}: dot={dot}",
            );
        }
    }

    #[test]
    fn engine_camera_view_mode_toggles_between_first_and_third_person() {
        let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(0, 0));

        assert_eq!(camera.view_mode(), EngineCameraViewMode::FirstPerson);
        assert_eq!(
            camera.toggle_view_mode(),
            EngineCameraViewMode::ThirdPersonBack
        );
        assert_eq!(camera.toggle_view_mode(), EngineCameraViewMode::FirstPerson);
    }

    #[test]
    fn third_person_render_camera_tracks_player_view_from_behind() {
        let snapshot =
            EngineCameraSnapshot::from_eye_pose(Vec3d::new(8.0, 70.0, 8.0), 0.0, 0.0, 24.0);

        let first = render_camera_from_snapshot(snapshot, 2);
        let third = render_camera_from_snapshot_with_view_mode(
            snapshot,
            EngineCameraViewMode::ThirdPersonBack,
            2,
        );

        assert_eq!(first.eye, [8.0, 70.0, 8.0]);
        assert_eq!(first.target, [8.0, 70.0, 9.0]);
        assert_eq!(third.eye, [8.0, 70.0, 4.0]);
        assert_eq!(third.target, [8.0, 70.0, 5.0]);
        assert_eq!(third.z_far, first.z_far);
    }

    #[test]
    fn local_player_actor_instance_uses_controller_feet_pose() {
        let client = ClientRuntime::local_integrated();
        let camera = EngineCameraController::from_eye_pose(
            Vec3d::new(1.25, 70.62, -3.5),
            std::f64::consts::FRAC_PI_2,
            0.0,
            24.0,
        );

        let actor = local_player_actor_instance(&camera, &client);

        assert_eq!(actor.feet_position, Vec3::new(1.25, 69.0, -3.5));
        assert!((actor.yaw_radians - std::f32::consts::FRAC_PI_2).abs() < 1.0e-6);
        assert_eq!(
            actor.shape,
            mclone_render::entity::ActorInstanceShape::AssetLabPlayer
        );
    }

    #[test]
    fn engine_camera_controller_picks_block_from_player_view() {
        let target = BlockPos::new(1, 2, 4);
        let mut client = ClientRuntime::local_integrated();
        client.apply_update(ServerUpdate::ChunkSnapshot(test_snapshot_with_block(
            ChunkPos::new(0, 0),
            target,
            BlockStateId(1),
        )));
        let interaction = ClientInteractionController::new();
        let camera =
            EngineCameraController::from_eye_pose(Vec3d::new(1.5, 2.5, 1.5), 0.0, 0.0, 32.0);

        let hit = camera.pick_block(&client, &interaction);

        assert_eq!(hit.hit_type(), HitResultType::Block);
        assert_eq!(hit.block_pos, target);
    }

    #[test]
    fn engine_camera_render_camera_targets_forward_direction() {
        let camera =
            EngineCameraController::from_eye_pose(Vec3d::new(1.0, 2.0, 3.0), 0.0, 0.0, 32.0);

        let render_camera = camera.render_camera(2);

        assert_eq!(render_camera.eye, [1.0, 2.0, 3.0]);
        assert_eq!(render_camera.target, [1.0, 2.0, 4.0]);
        assert_eq!(render_camera.up, [0.0, 1.0, 0.0]);
        assert!(render_camera.z_far > 700.0);
    }

    #[test]
    fn engine_render_session_prepares_ready_work_from_client_snapshots() {
        let chunk = ChunkPos::new(2, -1);
        let key = RenderSectionKey::new(2, 0, -1);
        let mut client = ClientRuntime::local_integrated();
        client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(
            chunk,
            0,
            SECTION_HEIGHT,
        )));
        let mut engine = EngineRenderSession::new(client);

        assert!(engine.mark_loaded_chunks_dirty_when_cache_empty());
        assert_eq!(
            engine.render_session().dirty().dirty_chunks,
            BTreeSet::from(render_dirty_chunk_neighborhood(chunk))
        );
        assert_eq!(engine.render_session().dirty().section_revision(key), 1);

        let sync_update = engine.prepare_sync_update(
            |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
            |dirty_work| {
                dirty_work
                    .loaded_dirty_sections_by_chunk
                    .keys()
                    .copied()
                    .collect()
            },
            usize::MAX,
            |_client, _key| RenderSectionNeighborReadiness::ReadyWithNeighbors,
            RenderSectionRemovalMode::Defer,
        );
        assert_eq!(
            sync_update.sync_plan.ready_plan.ready_section_keys,
            BTreeSet::from([key])
        );

        let snapshots = engine.client().chunk_snapshots().cloned().collect();
        let submission = engine
            .submit_prepared_sync_plan(&sync_update.sync_plan, snapshots, |_sync_plan, request| {
                Ok::<_, std::convert::Infallible>(request)
            })
            .expect("ready work should submit")
            .submission
            .expect("loaded dirty section should produce a request");

        assert_eq!(submission.output.target_sections, BTreeSet::from([key]));
        assert_eq!(
            engine.render_session().dirty().inflight_sections,
            BTreeSet::from([key])
        );
        assert!(engine.render_session().dirty().dirty_chunks.is_empty());
    }

    #[test]
    fn engine_render_session_applies_server_updates_and_marks_render_dirty() {
        let chunk = ChunkPos::new(0, 0);
        let key = RenderSectionKey::new(0, 7, 0);
        let mut client = ClientRuntime::local_integrated();
        client.apply_update(ServerUpdate::ChunkSnapshot(empty_test_snapshot(
            chunk,
            0,
            SECTION_HEIGHT * 16,
        )));
        let mut engine = EngineRenderSession::new(client);

        let report = engine.apply_server_updates(vec![
            ServerUpdate::TimeUpdate { day_time: 1 },
            ServerUpdate::SectionBlockUpdates {
                pos: chunk,
                section_y: 7,
                updates: vec![SectionBlockUpdate {
                    local_x: 8,
                    local_y: 8,
                    local_z: 8,
                    block_state: AIR_BLOCK_STATE_ID,
                }],
            },
        ]);

        assert_eq!(
            report,
            EngineServerUpdateReport {
                changed: true,
                updates: 2,
                snapshot_updates: 0,
                section_block_updates: 1,
                unload_updates: 0,
            }
        );
        assert_eq!(engine.client().day_time(), 1);
        assert!(engine.render_session().dirty().dirty_chunks.is_empty());
        assert_eq!(
            engine.render_session().dirty().dirty_sections,
            BTreeSet::from([key])
        );
    }

    #[test]
    fn engine_render_session_can_leave_snapshot_dirtying_to_view_sync_policy() {
        let chunk = ChunkPos::new(3, 4);
        let mut engine = EngineRenderSession::new(ClientRuntime::local_integrated());

        let report = engine.apply_server_updates_with_dirty_policy(
            vec![ServerUpdate::ChunkSnapshot(empty_test_snapshot(
                chunk,
                0,
                SECTION_HEIGHT,
            ))],
            EngineServerUpdateDirtyPolicy::SECTION_BLOCK_UPDATES_ONLY,
        );

        assert_eq!(
            report,
            EngineServerUpdateReport {
                changed: true,
                updates: 1,
                snapshot_updates: 1,
                section_block_updates: 0,
                unload_updates: 0,
            }
        );
        assert!(engine.client().chunk_snapshot(chunk).is_some());
        assert!(engine.render_session().dirty().dirty_chunks.is_empty());
        assert!(engine.render_session().dirty().dirty_sections.is_empty());
    }

    #[test]
    fn render_section_session_applies_loaded_view_sync_dirty_marks() {
        let removed_chunk = ChunkPos::new(0, 0);
        let edge_chunk = ChunkPos::new(1, 0);
        let retained_chunk = ChunkPos::new(2, 0);
        let added_chunk = ChunkPos::new(3, 0);
        let previous_chunks = BTreeSet::from([removed_chunk, edge_chunk, retained_chunk]);
        let current_chunks = BTreeSet::from([edge_chunk, retained_chunk, added_chunk]);
        let cached_removed_key = RenderSectionKey::new(0, 4, 0);
        let edge_key = RenderSectionKey::new(1, 4, 0);
        let added_key = RenderSectionKey::new(3, 4, 0);
        let mut session = RenderSectionSession::default();
        session.apply_finished_compile_report(
            &BTreeSet::from([cached_removed_key]),
            test_build_report([cached_removed_key]),
            &BTreeSet::new(),
            &BTreeSet::new(),
        );

        let sync = session.apply_loaded_view_sync(&previous_chunks, current_chunks, |pos| {
            if pos == edge_chunk {
                vec![edge_key]
            } else if pos == added_chunk {
                vec![added_key]
            } else {
                Vec::new()
            }
        });

        assert_eq!(sync.dirty_chunks, BTreeSet::from([edge_chunk, added_chunk]));
        assert_eq!(sync.removal_chunks, BTreeSet::from([removed_chunk]));
        assert_eq!(
            session.dirty().dirty_chunks,
            BTreeSet::from([removed_chunk, edge_chunk, added_chunk])
        );
        assert_eq!(session.dirty().section_revision(cached_removed_key), 1);
        assert_eq!(session.dirty().section_revision(edge_key), 1);
        assert_eq!(session.dirty().section_revision(added_key), 1);
    }

    #[test]
    fn render_section_dirty_state_tracks_dirty_inflight_and_stale_revisions() {
        let first = RenderSectionKey::new(0, 4, 0);
        let second = RenderSectionKey::new(0, 5, 0);
        let mut dirty = RenderSectionDirtyState::default();

        dirty.mark_chunk_dirty(ChunkPos::new(0, 0), [first, second]);
        assert_eq!(dirty.dirty_chunks, BTreeSet::from([ChunkPos::new(0, 0)]));
        assert_eq!(dirty.section_revision(first), 1);
        assert_eq!(dirty.section_revision(second), 1);

        let request = dirty.build_compile_request(BTreeSet::from([first, second]), Vec::new());
        dirty.mark_compile_submitted(&request.target_sections);
        assert_eq!(dirty.inflight_sections, BTreeSet::from([first, second]));

        dirty.mark_section_dirty(second);
        let acceptance = dirty.accept_completed_compile_result(&RenderSectionCompileResult {
            target_sections: request.target_sections,
            section_revisions: request.section_revisions,
            result: Ok(TexturedRenderSectionBuildReport::default()),
        });

        assert!(dirty.inflight_sections.is_empty());
        assert_eq!(acceptance.accepted_sections, BTreeSet::from([first]));
        assert_eq!(acceptance.stale_sections, BTreeSet::from([second]));
        assert_eq!(dirty.dirty_sections, BTreeSet::from([second]));
    }

    #[test]
    fn render_section_dirty_work_classifies_loaded_removal_and_stale_items() {
        let loaded_chunk = ChunkPos::new(0, 0);
        let removal_chunk = ChunkPos::new(1, 0);
        let stale_chunk = ChunkPos::new(2, 0);
        let loaded_section = RenderSectionKey::new(0, 4, 0);
        let chunk_removal_section = RenderSectionKey::new(1, 4, 0);
        let removal_section = RenderSectionKey::new(3, 4, 0);
        let stale_section = RenderSectionKey::new(4, 4, 0);
        let loaded_chunks = BTreeSet::from([loaded_chunk]);
        let cached_chunks = BTreeSet::from([removal_chunk]);
        let loaded_sections = BTreeSet::from([loaded_section]);
        let cached_sections = BTreeSet::from([chunk_removal_section, removal_section]);
        let mut dirty = RenderSectionDirtyState {
            dirty_chunks: BTreeSet::from([loaded_chunk, removal_chunk, stale_chunk]),
            dirty_sections: BTreeSet::from([
                loaded_section,
                chunk_removal_section,
                removal_section,
                stale_section,
            ]),
            inflight_sections: BTreeSet::from([chunk_removal_section, removal_section]),
            ..RenderSectionDirtyState::default()
        };

        let work = classify_render_section_dirty_work(
            &dirty,
            |pos| loaded_chunks.contains(&pos),
            |pos| cached_chunks.contains(&pos),
            |key| loaded_sections.contains(&key),
            |key| cached_sections.contains(&key),
        );

        assert_eq!(work.loaded_dirty_chunks, BTreeSet::from([loaded_chunk]));
        assert_eq!(work.removal_dirty_chunks, BTreeSet::from([removal_chunk]));
        assert_eq!(work.stale_dirty_chunks, BTreeSet::from([stale_chunk]));
        assert_eq!(
            work.loaded_dirty_sections_by_chunk,
            BTreeMap::from([(loaded_chunk, BTreeSet::from([loaded_section]))])
        );
        assert_eq!(
            work.removal_dirty_sections,
            BTreeSet::from([chunk_removal_section, removal_section])
        );
        assert_eq!(work.stale_dirty_sections, BTreeSet::from([stale_section]));

        dirty.discard_stale_dirty_work(&work.stale_dirty_chunks, &work.stale_dirty_sections);
        dirty.discard_removed_dirty_work(&work.removal_dirty_chunks, &work.removal_dirty_sections);

        assert_eq!(dirty.dirty_chunks, BTreeSet::from([loaded_chunk]));
        assert_eq!(dirty.dirty_sections, BTreeSet::from([loaded_section]));
        assert!(dirty.inflight_sections.is_empty());
    }

    #[test]
    fn render_section_ready_plan_selects_budgeted_ready_and_deferred_sections() {
        let loaded_chunk = ChunkPos::new(0, 0);
        let dirty_section_chunk = ChunkPos::new(2, 0);
        let skipped_section_chunk = ChunkPos::new(3, 0);
        let ready_from_chunk = RenderSectionKey::new(0, 4, 0);
        let deferred_from_chunk = RenderSectionKey::new(0, 5, 0);
        let inflight_from_chunk = RenderSectionKey::new(0, 6, 0);
        let near_dirty_section = RenderSectionKey::new(2, 4, 0);
        let skipped_dirty_section = RenderSectionKey::new(3, 4, 0);
        let sections_by_chunk = BTreeMap::from([
            (dirty_section_chunk, BTreeSet::from([near_dirty_section])),
            (
                skipped_section_chunk,
                BTreeSet::from([skipped_dirty_section]),
            ),
        ]);

        let plan = plan_ready_render_sections(
            [loaded_chunk],
            [dirty_section_chunk, skipped_section_chunk],
            &sections_by_chunk,
            2,
            |_| vec![ready_from_chunk, deferred_from_chunk, inflight_from_chunk],
            &BTreeSet::from([inflight_from_chunk]),
            |key| match key {
                key if key == ready_from_chunk => {
                    RenderSectionNeighborReadiness::ReadyWithNeighbors
                }
                key if key == deferred_from_chunk => {
                    RenderSectionNeighborReadiness::DeferredMissingNeighbors
                }
                key if key == near_dirty_section => RenderSectionNeighborReadiness::ReadyNearCamera,
                unexpected => panic!("unexpected readiness probe for {unexpected:?}"),
            },
        );

        assert_eq!(plan.budgeted_loaded_chunks, BTreeSet::from([loaded_chunk]));
        assert_eq!(
            plan.budgeted_dirty_section_chunks,
            BTreeSet::from([dirty_section_chunk])
        );
        assert_eq!(
            plan.ready_section_keys,
            BTreeSet::from([ready_from_chunk, near_dirty_section])
        );
        assert_eq!(
            plan.deferred_section_keys,
            BTreeSet::from([deferred_from_chunk, inflight_from_chunk])
        );
        assert_eq!(plan.near_exception_section_count, 1);
        assert_eq!(plan.deferred_section_count, 1);

        let mut dirty = RenderSectionDirtyState {
            dirty_chunks: BTreeSet::from([loaded_chunk]),
            dirty_sections: BTreeSet::from([
                ready_from_chunk,
                deferred_from_chunk,
                inflight_from_chunk,
                near_dirty_section,
            ]),
            ..RenderSectionDirtyState::default()
        };
        dirty.apply_ready_plan(&plan);

        assert!(dirty.dirty_chunks.is_empty());
        assert_eq!(
            dirty.dirty_sections,
            BTreeSet::from([deferred_from_chunk, inflight_from_chunk])
        );
    }

    #[test]
    fn render_section_ready_plan_skips_deferred_only_chunks_for_budget() {
        let deferred_chunk = ChunkPos::new(1, 0);
        let ready_chunk = ChunkPos::new(2, 0);
        let deferred = RenderSectionKey::new(1, 4, 0);
        let ready = RenderSectionKey::new(2, 4, 0);
        let sections_by_chunk = BTreeMap::from([
            (deferred_chunk, BTreeSet::from([deferred])),
            (ready_chunk, BTreeSet::from([ready])),
        ]);

        let plan = plan_ready_render_sections(
            [],
            [deferred_chunk, ready_chunk],
            &sections_by_chunk,
            1,
            |_| Vec::new(),
            &BTreeSet::new(),
            |key| {
                if key == ready {
                    RenderSectionNeighborReadiness::ReadyWithNeighbors
                } else {
                    RenderSectionNeighborReadiness::DeferredMissingNeighbors
                }
            },
        );

        assert!(plan.budgeted_loaded_chunks.is_empty());
        assert_eq!(
            plan.budgeted_dirty_section_chunks,
            BTreeSet::from([ready_chunk])
        );
        assert_eq!(plan.ready_section_keys, BTreeSet::from([ready]));
        assert_eq!(plan.deferred_section_keys, BTreeSet::from([deferred]));
        assert_eq!(plan.deferred_section_count, 1);
    }

    #[test]
    fn render_section_sync_plan_discards_stale_and_plans_ready_work() {
        let loaded_chunk = ChunkPos::new(0, 0);
        let stale_chunk = ChunkPos::new(1, 0);
        let ready = RenderSectionKey::new(0, 4, 0);
        let stale = RenderSectionKey::new(1, 4, 0);
        let mut dirty = RenderSectionDirtyState {
            dirty_chunks: BTreeSet::from([loaded_chunk, stale_chunk]),
            dirty_sections: BTreeSet::from([stale]),
            ..RenderSectionDirtyState::default()
        };
        let dirty_work = RenderSectionDirtyWork {
            loaded_dirty_chunks: BTreeSet::from([loaded_chunk]),
            stale_dirty_chunks: BTreeSet::from([stale_chunk]),
            stale_dirty_sections: BTreeSet::from([stale]),
            ..RenderSectionDirtyWork::default()
        };

        let plan = prepare_render_section_sync_plan(
            &mut dirty,
            dirty_work,
            [loaded_chunk],
            [],
            1,
            |_| vec![ready],
            |_| RenderSectionNeighborReadiness::ReadyWithNeighbors,
        );

        assert_eq!(dirty.dirty_chunks, BTreeSet::from([loaded_chunk]));
        assert!(dirty.dirty_sections.is_empty());
        assert_eq!(plan.ready_plan.ready_section_keys, BTreeSet::from([ready]));
        assert!(!plan.has_removals());
        assert_eq!(plan.ready_update(1).submitted_compile_section_count, 1);
    }

    #[test]
    fn render_view_compile_queue_starts_when_idle_and_skips_loaded_center() {
        let mut queue = RenderViewCompileQueue::default();
        let center = ChunkPos::new(2, -3);

        assert_eq!(
            queue.request(
                QueuedRenderViewCompile::new(center, false, "movement"),
                None,
                false
            ),
            RenderViewCompileQueueDecision::Start(QueuedRenderViewCompile::new(
                center, false, "movement"
            ))
        );
        assert_eq!(
            queue.request(
                QueuedRenderViewCompile::new(center, false, "movement"),
                Some(center),
                false
            ),
            RenderViewCompileQueueDecision::Skipped
        );
    }

    #[test]
    fn render_view_compile_queue_coalesces_while_busy_and_preserves_force() {
        let mut queue = RenderViewCompileQueue::default();
        let first = ChunkPos::new(0, 0);
        let second = ChunkPos::new(1, 0);

        assert_eq!(
            queue.request(
                QueuedRenderViewCompile::new(first, true, "interaction"),
                None,
                true
            ),
            RenderViewCompileQueueDecision::Queued(QueuedRenderViewCompile::new(
                first,
                true,
                "interaction"
            ))
        );
        assert_eq!(
            queue.request(
                QueuedRenderViewCompile::new(second, false, "movement"),
                None,
                true
            ),
            RenderViewCompileQueueDecision::Queued(QueuedRenderViewCompile::new(
                second, true, "movement"
            ))
        );
        assert_eq!(
            queue.take_next(None, false),
            RenderViewCompileQueueDecision::Start(QueuedRenderViewCompile::new(
                second, true, "movement"
            ))
        );
        assert_eq!(
            queue.take_next(None, false),
            RenderViewCompileQueueDecision::Idle
        );
    }

    #[test]
    fn render_view_compile_queue_latest_idle_or_loaded_request_discards_stale_queue() {
        let mut queue = RenderViewCompileQueue::default();
        let loaded = ChunkPos::new(0, 0);
        let stale = ChunkPos::new(1, 0);
        let fresh = ChunkPos::new(2, 0);

        assert!(matches!(
            queue.request(
                QueuedRenderViewCompile::new(stale, false, "movement"),
                Some(loaded),
                true
            ),
            RenderViewCompileQueueDecision::Queued(_)
        ));
        assert!(queue.has_queued());
        assert_eq!(
            queue.request(
                QueuedRenderViewCompile::new(loaded, false, "movement"),
                Some(loaded),
                true
            ),
            RenderViewCompileQueueDecision::Skipped
        );
        assert!(!queue.has_queued());

        assert!(matches!(
            queue.request(
                QueuedRenderViewCompile::new(stale, false, "movement"),
                Some(loaded),
                true
            ),
            RenderViewCompileQueueDecision::Queued(_)
        ));
        assert_eq!(
            queue.request(
                QueuedRenderViewCompile::new(fresh, false, "movement"),
                Some(loaded),
                false
            ),
            RenderViewCompileQueueDecision::Start(QueuedRenderViewCompile::new(
                fresh, false, "movement"
            ))
        );
        assert!(!queue.has_queued());
    }

    #[test]
    fn render_section_sync_update_applies_removals_for_native_style_sync() {
        let loaded_chunk = ChunkPos::new(0, 0);
        let removal_chunk = ChunkPos::new(1, 0);
        let ready = RenderSectionKey::new(0, 4, 0);
        let cached = RenderSectionKey::new(1, 4, 0);
        let mut session = RenderSectionSession::default();
        session.apply_finished_compile_report(
            &BTreeSet::from([cached]),
            TexturedRenderSectionBuildReport {
                sections: vec![TexturedRenderSectionMesh {
                    key: cached,
                    mesh: TexturedVisibleChunkMesh::default(),
                    visibility: VisibilitySet::all_visible(),
                }],
                visibility_graph: VisibilityGraphBuildStats::default(),
            },
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        session.mark_chunk_dirty(loaded_chunk, [ready]);
        session.mark_chunk_dirty(removal_chunk, [cached]);

        let update = session.prepare_sync_update(
            |pos| pos == loaded_chunk,
            |key| key == ready,
            |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
            |dirty_work| {
                dirty_work
                    .loaded_dirty_sections_by_chunk
                    .keys()
                    .copied()
                    .collect()
            },
            1,
            |_| vec![ready],
            |_| RenderSectionNeighborReadiness::ReadyWithNeighbors,
            RenderSectionRemovalMode::ApplyImmediately,
        );

        assert_eq!(update.cache_update.removed_section_count(), 1);
        assert!(!session.contains_section(cached));
        assert_eq!(
            update.sync_plan.ready_plan.ready_section_keys,
            BTreeSet::from([ready])
        );
        assert!(!session.dirty().dirty_chunks.contains(&removal_chunk));
    }

    #[test]
    fn render_section_sync_update_can_defer_removals_for_combined_web_uploads() {
        let loaded_chunk = ChunkPos::new(0, 0);
        let removal_chunk = ChunkPos::new(1, 0);
        let ready = RenderSectionKey::new(0, 4, 0);
        let cached = RenderSectionKey::new(1, 4, 0);
        let mut session = RenderSectionSession::default();
        session.apply_finished_compile_report(
            &BTreeSet::from([cached]),
            TexturedRenderSectionBuildReport {
                sections: vec![TexturedRenderSectionMesh {
                    key: cached,
                    mesh: TexturedVisibleChunkMesh::default(),
                    visibility: VisibilitySet::all_visible(),
                }],
                visibility_graph: VisibilityGraphBuildStats::default(),
            },
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        session.mark_chunk_dirty(loaded_chunk, [ready]);
        session.mark_chunk_dirty(removal_chunk, [cached]);

        let update = session.prepare_sync_update(
            |pos| pos == loaded_chunk,
            |key| key == ready,
            |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
            |dirty_work| {
                dirty_work
                    .loaded_dirty_sections_by_chunk
                    .keys()
                    .copied()
                    .collect()
            },
            1,
            |_| vec![ready],
            |_| RenderSectionNeighborReadiness::ReadyWithNeighbors,
            RenderSectionRemovalMode::Defer,
        );

        assert_eq!(update.cache_update.removed_section_count(), 0);
        assert!(session.contains_section(cached));
        assert_eq!(
            update.sync_plan.dirty_work.removal_dirty_chunks,
            BTreeSet::from([removal_chunk])
        );
        assert!(session.dirty().dirty_chunks.contains(&removal_chunk));
    }

    #[test]
    fn render_section_compile_finish_reports_accepted_and_stale_sections() {
        let accepted = RenderSectionKey::new(0, 4, 0);
        let stale = RenderSectionKey::new(0, 5, 0);
        let mut dirty = RenderSectionDirtyState::default();
        dirty.mark_section_dirty(accepted);
        dirty.mark_section_dirty(stale);
        let request = dirty.build_compile_request(BTreeSet::from([accepted, stale]), Vec::new());
        dirty.mark_compile_submitted(&request.target_sections);
        dirty.mark_section_dirty(stale);

        let finish = finish_render_section_compile_result(
            &mut dirty,
            RenderSectionCompileResult {
                target_sections: request.target_sections,
                section_revisions: request.section_revisions,
                result: Ok(TexturedRenderSectionBuildReport::default()),
            },
            7,
        )
        .unwrap();

        assert_eq!(finish.accepted_sections, BTreeSet::from([accepted]));
        assert_eq!(finish.stale_sections, BTreeSet::from([stale]));
        assert!(finish.build_report.is_some());
        assert_eq!(
            finish.acceptance_report,
            RenderSectionCompileAcceptanceReport {
                request_id: 7,
                submitted_section_count: 2,
                accepted_section_count: 1,
                stale_section_count: 1,
            }
        );
        assert!(dirty.inflight_sections.is_empty());
    }

    #[test]
    fn render_dirty_chunk_neighborhood_includes_cardinal_neighbors() {
        assert_eq!(
            render_dirty_chunk_neighborhood(ChunkPos::new(4, -2)),
            [
                ChunkPos::new(4, -2),
                ChunkPos::new(3, -2),
                ChunkPos::new(5, -2),
                ChunkPos::new(4, -3),
                ChunkPos::new(4, -1),
            ]
        );
    }

    #[test]
    fn render_section_compile_request_state_tracks_pending_revisions_and_stale_results() {
        let key = RenderSectionKey::new(0, 4, 0);
        let mut state = RenderSectionCompileRequestState::default();

        let first = state.begin_request("first", BTreeSet::from([key]));
        assert_eq!(
            first,
            RenderSectionCompileRequestInfo {
                request_id: 1,
                submitted_section_count: 1,
                pending_compile_jobs: 1,
            }
        );
        assert_eq!(state.section_revision(key), 1);
        let pending = state.remove_pending_request(first.request_id).unwrap();
        assert_eq!(pending.context, "first");
        let (_, completed) = pending.into_compile_result(Ok(TexturedRenderSectionBuildReport {
            sections: Vec::new(),
            visibility_graph: VisibilityGraphBuildStats::default(),
        }));
        let accepted = completed.partition_by_revision(|key| state.section_revision(key));
        assert_eq!(accepted.accepted_sections, BTreeSet::from([key]));
        assert!(accepted.stale_sections.is_empty());

        let second = state.begin_request("second", BTreeSet::from([key]));
        let pending = state.remove_pending_request(second.request_id).unwrap();
        state.bump_section_revisions(&BTreeSet::from([key]));
        let (_, completed) =
            pending.into_compile_result(Ok(TexturedRenderSectionBuildReport::default()));
        let stale = completed.partition_by_revision(|key| state.section_revision(key));
        assert!(stale.accepted_sections.is_empty());
        assert_eq!(stale.stale_sections, BTreeSet::from([key]));
    }

    #[test]
    fn render_section_compile_request_state_accepts_prepared_dirty_request() {
        let key = RenderSectionKey::new(0, 4, 0);
        let mut dirty = RenderSectionDirtyState::default();
        dirty.mark_section_dirty(key);
        let request = dirty.build_compile_request(BTreeSet::from([key]), Vec::new());
        let mut state = RenderSectionCompileRequestState::default();

        let info = state.begin_compile_request("prepared", request);

        assert_eq!(
            info,
            RenderSectionCompileRequestInfo {
                request_id: 1,
                submitted_section_count: 1,
                pending_compile_jobs: 1,
            }
        );
        let pending = state.remove_pending_request(info.request_id).unwrap();
        assert_eq!(pending.context, "prepared");
        let (_, completed) =
            pending.into_compile_result(Ok(TexturedRenderSectionBuildReport::default()));
        let accepted = dirty.accept_completed_compile_result(&completed);

        assert_eq!(accepted.accepted_sections, BTreeSet::from([key]));
        assert!(accepted.stale_sections.is_empty());
    }

    #[test]
    fn render_section_key_helpers_select_dirty_snapshot_sections() {
        let blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME * 2];
        let clean = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            32,
            &blocks,
        );
        let dirty = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(1, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            -16,
            32,
            &blocks,
        );

        let keys =
            target_section_keys_for_dirty_chunks([&clean, &dirty], &BTreeSet::from([dirty.pos]));

        assert_eq!(
            keys,
            BTreeSet::from([
                RenderSectionKey::new(1, -1, 0),
                RenderSectionKey::new(1, 0, 0),
            ])
        );
        assert!(snapshot_contains_render_section(
            &dirty,
            RenderSectionKey::new(1, -1, 0)
        ));
        assert_eq!(
            render_section_chunk_pos(RenderSectionKey::new(1, 0, 0)),
            dirty.pos
        );
    }

    #[test]
    fn cached_sections_apply_build_report_and_remove_unloaded_chunks() {
        let mut cache = CachedTexturedRenderSections::default();
        let key = RenderSectionKey::new(2, 4, -1);
        let report = TexturedRenderSectionBuildReport {
            sections: vec![TexturedRenderSectionMesh {
                key,
                mesh: TexturedVisibleChunkMesh::default(),
                visibility: VisibilitySet::all_visible(),
            }],
            visibility_graph: VisibilityGraphBuildStats::default(),
        };

        let update = cache.apply_build_report(
            &BTreeSet::from([key]),
            report,
            &BTreeSet::new(),
            &BTreeSet::new(),
        );

        assert_eq!(update.rebuilt_section_count(), 1);
        assert!(cache.contains_section(key));
        assert!(cache.contains_chunk(ChunkPos::new(2, -1)));

        let removal =
            cache.remove_sections(&BTreeSet::from([ChunkPos::new(2, -1)]), &BTreeSet::new());

        assert_eq!(removal.removed_section_count(), 1);
        assert!(!cache.contains_section(key));
        assert!(!cache.contains_chunk(ChunkPos::new(2, -1)));
    }

    #[test]
    fn render_section_session_owns_dirty_compile_and_cache_updates() {
        let mut session = RenderSectionSession::default();
        let key = RenderSectionKey::new(2, 4, -1);
        let pos = ChunkPos::new(2, -1);
        let ready_plan = RenderSectionReadyPlan {
            ready_section_keys: BTreeSet::from([key]),
            ..RenderSectionReadyPlan::default()
        };

        session.mark_section_dirty(key);
        let submission = session
            .submit_ready_plan_compile_request(&ready_plan, Vec::new(), |request| {
                Ok::<_, std::convert::Infallible>(request)
            })
            .expect("compile submission should not fail")
            .expect("ready section should build a compile request");
        assert_eq!(submission.submitted_section_count, 1);
        let request = submission.output;
        assert!(session.dirty().inflight_sections.contains(&key));

        let finish = session
            .finish_compile_result(
                RenderSectionCompileResult {
                    target_sections: request.target_sections,
                    section_revisions: request.section_revisions,
                    result: Ok(TexturedRenderSectionBuildReport {
                        sections: vec![TexturedRenderSectionMesh {
                            key,
                            mesh: TexturedVisibleChunkMesh {
                                vertices: vec![TexturedChunkVertex {
                                    position: [0.0, 0.0, 0.0],
                                    uv: [0.0, 0.0],
                                    color: [1.0, 1.0, 1.0, 1.0],
                                    packed_light: 0,
                                }],
                                indices: vec![0],
                                opaque_index_count: 1,
                            },
                            visibility: VisibilitySet::all_visible(),
                        }],
                        visibility_graph: VisibilityGraphBuildStats::default(),
                    }),
                },
                9,
            )
            .expect("compile finish should be accepted");

        assert_eq!(finish.accepted_sections, BTreeSet::from([key]));
        assert!(finish.stale_sections.is_empty());
        let update = session.apply_finished_compile_report(
            &finish.accepted_sections,
            finish.build_report.expect("accepted build report"),
            &BTreeSet::new(),
            &BTreeSet::new(),
        );

        assert_eq!(update.rebuilt_section_count(), 1);
        assert!(session.contains_section(key));
        assert!(session.contains_chunk(pos));
        assert!(session.dirty_is_empty());

        session.mark_section_dirty(key);
        let removal = session.apply_removals(&BTreeSet::from([pos]), &BTreeSet::new());

        assert_eq!(removal.removed_section_count(), 1);
        assert!(!session.contains_section(key));
        assert!(!session.contains_chunk(pos));
        assert!(session.dirty_is_empty());
    }

    #[test]
    fn render_section_session_collects_known_keys_and_requeues_stale_sections() {
        let mut session = RenderSectionSession::default();
        let pos = ChunkPos::new(2, -1);
        let cached = RenderSectionKey::new(2, 4, -1);
        let loaded = RenderSectionKey::new(2, 5, -1);
        let dirty = RenderSectionKey::new(2, 6, -1);
        let inflight = RenderSectionKey::new(2, 7, -1);
        let stale_unloaded = RenderSectionKey::new(2, 8, -1);

        session.apply_finished_compile_report(
            &BTreeSet::from([cached]),
            TexturedRenderSectionBuildReport {
                sections: vec![TexturedRenderSectionMesh {
                    key: cached,
                    mesh: TexturedVisibleChunkMesh::default(),
                    visibility: VisibilitySet::all_visible(),
                }],
                visibility_graph: VisibilityGraphBuildStats::default(),
            },
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        session.mark_section_dirty(dirty);
        session.mark_section_dirty(inflight);
        let inflight_plan = RenderSectionReadyPlan {
            ready_section_keys: BTreeSet::from([inflight]),
            ..RenderSectionReadyPlan::default()
        };
        session
            .submit_ready_plan_compile_request(&inflight_plan, Vec::new(), |request| {
                Ok::<_, std::convert::Infallible>(request)
            })
            .expect("compile submission should not fail")
            .expect("inflight section should build a compile request");

        assert_eq!(
            session.known_section_keys_for_chunk(pos, [loaded]),
            BTreeSet::from([cached, loaded, dirty, inflight])
        );

        let requeued = session
            .requeue_stale_sections(BTreeSet::from([cached, loaded, stale_unloaded]), |key| {
                key == loaded
            });

        assert_eq!(requeued, 2);
        assert!(session.dirty().dirty_sections.contains(&cached));
        assert!(session.dirty().dirty_sections.contains(&loaded));
        assert!(!session.dirty().dirty_sections.contains(&stale_unloaded));
    }

    #[test]
    fn render_section_session_finish_compile_update_applies_and_requeues_stale_sections() {
        let accepted = RenderSectionKey::new(0, 4, 0);
        let stale = RenderSectionKey::new(0, 5, 0);
        let mut session = RenderSectionSession::default();
        let ready_plan = RenderSectionReadyPlan {
            ready_section_keys: BTreeSet::from([accepted, stale]),
            ..RenderSectionReadyPlan::default()
        };

        session.mark_section_dirty(accepted);
        session.mark_section_dirty(stale);
        let request = session
            .submit_ready_plan_compile_request(&ready_plan, Vec::new(), |request| {
                Ok::<_, std::convert::Infallible>(request)
            })
            .expect("compile submission should not fail")
            .expect("ready sections should build a compile request")
            .output;
        session.mark_section_dirty(stale);
        let finished = session
            .finish_compile_update(
                RenderSectionCompileResult {
                    target_sections: request.target_sections,
                    section_revisions: request.section_revisions,
                    result: Ok(test_build_report([accepted, stale])),
                },
                17,
                &BTreeSet::new(),
                &BTreeSet::new(),
                |key| key == stale,
            )
            .expect("compile update should finish");

        assert_eq!(
            finished.acceptance_report,
            RenderSectionCompileAcceptanceReport {
                request_id: 17,
                submitted_section_count: 2,
                accepted_section_count: 1,
                stale_section_count: 1,
            }
        );
        assert_eq!(finished.accepted_sections, BTreeSet::from([accepted]));
        assert_eq!(finished.stale_sections, BTreeSet::from([stale]));
        assert_eq!(finished.cache_update.rebuilt_section_count(), 1);
        assert_eq!(finished.cache_update.completed_compile_section_count, 1);
        assert_eq!(finished.cache_update.stale_compile_section_count, 1);
        assert!(session.contains_section(accepted));
        assert!(!session.contains_section(stale));
        assert!(!session.dirty().dirty_sections.contains(&accepted));
        assert!(session.dirty().dirty_sections.contains(&stale));
        assert!(session.dirty().inflight_sections.is_empty());
    }

    #[test]
    fn render_section_session_finish_compile_update_applies_removals_without_accepted_sections() {
        let key = RenderSectionKey::new(1, 4, 0);
        let pos = ChunkPos::new(1, 0);
        let mut session = RenderSectionSession::default();
        session.apply_finished_compile_report(
            &BTreeSet::from([key]),
            test_build_report([key]),
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        let ready_plan = RenderSectionReadyPlan {
            ready_section_keys: BTreeSet::from([key]),
            ..RenderSectionReadyPlan::default()
        };

        session.mark_section_dirty(key);
        let request = session
            .submit_ready_plan_compile_request(&ready_plan, Vec::new(), |request| {
                Ok::<_, std::convert::Infallible>(request)
            })
            .expect("compile submission should not fail")
            .expect("ready section should build a compile request")
            .output;
        session.mark_section_dirty(key);
        let finished = session
            .finish_compile_update(
                RenderSectionCompileResult {
                    target_sections: request.target_sections,
                    section_revisions: request.section_revisions,
                    result: Ok(TexturedRenderSectionBuildReport::default()),
                },
                18,
                &BTreeSet::from([pos]),
                &BTreeSet::new(),
                |_| false,
            )
            .expect("removal-only compile update should finish");

        assert!(finished.accepted_sections.is_empty());
        assert_eq!(finished.stale_sections, BTreeSet::from([key]));
        assert_eq!(finished.cache_update.rebuilt_section_count(), 0);
        assert_eq!(finished.cache_update.removed_section_count(), 1);
        assert_eq!(finished.cache_update.stale_compile_section_count, 1);
        assert!(!session.contains_section(key));
        assert!(!session.dirty().dirty_sections.contains(&key));
        assert!(session.dirty().inflight_sections.is_empty());
    }

    #[test]
    fn render_section_session_keeps_ready_work_dirty_when_submit_fails() {
        let mut session = RenderSectionSession::default();
        let key = RenderSectionKey::new(2, 4, -1);
        let ready_plan = RenderSectionReadyPlan {
            ready_section_keys: BTreeSet::from([key]),
            ..RenderSectionReadyPlan::default()
        };

        session.mark_section_dirty(key);
        let result: std::result::Result<Option<RenderSectionCompileSubmission<()>>, &str> = session
            .submit_ready_plan_compile_request(&ready_plan, Vec::new(), |_request| {
                Err("submit failed")
            });

        assert_eq!(result, Err("submit failed"));
        assert!(session.dirty().dirty_sections.contains(&key));
        assert!(session.dirty().inflight_sections.is_empty());
    }

    #[test]
    fn render_section_session_submit_prepared_sync_plan_applies_empty_ready_work() {
        let chunk = ChunkPos::new(0, 0);
        let deferred = RenderSectionKey::new(0, 4, 0);
        let mut session = RenderSectionSession::default();
        session.mark_chunk_dirty(chunk, [deferred]);
        let sync_update = session.prepare_sync_update(
            |pos| pos == chunk,
            |key| key == deferred,
            |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
            |dirty_work| {
                dirty_work
                    .loaded_dirty_sections_by_chunk
                    .keys()
                    .copied()
                    .collect()
            },
            1,
            |_| vec![deferred],
            |_| RenderSectionNeighborReadiness::DeferredMissingNeighbors,
            RenderSectionRemovalMode::Defer,
        );

        let result: std::result::Result<RenderSectionReadyWorkSubmission<()>, &str> = session
            .submit_prepared_sync_plan(
                &sync_update.sync_plan,
                Vec::new(),
                |_sync_plan, _request| panic!("empty ready plans must not submit compile requests"),
            );

        let update = result.expect("empty ready plan should be applied");
        assert!(update.submission.is_none());
        assert_eq!(update.cache_update.submitted_compile_section_count, 0);
        assert_eq!(update.cache_update.deferred_section_count, 1);
        assert_eq!(session.dirty().dirty_chunks, BTreeSet::from([chunk]));
        assert_eq!(session.dirty().dirty_sections, BTreeSet::from([deferred]));
        assert!(session.dirty().inflight_sections.is_empty());
    }

    #[test]
    fn render_section_session_submit_prepared_sync_plan_marks_ready_work_inflight() {
        let key = RenderSectionKey::new(0, 4, 0);
        let mut session = RenderSectionSession::default();
        session.mark_section_dirty(key);
        let sync_update = session.prepare_sync_update(
            |_| true,
            |candidate| candidate == key,
            |dirty_work| dirty_work.loaded_dirty_chunks.iter().copied().collect(),
            |dirty_work| {
                dirty_work
                    .loaded_dirty_sections_by_chunk
                    .keys()
                    .copied()
                    .collect()
            },
            1,
            |_| Vec::new(),
            |_| RenderSectionNeighborReadiness::ReadyWithNeighbors,
            RenderSectionRemovalMode::Defer,
        );

        let update = session
            .submit_prepared_sync_plan(&sync_update.sync_plan, Vec::new(), |_sync_plan, request| {
                Ok::<_, std::convert::Infallible>(request)
            })
            .expect("ready plan should submit");
        let submission = update
            .submission
            .expect("ready section should produce a compile request");

        assert_eq!(submission.submitted_section_count, 1);
        assert_eq!(submission.output.target_sections, BTreeSet::from([key]));
        assert_eq!(update.cache_update.submitted_compile_section_count, 1);
        assert!(session.dirty().dirty_sections.is_empty());
        assert_eq!(session.dirty().inflight_sections, BTreeSet::from([key]));
    }

    #[test]
    fn compile_result_partitions_accepted_and_stale_revisions() {
        let unchanged = RenderSectionKey::new(0, 4, 0);
        let changed = RenderSectionKey::new(0, 5, 0);
        let default_revision = RenderSectionKey::new(0, 6, 0);
        let result = RenderSectionCompileResult {
            target_sections: BTreeSet::from([unchanged, changed, default_revision]),
            section_revisions: BTreeMap::from([(unchanged, 7), (changed, 3)]),
            result: Ok(TexturedRenderSectionBuildReport::default()),
        };

        let acceptance = result.partition_by_revision(|key| {
            if key == changed {
                4
            } else if key == unchanged {
                7
            } else {
                0
            }
        });

        assert_eq!(acceptance.accepted_section_count(), 2);
        assert_eq!(acceptance.stale_section_count(), 1);
        assert_eq!(
            acceptance.accepted_sections,
            BTreeSet::from([unchanged, default_revision])
        );
        assert_eq!(acceptance.stale_sections, BTreeSet::from([changed]));
    }

    #[test]
    fn packed_build_report_roundtrips_section_mesh_payload() {
        let key = RenderSectionKey::new(-2, 5, 7);
        let visibility = VisibilitySet::from_bits(0b101010);
        let report = TexturedRenderSectionBuildReport {
            sections: vec![TexturedRenderSectionMesh {
                key,
                mesh: TexturedVisibleChunkMesh {
                    vertices: vec![
                        TexturedChunkVertex {
                            position: [1.0, 2.0, 3.0],
                            uv: [0.25, 0.75],
                            color: [1.0, 0.5, 0.25, 1.0],
                            packed_light: 0x00f0_00f0,
                        },
                        TexturedChunkVertex {
                            position: [4.0, 5.0, 6.0],
                            uv: [0.5, 0.125],
                            color: [0.25, 0.5, 1.0, 1.0],
                            packed_light: 0x000f_000f,
                        },
                    ],
                    indices: vec![0, 1, 0],
                    opaque_index_count: 1,
                },
                visibility,
            }],
            visibility_graph: VisibilityGraphBuildStats {
                build_count: 1,
                total_ms: 2.5,
                worst_ms: 2.5,
            },
        };

        let encoded = encode_textured_render_section_build_report(&report);
        let decoded = decode_textured_render_section_build_report(&encoded).unwrap();

        assert_eq!(decoded, report);
        let summary = summarize_textured_render_section_build_report(&decoded);
        assert_eq!(summary.section_count, 1);
        assert_eq!(summary.non_empty_section_count, 1);
        assert_eq!(summary.vertex_count, 2);
        assert_eq!(summary.index_count, 3);
    }

    #[test]
    fn packed_build_report_rejects_invalid_bytes() {
        assert!(decode_textured_render_section_build_report(b"bad").is_err());

        let report = TexturedRenderSectionBuildReport::default();
        let mut encoded = encode_textured_render_section_build_report(&report);
        encoded.push(1);

        assert!(decode_textured_render_section_build_report(&encoded).is_err());
    }
}
