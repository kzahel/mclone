#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use mclone_client::ClientRuntime;
use mclone_core::{
    AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkPos, ChunkSnapshot,
    PackedLightSection, SECTION_HEIGHT,
};
use mclone_mesh::{
    RenderSectionKey, TexturedChunkMeshInput, TexturedChunkVertex, TexturedMeshCatalog,
    TexturedRenderSectionBuildReport, TexturedRenderSectionMesh, TexturedVisibleChunkMesh,
    VisibilityGraphBuildStats, VisibilitySet,
    build_textured_render_sections_for_section_set_with_stats,
    build_textured_render_sections_with_stats, quad_face_count_from_indices,
};

const PACKED_BUILD_REPORT_MAGIC: &[u8; 8] = b"MCRSBR1\0";

pub fn build_client_textured_sections(
    client: &ClientRuntime,
    catalog: &TexturedMeshCatalog,
) -> Result<TexturedRenderSectionBuildReport> {
    let chunks = mesh_chunks_from_client(client)?;
    let inputs = textured_mesh_inputs(&chunks);
    build_textured_render_sections_with_stats(&inputs, catalog)
        .context("failed to build textured sections")
}

pub fn build_render_sections_from_snapshots(
    snapshots: &[ChunkSnapshot],
    catalog: &TexturedMeshCatalog,
    target_sections: &BTreeSet<RenderSectionKey>,
) -> Result<TexturedRenderSectionBuildReport> {
    let chunks = snapshots
        .iter()
        .map(snapshot_mesh_block_state_ids)
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
    let budgeted_loaded_chunks = sorted_loaded_dirty_chunks
        .into_iter()
        .take(chunk_budget)
        .collect::<BTreeSet<_>>();
    let remaining_chunk_budget = chunk_budget.saturating_sub(budgeted_loaded_chunks.len());
    let budgeted_dirty_section_chunks = sorted_dirty_section_chunks
        .into_iter()
        .filter(|pos| !budgeted_loaded_chunks.contains(pos))
        .take(remaining_chunk_budget)
        .collect::<BTreeSet<_>>();
    let mut plan = RenderSectionReadyPlan {
        budgeted_loaded_chunks,
        budgeted_dirty_section_chunks,
        ..RenderSectionReadyPlan::default()
    };

    for pos in plan
        .budgeted_loaded_chunks
        .iter()
        .copied()
        .collect::<Vec<_>>()
    {
        for key in section_keys_for_chunk(pos) {
            plan_ready_render_section_key(
                &mut plan,
                key,
                inflight_sections,
                &mut section_readiness,
            );
        }
    }
    for pos in plan
        .budgeted_dirty_section_chunks
        .iter()
        .copied()
        .collect::<Vec<_>>()
    {
        if let Some(keys) = loaded_dirty_sections_by_chunk.get(&pos) {
            for key in keys {
                plan_ready_render_section_key(
                    &mut plan,
                    *key,
                    inflight_sections,
                    &mut section_readiness,
                );
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

#[derive(Clone, Debug, Default)]
pub struct RenderSectionSession {
    cache: CachedTexturedRenderSections,
    dirty: RenderSectionDirtyState,
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
            mesh: TexturedVisibleChunkMesh { vertices, indices },
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
        AIR_BLOCK_STATE_ID, BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkStatus,
    };
    use mclone_mesh::{
        TexturedChunkVertex, TexturedRenderSectionBuildReport, TexturedVisibleChunkMesh,
        VisibilityGraphBuildStats, VisibilitySet,
    };

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
        assert!(session.dirty().dirty_chunks.is_empty());
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
