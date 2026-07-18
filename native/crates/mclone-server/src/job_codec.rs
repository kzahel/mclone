#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{BlockPos, ChunkPos, ChunkSnapshot, ChunkStatus, PackedLightSection};
use mclone_protocol::{ServerUpdate, decode_server_update, encode_server_update};
use mclone_worldgen::feature::{DecorationStep, FeatureDecorationTiming};
use mclone_worldgen::levelgen::{
    GeneratedChunk, MutableChunkBlockBuffer, OverworldDependencyGenerationTiming,
    OverworldFeatureBatchTiming, OverworldFeatureDependencyCache,
    OverworldFeatureDependencyCacheReport, ScheduledTick, SmallIslandFeatureDependencyCache,
    SmallIslandFeatureDependencyCacheReport, SurfaceFillTiming, generate_flat_grass_chunk,
};

use crate::level_light_bridge::LevelLightComputationTiming;
use crate::light_mailbox::CompletedLightStatus;
use crate::light_status::{PendingLightStatus, PendingLightStatusBatch};
use crate::light_world::RetainedInitialLightState;
use crate::lighting_seed::provisional_sky_light_includes_chunk;
use crate::persistence::ScheduledTickRecord;
use crate::{
    ChunkJobId, GenerationExecutionRequest, GenerationInput, GenerationInputArtifact,
    GenerationPlanRequest, WorldGenerationDescriptor, WorldGenerationProfile,
};

const WORLDGEN_REQUEST_MAGIC: u32 = 0x5747_4A52;
const WORLDGEN_RESPONSE_MAGIC: u32 = 0x5747_4A53;
/// 069 Stage 1: the web worldgen worker holds a resident dependency cache across
/// jobs, so the request is a per-job *delta* (mirror generation + reset flag +
/// targets + only the dependency columns the worker does not already hold)
/// instead of re-shipping the whole 529-chunk dependency neighbourhood every job
/// — the 067 Stage 4 resident-mirror + delta pattern applied to worldgen. The
/// distinct magic keeps the delta frame from being mistaken for the legacy
/// full-frame request (`WORLDGEN_REQUEST_MAGIC`, kept for desktop-parity host
/// tests). Desktop is untouched: it moves the dependency `Vec` over `mpsc`.
const WORLDGEN_DELTA_REQUEST_MAGIC: u32 = 0x5747_4A44;
const LIGHT_REQUEST_MAGIC: u32 = 0x4C54_4A52;
const LIGHT_RESPONSE_MAGIC: u32 = 0x4C54_4A53;
const JOB_FRAME_VERSION: u32 = 5;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GenerationCacheReport {
    pub(crate) requested_dependency_chunks: usize,
    pub(crate) cache_hits: usize,
    pub(crate) generated_dependency_chunks: usize,
    pub(crate) retained_dependency_chunks: usize,
}

impl From<OverworldFeatureDependencyCacheReport> for GenerationCacheReport {
    fn from(report: OverworldFeatureDependencyCacheReport) -> Self {
        Self {
            requested_dependency_chunks: report.requested_dependency_chunks,
            cache_hits: report.cache_hits,
            generated_dependency_chunks: report.generated_dependency_chunks,
            retained_dependency_chunks: report.retained_dependency_chunks,
        }
    }
}

