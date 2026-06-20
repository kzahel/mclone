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
        let request_id = self.take_next_request_id();
        self.bump_section_revisions(&target_sections);
        let section_revisions = target_sections
            .iter()
            .map(|key| (*key, self.section_revision(*key)))
            .collect::<BTreeMap<_, _>>();
        let submitted_section_count = target_sections.len();
        self.pending_requests.insert(
            request_id,
            RenderSectionPendingCompileRequest {
                request_id,
                context,
                target_sections,
                section_revisions,
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
