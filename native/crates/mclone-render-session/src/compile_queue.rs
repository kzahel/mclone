use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderSectionCompileRequest {
    pub target_sections: BTreeSet<RenderSectionKey>,
    pub section_revisions: BTreeMap<RenderSectionKey, u64>,
    pub snapshots: Vec<ChunkSnapshot>,
    pub biome_zoom_seed: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionCompileRequestPayloadStats {
    pub target_section_count: usize,
    pub section_revision_count: usize,
    pub snapshot_count: usize,
    pub snapshot_section_count: usize,
    pub snapshot_light_section_count: usize,
    pub estimated_owned_bytes: usize,
}

impl RenderSectionCompileRequest {
    pub fn with_biome_zoom_seed(mut self, biome_zoom_seed: Option<i64>) -> Self {
        self.biome_zoom_seed = biome_zoom_seed;
        self
    }

    pub fn payload_stats(&self) -> RenderSectionCompileRequestPayloadStats {
        let snapshot_section_count = self
            .snapshots
            .iter()
            .map(|snapshot| snapshot.sections.len())
            .sum();
        let snapshot_light_section_count = self
            .snapshots
            .iter()
            .map(|snapshot| snapshot.light_sections.len())
            .sum();
        let mut estimated_owned_bytes = self.target_sections.len()
            * std::mem::size_of::<RenderSectionKey>()
            + self.section_revisions.len()
                * (std::mem::size_of::<RenderSectionKey>() + std::mem::size_of::<u64>())
            + self.snapshots.capacity() * std::mem::size_of::<ChunkSnapshot>();
        for snapshot in &self.snapshots {
            estimated_owned_bytes += snapshot.biomes.capacity() * std::mem::size_of::<i32>();
            estimated_owned_bytes +=
                snapshot.sections.capacity() * std::mem::size_of::<PackedChunkSection>();
            estimated_owned_bytes +=
                snapshot.light_sections.capacity() * std::mem::size_of::<PackedLightSection>();
            for section in &snapshot.sections {
                estimated_owned_bytes +=
                    section.palette_state_ids.capacity() * std::mem::size_of::<BlockStateId>();
                estimated_owned_bytes +=
                    section.packed_block_indices.capacity() * std::mem::size_of::<u64>();
            }
            for light_section in &snapshot.light_sections {
                if let Some(sky) = &light_section.sky {
                    estimated_owned_bytes += sky.capacity();
                }
                if let Some(block) = &light_section.block {
                    estimated_owned_bytes += block.capacity();
                }
            }
        }

        RenderSectionCompileRequestPayloadStats {
            target_section_count: self.target_sections.len(),
            section_revision_count: self.section_revisions.len(),
            snapshot_count: self.snapshots.len(),
            snapshot_section_count,
            snapshot_light_section_count,
            estimated_owned_bytes,
        }
    }
}

#[derive(Debug)]
pub struct RenderSectionCompileResult {
    pub target_sections: BTreeSet<RenderSectionKey>,
    pub section_revisions: BTreeMap<RenderSectionKey, u64>,
    pub result: std::result::Result<TexturedRenderSectionBuildReport, String>,
}

#[derive(Clone, Debug, Default, PartialEq)]

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
                biome_zoom_seed: None,
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
        write_u32(&mut out, section.mesh.solid_index_count());
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
        let solid_index_count = reader.read_u32()?;
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
                solid_index_count,
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

    fn submit_with_timing(
        &mut self,
        request: RenderSectionCompileRequest,
    ) -> Result<RenderSectionCompileSubmitTiming> {
        self.submit(request)?;
        Ok(RenderSectionCompileSubmitTiming::default())
    }

    fn try_recv_completed(&mut self) -> Result<Vec<RenderSectionCompileResult>>;

    fn pending_job_count(&self) -> usize;

    fn max_pending_job_count(&self) -> usize {
        1
    }

    fn available_pending_job_slots(&self) -> usize {
        self.max_pending_job_count()
            .saturating_sub(self.pending_job_count())
    }

    fn queued_compile_task_count(&self) -> usize {
        self.pending_job_count()
    }

    fn compile_worker_count(&self) -> usize {
        0
    }

    fn completed_compile_task_count(&self) -> usize {
        0
    }

    fn total_compile_worker_busy_us(&self) -> u128 {
        0
    }

    fn max_compile_worker_task_us(&self) -> u128 {
        0
    }

    fn release_completed_jobs(&mut self, _count: usize) -> usize {
        0
    }

    fn has_pending_job_capacity(&self) -> bool {
        self.available_pending_job_slots() > 0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionCompileQueueHealth {
    pub pending_jobs: usize,
    pub max_pending_jobs: usize,
    pub available_job_slots: usize,
    pub queued_compile_tasks: usize,
    pub compile_worker_count: usize,
    pub completed_compile_tasks: usize,
    pub total_compile_worker_busy_us: u128,
    pub max_compile_worker_task_us: u128,
}

impl RenderSectionCompileQueueHealth {
    pub fn from_compiler<C>(compiler: &C) -> Self
    where
        C: RenderSectionCompiler + ?Sized,
    {
        let pending_jobs = compiler.pending_job_count();
        let max_pending_jobs = compiler.max_pending_job_count();
        Self {
            pending_jobs,
            max_pending_jobs,
            available_job_slots: max_pending_jobs.saturating_sub(pending_jobs),
            queued_compile_tasks: compiler.queued_compile_task_count(),
            compile_worker_count: compiler.compile_worker_count(),
            completed_compile_tasks: compiler.completed_compile_task_count(),
            total_compile_worker_busy_us: compiler.total_compile_worker_busy_us(),
            max_compile_worker_task_us: compiler.max_compile_worker_task_us(),
        }
    }
}

pub trait RenderSectionCompileDispatcher: RenderSectionCompiler {
    fn queue_health(&self) -> RenderSectionCompileQueueHealth {
        RenderSectionCompileQueueHealth::from_compiler(self)
    }
}

impl<T> RenderSectionCompileDispatcher for T where T: RenderSectionCompiler {}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RenderSectionCompileSubmitTiming {
    pub capacity_check_ms: f64,
    pub command_send_ms: f64,
    pub command_lock_wait_ms: f64,
    pub command_slot_select_ms: f64,
    pub command_slot_write_ms: f64,
    pub command_queue_push_ms: f64,
    pub command_notify_ms: f64,
    pub command_post_enqueue_ms: f64,
    pub pending_mark_ms: f64,
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