impl From<SmallIslandFeatureDependencyCacheReport> for GenerationCacheReport {
    fn from(report: SmallIslandFeatureDependencyCacheReport) -> Self {
        Self {
            requested_dependency_chunks: report.requested_dependency_chunks,
            cache_hits: report.cache_hits,
            generated_dependency_chunks: report.generated_dependency_chunks,
            retained_dependency_chunks: report.retained_dependency_chunks,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GenerationDiagnostics {
    pub(crate) cache_report: GenerationCacheReport,
    /// Only the Overworld generator currently exposes detailed phase timing.
    /// Cache diagnostics remain profile-neutral.
    pub(crate) overworld_timing: Option<OverworldFeatureBatchTiming>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorldGenerationBatchResult {
    pub(crate) chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    pub(crate) retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    /// Present for dependency-bearing profiles. Target-only profiles do not
    /// manufacture zero-valued cache diagnostics.
    pub(crate) diagnostics: Option<GenerationDiagnostics>,
}

pub(crate) fn encode_worldgen_request(
    job_id: ChunkJobId,
    request: &GenerationExecutionRequest,
) -> Result<Vec<u8>, String> {
    let mut writer = FrameWriter::new(WORLDGEN_REQUEST_MAGIC);
    writer.write_u64(job_id.0);
    writer.write_generation_execution_request(request)?;
    Ok(writer.into_bytes())
}

pub(crate) fn decode_worldgen_response(bytes: &[u8]) -> Result<WorldgenJobFrame, String> {
    let mut reader = FrameReader::new(bytes, WORLDGEN_RESPONSE_MAGIC)?;
    let job_id = ChunkJobId(reader.read_u64()?);
    let descriptor = reader.read_world_generation_descriptor()?;
    let generated_chunks = reader.read_map("generated chunks", |reader| {
        let pos = reader.read_chunk_pos()?;
        let chunk = reader.read_generated_chunk()?;
        Ok((pos, chunk))
    })?;
    let retained_dependencies = reader.read_map("response dependencies", |reader| {
        let pos = reader.read_chunk_pos()?;
        let chunk = reader.read_mutable_chunk()?;
        Ok((pos, chunk))
    })?;
    let retained_dependency_positions =
        reader.read_vec("retained dependency positions", FrameReader::read_chunk_pos)?;
    let diagnostics = if reader.read_bool()? {
        let cache_report = reader.read_generation_cache_report()?;
        let overworld_timing = if reader.read_bool()? {
            Some(reader.read_feature_batch_timing()?)
        } else {
            None
        };
        Some(GenerationDiagnostics {
            cache_report,
            overworld_timing,
        })
    } else {
        None
    };
    reader.finish()?;
    Ok(WorldgenJobFrame {
        job_id,
        descriptor,
        generated_chunks,
        retained_dependencies,
        retained_dependency_positions,
        diagnostics,
    })
}

pub fn compute_worldgen_job_frame(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut reader = FrameReader::new(bytes, WORLDGEN_REQUEST_MAGIC)?;
    let job_id = ChunkJobId(reader.read_u64()?);
    let request = reader.read_generation_execution_request()?;
    reader.finish()?;

    let descriptor = request.descriptor();
    let targets = request.requested_outputs().to_vec();
    let mut executor = WorldGenerationExecutor::default();
    let result = executor.execute(request)?;

    // Stateless path: the cache started empty, so every retained column is new this
    // job and the response subset is the whole retained set (matches the legacy
    // full response and the native `mpsc` worker's full set).
    encode_worldgen_response(job_id, descriptor, result, &BTreeSet::new(), &targets)
}

#[derive(Debug, Default)]
pub(crate) struct WorldGenerationExecutor {
    overworld_cache: OverworldFeatureDependencyCache,
    small_island_cache: SmallIslandFeatureDependencyCache,
}

impl WorldGenerationExecutor {
    pub(crate) fn clear(&mut self) {
        self.overworld_cache.clear();
        self.small_island_cache.clear();
    }

    pub(crate) fn resident_positions(&self, profile: WorldGenerationProfile) -> BTreeSet<ChunkPos> {
        match profile {
            WorldGenerationProfile::Overworld => self.overworld_cache.resident_positions(),
            WorldGenerationProfile::SmallIslandV1 => self.small_island_cache.resident_positions(),
            WorldGenerationProfile::FlatGrassV1 | WorldGenerationProfile::AuthoredOnly { .. } => {
                BTreeSet::new()
            }
        }
    }

    pub(crate) fn retained_chunk_count(&self, profile: WorldGenerationProfile) -> usize {
        match profile {
            WorldGenerationProfile::Overworld => self.overworld_cache.retained_chunk_count(),
            WorldGenerationProfile::SmallIslandV1 => self.small_island_cache.retained_chunk_count(),
            WorldGenerationProfile::FlatGrassV1 | WorldGenerationProfile::AuthoredOnly { .. } => 0,
        }
    }

    pub(crate) fn execute(
        &mut self,
        request: GenerationExecutionRequest,
    ) -> Result<WorldGenerationBatchResult, String> {
        let declared_plan = request.plan.plan();
        let descriptor = request.descriptor();
        let targets = request.plan.requested_outputs;
        let mut seen_requirements = BTreeSet::new();
        for input in &request.seeded_inputs {
            if !declared_plan.prerequisites().contains(&input.requirement) {
                return Err(format!(
                    "{} input ({}, {}) at {:?} was not declared by its generation plan",
                    descriptor.profile.label(),
                    input.requirement.pos.x,
                    input.requirement.pos.z,
                    input.requirement.status
                ));
            }
            if !seen_requirements.insert(input.requirement) {
                return Err(format!(
                    "{} received duplicate input ({}, {}) at {:?}",
                    descriptor.profile.label(),
                    input.requirement.pos.x,
                    input.requirement.pos.z,
                    input.requirement.status
                ));
            }
        }
        let dependencies = request
            .seeded_inputs
            .into_iter()
            .map(GenerationInput::into_chunk_blocks)
            .collect::<Vec<_>>();

        match descriptor.profile {
            WorldGenerationProfile::Overworld => {
                let result = self
                    .overworld_cache
                    .generate_features_chunks_with_dependencies(
                        descriptor.seed,
                        targets.iter().copied(),
                        dependencies,
                    );
                Ok(WorldGenerationBatchResult {
                    chunks: result.chunks,
                    retained_dependencies: result.retained_dependencies,
                    diagnostics: Some(GenerationDiagnostics {
                        cache_report: result.cache_report.into(),
                        overworld_timing: Some(result.timing),
                    }),
                })
            }
            WorldGenerationProfile::FlatGrassV1 => {
                if !dependencies.is_empty() {
                    return Err(format!(
                        "flat-grass-v1 is target-only but received {} dependency chunks",
                        dependencies.len()
                    ));
                }
                let chunks = targets
                    .iter()
                    .copied()
                    .map(|pos| (pos, generate_flat_grass_chunk(pos.x, pos.z)))
                    .collect();
                Ok(WorldGenerationBatchResult {
                    chunks,
                    retained_dependencies: BTreeMap::new(),
                    diagnostics: None,
                })
            }
            WorldGenerationProfile::SmallIslandV1 => {
                let result = self
                    .small_island_cache
                    .generate_features_chunks_with_dependencies(
                        descriptor.seed,
                        targets.iter().copied(),
                        dependencies,
                    );
                Ok(WorldGenerationBatchResult {
                    chunks: result.chunks,
                    retained_dependencies: result.retained_dependencies,
                    diagnostics: Some(GenerationDiagnostics {
                        cache_report: result.cache_report.into(),
                        overworld_timing: None,
                    }),
                })
            }
            WorldGenerationProfile::AuthoredOnly { .. } => Err(
                "authored-only missing chunks must bypass the procedural worldgen worker"
                    .to_owned(),
            ),
        }
    }
}

/// 069 Stage 1: encode the request as a *delta* against the web worker's resident
/// dependency mirror. `reset` clears the worker mirror and adopts `generation`
/// (first job / post-failure full resync — every dependency the scheduler holds
/// ships as an `upsert`); a non-reset delta carries `generation` for the worker's
/// desync tripwire and only the dependency columns the worker does not already
/// hold. No eviction list is needed: the worker's dependency cache auto-retains
/// to each job's plan (`generate_features_chunks_with_dependencies`), so it bounds
/// its own memory, and the mailbox shadow is corrected from the response's
/// retained set. Dependency buffers are deterministic functions of `(seed, pos)`
/// and are never feature-mutated, so any column the worker is missing is
/// regenerated byte-identically — the delta only changes transport, never output.
pub(crate) fn encode_worldgen_delta_request(
    job_id: ChunkJobId,
    generation: u64,
    reset: bool,
    request: &GenerationExecutionRequest,
) -> Result<Vec<u8>, String> {
    let mut writer = FrameWriter::new(WORLDGEN_DELTA_REQUEST_MAGIC);
    writer.write_u64(job_id.0);
    writer.write_u64(generation);
    writer.write_bool(reset);
    writer.write_generation_execution_request(request)?;
    Ok(writer.into_bytes())
}

/// 069 Stage 2: a retained dependency column is shipped back in the response only
/// if it is **new to the worker this job** (so the scheduler can store it for
/// future re-seeds — it was a cache miss or freshly upserted column the scheduler
/// may not already hold) **or** within the **light-neighbour ring** of a target
/// (so `PendingLightStatus::from_feature_publication` produces byte-identical
/// `neighbor_blocks` straight from the response, with no dependence on the
/// eviction-prone scheduler holders). Columns that were already resident in the
/// worker mirror *and* outside every target's light ring are not re-shipped — the
/// scheduler already holds them (or regenerates them deterministically). On the
/// stateless full-frame path `resident_before` is empty, so this is the whole
/// retained set.
fn worldgen_response_subset_positions(
    retained: &BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    resident_before: &BTreeSet<ChunkPos>,
    targets: &[ChunkPos],
) -> Vec<ChunkPos> {
    retained
        .keys()
        .copied()
        .filter(|pos| {
            !resident_before.contains(pos)
                || targets
                    .iter()
                    .any(|target| provisional_sky_light_includes_chunk(*target, *pos))
        })
        .collect()
}

fn encode_worldgen_response(
    job_id: ChunkJobId,
    descriptor: WorldGenerationDescriptor,
    result: WorldGenerationBatchResult,
    resident_before: &BTreeSet<ChunkPos>,
    targets: &[ChunkPos],
) -> Result<Vec<u8>, String> {
    let subset_positions =
        worldgen_response_subset_positions(&result.retained_dependencies, resident_before, targets);

    let mut writer = FrameWriter::new(WORLDGEN_RESPONSE_MAGIC);
    writer.write_u64(job_id.0);
    writer.write_world_generation_descriptor(descriptor);
    writer.write_len("generated chunks", result.chunks.len())?;
    for (pos, chunk) in &result.chunks {
        writer.write_chunk_pos(*pos);
        writer.write_generated_chunk(chunk)?;
    }
    // Response-dependency subset (buffers): only the columns the main side needs to
    // consume — new-to-worker columns for holder storage + the targets' light ring.
    writer.write_len("response dependencies", subset_positions.len())?;
    for pos in &subset_positions {
        let dependency = result
            .retained_dependencies
            .get(pos)
            .expect("subset position must be present in retained dependencies");
        writer.write_chunk_pos(*pos);
        writer.write_mutable_chunk(dependency)?;
    }
    // Full retained positions (positions only) for the main-side mirror shadow.
    writer.write_len(
        "retained dependency positions",
        result.retained_dependencies.len(),
    )?;
    for pos in result.retained_dependencies.keys() {
        writer.write_chunk_pos(*pos);
    }
    writer.write_bool(result.diagnostics.is_some());
    if let Some(diagnostics) = result.diagnostics {
        writer.write_generation_cache_report(diagnostics.cache_report);
        writer.write_bool(diagnostics.overworld_timing.is_some());
        if let Some(timing) = diagnostics.overworld_timing {
            writer.write_feature_batch_timing(timing)?;
        }
    }
    Ok(writer.into_bytes())
}

/// 069 Stage 1: the web worldgen worker's resident session. Mirrors the 067
/// Stage 4 `WebRenderCompilerSession` resident-mirror discipline: one
/// profile-neutral dependency executor held across jobs (instead of a fresh
/// cache per job, as the stateless
/// [`compute_worldgen_job_frame`] free function and the native `mpsc` worker
/// still do) plus a `mirror_generation` epoch for the desync tripwire. Each job
/// applies its request delta to the resident cache, then generates from it, so
/// cache hits on the overlapping dependency neighbourhood avoid both the
/// re-serialization in and the regeneration of those columns.
#[derive(Debug, Default)]
pub struct WorldgenJobSession {
    executor: WorldGenerationExecutor,
    descriptor: Option<WorldGenerationDescriptor>,
    mirror_generation: Option<u64>,
    last_delta_upsert_count: usize,
    last_delta_reset: bool,
}

impl WorldgenJobSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a [`encode_worldgen_delta_request`] frame to the resident dependency
    /// mirror and generate the job from it, returning the worldgen response frame.
    /// A `reset` delta clears the mirror and adopts the delta's generation; a
    /// non-reset delta whose generation does not match the resident mirror's is a
    /// desync (e.g. a worker that silently lost its mirror) and is rejected loudly
    /// rather than generated against a partial mirror — the 067 Stage 4 desync
    /// tripwire. The response (069 Stage 2) ships only the dependency columns that
    /// became resident this job plus the targets' light ring; the resident mirror
    /// before this job determines that subset.
    pub fn compute_delta_job_frame(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let mut reader = FrameReader::new(bytes, WORLDGEN_DELTA_REQUEST_MAGIC)?;
        let job_id = ChunkJobId(reader.read_u64()?);
        let generation = reader.read_u64()?;
        let reset = reader.read_bool()?;
        let request = reader.read_generation_execution_request()?;
        reader.finish()?;
        let descriptor = request.descriptor();
        let targets = request.requested_outputs().to_vec();
        let upsert_count = request.seeded_inputs.len();

        if reset {
            self.executor.clear();
            self.descriptor = Some(descriptor);
            self.mirror_generation = Some(generation);
        } else if self.mirror_generation != Some(generation) {
            return Err(format!(
                "worldgen worker mirror desync: delta generation {generation} does not match \
                 resident mirror generation {:?}; a reset delta (full resync) is required",
                self.mirror_generation
            ));
        } else if self.descriptor != Some(descriptor) {
            return Err(format!(
                "worldgen worker descriptor changed from {:?} to {descriptor:?} without a reset",
                self.descriptor
            ));
        }

        self.last_delta_upsert_count = upsert_count;
        self.last_delta_reset = reset;

        // Snapshot what the mirror held before this job; the response ships only the
        // columns that become resident this job (plus the light ring) — see
        // `worldgen_response_subset_positions`.
        let resident_before = self.executor.resident_positions(descriptor.profile);

        let result = self.executor.execute(request)?;

        encode_worldgen_response(job_id, descriptor, result, &resident_before, &targets)
    }

    /// Number of dependency columns currently resident in the worker mirror
    /// (bounded to the last job's plan by the cache's own retain step).
    pub fn mirror_chunk_count(&self) -> usize {
        self.descriptor.map_or(0, |descriptor| {
            self.executor.retained_chunk_count(descriptor.profile)
        })
    }

    /// The resident mirror's adopted generation, or `None` before the first
    /// reset delta.
    pub const fn mirror_generation(&self) -> Option<u64> {
        self.mirror_generation
    }

    /// Generator identity adopted by the most recent reset delta.
    pub const fn descriptor(&self) -> Option<WorldGenerationDescriptor> {
        self.descriptor
    }

    /// Dependency columns shipped as upserts on the most recent delta.
    pub const fn last_delta_upsert_count(&self) -> usize {
        self.last_delta_upsert_count
    }

    /// Whether the most recent delta was a full-resync reset.
    pub const fn last_delta_was_reset(&self) -> bool {
        self.last_delta_reset
    }
}

pub(crate) fn encode_light_status_request(
    batch: PendingLightStatusBatch,
) -> Result<Vec<u8>, String> {
    let mut writer = FrameWriter::new(LIGHT_REQUEST_MAGIC);
    let statuses = batch.into_statuses();
    writer.write_len("light statuses", statuses.len())?;
    for status in &statuses {
        writer.write_pending_light_status(status)?;
    }
    Ok(writer.into_bytes())
}

pub(crate) fn decode_light_status_response(
    bytes: &[u8],
) -> Result<Vec<CompletedLightStatus>, String> {
    let mut reader = FrameReader::new(bytes, LIGHT_RESPONSE_MAGIC)?;
    let completed = reader.read_vec(
        "completed light statuses",
        FrameReader::read_completed_light_status,
    )?;
    reader.finish()?;
    Ok(completed)
}

pub fn compute_light_status_job_frame(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut reader = FrameReader::new(bytes, LIGHT_REQUEST_MAGIC)?;
    let statuses = reader.read_vec("light statuses", FrameReader::read_pending_light_status)?;
    reader.finish()?;

    let mut light_state = RetainedInitialLightState::new();
    let completed =
        CompletedLightStatus::from_batch(&mut light_state, PendingLightStatusBatch::new(statuses));

    let mut writer = FrameWriter::new(LIGHT_RESPONSE_MAGIC);
    writer.write_len("completed light statuses", completed.len())?;
    for status in &completed {
        writer.write_completed_light_status(status)?;
    }
    Ok(writer.into_bytes())
}

pub(crate) struct WorldgenJobFrame {
    pub(crate) job_id: ChunkJobId,
    pub(crate) descriptor: WorldGenerationDescriptor,
    pub(crate) generated_chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    /// 069 Stage 2: the *response-dependency subset* the worker actually shipped —
    /// the dependency columns new to the worker this job (for the scheduler's
    /// holder storage) plus the targets' light-neighbour ring (so light is
    /// byte-identical without sourcing from the eviction-prone holders). On the
    /// stateless full-frame path (and the native `mpsc` worker) this is the whole
    /// retained set. The scheduler feeds it to both `mark_dependency_ready` and
    /// `from_feature_publication`; both are satisfied because it contains the ring.
    pub(crate) retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    /// 069 Stage 2: the *full* set of dependency positions the worker mirror holds
    /// after this job (positions only, ~8 B each). The web mailbox uses this to
    /// correct its mirror shadow to the worker's authoritative retained set even
    /// though the buffers above are only a subset.
    pub(crate) retained_dependency_positions: Vec<ChunkPos>,
    pub(crate) diagnostics: Option<GenerationDiagnostics>,
}

struct FrameWriter {
    bytes: Vec<u8>,
}

impl FrameWriter {
    fn new(magic: u32) -> Self {
        let mut writer = Self { bytes: Vec::new() };
        writer.write_u32(magic);
        writer.write_u32(JOB_FRAME_VERSION);
        writer
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn write_bool(&mut self, value: bool) {
        self.write_u8(u8::from(value));
    }

    fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn write_i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u128(&mut self, value: u128) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_len(&mut self, field: &str, len: usize) -> Result<(), String> {
        let len = u32::try_from(len).map_err(|_| format!("{field} length {len} exceeds u32"))?;
        self.write_u32(len);
        Ok(())
    }

    fn write_bytes(&mut self, field: &str, bytes: &[u8]) -> Result<(), String> {
        self.write_len(field, bytes.len())?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn write_string(&mut self, field: &str, value: &str) -> Result<(), String> {
        self.write_bytes(field, value.as_bytes())
    }

    fn write_chunk_pos(&mut self, pos: ChunkPos) {
        self.write_i32(pos.x);
        self.write_i32(pos.z);
    }

    fn write_world_generation_descriptor(&mut self, descriptor: WorldGenerationDescriptor) {
        self.write_u8(descriptor.profile.codec_tag());
        self.write_i64(descriptor.seed);
    }

    fn write_chunk_status(&mut self, status: ChunkStatus) {
        self.write_u8(match status {
            ChunkStatus::Terrain => 0,
            ChunkStatus::Surface => 1,
            ChunkStatus::Features => 2,
            ChunkStatus::Light => 3,
            ChunkStatus::Full => 4,
        });
    }

    fn write_generation_execution_request(
        &mut self,
        request: &GenerationExecutionRequest,
    ) -> Result<(), String> {
        self.write_world_generation_descriptor(request.descriptor());
        self.write_len(
            "worldgen requested outputs",
            request.requested_outputs().len(),
        )?;
        for output in request.requested_outputs() {
            self.write_chunk_pos(*output);
        }
        self.write_len("worldgen seeded inputs", request.seeded_inputs.len())?;
        for input in &request.seeded_inputs {
            self.write_chunk_pos(input.requirement.pos);
            self.write_chunk_status(input.requirement.status);
            match &input.artifact {
                GenerationInputArtifact::ChunkBlocks(chunk) => {
                    self.write_u8(0);
                    self.write_mutable_chunk(chunk)?;
                }
            }
        }
        Ok(())
    }

    fn write_tick(&mut self, tick: &ScheduledTick) -> Result<(), String> {
        self.write_i32(tick.x);
        self.write_i32(tick.y);
        self.write_i32(tick.z);
        self.write_string("scheduled tick target", &tick.target)?;
        self.write_i32(tick.delay);
        Ok(())
    }

    fn write_tick_record(&mut self, tick: &ScheduledTickRecord) -> Result<(), String> {
        self.write_i32(tick.pos.x);
        self.write_i32(tick.pos.y);
        self.write_i32(tick.pos.z);
        self.write_string("scheduled tick target", &tick.target)?;
        self.write_i32(tick.delay);
        Ok(())
    }

    fn write_ticks(&mut self, field: &str, ticks: &[ScheduledTick]) -> Result<(), String> {
        self.write_len(field, ticks.len())?;
        for tick in ticks {
            self.write_tick(tick)?;
        }
        Ok(())
    }

    fn write_tick_records(
        &mut self,
        field: &str,
        ticks: &[ScheduledTickRecord],
    ) -> Result<(), String> {
        self.write_len(field, ticks.len())?;
        for tick in ticks {
            self.write_tick_record(tick)?;
        }
        Ok(())
    }

    fn write_generated_chunk(&mut self, chunk: &GeneratedChunk) -> Result<(), String> {
        self.write_i32(chunk.chunk_x);
        self.write_i32(chunk.chunk_z);
        self.write_i32(chunk.min_y);
        self.write_i32(chunk.height);
        self.write_bytes("generated chunk blocks", chunk.blocks())?;
        self.write_len("generated chunk biomes", chunk.biomes().len())?;
        for biome in chunk.biomes() {
            self.write_i32(*biome);
        }
        self.write_ticks("generated chunk block ticks", chunk.block_ticks())?;
        self.write_ticks("generated chunk liquid ticks", chunk.liquid_ticks())?;
        Ok(())
    }

    fn write_mutable_chunk(&mut self, chunk: &MutableChunkBlockBuffer) -> Result<(), String> {
        self.write_i32(chunk.chunk_x);
        self.write_i32(chunk.chunk_z);
        self.write_i32(chunk.min_y);
        self.write_i32(chunk.height);
        self.write_bytes("mutable chunk blocks", &chunk.blocks)?;
        self.write_bool(chunk.has_primed_worldgen_heightmaps());
        let glow_faces = chunk.glow_lichen_faces().collect::<Vec<_>>();
        self.write_len("glow lichen faces", glow_faces.len())?;
        for (index, faces) in glow_faces {
            let index = u32::try_from(index)
                .map_err(|_| format!("glow lichen face index {index} exceeds u32"))?;
            self.write_u32(index);
            self.write_u8(faces);
        }
        self.write_ticks("mutable chunk block ticks", chunk.block_ticks())?;
        self.write_ticks("mutable chunk liquid ticks", chunk.liquid_ticks())?;
        Ok(())
    }

    fn write_snapshot(&mut self, snapshot: &ChunkSnapshot) -> Result<(), String> {
        let frame = encode_server_update(&ServerUpdate::ChunkSnapshot(snapshot.clone()))
            .map_err(|error| error.to_string())?;
        self.write_bytes("chunk snapshot", &frame)
    }

    fn write_light_section(&mut self, section: &PackedLightSection) -> Result<(), String> {
        self.write_i32(section.section_y);
        self.write_optional_light_layer("sky light layer", section.sky.as_deref())?;
        self.write_optional_light_layer("block light layer", section.block.as_deref())?;
        Ok(())
    }

    fn write_optional_light_layer(
        &mut self,
        field: &str,
        layer: Option<&[u8]>,
    ) -> Result<(), String> {
        self.write_bool(layer.is_some());
        if let Some(layer) = layer {
            self.write_bytes(field, layer)?;
        }
        Ok(())
    }

    fn write_pending_light_status(&mut self, status: &PendingLightStatus) -> Result<(), String> {
        self.write_chunk_pos(status.pos);
        self.write_snapshot(&status.feature_snapshot)?;
        self.write_tick_records(
            "light status scheduled block ticks",
            &status.scheduled_block_ticks,
        )?;
        self.write_tick_records(
            "light status scheduled fluid ticks",
            &status.scheduled_fluid_ticks,
        )?;
        self.write_bytes("light status raw blocks", status.raw_blocks())?;
        self.write_len(
            "light status neighbor blocks",
            status.neighbor_blocks().len(),
        )?;
        for (pos, blocks) in status.neighbor_blocks() {
            self.write_chunk_pos(*pos);
            self.write_bytes("light status neighbor block data", blocks)?;
        }
        Ok(())
    }

    fn write_completed_light_status(
        &mut self,
        status: &CompletedLightStatus,
    ) -> Result<(), String> {
        self.write_chunk_pos(status.pos);
        self.write_snapshot(&status.feature_snapshot)?;
        self.write_tick_records(
            "completed light status scheduled block ticks",
            &status.scheduled_block_ticks,
        )?;
        self.write_tick_records(
            "completed light status scheduled fluid ticks",
            &status.scheduled_fluid_ticks,
        )?;
        self.write_len("completed light sections", status.light_sections.len())?;
        for section in &status.light_sections {
            self.write_light_section(section)?;
        }
        self.write_bool(status.batch_compute_leader);
        self.write_u128(status.compute_us);
        self.write_level_light_timing(status.timing);
        Ok(())
    }

    fn write_generation_cache_report(&mut self, report: GenerationCacheReport) {
        self.write_u64(report.requested_dependency_chunks as u64);
        self.write_u64(report.cache_hits as u64);
        self.write_u64(report.generated_dependency_chunks as u64);
        self.write_u64(report.retained_dependency_chunks as u64);
    }

    fn write_surface_fill_timing(&mut self, timing: SurfaceFillTiming) {
        self.write_u128(timing.chunk_alloc_us);
        self.write_u128(timing.noise_columns_us);
        self.write_u128(timing.terrain_fill_us);
        self.write_u64(timing.non_air_blocks_written as u64);
    }

    fn write_dependency_timing(&mut self, timing: OverworldDependencyGenerationTiming) {
        self.write_u128(timing.generator_setup_us);
        self.write_u128(timing.surface_fill_us);
        self.write_surface_fill_timing(timing.surface_fill);
        self.write_u128(timing.surface_bedrock_us);
        self.write_u128(timing.air_carvers_us);
        self.write_u128(timing.liquid_carvers_us);
        self.write_u128(timing.heightmap_prime_us);
    }

    fn write_feature_batch_timing(
        &mut self,
        timing: OverworldFeatureBatchTiming,
    ) -> Result<(), String> {
        self.write_u128(timing.seed_dependency_insert_us);
        self.write_u128(timing.plan_us);
        self.write_u128(timing.dependency_cache_hit_clone_us);
        self.write_u128(timing.dependency_generate_us);
        self.write_dependency_timing(timing.dependency_generation);
        self.write_u128(timing.dependency_insert_clone_us);
        self.write_u128(timing.dependency_retain_us);
        self.write_u128(timing.retained_dependency_clone_us);
        self.write_u128(timing.feature_region_init_us);
        self.write_u128(timing.feature_decoration_us);
        self.write_len(
            "feature decoration timing steps",
            timing.feature_decoration_steps.step_us.len(),
        )?;
        for step_us in timing.feature_decoration_steps.step_us {
            self.write_u128(step_us);
        }
        self.write_u128(timing.target_extract_us);
        Ok(())
    }

    fn write_level_light_timing(&mut self, timing: LevelLightComputationTiming) {
        self.write_u128(timing.total_us);
        self.write_u128(timing.world_init_us);
        self.write_u128(timing.active_sections_us);
        self.write_u128(timing.sky_source_scan_us);
        self.write_u128(timing.block_source_scan_us);
        self.write_u128(timing.engine_init_us);
        self.write_u128(timing.section_setup_us);
        self.write_u128(timing.section_status_update_us);
        self.write_u128(timing.sky_column_enable_us);
        self.write_u128(timing.sky_source_enqueue_us);
        self.write_u128(timing.block_source_enqueue_us);
        self.write_u64(timing.light_status_input_chunks as u64);
        self.write_u64(timing.light_status_inserted_chunks as u64);
        self.write_u64(timing.light_status_replaced_chunks as u64);
        self.write_u64(timing.light_status_unchanged_chunks as u64);
        self.write_u64(timing.changed_block_raw_checks as u64);
        self.write_u64(timing.changed_block_light_property_changes as u64);
        self.write_u64(timing.changed_block_opacity_changes as u64);
        self.write_u64(timing.changed_block_emission_changes as u64);
        self.write_u64(timing.changed_block_raw_only_changes as u64);
        self.write_u128(timing.changed_block_check_us);
        self.write_u128(timing.run_updates_us);
        self.write_u64(timing.run_update_iterations as u64);
        self.write_u64(timing.block_run_update_calls as u64);
        self.write_u64(timing.sky_run_update_calls as u64);
        self.write_u64(timing.block_run_update_processed_nodes as u64);
        self.write_u64(timing.sky_run_update_processed_nodes as u64);
        self.write_u64(timing.max_block_run_update_queue_before as u64);
        self.write_u64(timing.max_sky_run_update_queue_before as u64);
        self.write_u64(timing.final_block_run_update_queue_after as u64);
        self.write_u64(timing.final_sky_run_update_queue_after as u64);
        self.write_u128(timing.block_run_updates_us);
        self.write_u128(timing.sky_run_updates_us);
        self.write_u64(timing.sky_source_update_count as u64);
        self.write_u128(timing.sky_source_updates_us);
        self.write_u128(timing.block_run_update_graph_us);
        self.write_u128(timing.sky_run_update_graph_us);
        self.write_u128(timing.block_run_update_storage_swap_us);
        self.write_u128(timing.sky_run_update_storage_swap_us);
        self.write_u64(timing.block_run_update_affected_sections as u64);
        self.write_u64(timing.sky_run_update_affected_sections as u64);
        self.write_u128(timing.collect_sections_us);
    }
}

struct FrameReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> FrameReader<'a> {
    fn new(bytes: &'a [u8], expected_magic: u32) -> Result<Self, String> {
        let mut reader = Self { bytes, offset: 0 };
        let magic = reader.read_u32()?;
        if magic != expected_magic {
            return Err(format!(
                "unexpected job frame magic {magic:#010x}; expected {expected_magic:#010x}"
            ));
        }
        let version = reader.read_u32()?;
        if version != JOB_FRAME_VERSION {
            return Err(format!("unsupported job frame version {version}"));
        }
        Ok(reader)
    }

    fn finish(&self) -> Result<(), String> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(format!(
                "job frame had {} trailing bytes",
                self.bytes.len() - self.offset
            ))
        }
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| "job frame offset overflow".to_owned())?;
        if end > self.bytes.len() {
            return Err(format!(
                "job frame ended while reading {len} bytes at offset {}",
                self.offset
            ));
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn read_bool(&mut self) -> Result<bool, String> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(format!("invalid bool byte {value}")),
        }
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_i32(&mut self) -> Result<i32, String> {
        let mut bytes = [0; 4];
        bytes.copy_from_slice(self.read_exact(4)?);
        Ok(i32::from_le_bytes(bytes))
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        let mut bytes = [0; 4];
        bytes.copy_from_slice(self.read_exact(4)?);
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_i64(&mut self) -> Result<i64, String> {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(self.read_exact(8)?);
        Ok(i64::from_le_bytes(bytes))
    }

    fn read_u64(&mut self) -> Result<u64, String> {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(self.read_exact(8)?);
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_u128(&mut self) -> Result<u128, String> {
        let mut bytes = [0; 16];
        bytes.copy_from_slice(self.read_exact(16)?);
        Ok(u128::from_le_bytes(bytes))
    }

    fn read_len(&mut self, field: &str) -> Result<usize, String> {
        usize::try_from(self.read_u32()?).map_err(|_| format!("{field} length exceeds usize"))
    }

    fn read_bytes(&mut self, field: &str) -> Result<Vec<u8>, String> {
        let len = self.read_len(field)?;
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_string(&mut self, field: &str) -> Result<String, String> {
        String::from_utf8(self.read_bytes(field)?)
            .map_err(|error| format!("{field} was not valid UTF-8: {error}"))
    }

    fn read_vec<T>(
        &mut self,
        field: &str,
        mut read_item: impl FnMut(&mut Self) -> Result<T, String>,
    ) -> Result<Vec<T>, String> {
        let len = self.read_len(field)?;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(read_item(self)?);
        }
        Ok(out)
    }

    fn read_map<K: Ord, V>(
        &mut self,
        field: &str,
        mut read_item: impl FnMut(&mut Self) -> Result<(K, V), String>,
    ) -> Result<BTreeMap<K, V>, String> {
        let len = self.read_len(field)?;
        let mut out = BTreeMap::new();
        for _ in 0..len {
            let (key, value) = read_item(self)?;
            out.insert(key, value);
        }
        Ok(out)
    }

    fn read_chunk_pos(&mut self) -> Result<ChunkPos, String> {
        Ok(ChunkPos::new(self.read_i32()?, self.read_i32()?))
    }

    fn read_world_generation_descriptor(&mut self) -> Result<WorldGenerationDescriptor, String> {
        let tag = self.read_u8()?;
        let profile = WorldGenerationProfile::from_codec_tag(tag)
            .ok_or_else(|| format!("unknown world generation profile tag {tag}"))?;
        let seed = self.read_i64()?;
        Ok(WorldGenerationDescriptor::new(profile, seed))
    }

    fn read_chunk_status(&mut self) -> Result<ChunkStatus, String> {
        match self.read_u8()? {
            0 => Ok(ChunkStatus::Terrain),
            1 => Ok(ChunkStatus::Surface),
            2 => Ok(ChunkStatus::Features),
            3 => Ok(ChunkStatus::Light),
            4 => Ok(ChunkStatus::Full),
            tag => Err(format!("unknown chunk status tag {tag}")),
        }
    }

    fn read_generation_execution_request(&mut self) -> Result<GenerationExecutionRequest, String> {
        let descriptor = self.read_world_generation_descriptor()?;
        let requested_outputs =
            self.read_vec("worldgen requested outputs", FrameReader::read_chunk_pos)?;
        let seeded_inputs = self.read_vec("worldgen seeded inputs", |reader| {
            let pos = reader.read_chunk_pos()?;
            let status = reader.read_chunk_status()?;
            let requirement = mclone_worldgen::levelgen::ChunkStatusRequirement { pos, status };
            let artifact_tag = reader.read_u8()?;
            let chunk = match artifact_tag {
                0 => reader.read_mutable_chunk()?,
                tag => return Err(format!("unknown generation input artifact tag {tag}")),
            };
            GenerationInput::chunk_blocks(requirement, chunk)
        })?;
        Ok(GenerationExecutionRequest::new(
            GenerationPlanRequest::new(descriptor, requested_outputs),
            seeded_inputs,
        ))
    }

    fn read_tick(&mut self) -> Result<ScheduledTick, String> {
        let x = self.read_i32()?;
        let y = self.read_i32()?;
        let z = self.read_i32()?;
        let target = self.read_string("scheduled tick target")?;
        let delay = self.read_i32()?;
        Ok(ScheduledTick::new(x, y, z, target, delay))
    }

    fn read_tick_record(&mut self) -> Result<ScheduledTickRecord, String> {
        let x = self.read_i32()?;
        let y = self.read_i32()?;
        let z = self.read_i32()?;
        let target = self.read_string("scheduled tick target")?;
        let delay = self.read_i32()?;
        Ok(ScheduledTickRecord::new(
            BlockPos::new(x, y, z),
            target,
            delay,
        ))
    }

    fn read_ticks(&mut self, field: &str) -> Result<Vec<ScheduledTick>, String> {
        self.read_vec(field, FrameReader::read_tick)
    }

    fn read_tick_records(&mut self, field: &str) -> Result<Vec<ScheduledTickRecord>, String> {
        self.read_vec(field, FrameReader::read_tick_record)
    }

    fn read_generated_chunk(&mut self) -> Result<GeneratedChunk, String> {
        let chunk_x = self.read_i32()?;
        let chunk_z = self.read_i32()?;
        let min_y = self.read_i32()?;
        let height = self.read_i32()?;
        let blocks = self.read_bytes("generated chunk blocks")?;
        let biomes = self.read_vec("generated chunk biomes", FrameReader::read_i32)?;
        let block_ticks = self.read_ticks("generated chunk block ticks")?;
        let liquid_ticks = self.read_ticks("generated chunk liquid ticks")?;
        Ok(GeneratedChunk::from_raw_parts_with_ticks_and_biomes(
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks,
            biomes,
            block_ticks,
            liquid_ticks,
        ))
    }

    fn read_mutable_chunk(&mut self) -> Result<MutableChunkBlockBuffer, String> {
        let chunk_x = self.read_i32()?;
        let chunk_z = self.read_i32()?;
        let min_y = self.read_i32()?;
        let height = self.read_i32()?;
        let blocks = self.read_bytes("mutable chunk blocks")?;
        let prime_worldgen_heightmaps = self.read_bool()?;
        let glow_lichen_faces = self.read_map("glow lichen faces", |reader| {
            let index = reader.read_u32()? as usize;
            let faces = reader.read_u8()?;
            Ok((index, faces))
        })?;
        let block_ticks = self.read_ticks("mutable chunk block ticks")?;
        let liquid_ticks = self.read_ticks("mutable chunk liquid ticks")?;
        Ok(MutableChunkBlockBuffer::from_raw_parts_with_metadata(
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks,
            glow_lichen_faces,
            block_ticks,
            liquid_ticks,
            prime_worldgen_heightmaps,
        ))
    }

    fn read_snapshot(&mut self) -> Result<ChunkSnapshot, String> {
        let frame = self.read_bytes("chunk snapshot")?;
        match decode_server_update(&frame).map_err(|error| error.to_string())? {
            ServerUpdate::ChunkSnapshot(snapshot) => Ok(snapshot),
            update => Err(format!(
                "expected chunk snapshot frame, decoded update {update:?}"
            )),
        }
    }

    fn read_light_section(&mut self) -> Result<PackedLightSection, String> {
        let section_y = self.read_i32()?;
        let sky = self.read_optional_light_layer("sky light layer")?;
        let block = self.read_optional_light_layer("block light layer")?;
        Ok(PackedLightSection::new(section_y, sky, block))
    }

    fn read_optional_light_layer(&mut self, field: &str) -> Result<Option<Vec<u8>>, String> {
        if self.read_bool()? {
            Ok(Some(self.read_bytes(field)?))
        } else {
            Ok(None)
        }
    }

    fn read_pending_light_status(&mut self) -> Result<PendingLightStatus, String> {
        let pos = self.read_chunk_pos()?;
        let feature_snapshot = self.read_snapshot()?;
        let scheduled_block_ticks = self.read_tick_records("light status scheduled block ticks")?;
        let scheduled_fluid_ticks = self.read_tick_records("light status scheduled fluid ticks")?;
        let raw_blocks = self.read_bytes("light status raw blocks")?;
        let neighbor_blocks = self.read_vec("light status neighbor blocks", |reader| {
            let pos = reader.read_chunk_pos()?;
            let blocks = reader.read_bytes("light status neighbor block data")?;
            Ok((pos, blocks))
        })?;
        Ok(PendingLightStatus::from_parts(
            pos,
            feature_snapshot,
            raw_blocks,
            neighbor_blocks,
        ))
        .map(|mut status| {
            status.scheduled_block_ticks = scheduled_block_ticks;
            status.scheduled_fluid_ticks = scheduled_fluid_ticks;
            status
        })
    }

    fn read_completed_light_status(&mut self) -> Result<CompletedLightStatus, String> {
        let pos = self.read_chunk_pos()?;
        let feature_snapshot = self.read_snapshot()?;
        let scheduled_block_ticks =
            self.read_tick_records("completed light status scheduled block ticks")?;
        let scheduled_fluid_ticks =
            self.read_tick_records("completed light status scheduled fluid ticks")?;
        let light_sections =
            self.read_vec("completed light sections", FrameReader::read_light_section)?;
        let batch_compute_leader = self.read_bool()?;
        let compute_us = self.read_u128()?;
        let timing = self.read_level_light_timing()?;
        Ok(CompletedLightStatus {
            pos,
            feature_snapshot,
            scheduled_block_ticks,
            scheduled_fluid_ticks,
            light_sections,
            batch_compute_leader,
            compute_us,
            timing,
        })
    }

    fn read_generation_cache_report(&mut self) -> Result<GenerationCacheReport, String> {
        Ok(GenerationCacheReport {
            requested_dependency_chunks: self.read_u64()? as usize,
            cache_hits: self.read_u64()? as usize,
            generated_dependency_chunks: self.read_u64()? as usize,
            retained_dependency_chunks: self.read_u64()? as usize,
        })
    }

    fn read_surface_fill_timing(&mut self) -> Result<SurfaceFillTiming, String> {
        Ok(SurfaceFillTiming {
            chunk_alloc_us: self.read_u128()?,
            noise_columns_us: self.read_u128()?,
            terrain_fill_us: self.read_u128()?,
            non_air_blocks_written: self.read_u64()? as usize,
        })
    }

    fn read_dependency_timing(&mut self) -> Result<OverworldDependencyGenerationTiming, String> {
        Ok(OverworldDependencyGenerationTiming {
            generator_setup_us: self.read_u128()?,
            surface_fill_us: self.read_u128()?,
            surface_fill: self.read_surface_fill_timing()?,
            surface_bedrock_us: self.read_u128()?,
            air_carvers_us: self.read_u128()?,
            liquid_carvers_us: self.read_u128()?,
            heightmap_prime_us: self.read_u128()?,
        })
    }

    fn read_feature_batch_timing(&mut self) -> Result<OverworldFeatureBatchTiming, String> {
        let seed_dependency_insert_us = self.read_u128()?;
        let plan_us = self.read_u128()?;
        let dependency_cache_hit_clone_us = self.read_u128()?;
        let dependency_generate_us = self.read_u128()?;
        let dependency_generation = self.read_dependency_timing()?;
        let dependency_insert_clone_us = self.read_u128()?;
        let dependency_retain_us = self.read_u128()?;
        let retained_dependency_clone_us = self.read_u128()?;
        let feature_region_init_us = self.read_u128()?;
        let feature_decoration_us = self.read_u128()?;
        let step_count = self.read_len("feature decoration timing steps")?;
        if step_count != DecorationStep::COUNT {
            return Err(format!(
                "feature decoration timing had {step_count} steps; expected {}",
                DecorationStep::COUNT
            ));
        }
        let mut step_us = [0; DecorationStep::COUNT];
        for value in &mut step_us {
            *value = self.read_u128()?;
        }
        let target_extract_us = self.read_u128()?;
        Ok(OverworldFeatureBatchTiming {
            seed_dependency_insert_us,
            plan_us,
            dependency_cache_hit_clone_us,
            dependency_generate_us,
            dependency_generation,
            dependency_insert_clone_us,
            dependency_retain_us,
            retained_dependency_clone_us,
            feature_region_init_us,
            feature_decoration_us,
            feature_decoration_steps: FeatureDecorationTiming { step_us },
            target_extract_us,
        })
    }

    fn read_level_light_timing(&mut self) -> Result<LevelLightComputationTiming, String> {
        Ok(LevelLightComputationTiming {
            total_us: self.read_u128()?,
            world_init_us: self.read_u128()?,
            active_sections_us: self.read_u128()?,
            sky_source_scan_us: self.read_u128()?,
            block_source_scan_us: self.read_u128()?,
            engine_init_us: self.read_u128()?,
            section_setup_us: self.read_u128()?,
            section_status_update_us: self.read_u128()?,
            sky_column_enable_us: self.read_u128()?,
            sky_source_enqueue_us: self.read_u128()?,
            block_source_enqueue_us: self.read_u128()?,
            light_status_input_chunks: self.read_u64()? as usize,
            light_status_inserted_chunks: self.read_u64()? as usize,
            light_status_replaced_chunks: self.read_u64()? as usize,
            light_status_unchanged_chunks: self.read_u64()? as usize,
            changed_block_raw_checks: self.read_u64()? as usize,
            changed_block_light_property_changes: self.read_u64()? as usize,
            changed_block_opacity_changes: self.read_u64()? as usize,
            changed_block_emission_changes: self.read_u64()? as usize,
            changed_block_raw_only_changes: self.read_u64()? as usize,
            changed_block_check_us: self.read_u128()?,
            run_updates_us: self.read_u128()?,
            run_update_iterations: self.read_u64()? as usize,
            block_run_update_calls: self.read_u64()? as usize,
            sky_run_update_calls: self.read_u64()? as usize,
            block_run_update_processed_nodes: self.read_u64()? as usize,
            sky_run_update_processed_nodes: self.read_u64()? as usize,
            max_block_run_update_queue_before: self.read_u64()? as usize,
            max_sky_run_update_queue_before: self.read_u64()? as usize,
            final_block_run_update_queue_after: self.read_u64()? as usize,
            final_sky_run_update_queue_after: self.read_u64()? as usize,
            block_run_updates_us: self.read_u128()?,
            sky_run_updates_us: self.read_u128()?,
            sky_source_update_count: self.read_u64()? as usize,
            sky_source_updates_us: self.read_u128()?,
            block_run_update_graph_us: self.read_u128()?,
            sky_run_update_graph_us: self.read_u128()?,
            block_run_update_storage_swap_us: self.read_u128()?,
            sky_run_update_storage_swap_us: self.read_u128()?,
            block_run_update_affected_sections: self.read_u64()? as usize,
            sky_run_update_affected_sections: self.read_u64()? as usize,
            collect_sections_us: self.read_u128()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkStatus};
    use mclone_worldgen::levelgen::OverworldFeatureBatchResult;

    fn execution_request(
        descriptor: WorldGenerationDescriptor,
        targets: &[ChunkPos],
        seeded_inputs: Vec<GenerationInput>,
    ) -> GenerationExecutionRequest {
        GenerationExecutionRequest::new(
            GenerationPlanRequest::new(descriptor, targets.to_vec()),
            seeded_inputs,
        )
    }

    fn full_worldgen_frame(
        job_id: ChunkJobId,
        descriptor: WorldGenerationDescriptor,
        targets: &[ChunkPos],
    ) -> Result<Vec<u8>, String> {
        encode_worldgen_request(job_id, &execution_request(descriptor, targets, Vec::new()))
    }

    fn delta_worldgen_frame(
        job_id: ChunkJobId,
        descriptor: WorldGenerationDescriptor,
        generation: u64,
        reset: bool,
        targets: &[ChunkPos],
    ) -> Result<Vec<u8>, String> {
        encode_worldgen_delta_request(
            job_id,
            generation,
            reset,
            &execution_request(descriptor, targets, Vec::new()),
        )
    }

    #[test]
    fn worldgen_job_frame_crosses_command_update_boundary() {
        let request = full_worldgen_frame(
            ChunkJobId(7),
            WorldGenerationDescriptor::overworld(12_345),
            &[ChunkPos::new(0, 0)],
        )
        .unwrap();
        let response = compute_worldgen_job_frame(&request).unwrap();
        let decoded = decode_worldgen_response(&response).unwrap();

        assert_eq!(decoded.job_id, ChunkJobId(7));
        assert_eq!(
            decoded.descriptor,
            WorldGenerationDescriptor::overworld(12_345)
        );
        assert!(decoded.generated_chunks.contains_key(&ChunkPos::new(0, 0)));
        let diagnostics = decoded.diagnostics.expect("overworld response diagnostics");
        assert_eq!(
            diagnostics.cache_report.retained_dependency_chunks,
            diagnostics.cache_report.requested_dependency_chunks
        );
        assert!(diagnostics.cache_report.retained_dependency_chunks > 0);
    }

    #[test]
    fn diagnostics_follow_the_profile_contract() {
        let target = [ChunkPos::new(0, 0)];
        let flat = full_worldgen_frame(
            ChunkJobId(8),
            WorldGenerationDescriptor::new(WorldGenerationProfile::FlatGrassV1, 12_345),
            &target,
        )
        .and_then(|frame| compute_worldgen_job_frame(&frame))
        .and_then(|frame| decode_worldgen_response(&frame))
        .unwrap();
        assert!(flat.diagnostics.is_none());

        let island = full_worldgen_frame(
            ChunkJobId(9),
            WorldGenerationDescriptor::new(WorldGenerationProfile::SmallIslandV1, 12_345),
            &target,
        )
        .and_then(|frame| compute_worldgen_job_frame(&frame))
        .and_then(|frame| decode_worldgen_response(&frame))
        .unwrap();
        let diagnostics = island.diagnostics.expect("island cache diagnostics");
        assert_eq!(diagnostics.cache_report.requested_dependency_chunks, 25);
        assert!(diagnostics.overworld_timing.is_none());
    }

    #[test]
    fn worldgen_frame_preserves_and_validates_typed_input_requirements() {
        let descriptor =
            WorldGenerationDescriptor::new(WorldGenerationProfile::SmallIslandV1, 12_345);
        let target = ChunkPos::new(0, 0);
        let requirement =
            mclone_worldgen::levelgen::ChunkStatusRequirement::new(target, ChunkStatus::Terrain);
        let input = GenerationInput::chunk_blocks(
            requirement,
            MutableChunkBlockBuffer::new(target.x, target.z, 0, 256),
        )
        .unwrap();
        let request = execution_request(descriptor, &[target], vec![input]);
        let frame = encode_worldgen_request(ChunkJobId(10), &request).unwrap();

        let error = compute_worldgen_job_frame(&frame).unwrap_err();
        assert!(error.contains("Terrain"), "unexpected error: {error}");
        assert!(error.contains("not declared"), "unexpected error: {error}");
    }

    fn stateless_reference(seed: i64, targets: &[ChunkPos]) -> OverworldFeatureBatchResult {
        let mut cache = OverworldFeatureDependencyCache::new();
        cache.generate_features_chunks_with_dependencies(seed, targets.iter().copied(), Vec::new())
    }

    #[test]
    fn worldgen_delta_session_matches_stateless_full_frame() {
        // 069 Stage 1: the resident-session delta path must produce byte-identical
        // worldgen output to the stateless full-frame path — the delta changes only
        // transport, never generation. First job = reset full resync (no deps held).
        let seed = 12_345;
        let descriptor = WorldGenerationDescriptor::overworld(seed);
        let targets = [ChunkPos::new(0, 0)];
        let reference = stateless_reference(seed, &targets);

        let mut session = WorldgenJobSession::new();
        let request = delta_worldgen_frame(ChunkJobId(1), descriptor, 1, true, &targets).unwrap();
        let decoded =
            decode_worldgen_response(&session.compute_delta_job_frame(&request).unwrap()).unwrap();

        assert_eq!(decoded.job_id, ChunkJobId(1));
        assert_eq!(decoded.descriptor, descriptor);
        assert_eq!(decoded.generated_chunks, reference.chunks);
        // Reset job: the mirror started empty, so every retained column is new this
        // job and the response subset is the whole retained set (matches the
        // stateless full-frame path and the native `mpsc` worker's full set).
        assert_eq!(
            decoded.retained_dependencies,
            reference.retained_dependencies
        );
        let positions: BTreeSet<_> = decoded
            .retained_dependency_positions
            .iter()
            .copied()
            .collect();
        assert_eq!(
            positions,
            reference
                .retained_dependencies
                .keys()
                .copied()
                .collect::<BTreeSet<_>>()
        );
        assert_eq!(session.mirror_generation(), Some(1));
        assert!(session.last_delta_was_reset());
        assert_eq!(
            session.mirror_chunk_count(),
            reference.retained_dependencies.len()
        );
    }

    #[test]
    fn worldgen_delta_session_reuses_resident_mirror_across_jobs() {
        // A second job whose dependency neighbourhood overlaps the first reuses the
        // resident mirror (cache hits) yet still produces byte-identical output to a
        // cold stateless generation of the same targets — residency + delta preserve
        // worldgen parity. Job 2 ships zero upserts: the worker reuses its mirror for
        // the overlap and regenerates the new strip itself.
        let seed = 12_345;
        let descriptor = WorldGenerationDescriptor::overworld(seed);
        let first_targets = [ChunkPos::new(0, 0)];
        let second_targets = [ChunkPos::new(1, 0)];

        let mut session = WorldgenJobSession::new();
        let req1 =
            delta_worldgen_frame(ChunkJobId(1), descriptor, 7, true, &first_targets).unwrap();
        session.compute_delta_job_frame(&req1).unwrap();

        let req2 =
            delta_worldgen_frame(ChunkJobId(2), descriptor, 7, false, &second_targets).unwrap();
        let decoded2 =
            decode_worldgen_response(&session.compute_delta_job_frame(&req2).unwrap()).unwrap();

        let reference2 = stateless_reference(seed, &second_targets);

        // Generation parity: the target chunks are byte-identical to a cold generation.
        assert_eq!(decoded2.generated_chunks, reference2.chunks);
        assert!(
            decoded2
                .diagnostics
                .expect("overworld response diagnostics")
                .cache_report
                .cache_hits
                > 0,
            "warm job should reuse resident dependency columns"
        );
        assert_eq!(session.last_delta_upsert_count(), 0);
        assert!(!session.last_delta_was_reset());

        // 069 Stage 2: the response ships only a *subset* of the dependency buffers
        // (the columns new to the worker this job + the targets' light ring), so it
        // is strictly smaller than the full retained set the warm job actually holds.
        let full_positions: BTreeSet<_> =
            reference2.retained_dependencies.keys().copied().collect();
        let shadow_positions: BTreeSet<_> = decoded2
            .retained_dependency_positions
            .iter()
            .copied()
            .collect();
        assert_eq!(
            shadow_positions, full_positions,
            "retained_dependency_positions must report the full retained set for the shadow"
        );
        assert!(
            decoded2.retained_dependencies.len() < full_positions.len(),
            "warm response subset ({}) should be smaller than the full retained set ({})",
            decoded2.retained_dependencies.len(),
            full_positions.len()
        );

        // Coupling B: light reads only the 3x3 ring (provisional_sky_light_includes_chunk)
        // of targets from retained_dependencies. Every retained dep in that ring must be
        // present in the response subset (with byte-identical bytes), so
        // PendingLightStatus::from_feature_publication produces identical neighbor_blocks.
        for (pos, buffer) in &reference2.retained_dependencies {
            let in_light_ring = second_targets
                .iter()
                .any(|target| provisional_sky_light_includes_chunk(*target, *pos));
            if in_light_ring {
                let shipped = decoded2.retained_dependencies.get(pos).unwrap_or_else(|| {
                    panic!("light-ring dependency {pos:?} missing from response subset")
                });
                assert_eq!(
                    shipped, buffer,
                    "light-ring dependency {pos:?} bytes diverged"
                );
            }
        }
    }

    #[test]
    fn worldgen_delta_session_rejects_generation_desync() {
        // 067 Stage 4 desync tripwire: a non-reset delta whose generation does not
        // match the resident mirror is rejected loudly instead of generated against a
        // partial mirror, and a non-reset delta before any reset (mirror generation
        // None) is likewise a desync.
        let seed = 12_345;
        let descriptor = WorldGenerationDescriptor::overworld(seed);
        let targets = [ChunkPos::new(0, 0)];

        let mut session = WorldgenJobSession::new();
        let req_reset = delta_worldgen_frame(ChunkJobId(1), descriptor, 1, true, &targets).unwrap();
        session.compute_delta_job_frame(&req_reset).unwrap();

        let req_bad = delta_worldgen_frame(ChunkJobId(2), descriptor, 2, false, &targets).unwrap();
        let err = session.compute_delta_job_frame(&req_bad).unwrap_err();
        assert!(err.contains("desync"), "unexpected error: {err}");

        let mut fresh = WorldgenJobSession::new();
        let req_premature =
            delta_worldgen_frame(ChunkJobId(3), descriptor, 1, false, &targets).unwrap();
        assert!(fresh.compute_delta_job_frame(&req_premature).is_err());
    }

    #[test]
    fn worldgen_delta_session_requires_reset_for_descriptor_change() {
        let targets = [ChunkPos::new(0, 0)];
        let mut session = WorldgenJobSession::new();
        let first = delta_worldgen_frame(
            ChunkJobId(1),
            WorldGenerationDescriptor::overworld(12_345),
            1,
            true,
            &targets,
        )
        .unwrap();
        session.compute_delta_job_frame(&first).unwrap();

        let changed_without_reset = delta_worldgen_frame(
            ChunkJobId(2),
            WorldGenerationDescriptor::overworld(54_321),
            1,
            false,
            &targets,
        )
        .unwrap();
        let error = session
            .compute_delta_job_frame(&changed_without_reset)
            .unwrap_err();
        assert!(error.contains("descriptor changed"), "{error}");

        let changed_with_reset = delta_worldgen_frame(
            ChunkJobId(3),
            WorldGenerationDescriptor::overworld(54_321),
            2,
            true,
            &targets,
        )
        .unwrap();
        session
            .compute_delta_job_frame(&changed_with_reset)
            .unwrap();
        assert_eq!(
            session.descriptor(),
            Some(WorldGenerationDescriptor::overworld(54_321))
        );
    }

    #[test]
    fn flat_grass_frames_are_target_only_and_batch_order_independent() {
        let descriptor = WorldGenerationDescriptor::new(
            WorldGenerationProfile::FlatGrassV1,
            -9_223_372_036_854_775,
        );
        let targets = [ChunkPos::new(7, -9), ChunkPos::new(-2, 3)];
        let forward = full_worldgen_frame(ChunkJobId(1), descriptor, &targets)
            .and_then(|frame| compute_worldgen_job_frame(&frame))
            .and_then(|frame| decode_worldgen_response(&frame))
            .unwrap();
        let reversed_targets = [targets[1], targets[0]];
        let reversed = full_worldgen_frame(ChunkJobId(2), descriptor, &reversed_targets)
            .and_then(|frame| compute_worldgen_job_frame(&frame))
            .and_then(|frame| decode_worldgen_response(&frame))
            .unwrap();

        assert_eq!(forward.descriptor, descriptor);
        assert_eq!(forward.generated_chunks, reversed.generated_chunks);
        assert!(forward.retained_dependencies.is_empty());
        assert!(forward.retained_dependency_positions.is_empty());
        assert_eq!(
            forward
                .generated_chunks
                .values()
                .map(GeneratedChunk::non_air_block_count)
                .collect::<Vec<_>>(),
            vec![1_024, 1_024]
        );
    }

    #[test]
    fn small_island_frames_are_dependency_bearing_partition_and_order_independent() {
        let descriptor =
            WorldGenerationDescriptor::new(WorldGenerationProfile::SmallIslandV1, -98_765);
        let targets = [
            ChunkPos::new(1, -2),
            ChunkPos::new(-3, 4),
            ChunkPos::new(0, 0),
        ];
        let batch = full_worldgen_frame(ChunkJobId(1), descriptor, &targets)
            .and_then(|frame| compute_worldgen_job_frame(&frame))
            .and_then(|frame| decode_worldgen_response(&frame))
            .unwrap();
        let reversed_targets = [targets[2], targets[1], targets[0]];
        let reversed = full_worldgen_frame(ChunkJobId(2), descriptor, &reversed_targets)
            .and_then(|frame| compute_worldgen_job_frame(&frame))
            .and_then(|frame| decode_worldgen_response(&frame))
            .unwrap();
        let partitioned = targets
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(index, target)| {
                full_worldgen_frame(ChunkJobId(index as u64 + 3), descriptor, &[target])
                    .and_then(|frame| compute_worldgen_job_frame(&frame))
                    .and_then(|frame| decode_worldgen_response(&frame))
                    .unwrap()
                    .generated_chunks
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(batch.descriptor, descriptor);
        assert_eq!(batch.generated_chunks, reversed.generated_chunks);
        assert_eq!(batch.generated_chunks, partitioned);
        assert!(!batch.retained_dependencies.is_empty());
        assert!(!batch.retained_dependency_positions.is_empty());
        assert!(batch.diagnostics.is_some());

        let other_seed = WorldGenerationDescriptor::new(
            WorldGenerationProfile::SmallIslandV1,
            descriptor.seed + 1,
        );
        let changed = full_worldgen_frame(ChunkJobId(9), other_seed, &targets)
            .and_then(|frame| compute_worldgen_job_frame(&frame))
            .and_then(|frame| decode_worldgen_response(&frame))
            .unwrap();
        assert_ne!(batch.generated_chunks, changed.generated_chunks);
    }

    #[test]
    fn light_status_job_frame_roundtrips_completed_status() {
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Features,
            ChunkRevision(3),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        );
        let pending = PendingLightStatus::from_parts(
            ChunkPos::new(0, 0),
            snapshot.clone(),
            vec![0; CHUNK_SECTION_VOLUME],
            Vec::new(),
        );
        let request =
            encode_light_status_request(PendingLightStatusBatch::new(vec![pending])).unwrap();
        let response = compute_light_status_job_frame(&request).unwrap();
        let completed = decode_light_status_response(&response).unwrap();

        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].pos, ChunkPos::new(0, 0));
        assert_eq!(completed[0].feature_snapshot, snapshot);
        assert!(completed[0].batch_compute_leader);
    }
}
