use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::mem::size_of;
use std::num::{NonZeroU32, NonZeroU64};
use std::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use std::thread::{self, JoinHandle};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use mclone_core::{BlockStateId, ChunkPos, HorizontalTopology};
use mclone_mesh::{TexturedBlockTint, TexturedMeshCatalog};
use mclone_render_color::{RenderTargetColorTransform, color_transform_wgpu};
use mclone_worldgen::levelgen::{
    McloneOverworldSamplingTopology, McloneOverworldVegetationPlanCache, McloneTreeFamily,
    McloneVegetationSource,
};
use mclone_worldgen::terrain_preview::{
    TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS, TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING,
    TerrainPreviewComparison, TerrainPreviewCompileWork, TerrainPreviewContentStage,
    TerrainPreviewProfile, TerrainPreviewReferenceGrid, TerrainPreviewRequest,
    TerrainPreviewSample, TerrainPreviewSurfaceQuality, TerrainPreviewVegetationProduct,
    ValidatedTerrainPreviewRequest, terrain_preview_gpu_compile_work,
    terrain_preview_max_tree_record_sample_spacing,
};
use mclone_worldgen::terrain_vegetation::{
    TerrainVegetationSourceIdentity, terrain_vegetation_coverage_receipt,
};

use super::{
    BoundedRepresentationOwnershipSnapshot, ExactPaintedCoverageSnapshot, McloneTreeOccurrenceId,
    McloneTreeOwnershipCandidate, TERRAIN_EXACT_BOUNDARY_MAX_BLOCKS_PER_AXIS,
    TERRAIN_EXACT_COVERAGE_MASK_BYTES, TERRAIN_EXACT_FRONTIER_TREE_INSET_BLOCKS,
    TERRAIN_EXACT_TRANSITION_MAX_TEXELS_PER_AXIS, TERRAIN_LOD_HIGH_LEVEL_COUNT,
    TERRAIN_PREVIEW_DEPTH_FORMAT, TERRAIN_PREVIEW_SAMPLE_BYTES, TERRAIN_PREVIEW_UNIFORM_BYTES,
    TERRAIN_PREVIEW_WORKGROUP_AXIS, TerrainClipmap, TerrainClipmapConfig,
    TerrainClipmapDiagnostics, TerrainClipmapTile, TerrainCompositionSourceIdentity,
    TerrainExactBoundaryProfile, TerrainExactCoverageMask, TerrainExactCoverageMode,
    TerrainExactTransitionField, TerrainFrontierAdmissionReceipt, TerrainFrontierClosure,
    TerrainFrontierDirection, TerrainFrontierFineTileKey, TerrainFrontierPlan,
    TerrainFrontierPlanOptions, TerrainFrontierPlanReceipt, TerrainFrontierPlanState,
    TerrainFrontierTopology, TerrainFrontierTopologyOptions, TerrainFrontierTopologyReceipt,
    TerrainFrontierTopologyState, TerrainHorizonDiagnostic, TerrainHorizonPresentation,
    TerrainPreviewCamera, TerrainPreviewDrawOptions, TerrainPreviewLayer, TerrainPreviewSource,
    TerrainPreviewSplitLayout, TerrainVegetationCoordinator, TerrainVegetationCoordinatorState,
    TerrainVegetationDesiredTile, TerrainVegetationExecutor, TerrainVegetationExecutorKind,
    TerrainVegetationSlotToken, TerrainViewportPlan, TerrainViewportTileId,
    horizon_admission::{
        TERRAIN_HORIZON_STAGING_SLOTS_PER_LEVEL, TerrainHorizonAdmission,
        TerrainHorizonBeginTransition, TerrainHorizonLevelPresentation, TerrainHorizonResourceTile,
    },
    mclone_tree_ownership_snapshot, parse_samples, terrain_frontier_presentation_identity,
    terrain_preview_compute_wgsl, terrain_preview_focus_y_for_profile,
    terrain_preview_render_multiview_wgsl, terrain_preview_render_wgsl,
    terrain_preview_tree_multiview_wgsl, terrain_preview_tree_wgsl,
    viewport_uniform_bytes_for_request, viewport_uniform_bytes_for_request_with_presentation,
};
use crate::frontier_admission::{
    TerrainFrontierAdmissionResource, TerrainFrontierAdmissionTracker,
};

pub const TERRAIN_PREVIEW_MATERIAL_UV_COUNT: usize = 256;
const TERRAIN_PREVIEW_MATERIAL_TABLE_BYTES: u64 =
    (TERRAIN_PREVIEW_MATERIAL_UV_COUNT * 5 * 4 * size_of::<f32>()) as u64;
const TERRAIN_EXACT_COVERAGE_UNIFORM_BYTES: u64 = 64;
const TERRAIN_EXACT_CONNECTOR_INSTANCE_BYTES: u64 = 12;
const TERRAIN_EXACT_CONNECTOR_VERTICES_PER_INSTANCE: u32 = 6;
const TERRAIN_FRONTIER_CONNECTOR_FLAG: u32 = 1 << 8;
const TERRAIN_FRONTIER_CONNECTOR_WATER_FLAG: u32 = 1 << 9;
const TERRAIN_FRONTIER_CONNECTOR_FALLBACK_FLAG: u32 = 1 << 10;
const TERRAIN_FRONTIER_CONNECTOR_OUTER_FLAG: u32 = 1 << 11;
const TERRAIN_FRONTIER_DISPATCHES_PER_FRAME: usize = 4;
const TERRAIN_FRONTIER_SUPPORT_LOOKUP_MAX_TILES_PER_AXIS: usize = 18;
const TERRAIN_FRONTIER_SUPPORT_LOOKUP_BUFFER_BYTES: u64 = 96;
const TERRAIN_HORIZON_TREE_CULL_MARGIN_BLOCKS: f32 = 16.0;
const CONTINENTAL_PROXY_TREE_MAX_VIEW_BLOCKS: f64 = 768.0;
const TERRAIN_HORIZON_CULL_MIN_Y: f32 = -64.0;
const TERRAIN_HORIZON_CULL_MAX_Y: f32 = 512.0;
const TERRAIN_EXACT_CONNECTOR_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Sint32x2,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Uint32,
        offset: 8,
        shader_location: 1,
    },
];

fn terrain_exact_connector_vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: TERRAIN_EXACT_CONNECTOR_INSTANCE_BYTES,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &TERRAIN_EXACT_CONNECTOR_VERTEX_ATTRIBUTES,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TerrainHorizonRenderTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub depth_view: &'a wgpu::TextureView,
    pub color_load: wgpu::LoadOp<wgpu::Color>,
    pub color_store: wgpu::StoreOp,
    pub depth_load: wgpu::LoadOp<f32>,
    pub depth_store: wgpu::StoreOp,
}

#[derive(Clone, Copy, Debug)]
pub struct TerrainPreviewMaterialAtlas<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    pub material_table: &'a TerrainPreviewMaterialTable,
}

/// Active-pack material facts consumed by procedural terrain. The table is
/// copied into fixed GPU uniform storage when a renderer is created.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainPreviewMaterialTable {
    top_uvs: [[f32; 4]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT],
    side_uvs: [[f32; 4]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT],
    tint_flags: [[f32; 4]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT],
    grass_tints: [[f32; 4]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT],
    water_tints: [[f32; 4]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT],
}

impl TerrainPreviewMaterialTable {
    pub fn from_catalog(catalog: &TexturedMeshCatalog) -> Self {
        let mut table = Self {
            top_uvs: [[0.0, 0.0, 1.0, 1.0]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT],
            side_uvs: [[0.0, 0.0, 1.0, 1.0]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT],
            tint_flags: [[0.0; 4]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT],
            grass_tints: [[1.0; 4]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT],
            water_tints: [[1.0; 4]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT],
        };
        for raw_id in 0..TERRAIN_PREVIEW_MATERIAL_UV_COUNT {
            if let Some(material) = catalog.terrain_surface_material(BlockStateId(raw_id as u32)) {
                table.top_uvs[raw_id] = sprite_rect(material.top);
                table.side_uvs[raw_id] = sprite_rect(material.side);
                table.tint_flags[raw_id][0] =
                    f32::from(material.top_tint == TexturedBlockTint::Grass);
                table.tint_flags[raw_id][1] =
                    f32::from(material.side_tint == TexturedBlockTint::Grass);
            }
            let tint = catalog.terrain_grass_tint(raw_id as i32);
            table.grass_tints[raw_id] = [tint[0], tint[1], tint[2], 1.0];
            table.water_tints[raw_id] = catalog.terrain_water_tint(raw_id as i32);
        }
        table
    }

    fn uniform_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(TERRAIN_PREVIEW_MATERIAL_TABLE_BYTES as usize);
        for table in [
            &self.top_uvs,
            &self.side_uvs,
            &self.tint_flags,
            &self.grass_tints,
            &self.water_tints,
        ] {
            for entry in table {
                for value in entry {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        debug_assert_eq!(bytes.len(), TERRAIN_PREVIEW_MATERIAL_TABLE_BYTES as usize);
        bytes
    }
}

fn sprite_rect(sprite: mclone_mesh::AtlasSpriteUv) -> [f32; 4] {
    [sprite.u0, sprite.v0, sprite.u1, sprite.v1]
}

pub const TERRAIN_VIEWPORT_MAX_RESIDENT_TILES: usize = 192;
pub const TERRAIN_VIEWPORT_TILE_COMPILES_PER_FRAME: usize = 4;
pub const TERRAIN_VIEWPORT_GPU_DISPATCHES_PER_FRAME: usize = 16;
pub const TERRAIN_VIEWPORT_CPU_COMPILE_BUDGET_MICROS: u64 = 8_000;
const TERRAIN_PREVIEW_TREE_INSTANCE_FLOATS: usize = 12;
const TERRAIN_PREVIEW_TREE_INSTANCE_BYTES: u64 =
    (TERRAIN_PREVIEW_TREE_INSTANCE_FLOATS * size_of::<f32>()) as u64;
const TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE: u32 = 108;
const TERRAIN_HORIZON_CANOPY_CELLS_PER_AXIS: u32 = 16;
const TERRAIN_HORIZON_CANOPY_CELLS_PER_TILE: u32 =
    TERRAIN_HORIZON_CANOPY_CELLS_PER_AXIS * TERRAIN_HORIZON_CANOPY_CELLS_PER_AXIS;
const TERRAIN_HORIZON_CANOPY_VERTICES_PER_CELL: u32 = 12;
const TERRAIN_HORIZON_CANOPY_VERTICES_PER_TILE: u32 =
    TERRAIN_HORIZON_CANOPY_CELLS_PER_TILE * TERRAIN_HORIZON_CANOPY_VERTICES_PER_CELL;
const TERRAIN_HORIZON_NORMAL_HALO_RADIUS: u32 = 2;
const TERRAIN_HORIZON_NORMAL_EDGE_WEST: u32 = 1 << 27;
const TERRAIN_HORIZON_NORMAL_EDGE_EAST: u32 = 1 << 28;
const TERRAIN_HORIZON_NORMAL_EDGE_NORTH: u32 = 1 << 29;
const TERRAIN_HORIZON_NORMAL_EDGE_SOUTH: u32 = 1 << 30;
const TERRAIN_HORIZON_SMOOTH_VERTICES_PER_CELL: u32 = 6;
const TERRAIN_HORIZON_FOREST_REVEAL_SECONDS: f32 = 0.65;
const TERRAIN_HORIZON_FOREST_REVEAL_MAX_STEP_SECONDS: f32 = 0.10;
#[cfg(not(target_arch = "wasm32"))]
const TERRAIN_HORIZON_CPU_MAX_WORKERS: usize = 4;

#[cfg(not(target_arch = "wasm32"))]
type TerrainHorizonRevealClock = Instant;
#[cfg(target_arch = "wasm32")]
type TerrainHorizonRevealClock = f64;

#[cfg(not(target_arch = "wasm32"))]
fn terrain_horizon_reveal_now() -> TerrainHorizonRevealClock {
    Instant::now()
}

#[cfg(target_arch = "wasm32")]
fn terrain_horizon_reveal_now() -> TerrainHorizonRevealClock {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn terrain_horizon_reveal_elapsed_seconds(
    previous: TerrainHorizonRevealClock,
    current: TerrainHorizonRevealClock,
) -> f32 {
    current.saturating_duration_since(previous).as_secs_f32()
}

#[cfg(target_arch = "wasm32")]
fn terrain_horizon_reveal_elapsed_seconds(
    previous: TerrainHorizonRevealClock,
    current: TerrainHorizonRevealClock,
) -> f32 {
    ((current - previous).max(0.0) / 1_000.0) as f32
}

fn advance_forest_reveal(current: f32, elapsed_seconds: f32) -> f32 {
    let step = elapsed_seconds.clamp(0.0, TERRAIN_HORIZON_FOREST_REVEAL_MAX_STEP_SECONDS)
        / TERRAIN_HORIZON_FOREST_REVEAL_SECONDS;
    (current + step).clamp(0.0, 1.0)
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
struct TerrainHorizonCpuCompileJob {
    source_generation: u64,
    resource: TerrainHorizonResourceTile,
    request: TerrainPreviewRequest,
}

#[cfg(not(target_arch = "wasm32"))]
struct TerrainHorizonCpuCompileResult {
    source_generation: u64,
    resource: TerrainHorizonResourceTile,
    request: TerrainPreviewRequest,
    compiled: Result<(TerrainPreviewReferenceGrid, Vec<f32>), String>,
    compile_micros: u64,
}

#[cfg(not(target_arch = "wasm32"))]
struct TerrainHorizonCpuWorker {
    requests: Option<mpsc::SyncSender<TerrainHorizonCpuCompileJob>>,
    handle: Option<JoinHandle<()>>,
}

#[cfg(not(target_arch = "wasm32"))]
struct TerrainHorizonCpuCompiler {
    workers: Vec<TerrainHorizonCpuWorker>,
    completions: mpsc::Receiver<TerrainHorizonCpuCompileResult>,
    next_worker: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl TerrainHorizonCpuCompiler {
    fn new() -> Result<Self, String> {
        let available = thread::available_parallelism().map_or(1, usize::from);
        let worker_count = available
            .saturating_sub(2)
            .clamp(1, TERRAIN_HORIZON_CPU_MAX_WORKERS);
        let (completion_sender, completions) = mpsc::channel();
        let mut workers = Vec::with_capacity(worker_count);
        for index in 0..worker_count {
            let (request_sender, request_receiver) =
                mpsc::sync_channel::<TerrainHorizonCpuCompileJob>(1);
            let completion_sender = completion_sender.clone();
            let handle = thread::Builder::new()
                .name(format!("mclone-terrain-horizon-{index}"))
                .spawn(move || {
                    while let Ok(job) = request_receiver.recv() {
                        let started = Instant::now();
                        let compiled =
                            TerrainPreviewReferenceGrid::compile_continental_with_height_halo(
                                job.request,
                                TERRAIN_HORIZON_NORMAL_HALO_RADIUS,
                            );
                        let result = TerrainHorizonCpuCompileResult {
                            source_generation: job.source_generation,
                            resource: job.resource,
                            request: job.request,
                            compiled,
                            compile_micros: u64::try_from(started.elapsed().as_micros())
                                .unwrap_or(u64::MAX),
                        };
                        if completion_sender.send(result).is_err() {
                            break;
                        }
                    }
                })
                .map_err(|error| format!("failed to spawn terrain horizon worker: {error}"))?;
            workers.push(TerrainHorizonCpuWorker {
                requests: Some(request_sender),
                handle: Some(handle),
            });
        }
        Ok(Self {
            workers,
            completions,
            next_worker: 0,
        })
    }

    fn worker_count(&self) -> usize {
        self.workers.len()
    }

    fn try_submit(&mut self, job: TerrainHorizonCpuCompileJob) -> Result<bool, String> {
        for offset in 0..self.workers.len() {
            let index = (self.next_worker + offset) % self.workers.len();
            let sender = self.workers[index]
                .requests
                .as_ref()
                .ok_or("terrain horizon worker is shutting down")?;
            match sender.try_send(job) {
                Ok(()) => {
                    self.next_worker = (index + 1) % self.workers.len();
                    return Ok(true);
                }
                Err(mpsc::TrySendError::Full(_)) => {}
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err("terrain horizon worker disconnected".to_owned());
                }
            }
        }
        Ok(false)
    }

    fn try_recv(&mut self) -> Result<Option<TerrainHorizonCpuCompileResult>, String> {
        match self.completions.try_recv() {
            Ok(result) => Ok(Some(result)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("terrain horizon completion channel disconnected".to_owned())
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for TerrainHorizonCpuCompiler {
    fn drop(&mut self) {
        for worker in &mut self.workers {
            worker.requests = None;
        }
        for worker in &mut self.workers {
            if let Some(handle) = worker.handle.take() {
                let _ = handle.join();
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainViewportFrameStats {
    pub revision: u64,
    pub requested_spacing: u32,
    pub effective_spacing: u32,
    pub published_spacing: u32,
    pub cpu_published_spacing: u32,
    pub gpu_published_spacing: u32,
    pub view_width_blocks: u32,
    pub view_height_blocks: u32,
    pub level_count: u32,
    pub visible_tile_count: u32,
    pub published_tile_count: u32,
    pub cpu_published_tile_count: u32,
    pub gpu_published_tile_count: u32,
    pub resident_tile_count: u32,
    pub queued_tile_count: u32,
    pub cpu_queued_tile_count: u32,
    pub gpu_queued_tile_count: u32,
    pub pending_readback_count: u32,
    pub cpu_compiled_tiles: u32,
    pub cpu_compiled_tiles_total: u64,
    pub gpu_dispatched_tiles: u32,
    pub gpu_dispatched_tiles_total: u64,
    pub macro_compiled_tiles_total: u64,
    pub request_cpu_compiled_tiles: u32,
    pub request_gpu_dispatched_tiles: u32,
    pub request_macro_compiled_tiles: u32,
    pub request_cache_hit_tiles: u32,
    pub request_cpu_compile_work: TerrainPreviewCompileWork,
    pub request_gpu_compile_work: TerrainPreviewCompileWork,
    pub evicted_tiles_total: u64,
    pub stale_result_count: u64,
    pub sample_count: u32,
    pub vertex_count: u32,
    pub vegetation_summary_tile_count: u32,
    pub cpu_vegetation_summary_tile_count: u32,
    pub gpu_vegetation_summary_tile_count: u32,
    pub vegetation_record_tile_count: u32,
    pub vegetation_aggregated_tile_count: u32,
    pub tree_instance_count: u32,
    pub tree_instance_bytes: u64,
    pub tree_proxy_vertex_count: u32,
    pub vegetation_cell_requests: u64,
    pub vegetation_cell_hits: u64,
    pub vegetation_cell_misses: u64,
    pub retained_vegetation_cells: u32,
    pub reference_bytes: u64,
    pub gpu_sample_bytes: u64,
    pub readback_bytes: u64,
    pub request_readback_bytes: u64,
    pub resident_bytes: u64,
    pub cpu_reference_micros: u64,
    pub cpu_vegetation_micros: u64,
    pub cpu_pack_upload_micros: u64,
    pub request_cpu_reference_micros: u64,
    pub request_cpu_vegetation_micros: u64,
    pub request_cpu_pack_upload_micros: u64,
    pub request_macro_compile_micros: u64,
    pub cpu_coarse_ready: bool,
    pub cpu_target_ready: bool,
    pub gpu_coarse_ready: bool,
    pub gpu_target_ready: bool,
    pub coarse_ready: bool,
    pub target_ready: bool,
    pub cache_enabled: bool,
    pub budget_limited: bool,
    pub needs_redraw: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainViewportCompletedComparison {
    pub revision: u64,
    pub sample_spacing: u32,
    pub tile_count: u32,
    pub comparison: TerrainPreviewComparison,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainViewportExternalCpuRequest {
    pub revision: u64,
    pub tile: TerrainViewportTileId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainHorizonFrameStats {
    pub revision: u64,
    pub allocation_slots: u32,
    pub staging_slots: u32,
    pub normal_halo_radius: u32,
    pub normal_halo_samples_per_tile: u32,
    pub normal_halo_fixed_bytes: u64,
    pub normal_height_fixed_bytes: u64,
    pub ready_slots: u32,
    pub requested_levels: u32,
    pub staged_levels: u32,
    pub committed_levels: u32,
    pub vegetation_committed_levels: u32,
    pub atomic_level_commits: u64,
    pub deferred_transition_attempts: u64,
    pub pending_refills: u32,
    pub dispatched_refills: u32,
    pub dispatched_refills_total: u64,
    pub cpu_compile_workers: u32,
    pub cpu_compile_in_flight: u32,
    pub cpu_compile_submitted_total: u64,
    pub cpu_compile_completed_total: u64,
    pub cpu_compile_micros_total: u64,
    pub cpu_compile_stale_results_total: u64,
    pub drawn_levels: u32,
    pub drawn_tiles: u32,
    pub drawn_tiles_by_level: [u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize],
    pub inner_hole_culled_tiles: u32,
    pub frustum_culled_tiles: u32,
    pub far_culled_tiles: u32,
    pub drawn_tree_tiles: u32,
    pub drawn_tree_instances: u32,
    pub drawn_tree_tiles_by_level: [u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize],
    pub drawn_tree_instances_by_level: [u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize],
    pub drawn_canopy_tiles: u32,
    pub drawn_canopy_cells: u32,
    pub drawn_canopy_vertices: u32,
    pub drawn_canopy_tiles_by_level: [u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize],
    pub drawn_canopy_cells_by_level: [u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize],
    pub revealing_proxy_tiles: u32,
    pub revealing_canopy_tiles: u32,
    pub exact_connector_segments: u32,
    pub exact_connector_vertex_count: u32,
    pub frontier_support_allocated_tiles: u32,
    pub frontier_support_ready_tiles: u32,
    pub frontier_support_pending_tiles: u32,
    pub frontier_support_drawn_tiles: u32,
    pub frontier_support_dispatches: u32,
    pub frontier_support_dispatches_total: u64,
    pub frontier_support_resource_bytes: u64,
    pub frontier_support_vertex_count: u32,
    pub frontier_connector_segments: u32,
    pub frontier_connector_vertex_count: u32,
    pub frontier_connector_bytes: u64,
    pub vertex_count: u32,
    pub vegetation_ready_tiles: u32,
    pub pending_vegetation_tiles: u32,
    pub tree_instance_count: u32,
    pub tree_proxy_suppressed_instances: u32,
    pub tree_proxy_suppressed_records: u32,
    pub tree_proxy_missing_exact_records: u32,
    pub tree_proxy_missing_proxy_records: u32,
    pub tree_proxy_vertex_count: u32,
    pub tree_ownership_generation: u64,
    pub tree_ownership_units: u32,
    pub exact_owned_tree_records: u32,
    pub proxy_owned_tree_records: u32,
    pub fixed_resident_bytes: u64,
    pub exact_connector_bytes: u64,
    pub vegetation_bytes: u64,
    pub resident_bytes: u64,
    pub exact_coverage_mode: TerrainExactCoverageMode,
    pub exact_coverage_generation: u64,
    pub exact_painted_chunks: u32,
    pub exact_coverage_mask_bytes: u64,
    pub exact_transition_preparation_micros: u64,
    pub exact_transition_payload_bytes: u64,
    pub exact_boundary_columns: u32,
    pub exact_boundary_payload_bytes: u64,
    pub frontier: TerrainFrontierPlanReceipt,
    pub frontier_plan_failures: u64,
    pub frontier_topology: TerrainFrontierTopologyReceipt,
    pub frontier_topology_failures: u64,
    pub frontier_admission: TerrainFrontierAdmissionReceipt,
    pub vegetation_service: TerrainHorizonVegetationServiceStats,
    pub finest_sample_spacing: u32,
    pub coarse_ready: bool,
    pub target_ready: bool,
    pub needs_redraw: bool,
    pub residency: TerrainClipmapDiagnostics,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainHorizonVegetationServiceStats {
    pub enabled: bool,
    pub coordinator_state: Option<TerrainVegetationCoordinatorState>,
    pub executor_kind: Option<TerrainVegetationExecutorKind>,
    pub source_fingerprint: u64,
    pub terrain_source_revision: u64,
    pub compiler_source_revision: u64,
    pub vegetation_plan_revision: u64,
    pub product_revision: u32,
    pub record_hash: u64,
    pub family_counts: [u32; 4],
    pub record_count: u32,
    pub product_count: u32,
    pub executor_generation: u32,
    pub source_epoch: u32,
    pub coverage_revision: u64,
    pub desired_tiles: u32,
    pub queued_tiles: u32,
    pub resident_tiles: u32,
    pub in_flight: bool,
    pub submitted_jobs: u64,
    pub completed_jobs: u64,
    pub admitted_products: u64,
    pub source_resets: u64,
    pub transport_failures: u64,
    pub executor_restarts: u64,
    pub job_failures: u64,
    pub stale_completions: u64,
    pub superseded_completions: u64,
    pub submit_full_count: u64,
    pub compile_micros: u64,
    pub cache_cell_requests: u64,
    pub cache_cell_hits: u64,
    pub cache_cell_misses: u64,
    pub cache_retained_cells: u64,
    pub cache_retained_preliminary_candidates: u64,
    pub executor_submitted_jobs: u64,
    pub executor_completed_jobs: u64,
    pub executor_transport_failures: u64,
    pub executor_restart_count: u64,
    pub result_capacity_bytes: u64,
    pub result_high_water_bytes: u64,
    pub result_overflow_count: u64,
    pub copied_result_bytes: u64,
    pub main_decode_micros: u64,
}

struct EncodedTerrainViewportReadbacks {
    readbacks: Vec<EncodedTerrainViewportReadback>,
}

struct EncodedTerrainViewportReadback {
    cache_epoch: u64,
    tile: TerrainViewportTileId,
    readback_buffer: wgpu::Buffer,
    byte_len: u64,
}

struct PendingTerrainViewportReadback {
    cache_epoch: u64,
    tile: TerrainViewportTileId,
    readback_buffer: wgpu::Buffer,
    byte_len: u64,
    receiver: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
}

struct TerrainViewportDepthTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl TerrainViewportDepthTarget {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_terrain_viewport_depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TERRAIN_PREVIEW_DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            width,
            height,
        }
    }
}

struct TerrainPreviewMaterialResources {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    _table_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

struct TerrainExactCoverageResources {
    _uniform_buffer: wgpu::Buffer,
    _mask_buffer: wgpu::Buffer,
    _transition_texture: wgpu::Texture,
    _transition_view: wgpu::TextureView,
    _transition_sampler: wgpu::Sampler,
    _boundary_texture: wgpu::Texture,
    _boundary_view: wgpu::TextureView,
    _frontier_support_lookup_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    mask: TerrainExactCoverageMask,
    transition: TerrainExactTransitionField,
    boundary: TerrainExactBoundaryProfile,
    connector_instances: Vec<TerrainExactConnectorInstance>,
    mode: TerrainExactCoverageMode,
    uploaded_mode: TerrainExactCoverageMode,
    frontier_support_tiles: BTreeSet<super::TerrainFrontierFineTileKey>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TerrainExactConnectorInstance {
    cell_world_x: i32,
    cell_world_z: i32,
    side: u32,
}

impl TerrainExactConnectorInstance {
    fn bytes(self) -> [u8; TERRAIN_EXACT_CONNECTOR_INSTANCE_BYTES as usize] {
        let mut bytes = [0; TERRAIN_EXACT_CONNECTOR_INSTANCE_BYTES as usize];
        bytes[0..4].copy_from_slice(&self.cell_world_x.to_ne_bytes());
        bytes[4..8].copy_from_slice(&self.cell_world_z.to_ne_bytes());
        bytes[8..12].copy_from_slice(&self.side.to_ne_bytes());
        bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TerrainFrontierConnectorInstance {
    geometry: TerrainExactConnectorInstance,
    owner_sample_spacing: u32,
    owner_tile_x: i32,
    owner_tile_z: i32,
    fallback: bool,
    water: bool,
    outer: bool,
}

impl TerrainFrontierConnectorInstance {
    fn owned_by(&self, request: ValidatedTerrainPreviewRequest) -> bool {
        let footprint = i32::try_from(request.footprint_blocks()).unwrap_or(i32::MAX);
        self.owner_sample_spacing == request.request().sample_spacing
            && self.owner_tile_x == request.min_x().div_euclid(footprint)
            && self.owner_tile_z == request.min_z().div_euclid(footprint)
    }

    fn bytes(self) -> [u8; TERRAIN_EXACT_CONNECTOR_INSTANCE_BYTES as usize] {
        self.geometry.bytes()
    }
}

struct TerrainFrontierSupportGpuTile {
    key: TerrainFrontierFineTileKey,
    tile: TerrainViewportGpuTile,
    ready: bool,
    outer_edge_flags: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerrainFrontierSupportGpuIdentity {
    seed: i64,
    content_stage: TerrainPreviewContentStage,
    exact_generation: u64,
    presentation: super::TerrainFrontierPresentationIdentity,
    support_pool_capacity: u32,
    selected_tiles: BTreeSet<TerrainFrontierFineTileKey>,
}

struct TerrainFrontierSupportGpu {
    identity: TerrainFrontierSupportGpuIdentity,
    tiles: Vec<TerrainFrontierSupportGpuTile>,
    connector_instances: Vec<TerrainFrontierConnectorInstance>,
    committed: bool,
    dispatched_total: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TerrainFrontierSupportLookup {
    origin_tile_x: i32,
    origin_tile_z: i32,
    width: u32,
    height: u32,
    rows: [u32; TERRAIN_FRONTIER_SUPPORT_LOOKUP_MAX_TILES_PER_AXIS],
}

impl TerrainFrontierSupportLookup {
    fn from_tiles(tiles: &BTreeSet<TerrainFrontierFineTileKey>) -> Result<Self, String> {
        let Some(first) = tiles.first() else {
            return Ok(Self::default());
        };
        let mut min_x = first.tile_x;
        let mut max_x = first.tile_x;
        let mut min_z = first.tile_z;
        let mut max_z = first.tile_z;
        for tile in tiles {
            min_x = min_x.min(tile.tile_x);
            max_x = max_x.max(tile.tile_x);
            min_z = min_z.min(tile.tile_z);
            max_z = max_z.max(tile.tile_z);
        }
        let width = max_x
            .checked_sub(min_x)
            .and_then(|span| span.checked_add(1))
            .ok_or("frontier support lookup width overflow")?;
        let height = max_z
            .checked_sub(min_z)
            .and_then(|span| span.checked_add(1))
            .ok_or("frontier support lookup height overflow")?;
        if width > TERRAIN_FRONTIER_SUPPORT_LOOKUP_MAX_TILES_PER_AXIS as i64
            || height > TERRAIN_FRONTIER_SUPPORT_LOOKUP_MAX_TILES_PER_AXIS as i64
        {
            return Err(format!(
                "frontier support lookup {}x{} exceeds {}x{} tiles",
                width,
                height,
                TERRAIN_FRONTIER_SUPPORT_LOOKUP_MAX_TILES_PER_AXIS,
                TERRAIN_FRONTIER_SUPPORT_LOOKUP_MAX_TILES_PER_AXIS,
            ));
        }
        let origin_tile_x = i32::try_from(min_x)
            .map_err(|_| "frontier support lookup X origin exceeds shader coordinates")?;
        let origin_tile_z = i32::try_from(min_z)
            .map_err(|_| "frontier support lookup Z origin exceeds shader coordinates")?;
        let mut lookup = Self {
            origin_tile_x,
            origin_tile_z,
            width: u32::try_from(width).expect("bounded frontier lookup width fits u32"),
            height: u32::try_from(height).expect("bounded frontier lookup height fits u32"),
            rows: [0; TERRAIN_FRONTIER_SUPPORT_LOOKUP_MAX_TILES_PER_AXIS],
        };
        for tile in tiles {
            let local_x = usize::try_from(tile.tile_x - min_x)
                .expect("bounded frontier lookup X offset fits usize");
            let local_z = usize::try_from(tile.tile_z - min_z)
                .expect("bounded frontier lookup Z offset fits usize");
            lookup.rows[local_z] |= 1_u32 << local_x;
        }
        Ok(lookup)
    }

    fn contains(&self, tile: TerrainFrontierFineTileKey) -> bool {
        let local_x = tile.tile_x - i64::from(self.origin_tile_x);
        let local_z = tile.tile_z - i64::from(self.origin_tile_z);
        if local_x < 0
            || local_z < 0
            || local_x >= i64::from(self.width)
            || local_z >= i64::from(self.height)
        {
            return false;
        }
        self.rows[local_z as usize] & (1_u32 << local_x as u32) != 0
    }

    fn selected_count(&self) -> u32 {
        self.rows.iter().map(|row| row.count_ones()).sum()
    }

    fn bytes(&self) -> [u8; TERRAIN_FRONTIER_SUPPORT_LOOKUP_BUFFER_BYTES as usize] {
        let mut bytes = [0; TERRAIN_FRONTIER_SUPPORT_LOOKUP_BUFFER_BYTES as usize];
        bytes[0..4].copy_from_slice(&self.origin_tile_x.to_ne_bytes());
        bytes[4..8].copy_from_slice(&self.origin_tile_z.to_ne_bytes());
        bytes[8..12].copy_from_slice(&(self.width as i32).to_ne_bytes());
        bytes[12..16].copy_from_slice(&(self.height as i32).to_ne_bytes());
        for (index, row) in self.rows.iter().enumerate() {
            let start = 16 + index * size_of::<u32>();
            bytes[start..start + 4].copy_from_slice(&row.to_ne_bytes());
        }
        bytes
    }
}

impl TerrainExactCoverageResources {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> Result<Self, String> {
        let snapshot = ExactPaintedCoverageSnapshot::new(
            TerrainCompositionSourceIdentity::new(TerrainPreviewProfile::McloneOverworldV1, 0),
            1,
            [],
        )?;
        let mask = snapshot.packed_mask()?;
        let transition = TerrainExactTransitionField::from_coverage(&snapshot)?;
        let boundary = TerrainExactBoundaryProfile::empty(&snapshot)?;
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_exact_coverage_uniform"),
            size: TERRAIN_EXACT_COVERAGE_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mask_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_exact_coverage_mask"),
            size: TERRAIN_EXACT_COVERAGE_MASK_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let transition_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_terrain_exact_transition_field"),
            size: wgpu::Extent3d {
                width: TERRAIN_EXACT_TRANSITION_MAX_TEXELS_PER_AXIS,
                height: TERRAIN_EXACT_TRANSITION_MAX_TEXELS_PER_AXIS,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let transition_view =
            transition_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let transition_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mclone_terrain_exact_transition_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let boundary_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_terrain_exact_boundary_profile"),
            size: wgpu::Extent3d {
                width: TERRAIN_EXACT_BOUNDARY_MAX_BLOCKS_PER_AXIS,
                height: TERRAIN_EXACT_BOUNDARY_MAX_BLOCKS_PER_AXIS,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let boundary_view = boundary_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let frontier_support_lookup_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_frontier_support_lookup"),
            size: TERRAIN_FRONTIER_SUPPORT_LOOKUP_BUFFER_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &uniform_buffer,
            0,
            &terrain_exact_uniform_bytes(
                &mask,
                &transition,
                &boundary,
                TerrainExactCoverageMode::Disabled,
            ),
        );
        queue.write_buffer(&mask_buffer, 0, &mask.word_bytes());
        queue.write_buffer(
            &frontier_support_lookup_buffer,
            0,
            &TerrainFrontierSupportLookup::default().bytes(),
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_exact_coverage_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: mask_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&transition_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&transition_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&boundary_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: frontier_support_lookup_buffer.as_entire_binding(),
                },
            ],
        });
        Ok(Self {
            _uniform_buffer: uniform_buffer,
            _mask_buffer: mask_buffer,
            _transition_texture: transition_texture,
            _transition_view: transition_view,
            _transition_sampler: transition_sampler,
            _boundary_texture: boundary_texture,
            _boundary_view: boundary_view,
            _frontier_support_lookup_buffer: frontier_support_lookup_buffer,
            bind_group,
            mask,
            transition,
            boundary,
            connector_instances: Vec::new(),
            mode: TerrainExactCoverageMode::Disabled,
            uploaded_mode: TerrainExactCoverageMode::Disabled,
            frontier_support_tiles: BTreeSet::new(),
        })
    }

    fn set_snapshot(
        &mut self,
        queue: &wgpu::Queue,
        snapshot: &ExactPaintedCoverageSnapshot,
        transition: &TerrainExactTransitionField,
        boundary: &TerrainExactBoundaryProfile,
        mode: TerrainExactCoverageMode,
    ) -> Result<(), String> {
        if transition.source() != snapshot.source()
            || transition.generation() != snapshot.generation()
        {
            return Err("exact transition field does not match its coverage generation".to_owned());
        }
        if boundary.source() != snapshot.source() || boundary.generation() != snapshot.generation()
        {
            return Err("exact boundary profile does not match its coverage generation".to_owned());
        }
        if self.transition.source() == transition.source()
            && self.transition.generation() == transition.generation()
            && self.boundary == *boundary
            && self.mode == mode
        {
            return Ok(());
        }
        let mask = snapshot.packed_mask()?;
        queue.write_buffer(&self._mask_buffer, 0, &mask.word_bytes());
        let [width, height] = transition.dimensions();
        if width > 0 && height > 0 {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self._transition_texture,
                    mip_level: 0,
                    origin: Default::default(),
                    aspect: Default::default(),
                },
                transition.weights(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
        let [boundary_width, boundary_height] = boundary.dimensions();
        if boundary_width > 0 && boundary_height > 0 {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self._boundary_texture,
                    mip_level: 0,
                    origin: Default::default(),
                    aspect: Default::default(),
                },
                &boundary.packed_bytes(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(boundary_width * size_of::<u32>() as u32),
                    rows_per_image: Some(boundary_height),
                },
                wgpu::Extent3d {
                    width: boundary_width,
                    height: boundary_height,
                    depth_or_array_layers: 1,
                },
            );
        }
        queue.write_buffer(
            &self._uniform_buffer,
            0,
            &terrain_exact_uniform_bytes(&mask, transition, boundary, mode),
        );
        self.mask = mask;
        self.transition = transition.clone();
        self.boundary = boundary.clone();
        self.connector_instances = terrain_exact_connector_instances(&self.mask, boundary);
        self.mode = mode;
        self.uploaded_mode = mode;
        Ok(())
    }

    fn disable(&mut self) {
        self.mode = TerrainExactCoverageMode::Disabled;
    }

    fn set_frontier_support_tiles(
        &mut self,
        queue: &wgpu::Queue,
        tiles: &BTreeSet<super::TerrainFrontierFineTileKey>,
    ) -> Result<(), String> {
        if self.frontier_support_tiles == *tiles {
            return Ok(());
        }
        if tiles.len() > super::TERRAIN_FRONTIER_FINE_TILE_CAPACITY as usize {
            return Err("frontier support tiles exceed the fixed resource pool".to_owned());
        }
        let lookup = TerrainFrontierSupportLookup::from_tiles(tiles)?;
        debug_assert_eq!(lookup.selected_count(), tiles.len() as u32);
        debug_assert!(tiles.iter().all(|tile| lookup.contains(*tile)));
        queue.write_buffer(&self._frontier_support_lookup_buffer, 0, &lookup.bytes());
        self.frontier_support_tiles = tiles.clone();
        Ok(())
    }

    fn sync(&mut self, queue: &wgpu::Queue) {
        if self.uploaded_mode != self.mode {
            queue.write_buffer(
                &self._uniform_buffer,
                0,
                &terrain_exact_uniform_bytes(
                    &self.mask,
                    &self.transition,
                    &self.boundary,
                    self.mode,
                ),
            );
            self.uploaded_mode = self.mode;
        }
    }
}

fn terrain_exact_connector_instances(
    mask: &TerrainExactCoverageMask,
    boundary: &TerrainExactBoundaryProfile,
) -> Vec<TerrainExactConnectorInstance> {
    let [origin_x, origin_z] = boundary.origin_blocks();
    let [width, height] = boundary.dimensions();
    let mut instances = Vec::new();
    for local_z in 0..height {
        for local_x in 0..width {
            let packed = boundary.packed()[(local_z * width + local_x) as usize];
            if packed & 0x8000_0000 == 0 || packed & 0x0100_0000 != 0 {
                continue;
            }
            let Some(world_x) = origin_x.checked_add(local_x as i32) else {
                continue;
            };
            let Some(world_z) = origin_z.checked_add(local_z as i32) else {
                continue;
            };
            for (dx, dz, side) in [(1, 0, 0), (-1, 0, 1), (0, 1, 2), (0, -1, 3)] {
                let Some(cell_world_x) = world_x.checked_add(dx) else {
                    continue;
                };
                let Some(cell_world_z) = world_z.checked_add(dz) else {
                    continue;
                };
                if !mask.contains(ChunkPos::from_block_coords(cell_world_x, cell_world_z)) {
                    instances.push(TerrainExactConnectorInstance {
                        cell_world_x,
                        cell_world_z,
                        side,
                    });
                }
            }
        }
    }
    instances
}

fn terrain_frontier_connector_instances(
    proof: &TerrainFrontierTopology,
) -> Result<Vec<TerrainFrontierConnectorInstance>, String> {
    if proof.receipt().state != TerrainFrontierTopologyState::Complete {
        return Err("frontier proof connectors require a complete topology".to_owned());
    }
    let mut instances = Vec::new();
    for segment in proof.segments() {
        let (fallback, water) = match segment.closure {
            TerrainFrontierClosure::PreferredSolidConnector => (false, false),
            TerrainFrontierClosure::ResolutionAwareSolidConnector => (true, false),
            TerrainFrontierClosure::PreferredWaterCurtain => (false, true),
            TerrainFrontierClosure::ResolutionAwareWaterCurtain => (true, true),
            TerrainFrontierClosure::WorldBoundary => continue,
            TerrainFrontierClosure::UnsupportedExactProfile
            | TerrainFrontierClosure::UnsupportedProceduralCoverage => {
                return Err("complete frontier topology contains an unsupported segment".to_owned());
            }
        };
        let owner_sample_spacing = if fallback {
            segment
                .procedural_sample_spacing
                .ok_or("frontier fallback segment lacks a procedural spacing")?
        } else {
            1
        };
        let owner_footprint = i64::from(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS)
            .checked_mul(i64::from(owner_sample_spacing))
            .ok_or("frontier connector owner footprint overflow")?;
        let (owner_tile_x, owner_tile_z) = if fallback {
            (
                segment.procedural_block[0].div_euclid(owner_footprint),
                segment.procedural_block[1].div_euclid(owner_footprint),
            )
        } else {
            (segment.support_tile.tile_x, segment.support_tile.tile_z)
        };
        let side = match segment.direction {
            TerrainFrontierDirection::West => 1,
            TerrainFrontierDirection::East => 0,
            TerrainFrontierDirection::North => 3,
            TerrainFrontierDirection::South => 2,
        } | TERRAIN_FRONTIER_CONNECTOR_FLAG
            | u32::from(water) * TERRAIN_FRONTIER_CONNECTOR_WATER_FLAG
            | u32::from(fallback) * TERRAIN_FRONTIER_CONNECTOR_FALLBACK_FLAG;
        instances.push(TerrainFrontierConnectorInstance {
            geometry: TerrainExactConnectorInstance {
                cell_world_x: i32::try_from(segment.procedural_block[0])
                    .map_err(|_| "frontier connector X exceeds shader coordinates")?,
                cell_world_z: i32::try_from(segment.procedural_block[1])
                    .map_err(|_| "frontier connector Z exceeds shader coordinates")?,
                side,
            },
            owner_sample_spacing,
            owner_tile_x: i32::try_from(owner_tile_x)
                .map_err(|_| "frontier connector owner tile X exceeds shader coordinates")?,
            owner_tile_z: i32::try_from(owner_tile_z)
                .map_err(|_| "frontier connector owner tile Z exceeds shader coordinates")?,
            fallback,
            water,
            outer: false,
        });
    }

    let tile_blocks = i64::from(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS);
    for edge in proof.outer_edges() {
        let min_x = edge.tile.tile_x.saturating_mul(tile_blocks);
        let min_z = edge.tile.tile_z.saturating_mul(tile_blocks);
        for offset in 0..tile_blocks {
            let (cell_x, cell_z, side) = match edge.direction {
                TerrainFrontierDirection::West => (min_x, min_z + offset, 0),
                TerrainFrontierDirection::East => (min_x + tile_blocks - 1, min_z + offset, 1),
                TerrainFrontierDirection::North => (min_x + offset, min_z, 2),
                TerrainFrontierDirection::South => (min_x + offset, min_z + tile_blocks - 1, 3),
            };
            instances.push(TerrainFrontierConnectorInstance {
                geometry: TerrainExactConnectorInstance {
                    cell_world_x: i32::try_from(cell_x)
                        .map_err(|_| "frontier outer connector X exceeds shader coordinates")?,
                    cell_world_z: i32::try_from(cell_z)
                        .map_err(|_| "frontier outer connector Z exceeds shader coordinates")?,
                    side: side
                        | TERRAIN_FRONTIER_CONNECTOR_FLAG
                        | TERRAIN_FRONTIER_CONNECTOR_OUTER_FLAG,
                },
                owner_sample_spacing: 1,
                owner_tile_x: i32::try_from(edge.tile.tile_x)
                    .map_err(|_| "frontier outer owner tile X exceeds shader coordinates")?,
                owner_tile_z: i32::try_from(edge.tile.tile_z)
                    .map_err(|_| "frontier outer owner tile Z exceeds shader coordinates")?,
                fallback: false,
                water: false,
                outer: true,
            });
        }
    }
    Ok(instances)
}

fn terrain_exact_uniform_bytes(
    mask: &TerrainExactCoverageMask,
    transition: &TerrainExactTransitionField,
    boundary: &TerrainExactBoundaryProfile,
    mode: TerrainExactCoverageMode,
) -> [u8; TERRAIN_EXACT_COVERAGE_UNIFORM_BYTES as usize] {
    let mut bytes = [0; TERRAIN_EXACT_COVERAGE_UNIFORM_BYTES as usize];
    bytes[..32].copy_from_slice(&mask.uniform_bytes(mode));
    let [transition_origin_x, transition_origin_z] = transition.origin_blocks();
    let [transition_width, transition_height] = transition.dimensions();
    for (index, value) in [
        transition_origin_x,
        transition_origin_z,
        transition_width as i32,
        transition_height as i32,
    ]
    .into_iter()
    .enumerate()
    {
        let start = 32 + index * size_of::<i32>();
        bytes[start..start + size_of::<i32>()].copy_from_slice(&value.to_ne_bytes());
    }
    let [boundary_origin_x, boundary_origin_z] = boundary.origin_blocks();
    let [boundary_width, boundary_height] = boundary.dimensions();
    for (index, value) in [
        boundary_origin_x,
        boundary_origin_z,
        boundary_width as i32,
        boundary_height as i32,
    ]
    .into_iter()
    .enumerate()
    {
        let start = 48 + index * size_of::<i32>();
        bytes[start..start + size_of::<i32>()].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

impl TerrainPreviewMaterialResources {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        atlas: TerrainPreviewMaterialAtlas<'_>,
    ) -> Result<Self, String> {
        let width = atlas.width.max(1);
        let height = atlas.height.max(1);
        let expected_len = width as usize * height as usize * 4;
        if atlas.rgba.len() != expected_len {
            return Err(format!(
                "terrain preview material atlas has {} bytes; expected {expected_len} for \
                 {width}x{height} RGBA",
                atlas.rgba.len()
            ));
        }
        let mip_levels = generate_material_mips(width, height, atlas.rgba, 5);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_terrain_preview_material_atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (mip_level, mip) in mip_levels.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip_level as u32,
                    origin: Default::default(),
                    aspect: Default::default(),
                },
                &mip.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(mip.width * 4),
                    rows_per_image: Some(mip.height),
                },
                wgpu::Extent3d {
                    width: mip.width,
                    height: mip.height,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mclone_terrain_preview_material_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_max_clamp: mip_levels.len().saturating_sub(1) as f32,
            ..Default::default()
        });
        let table_bytes = atlas.material_table.uniform_bytes();
        let table_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_preview_material_table"),
            size: table_bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&table_buffer, 0, &table_bytes);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_preview_material_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: table_buffer.as_entire_binding(),
                },
            ],
        });
        Ok(Self {
            _texture: texture,
            _view: view,
            _sampler: sampler,
            _table_buffer: table_buffer,
            bind_group,
        })
    }
}

struct TerrainPreviewMaterialMip {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn generate_material_mips(
    width: u32,
    height: u32,
    rgba: &[u8],
    max_level_count: usize,
) -> Vec<TerrainPreviewMaterialMip> {
    let mut levels = vec![TerrainPreviewMaterialMip {
        width,
        height,
        rgba: rgba.to_vec(),
    }];
    while levels.len() < max_level_count
        && levels
            .last()
            .is_some_and(|level| level.width > 1 || level.height > 1)
    {
        let previous = levels.last().expect("base material mip exists");
        let next_width = (previous.width / 2).max(1);
        let next_height = (previous.height / 2).max(1);
        let mut next_rgba = vec![0_u8; next_width as usize * next_height as usize * 4];
        for y in 0..next_height {
            for x in 0..next_width {
                let mut sum = [0_u32; 4];
                let mut count = 0_u32;
                for source_y in (y * 2)..(y * 2 + 2).min(previous.height) {
                    for source_x in (x * 2)..(x * 2 + 2).min(previous.width) {
                        let source = ((source_y * previous.width + source_x) * 4) as usize;
                        for channel in 0..4 {
                            sum[channel] += u32::from(previous.rgba[source + channel]);
                        }
                        count += 1;
                    }
                }
                let destination = ((y * next_width + x) * 4) as usize;
                for channel in 0..4 {
                    next_rgba[destination + channel] = (sum[channel] / count) as u8;
                }
            }
        }
        levels.push(TerrainPreviewMaterialMip {
            width: next_width,
            height: next_height,
            rgba: next_rgba,
        });
    }
    levels
}

struct TerrainViewportGpuTile {
    request: ValidatedTerrainPreviewRequest,
    reference: Option<TerrainPreviewReferenceGrid>,
    vegetation: Option<TerrainPreviewVegetationProduct>,
    gpu_samples: Option<Vec<TerrainPreviewSample>>,
    gpu_submitted: bool,
    uniform_buffer: wgpu::Buffer,
    tree_uniform_buffer: wgpu::Buffer,
    gpu_sample_buffer: wgpu::Buffer,
    _normal_height_buffer: wgpu::Buffer,
    reference_sample_buffer: Option<wgpu::Buffer>,
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    tree_render_bind_group: wgpu::BindGroup,
    tree_instance_buffer: Option<wgpu::Buffer>,
    tree_instance_count: u32,
    tree_suppressed_instance_count: u32,
    tree_instance_bytes: u64,
    proxy_reveal: f32,
    canopy_reveal: f32,
    exact_connector_buffer: Option<wgpu::Buffer>,
    exact_connector_instance_count: u32,
    exact_connector_instance_bytes: u64,
    exact_connector_generation: u64,
    exact_connector_request: Option<TerrainPreviewRequest>,
    frontier_connector_buffer: Option<wgpu::Buffer>,
    frontier_connector_instance_count: u32,
    frontier_connector_instance_bytes: u64,
    frontier_connector_identity: Option<(
        u64,
        super::TerrainFrontierPresentationIdentity,
        TerrainPreviewRequest,
    )>,
    last_used: u64,
}

impl TerrainViewportGpuTile {
    fn new(
        device: &wgpu::Device,
        compute_layout: &wgpu::BindGroupLayout,
        render_layout: &wgpu::BindGroupLayout,
        sample_byte_len: u64,
        normal_height_buffer: wgpu::Buffer,
        tile_id: TerrainViewportTileId,
    ) -> Result<Self, String> {
        Self::new_with_reference_buffer(
            device,
            compute_layout,
            render_layout,
            sample_byte_len,
            normal_height_buffer,
            tile_id,
            true,
        )
    }

    fn new_gpu_only(
        device: &wgpu::Device,
        compute_layout: &wgpu::BindGroupLayout,
        render_layout: &wgpu::BindGroupLayout,
        sample_byte_len: u64,
        normal_height_byte_len: u64,
        tile_id: TerrainViewportTileId,
    ) -> Result<Self, String> {
        let normal_height_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_horizon_normal_heights"),
            size: normal_height_byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self::new_with_reference_buffer(
            device,
            compute_layout,
            render_layout,
            sample_byte_len,
            normal_height_buffer,
            tile_id,
            false,
        )
    }

    fn new_with_reference_buffer(
        device: &wgpu::Device,
        compute_layout: &wgpu::BindGroupLayout,
        render_layout: &wgpu::BindGroupLayout,
        sample_byte_len: u64,
        normal_height_buffer: wgpu::Buffer,
        tile_id: TerrainViewportTileId,
        allocate_reference_buffer: bool,
    ) -> Result<Self, String> {
        let request = tile_id.preview_request().validate()?;
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_viewport_tile_uniforms"),
            size: TERRAIN_PREVIEW_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let tree_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_viewport_tree_uniforms"),
            size: TERRAIN_PREVIEW_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let gpu_sample_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_viewport_gpu_samples"),
            size: sample_byte_len,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let reference_sample_buffer = allocate_reference_buffer.then(|| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_terrain_viewport_reference_samples"),
                size: sample_byte_len,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_viewport_compute_bind_group"),
            layout: compute_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu_sample_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: normal_height_buffer.as_entire_binding(),
                },
            ],
        });
        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_viewport_render_bind_group"),
            layout: render_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu_sample_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: reference_sample_buffer
                        .as_ref()
                        .unwrap_or(&gpu_sample_buffer)
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: normal_height_buffer.as_entire_binding(),
                },
            ],
        });
        let tree_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mclone_terrain_viewport_tree_render_bind_group"),
            layout: render_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: tree_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu_sample_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: reference_sample_buffer
                        .as_ref()
                        .unwrap_or(&gpu_sample_buffer)
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: normal_height_buffer.as_entire_binding(),
                },
            ],
        });
        Ok(Self {
            request,
            reference: None,
            vegetation: None,
            gpu_samples: None,
            gpu_submitted: false,
            uniform_buffer,
            tree_uniform_buffer,
            gpu_sample_buffer,
            _normal_height_buffer: normal_height_buffer,
            reference_sample_buffer,
            compute_bind_group,
            render_bind_group,
            tree_render_bind_group,
            tree_instance_buffer: None,
            tree_instance_count: 0,
            tree_suppressed_instance_count: 0,
            tree_instance_bytes: 0,
            proxy_reveal: 1.0,
            canopy_reveal: 1.0,
            exact_connector_buffer: None,
            exact_connector_instance_count: 0,
            exact_connector_instance_bytes: 0,
            exact_connector_generation: 0,
            exact_connector_request: None,
            frontier_connector_buffer: None,
            frontier_connector_instance_count: 0,
            frontier_connector_instance_bytes: 0,
            frontier_connector_identity: None,
            last_used: 0,
        })
    }

    fn upload_reference(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sample_byte_len: u64,
        reference: TerrainPreviewReferenceGrid,
        vegetation: TerrainPreviewVegetationProduct,
    ) -> Result<(), String> {
        let reference_bytes = reference.packed_bytes();
        if reference_bytes.len() as u64 != sample_byte_len {
            return Err(format!(
                "terrain viewport reference upload is {} bytes, expected {}",
                reference_bytes.len(),
                sample_byte_len
            ));
        }
        let reference_sample_buffer = self
            .reference_sample_buffer
            .as_ref()
            .ok_or("terrain viewport reference upload requires a reference buffer")?;
        queue.write_buffer(reference_sample_buffer, 0, &reference_bytes);
        self.upload_vegetation(device, queue, vegetation)?;
        self.reference = Some(reference);
        Ok(())
    }

    fn upload_horizon_reference(
        &mut self,
        queue: &wgpu::Queue,
        sample_byte_len: u64,
        reference: TerrainPreviewReferenceGrid,
        height_halo: &[f32],
    ) -> Result<(), String> {
        let reference_bytes = reference.packed_bytes();
        if reference_bytes.len() as u64 != sample_byte_len {
            return Err(format!(
                "terrain horizon reference upload is {} bytes, expected {sample_byte_len}",
                reference_bytes.len()
            ));
        }
        let expected_height_count = usize::try_from(terrain_horizon_samples_per_axis())
            .map_err(|_| "terrain horizon halo axis exceeds usize")?
            .checked_pow(2)
            .ok_or("terrain horizon halo sample count overflow")?;
        if height_halo.len() != expected_height_count {
            return Err(format!(
                "terrain horizon height halo has {} samples, expected {expected_height_count}",
                height_halo.len()
            ));
        }
        let mut height_bytes = Vec::with_capacity(height_halo.len() * size_of::<f32>());
        for height in height_halo {
            height_bytes.extend_from_slice(&height.to_le_bytes());
        }
        queue.write_buffer(&self.gpu_sample_buffer, 0, &reference_bytes);
        queue.write_buffer(&self._normal_height_buffer, 0, &height_bytes);
        self.reference = Some(reference);
        self.gpu_submitted = true;
        Ok(())
    }

    fn upload_vegetation(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vegetation: TerrainPreviewVegetationProduct,
    ) -> Result<(), String> {
        self.upload_vegetation_filtered(device, queue, vegetation, &BTreeSet::new())
    }

    fn upload_vegetation_filtered(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vegetation: TerrainPreviewVegetationProduct,
        exact_owned_tree_ids: &BTreeSet<McloneTreeOccurrenceId>,
    ) -> Result<(), String> {
        let (tree_bytes, tree_instance_count, tree_suppressed_instance_count) =
            tree_instance_bytes_filtered(&vegetation, exact_owned_tree_ids)?;
        self.install_tree_instances(
            device,
            queue,
            tree_bytes,
            tree_instance_count,
            tree_suppressed_instance_count,
        );
        self.vegetation = Some(vegetation);
        Ok(())
    }

    fn advance_forest_reveal(&mut self, elapsed_seconds: f32) {
        self.proxy_reveal = advance_forest_reveal(self.proxy_reveal, elapsed_seconds);
        self.canopy_reveal = advance_forest_reveal(self.canopy_reveal, elapsed_seconds);
    }

    fn reset_proxy_reveal(&mut self) {
        self.proxy_reveal = 0.0;
    }

    fn reset_canopy_reveal(&mut self) {
        self.canopy_reveal = 0.0;
    }

    fn refresh_tree_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        exact_owned_tree_ids: &BTreeSet<McloneTreeOccurrenceId>,
    ) -> Result<(), String> {
        let Some(vegetation) = self.vegetation.as_ref() else {
            return Ok(());
        };
        let (tree_bytes, tree_instance_count, tree_suppressed_instance_count) =
            tree_instance_bytes_filtered(vegetation, exact_owned_tree_ids)?;
        self.install_tree_instances(
            device,
            queue,
            tree_bytes,
            tree_instance_count,
            tree_suppressed_instance_count,
        );
        Ok(())
    }

    fn install_tree_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tree_bytes: Vec<u8>,
        tree_instance_count: u32,
        tree_suppressed_instance_count: u32,
    ) {
        self.tree_instance_buffer = if tree_bytes.is_empty() {
            None
        } else {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_terrain_viewport_tree_instances"),
                size: tree_bytes.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, &tree_bytes);
            Some(buffer)
        };
        self.tree_instance_count = tree_instance_count;
        self.tree_suppressed_instance_count = tree_suppressed_instance_count;
        self.tree_instance_bytes = tree_bytes.len() as u64;
    }

    fn clear_vegetation(&mut self) {
        self.vegetation = None;
        self.tree_instance_buffer = None;
        self.tree_instance_count = 0;
        self.tree_suppressed_instance_count = 0;
        self.tree_instance_bytes = 0;
        self.reset_proxy_reveal();
    }

    fn refresh_exact_connectors(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        generation: u64,
        instances: &[TerrainExactConnectorInstance],
    ) {
        let request = self.request.request();
        if self.exact_connector_generation == generation
            && self.exact_connector_request == Some(request)
        {
            return;
        }
        let min_x = self.request.min_x();
        let min_z = self.request.min_z();
        let max_x = min_x.saturating_add_unsigned(self.request.footprint_blocks());
        let max_z = min_z.saturating_add_unsigned(self.request.footprint_blocks());
        let selected = if request.sample_spacing == 1 {
            instances
                .iter()
                .copied()
                .filter(|instance| {
                    instance.cell_world_x >= min_x
                        && instance.cell_world_x < max_x
                        && instance.cell_world_z >= min_z
                        && instance.cell_world_z < max_z
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let bytes = selected
            .iter()
            .flat_map(|instance| instance.bytes())
            .collect::<Vec<_>>();
        self.exact_connector_buffer = if bytes.is_empty() {
            None
        } else {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_terrain_exact_connector_instances"),
                size: bytes.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, &bytes);
            Some(buffer)
        };
        self.exact_connector_instance_count = selected.len().try_into().unwrap_or(u32::MAX);
        self.exact_connector_instance_bytes = bytes.len() as u64;
        self.exact_connector_generation = generation;
        self.exact_connector_request = Some(request);
    }

    fn clear_exact_connectors(&mut self) {
        self.exact_connector_buffer = None;
        self.exact_connector_instance_count = 0;
        self.exact_connector_instance_bytes = 0;
        self.exact_connector_generation = 0;
        self.exact_connector_request = None;
    }

    fn refresh_frontier_connectors(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        exact_generation: u64,
        presentation: super::TerrainFrontierPresentationIdentity,
        instances: &[TerrainFrontierConnectorInstance],
    ) {
        let request = self.request.request();
        let identity = (exact_generation, presentation, request);
        if self.frontier_connector_identity == Some(identity) {
            return;
        }
        let selected = instances
            .iter()
            .copied()
            .filter(|instance| instance.owned_by(self.request))
            .collect::<Vec<_>>();
        let bytes = selected
            .iter()
            .flat_map(|instance| instance.bytes())
            .collect::<Vec<_>>();
        self.frontier_connector_buffer = if bytes.is_empty() {
            None
        } else {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mclone_terrain_frontier_connector_instances"),
                size: bytes.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, &bytes);
            Some(buffer)
        };
        self.frontier_connector_instance_count = selected.len().try_into().unwrap_or(u32::MAX);
        self.frontier_connector_instance_bytes = bytes.len() as u64;
        self.frontier_connector_identity = Some(identity);
    }

    fn clear_frontier_connectors(&mut self) {
        self.frontier_connector_buffer = None;
        self.frontier_connector_instance_count = 0;
        self.frontier_connector_instance_bytes = 0;
        self.frontier_connector_identity = None;
    }

    fn upload_macro(
        &mut self,
        queue: &wgpu::Queue,
        sample_byte_len: u64,
        macro_grid: TerrainPreviewReferenceGrid,
    ) -> Result<(), String> {
        let bytes = macro_grid.packed_bytes();
        if bytes.len() as u64 != sample_byte_len {
            return Err(format!(
                "terrain viewport macro upload is {} bytes, expected {}",
                bytes.len(),
                sample_byte_len
            ));
        }
        queue.write_buffer(&self.gpu_sample_buffer, 0, &bytes);
        self.gpu_samples = Some(macro_grid.samples().to_vec());
        self.gpu_submitted = true;
        Ok(())
    }
}

pub struct TerrainViewportRenderer {
    sample_count_per_tile: u32,
    sample_byte_len: u64,
    normal_height_scratch_buffer: wgpu::Buffer,
    compute_layout: wgpu::BindGroupLayout,
    render_layout: wgpu::BindGroupLayout,
    _material_resources: TerrainPreviewMaterialResources,
    exact_coverage: TerrainExactCoverageResources,
    compute_pipeline: Option<wgpu::ComputePipeline>,
    horizon_compute_pipeline: Option<wgpu::ComputePipeline>,
    render_pipeline: Option<wgpu::RenderPipeline>,
    horizon_render_pipeline: Option<wgpu::RenderPipeline>,
    horizon_multiview_render_pipeline: Option<wgpu::RenderPipeline>,
    horizon_exact_connector_pipeline: Option<wgpu::RenderPipeline>,
    horizon_exact_connector_multiview_pipeline: Option<wgpu::RenderPipeline>,
    horizon_render_cell_stride: u32,
    tree_pipeline: wgpu::RenderPipeline,
    tree_multiview_pipeline: Option<wgpu::RenderPipeline>,
    canopy_pipeline: Option<wgpu::RenderPipeline>,
    canopy_multiview_pipeline: Option<wgpu::RenderPipeline>,
    clear_color: wgpu::Color,
    depth: TerrainViewportDepthTarget,
    depth_capture_enabled: bool,
    cache: HashMap<TerrainViewportTileId, TerrainViewportGpuTile>,
    vegetation_cache: Option<McloneOverworldVegetationPlanCache>,
    pending: Vec<PendingTerrainViewportReadback>,
    plan: Option<TerrainViewportPlan>,
    cpu_queue: VecDeque<TerrainViewportTileId>,
    external_cpu_inflight: HashMap<TerrainViewportTileId, u64>,
    external_macro_inflight: HashMap<TerrainViewportTileId, u64>,
    gpu_queue: VecDeque<TerrainViewportTileId>,
    cpu_published_level: Option<usize>,
    gpu_published_level: Option<usize>,
    cache_enabled: bool,
    cache_epoch: u64,
    latest_revision: u64,
    last_comparison_revision: Option<u64>,
    use_clock: u64,
    cpu_compiled_tiles_total: u64,
    gpu_dispatched_tiles_total: u64,
    macro_compiled_tiles_total: u64,
    request_cpu_compiled_tiles: u32,
    request_gpu_dispatched_tiles: u32,
    request_macro_compiled_tiles: u32,
    request_cache_hit_tiles: u32,
    request_cpu_compile_work: TerrainPreviewCompileWork,
    request_gpu_compile_work: TerrainPreviewCompileWork,
    request_cpu_reference_micros: u64,
    request_cpu_vegetation_micros: u64,
    request_cpu_pack_upload_micros: u64,
    request_macro_compile_micros: u64,
    request_vegetation_cell_requests: u64,
    request_vegetation_cell_hits: u64,
    request_vegetation_cell_misses: u64,
    evicted_tiles_total: u64,
    stale_result_count: u64,
    focus_request: Option<(TerrainPreviewProfile, i64, i32, i32)>,
    focus_y: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerrainViewportPipelineSet {
    Viewport,
    Horizon,
}

impl TerrainViewportRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
    ) -> Result<Self, String> {
        Self::new_with_target_color_transform(
            device,
            queue,
            color_format,
            width,
            height,
            material_atlas,
            RenderTargetColorTransform::Identity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_target_color_transform(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        target_color_transform: RenderTargetColorTransform,
    ) -> Result<Self, String> {
        Self::new_with_pipeline_set(
            device,
            queue,
            color_format,
            width,
            height,
            material_atlas,
            target_color_transform,
            TerrainViewportPipelineSet::Viewport,
            1,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_horizon_with_target_color_transform(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        target_color_transform: RenderTargetColorTransform,
        render_cell_stride: u32,
    ) -> Result<Self, String> {
        Self::new_with_pipeline_set(
            device,
            queue,
            color_format,
            width,
            height,
            material_atlas,
            target_color_transform,
            TerrainViewportPipelineSet::Horizon,
            render_cell_stride,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_pipeline_set(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        target_color_transform: RenderTargetColorTransform,
        pipeline_set: TerrainViewportPipelineSet,
        horizon_render_cell_stride: u32,
    ) -> Result<Self, String> {
        let samples_per_axis = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS + 1;
        let sample_count_per_tile = samples_per_axis
            .checked_mul(samples_per_axis)
            .ok_or("terrain viewport sample count overflow")?;
        let sample_byte_len = u64::from(sample_count_per_tile)
            .checked_mul(TERRAIN_PREVIEW_SAMPLE_BYTES)
            .ok_or("terrain viewport sample buffer size overflow")?;
        let normal_height_byte_len = u64::from(sample_count_per_tile)
            .checked_mul(size_of::<f32>() as u64)
            .ok_or("terrain viewport normal-height buffer size overflow")?;
        let normal_height_scratch_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_terrain_viewport_normal_height_scratch"),
            size: normal_height_byte_len,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_terrain_viewport_compute_layout"),
            entries: &[
                uniform_layout_entry(0, wgpu::ShaderStages::COMPUTE),
                storage_layout_entry(1, wgpu::ShaderStages::COMPUTE, false, sample_byte_len),
                storage_layout_entry(
                    2,
                    wgpu::ShaderStages::COMPUTE,
                    false,
                    normal_height_byte_len,
                ),
            ],
        });
        let render_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_terrain_viewport_render_layout"),
            entries: &[
                uniform_layout_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                storage_layout_entry(
                    1,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    true,
                    sample_byte_len,
                ),
                storage_layout_entry(
                    2,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    true,
                    sample_byte_len,
                ),
                storage_layout_entry(
                    3,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    true,
                    normal_height_byte_len,
                ),
            ],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mclone_terrain_viewport_material_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                uniform_layout_entry_with_size(
                    2,
                    wgpu::ShaderStages::FRAGMENT,
                    TERRAIN_PREVIEW_MATERIAL_TABLE_BYTES,
                ),
            ],
        });
        let exact_coverage_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mclone_terrain_exact_coverage_layout"),
                entries: &[
                    uniform_layout_entry_with_size(
                        0,
                        wgpu::ShaderStages::VERTEX_FRAGMENT,
                        TERRAIN_EXACT_COVERAGE_UNIFORM_BYTES,
                    ),
                    storage_layout_entry(
                        1,
                        wgpu::ShaderStages::VERTEX_FRAGMENT,
                        true,
                        TERRAIN_EXACT_COVERAGE_MASK_BYTES,
                    ),
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    storage_layout_entry(
                        5,
                        wgpu::ShaderStages::VERTEX_FRAGMENT,
                        true,
                        TERRAIN_FRONTIER_SUPPORT_LOOKUP_BUFFER_BYTES,
                    ),
                ],
            });
        let material_resources =
            TerrainPreviewMaterialResources::new(device, queue, &material_layout, material_atlas)?;
        let exact_coverage =
            TerrainExactCoverageResources::new(device, queue, &exact_coverage_layout)?;
        let multiview_enabled = pipeline_set == TerrainViewportPipelineSet::Horizon
            && device.features().contains(wgpu::Features::MULTIVIEW);
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_terrain_viewport_compute_shader"),
            source: wgpu::ShaderSource::Wgsl(terrain_preview_compute_wgsl().into()),
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_terrain_viewport_render_shader"),
            source: wgpu::ShaderSource::Wgsl(
                terrain_preview_render_wgsl(target_color_transform).into(),
            ),
        });
        let tree_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mclone_terrain_viewport_tree_shader"),
            source: wgpu::ShaderSource::Wgsl(
                terrain_preview_tree_wgsl(target_color_transform).into(),
            ),
        });
        let multiview_render_shader = multiview_enabled.then(|| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("mclone_terrain_horizon_multiview_render_shader"),
                source: wgpu::ShaderSource::Wgsl(
                    terrain_preview_render_multiview_wgsl(target_color_transform).into(),
                ),
            })
        });
        let multiview_tree_shader = multiview_enabled.then(|| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("mclone_terrain_horizon_multiview_tree_shader"),
                source: wgpu::ShaderSource::Wgsl(
                    terrain_preview_tree_multiview_wgsl(target_color_transform).into(),
                ),
            })
        });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mclone_terrain_viewport_compute_pipeline_layout"),
                bind_group_layouts: &[&compute_layout],
                push_constant_ranges: &[],
            });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mclone_terrain_viewport_render_pipeline_layout"),
                bind_group_layouts: &[&render_layout, &material_layout, &exact_coverage_layout],
                push_constant_ranges: &[],
            });
        let tree_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mclone_terrain_viewport_tree_pipeline_layout"),
            bind_group_layouts: &[&render_layout, &exact_coverage_layout],
            push_constant_ranges: &[],
        });
        let horizon_compute_constants = [(
            "terrain_sample_halo_radius",
            f64::from(TERRAIN_HORIZON_NORMAL_HALO_RADIUS),
        )];
        let horizon_render_constants = [
            (
                "terrain_sample_halo_radius",
                f64::from(TERRAIN_HORIZON_NORMAL_HALO_RADIUS),
            ),
            (
                "terrain_render_cell_stride",
                f64::from(horizon_render_cell_stride),
            ),
        ];
        let compute_pipeline = (pipeline_set == TerrainViewportPipelineSet::Viewport).then(|| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("mclone_terrain_viewport_compute_pipeline"),
                layout: Some(&compute_pipeline_layout),
                module: &compute_shader,
                entry_point: Some("compute_main"),
                compilation_options: Default::default(),
                cache: None,
            })
        });
        let horizon_compute_pipeline =
            (pipeline_set == TerrainViewportPipelineSet::Horizon).then(|| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("mclone_terrain_horizon_compute_pipeline"),
                    layout: Some(&compute_pipeline_layout),
                    module: &compute_shader,
                    entry_point: Some("compute_main"),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &horizon_compute_constants,
                        ..Default::default()
                    },
                    cache: None,
                })
            });
        let render_pipeline = (pipeline_set == TerrainViewportPipelineSet::Viewport).then(|| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("mclone_terrain_viewport_render_pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &render_shader,
                    entry_point: Some("vertex_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &render_shader,
                    entry_point: Some("fragment_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: TERRAIN_PREVIEW_DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::GreaterEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        });
        let horizon_render_pipeline =
            (pipeline_set == TerrainViewportPipelineSet::Horizon).then(|| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("mclone_terrain_horizon_render_pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &render_shader,
                        entry_point: Some("vertex_main"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants: &horizon_render_constants,
                            ..Default::default()
                        },
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &render_shader,
                        entry_point: Some("fragment_main"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: color_format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: TERRAIN_PREVIEW_DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::GreaterEqual,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: Default::default(),
                    multiview: None,
                    cache: None,
                })
            });
        let horizon_multiview_render_pipeline = multiview_enabled.then(|| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("mclone_terrain_horizon_multiview_render_pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: multiview_render_shader
                        .as_ref()
                        .expect("multiview shader exists when multiview is enabled"),
                    entry_point: Some("vertex_multiview_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &horizon_render_constants,
                        ..Default::default()
                    },
                },
                fragment: Some(wgpu::FragmentState {
                    module: &render_shader,
                    entry_point: Some("fragment_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: TERRAIN_PREVIEW_DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::GreaterEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: NonZeroU32::new(2),
                cache: None,
            })
        });
        let horizon_exact_connector_pipeline =
            (pipeline_set == TerrainViewportPipelineSet::Horizon).then(|| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("mclone_terrain_horizon_exact_connector_pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &render_shader,
                        entry_point: Some("exact_connector_vertex_main"),
                        buffers: &[terrain_exact_connector_vertex_buffer_layout()],
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants: &horizon_render_constants,
                            ..Default::default()
                        },
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &render_shader,
                        entry_point: Some("fragment_main"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: color_format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: TERRAIN_PREVIEW_DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::GreaterEqual,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: Default::default(),
                    multiview: None,
                    cache: None,
                })
            });
        let horizon_exact_connector_multiview_pipeline = multiview_enabled.then(|| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("mclone_terrain_horizon_exact_connector_multiview_pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: multiview_render_shader
                        .as_ref()
                        .expect("multiview shader exists when multiview is enabled"),
                    entry_point: Some("exact_connector_vertex_multiview_main"),
                    buffers: &[terrain_exact_connector_vertex_buffer_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &horizon_render_constants,
                        ..Default::default()
                    },
                },
                fragment: Some(wgpu::FragmentState {
                    module: &render_shader,
                    entry_point: Some("fragment_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: TERRAIN_PREVIEW_DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::GreaterEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: NonZeroU32::new(2),
                cache: None,
            })
        });
        let tree_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mclone_terrain_viewport_tree_pipeline"),
            layout: Some(&tree_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &tree_shader,
                entry_point: Some("vertex_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: TERRAIN_PREVIEW_TREE_INSTANCE_BYTES,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 32,
                            shader_location: 2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &tree_shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: TERRAIN_PREVIEW_DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let tree_multiview_pipeline = multiview_enabled.then(|| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("mclone_terrain_viewport_tree_multiview_pipeline"),
                layout: Some(&tree_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: multiview_tree_shader
                        .as_ref()
                        .expect("multiview tree shader exists when multiview is enabled"),
                    entry_point: Some("vertex_multiview_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: TERRAIN_PREVIEW_TREE_INSTANCE_BYTES,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 0,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 16,
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 32,
                                shader_location: 2,
                            },
                        ],
                    }],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &tree_shader,
                    entry_point: Some("fragment_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: TERRAIN_PREVIEW_DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::GreaterEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: NonZeroU32::new(2),
                cache: None,
            })
        });
        let canopy_pipeline = (pipeline_set == TerrainViewportPipelineSet::Horizon).then(|| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("mclone_terrain_horizon_canopy_pipeline"),
                layout: Some(&tree_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &tree_shader,
                    entry_point: Some("canopy_vertex_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &tree_shader,
                    entry_point: Some("canopy_fragment_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: TERRAIN_PREVIEW_DEPTH_FORMAT,
                    // Canopy is a continuous projected-scale veil over the
                    // opaque ground. A depth write would make fractional
                    // canopy occlude later proxy trees like an opaque sheet.
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::GreaterEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        });
        let canopy_multiview_pipeline = multiview_enabled.then(|| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("mclone_terrain_horizon_canopy_multiview_pipeline"),
                layout: Some(&tree_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: multiview_tree_shader
                        .as_ref()
                        .expect("multiview tree shader exists when multiview is enabled"),
                    entry_point: Some("canopy_vertex_multiview_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &tree_shader,
                    entry_point: Some("canopy_fragment_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: TERRAIN_PREVIEW_DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::GreaterEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: NonZeroU32::new(2),
                cache: None,
            })
        });
        Ok(Self {
            sample_count_per_tile,
            sample_byte_len,
            normal_height_scratch_buffer,
            compute_layout,
            render_layout,
            _material_resources: material_resources,
            exact_coverage,
            compute_pipeline,
            horizon_compute_pipeline,
            render_pipeline,
            horizon_render_pipeline,
            horizon_multiview_render_pipeline,
            horizon_exact_connector_pipeline,
            horizon_exact_connector_multiview_pipeline,
            horizon_render_cell_stride,
            tree_pipeline,
            tree_multiview_pipeline,
            canopy_pipeline,
            canopy_multiview_pipeline,
            clear_color: color_transform_wgpu(
                wgpu::Color {
                    r: 0.025,
                    g: 0.035,
                    b: 0.055,
                    a: 1.0,
                },
                target_color_transform,
            ),
            depth: TerrainViewportDepthTarget::new(device, width, height),
            depth_capture_enabled: false,
            cache: HashMap::new(),
            vegetation_cache: None,
            pending: Vec::new(),
            plan: None,
            cpu_queue: VecDeque::new(),
            external_cpu_inflight: HashMap::new(),
            external_macro_inflight: HashMap::new(),
            gpu_queue: VecDeque::new(),
            cpu_published_level: None,
            gpu_published_level: None,
            cache_enabled: true,
            cache_epoch: 0,
            latest_revision: 0,
            last_comparison_revision: None,
            use_clock: 0,
            cpu_compiled_tiles_total: 0,
            gpu_dispatched_tiles_total: 0,
            macro_compiled_tiles_total: 0,
            request_cpu_compiled_tiles: 0,
            request_gpu_dispatched_tiles: 0,
            request_macro_compiled_tiles: 0,
            request_cache_hit_tiles: 0,
            request_cpu_compile_work: TerrainPreviewCompileWork::default(),
            request_gpu_compile_work: TerrainPreviewCompileWork::default(),
            request_cpu_reference_micros: 0,
            request_cpu_vegetation_micros: 0,
            request_cpu_pack_upload_micros: 0,
            request_macro_compile_micros: 0,
            request_vegetation_cell_requests: 0,
            request_vegetation_cell_hits: 0,
            request_vegetation_cell_misses: 0,
            evicted_tiles_total: 0,
            stale_result_count: 0,
            focus_request: None,
            focus_y: 64.0,
        })
    }

    pub fn set_cache_enabled(&mut self, enabled: bool) {
        if self.cache_enabled == enabled {
            return;
        }
        self.cache_enabled = enabled;
        if !enabled {
            self.clear_cache();
        }
    }

    pub fn clear_cache(&mut self) {
        self.stale_result_count = self.stale_result_count.saturating_add(
            (self.cpu_queue.len()
                + self.external_cpu_inflight.len()
                + self.gpu_queue.len()
                + self.external_macro_inflight.len()) as u64,
        );
        self.cache.clear();
        self.vegetation_cache = None;
        self.cpu_queue.clear();
        self.external_cpu_inflight.clear();
        self.external_macro_inflight.clear();
        self.gpu_queue.clear();
        self.cpu_published_level = None;
        self.gpu_published_level = None;
        self.cache_epoch = self.cache_epoch.saturating_add(1);
        self.last_comparison_revision = None;
        self.reset_request_counters();
        self.rebuild_queues();
    }

    pub fn set_viewport(&mut self, revision: u64, plan: TerrainViewportPlan) {
        self.latest_revision = revision;
        let focus_request = (
            plan.request.profile,
            plan.request.seed,
            plan.request.center_x,
            plan.request.center_z,
        );
        if self.focus_request != Some(focus_request) {
            self.focus_request = Some(focus_request);
            self.focus_y = terrain_preview_focus_y_for_profile(
                focus_request.0,
                focus_request.1,
                focus_request.2,
                focus_request.3,
            );
        }
        if self.plan.as_ref() == Some(&plan) {
            return;
        }
        if self.cache_enabled {
            self.stale_result_count = self.stale_result_count.saturating_add(
                (self.cpu_queue.len()
                    + self.external_cpu_inflight.len()
                    + self.gpu_queue.len()
                    + self.external_macro_inflight.len()) as u64,
            );
            self.cpu_queue.clear();
            self.external_cpu_inflight.clear();
            self.external_macro_inflight.clear();
            self.gpu_queue.clear();
        } else {
            self.cache.clear();
            self.vegetation_cache = None;
            self.cpu_queue.clear();
            self.external_cpu_inflight.clear();
            self.external_macro_inflight.clear();
            self.gpu_queue.clear();
            self.cache_epoch = self.cache_epoch.saturating_add(1);
        }
        self.cpu_published_level = None;
        self.gpu_published_level = None;
        self.last_comparison_revision = None;
        self.plan = Some(plan);
        self.reset_request_counters();
        self.request_cache_hit_tiles = self.current_plan_cache_hits();
        self.rebuild_queues();
        self.refresh_published_levels();
        self.touch_current_tiles();
        self.evict_unused_tiles();
    }

    pub const fn stale_result_count(&self) -> u64 {
        self.stale_result_count
    }

    pub fn take_external_cpu_request(&mut self) -> Option<TerrainViewportExternalCpuRequest> {
        let plan = self.plan.as_ref()?;
        if plan.request.profile != TerrainPreviewProfile::VanillaOverworld {
            return None;
        }
        let tile = self.take_next_missing_cpu_tile()?;
        self.external_cpu_inflight
            .insert(tile, self.latest_revision);
        Some(TerrainViewportExternalCpuRequest {
            revision: self.latest_revision,
            tile,
        })
    }

    pub fn accept_external_cpu_tile(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: TerrainViewportExternalCpuRequest,
        reference: TerrainPreviewReferenceGrid,
        compile_micros: u64,
    ) -> Result<bool, String> {
        let active = self.external_cpu_inflight.get(&request.tile).copied();
        if active != Some(request.revision) || !self.plan_contains_tile(request.tile) {
            self.stale_result_count = self.stale_result_count.saturating_add(1);
            return Ok(false);
        }
        self.external_cpu_inflight.remove(&request.tile);
        if reference.request().request() != request.tile.preview_request() {
            return Err("external terrain preview grid does not match its tile request".to_owned());
        }
        let compile_work = reference.compile_work();
        let vegetation = TerrainPreviewVegetationProduct::compile(request.tile.preview_request())?;
        if !self.cache.contains_key(&request.tile) {
            let tile = TerrainViewportGpuTile::new(
                device,
                &self.compute_layout,
                &self.render_layout,
                self.sample_byte_len,
                self.normal_height_scratch_buffer.clone(),
                request.tile,
            )?;
            self.cache.insert(request.tile, tile);
        }
        let tile = self
            .cache
            .get_mut(&request.tile)
            .expect("created external terrain viewport tile is resident");
        tile.upload_reference(device, queue, self.sample_byte_len, reference, vegetation)?;
        self.use_clock = self.use_clock.saturating_add(1);
        tile.last_used = self.use_clock;
        self.cpu_compiled_tiles_total = self.cpu_compiled_tiles_total.saturating_add(1);
        self.request_cpu_compiled_tiles = self.request_cpu_compiled_tiles.saturating_add(1);
        self.request_cpu_compile_work = self.request_cpu_compile_work.saturating_add(compile_work);
        self.request_cpu_reference_micros = self
            .request_cpu_reference_micros
            .saturating_add(compile_micros);
        self.refresh_published_levels();
        self.touch_current_tiles();
        self.evict_unused_tiles();
        Ok(true)
    }

    pub fn reject_external_cpu_tile(&mut self, request: TerrainViewportExternalCpuRequest) {
        if self.external_cpu_inflight.get(&request.tile).copied() != Some(request.revision) {
            return;
        }
        self.external_cpu_inflight.remove(&request.tile);
        if self.plan_contains_tile(request.tile) {
            self.cpu_queue.push_front(request.tile);
        }
    }

    pub fn take_external_macro_request(&mut self) -> Option<TerrainViewportExternalCpuRequest> {
        let plan = self.plan.as_ref()?;
        if plan.request.profile != TerrainPreviewProfile::VanillaOverworld {
            return None;
        }
        let tile = self.take_next_missing_gpu_tile()?;
        self.external_macro_inflight
            .insert(tile, self.latest_revision);
        Some(TerrainViewportExternalCpuRequest {
            revision: self.latest_revision,
            tile,
        })
    }

    pub fn accept_external_macro_tile(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: TerrainViewportExternalCpuRequest,
        macro_grid: TerrainPreviewReferenceGrid,
        compile_micros: u64,
    ) -> Result<bool, String> {
        let active = self.external_macro_inflight.get(&request.tile).copied();
        if active != Some(request.revision) || !self.plan_contains_tile(request.tile) {
            self.stale_result_count = self.stale_result_count.saturating_add(1);
            return Ok(false);
        }
        self.external_macro_inflight.remove(&request.tile);
        if macro_grid.request().request() != request.tile.preview_request() {
            return Err("external terrain macro grid does not match its tile request".to_owned());
        }
        if !self.cache.contains_key(&request.tile) {
            let tile = TerrainViewportGpuTile::new(
                device,
                &self.compute_layout,
                &self.render_layout,
                self.sample_byte_len,
                self.normal_height_scratch_buffer.clone(),
                request.tile,
            )?;
            self.cache.insert(request.tile, tile);
        }
        let tile = self
            .cache
            .get_mut(&request.tile)
            .expect("created external terrain macro tile is resident");
        tile.upload_macro(queue, self.sample_byte_len, macro_grid)?;
        self.use_clock = self.use_clock.saturating_add(1);
        tile.last_used = self.use_clock;
        self.macro_compiled_tiles_total = self.macro_compiled_tiles_total.saturating_add(1);
        self.request_macro_compiled_tiles = self.request_macro_compiled_tiles.saturating_add(1);
        self.request_macro_compile_micros = self
            .request_macro_compile_micros
            .saturating_add(compile_micros);
        self.refresh_published_levels();
        self.touch_current_tiles();
        self.evict_unused_tiles();
        Ok(true)
    }

    pub fn reject_external_macro_tile(&mut self, request: TerrainViewportExternalCpuRequest) {
        if self.external_macro_inflight.get(&request.tile).copied() != Some(request.revision) {
            return;
        }
        self.external_macro_inflight.remove(&request.tile);
        if self.plan_contains_tile(request.tile) {
            self.gpu_queue.push_front(request.tile);
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.depth.width != width || self.depth.height != height {
            self.depth = TerrainViewportDepthTarget::new(device, width, height);
        }
    }

    pub fn copy_depth_to_buffer(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
        bytes_per_row: u32,
    ) -> Result<(), String> {
        let unpadded_row_bytes = self
            .depth
            .width
            .checked_mul(size_of::<f32>() as u32)
            .ok_or("terrain viewport depth row byte length overflow")?;
        if bytes_per_row < unpadded_row_bytes
            || !bytes_per_row.is_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        {
            return Err(format!(
                "terrain viewport depth copy row length must be at least \
                 {unpadded_row_bytes} bytes and {}-byte aligned, got {bytes_per_row}",
                wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
            ));
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.depth.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: destination,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(self.depth.height),
                },
            },
            wgpu::Extent3d {
                width: self.depth.width,
                height: self.depth.height,
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }

    pub fn set_depth_capture_enabled(&mut self, enabled: bool) {
        self.depth_capture_enabled = enabled;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        options: TerrainPreviewDrawOptions,
        camera: TerrainPreviewCamera,
        mut clock_ms: impl FnMut() -> f64,
    ) -> Result<TerrainViewportFrameStats, String> {
        self.resize(device, width, height);
        let plan = self
            .plan
            .clone()
            .ok_or("terrain viewport renderer has no active plan")?;
        let cpu_required =
            source_needs_cpu(options, plan.request.content_stage, plan.effective_spacing);
        let gpu_required = source_needs_gpu(options, plan.request.profile);
        let focus_y = self.focus_y;
        let mut encoded_readbacks = Vec::new();
        let mut gpu_dispatched_tiles = 0_u32;

        if gpu_required && plan.request.profile.supports_gpu_lod() {
            let mut compute_encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("mclone_terrain_viewport_compute_encoder"),
                });
            for _ in 0..TERRAIN_VIEWPORT_GPU_DISPATCHES_PER_FRAME {
                let Some(tile_id) = self.take_next_missing_gpu_tile() else {
                    break;
                };
                if !self.cache.contains_key(&tile_id) {
                    let tile = TerrainViewportGpuTile::new(
                        device,
                        &self.compute_layout,
                        &self.render_layout,
                        self.sample_byte_len,
                        self.normal_height_scratch_buffer.clone(),
                        tile_id,
                    )?;
                    self.cache.insert(tile_id, tile);
                }
                let tile = self
                    .cache
                    .get_mut(&tile_id)
                    .expect("created terrain viewport GPU tile is resident");
                queue.write_buffer(
                    &tile.uniform_buffer,
                    0,
                    &viewport_uniform_bytes_for_request(
                        tile.request,
                        width,
                        height,
                        options,
                        camera,
                        plan.request.center_x,
                        plan.request.center_z,
                        plan.view_width_blocks,
                        plan.view_height_blocks,
                        focus_y,
                    ),
                );
                {
                    let mut pass =
                        compute_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("mclone_terrain_viewport_compute_pass"),
                            timestamp_writes: None,
                        });
                    pass.set_pipeline(
                        self.compute_pipeline
                            .as_ref()
                            .expect("viewport renderer owns its compute pipeline"),
                    );
                    pass.set_bind_group(0, &tile.compute_bind_group, &[]);
                    let workgroups = (TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS + 1)
                        .div_ceil(TERRAIN_PREVIEW_WORKGROUP_AXIS);
                    pass.dispatch_workgroups(workgroups, workgroups, 1);
                }
                let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("mclone_terrain_viewport_readback"),
                    size: self.sample_byte_len,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                compute_encoder.copy_buffer_to_buffer(
                    &tile.gpu_sample_buffer,
                    0,
                    &readback_buffer,
                    0,
                    self.sample_byte_len,
                );
                tile.gpu_submitted = true;
                self.use_clock = self.use_clock.saturating_add(1);
                tile.last_used = self.use_clock;
                encoded_readbacks.push(EncodedTerrainViewportReadback {
                    cache_epoch: self.cache_epoch,
                    tile: tile_id,
                    readback_buffer,
                    byte_len: self.sample_byte_len,
                });
                gpu_dispatched_tiles = gpu_dispatched_tiles.saturating_add(1);
                self.gpu_dispatched_tiles_total = self.gpu_dispatched_tiles_total.saturating_add(1);
                self.request_gpu_dispatched_tiles =
                    self.request_gpu_dispatched_tiles.saturating_add(1);
                self.request_gpu_compile_work = self
                    .request_gpu_compile_work
                    .saturating_add(terrain_preview_gpu_compile_work(tile.request));
            }
            if !encoded_readbacks.is_empty() {
                queue.submit(std::iter::once(compute_encoder.finish()));
                self.mark_submitted(EncodedTerrainViewportReadbacks {
                    readbacks: encoded_readbacks,
                });
            }
        }

        let mut cpu_compiled_tiles = 0_u32;
        let mut cpu_reference_micros = 0_u64;
        let mut cpu_vegetation_micros = 0_u64;
        let mut cpu_pack_upload_micros = 0_u64;

        if cpu_required && plan.request.profile != TerrainPreviewProfile::VanillaOverworld {
            for _ in 0..TERRAIN_VIEWPORT_TILE_COMPILES_PER_FRAME {
                let frame_cpu_micros = cpu_reference_micros
                    .saturating_add(cpu_vegetation_micros)
                    .saturating_add(cpu_pack_upload_micros);
                if cpu_compiled_tiles > 0
                    && frame_cpu_micros >= TERRAIN_VIEWPORT_CPU_COMPILE_BUDGET_MICROS
                {
                    break;
                }
                let Some(tile_id) = self.take_next_missing_cpu_tile() else {
                    break;
                };
                let reference_started = clock_ms();
                let reference = TerrainPreviewReferenceGrid::compile(tile_id.preview_request())?;
                let compile_work = reference.compile_work();
                let reference_micros =
                    ((clock_ms() - reference_started).max(0.0) * 1_000.0).round();
                cpu_reference_micros = cpu_reference_micros
                    .saturating_add(reference_micros.min(u64::MAX as f64) as u64);
                let vegetation_started = clock_ms();
                let vegetation = self.compile_vegetation_product(tile_id)?;
                let vegetation_report = vegetation.cache_report();
                let vegetation_micros =
                    ((clock_ms() - vegetation_started).max(0.0) * 1_000.0).round();
                cpu_vegetation_micros = cpu_vegetation_micros
                    .saturating_add(vegetation_micros.min(u64::MAX as f64) as u64);
                if !self.cache.contains_key(&tile_id) {
                    let tile = TerrainViewportGpuTile::new(
                        device,
                        &self.compute_layout,
                        &self.render_layout,
                        self.sample_byte_len,
                        self.normal_height_scratch_buffer.clone(),
                        tile_id,
                    )?;
                    self.cache.insert(tile_id, tile);
                }
                let tile = self
                    .cache
                    .get_mut(&tile_id)
                    .expect("created terrain viewport CPU tile is resident");
                let upload_started = clock_ms();
                tile.upload_reference(device, queue, self.sample_byte_len, reference, vegetation)?;
                let upload_micros = ((clock_ms() - upload_started).max(0.0) * 1_000.0).round();
                cpu_pack_upload_micros = cpu_pack_upload_micros
                    .saturating_add(upload_micros.min(u64::MAX as f64) as u64);
                self.use_clock = self.use_clock.saturating_add(1);
                tile.last_used = self.use_clock;
                cpu_compiled_tiles = cpu_compiled_tiles.saturating_add(1);
                self.cpu_compiled_tiles_total = self.cpu_compiled_tiles_total.saturating_add(1);
                self.request_cpu_compiled_tiles = self.request_cpu_compiled_tiles.saturating_add(1);
                self.request_cpu_compile_work =
                    self.request_cpu_compile_work.saturating_add(compile_work);
                self.request_vegetation_cell_requests = self
                    .request_vegetation_cell_requests
                    .saturating_add(vegetation_report.cell_requests);
                self.request_vegetation_cell_hits = self
                    .request_vegetation_cell_hits
                    .saturating_add(vegetation_report.cell_hits);
                self.request_vegetation_cell_misses = self
                    .request_vegetation_cell_misses
                    .saturating_add(vegetation_report.cell_misses);
            }
        }
        self.request_cpu_reference_micros = self
            .request_cpu_reference_micros
            .saturating_add(cpu_reference_micros);
        self.request_cpu_vegetation_micros = self
            .request_cpu_vegetation_micros
            .saturating_add(cpu_vegetation_micros);
        self.request_cpu_pack_upload_micros = self
            .request_cpu_pack_upload_micros
            .saturating_add(cpu_pack_upload_micros);

        self.refresh_published_levels();
        let cpu_published_tiles = self.cpu_published_tiles().to_vec();
        let gpu_published_tiles = self.gpu_published_tiles().to_vec();
        let drawn_tiles = cpu_published_tiles
            .iter()
            .chain(gpu_published_tiles.iter())
            .copied()
            .collect::<HashSet<_>>();
        for tile_id in &drawn_tiles {
            let tile = self
                .cache
                .get_mut(tile_id)
                .expect("published terrain viewport tiles are resident");
            queue.write_buffer(
                &tile.uniform_buffer,
                0,
                &viewport_uniform_bytes_for_request(
                    tile.request,
                    width,
                    height,
                    options,
                    camera,
                    plan.request.center_x,
                    plan.request.center_z,
                    plan.view_width_blocks,
                    plan.view_height_blocks,
                    focus_y,
                ),
            );
            self.use_clock = self.use_clock.saturating_add(1);
            tile.last_used = self.use_clock;
        }
        self.encode_draw(
            encoder,
            color_view,
            width.max(1),
            height.max(1),
            options,
            &cpu_published_tiles,
            &gpu_published_tiles,
        );
        self.evict_unused_tiles();

        let target = plan.target_level();
        let cpu_coarse_ready = self.level_cpu_ready(plan.coarsest_level().visible_tiles.as_slice());
        let cpu_target_ready = self.level_cpu_ready(target.visible_tiles.as_slice());
        let gpu_coarse_ready = self.level_gpu_ready(plan.coarsest_level().visible_tiles.as_slice());
        let gpu_target_ready = self.level_gpu_ready(target.visible_tiles.as_slice());
        let coarse_ready =
            (!cpu_required || cpu_coarse_ready) && (!gpu_required || gpu_coarse_ready);
        let target_ready =
            (!cpu_required || cpu_target_ready) && (!gpu_required || gpu_target_ready);
        let cpu_published_tile_count = u32::try_from(cpu_published_tiles.len()).unwrap_or(u32::MAX);
        let gpu_published_tile_count = u32::try_from(gpu_published_tiles.len()).unwrap_or(u32::MAX);
        let published_tile_count = match options.source {
            TerrainPreviewSource::Reference => cpu_published_tile_count,
            TerrainPreviewSource::Gpu | TerrainPreviewSource::Macro => gpu_published_tile_count,
            TerrainPreviewSource::Split => cpu_published_tile_count.min(gpu_published_tile_count),
        };
        let sample_count = cpu_published_tile_count
            .max(gpu_published_tile_count)
            .saturating_mul(self.sample_count_per_tile);
        let terrain_vertex_count = cpu_published_tile_count
            .saturating_add(gpu_published_tile_count)
            .saturating_mul(TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS.pow(2) * 6);
        let vegetation_tiles = drawn_tiles
            .iter()
            .filter_map(|tile_id| self.cache.get(tile_id))
            .filter_map(|tile| tile.vegetation.as_ref())
            .collect::<Vec<_>>();
        let cpu_vegetation_summary_tile_count = u32::try_from(
            vegetation_tiles
                .iter()
                .filter(|product| product.summary_available())
                .count(),
        )
        .unwrap_or(u32::MAX);
        let gpu_vegetation_summary_tile_count = u32::try_from(
            gpu_published_tiles
                .iter()
                .filter(|tile_id| tile_id.content_stage == TerrainPreviewContentStage::Cover)
                .count(),
        )
        .unwrap_or(u32::MAX);
        let vegetation_summary_tile_count = match options.source {
            TerrainPreviewSource::Reference => cpu_vegetation_summary_tile_count,
            TerrainPreviewSource::Gpu | TerrainPreviewSource::Macro => {
                gpu_vegetation_summary_tile_count
            }
            TerrainPreviewSource::Split => {
                cpu_vegetation_summary_tile_count.max(gpu_vegetation_summary_tile_count)
            }
        };
        let vegetation_record_tile_count = u32::try_from(
            vegetation_tiles
                .iter()
                .filter(|product| product.records_requested())
                .count(),
        )
        .unwrap_or(u32::MAX);
        let cpu_vegetation_aggregated_tile_count = u32::try_from(
            vegetation_tiles
                .iter()
                .filter(|product| product.records_aggregated())
                .count(),
        )
        .unwrap_or(u32::MAX);
        let gpu_vegetation_aggregated_tile_count = u32::try_from(
            gpu_published_tiles
                .iter()
                .filter(|tile_id| {
                    tile_id.content_stage == TerrainPreviewContentStage::Cover
                        && tile_id.sample_spacing > TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING
                })
                .count(),
        )
        .unwrap_or(u32::MAX);
        let vegetation_aggregated_tile_count = match options.source {
            TerrainPreviewSource::Reference => cpu_vegetation_aggregated_tile_count,
            TerrainPreviewSource::Gpu | TerrainPreviewSource::Macro => {
                gpu_vegetation_aggregated_tile_count
            }
            TerrainPreviewSource::Split => {
                cpu_vegetation_aggregated_tile_count.max(gpu_vegetation_aggregated_tile_count)
            }
        };
        let tree_instance_count = vegetation_tiles.iter().fold(0_u32, |count, product| {
            count.saturating_add(u32::try_from(product.occurrences().len()).unwrap_or(u32::MAX))
        });
        let tree_instance_bytes = drawn_tiles
            .iter()
            .filter_map(|tile_id| self.cache.get(tile_id))
            .map(|tile| tile.tree_instance_bytes)
            .sum::<u64>();
        let tree_proxy_vertex_count = if options.layer == TerrainPreviewLayer::Terrain
            && plan.request.content_stage == TerrainPreviewContentStage::Cover
        {
            render_panels(
                options.source,
                options.split_layout,
                width.max(1),
                height.max(1),
            )
            .iter()
            .fold(0_u32, |count, panel| {
                let tiles = if options.source == TerrainPreviewSource::Reference
                    || (options.source == TerrainPreviewSource::Split && panel.instance == 0)
                {
                    &cpu_published_tiles
                } else {
                    &gpu_published_tiles
                };
                count.saturating_add(tiles.iter().fold(0_u32, |tile_count, tile_id| {
                    let instances = self
                        .cache
                        .get(tile_id)
                        .map_or(0, |tile| tile.tree_instance_count);
                    tile_count.saturating_add(
                        instances.saturating_mul(TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE),
                    )
                }))
            })
        } else {
            0
        };
        let vertex_count = terrain_vertex_count.saturating_add(tree_proxy_vertex_count);
        let resident_tile_count = u32::try_from(self.cache.len()).unwrap_or(u32::MAX);
        let cpu_queued_tile_count = if cpu_required {
            u32::try_from(self.cpu_queue.len() + self.external_cpu_inflight.len())
                .unwrap_or(u32::MAX)
        } else {
            0
        };
        let gpu_queued_tile_count = if gpu_required {
            u32::try_from(self.gpu_queue.len() + self.external_macro_inflight.len())
                .unwrap_or(u32::MAX)
        } else {
            0
        };
        let pending_readback_count = self
            .pending
            .iter()
            .filter(|pending| pending.cache_epoch == self.cache_epoch)
            .count();
        let cpu_ready_tiles = self
            .cache
            .values()
            .filter(|tile| tile.reference.is_some())
            .count() as u64;
        let gpu_ready_tiles = self
            .cache
            .values()
            .filter(|tile| tile.gpu_submitted)
            .count() as u64;
        let resident_tree_instance_bytes = self
            .cache
            .values()
            .map(|tile| tile.tree_instance_bytes)
            .sum::<u64>();
        let retained_vegetation_cells = self.vegetation_cache.as_ref().map_or(0, |cache| {
            u32::try_from(cache.report().retained_cells).unwrap_or(u32::MAX)
        });
        let cpu_published_spacing = self
            .cpu_published_level
            .map(|index| plan.levels[index].sample_spacing)
            .unwrap_or(0);
        let gpu_published_spacing = self
            .gpu_published_level
            .map(|index| plan.levels[index].sample_spacing)
            .unwrap_or(0);
        let published_spacing = match options.source {
            TerrainPreviewSource::Reference => cpu_published_spacing,
            TerrainPreviewSource::Gpu | TerrainPreviewSource::Macro => gpu_published_spacing,
            TerrainPreviewSource::Split => match (cpu_published_spacing, gpu_published_spacing) {
                (0, _) | (_, 0) => 0,
                (cpu, gpu) => cpu.max(gpu),
            },
        };
        let stats = TerrainViewportFrameStats {
            revision: self.latest_revision,
            requested_spacing: plan.requested_spacing,
            effective_spacing: plan.effective_spacing,
            published_spacing,
            cpu_published_spacing,
            gpu_published_spacing,
            view_width_blocks: plan.view_width_blocks,
            view_height_blocks: plan.view_height_blocks,
            level_count: u32::try_from(plan.levels.len()).unwrap_or(u32::MAX),
            visible_tile_count: u32::try_from(target.visible_tiles.len()).unwrap_or(u32::MAX),
            published_tile_count,
            cpu_published_tile_count,
            gpu_published_tile_count,
            resident_tile_count,
            queued_tile_count: cpu_queued_tile_count.saturating_add(gpu_queued_tile_count),
            cpu_queued_tile_count,
            gpu_queued_tile_count,
            pending_readback_count: u32::try_from(pending_readback_count).unwrap_or(u32::MAX),
            cpu_compiled_tiles,
            cpu_compiled_tiles_total: self.cpu_compiled_tiles_total,
            gpu_dispatched_tiles,
            gpu_dispatched_tiles_total: self.gpu_dispatched_tiles_total,
            macro_compiled_tiles_total: self.macro_compiled_tiles_total,
            request_cpu_compiled_tiles: self.request_cpu_compiled_tiles,
            request_gpu_dispatched_tiles: self.request_gpu_dispatched_tiles,
            request_macro_compiled_tiles: self.request_macro_compiled_tiles,
            request_cache_hit_tiles: self.request_cache_hit_tiles,
            request_cpu_compile_work: self.request_cpu_compile_work,
            request_gpu_compile_work: self.request_gpu_compile_work,
            evicted_tiles_total: self.evicted_tiles_total,
            stale_result_count: self.stale_result_count,
            sample_count,
            vertex_count,
            vegetation_summary_tile_count,
            cpu_vegetation_summary_tile_count,
            gpu_vegetation_summary_tile_count,
            vegetation_record_tile_count,
            vegetation_aggregated_tile_count,
            tree_instance_count,
            tree_instance_bytes,
            tree_proxy_vertex_count,
            vegetation_cell_requests: self.request_vegetation_cell_requests,
            vegetation_cell_hits: self.request_vegetation_cell_hits,
            vegetation_cell_misses: self.request_vegetation_cell_misses,
            retained_vegetation_cells,
            reference_bytes: cpu_ready_tiles * self.sample_byte_len,
            gpu_sample_bytes: gpu_ready_tiles * self.sample_byte_len,
            readback_bytes: u64::from(gpu_dispatched_tiles) * self.sample_byte_len,
            request_readback_bytes: u64::from(self.request_gpu_dispatched_tiles)
                * self.sample_byte_len,
            resident_bytes: u64::from(resident_tile_count)
                * (self.sample_byte_len * 2 + TERRAIN_PREVIEW_UNIFORM_BYTES)
                + resident_tree_instance_bytes,
            cpu_reference_micros,
            cpu_vegetation_micros,
            cpu_pack_upload_micros,
            request_cpu_reference_micros: self.request_cpu_reference_micros,
            request_cpu_vegetation_micros: self.request_cpu_vegetation_micros,
            request_cpu_pack_upload_micros: self.request_cpu_pack_upload_micros,
            request_macro_compile_micros: self.request_macro_compile_micros,
            cpu_coarse_ready,
            cpu_target_ready,
            gpu_coarse_ready,
            gpu_target_ready,
            coarse_ready,
            target_ready,
            cache_enabled: self.cache_enabled,
            budget_limited: plan.budget_limited,
            needs_redraw: cpu_queued_tile_count > 0
                || gpu_queued_tile_count > 0
                || pending_readback_count > 0,
        };
        Ok(stats)
    }

    fn mark_submitted(&mut self, encoded: EncodedTerrainViewportReadbacks) {
        for readback in encoded.readbacks {
            let (sender, receiver) = mpsc::channel();
            readback
                .readback_buffer
                .slice(..readback.byte_len)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            self.pending.push(PendingTerrainViewportReadback {
                cache_epoch: readback.cache_epoch,
                tile: readback.tile,
                readback_buffer: readback.readback_buffer,
                byte_len: readback.byte_len,
                receiver,
            });
        }
    }

    pub fn poll_completed(
        &mut self,
        device: &wgpu::Device,
    ) -> Vec<Result<TerrainViewportCompletedComparison, String>> {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut completed = Vec::new();
        let mut index = 0;
        while index < self.pending.len() {
            match self.pending[index].receiver.try_recv() {
                Ok(Ok(())) => {
                    let pending = self.pending.remove(index);
                    let mapped = pending
                        .readback_buffer
                        .slice(..pending.byte_len)
                        .get_mapped_range();
                    let samples = parse_samples(&mapped);
                    drop(mapped);
                    pending.readback_buffer.unmap();
                    if pending.cache_epoch != self.cache_epoch {
                        self.stale_result_count = self.stale_result_count.saturating_add(1);
                        continue;
                    }
                    match (self.cache.get_mut(&pending.tile), samples) {
                        (Some(tile), Ok(samples)) => tile.gpu_samples = Some(samples),
                        (Some(_), Err(error)) => completed.push(Err(error)),
                        (None, _) => {
                            self.stale_result_count = self.stale_result_count.saturating_add(1);
                        }
                    }
                }
                Ok(Err(error)) => {
                    let pending = self.pending.remove(index);
                    completed.push(Err(format!(
                        "terrain viewport tile {:?} readback failed: {error}",
                        pending.tile
                    )));
                }
                Err(mpsc::TryRecvError::Empty) => index += 1,
                Err(mpsc::TryRecvError::Disconnected) => {
                    let pending = self.pending.remove(index);
                    completed.push(Err(format!(
                        "terrain viewport tile {:?} readback callback disconnected",
                        pending.tile
                    )));
                }
            }
        }
        self.refresh_published_levels();
        if self.last_comparison_revision != Some(self.latest_revision)
            && let Some(result) = self.aggregate_target_comparison()
        {
            match result {
                Ok(comparison) => {
                    self.last_comparison_revision = Some(self.latest_revision);
                    completed.push(Ok(comparison));
                }
                Err(error) => completed.push(Err(error)),
            }
        }
        completed
    }

    fn compile_vegetation_product(
        &mut self,
        tile: TerrainViewportTileId,
    ) -> Result<TerrainPreviewVegetationProduct, String> {
        let request = tile.preview_request();
        if request.profile != TerrainPreviewProfile::McloneOverworldV1
            || request.content_stage != TerrainPreviewContentStage::Cover
        {
            return TerrainPreviewVegetationProduct::compile(request);
        }
        let source =
            McloneVegetationSource::new(request.seed, McloneOverworldSamplingTopology::Unbounded);
        let cache = self
            .vegetation_cache
            .get_or_insert_with(|| McloneOverworldVegetationPlanCache::new(source));
        TerrainPreviewVegetationProduct::compile_with_cache(request, cache)
    }

    fn reset_request_counters(&mut self) {
        self.request_cpu_compiled_tiles = 0;
        self.request_gpu_dispatched_tiles = 0;
        self.request_macro_compiled_tiles = 0;
        self.request_cache_hit_tiles = 0;
        self.request_cpu_compile_work = TerrainPreviewCompileWork::default();
        self.request_gpu_compile_work = TerrainPreviewCompileWork::default();
        self.request_cpu_reference_micros = 0;
        self.request_cpu_vegetation_micros = 0;
        self.request_cpu_pack_upload_micros = 0;
        self.request_macro_compile_micros = 0;
        self.request_vegetation_cell_requests = 0;
        self.request_vegetation_cell_hits = 0;
        self.request_vegetation_cell_misses = 0;
    }

    fn current_plan_cache_hits(&self) -> u32 {
        let Some(plan) = &self.plan else {
            return 0;
        };
        let mut seen = HashSet::new();
        u32::try_from(
            plan.levels
                .iter()
                .flat_map(|level| level.visible_tiles.iter())
                .filter(|tile| seen.insert(**tile) && self.cache.contains_key(tile))
                .count(),
        )
        .unwrap_or(u32::MAX)
    }

    fn rebuild_queues(&mut self) {
        let Some(plan) = self.plan.as_ref() else {
            return;
        };
        let mut cpu_scheduled = HashSet::new();
        let mut gpu_scheduled = HashSet::new();
        for level in &plan.levels {
            for tile in &level.visible_tiles {
                let resident = self.cache.get(tile);
                if resident.is_none_or(|resident| resident.reference.is_none())
                    && cpu_scheduled.insert(*tile)
                {
                    self.cpu_queue.push_back(*tile);
                }
                if resident.is_none_or(|resident| !resident.gpu_submitted)
                    && gpu_scheduled.insert(*tile)
                {
                    self.gpu_queue.push_back(*tile);
                }
            }
        }
        if self.cache_enabled {
            for tile in &plan.target_level().preload_tiles {
                let resident = self.cache.get(tile);
                if resident.is_none_or(|resident| resident.reference.is_none())
                    && cpu_scheduled.insert(*tile)
                {
                    self.cpu_queue.push_back(*tile);
                }
                if resident.is_none_or(|resident| !resident.gpu_submitted)
                    && gpu_scheduled.insert(*tile)
                {
                    self.gpu_queue.push_back(*tile);
                }
            }
        }
    }

    fn take_next_missing_cpu_tile(&mut self) -> Option<TerrainViewportTileId> {
        while let Some(tile) = self.cpu_queue.pop_front() {
            if self
                .cache
                .get(&tile)
                .is_none_or(|resident| resident.reference.is_none())
            {
                return Some(tile);
            }
        }
        None
    }

    fn take_next_missing_gpu_tile(&mut self) -> Option<TerrainViewportTileId> {
        while let Some(tile) = self.gpu_queue.pop_front() {
            if self
                .cache
                .get(&tile)
                .is_none_or(|resident| !resident.gpu_submitted)
            {
                return Some(tile);
            }
        }
        None
    }

    fn plan_contains_tile(&self, tile: TerrainViewportTileId) -> bool {
        self.plan.as_ref().is_some_and(|plan| {
            plan.levels
                .iter()
                .any(|level| level.visible_tiles.contains(&tile))
                || plan.target_level().preload_tiles.contains(&tile)
        })
    }

    fn refresh_published_levels(&mut self) {
        let Some(plan) = self.plan.as_ref() else {
            self.cpu_published_level = None;
            self.gpu_published_level = None;
            return;
        };
        self.cpu_published_level = plan
            .levels
            .iter()
            .enumerate()
            .filter(|(_, level)| self.level_cpu_ready(level.visible_tiles.as_slice()))
            .map(|(index, _)| index)
            .next_back();
        self.gpu_published_level = plan
            .levels
            .iter()
            .enumerate()
            .filter(|(_, level)| self.level_gpu_ready(level.visible_tiles.as_slice()))
            .map(|(index, _)| index)
            .next_back();
    }

    fn level_cpu_ready(&self, tiles: &[TerrainViewportTileId]) -> bool {
        tiles.iter().all(|tile| {
            self.cache
                .get(tile)
                .is_some_and(|resident| resident.reference.is_some())
        })
    }

    fn level_gpu_ready(&self, tiles: &[TerrainViewportTileId]) -> bool {
        tiles.iter().all(|tile| {
            self.cache
                .get(tile)
                .is_some_and(|resident| resident.gpu_samples.is_some())
        })
    }

    fn cpu_published_tiles(&self) -> &[TerrainViewportTileId] {
        self.plan
            .as_ref()
            .zip(self.cpu_published_level)
            .map(|(plan, index)| plan.levels[index].visible_tiles.as_slice())
            .unwrap_or(&[])
    }

    fn gpu_published_tiles(&self) -> &[TerrainViewportTileId] {
        self.plan
            .as_ref()
            .zip(self.gpu_published_level)
            .map(|(plan, index)| plan.levels[index].visible_tiles.as_slice())
            .unwrap_or(&[])
    }

    fn touch_current_tiles(&mut self) {
        let protected = self.protected_tiles();
        for tile_id in protected {
            if let Some(tile) = self.cache.get_mut(&tile_id) {
                self.use_clock = self.use_clock.saturating_add(1);
                tile.last_used = self.use_clock;
            }
        }
    }

    fn protected_tiles(&self) -> HashSet<TerrainViewportTileId> {
        let mut protected = HashSet::new();
        if let Some(plan) = &self.plan {
            for level in &plan.levels {
                protected.extend(level.visible_tiles.iter().copied());
            }
            if self.cache_enabled {
                protected.extend(plan.target_level().preload_tiles.iter().copied());
            }
        }
        protected.extend(
            self.pending
                .iter()
                .filter(|pending| pending.cache_epoch == self.cache_epoch)
                .map(|pending| pending.tile),
        );
        protected
    }

    fn evict_unused_tiles(&mut self) {
        let protected = self.protected_tiles();
        while self.cache.len() > TERRAIN_VIEWPORT_MAX_RESIDENT_TILES {
            let candidate = self
                .cache
                .iter()
                .filter(|(tile, _)| !protected.contains(tile))
                .min_by_key(|(_, resident)| resident.last_used)
                .map(|(tile, _)| *tile);
            let Some(candidate) = candidate else {
                break;
            };
            self.cache.remove(&candidate);
            self.evicted_tiles_total = self.evicted_tiles_total.saturating_add(1);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        options: TerrainPreviewDrawOptions,
        cpu_tiles: &[TerrainViewportTileId],
        gpu_tiles: &[TerrainViewportTileId],
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_terrain_viewport_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: if self.depth_capture_enabled {
                        wgpu::StoreOp::Store
                    } else {
                        wgpu::StoreOp::Discard
                    },
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(
            self.render_pipeline
                .as_ref()
                .expect("viewport renderer owns its render pipeline"),
        );
        pass.set_bind_group(1, &self._material_resources.bind_group, &[]);
        pass.set_bind_group(2, &self.exact_coverage.bind_group, &[]);
        let panels = render_panels(options.source, options.split_layout, width, height);
        for panel in &panels {
            pass.set_scissor_rect(panel.x, panel.y, panel.width, panel.height);
            let tiles = if options.source == TerrainPreviewSource::Reference
                || (options.source == TerrainPreviewSource::Split && panel.instance == 0)
            {
                cpu_tiles
            } else {
                gpu_tiles
            };
            for tile_id in tiles {
                let tile = self
                    .cache
                    .get(tile_id)
                    .expect("drawn terrain viewport tiles are resident");
                if options.layer == TerrainPreviewLayer::Error
                    && (tile.reference.is_none() || tile.gpu_samples.is_none())
                {
                    continue;
                }
                pass.set_bind_group(0, &tile.render_bind_group, &[]);
                pass.draw(
                    0..TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS.pow(2) * 6,
                    panel.instance..panel.instance + 1,
                );
            }
        }
        if options.layer != TerrainPreviewLayer::Terrain {
            return;
        }
        pass.set_pipeline(&self.tree_pipeline);
        pass.set_bind_group(1, &self.exact_coverage.bind_group, &[]);
        for panel in &panels {
            pass.set_scissor_rect(panel.x, panel.y, panel.width, panel.height);
            let tiles = if options.source == TerrainPreviewSource::Reference
                || (options.source == TerrainPreviewSource::Split && panel.instance == 0)
            {
                cpu_tiles
            } else {
                gpu_tiles
            };
            for tile_id in tiles {
                let tile = self
                    .cache
                    .get(tile_id)
                    .expect("drawn terrain viewport tiles are resident");
                let Some(instance_buffer) = tile.tree_instance_buffer.as_ref() else {
                    continue;
                };
                pass.set_bind_group(0, &tile.render_bind_group, &[]);
                pass.set_vertex_buffer(0, instance_buffer.slice(..));
                let first_instance = panel.instance.saturating_mul(tile.tree_instance_count);
                pass.draw(
                    0..TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE,
                    first_instance..first_instance.saturating_add(tile.tree_instance_count),
                );
            }
        }
    }

    fn aggregate_target_comparison(
        &self,
    ) -> Option<Result<TerrainViewportCompletedComparison, String>> {
        let plan = self.plan.as_ref()?;
        let target = plan.target_level();
        let sample_capacity = target
            .visible_tiles
            .len()
            .checked_mul(usize::try_from(self.sample_count_per_tile).ok()?)?;
        let mut reference_samples = Vec::with_capacity(sample_capacity);
        let mut gpu_samples = Vec::with_capacity(sample_capacity);
        for tile_id in &target.visible_tiles {
            let tile = self.cache.get(tile_id)?;
            let samples = tile.gpu_samples.as_ref()?;
            let reference = tile.reference.as_ref()?.samples();
            reference_samples.extend_from_slice(reference);
            gpu_samples.extend(samples.iter().zip(reference).map(|(gpu, cpu)| {
                if tile_id.content_stage.includes_structured_hydrology()
                    && cpu.planned_stream_influence > 0.0
                {
                    *cpu
                } else {
                    *gpu
                }
            }));
        }
        Some(
            TerrainPreviewComparison::compare_samples(&reference_samples, &gpu_samples).map(
                |comparison| TerrainViewportCompletedComparison {
                    revision: self.latest_revision,
                    sample_spacing: target.sample_spacing,
                    tile_count: u32::try_from(target.visible_tiles.len()).unwrap_or(u32::MAX),
                    comparison,
                },
            ),
        )
    }
}

pub struct TerrainHorizonRenderer {
    renderer: TerrainViewportRenderer,
    clipmap: TerrainClipmap,
    slots: Vec<TerrainViewportGpuTile>,
    admission: TerrainHorizonAdmission,
    pending: VecDeque<TerrainHorizonResourceTile>,
    #[cfg(not(target_arch = "wasm32"))]
    cpu_compiler: Option<TerrainHorizonCpuCompiler>,
    cpu_source_generation: u64,
    cpu_compile_in_flight: u32,
    cpu_compile_submitted_total: u64,
    cpu_compile_completed_total: u64,
    cpu_compile_micros_total: u64,
    cpu_compile_stale_results_total: u64,
    vegetation_max_sample_spacing: u32,
    vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
    vegetation_coordinator: Option<TerrainVegetationCoordinator>,
    vegetation_error: Option<String>,
    profile: TerrainPreviewProfile,
    seed: i64,
    requested_center_x: i32,
    requested_center_z: i32,
    content_stage: TerrainPreviewContentStage,
    dispatched_refills_total: u64,
    exact_coverage_snapshot: Option<ExactPaintedCoverageSnapshot>,
    exact_coverage_stabilizing: bool,
    exact_topology: HorizontalTopology,
    frontier_plan: Option<TerrainFrontierPlan>,
    frontier_receipt: TerrainFrontierPlanReceipt,
    frontier_plan_failures: u64,
    frontier_topology: Option<TerrainFrontierTopology>,
    frontier_topology_receipt: TerrainFrontierTopologyReceipt,
    frontier_topology_failures: u64,
    /// Complete certificate currently consumed by every render view.
    frontier_support: Option<TerrainFrontierSupportGpu>,
    /// Preferred fine-support certificate compiling behind the active
    /// synchronous fallback.
    frontier_support_pending: Option<TerrainFrontierSupportGpu>,
    frontier_support_dispatches_total: u64,
    frontier_admission: TerrainFrontierAdmissionTracker,
    frontier_observer_chunk: [i64; 2],
    authoritative_tree_ownership: bool,
    tree_ownership: Option<BoundedRepresentationOwnershipSnapshot<McloneTreeOccurrenceId>>,
    exact_owned_tree_ids: BTreeSet<McloneTreeOccurrenceId>,
    visible_terrain_slots: Vec<bool>,
    visible_vegetation_slots: Vec<bool>,
    forest_reveal_clock: TerrainHorizonRevealClock,
}

impl TerrainHorizonRenderer {
    pub fn reset_source(&mut self) {
        self.clipmap = TerrainClipmap::new(self.clipmap.config())
            .expect("an already validated terrain clipmap config remains valid");
        self.admission.source_reset();
        self.pending.clear();
        self.cpu_source_generation = self.cpu_source_generation.wrapping_add(1);
        for slot in &mut self.slots {
            slot.clear_vegetation();
            slot.clear_exact_connectors();
            slot.clear_frontier_connectors();
        }
        self.renderer.exact_coverage.disable();
        self.exact_coverage_snapshot = None;
        self.exact_coverage_stabilizing = false;
        self.exact_topology = HorizontalTopology::UNBOUNDED;
        self.frontier_plan = None;
        self.frontier_receipt = TerrainFrontierPlanReceipt::default();
        self.frontier_topology = None;
        self.frontier_topology_receipt = TerrainFrontierTopologyReceipt::default();
        self.frontier_support = None;
        self.frontier_support_pending = None;
        self.frontier_observer_chunk = [0, 0];
        self.tree_ownership = None;
        self.exact_owned_tree_ids.clear();
        self.forest_reveal_clock = terrain_horizon_reveal_now();
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        config: TerrainClipmapConfig,
        vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
    ) -> Result<Self, String> {
        Self::new_with_target_color_transform(
            device,
            queue,
            color_format,
            width,
            height,
            material_atlas,
            config,
            vegetation_executor,
            RenderTargetColorTransform::Identity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_target_color_transform(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        config: TerrainClipmapConfig,
        vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
        target_color_transform: RenderTargetColorTransform,
    ) -> Result<Self, String> {
        Self::new_with_target_color_transform_and_cell_stride(
            device,
            queue,
            color_format,
            width,
            height,
            material_atlas,
            config,
            1,
            TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING,
            vegetation_executor,
            target_color_transform,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_target_color_transform_and_cell_stride(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        config: TerrainClipmapConfig,
        render_cell_stride: u32,
        vegetation_max_sample_spacing: u32,
        vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
        target_color_transform: RenderTargetColorTransform,
    ) -> Result<Self, String> {
        Self::new_for_profile_with_target_color_transform_and_cell_stride(
            device,
            queue,
            color_format,
            width,
            height,
            material_atlas,
            config,
            render_cell_stride,
            vegetation_max_sample_spacing,
            vegetation_executor,
            target_color_transform,
            TerrainPreviewProfile::McloneOverworldV1,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_for_profile_with_target_color_transform_and_cell_stride(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        material_atlas: TerrainPreviewMaterialAtlas<'_>,
        config: TerrainClipmapConfig,
        render_cell_stride: u32,
        vegetation_max_sample_spacing: u32,
        vegetation_executor: Option<Box<dyn TerrainVegetationExecutor>>,
        target_color_transform: RenderTargetColorTransform,
        profile: TerrainPreviewProfile,
    ) -> Result<Self, String> {
        if render_cell_stride == 0
            || !render_cell_stride.is_power_of_two()
            || TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS % render_cell_stride != 0
        {
            return Err(format!(
                "terrain horizon render cell stride {render_cell_stride} must divide \
                {TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS}"
            ));
        }
        if render_cell_stride > super::TERRAIN_HORIZON_MAX_PROVEN_RENDER_CELL_STRIDE {
            return Err(format!(
                "terrain horizon render cell stride {render_cell_stride} exceeds the proven \
                 geometry/normal transition contract {}",
                super::TERRAIN_HORIZON_MAX_PROVEN_RENDER_CELL_STRIDE,
            ));
        }
        let vegetation_record_limit = terrain_preview_max_tree_record_sample_spacing(profile);
        if !vegetation_max_sample_spacing.is_power_of_two()
            || vegetation_max_sample_spacing < config.base_sample_spacing
            || vegetation_max_sample_spacing > vegetation_record_limit
        {
            return Err(format!(
                "terrain horizon vegetation spacing {vegetation_max_sample_spacing} is outside \
                 the profile record range through {vegetation_record_limit}"
            ));
        }
        let clipmap = TerrainClipmap::new(config)?;
        let config = clipmap.config();
        let renderer = TerrainViewportRenderer::new_horizon_with_target_color_transform(
            device,
            queue,
            color_format,
            width,
            height,
            material_atlas,
            target_color_transform,
            render_cell_stride,
        )?;
        let admission = TerrainHorizonAdmission::new(config.level_count, config.slots_per_level())?;
        let mut slots = Vec::with_capacity(config.allocation_slots() as usize);
        let horizon_normal_height_byte_len = terrain_horizon_normal_height_byte_len()?;
        let resource_slots_per_level = config
            .slots_per_level()
            .checked_add(TERRAIN_HORIZON_STAGING_SLOTS_PER_LEVEL)
            .ok_or("terrain horizon resource slots per level overflow")?;
        for level in 0..config.level_count {
            for resource in 0..resource_slots_per_level {
                slots.push(TerrainViewportGpuTile::new_gpu_only(
                    device,
                    &renderer.compute_layout,
                    &renderer.render_layout,
                    renderer.sample_byte_len,
                    horizon_normal_height_byte_len,
                    TerrainViewportTileId {
                        profile,
                        seed: 0,
                        tile_x: (resource % config.tiles_per_axis) as i32,
                        tile_z: (resource / config.tiles_per_axis) as i32,
                        sample_spacing: config.sample_spacing(level),
                        content_stage: TerrainPreviewContentStage::Cover,
                        surface_quality: TerrainPreviewSurfaceQuality::Inferred,
                    },
                )?);
            }
        }
        debug_assert_eq!(slots.len(), admission.resource_slots() as usize);
        let resource_slot_count = slots.len();
        #[cfg(not(target_arch = "wasm32"))]
        let cpu_compiler = if profile.supports_gpu_lod() {
            None
        } else {
            Some(TerrainHorizonCpuCompiler::new()?)
        };
        Ok(Self {
            renderer,
            clipmap,
            slots,
            admission,
            pending: VecDeque::with_capacity(config.allocation_slots() as usize),
            #[cfg(not(target_arch = "wasm32"))]
            cpu_compiler,
            cpu_source_generation: 1,
            cpu_compile_in_flight: 0,
            cpu_compile_submitted_total: 0,
            cpu_compile_completed_total: 0,
            cpu_compile_micros_total: 0,
            cpu_compile_stale_results_total: 0,
            vegetation_max_sample_spacing,
            vegetation_executor,
            vegetation_coordinator: None,
            vegetation_error: None,
            profile,
            seed: 0,
            requested_center_x: 0,
            requested_center_z: 0,
            content_stage: TerrainPreviewContentStage::Cover,
            dispatched_refills_total: 0,
            exact_coverage_snapshot: None,
            exact_coverage_stabilizing: false,
            exact_topology: HorizontalTopology::UNBOUNDED,
            frontier_plan: None,
            frontier_receipt: TerrainFrontierPlanReceipt::default(),
            frontier_plan_failures: 0,
            frontier_topology: None,
            frontier_topology_receipt: TerrainFrontierTopologyReceipt::default(),
            frontier_topology_failures: 0,
            frontier_support: None,
            frontier_support_pending: None,
            frontier_support_dispatches_total: 0,
            frontier_admission: TerrainFrontierAdmissionTracker::default(),
            frontier_observer_chunk: [0, 0],
            authoritative_tree_ownership: false,
            tree_ownership: None,
            exact_owned_tree_ids: BTreeSet::new(),
            visible_terrain_slots: vec![false; resource_slot_count],
            visible_vegetation_slots: vec![false; resource_slot_count],
            forest_reveal_clock: terrain_horizon_reveal_now(),
        })
    }

    pub const fn config(&self) -> TerrainClipmapConfig {
        self.clipmap.config()
    }

    pub const fn diagnostics(&self) -> TerrainClipmapDiagnostics {
        self.clipmap.diagnostics()
    }

    /// Resize only the clipmap's outer levels and proxy-tree bound.
    ///
    /// Presets keep tile width, base spacing, and render stride stable, so
    /// common resource pools and their committed products retain identity.
    pub fn reconfigure_lod(
        &mut self,
        device: &wgpu::Device,
        config: TerrainClipmapConfig,
        vegetation_max_sample_spacing: u32,
    ) -> Result<bool, String> {
        let config = config.validate()?;
        let current = self.clipmap.config();
        if config.tiles_per_axis != current.tiles_per_axis
            || config.base_sample_spacing != current.base_sample_spacing
        {
            return Err(
                "live terrain LOD reconfiguration may change only the outer level bound".to_owned(),
            );
        }
        let vegetation_record_limit = terrain_preview_max_tree_record_sample_spacing(self.profile);
        if !vegetation_max_sample_spacing.is_power_of_two()
            || vegetation_max_sample_spacing < config.base_sample_spacing
            || vegetation_max_sample_spacing > vegetation_record_limit
        {
            return Err(format!(
                "terrain horizon vegetation spacing {vegetation_max_sample_spacing} is outside \
                 the profile record range through {vegetation_record_limit}"
            ));
        }
        if config == current && vegetation_max_sample_spacing == self.vegetation_max_sample_spacing
        {
            return Ok(false);
        }

        let mut next_clipmap = self.clipmap.clone();
        next_clipmap.reconfigure_level_count(config.level_count)?;
        let mut next_admission = self.admission.clone();
        next_admission.resize_levels(config.level_count, config.slots_per_level())?;

        let mut added_slots = Vec::new();
        if config.level_count > current.level_count {
            let horizon_normal_height_byte_len = terrain_horizon_normal_height_byte_len()?;
            let resource_slots_per_level = config
                .slots_per_level()
                .checked_add(TERRAIN_HORIZON_STAGING_SLOTS_PER_LEVEL)
                .ok_or("terrain horizon resource slots per level overflow")?;
            added_slots.reserve(
                config
                    .level_count
                    .saturating_sub(current.level_count)
                    .saturating_mul(resource_slots_per_level) as usize,
            );
            for level in current.level_count..config.level_count {
                for resource in 0..resource_slots_per_level {
                    added_slots.push(TerrainViewportGpuTile::new_gpu_only(
                        device,
                        &self.renderer.compute_layout,
                        &self.renderer.render_layout,
                        self.renderer.sample_byte_len,
                        horizon_normal_height_byte_len,
                        TerrainViewportTileId {
                            profile: self.profile,
                            seed: 0,
                            tile_x: (resource % config.tiles_per_axis) as i32,
                            tile_z: (resource / config.tiles_per_axis) as i32,
                            sample_spacing: config.sample_spacing(level),
                            content_stage: TerrainPreviewContentStage::Cover,
                            surface_quality: TerrainPreviewSurfaceQuality::Inferred,
                        },
                    )?);
                }
            }
        }

        self.clipmap = next_clipmap;
        self.admission = next_admission;
        self.pending
            .retain(|resource| resource.tile.level < config.level_count);
        if config.level_count < current.level_count {
            self.slots
                .truncate(self.admission.resource_slots() as usize);
        } else {
            self.slots.extend(added_slots);
        }
        debug_assert_eq!(self.slots.len(), self.admission.resource_slots() as usize);
        self.visible_terrain_slots.resize(self.slots.len(), false);
        self.visible_vegetation_slots
            .resize(self.slots.len(), false);
        self.vegetation_max_sample_spacing = vegetation_max_sample_spacing;
        if let Some(coordinator) = &mut self.vegetation_coordinator {
            coordinator.reconfigure_maximum_desired_tiles(
                maximum_terrain_vegetation_desired_tiles(config, vegetation_max_sample_spacing)?,
            )?;
        }
        self.admission
            .clear_vegetation_above_sample_spacing(vegetation_max_sample_spacing);
        for slot in &mut self.slots {
            if slot.request.request().sample_spacing > vegetation_max_sample_spacing {
                slot.clear_vegetation();
            }
        }
        self.refresh_after_lod_reconfiguration()?;
        Ok(true)
    }

    fn refresh_after_lod_reconfiguration(&mut self) -> Result<(), String> {
        if self.admission.has_staged_levels() {
            self.refresh_vegetation_desired(self.requested_center_x, self.requested_center_z)
        } else {
            self.schedule_requested_transition()
        }
    }

    pub fn set_view(
        &mut self,
        seed: i64,
        center_x: i32,
        center_z: i32,
        content_stage: TerrainPreviewContentStage,
    ) {
        let source_changed = self.seed != seed || self.content_stage != content_stage;
        if source_changed {
            self.reset_source();
        }
        self.seed = seed;
        self.requested_center_x = center_x;
        self.requested_center_z = center_z;
        self.content_stage = content_stage;
        if let Err(error) = self.schedule_requested_transition() {
            self.vegetation_error = Some(error);
        }
    }

    pub fn set_exact_painted_coverage(
        &mut self,
        queue: &wgpu::Queue,
        snapshot: &ExactPaintedCoverageSnapshot,
        transition: &TerrainExactTransitionField,
        boundary: &TerrainExactBoundaryProfile,
        mode: TerrainExactCoverageMode,
        topology: HorizontalTopology,
    ) -> Result<(), String> {
        let expected = TerrainCompositionSourceIdentity::new(self.profile, self.seed);
        if snapshot.source() != expected {
            return Err(format!(
                "exact-painted coverage source {:?} does not match horizon source {:?}",
                snapshot.source(),
                expected
            ));
        }
        let frontier_changed = self.exact_coverage_snapshot.as_ref() != Some(snapshot)
            || self.renderer.exact_coverage.boundary != *boundary
            || self.exact_topology != topology;
        self.renderer
            .exact_coverage
            .set_snapshot(queue, snapshot, transition, boundary, mode)?;
        self.exact_coverage_snapshot = Some(snapshot.clone());
        self.exact_topology = topology;
        if frontier_changed {
            self.frontier_plan = None;
            self.frontier_receipt = TerrainFrontierPlanReceipt::default();
            self.frontier_topology = None;
            self.frontier_topology_receipt = TerrainFrontierTopologyReceipt::default();
            if self.frontier_support_pending.take().is_some() {
                self.frontier_admission.record_coalesced_generation();
            }
        }
        Ok(())
    }

    pub fn set_exact_coverage_stabilizing(&mut self, stabilizing: bool) {
        self.exact_coverage_stabilizing = stabilizing;
    }

    pub fn clear_exact_painted_coverage(&mut self) {
        self.renderer.exact_coverage.disable();
        self.exact_coverage_snapshot = None;
        self.exact_coverage_stabilizing = false;
        self.frontier_plan = None;
        self.frontier_receipt = TerrainFrontierPlanReceipt::default();
        self.frontier_topology = None;
        self.frontier_topology_receipt = TerrainFrontierTopologyReceipt::default();
        self.frontier_support = None;
        self.frontier_support_pending = None;
    }

    fn refresh_frontier_plan(
        &mut self,
        levels: &[TerrainHorizonLevelPresentation],
    ) -> Result<(), String> {
        if self.renderer.exact_coverage.mode == TerrainExactCoverageMode::Disabled {
            self.frontier_plan = None;
            self.frontier_receipt = TerrainFrontierPlanReceipt::default();
            self.frontier_topology = None;
            self.frontier_topology_receipt = TerrainFrontierTopologyReceipt::default();
            return Ok(());
        }
        let Some(coverage) = self.exact_coverage_snapshot.as_ref() else {
            self.frontier_plan = None;
            self.frontier_receipt = TerrainFrontierPlanReceipt {
                enabled: true,
                state: TerrainFrontierPlanState::Invalid,
                ..Default::default()
            };
            self.frontier_topology = None;
            self.frontier_topology_receipt = TerrainFrontierTopologyReceipt::default();
            self.frontier_plan_failures = self.frontier_plan_failures.saturating_add(1);
            return Ok(());
        };
        let snapshots = levels
            .iter()
            .map(|level| level.snapshot.clone())
            .collect::<Vec<_>>();
        let presentation = terrain_frontier_presentation_identity(&snapshots);
        let observer_chunk = [
            i64::from(self.requested_center_x.div_euclid(16)),
            i64::from(self.requested_center_z.div_euclid(16)),
        ];
        let observer_changed =
            !self.exact_topology.is_unbounded() && self.frontier_observer_chunk != observer_chunk;
        if self.frontier_receipt.enabled
            && self.frontier_receipt.exact_generation == coverage.generation()
            && self.frontier_receipt.presentation == presentation
            && !observer_changed
        {
            return Ok(());
        }
        let observer_blocks = [
            observer_chunk[0].saturating_mul(16).saturating_add(8),
            observer_chunk[1].saturating_mul(16).saturating_add(8),
        ];
        match TerrainFrontierPlan::prepare(
            coverage,
            &self.renderer.exact_coverage.boundary,
            self.exact_topology,
            observer_blocks,
            &snapshots,
            TerrainFrontierPlanOptions::default(),
        ) {
            Ok(plan) => {
                self.frontier_receipt = plan.receipt();
                match TerrainFrontierTopology::prepare(
                    &plan,
                    TerrainFrontierTopologyOptions::default(),
                ) {
                    Ok(topology) => {
                        self.frontier_topology_receipt = topology.receipt();
                        self.frontier_topology = Some(topology);
                    }
                    Err(error) => {
                        log::warn!("terrain frontier topology rejected: {error}");
                        self.frontier_topology = None;
                        self.frontier_topology_receipt = TerrainFrontierTopologyReceipt::default();
                        self.frontier_topology_failures =
                            self.frontier_topology_failures.saturating_add(1);
                    }
                }
                self.frontier_plan = Some(plan);
            }
            Err(error) => {
                log::warn!("terrain frontier plan rejected: {error}");
                self.frontier_plan = None;
                self.frontier_topology = None;
                self.frontier_topology_receipt = TerrainFrontierTopologyReceipt::default();
                self.frontier_receipt = TerrainFrontierPlanReceipt {
                    enabled: true,
                    state: TerrainFrontierPlanState::Invalid,
                    exact_generation: coverage.generation(),
                    presentation,
                    ..Default::default()
                };
                self.frontier_plan_failures = self.frontier_plan_failures.saturating_add(1);
            }
        }
        self.frontier_observer_chunk = observer_chunk;
        Ok(())
    }

    fn disable_frontier_support(&mut self, queue: &wgpu::Queue) -> Result<(), String> {
        self.frontier_support = None;
        self.frontier_support_pending = None;
        self.renderer
            .exact_coverage
            .set_frontier_support_tiles(queue, &BTreeSet::new())?;
        for slot in &mut self.slots {
            slot.clear_frontier_connectors();
        }
        Ok(())
    }

    fn refresh_frontier_topology_capacity(&mut self, support_pool_capacity: u32) {
        if self.frontier_topology_receipt.support_pool_capacity == support_pool_capacity {
            return;
        }
        let Some(plan) = self.frontier_plan.as_ref() else {
            return;
        };
        match TerrainFrontierTopology::prepare(
            plan,
            TerrainFrontierTopologyOptions {
                fine_tile_capacity: support_pool_capacity,
            },
        ) {
            Ok(topology) => {
                self.frontier_topology_receipt = topology.receipt();
                self.frontier_topology = Some(topology);
                if self.frontier_support_pending.take().is_some() {
                    self.frontier_admission.record_coalesced_generation();
                }
            }
            Err(_error) => {
                self.frontier_topology = None;
                self.frontier_topology_receipt = TerrainFrontierTopologyReceipt::default();
                self.frontier_support_pending = None;
                self.frontier_topology_failures = self.frontier_topology_failures.saturating_add(1);
            }
        }
    }

    fn build_frontier_support(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        topology: &TerrainFrontierTopology,
    ) -> Result<TerrainFrontierSupportGpu, String> {
        if topology.receipt().state != TerrainFrontierTopologyState::Complete {
            return Err("cannot build an incomplete frontier certificate".to_owned());
        }
        let identity = TerrainFrontierSupportGpuIdentity {
            seed: self.seed,
            content_stage: self.content_stage,
            exact_generation: topology.receipt().exact_generation,
            presentation: topology.receipt().presentation,
            support_pool_capacity: topology.receipt().support_pool_capacity,
            selected_tiles: topology.selected_support_tiles().clone(),
        };
        let connector_instances = terrain_frontier_connector_instances(topology)?;
        let normal_height_byte_len = terrain_horizon_normal_height_byte_len()?;
        let mut tiles = Vec::with_capacity(identity.selected_tiles.len());
        for key in &identity.selected_tiles {
            let tile_x = i32::try_from(key.tile_x)
                .map_err(|_| "frontier support tile X exceeds shader coordinates")?;
            let tile_z = i32::try_from(key.tile_z)
                .map_err(|_| "frontier support tile Z exceeds shader coordinates")?;
            let mut outer_edge_flags = 0;
            for edge in topology
                .outer_edges()
                .iter()
                .filter(|edge| edge.tile == *key)
            {
                outer_edge_flags |= match edge.direction {
                    TerrainFrontierDirection::West => TERRAIN_HORIZON_NORMAL_EDGE_WEST,
                    TerrainFrontierDirection::East => TERRAIN_HORIZON_NORMAL_EDGE_EAST,
                    TerrainFrontierDirection::North => TERRAIN_HORIZON_NORMAL_EDGE_NORTH,
                    TerrainFrontierDirection::South => TERRAIN_HORIZON_NORMAL_EDGE_SOUTH,
                };
            }
            let mut tile = TerrainViewportGpuTile::new_gpu_only(
                device,
                &self.renderer.compute_layout,
                &self.renderer.render_layout,
                self.renderer.sample_byte_len,
                normal_height_byte_len,
                TerrainViewportTileId {
                    profile: self.profile,
                    seed: self.seed,
                    tile_x,
                    tile_z,
                    sample_spacing: 1,
                    content_stage: self.content_stage,
                    surface_quality: TerrainPreviewSurfaceQuality::Inferred,
                },
            )?;
            tile.refresh_frontier_connectors(
                device,
                queue,
                identity.exact_generation,
                identity.presentation,
                &connector_instances,
            );
            tiles.push(TerrainFrontierSupportGpuTile {
                key: *key,
                tile,
                ready: false,
                outer_edge_flags,
            });
        }
        let committed = tiles.is_empty();
        Ok(TerrainFrontierSupportGpu {
            identity,
            tiles,
            connector_instances,
            committed,
            dispatched_total: 0,
        })
    }

    fn ensure_frontier_fallback(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), String> {
        let Some(plan) = self.frontier_plan.as_ref() else {
            return self.disable_frontier_support(queue);
        };
        let fallback = TerrainFrontierTopology::prepare(
            plan,
            TerrainFrontierTopologyOptions {
                fine_tile_capacity: 0,
            },
        )?;
        let receipt = fallback.receipt();
        if receipt.state != TerrainFrontierTopologyState::Complete {
            return Err("frontier fallback could not certify the exact boundary".to_owned());
        }
        let active_matches = self.frontier_support.as_ref().is_some_and(|support| {
            support.committed
                && support.identity.exact_generation == receipt.exact_generation
                && support.identity.presentation == receipt.presentation
        });
        if active_matches {
            return Ok(());
        }
        let support = self.build_frontier_support(device, queue, &fallback)?;
        debug_assert!(support.committed);
        self.commit_frontier_support(queue, support)?;
        self.frontier_admission.record_fallback_commit();
        Ok(())
    }

    fn commit_frontier_support(
        &mut self,
        queue: &wgpu::Queue,
        support: TerrainFrontierSupportGpu,
    ) -> Result<(), String> {
        self.renderer
            .exact_coverage
            .set_frontier_support_tiles(queue, &support.identity.selected_tiles)?;
        // Connector buffers are tile-owned. An epoch switch must release
        // invisible slots from the prior epoch instead of waiting for those
        // slots to become visible and refresh lazily.
        for slot in &mut self.slots {
            slot.clear_frontier_connectors();
        }
        self.frontier_support = Some(support);
        Ok(())
    }

    fn prepare_frontier_support(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), String> {
        let Some(topology) = self.frontier_topology.as_ref() else {
            return Ok(());
        };
        if topology.receipt().state != TerrainFrontierTopologyState::Complete {
            return Ok(());
        }
        let receipt = topology.receipt();
        let desired_matches = |support: &TerrainFrontierSupportGpu| {
            support.identity.exact_generation == receipt.exact_generation
                && support.identity.presentation == receipt.presentation
                && support.identity.support_pool_capacity == receipt.support_pool_capacity
                && support.identity.selected_tiles == *topology.selected_support_tiles()
        };
        if self
            .frontier_support
            .as_ref()
            .is_some_and(|support| support.committed && desired_matches(support))
            || self
                .frontier_support_pending
                .as_ref()
                .is_some_and(desired_matches)
        {
            return Ok(());
        }
        let pending = self.build_frontier_support(device, queue, topology)?;
        if self.frontier_support_pending.replace(pending).is_some() {
            self.frontier_admission.record_coalesced_generation();
        }
        Ok(())
    }

    fn frontier_admission_receipt(
        &self,
        exact_frontier_required: bool,
        warming: bool,
    ) -> TerrainFrontierAdmissionReceipt {
        let resource = |support: &TerrainFrontierSupportGpu| TerrainFrontierAdmissionResource {
            committed: support.committed,
            exact_generation: support.identity.exact_generation,
            presentation: support.identity.presentation,
            support_capacity: support.identity.support_pool_capacity,
            support_tiles: support.tiles.len().try_into().unwrap_or(u32::MAX),
        };
        self.frontier_admission.receipt(
            exact_frontier_required,
            warming,
            self.frontier_support.as_ref().map(resource),
            self.frontier_support_pending.as_ref().map(resource),
        )
    }

    pub fn set_authoritative_tree_ownership(&mut self, enabled: bool) {
        self.authoritative_tree_ownership = enabled;
    }

    pub const fn authoritative_tree_ownership(&self) -> bool {
        self.authoritative_tree_ownership
    }

    pub fn set_tree_ownership(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        snapshot: &BoundedRepresentationOwnershipSnapshot<McloneTreeOccurrenceId>,
    ) -> Result<(), String> {
        let expected = TerrainCompositionSourceIdentity::new(self.profile, self.seed);
        if snapshot.source() != expected {
            return Err(format!(
                "tree ownership source {:?} does not match horizon source {:?}",
                snapshot.source(),
                expected
            ));
        }
        let coverage_generation = self.renderer.exact_coverage.mask.generation;
        if self.renderer.exact_coverage.mode != TerrainExactCoverageMode::Disabled
            && snapshot.generation() != coverage_generation
        {
            return Err(format!(
                "tree ownership generation {} does not match exact coverage generation \
                 {coverage_generation}",
                snapshot.generation()
            ));
        }
        let exact_owned_tree_ids = snapshot.exact_owned_ids().copied().collect::<BTreeSet<_>>();
        if self.tree_ownership.as_ref() == Some(snapshot)
            && self.exact_owned_tree_ids == exact_owned_tree_ids
        {
            return Ok(());
        }
        let changed = self
            .exact_owned_tree_ids
            .symmetric_difference(&exact_owned_tree_ids)
            .copied()
            .collect::<BTreeSet<_>>();
        for slot in &mut self.slots {
            if slot.vegetation.as_ref().is_some_and(|vegetation| {
                vegetation
                    .occurrences()
                    .iter()
                    .any(|occurrence| changed.contains(&McloneTreeOccurrenceId::from(*occurrence)))
            }) {
                slot.refresh_tree_instances(device, queue, &exact_owned_tree_ids)?;
            }
        }
        self.tree_ownership = Some(snapshot.clone());
        self.exact_owned_tree_ids = exact_owned_tree_ids;
        Ok(())
    }

    pub fn clear_tree_ownership(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), String> {
        if self.tree_ownership.is_none() && self.exact_owned_tree_ids.is_empty() {
            return Ok(());
        }
        self.exact_owned_tree_ids.clear();
        for slot in &mut self.slots {
            if slot.tree_suppressed_instance_count > 0 {
                slot.refresh_tree_instances(device, queue, &self.exact_owned_tree_ids)?;
            }
        }
        self.tree_ownership = None;
        Ok(())
    }

    fn schedule_requested_transition(&mut self) -> Result<(), String> {
        if self.admission.has_staged_levels() {
            return Ok(());
        }
        let mut requested_clipmap = self.clipmap.clone();
        let update =
            requested_clipmap.update_center(self.requested_center_x, self.requested_center_z);
        let (transition, entering) = self
            .admission
            .begin_transition(&requested_clipmap.levels(), &update.rebased_levels)?;
        if transition == TerrainHorizonBeginTransition::Deferred {
            return Ok(());
        }
        self.clipmap = requested_clipmap;
        if transition == TerrainHorizonBeginTransition::Started {
            for resource in entering {
                self.slots[resource.resource_slot as usize].clear_vegetation();
                self.pending.push_back(resource);
            }
        }
        self.refresh_vegetation_desired(self.requested_center_x, self.requested_center_z)
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.renderer.resize(device, width, height);
    }

    pub fn set_depth_capture_enabled(&mut self, enabled: bool) {
        self.renderer.set_depth_capture_enabled(enabled);
    }

    pub fn copy_depth_to_buffer(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
        bytes_per_row: u32,
    ) -> Result<(), String> {
        self.renderer
            .copy_depth_to_buffer(encoder, destination, bytes_per_row)
    }

    pub fn shutdown_vegetation(&mut self) {
        if let Some(coordinator) = self.vegetation_coordinator.as_mut() {
            coordinator.shutdown();
        }
    }

    pub fn vegetation_shutdown_complete(&self) -> bool {
        self.vegetation_coordinator
            .as_ref()
            .is_none_or(|coordinator| {
                coordinator.state() == TerrainVegetationCoordinatorState::Terminated
            })
    }

    fn pending_terrain_refills(&self) -> u32 {
        u32::try_from(self.pending.len())
            .unwrap_or(u32::MAX)
            .saturating_add(self.cpu_compile_in_flight)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn admit_completed_cpu_refills(&mut self, queue: &wgpu::Queue) -> Result<u32, String> {
        let mut admitted = 0_u32;
        loop {
            let result = match self.cpu_compiler.as_mut() {
                Some(compiler) => compiler.try_recv()?,
                None => None,
            };
            let Some(result) = result else {
                break;
            };
            self.cpu_compile_in_flight = self.cpu_compile_in_flight.saturating_sub(1);
            self.cpu_compile_completed_total = self.cpu_compile_completed_total.saturating_add(1);
            self.cpu_compile_micros_total = self
                .cpu_compile_micros_total
                .saturating_add(result.compile_micros);

            let resource = result.resource;
            let expected_tile =
                terrain_horizon_tile_id(self.profile, self.seed, self.content_stage, resource.tile);
            if result.source_generation != self.cpu_source_generation
                || result.request != expected_tile.preview_request()
                || self.admission.assignment(resource.resource_slot) != Some(resource.tile)
                || self.admission.slot_generation(resource.resource_slot)
                    != Some(resource.slot_generation)
            {
                self.cpu_compile_stale_results_total =
                    self.cpu_compile_stale_results_total.saturating_add(1);
                continue;
            }
            let (reference, height_halo) = result.compiled?;
            self.slots[resource.resource_slot as usize].upload_horizon_reference(
                queue,
                self.renderer.sample_byte_len,
                reference,
                &height_halo,
            )?;
            self.admission.mark_ready(resource.resource_slot)?;
            admitted = admitted.saturating_add(1);
        }
        Ok(admitted)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn submit_cpu_refills(&mut self) -> Result<(), String> {
        loop {
            let Some(resource) = self.pending.front().copied() else {
                break;
            };
            if self.admission.assignment(resource.resource_slot) != Some(resource.tile)
                || self.admission.slot_generation(resource.resource_slot)
                    != Some(resource.slot_generation)
            {
                self.pending.pop_front();
                continue;
            }
            let request =
                terrain_horizon_tile_id(self.profile, self.seed, self.content_stage, resource.tile)
                    .preview_request();
            let job = TerrainHorizonCpuCompileJob {
                source_generation: self.cpu_source_generation,
                resource,
                request,
            };
            let submitted = self
                .cpu_compiler
                .as_mut()
                .ok_or("continental horizon has no CPU compiler")?
                .try_submit(job)?;
            if !submitted {
                break;
            }
            self.pending.pop_front();
            self.cpu_compile_in_flight = self.cpu_compile_in_flight.saturating_add(1);
            self.cpu_compile_submitted_total = self.cpu_compile_submitted_total.saturating_add(1);
        }
        Ok(())
    }

    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        presentation: TerrainHorizonPresentation,
    ) -> Result<TerrainHorizonFrameStats, String> {
        self.encode_impl(
            device,
            queue,
            encoder,
            color_view,
            None,
            wgpu::LoadOp::Clear(self.renderer.clear_color),
            wgpu::StoreOp::Store,
            wgpu::LoadOp::Clear(0.0),
            None,
            width,
            height,
            presentation,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode_to_target(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: TerrainHorizonRenderTarget<'_>,
        width: u32,
        height: u32,
        presentation: TerrainHorizonPresentation,
    ) -> Result<TerrainHorizonFrameStats, String> {
        self.encode_impl(
            device,
            queue,
            encoder,
            target.color_view,
            Some(target.depth_view),
            target.color_load,
            target.color_store,
            target.depth_load,
            Some(target.depth_store),
            width,
            height,
            presentation,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode_multiview_to_target(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: TerrainHorizonRenderTarget<'_>,
        width: u32,
        height: u32,
        presentation: TerrainHorizonPresentation,
    ) -> Result<TerrainHorizonFrameStats, String> {
        if presentation.multiview_render_view_override.is_none() {
            return Err("terrain horizon multiview requires two render views".to_owned());
        }
        if self.renderer.horizon_multiview_render_pipeline.is_none()
            || self.renderer.tree_multiview_pipeline.is_none()
            || self.renderer.canopy_multiview_pipeline.is_none()
        {
            return Err("terrain horizon multiview pipelines are unavailable".to_owned());
        }
        self.encode_impl(
            device,
            queue,
            encoder,
            target.color_view,
            Some(target.depth_view),
            target.color_load,
            target.color_store,
            target.depth_load,
            Some(target.depth_store),
            width,
            height,
            presentation,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_impl(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        external_depth_view: Option<&wgpu::TextureView>,
        color_load: wgpu::LoadOp<wgpu::Color>,
        color_store: wgpu::StoreOp,
        depth_load: wgpu::LoadOp<f32>,
        depth_store: Option<wgpu::StoreOp>,
        width: u32,
        height: u32,
        presentation: TerrainHorizonPresentation,
        multiview: bool,
    ) -> Result<TerrainHorizonFrameStats, String> {
        self.resize(device, width, height);
        let reveal_now = terrain_horizon_reveal_now();
        let reveal_elapsed =
            terrain_horizon_reveal_elapsed_seconds(self.forest_reveal_clock, reveal_now);
        self.forest_reveal_clock = reveal_now;
        for slot in &mut self.slots {
            slot.advance_forest_reveal(reveal_elapsed);
        }
        self.renderer.exact_coverage.sync(queue);
        if !self.admission.has_staged_levels()
            && (self.clipmap.center() != (self.requested_center_x, self.requested_center_z)
                || !self.clipmap.origins_settled())
            && let Err(error) = self.schedule_requested_transition()
        {
            self.vegetation_error = Some(error);
        }
        let uniform_presentation = presentation.uniform_facts()?;
        let render_view_overrides = if multiview {
            presentation
                .multiview_render_view_override
                .expect("multiview presentation was validated")
                .map(Some)
        } else {
            [presentation.render_view_override, None]
        };
        let options = TerrainPreviewDrawOptions {
            source: TerrainPreviewSource::Gpu,
            view: presentation.view,
            layer: TerrainPreviewLayer::Terrain,
            split_layout: TerrainPreviewSplitLayout::Columns,
        };
        let focus_y = presentation.target_y;
        let mut dispatched_refills = 0_u32;
        if self.profile.supports_gpu_lod() {
            for _ in 0..TERRAIN_VIEWPORT_GPU_DISPATCHES_PER_FRAME {
                let Some(resource) = self.pending.pop_front() else {
                    break;
                };
                let tile = resource.tile;
                let slot_index = resource.resource_slot as usize;
                if self.admission.assignment(resource.resource_slot) != Some(tile)
                    || self.admission.slot_generation(resource.resource_slot)
                        != Some(resource.slot_generation)
                {
                    continue;
                }
                let tile_id =
                    terrain_horizon_tile_id(self.profile, self.seed, self.content_stage, tile);
                let slot = &mut self.slots[slot_index];
                slot.request = tile_id.preview_request().validate()?;
                queue.write_buffer(
                    &slot.uniform_buffer,
                    0,
                    &terrain_horizon_uniform_bytes(
                        slot.request,
                        width,
                        height,
                        options,
                        presentation.camera,
                        uniform_presentation,
                        focus_y,
                        None,
                        0,
                        if multiview { 0b11 } else { 0b01 },
                        render_view_overrides,
                        presentation.sky_darken,
                        presentation.fog,
                        [1.0, 1.0],
                        presentation.diagnostic,
                    ),
                );
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("mclone_terrain_horizon_compute_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(
                    self.renderer
                        .horizon_compute_pipeline
                        .as_ref()
                        .expect("horizon renderer owns its compute pipeline"),
                );
                pass.set_bind_group(0, &slot.compute_bind_group, &[]);
                let workgroups =
                    terrain_horizon_samples_per_axis().div_ceil(TERRAIN_PREVIEW_WORKGROUP_AXIS);
                pass.dispatch_workgroups(workgroups, workgroups, 1);
                slot.gpu_submitted = true;
                self.admission.mark_ready(resource.resource_slot)?;
                dispatched_refills = dispatched_refills.saturating_add(1);
            }
        } else {
            #[cfg(not(target_arch = "wasm32"))]
            {
                dispatched_refills = self.admit_completed_cpu_refills(queue)?;
                self.submit_cpu_refills()?;
            }
            #[cfg(target_arch = "wasm32")]
            for _ in 0..1 {
                let Some(resource) = self.pending.pop_front() else {
                    break;
                };
                let tile = resource.tile;
                if self.admission.assignment(resource.resource_slot) != Some(tile)
                    || self.admission.slot_generation(resource.resource_slot)
                        != Some(resource.slot_generation)
                {
                    continue;
                }
                let tile_id =
                    terrain_horizon_tile_id(self.profile, self.seed, self.content_stage, tile);
                let slot = &mut self.slots[resource.resource_slot as usize];
                slot.request = tile_id.preview_request().validate()?;
                let (reference, height_halo) =
                    TerrainPreviewReferenceGrid::compile_continental_with_height_halo(
                        tile_id.preview_request(),
                        TERRAIN_HORIZON_NORMAL_HALO_RADIUS,
                    )?;
                slot.upload_horizon_reference(
                    queue,
                    self.renderer.sample_byte_len,
                    reference,
                    &height_halo,
                )?;
                self.admission.mark_ready(resource.resource_slot)?;
                dispatched_refills = dispatched_refills.saturating_add(1);
            }
        }
        self.clipmap.note_refills_completed(dispatched_refills);
        self.dispatched_refills_total = self
            .dispatched_refills_total
            .saturating_add(u64::from(dispatched_refills));
        let prior_terrain_tiles = self
            .admission
            .terrain_presentations()
            .into_iter()
            .flat_map(|level| level.tiles)
            .map(|resource| terrain_horizon_semantic_tile_key(resource.tile))
            .collect::<HashSet<_>>();
        if self.admission.commit_ready_terrain() > 0 {
            for resource in self
                .admission
                .terrain_presentations()
                .into_iter()
                .flat_map(|level| level.tiles)
                .filter(|resource| {
                    !prior_terrain_tiles.contains(&terrain_horizon_semantic_tile_key(resource.tile))
                })
            {
                self.slots[resource.resource_slot as usize].reset_canopy_reveal();
            }
        }

        if let Some(error) = self.vegetation_error.take() {
            return Err(error);
        }
        if let Some(coordinator) = self.vegetation_coordinator.as_mut()
            && let Some(admission) = coordinator.pump()
        {
            let slot_index = admission.identity.slot.physical_slot as usize;
            if self
                .admission
                .slot_generation(admission.identity.slot.physical_slot)
                != Some(admission.identity.slot.slot_generation)
                || self
                    .admission
                    .assignment(admission.identity.slot.physical_slot)
                    .is_none_or(|tile| {
                        tile.tile_x != admission.identity.tile.tile_x
                            || tile.tile_z != admission.identity.tile.tile_z
                            || tile.sample_spacing != admission.identity.tile.sample_spacing
                    })
            {
                return Err(
                    "terrain vegetation coordinator admitted a mismatched physical slot".to_owned(),
                );
            }
            self.slots[slot_index].upload_vegetation_filtered(
                device,
                queue,
                admission.product,
                &self.exact_owned_tree_ids,
            )?;
        }
        let slots = &self.slots;
        let profile = self.profile;
        let seed = self.seed;
        let content_stage = self.content_stage;
        let prior_vegetation_tiles = self
            .admission
            .vegetation_presentations()
            .into_iter()
            .flat_map(|level| level.tiles)
            .map(|resource| terrain_horizon_semantic_tile_key(resource.tile))
            .collect::<HashSet<_>>();
        if self.admission.commit_ready_vegetation(|resource| {
            slots[resource.resource_slot as usize]
                .vegetation
                .as_ref()
                .is_some_and(|product| {
                    product.request().request()
                        == terrain_horizon_tile_id(profile, seed, content_stage, resource.tile)
                            .preview_request()
                })
        }) > 0
        {
            for resource in self
                .admission
                .vegetation_presentations()
                .into_iter()
                .flat_map(|level| level.tiles)
                .filter(|resource| {
                    !prior_vegetation_tiles
                        .contains(&terrain_horizon_semantic_tile_key(resource.tile))
                })
            {
                self.slots[resource.resource_slot as usize].reset_proxy_reveal();
            }
        }
        self.refresh_authoritative_tree_ownership(device, queue)?;

        let terrain_levels = self.admission.terrain_presentations();
        self.refresh_frontier_plan(&terrain_levels)?;
        let frontier_support_capacity =
            if presentation.diagnostic == TerrainHorizonDiagnostic::FrontierFallback {
                1
            } else {
                super::TERRAIN_FRONTIER_FINE_TILE_CAPACITY
            };
        self.refresh_frontier_topology_capacity(frontier_support_capacity);
        let exact_frontier_required = self.renderer.exact_coverage.mode
            != TerrainExactCoverageMode::Disabled
            && self
                .exact_coverage_snapshot
                .as_ref()
                .is_some_and(|coverage| !coverage.chunks().is_empty());
        let exact_frontier_certifiable = exact_frontier_required
            && self.frontier_topology_receipt.state == TerrainFrontierTopologyState::Complete;
        let frontier_warming = exact_frontier_warming(
            exact_frontier_required,
            exact_frontier_certifiable,
            self.exact_coverage_stabilizing,
            terrain_levels.is_empty(),
            self.pending_terrain_refills() > 0,
            self.admission.has_staged_levels(),
        );
        if exact_frontier_required && !exact_frontier_certifiable {
            self.disable_frontier_support(queue)?;
            if !frontier_warming {
                return Err(format!(
                    "exact terrain generation {} has no complete frontier certificate",
                    self.renderer.exact_coverage.mask.generation,
                ));
            }
        }
        if exact_frontier_certifiable {
            self.ensure_frontier_fallback(device, queue)?;
            if exact_frontier_preferred_support_allowed(
                exact_frontier_certifiable,
                self.exact_coverage_stabilizing,
            ) {
                self.prepare_frontier_support(device, queue)?;
            } else if self.frontier_support_pending.take().is_some() {
                // A preferred pool is an optimization behind the complete
                // zero-capacity fallback. Do not repeatedly allocate pools
                // for partial exact generations that the live view has
                // explicitly declared transient.
                self.frontier_admission.record_coalesced_generation();
            }
        } else {
            self.disable_frontier_support(queue)?;
        }
        let mut frontier_support_dispatches = 0_u32;
        if exact_frontier_certifiable
            && self.pending_terrain_refills() == 0
            && !self.admission.has_staged_levels()
            && let Some(support) = self.frontier_support_pending.as_mut()
        {
            let dispatch_limit = if self.profile.supports_gpu_lod() {
                TERRAIN_FRONTIER_DISPATCHES_PER_FRAME
            } else {
                1
            };
            for support_tile in support
                .tiles
                .iter_mut()
                .filter(|tile| !tile.ready)
                .take(dispatch_limit)
            {
                queue.write_buffer(
                    &support_tile.tile.uniform_buffer,
                    0,
                    &terrain_horizon_uniform_bytes(
                        support_tile.tile.request,
                        width,
                        height,
                        options,
                        presentation.camera,
                        uniform_presentation,
                        focus_y,
                        None,
                        support_tile.outer_edge_flags,
                        if multiview { 0b11 } else { 0b01 },
                        render_view_overrides,
                        presentation.sky_darken,
                        presentation.fog,
                        [1.0, 1.0],
                        presentation.diagnostic,
                    ),
                );
                if self.profile.supports_gpu_lod() {
                    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("mclone_terrain_frontier_support_compute_pass"),
                        timestamp_writes: None,
                    });
                    pass.set_pipeline(
                        self.renderer
                            .horizon_compute_pipeline
                            .as_ref()
                            .expect("horizon renderer owns its compute pipeline"),
                    );
                    pass.set_bind_group(0, &support_tile.tile.compute_bind_group, &[]);
                    let workgroups =
                        terrain_horizon_samples_per_axis().div_ceil(TERRAIN_PREVIEW_WORKGROUP_AXIS);
                    pass.dispatch_workgroups(workgroups, workgroups, 1);
                    support_tile.tile.gpu_submitted = true;
                } else {
                    let (reference, height_halo) =
                        TerrainPreviewReferenceGrid::compile_continental_with_height_halo(
                            support_tile.tile.request.request(),
                            TERRAIN_HORIZON_NORMAL_HALO_RADIUS,
                        )?;
                    support_tile.tile.upload_horizon_reference(
                        queue,
                        self.renderer.sample_byte_len,
                        reference,
                        &height_halo,
                    )?;
                }
                support_tile.ready = true;
                frontier_support_dispatches = frontier_support_dispatches.saturating_add(1);
            }
            support.dispatched_total = support
                .dispatched_total
                .saturating_add(u64::from(frontier_support_dispatches));
            if !support.committed && support.tiles.iter().all(|tile| tile.ready) {
                support.committed = true;
            }
        }
        self.frontier_support_dispatches_total = self
            .frontier_support_dispatches_total
            .saturating_add(u64::from(frontier_support_dispatches));
        if self
            .frontier_support_pending
            .as_ref()
            .is_some_and(|support| support.committed)
        {
            let support = self
                .frontier_support_pending
                .take()
                .expect("checked committed frontier support remains pending");
            self.commit_frontier_support(queue, support)?;
            self.frontier_admission.record_preferred_commit();
        }
        let committed_support_tiles = self
            .frontier_support
            .as_ref()
            .filter(|support| support.committed)
            .map(|support| support.identity.selected_tiles.clone())
            .unwrap_or_default();
        self.renderer
            .exact_coverage
            .set_frontier_support_tiles(queue, &committed_support_tiles)?;
        let frontier_certificate_active = self.frontier_support.as_ref().is_some_and(|support| {
            support.committed
                && support.identity.exact_generation == self.renderer.exact_coverage.mask.generation
                && support.identity.presentation == self.frontier_receipt.presentation
        });
        let far_culls = render_view_overrides.map(|view| {
            presentation
                .fog
                .far_cull_distance()
                .zip(view)
                .map(|(distance, view)| (distance, view.camera_position))
        });
        let minimum_draw_sample_spacing = terrain_horizon_overview_minimum_sample_spacing(
            presentation.width_blocks,
            width,
            self.renderer.exact_coverage.mode != TerrainExactCoverageMode::Disabled,
            terrain_levels
                .last()
                .map_or(1, |level| level.snapshot.sample_spacing),
        );
        self.visible_terrain_slots.fill(false);
        let mut inner_hole_culled_tiles = 0_u32;
        let mut frustum_culled_tiles = 0_u32;
        let mut far_culled_tiles = 0_u32;
        let exact_connector_generation = self.renderer.exact_coverage.mask.generation;
        let exact_connector_instances =
            if self.renderer.exact_coverage.mode != TerrainExactCoverageMode::Disabled {
                self.renderer.exact_coverage.connector_instances.clone()
            } else {
                Vec::new()
            };
        let frontier_connectors = self
            .frontier_support
            .as_ref()
            .filter(|support| support.committed)
            .map(|support| {
                (
                    support.identity.exact_generation,
                    support.identity.presentation,
                    support.connector_instances.as_slice(),
                )
            });
        if !frontier_warming {
            for level in &terrain_levels {
                if level.snapshot.sample_spacing < minimum_draw_sample_spacing {
                    continue;
                }
                let inner_hole = finer_drawn_level_bounds(
                    &terrain_levels,
                    level.snapshot.level,
                    minimum_draw_sample_spacing,
                );
                for resource in &level.tiles {
                    match terrain_horizon_tile_visibility_for_views(
                        resource.tile,
                        inner_hole,
                        0.0,
                        far_culls,
                        render_view_overrides,
                        uniform_presentation,
                    ) {
                        TerrainHorizonTileVisibility::Visible => {}
                        TerrainHorizonTileVisibility::InnerHole => {
                            inner_hole_culled_tiles = inner_hole_culled_tiles.saturating_add(1);
                            continue;
                        }
                        TerrainHorizonTileVisibility::Frustum => {
                            frustum_culled_tiles = frustum_culled_tiles.saturating_add(1);
                            continue;
                        }
                        TerrainHorizonTileVisibility::Far => {
                            far_culled_tiles = far_culled_tiles.saturating_add(1);
                            continue;
                        }
                    }
                    self.visible_terrain_slots[resource.resource_slot as usize] = true;
                    let view_mask = terrain_horizon_tile_view_mask(
                        resource.tile,
                        inner_hole,
                        0.0,
                        far_culls,
                        render_view_overrides,
                        uniform_presentation,
                    );
                    let slot_index = resource.resource_slot as usize;
                    let outer_edge_flags = terrain_horizon_outer_edge_flags(
                        level,
                        resource.tile,
                        self.clipmap.config().level_count,
                    );
                    let slot = &mut self.slots[slot_index];
                    slot.refresh_exact_connectors(
                        device,
                        queue,
                        exact_connector_generation,
                        &exact_connector_instances,
                    );
                    if let Some((generation, frontier_presentation, connectors)) =
                        frontier_connectors
                    {
                        slot.refresh_frontier_connectors(
                            device,
                            queue,
                            generation,
                            frontier_presentation,
                            connectors,
                        );
                    }
                    queue.write_buffer(
                        &slot.uniform_buffer,
                        0,
                        &terrain_horizon_uniform_bytes(
                            slot.request,
                            width,
                            height,
                            options,
                            presentation.camera,
                            uniform_presentation,
                            focus_y,
                            inner_hole,
                            outer_edge_flags,
                            view_mask,
                            render_view_overrides,
                            presentation.sky_darken,
                            presentation.fog,
                            [1.0, 1.0],
                            presentation.diagnostic,
                        ),
                    );
                }
            }
        }
        let mut frontier_support_visible = self
            .frontier_support
            .as_ref()
            .map_or_else(Vec::new, |support| vec![false; support.tiles.len()]);
        if !frontier_warming
            && let Some(support) = self
                .frontier_support
                .as_mut()
                .filter(|support| support.committed)
        {
            for (index, support_tile) in support.tiles.iter_mut().enumerate() {
                let clipmap_tile = TerrainClipmapTile {
                    level: 0,
                    tile_x: i32::try_from(support_tile.key.tile_x)
                        .map_err(|_| "frontier support visible tile X exceeds coordinates")?,
                    tile_z: i32::try_from(support_tile.key.tile_z)
                        .map_err(|_| "frontier support visible tile Z exceeds coordinates")?,
                    sample_spacing: 1,
                    physical_x: 0,
                    physical_z: 0,
                    physical_slot: 0,
                };
                if terrain_horizon_tile_visibility_for_views(
                    clipmap_tile,
                    None,
                    0.0,
                    far_culls,
                    render_view_overrides,
                    uniform_presentation,
                ) != TerrainHorizonTileVisibility::Visible
                {
                    continue;
                }
                frontier_support_visible[index] = true;
                let view_mask = terrain_horizon_tile_view_mask(
                    clipmap_tile,
                    None,
                    0.0,
                    far_culls,
                    render_view_overrides,
                    uniform_presentation,
                );
                queue.write_buffer(
                    &support_tile.tile.uniform_buffer,
                    0,
                    &terrain_horizon_uniform_bytes(
                        support_tile.tile.request,
                        width,
                        height,
                        options,
                        presentation.camera,
                        uniform_presentation,
                        focus_y,
                        None,
                        support_tile.outer_edge_flags,
                        view_mask,
                        render_view_overrides,
                        presentation.sky_darken,
                        presentation.fog,
                        [1.0, 1.0],
                        presentation.diagnostic,
                    ),
                );
            }
        }
        let vegetation_levels = self.admission.vegetation_presentations();
        self.visible_vegetation_slots.fill(false);
        let candidate_proxy_geometry_visible = self.profile
            != TerrainPreviewProfile::ContinentalEcoregionCandidate
            || presentation.width_blocks.max(presentation.height_blocks)
                <= CONTINENTAL_PROXY_TREE_MAX_VIEW_BLOCKS;
        if !frontier_warming {
            for level in &vegetation_levels {
                if !candidate_proxy_geometry_visible {
                    continue;
                }
                if level.snapshot.sample_spacing < minimum_draw_sample_spacing {
                    continue;
                }
                if level.snapshot.sample_spacing > self.vegetation_max_sample_spacing {
                    continue;
                }
                let inner_hole = finer_drawn_level_bounds(
                    &vegetation_levels,
                    level.snapshot.level,
                    minimum_draw_sample_spacing,
                );
                for resource in &level.tiles {
                    if terrain_horizon_tile_visibility_for_views(
                        resource.tile,
                        inner_hole,
                        TERRAIN_HORIZON_TREE_CULL_MARGIN_BLOCKS,
                        far_culls,
                        render_view_overrides,
                        uniform_presentation,
                    ) != TerrainHorizonTileVisibility::Visible
                    {
                        continue;
                    }
                    self.visible_vegetation_slots[resource.resource_slot as usize] = true;
                    let view_mask = terrain_horizon_tile_view_mask(
                        resource.tile,
                        inner_hole,
                        TERRAIN_HORIZON_TREE_CULL_MARGIN_BLOCKS,
                        far_culls,
                        render_view_overrides,
                        uniform_presentation,
                    );
                    let slot = &self.slots[resource.resource_slot as usize];
                    queue.write_buffer(
                        &slot.tree_uniform_buffer,
                        0,
                        &terrain_horizon_uniform_bytes(
                            slot.request,
                            width,
                            height,
                            options,
                            presentation.camera,
                            uniform_presentation,
                            focus_y,
                            inner_hole,
                            0,
                            view_mask,
                            render_view_overrides,
                            presentation.sky_darken,
                            presentation.fog,
                            [slot.proxy_reveal, slot.canopy_reveal],
                            presentation.diagnostic,
                        ),
                    );
                }
            }
            for level in &terrain_levels {
                if level.snapshot.sample_spacing < minimum_draw_sample_spacing {
                    continue;
                }
                if !terrain_horizon_level_uses_canopy(
                    self.profile,
                    self.content_stage,
                    level.snapshot.sample_spacing,
                    self.vegetation_max_sample_spacing,
                ) {
                    continue;
                }
                let inner_hole = finer_drawn_level_bounds(
                    &terrain_levels,
                    level.snapshot.level,
                    minimum_draw_sample_spacing,
                );
                for resource in &level.tiles {
                    if terrain_horizon_tile_visibility_for_views(
                        resource.tile,
                        inner_hole,
                        TERRAIN_HORIZON_TREE_CULL_MARGIN_BLOCKS,
                        far_culls,
                        render_view_overrides,
                        uniform_presentation,
                    ) != TerrainHorizonTileVisibility::Visible
                    {
                        continue;
                    }
                    self.visible_vegetation_slots[resource.resource_slot as usize] = true;
                    let view_mask = terrain_horizon_tile_view_mask(
                        resource.tile,
                        inner_hole,
                        TERRAIN_HORIZON_TREE_CULL_MARGIN_BLOCKS,
                        far_culls,
                        render_view_overrides,
                        uniform_presentation,
                    );
                    let slot = &self.slots[resource.resource_slot as usize];
                    queue.write_buffer(
                        &slot.tree_uniform_buffer,
                        0,
                        &terrain_horizon_uniform_bytes(
                            slot.request,
                            width,
                            height,
                            options,
                            presentation.camera,
                            uniform_presentation,
                            focus_y,
                            inner_hole,
                            0,
                            view_mask,
                            render_view_overrides,
                            presentation.sky_darken,
                            presentation.fog,
                            [slot.proxy_reveal, slot.canopy_reveal],
                            presentation.diagnostic,
                        ),
                    );
                }
            }
        }

        let mut drawn_levels = 0_u32;
        let mut drawn_tiles = 0_u32;
        let mut drawn_tiles_by_level = [0_u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize];
        let mut drawn_exact_connector_segments = 0_u32;
        let mut drawn_frontier_support_tiles = 0_u32;
        let mut drawn_frontier_connector_segments = 0_u32;
        let mut drawn_tree_tiles = 0_u32;
        let mut drawn_tree_instances = 0_u32;
        let mut drawn_tree_tiles_by_level = [0_u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize];
        let mut drawn_tree_instances_by_level = [0_u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize];
        let mut drawn_canopy_tiles = 0_u32;
        let mut drawn_canopy_cells = 0_u32;
        let mut drawn_canopy_vertices = 0_u32;
        let mut drawn_canopy_tiles_by_level = [0_u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize];
        let mut drawn_canopy_cells_by_level = [0_u32; TERRAIN_LOD_HIGH_LEVEL_COUNT as usize];
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mclone_terrain_horizon_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: color_load,
                        store: color_store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: external_depth_view.unwrap_or(&self.renderer.depth.view),
                    depth_ops: Some(wgpu::Operations {
                        load: depth_load,
                        store: depth_store.unwrap_or(if self.renderer.depth_capture_enabled {
                            wgpu::StoreOp::Store
                        } else {
                            wgpu::StoreOp::Discard
                        }),
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            let terrain_pipeline = if multiview {
                self.renderer
                    .horizon_multiview_render_pipeline
                    .as_ref()
                    .expect("validated horizon multiview pipeline remains available")
            } else {
                self.renderer
                    .horizon_render_pipeline
                    .as_ref()
                    .expect("horizon renderer owns its render pipeline")
            };
            pass.set_pipeline(terrain_pipeline);
            pass.set_bind_group(1, &self.renderer._material_resources.bind_group, &[]);
            pass.set_bind_group(2, &self.renderer.exact_coverage.bind_group, &[]);
            for level in terrain_levels.iter().rev() {
                let mut level_drawn = false;
                for resource in &level.tiles {
                    if !self.visible_terrain_slots[resource.resource_slot as usize] {
                        continue;
                    }
                    let slot_index = resource.resource_slot as usize;
                    pass.set_bind_group(0, &self.slots[slot_index].render_bind_group, &[]);
                    let render_cells = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS
                        / self.renderer.horizon_render_cell_stride;
                    pass.draw(
                        0..render_cells.pow(2)
                            * terrain_horizon_vertices_per_cell(level.snapshot.sample_spacing),
                        0..1,
                    );
                    drawn_tiles = drawn_tiles.saturating_add(1);
                    if let Some(count) = drawn_tiles_by_level.get_mut(level.snapshot.level as usize)
                    {
                        *count = count.saturating_add(1);
                    }
                    level_drawn = true;
                }
                if level_drawn {
                    drawn_levels = drawn_levels.saturating_add(1);
                }
            }
            if let Some(support) = self
                .frontier_support
                .as_ref()
                .filter(|support| support.committed)
            {
                for (index, support_tile) in support.tiles.iter().enumerate() {
                    if !frontier_support_visible[index] {
                        continue;
                    }
                    pass.set_bind_group(0, &support_tile.tile.render_bind_group, &[]);
                    let render_cells = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS
                        / self.renderer.horizon_render_cell_stride;
                    pass.draw(
                        0..render_cells.pow(2) * terrain_horizon_vertices_per_cell(1),
                        0..1,
                    );
                    drawn_frontier_support_tiles = drawn_frontier_support_tiles.saturating_add(1);
                }
            }
            if self.renderer.exact_coverage.mode != TerrainExactCoverageMode::Disabled {
                let connector_pipeline = if multiview {
                    self.renderer
                        .horizon_exact_connector_multiview_pipeline
                        .as_ref()
                        .expect("validated exact connector multiview pipeline remains available")
                } else {
                    self.renderer
                        .horizon_exact_connector_pipeline
                        .as_ref()
                        .expect("horizon renderer owns its exact connector pipeline")
                };
                pass.set_pipeline(connector_pipeline);
                if frontier_certificate_active {
                    for level in &terrain_levels {
                        for resource in &level.tiles {
                            if !self.visible_terrain_slots[resource.resource_slot as usize] {
                                continue;
                            }
                            let slot = &self.slots[resource.resource_slot as usize];
                            let Some(instance_buffer) = slot.frontier_connector_buffer.as_ref()
                            else {
                                continue;
                            };
                            pass.set_bind_group(0, &slot.render_bind_group, &[]);
                            pass.set_vertex_buffer(0, instance_buffer.slice(..));
                            pass.draw(
                                0..TERRAIN_EXACT_CONNECTOR_VERTICES_PER_INSTANCE,
                                0..slot.frontier_connector_instance_count,
                            );
                            drawn_frontier_connector_segments = drawn_frontier_connector_segments
                                .saturating_add(slot.frontier_connector_instance_count);
                        }
                    }
                    if let Some(support) = self.frontier_support.as_ref() {
                        for (index, support_tile) in support.tiles.iter().enumerate() {
                            if !frontier_support_visible[index] {
                                continue;
                            }
                            let Some(instance_buffer) =
                                support_tile.tile.frontier_connector_buffer.as_ref()
                            else {
                                continue;
                            };
                            pass.set_bind_group(0, &support_tile.tile.render_bind_group, &[]);
                            pass.set_vertex_buffer(0, instance_buffer.slice(..));
                            pass.draw(
                                0..TERRAIN_EXACT_CONNECTOR_VERTICES_PER_INSTANCE,
                                0..support_tile.tile.frontier_connector_instance_count,
                            );
                            drawn_frontier_connector_segments = drawn_frontier_connector_segments
                                .saturating_add(
                                    support_tile.tile.frontier_connector_instance_count,
                                );
                        }
                    }
                } else {
                    for level in &terrain_levels {
                        if level.snapshot.sample_spacing != 1 {
                            continue;
                        }
                        for resource in &level.tiles {
                            if !self.visible_terrain_slots[resource.resource_slot as usize] {
                                continue;
                            }
                            let slot = &self.slots[resource.resource_slot as usize];
                            let Some(instance_buffer) = slot.exact_connector_buffer.as_ref() else {
                                continue;
                            };
                            pass.set_bind_group(0, &slot.render_bind_group, &[]);
                            pass.set_vertex_buffer(0, instance_buffer.slice(..));
                            pass.draw(
                                0..TERRAIN_EXACT_CONNECTOR_VERTICES_PER_INSTANCE,
                                0..slot.exact_connector_instance_count,
                            );
                            drawn_exact_connector_segments = drawn_exact_connector_segments
                                .saturating_add(slot.exact_connector_instance_count);
                        }
                    }
                }
            }
            let tree_pipeline = if multiview {
                self.renderer
                    .tree_multiview_pipeline
                    .as_ref()
                    .expect("validated tree multiview pipeline remains available")
            } else {
                &self.renderer.tree_pipeline
            };
            let canopy_pipeline = if multiview {
                self.renderer
                    .canopy_multiview_pipeline
                    .as_ref()
                    .expect("validated canopy multiview pipeline remains available")
            } else {
                self.renderer
                    .canopy_pipeline
                    .as_ref()
                    .expect("horizon renderer owns its canopy pipeline")
            };
            pass.set_pipeline(canopy_pipeline);
            pass.set_bind_group(1, &self.renderer.exact_coverage.bind_group, &[]);
            for level in &terrain_levels {
                if level.snapshot.sample_spacing < minimum_draw_sample_spacing {
                    continue;
                }
                if !terrain_horizon_level_uses_canopy(
                    self.profile,
                    self.content_stage,
                    level.snapshot.sample_spacing,
                    self.vegetation_max_sample_spacing,
                ) {
                    continue;
                }
                for resource in &level.tiles {
                    if !self.visible_vegetation_slots[resource.resource_slot as usize] {
                        continue;
                    }
                    let slot = &self.slots[resource.resource_slot as usize];
                    pass.set_bind_group(0, &slot.tree_render_bind_group, &[]);
                    pass.draw(0..TERRAIN_HORIZON_CANOPY_VERTICES_PER_TILE, 0..1);
                    drawn_canopy_tiles = drawn_canopy_tiles.saturating_add(1);
                    drawn_canopy_cells =
                        drawn_canopy_cells.saturating_add(TERRAIN_HORIZON_CANOPY_CELLS_PER_TILE);
                    drawn_canopy_vertices = drawn_canopy_vertices
                        .saturating_add(TERRAIN_HORIZON_CANOPY_VERTICES_PER_TILE);
                    if let Some(count) =
                        drawn_canopy_tiles_by_level.get_mut(level.snapshot.level as usize)
                    {
                        *count = count.saturating_add(1);
                    }
                    if let Some(count) =
                        drawn_canopy_cells_by_level.get_mut(level.snapshot.level as usize)
                    {
                        *count = count.saturating_add(TERRAIN_HORIZON_CANOPY_CELLS_PER_TILE);
                    }
                }
            }
            pass.set_pipeline(tree_pipeline);
            pass.set_bind_group(1, &self.renderer.exact_coverage.bind_group, &[]);
            for level in &vegetation_levels {
                if level.snapshot.sample_spacing < minimum_draw_sample_spacing {
                    continue;
                }
                if level.snapshot.sample_spacing > self.vegetation_max_sample_spacing {
                    continue;
                }
                for resource in &level.tiles {
                    if !self.visible_vegetation_slots[resource.resource_slot as usize] {
                        continue;
                    }
                    let slot = &self.slots[resource.resource_slot as usize];
                    let Some(instance_buffer) = slot.tree_instance_buffer.as_ref() else {
                        continue;
                    };
                    pass.set_bind_group(0, &slot.tree_render_bind_group, &[]);
                    pass.set_vertex_buffer(0, instance_buffer.slice(..));
                    pass.draw(
                        0..TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE,
                        0..slot.tree_instance_count,
                    );
                    drawn_tree_tiles = drawn_tree_tiles.saturating_add(1);
                    drawn_tree_instances =
                        drawn_tree_instances.saturating_add(slot.tree_instance_count);
                    if let Some(count) =
                        drawn_tree_tiles_by_level.get_mut(level.snapshot.level as usize)
                    {
                        *count = count.saturating_add(1);
                    }
                    if let Some(count) =
                        drawn_tree_instances_by_level.get_mut(level.snapshot.level as usize)
                    {
                        *count = count.saturating_add(slot.tree_instance_count);
                    }
                }
            }
        }

        let admission_diagnostics = self.admission.diagnostics();
        let allocation_slots = admission_diagnostics.logical_slots;
        let ready_slots = terrain_levels
            .iter()
            .map(|level| level.tiles.len() as u32)
            .sum();
        let vegetation_resources = vegetation_levels
            .iter()
            .filter(|level| level.snapshot.sample_spacing <= self.vegetation_max_sample_spacing)
            .flat_map(|level| level.tiles.iter())
            .collect::<Vec<_>>();
        let revealing_proxy_tiles = vegetation_resources
            .iter()
            .filter(|resource| self.slots[resource.resource_slot as usize].proxy_reveal < 1.0)
            .count()
            .try_into()
            .unwrap_or(u32::MAX);
        let revealing_canopy_tiles = terrain_levels
            .iter()
            .filter(|level| {
                terrain_horizon_level_uses_canopy(
                    self.profile,
                    self.content_stage,
                    level.snapshot.sample_spacing,
                    self.vegetation_max_sample_spacing,
                )
            })
            .flat_map(|level| level.tiles.iter())
            .filter(|resource| self.slots[resource.resource_slot as usize].canopy_reveal < 1.0)
            .count()
            .try_into()
            .unwrap_or(u32::MAX);
        let tree_instance_count = vegetation_resources.iter().fold(0_u32, |count, resource| {
            count.saturating_add(self.slots[resource.resource_slot as usize].tree_instance_count)
        });
        let tree_proxy_suppressed_instances =
            vegetation_resources.iter().fold(0_u32, |count, resource| {
                count.saturating_add(
                    self.slots[resource.resource_slot as usize].tree_suppressed_instance_count,
                )
            });
        let resident_tree_ids = vegetation_resources
            .iter()
            .filter_map(|resource| {
                self.slots[resource.resource_slot as usize]
                    .vegetation
                    .as_ref()
            })
            .flat_map(|vegetation| vegetation.occurrences())
            .copied()
            .map(McloneTreeOccurrenceId::from)
            .collect::<BTreeSet<_>>();
        let tree_proxy_suppressed_records = self
            .exact_owned_tree_ids
            .intersection(&resident_tree_ids)
            .count()
            .try_into()
            .unwrap_or(u32::MAX);
        let tree_proxy_missing_exact_records = self
            .exact_owned_tree_ids
            .difference(&resident_tree_ids)
            .count()
            .try_into()
            .unwrap_or(u32::MAX);
        let proxy_owned_tree_ids = self
            .tree_ownership
            .as_ref()
            .into_iter()
            .flat_map(BoundedRepresentationOwnershipSnapshot::approximate_owned_ids)
            .copied()
            .collect::<BTreeSet<_>>();
        let tree_proxy_missing_proxy_records = proxy_owned_tree_ids
            .difference(&resident_tree_ids)
            .count()
            .try_into()
            .unwrap_or(u32::MAX);
        let tree_proxy_vertex_count =
            tree_instance_count.saturating_mul(TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE);
        let render_cells =
            TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS / self.renderer.horizon_render_cell_stride;
        let terrain_vertex_count = terrain_levels.iter().fold(0_u32, |count, level| {
            let visible_tiles: u32 = level
                .tiles
                .iter()
                .filter(|resource| self.visible_terrain_slots[resource.resource_slot as usize])
                .count()
                .try_into()
                .unwrap_or(u32::MAX);
            count.saturating_add(
                visible_tiles
                    .saturating_mul(render_cells.pow(2))
                    .saturating_mul(terrain_horizon_vertices_per_cell(
                        level.snapshot.sample_spacing,
                    )),
            )
        });
        let exact_connector_vertex_count = drawn_exact_connector_segments
            .saturating_mul(TERRAIN_EXACT_CONNECTOR_VERTICES_PER_INSTANCE);
        let frontier_support_vertex_count = drawn_frontier_support_tiles
            .saturating_mul(render_cells.pow(2))
            .saturating_mul(terrain_horizon_vertices_per_cell(1));
        let frontier_connector_vertex_count = drawn_frontier_connector_segments
            .saturating_mul(TERRAIN_EXACT_CONNECTOR_VERTICES_PER_INSTANCE);
        let vertex_count = terrain_vertex_count
            .saturating_add(exact_connector_vertex_count)
            .saturating_add(frontier_support_vertex_count)
            .saturating_add(frontier_connector_vertex_count)
            .saturating_add(drawn_canopy_vertices)
            .saturating_add(
                drawn_tree_instances.saturating_mul(TERRAIN_PREVIEW_TREE_VERTICES_PER_INSTANCE),
            );
        let normal_halo_samples_per_tile = terrain_horizon_normal_halo_samples_per_tile();
        let normal_halo_fixed_bytes = u64::from(self.admission.resource_slots())
            .saturating_mul(u64::from(normal_halo_samples_per_tile))
            .saturating_mul(size_of::<f32>() as u64);
        let normal_height_fixed_bytes = u64::from(self.admission.resource_slots())
            .saturating_mul(terrain_horizon_normal_height_byte_len()?);
        let fixed_resident_bytes = u64::from(self.admission.resource_slots())
            .saturating_mul(
                self.renderer
                    .sample_byte_len
                    .saturating_add(terrain_horizon_normal_height_byte_len()?)
                    .saturating_add(TERRAIN_PREVIEW_UNIFORM_BYTES.saturating_mul(2)),
            )
            .saturating_add(TERRAIN_EXACT_COVERAGE_UNIFORM_BYTES)
            .saturating_add(TERRAIN_EXACT_COVERAGE_MASK_BYTES)
            .saturating_add(super::TERRAIN_EXACT_TRANSITION_MAX_BYTES)
            .saturating_add(super::TERRAIN_EXACT_BOUNDARY_MAX_BYTES)
            .saturating_add(TERRAIN_FRONTIER_SUPPORT_LOOKUP_BUFFER_BYTES);
        let vegetation_bytes = self
            .slots
            .iter()
            .map(|slot| slot.tree_instance_bytes)
            .sum::<u64>();
        let exact_connector_bytes = self
            .slots
            .iter()
            .map(|slot| slot.exact_connector_instance_bytes)
            .sum::<u64>();
        let frontier_generations = self
            .frontier_support
            .iter()
            .chain(self.frontier_support_pending.iter());
        let frontier_support_allocated_tiles =
            frontier_generations.clone().fold(0_u32, |count, support| {
                count.saturating_add(support.tiles.len().try_into().unwrap_or(u32::MAX))
            });
        let frontier_support_ready_tiles = frontier_generations.fold(0_u32, |count, support| {
            count.saturating_add(
                support
                    .tiles
                    .iter()
                    .filter(|tile| tile.ready)
                    .count()
                    .try_into()
                    .unwrap_or(u32::MAX),
            )
        });
        let frontier_support_pending_tiles =
            frontier_support_allocated_tiles.saturating_sub(frontier_support_ready_tiles);
        let frontier_support_resource_bytes = u64::from(frontier_support_allocated_tiles)
            .saturating_mul(super::TERRAIN_FRONTIER_TERRAIN_RESOURCE_BYTES);
        let frontier_connector_bytes = self
            .slots
            .iter()
            .map(|slot| slot.frontier_connector_instance_bytes)
            .chain(self.frontier_support.iter().flat_map(|support| {
                support
                    .tiles
                    .iter()
                    .map(|tile| tile.tile.frontier_connector_instance_bytes)
            }))
            .chain(self.frontier_support_pending.iter().flat_map(|support| {
                support
                    .tiles
                    .iter()
                    .map(|tile| tile.tile.frontier_connector_instance_bytes)
            }))
            .sum::<u64>();
        let resident_bytes = fixed_resident_bytes
            .saturating_add(exact_connector_bytes)
            .saturating_add(frontier_support_resource_bytes)
            .saturating_add(frontier_connector_bytes)
            .saturating_add(vegetation_bytes);
        let vegetation_ready_tiles = vegetation_resources
            .iter()
            .filter(|resource| {
                self.slots[resource.resource_slot as usize]
                    .vegetation
                    .is_some()
            })
            .count() as u32;
        let (pending_vegetation_tiles, vegetation_settled) = self
            .vegetation_coordinator
            .as_ref()
            .map(|coordinator| {
                let diagnostics = coordinator.diagnostics();
                let pending = diagnostics
                    .queued_tiles
                    .saturating_add(u32::from(diagnostics.in_flight));
                let settled = match diagnostics.state {
                    TerrainVegetationCoordinatorState::Running => {
                        diagnostics.desired_tiles == diagnostics.resident_tiles && pending == 0
                    }
                    TerrainVegetationCoordinatorState::Failed
                    | TerrainVegetationCoordinatorState::Terminated => true,
                    TerrainVegetationCoordinatorState::Starting
                    | TerrainVegetationCoordinatorState::ShuttingDown => false,
                };
                (pending, settled)
            })
            .unwrap_or((0, true));
        let vegetation_service = self.vegetation_service_stats(&vegetation_levels)?;
        if vegetation_service.record_count
            != tree_instance_count.saturating_add(tree_proxy_suppressed_instances)
        {
            return Err(format!(
                "terrain vegetation receipt counts {} records but GPU slots contain \
                 {tree_instance_count} proxy instances and {tree_proxy_suppressed_instances} \
                 exact-owned suppressed instances",
                vegetation_service.record_count,
            ));
        }
        let tree_ownership_generation = self
            .tree_ownership
            .as_ref()
            .map_or(0, BoundedRepresentationOwnershipSnapshot::generation);
        let tree_ownership_units = self
            .tree_ownership
            .as_ref()
            .map_or(0, |snapshot| snapshot.units().len() as u32);
        let exact_owned_tree_records = self
            .tree_ownership
            .as_ref()
            .map_or(0, |snapshot| snapshot.exact_owned_ids().count() as u32);
        let proxy_owned_tree_records = self.tree_ownership.as_ref().map_or(0, |snapshot| {
            snapshot.approximate_owned_ids().count() as u32
        });
        let target_ready = ready_slots == allocation_slots
            && self.pending_terrain_refills() == 0
            && !self.admission.has_staged_levels()
            && self.clipmap.center() == (self.requested_center_x, self.requested_center_z)
            && self.clipmap.origins_settled()
            && vegetation_settled
            && (!exact_frontier_required
                || (frontier_certificate_active && self.frontier_support_pending.is_none()));
        let frontier_admission =
            self.frontier_admission_receipt(exact_frontier_required, frontier_warming);
        Ok(TerrainHorizonFrameStats {
            revision: self.clipmap.diagnostics().revision,
            allocation_slots,
            staging_slots: admission_diagnostics.staging_slots,
            normal_halo_radius: TERRAIN_HORIZON_NORMAL_HALO_RADIUS,
            normal_halo_samples_per_tile,
            normal_halo_fixed_bytes,
            normal_height_fixed_bytes,
            ready_slots,
            requested_levels: admission_diagnostics.requested_levels,
            staged_levels: admission_diagnostics.staged_levels,
            committed_levels: admission_diagnostics.committed_levels,
            vegetation_committed_levels: admission_diagnostics.vegetation_committed_levels,
            atomic_level_commits: admission_diagnostics.atomic_level_commits,
            deferred_transition_attempts: admission_diagnostics.deferred_transition_attempts,
            pending_refills: self.pending_terrain_refills(),
            dispatched_refills,
            dispatched_refills_total: self.dispatched_refills_total,
            #[cfg(not(target_arch = "wasm32"))]
            cpu_compile_workers: self
                .cpu_compiler
                .as_ref()
                .map_or(0, |compiler| compiler.worker_count() as u32),
            #[cfg(target_arch = "wasm32")]
            cpu_compile_workers: 0,
            cpu_compile_in_flight: self.cpu_compile_in_flight,
            cpu_compile_submitted_total: self.cpu_compile_submitted_total,
            cpu_compile_completed_total: self.cpu_compile_completed_total,
            cpu_compile_micros_total: self.cpu_compile_micros_total,
            cpu_compile_stale_results_total: self.cpu_compile_stale_results_total,
            drawn_levels,
            drawn_tiles,
            drawn_tiles_by_level,
            inner_hole_culled_tiles,
            frustum_culled_tiles,
            far_culled_tiles,
            drawn_tree_tiles,
            drawn_tree_instances,
            drawn_tree_tiles_by_level,
            drawn_tree_instances_by_level,
            drawn_canopy_tiles,
            drawn_canopy_cells,
            drawn_canopy_vertices,
            drawn_canopy_tiles_by_level,
            drawn_canopy_cells_by_level,
            revealing_proxy_tiles,
            revealing_canopy_tiles,
            exact_connector_segments: drawn_exact_connector_segments,
            exact_connector_vertex_count,
            frontier_support_allocated_tiles,
            frontier_support_ready_tiles,
            frontier_support_pending_tiles,
            frontier_support_drawn_tiles: drawn_frontier_support_tiles,
            frontier_support_dispatches,
            frontier_support_dispatches_total: self.frontier_support_dispatches_total,
            frontier_support_resource_bytes,
            frontier_support_vertex_count,
            frontier_connector_segments: drawn_frontier_connector_segments,
            frontier_connector_vertex_count,
            frontier_connector_bytes,
            vertex_count,
            vegetation_ready_tiles,
            pending_vegetation_tiles,
            tree_instance_count,
            tree_proxy_suppressed_instances,
            tree_proxy_suppressed_records,
            tree_proxy_missing_exact_records,
            tree_proxy_missing_proxy_records,
            tree_proxy_vertex_count,
            tree_ownership_generation,
            tree_ownership_units,
            exact_owned_tree_records,
            proxy_owned_tree_records,
            fixed_resident_bytes,
            exact_connector_bytes,
            vegetation_bytes,
            resident_bytes,
            exact_coverage_mode: self.renderer.exact_coverage.mode,
            exact_coverage_generation: self.renderer.exact_coverage.mask.generation,
            exact_painted_chunks: self.renderer.exact_coverage.mask.painted_chunks,
            exact_coverage_mask_bytes: TERRAIN_EXACT_COVERAGE_MASK_BYTES,
            exact_transition_preparation_micros: self
                .renderer
                .exact_coverage
                .transition
                .preparation_micros(),
            exact_transition_payload_bytes: self.renderer.exact_coverage.transition.payload_bytes(),
            exact_boundary_columns: self.renderer.exact_coverage.boundary.valid_columns(),
            exact_boundary_payload_bytes: self.renderer.exact_coverage.boundary.payload_bytes(),
            frontier: self.frontier_receipt,
            frontier_plan_failures: self.frontier_plan_failures,
            frontier_topology: self.frontier_topology_receipt,
            frontier_topology_failures: self.frontier_topology_failures,
            frontier_admission,
            vegetation_service,
            finest_sample_spacing: self.clipmap.config().base_sample_spacing,
            coarse_ready: drawn_levels > 0,
            target_ready,
            needs_redraw: !target_ready || revealing_proxy_tiles > 0 || revealing_canopy_tiles > 0,
            residency: self.clipmap.diagnostics(),
        })
    }

    fn vegetation_service_stats(
        &self,
        levels: &[TerrainHorizonLevelPresentation],
    ) -> Result<TerrainHorizonVegetationServiceStats, String> {
        let Some(coordinator) = self.vegetation_coordinator.as_ref() else {
            return Ok(TerrainHorizonVegetationServiceStats {
                enabled: self.vegetation_executor.is_some(),
                ..Default::default()
            });
        };
        let diagnostics = coordinator.diagnostics();
        let source = coordinator.source();
        let receipt = terrain_vegetation_coverage_receipt(
            source,
            levels
                .iter()
                .filter(|level| level.snapshot.sample_spacing <= self.vegetation_max_sample_spacing)
                .flat_map(|level| level.tiles.iter())
                .filter_map(|resource| {
                    self.slots[resource.resource_slot as usize]
                        .vegetation
                        .as_ref()
                }),
        )?;
        let executor = diagnostics.executor;
        Ok(TerrainHorizonVegetationServiceStats {
            enabled: true,
            coordinator_state: Some(diagnostics.state),
            executor_kind: Some(diagnostics.executor_kind),
            source_fingerprint: receipt.source_fingerprint,
            terrain_source_revision: source.terrain_source_revision,
            compiler_source_revision: source.compiler_source_revision,
            vegetation_plan_revision: source.vegetation_plan_revision,
            product_revision: source.product_revision,
            record_hash: receipt.record_hash,
            family_counts: receipt.family_counts,
            record_count: receipt.record_count,
            product_count: receipt.product_count,
            executor_generation: diagnostics.executor_generation,
            source_epoch: diagnostics.source_epoch,
            coverage_revision: diagnostics.coverage_revision,
            desired_tiles: diagnostics.desired_tiles,
            queued_tiles: diagnostics.queued_tiles,
            resident_tiles: diagnostics.resident_tiles,
            in_flight: diagnostics.in_flight,
            submitted_jobs: diagnostics.submitted_jobs,
            completed_jobs: diagnostics.completed_jobs,
            admitted_products: diagnostics.admitted_products,
            source_resets: diagnostics.source_resets,
            transport_failures: diagnostics.transport_failures,
            executor_restarts: diagnostics.executor_restarts,
            job_failures: diagnostics.job_failures,
            stale_completions: diagnostics
                .stale_generation_completions
                .saturating_add(diagnostics.stale_source_completions)
                .saturating_add(diagnostics.stale_request_completions)
                .saturating_add(diagnostics.stale_slot_completions),
            superseded_completions: diagnostics.superseded_completions,
            submit_full_count: diagnostics.submit_full_count,
            compile_micros: diagnostics.compile_micros,
            cache_cell_requests: diagnostics.cache_cell_requests,
            cache_cell_hits: diagnostics.cache_cell_hits,
            cache_cell_misses: diagnostics.cache_cell_misses,
            cache_retained_cells: diagnostics.cache_retained_cells,
            cache_retained_preliminary_candidates: diagnostics
                .cache_retained_preliminary_candidates,
            executor_submitted_jobs: executor.submitted_jobs,
            executor_completed_jobs: executor.completed_jobs,
            executor_transport_failures: executor.transport_failures,
            executor_restart_count: executor.restarts,
            result_capacity_bytes: executor.result_capacity_bytes,
            result_high_water_bytes: executor.result_high_water_bytes,
            result_overflow_count: executor.result_overflows,
            copied_result_bytes: executor.copied_result_bytes,
            main_decode_micros: executor.main_decode_micros,
        })
    }

    fn refresh_vegetation_desired(&mut self, focus_x: i32, focus_z: i32) -> Result<(), String> {
        if self.vegetation_executor.is_none() && self.vegetation_coordinator.is_none() {
            return Ok(());
        }
        let source =
            terrain_horizon_vegetation_source(self.profile, self.seed, self.content_stage)?;
        let desired = self
            .admission
            .current_requested_presentations()
            .into_iter()
            .filter(|level| level.snapshot.sample_spacing <= self.vegetation_max_sample_spacing)
            .flat_map(|level| level.tiles)
            .map(|resource| TerrainVegetationDesiredTile {
                tile: terrain_horizon_tile_id(
                    self.profile,
                    self.seed,
                    self.content_stage,
                    resource.tile,
                ),
                slot: TerrainVegetationSlotToken {
                    physical_slot: resource.resource_slot,
                    slot_generation: resource.slot_generation,
                },
            })
            .collect::<Vec<_>>();
        if self.vegetation_coordinator.is_none() {
            let executor = self
                .vegetation_executor
                .take()
                .expect("checked terrain vegetation executor");
            if desired.is_empty() {
                return Err(
                    "terrain vegetation was enabled for a clipmap with no record levels".to_owned(),
                );
            }
            self.vegetation_coordinator = Some(TerrainVegetationCoordinator::new(
                executor,
                source,
                maximum_terrain_vegetation_desired_tiles(
                    self.clipmap.config(),
                    self.vegetation_max_sample_spacing,
                )?,
            )?);
        }
        self.vegetation_coordinator
            .as_mut()
            .expect("created terrain vegetation coordinator")
            .update_desired(source, focus_x, focus_z, desired)
    }

    fn refresh_authoritative_tree_ownership(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), String> {
        if !self.authoritative_tree_ownership {
            return Ok(());
        }
        let Some(coverage) = self.exact_coverage_snapshot.clone() else {
            return self.clear_tree_ownership(device, queue);
        };
        let mut occurrences = BTreeMap::new();
        for occurrence in self
            .slots
            .iter()
            .filter_map(|slot| slot.vegetation.as_ref())
            .flat_map(|vegetation| vegetation.occurrences().iter().copied())
        {
            occurrences
                .entry(McloneTreeOccurrenceId::from(occurrence))
                .or_insert(occurrence);
        }
        let snapshot = mclone_tree_ownership_snapshot(
            coverage.source(),
            coverage.generation(),
            &coverage,
            TERRAIN_EXACT_FRONTIER_TREE_INSET_BLOCKS,
            occurrences
                .into_values()
                // Exact-ready coverage owns both a present tree and an edited
                // absence. Proxy readiness is the resident product itself.
                .map(|occurrence| McloneTreeOwnershipCandidate::new(occurrence, true, true)),
        )?;
        self.set_tree_ownership(device, queue, &snapshot)
    }
}

fn finer_drawn_level_bounds(
    levels: &[TerrainHorizonLevelPresentation],
    level: u32,
    minimum_sample_spacing: u32,
) -> Option<super::TerrainClipmapBounds> {
    levels
        .iter()
        .filter(|candidate| {
            candidate.snapshot.level < level
                && candidate.snapshot.sample_spacing >= minimum_sample_spacing
        })
        .max_by_key(|candidate| candidate.snapshot.level)
        .map(|candidate| candidate.snapshot.bounds)
}

fn terrain_horizon_overview_minimum_sample_spacing(
    width_blocks: f64,
    width_pixels: u32,
    exact_composition_active: bool,
    coarsest_sample_spacing: u32,
) -> u32 {
    if exact_composition_active {
        return 1;
    }
    let blocks_per_pixel = width_blocks / f64::from(width_pixels.max(1));
    if !blocks_per_pixel.is_finite() || blocks_per_pixel <= 1.0 {
        return 1;
    }
    (blocks_per_pixel.ceil().min(f64::from(u32::MAX)) as u32)
        .checked_next_power_of_two()
        .unwrap_or(1 << 31)
        .min(coarsest_sample_spacing.max(1))
}

const fn terrain_horizon_level_uses_canopy(
    profile: TerrainPreviewProfile,
    content_stage: TerrainPreviewContentStage,
    sample_spacing: u32,
    _vegetation_max_sample_spacing: u32,
) -> bool {
    matches!(
        profile,
        TerrainPreviewProfile::ContinentalEcoregionCandidate
            | TerrainPreviewProfile::McloneOverworldV2
    ) && matches!(content_stage, TerrainPreviewContentStage::Cover)
        // Spacing one is the detailed procedural-tree handoff beside exact
        // terrain. Drawing the aerial canopy there duplicates those proxies,
        // exposes the fan closest to a ground observer, and adds an entire
        // 16-tile translucent layer on Quest Low. Coarser rings retain the
        // continuous forest-mass representation.
        && sample_spacing > 1
}

const fn terrain_horizon_semantic_tile_key(tile: TerrainClipmapTile) -> (u32, i32, i32, u32) {
    (tile.level, tile.tile_x, tile.tile_z, tile.sample_spacing)
}

fn terrain_horizon_samples_per_axis() -> u32 {
    TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS
        .saturating_add(1)
        .saturating_add(TERRAIN_HORIZON_NORMAL_HALO_RADIUS.saturating_mul(2))
}

const fn terrain_horizon_vertices_per_cell(_sample_spacing: u32) -> u32 {
    TERRAIN_HORIZON_SMOOTH_VERTICES_PER_CELL
}

fn terrain_horizon_normal_halo_samples_per_tile() -> u32 {
    let drawn_samples_per_axis = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS.saturating_add(1);
    terrain_horizon_samples_per_axis()
        .saturating_mul(terrain_horizon_samples_per_axis())
        .saturating_sub(drawn_samples_per_axis.saturating_mul(drawn_samples_per_axis))
}

fn terrain_horizon_normal_height_byte_len() -> Result<u64, String> {
    let samples_per_axis = terrain_horizon_samples_per_axis();
    u64::from(
        samples_per_axis
            .checked_mul(samples_per_axis)
            .ok_or("terrain horizon halo sample count overflow")?,
    )
    .checked_mul(size_of::<f32>() as u64)
    .ok_or_else(|| "terrain horizon normal-height byte size overflow".to_owned())
}

#[allow(clippy::too_many_arguments)]
fn terrain_horizon_uniform_bytes(
    request: ValidatedTerrainPreviewRequest,
    width: u32,
    height: u32,
    options: TerrainPreviewDrawOptions,
    camera: TerrainPreviewCamera,
    presentation: super::TerrainPreviewUniformPresentation,
    focus_y: f32,
    inner_hole: Option<super::TerrainClipmapBounds>,
    normal_edge_flags: u32,
    view_mask: u32,
    render_view_overrides: [Option<mclone_render::chunk::ChunkRenderView>; 2],
    sky_darken: f32,
    fog: mclone_render::fog::RenderFog,
    forest_reveal: [f32; 2],
    diagnostic: super::TerrainHorizonDiagnostic,
) -> Vec<u8> {
    debug_assert_eq!(
        normal_edge_flags
            & !(TERRAIN_HORIZON_NORMAL_EDGE_WEST
                | TERRAIN_HORIZON_NORMAL_EDGE_EAST
                | TERRAIN_HORIZON_NORMAL_EDGE_NORTH
                | TERRAIN_HORIZON_NORMAL_EDGE_SOUTH),
        0
    );
    let mut bytes = viewport_uniform_bytes_for_request_with_presentation(
        request,
        width,
        height,
        options,
        camera,
        presentation,
        focus_y,
        inner_hole,
        render_view_overrides[0],
    );
    const CONTENT_STAGE_FLAGS_W_OFFSET: usize = 8 * 16 + 3 * 4;
    let flag_bytes = bytes
        .get_mut(CONTENT_STAGE_FLAGS_W_OFFSET..CONTENT_STAGE_FLAGS_W_OFFSET + 4)
        .expect("terrain preview uniform content flags remain present");
    let flags = u32::from_le_bytes(
        flag_bytes
            .try_into()
            .expect("terrain preview uniform flag word remains four bytes"),
    ) | normal_edge_flags;
    flag_bytes.copy_from_slice(&flags.to_le_bytes());
    const FOG_CAMERA_OFFSET: usize = 14 * 16;
    let camera = render_view_overrides[0].map_or(glam::Vec3::ZERO, |view| view.camera_position);
    let fog_distances = fog.shader_distances();
    let lightmap = mclone_render::light_texture::lightmap_color(0, 15, sky_darken);
    let values = [
        camera.x,
        camera.y,
        camera.z,
        fog.ground_base_y,
        lightmap[0],
        lightmap[1],
        fog.shader_options(),
        lightmap[2],
        fog.color[0],
        fog.color[1],
        fog.color[2],
        fog.max_opacity,
        fog_distances[0],
        fog_distances[1],
        0.0,
        0.0,
    ];
    for (index, value) in values.into_iter().enumerate() {
        let start = FOG_CAMERA_OFFSET + index * size_of::<f32>();
        bytes[start..start + size_of::<f32>()].copy_from_slice(&value.to_le_bytes());
    }
    const RIGHT_VIEW_PROJECTION_OFFSET: usize = 18 * 16;
    const RIGHT_FOG_CAMERA_OFFSET: usize = 22 * 16;
    if let Some(right_view) = render_view_overrides[1] {
        let right_view_projection =
            super::terrain_relative_view_projection(right_view, presentation);
        for (index, value) in right_view_projection
            .to_cols_array()
            .into_iter()
            .enumerate()
        {
            let start = RIGHT_VIEW_PROJECTION_OFFSET + index * size_of::<f32>();
            bytes[start..start + size_of::<f32>()].copy_from_slice(&value.to_le_bytes());
        }
        for (index, value) in [
            right_view.camera_position.x,
            right_view.camera_position.y,
            right_view.camera_position.z,
            fog.ground_base_y,
        ]
        .into_iter()
        .enumerate()
        {
            let start = RIGHT_FOG_CAMERA_OFFSET + index * size_of::<f32>();
            bytes[start..start + size_of::<f32>()].copy_from_slice(&value.to_le_bytes());
        }
    }
    const MULTIVIEW_OPTIONS_OFFSET: usize = 23 * 16;
    bytes[MULTIVIEW_OPTIONS_OFFSET..MULTIVIEW_OPTIONS_OFFSET + size_of::<u32>()]
        .copy_from_slice(&view_mask.to_le_bytes());
    bytes[MULTIVIEW_OPTIONS_OFFSET + size_of::<u32>()
        ..MULTIVIEW_OPTIONS_OFFSET + 2 * size_of::<u32>()]
        .copy_from_slice(&(diagnostic as u32).to_le_bytes());
    bytes[MULTIVIEW_OPTIONS_OFFSET + 2 * size_of::<u32>()
        ..MULTIVIEW_OPTIONS_OFFSET + 3 * size_of::<u32>()]
        .copy_from_slice(&forest_reveal[0].clamp(0.0, 1.0).to_bits().to_le_bytes());
    bytes[MULTIVIEW_OPTIONS_OFFSET + 3 * size_of::<u32>()
        ..MULTIVIEW_OPTIONS_OFFSET + 4 * size_of::<u32>()]
        .copy_from_slice(&forest_reveal[1].clamp(0.0, 1.0).to_bits().to_le_bytes());
    bytes
}

fn terrain_horizon_tile_beyond_distance(
    tile: TerrainClipmapTile,
    camera: glam::Vec3,
    max_distance: f32,
) -> bool {
    let footprint = f64::from(tile.footprint_blocks());
    let min_x = tile.min_x() as f64;
    let min_z = tile.min_z() as f64;
    let camera_x = f64::from(camera.x);
    let camera_z = f64::from(camera.z);
    let nearest_x = camera_x.clamp(min_x, min_x + footprint);
    let nearest_z = camera_z.clamp(min_z, min_z + footprint);
    (camera_x - nearest_x).hypot(camera_z - nearest_z) > f64::from(max_distance)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerrainHorizonTileVisibility {
    Visible,
    InnerHole,
    Frustum,
    Far,
}

fn terrain_horizon_tile_view_mask(
    tile: TerrainClipmapTile,
    inner_hole: Option<super::TerrainClipmapBounds>,
    margin_blocks: f32,
    far_culls: [Option<(f32, glam::Vec3)>; 2],
    render_views: [Option<mclone_render::chunk::ChunkRenderView>; 2],
    presentation: super::TerrainPreviewUniformPresentation,
) -> u32 {
    let mut mask = 0_u32;
    let mut observed_view = false;
    for index in 0..render_views.len() {
        if render_views[index].is_none() && far_culls[index].is_none() {
            continue;
        }
        observed_view = true;
        if terrain_horizon_tile_visibility(
            tile,
            inner_hole,
            margin_blocks,
            far_culls[index],
            render_views[index],
            presentation,
        ) == TerrainHorizonTileVisibility::Visible
        {
            mask |= 1 << index;
        }
    }
    if observed_view { mask } else { 1 }
}

fn terrain_horizon_tile_visibility_for_views(
    tile: TerrainClipmapTile,
    inner_hole: Option<super::TerrainClipmapBounds>,
    margin_blocks: f32,
    far_culls: [Option<(f32, glam::Vec3)>; 2],
    render_views: [Option<mclone_render::chunk::ChunkRenderView>; 2],
    presentation: super::TerrainPreviewUniformPresentation,
) -> TerrainHorizonTileVisibility {
    let mut observed_view = false;
    let mut all_far = true;
    for index in 0..render_views.len() {
        if render_views[index].is_none() && far_culls[index].is_none() {
            continue;
        }
        observed_view = true;
        match terrain_horizon_tile_visibility(
            tile,
            inner_hole,
            margin_blocks,
            far_culls[index],
            render_views[index],
            presentation,
        ) {
            TerrainHorizonTileVisibility::Visible => {
                return TerrainHorizonTileVisibility::Visible;
            }
            TerrainHorizonTileVisibility::InnerHole => {
                return TerrainHorizonTileVisibility::InnerHole;
            }
            TerrainHorizonTileVisibility::Frustum => all_far = false,
            TerrainHorizonTileVisibility::Far => {}
        }
    }
    if !observed_view {
        return terrain_horizon_tile_visibility(
            tile,
            inner_hole,
            margin_blocks,
            None,
            None,
            presentation,
        );
    }
    if all_far {
        TerrainHorizonTileVisibility::Far
    } else {
        TerrainHorizonTileVisibility::Frustum
    }
}

fn exact_frontier_warming(
    exact_frontier_required: bool,
    exact_frontier_certifiable: bool,
    exact_coverage_stabilizing: bool,
    terrain_levels_empty: bool,
    refills_pending: bool,
    levels_staged: bool,
) -> bool {
    exact_frontier_required
        && !exact_frontier_certifiable
        && (exact_coverage_stabilizing || terrain_levels_empty || refills_pending || levels_staged)
}

const fn exact_frontier_preferred_support_allowed(
    exact_frontier_certifiable: bool,
    exact_coverage_stabilizing: bool,
) -> bool {
    exact_frontier_certifiable && !exact_coverage_stabilizing
}

fn terrain_horizon_tile_visibility(
    tile: TerrainClipmapTile,
    inner_hole: Option<super::TerrainClipmapBounds>,
    margin_blocks: f32,
    far_cull: Option<(f32, glam::Vec3)>,
    render_view: Option<mclone_render::chunk::ChunkRenderView>,
    presentation: super::TerrainPreviewUniformPresentation,
) -> TerrainHorizonTileVisibility {
    let footprint = i64::from(tile.footprint_blocks());
    let min_x = tile.min_x();
    let min_z = tile.min_z();
    let max_x = min_x + footprint;
    let max_z = min_z + footprint;
    let margin_i64 = margin_blocks.ceil() as i64;
    if inner_hole.is_some_and(|hole| {
        min_x - margin_i64 >= hole.min_x
            && min_z - margin_i64 >= hole.min_z
            && max_x + margin_i64 <= hole.max_x
            && max_z + margin_i64 <= hole.max_z
    }) {
        return TerrainHorizonTileVisibility::InnerHole;
    }
    if far_cull.is_some_and(|(distance, camera)| {
        terrain_horizon_tile_beyond_distance(tile, camera, distance + margin_blocks)
    }) {
        return TerrainHorizonTileVisibility::Far;
    }
    let Some(render_view) = render_view else {
        return TerrainHorizonTileVisibility::Visible;
    };
    let anchor_x = i64::from(presentation.anchor_x);
    let anchor_z = i64::from(presentation.anchor_z);
    let relative_min = glam::Vec3::new(
        (min_x - anchor_x) as f32 - presentation.fraction_x - margin_blocks,
        TERRAIN_HORIZON_CULL_MIN_Y,
        (min_z - anchor_z) as f32 - presentation.fraction_z - margin_blocks,
    );
    let relative_max = glam::Vec3::new(
        (max_x - anchor_x) as f32 - presentation.fraction_x + margin_blocks,
        TERRAIN_HORIZON_CULL_MAX_Y,
        (max_z - anchor_z) as f32 - presentation.fraction_z + margin_blocks,
    );
    let relative_view_projection =
        super::terrain_relative_view_projection(render_view, presentation);
    if terrain_horizon_clip_aabb_visible(relative_view_projection, relative_min, relative_max) {
        TerrainHorizonTileVisibility::Visible
    } else {
        TerrainHorizonTileVisibility::Frustum
    }
}

fn terrain_horizon_clip_aabb_visible(
    view_projection: glam::Mat4,
    min: glam::Vec3,
    max: glam::Vec3,
) -> bool {
    let corners = [
        glam::Vec3::new(min.x, min.y, min.z),
        glam::Vec3::new(max.x, min.y, min.z),
        glam::Vec3::new(min.x, max.y, min.z),
        glam::Vec3::new(max.x, max.y, min.z),
        glam::Vec3::new(min.x, min.y, max.z),
        glam::Vec3::new(max.x, min.y, max.z),
        glam::Vec3::new(min.x, max.y, max.z),
        glam::Vec3::new(max.x, max.y, max.z),
    ];
    let mut outside_left = true;
    let mut outside_right = true;
    let mut outside_bottom = true;
    let mut outside_top = true;
    let mut outside_near = true;
    let mut outside_far = true;
    for corner in corners {
        let clip = view_projection * corner.extend(1.0);
        outside_left &= clip.x < -clip.w;
        outside_right &= clip.x > clip.w;
        outside_bottom &= clip.y < -clip.w;
        outside_top &= clip.y > clip.w;
        // Preserve the renderer's conservative OpenGL-style near test. It is
        // deliberately wider than WebGPU's zero-to-w clip volume and avoids
        // dropping reversed-Z geometry at the near plane.
        outside_near &= clip.z < -clip.w;
        outside_far &= clip.z > clip.w;
    }
    !(outside_left || outside_right || outside_bottom || outside_top || outside_near || outside_far)
}

fn terrain_horizon_outer_edge_flags(
    level: &TerrainHorizonLevelPresentation,
    tile: TerrainClipmapTile,
    level_count: u32,
) -> u32 {
    if level.snapshot.level.saturating_add(1) >= level_count {
        return 0;
    }
    let min_tile_x = level
        .snapshot
        .tiles
        .iter()
        .map(|tile| tile.tile_x)
        .min()
        .unwrap_or(tile.tile_x);
    let max_tile_x = level
        .snapshot
        .tiles
        .iter()
        .map(|tile| tile.tile_x)
        .max()
        .unwrap_or(tile.tile_x);
    let min_tile_z = level
        .snapshot
        .tiles
        .iter()
        .map(|tile| tile.tile_z)
        .min()
        .unwrap_or(tile.tile_z);
    let max_tile_z = level
        .snapshot
        .tiles
        .iter()
        .map(|tile| tile.tile_z)
        .max()
        .unwrap_or(tile.tile_z);
    let mut flags = 0;
    if tile.tile_x == min_tile_x {
        flags |= TERRAIN_HORIZON_NORMAL_EDGE_WEST;
    }
    if tile.tile_x == max_tile_x {
        flags |= TERRAIN_HORIZON_NORMAL_EDGE_EAST;
    }
    if tile.tile_z == min_tile_z {
        flags |= TERRAIN_HORIZON_NORMAL_EDGE_NORTH;
    }
    if tile.tile_z == max_tile_z {
        flags |= TERRAIN_HORIZON_NORMAL_EDGE_SOUTH;
    }
    flags
}

fn terrain_horizon_tile_id(
    profile: TerrainPreviewProfile,
    seed: i64,
    content_stage: TerrainPreviewContentStage,
    tile: TerrainClipmapTile,
) -> TerrainViewportTileId {
    TerrainViewportTileId {
        profile,
        seed,
        tile_x: tile.tile_x,
        tile_z: tile.tile_z,
        sample_spacing: tile.sample_spacing,
        content_stage,
        surface_quality: TerrainPreviewSurfaceQuality::Inferred,
    }
}

fn maximum_terrain_vegetation_desired_tiles(
    config: TerrainClipmapConfig,
    vegetation_max_sample_spacing: u32,
) -> Result<usize, String> {
    let ratio = vegetation_max_sample_spacing
        .checked_div(config.base_sample_spacing)
        .ok_or("terrain vegetation base spacing is zero")?;
    let record_level_count = ratio
        .checked_ilog2()
        .ok_or("terrain vegetation record level range is empty")?
        .saturating_add(1)
        .min(config.level_count);
    usize::try_from(
        config
            .slots_per_level()
            .checked_mul(record_level_count)
            .ok_or("terrain vegetation desired-tile bound overflow")?,
    )
    .map_err(|_| "terrain vegetation desired-tile bound exceeds usize".to_owned())
}

fn terrain_horizon_vegetation_source(
    profile: TerrainPreviewProfile,
    seed: i64,
    content_stage: TerrainPreviewContentStage,
) -> Result<TerrainVegetationSourceIdentity, String> {
    TerrainVegetationSourceIdentity::for_request(TerrainPreviewRequest {
        profile,
        seed,
        center_x: 0,
        center_z: 0,
        sample_spacing: 1,
        cells_per_axis: TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS,
        topology: McloneOverworldSamplingTopology::Unbounded,
        content_stage,
        surface_quality: TerrainPreviewSurfaceQuality::Inferred,
    })
}

#[cfg(test)]
fn tree_instance_bytes(
    vegetation: &TerrainPreviewVegetationProduct,
) -> Result<(Vec<u8>, u32), String> {
    let (bytes, instance_count, suppressed_instance_count) =
        tree_instance_bytes_filtered(vegetation, &BTreeSet::new())?;
    debug_assert_eq!(suppressed_instance_count, 0);
    Ok((bytes, instance_count))
}

fn tree_instance_bytes_filtered(
    vegetation: &TerrainPreviewVegetationProduct,
    exact_owned_tree_ids: &BTreeSet<McloneTreeOccurrenceId>,
) -> Result<(Vec<u8>, u32, u32), String> {
    let included_occurrences = vegetation
        .occurrences()
        .iter()
        .filter(|occurrence| {
            !exact_owned_tree_ids.contains(&McloneTreeOccurrenceId::from(**occurrence))
        })
        .collect::<Vec<_>>();
    let instance_count = u32::try_from(included_occurrences.len())
        .map_err(|_| "terrain preview tree instance count exceeds u32")?;
    let suppressed_instance_count = u32::try_from(
        vegetation
            .occurrences()
            .len()
            .saturating_sub(included_occurrences.len()),
    )
    .map_err(|_| "terrain preview suppressed tree instance count exceeds u32")?;
    let byte_capacity = vegetation
        .occurrences()
        .len()
        .checked_mul(TERRAIN_PREVIEW_TREE_INSTANCE_BYTES as usize)
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or("terrain preview tree instance byte size overflow")?;
    let mut bytes = Vec::with_capacity(byte_capacity);
    for panel in 0..2 {
        for occurrence in &included_occurrences {
            let base = occurrence
                .working_base()
                .map_err(|error| format!("terrain preview tree base is invalid: {error}"))?;
            let family = match occurrence.record.family {
                McloneTreeFamily::TemperateBroadleaf => 1.0,
                McloneTreeFamily::CoolWetConifer => 2.0,
                McloneTreeFamily::WarmDryAcacia => 3.0,
                McloneTreeFamily::HumidJungleBroadleaf => 4.0,
            };
            let values = [
                base.x as f32,
                base.y as f32,
                base.z as f32,
                f32::from(occurrence.record.trunk_height),
                f32::from(occurrence.record.crown_radius),
                f32::from(occurrence.record.crown_depth),
                family,
                f32::from(occurrence.record.orientation),
                panel as f32,
                f32::from(occurrence.record.landmark_rank),
                0.0,
                0.0,
            ];
            for value in values {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    Ok((bytes, instance_count, suppressed_instance_count))
}

#[derive(Clone, Copy)]
struct RenderPanel {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    instance: u32,
}

fn render_panels(
    source: TerrainPreviewSource,
    split_layout: TerrainPreviewSplitLayout,
    width: u32,
    height: u32,
) -> Vec<RenderPanel> {
    if source != TerrainPreviewSource::Split {
        return vec![RenderPanel {
            x: 0,
            y: 0,
            width,
            height,
            instance: 0,
        }];
    }
    if split_layout == TerrainPreviewSplitLayout::Rows {
        let first_height = (height / 2).max(1);
        let second_height = height.saturating_sub(first_height).max(1);
        vec![
            RenderPanel {
                x: 0,
                y: 0,
                width,
                height: first_height,
                instance: 0,
            },
            RenderPanel {
                x: 0,
                y: first_height,
                width,
                height: second_height,
                instance: 1,
            },
        ]
    } else {
        let first_width = (width / 2).max(1);
        let second_width = width.saturating_sub(first_width).max(1);
        vec![
            RenderPanel {
                x: 0,
                y: 0,
                width: first_width,
                height,
                instance: 0,
            },
            RenderPanel {
                x: first_width,
                y: 0,
                width: second_width,
                height,
                instance: 1,
            },
        ]
    }
}

fn source_needs_cpu(
    options: TerrainPreviewDrawOptions,
    content_stage: TerrainPreviewContentStage,
    effective_spacing: u32,
) -> bool {
    !matches!(
        options.source,
        TerrainPreviewSource::Gpu | TerrainPreviewSource::Macro
    ) || options.layer == TerrainPreviewLayer::Error
        || (content_stage.includes_structured_hydrology()
            && effective_spacing <= TERRAIN_PREVIEW_MAX_TREE_RECORD_SAMPLE_SPACING)
}

fn source_needs_gpu(options: TerrainPreviewDrawOptions, profile: TerrainPreviewProfile) -> bool {
    match profile {
        TerrainPreviewProfile::McloneOverworldV1 => {
            matches!(
                options.source,
                TerrainPreviewSource::Gpu | TerrainPreviewSource::Split
            ) || options.layer == TerrainPreviewLayer::Error
        }
        TerrainPreviewProfile::VanillaOverworld => {
            matches!(
                options.source,
                TerrainPreviewSource::Macro | TerrainPreviewSource::Split
            ) || options.layer == TerrainPreviewLayer::Error
        }
        TerrainPreviewProfile::ContinentalEcoregionCandidate
        | TerrainPreviewProfile::McloneOverworldV2 => false,
    }
}

fn uniform_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    uniform_layout_entry_with_size(binding, visibility, TERRAIN_PREVIEW_UNIFORM_BYTES)
}

fn uniform_layout_entry_with_size(
    binding: u32,
    visibility: wgpu::ShaderStages,
    byte_len: u64,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(byte_len),
        },
        count: None,
    }
}

fn storage_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    read_only: bool,
    byte_len: u64,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(byte_len),
        },
        count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::terrain_preview::TerrainPreviewRequest;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_horizon_compiler_moves_continental_tiles_off_thread() {
        let mut compiler = TerrainHorizonCpuCompiler::new().unwrap();
        assert!((1..=TERRAIN_HORIZON_CPU_MAX_WORKERS).contains(&compiler.worker_count()));
        let resource = TerrainHorizonResourceTile {
            tile: TerrainClipmapTile {
                level: 9,
                tile_x: 0,
                tile_z: 0,
                sample_spacing: 512,
                physical_x: 0,
                physical_z: 0,
                physical_slot: 0,
            },
            resource_slot: 0,
            slot_generation: 1,
        };
        let request = TerrainPreviewRequest::new(12_345, 0, 0, 512)
            .with_profile(TerrainPreviewProfile::McloneOverworldV2)
            .with_content_stage(TerrainPreviewContentStage::Cover);
        assert!(
            compiler
                .try_submit(TerrainHorizonCpuCompileJob {
                    source_generation: 7,
                    resource,
                    request,
                })
                .unwrap()
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let result = loop {
            if let Some(result) = compiler.try_recv().unwrap() {
                break result;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "continental horizon worker did not complete its bounded tile"
            );
            std::thread::yield_now();
        };
        assert_eq!(result.source_generation, 7);
        assert_eq!(result.resource, resource);
        assert_eq!(result.request, request);
        let (grid, halo) = result.compiled.unwrap();
        assert_eq!(grid.samples().len(), 65 * 65);
        assert_eq!(halo.len(), 69 * 69);
        assert!(result.compile_micros > 0);
    }

    #[test]
    fn canopy_is_fixed_budget_and_available_across_v2_levels() {
        assert_eq!(TERRAIN_HORIZON_CANOPY_CELLS_PER_TILE, 256);
        assert_eq!(TERRAIN_HORIZON_CANOPY_VERTICES_PER_TILE, 3_072);
        let shader = super::super::TERRAIN_PREVIEW_TREE_WGSL;
        assert!(shader.contains("let proxy_opacity = select("));
        assert!(shader.contains("let eye_above_crown = eye.y - input.world_position.y;"));
        assert!(shader.contains("let aerial_suitability = smoothstep(6.0, 32.0"));
        assert_eq!(
            shader
                .matches("forest_representation_weight(input, blocks_per_pixel)")
                .count(),
            2
        );
        assert!(shader.contains("horizon_forest_reveal(params.multiview_options.z)"));
        assert!(shader.contains("horizon_forest_reveal(params.multiview_options.w)"));
        assert!(!terrain_horizon_level_uses_canopy(
            TerrainPreviewProfile::McloneOverworldV2,
            TerrainPreviewContentStage::Cover,
            1,
            1,
        ));
        assert!(terrain_horizon_level_uses_canopy(
            TerrainPreviewProfile::McloneOverworldV2,
            TerrainPreviewContentStage::Cover,
            4,
            4,
        ));
        assert!(terrain_horizon_level_uses_canopy(
            TerrainPreviewProfile::McloneOverworldV2,
            TerrainPreviewContentStage::Cover,
            8,
            4,
        ));
        assert!(!terrain_horizon_level_uses_canopy(
            TerrainPreviewProfile::McloneOverworldV2,
            TerrainPreviewContentStage::Surface,
            8,
            4,
        ));
        assert!(!terrain_horizon_level_uses_canopy(
            TerrainPreviewProfile::McloneOverworldV1,
            TerrainPreviewContentStage::Cover,
            8,
            4,
        ));
        assert!(!terrain_horizon_level_uses_canopy(
            TerrainPreviewProfile::VanillaOverworld,
            TerrainPreviewContentStage::Cover,
            8,
            4,
        ));
        assert_eq!(
            terrain_horizon_overview_minimum_sample_spacing(8_192.0, 1_280, false, 512,),
            8,
        );
        assert_eq!(
            terrain_horizon_overview_minimum_sample_spacing(512.0, 1_280, false, 512,),
            1,
        );
        assert_eq!(
            terrain_horizon_overview_minimum_sample_spacing(8_192.0, 1_280, true, 512,),
            1,
        );
        assert_eq!(
            terrain_horizon_overview_minimum_sample_spacing(8_192.0, 1_280, false, 4,),
            4,
        );
    }

    #[test]
    fn forest_reveal_is_time_based_and_clamps_long_pauses() {
        let at_30_hz = (0..18).fold(0.0, |value, _| advance_forest_reveal(value, 1.0 / 30.0));
        let at_120_hz = (0..72).fold(0.0, |value, _| advance_forest_reveal(value, 1.0 / 120.0));
        assert!((at_30_hz - at_120_hz).abs() < 0.000_1);
        assert!(at_30_hz > 0.9 && at_30_hz < 1.0);

        let after_long_pause = advance_forest_reveal(0.0, 20.0);
        assert!(after_long_pause > 0.15 && after_long_pause < 0.16);
    }

    #[test]
    fn compare_panels_match_shader_layout() {
        assert_eq!(
            render_panels(
                TerrainPreviewSource::Split,
                TerrainPreviewSplitLayout::Columns,
                1200,
                700
            )
            .len(),
            2
        );
        let wide = render_panels(
            TerrainPreviewSource::Split,
            TerrainPreviewSplitLayout::Columns,
            1200,
            900,
        );
        assert_eq!((wide[0].width, wide[1].x), (600, 600));
        let tall = render_panels(
            TerrainPreviewSource::Split,
            TerrainPreviewSplitLayout::Rows,
            1200,
            900,
        );
        assert_eq!((tall[0].height, tall[1].y), (450, 450));
        assert_eq!(
            render_panels(
                TerrainPreviewSource::Gpu,
                TerrainPreviewSplitLayout::Rows,
                400,
                900
            )
            .len(),
            1
        );
    }

    #[test]
    fn compute_template_is_still_the_shared_production_kernel() {
        assert!(
            super::super::TERRAIN_PREVIEW_COMPUTE_WGSL_TEMPLATE.contains("fn land_surface_height")
        );
        assert_eq!(
            mclone_worldgen::terrain_preview::TERRAIN_PREVIEW_SAMPLE_FLOATS,
            usize::try_from(TERRAIN_PREVIEW_SAMPLE_BYTES / 4).unwrap()
        );
    }

    #[test]
    fn horizon_halo_keeps_draw_topology_fixed_and_reports_its_cost() {
        let drawn_samples_per_axis = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS + 1;
        assert_eq!(drawn_samples_per_axis, 65);
        assert_eq!(terrain_horizon_samples_per_axis(), 69);
        assert_eq!(terrain_horizon_normal_halo_samples_per_tile(), 536);
        assert_eq!(
            terrain_horizon_normal_height_byte_len().unwrap(),
            u64::from(69_u32.pow(2)) * size_of::<f32>() as u64
        );
    }

    #[test]
    fn horizon_outer_edges_select_the_coarse_normal_footprint() {
        let mut clipmap = TerrainClipmap::new(TerrainClipmapConfig {
            level_count: 2,
            tiles_per_axis: 4,
            base_sample_spacing: 1,
        })
        .unwrap();
        clipmap.update_center(0, 0);
        let snapshots = clipmap.levels();
        let presentation =
            |snapshot: super::super::TerrainClipmapLevelSnapshot| TerrainHorizonLevelPresentation {
                tiles: snapshot
                    .tiles
                    .iter()
                    .enumerate()
                    .map(|(slot, tile)| TerrainHorizonResourceTile {
                        tile: *tile,
                        resource_slot: slot as u32,
                        slot_generation: 1,
                    })
                    .collect(),
                snapshot,
            };
        let fine = presentation(snapshots[0].clone());
        let west_north = fine
            .tiles
            .iter()
            .find(|resource| {
                resource.tile.tile_x == fine.snapshot.origin_tile_x
                    && resource.tile.tile_z == fine.snapshot.origin_tile_z
            })
            .unwrap()
            .tile;
        assert_eq!(
            terrain_horizon_outer_edge_flags(&fine, west_north, 2),
            TERRAIN_HORIZON_NORMAL_EDGE_WEST | TERRAIN_HORIZON_NORMAL_EDGE_NORTH
        );
        let interior = fine
            .tiles
            .iter()
            .find(|resource| {
                resource.tile.tile_x == fine.snapshot.origin_tile_x + 1
                    && resource.tile.tile_z == fine.snapshot.origin_tile_z + 1
            })
            .unwrap()
            .tile;
        assert_eq!(terrain_horizon_outer_edge_flags(&fine, interior, 2), 0);

        let coarse = presentation(snapshots[1].clone());
        assert_eq!(
            terrain_horizon_outer_edge_flags(&coarse, coarse.tiles[0].tile, 2),
            0
        );

        let shared_boundary = 256_i32;
        let fine_wide_footprint = (
            shared_boundary - 2 * fine.snapshot.sample_spacing as i32,
            shared_boundary + 2 * fine.snapshot.sample_spacing as i32,
        );
        let coarse_narrow_footprint = (
            shared_boundary - coarse.snapshot.sample_spacing as i32,
            shared_boundary + coarse.snapshot.sample_spacing as i32,
        );
        assert_eq!(fine_wide_footprint, coarse_narrow_footprint);
    }

    #[test]
    fn horizon_uses_only_smooth_geometry_and_the_exact_connector() {
        for spacing in [1, 2, 4, 8, 16, 32, 64, 128, 256, 512] {
            assert_eq!(
                terrain_horizon_vertices_per_cell(spacing),
                TERRAIN_HORIZON_SMOOTH_VERTICES_PER_CELL
            );
        }

        let shader = super::super::TERRAIN_PREVIEW_RENDER_WGSL;
        assert!(shader.contains("u32(local.y) * 65u"));
        assert!(super::super::TERRAIN_PREVIEW_TREE_WGSL.contains("u32(local.y) * 65u"));
        assert!(shader.contains("let cell_index = vertex_index / 6u;"));
        assert!(shader.contains("let appearance_transition_weight = exact_transition_weight"));
        assert!(shader.contains("var exact_boundary_profile: texture_2d<u32>;"));
        assert!(shader.contains("fn exact_connector_vertex("));
        assert!(shader.contains("bottom_y = min(procedural_y, exact_y)"));
        assert!(shader.contains("vec2<f32>(world_z, -world_y)"));
        assert!(shader.contains("vec2<f32>(world_x, -world_y)"));
        assert!(shader.contains("fn exact_connector_vertex_main("));
        assert!(shader.contains("fn frontier_connector_vertex("));
        assert!(shader.contains("fn frontier_connector_height("));
        assert!(shader.contains("TERRAIN_FRONTIER_CONNECTOR_WATER_FLAG"));
        assert!(shader.contains("TERRAIN_FRONTIER_CONNECTOR_OUTER_FLAG"));
        assert!(shader.contains("frontier_support_tile_selected(input.world_xz)"));
        assert!(shader.contains("frontier_support_lookup.rows[u32(local.y)]"));
        assert!(!shader.contains("for (var index = 0u; index < TERRAIN_FRONTIER_SUPPORT"));
        assert!(shader.contains(
            "if u32(params.origin_spacing_cells.z) > 1u\n        && frontier_support_tile_selected"
        ));
        assert!(shader.contains("out.side_surface = 1u;"));
        assert!(!shader.contains("let vertices_per_cell = select("));
        assert!(!shader.contains("round(stitched_height)"));
    }

    #[test]
    fn frontier_support_lookup_is_bounded_direct_and_negative_safe() {
        let tiles = [
            TerrainFrontierFineTileKey {
                tile_x: -9,
                tile_z: -5,
            },
            TerrainFrontierFineTileKey {
                tile_x: 8,
                tile_z: -5,
            },
            TerrainFrontierFineTileKey {
                tile_x: -1,
                tile_z: 3,
            },
            TerrainFrontierFineTileKey {
                tile_x: 8,
                tile_z: 12,
            },
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let lookup = TerrainFrontierSupportLookup::from_tiles(&tiles).unwrap();
        assert_eq!((lookup.origin_tile_x, lookup.origin_tile_z), (-9, -5));
        assert_eq!((lookup.width, lookup.height), (18, 18));
        assert_eq!(lookup.selected_count(), tiles.len() as u32);
        for tile in &tiles {
            assert!(lookup.contains(*tile));
        }
        assert!(!lookup.contains(TerrainFrontierFineTileKey {
            tile_x: -8,
            tile_z: -5,
        }));
        assert!(!lookup.contains(TerrainFrontierFineTileKey {
            tile_x: 9,
            tile_z: 12,
        }));
        assert_eq!(lookup.bytes().len(), 96);

        let too_wide = [
            TerrainFrontierFineTileKey {
                tile_x: -9,
                tile_z: 0,
            },
            TerrainFrontierFineTileKey {
                tile_x: 9,
                tile_z: 0,
            },
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert!(TerrainFrontierSupportLookup::from_tiles(&too_wide).is_err());
    }

    #[test]
    fn exact_connector_instances_are_perimeter_bounded() {
        let source =
            TerrainCompositionSourceIdentity::new(TerrainPreviewProfile::McloneOverworldV1, 12_345);
        let coverage = ExactPaintedCoverageSnapshot::new(source, 7, [ChunkPos::new(0, 0)]).unwrap();
        let columns = crate::terrain_exact_exposed_boundary_blocks(
            &coverage,
            mclone_core::HorizontalTopology::UNBOUNDED,
        )
        .unwrap()
        .into_iter()
        .map(|[world_x, world_z]| crate::TerrainExactBoundaryColumn {
            world_x,
            world_z,
            solid_top_y: 72,
            side_material: Some(4),
            water: false,
        });
        let boundary = TerrainExactBoundaryProfile::from_columns(&coverage, columns).unwrap();
        let instances =
            terrain_exact_connector_instances(&coverage.packed_mask().unwrap(), &boundary);
        assert_eq!(instances.len(), 16 * 4);
        assert!(instances.iter().all(|instance| instance.side < 4));
    }

    #[test]
    fn frontier_connectors_have_one_typed_owner_and_outer_closure() {
        let source =
            TerrainCompositionSourceIdentity::new(TerrainPreviewProfile::McloneOverworldV1, 12_345);
        let coverage = ExactPaintedCoverageSnapshot::new(
            source,
            9,
            (-8..=8).flat_map(|z| (-8..=8).map(move |x| ChunkPos::new(x, z))),
        )
        .unwrap();
        let boundary = TerrainExactBoundaryProfile::from_columns(
            &coverage,
            crate::terrain_exact_exposed_boundary_blocks(&coverage, HorizontalTopology::UNBOUNDED)
                .unwrap()
                .into_iter()
                .map(|[world_x, world_z]| crate::TerrainExactBoundaryColumn {
                    world_x,
                    world_z,
                    solid_top_y: 72,
                    side_material: Some(4),
                    water: (world_x + world_z).rem_euclid(5) == 0,
                }),
        )
        .unwrap();
        let mut clipmap = TerrainClipmap::new(TerrainClipmapConfig {
            level_count: 6,
            tiles_per_axis: 4,
            base_sample_spacing: 1,
        })
        .unwrap();
        clipmap.update_center(8, 8);
        let plan = TerrainFrontierPlan::prepare(
            &coverage,
            &boundary,
            HorizontalTopology::UNBOUNDED,
            [8, 8],
            &clipmap.levels(),
            TerrainFrontierPlanOptions::default(),
        )
        .unwrap();
        let proof = TerrainFrontierTopology::prepare(
            &plan,
            TerrainFrontierTopologyOptions {
                fine_tile_capacity: 1,
            },
        )
        .unwrap();
        let instances = terrain_frontier_connector_instances(&proof).unwrap();
        let boundary_instances = instances.iter().filter(|instance| !instance.outer).count();
        let outer_instances = instances.iter().filter(|instance| instance.outer).count();
        assert_eq!(
            boundary_instances,
            proof.receipt().certified_segments as usize
        );
        assert_eq!(
            outer_instances,
            proof.receipt().outer_stitch_segments as usize
        );
        assert!(
            instances
                .iter()
                .all(|instance| { instance.geometry.side & TERRAIN_FRONTIER_CONNECTOR_FLAG != 0 })
        );
        assert!(instances.iter().any(|instance| instance.fallback));
        assert!(instances.iter().any(|instance| instance.water));
        assert!(instances.iter().any(|instance| instance.outer));
        for instance in &instances {
            assert!(
                instance.owned_by(
                    TerrainViewportTileId {
                        profile: TerrainPreviewProfile::McloneOverworldV1,
                        seed: 12_345,
                        tile_x: instance.owner_tile_x,
                        tile_z: instance.owner_tile_z,
                        sample_spacing: instance.owner_sample_spacing,
                        content_stage: TerrainPreviewContentStage::Cover,
                        surface_quality: TerrainPreviewSurfaceQuality::Inferred,
                    }
                    .preview_request()
                    .validate()
                    .unwrap()
                )
            );
        }
    }

    #[test]
    fn horizon_transition_uses_active_pack_top_and_connector_faces() {
        let table =
            TerrainPreviewMaterialTable::from_catalog(&mclone_mesh::TexturedMeshCatalog::default());
        assert_eq!(
            table.uniform_bytes().len(),
            TERRAIN_PREVIEW_MATERIAL_TABLE_BYTES as usize
        );

        let shader = super::super::terrain_preview_render_wgsl(
            mclone_render_color::RenderTargetColorTransform::Identity,
        );
        assert!(shader.contains("material_table.side_uvs[material]"));
        assert!(shader.contains("material_table.grass_tints"));
        assert!(shader.contains("material_table.water_tints"));
        assert!(shader.contains("material_uses_grass_tint(input.material, false)"));
        assert!(shader.contains("full_sky_environmental_illumination()"));
        assert!(shader.contains("if display_material == 2u"));
        assert!(shader.contains("if appearance_weight > 0.0"));
        assert!(!shader.contains("} else if input.textured != 0u && input.material < 256u"));
        assert!(shader.contains("exact_transition_weight(world_position.xz)"));
    }

    #[test]
    fn horizon_diagnostics_isolate_surface_terms_in_terrain_and_trees() {
        let terrain = super::super::TERRAIN_PREVIEW_RENDER_WGSL;
        let trees = super::super::TERRAIN_PREVIEW_TREE_WGSL;
        for shader in [terrain, trees] {
            assert!(shader.contains("params.multiview_options.y"));
            assert!(shader.contains("let horizon_diagnostic ="));
            assert!(shader.contains("TERRAIN_HORIZON_DIAGNOSTIC_ALBEDO"));
            assert!(shader.contains("TERRAIN_HORIZON_DIAGNOSTIC_ENVIRONMENT"));
            assert!(shader.contains("TERRAIN_HORIZON_DIAGNOSTIC_GEOMETRY"));
            assert!(shader.contains("TERRAIN_HORIZON_DIAGNOSTIC_OCCLUSION"));
            assert!(shader.contains("TERRAIN_HORIZON_DIAGNOSTIC_FRONTIER_SUPPORT"));
        }
        assert!(terrain.contains("fn exact_frontier_adjacent("));
        assert!(terrain.contains("diagnostic_river_alpha"));
        assert!(terrain.contains("material_texture_weight("));
    }

    #[test]
    fn horizon_environment_is_topology_independent_for_terrain_water_and_trees() {
        let terrain = super::super::TERRAIN_PREVIEW_RENDER_WGSL;
        let trees = super::super::TERRAIN_PREVIEW_TREE_WGSL;
        for shader in [terrain, trees] {
            assert!(shader.contains("fn full_sky_environmental_illumination()"));
            assert!(shader.contains(
                "let environmental_illumination = full_sky_environmental_illumination();"
            ));
            assert!(shader.contains("color = environmental_illumination;"));
        }
        assert!(terrain.contains("color *= environmental_illumination;"));
        assert!(!terrain.contains("* near_surface_lightmap()"));
        assert!(trees.contains("input.color.rgb * input.color.a * environmental_illumination"));
    }

    #[test]
    fn horizon_smooth_shade_keeps_one_geometry_owner() {
        let shader = super::super::TERRAIN_PREVIEW_RENDER_WGSL;
        assert!(shader.contains("let smooth_geometric_shade = clamp("));
        assert!(shader.contains("let light = smooth_geometric_shade;"));
        assert!(!shader.contains("voxel_smooth_transition_weight"));
        assert!(!shader.contains("TERRAIN_HORIZON_VOXEL"));
    }

    #[test]
    fn horizon_surface_response_converges_from_exact_to_smooth_terrain() {
        let shader = super::super::TERRAIN_PREVIEW_RENDER_WGSL;
        assert!(shader.contains("fn material_uses_grass_tint("));
        assert!(shader.contains("albedo = surface_tint(input, input.material, false);"));
        assert!(shader.contains("let resolved_exact_weight = clamp(exact_weight"));
        assert!(shader.contains("let exact_aligned_height = floor(stitched_height);"));
        assert!(shader.contains("let summary_presentation_weight = 1.0 - input.world_position.w;"));
        assert!(shader.matches("production_mclone_profile(),").count() >= 2);
        assert!(
            !shader.contains("mclone_grass_biome(input.biome),\n        preview_profile() == 0u,")
        );
        assert_eq!(shader.matches("textured_mclone_profile(),").count(), 3);
        assert_eq!(shader.matches("input.world_position.w,").count(), 3);
        // The ordinary exact connector and isolated frontier-proof connector
        // each produce a typed vertical side surface.
        assert_eq!(shader.matches("out.side_surface = 1u;").count(), 2);
    }

    #[test]
    fn horizon_exact_coverage_discards_every_procedural_surface_class() {
        let shader = super::super::TERRAIN_PREVIEW_RENDER_WGSL;
        assert!(!shader.contains("exact_chunk_painted_interior"));
        assert!(!shader.contains("let collar = 1.5"));
        assert!(shader.contains(
            "if exact_coverage.mode_count_generation.x == 1u\n        \
             && exact_painted"
        ));
        assert!(!shader.contains(
            "if exact_coverage.mode_count_generation.x == 1u\n        \
             && input.material != 2u"
        ));
        assert_eq!(TERRAIN_EXACT_FRONTIER_TREE_INSET_BLOCKS, 0.0);
    }

    #[test]
    fn horizon_water_handoff_uses_only_the_procedural_side_transition_band() {
        let shader = super::super::TERRAIN_PREVIEW_RENDER_WGSL;
        assert!(shader.contains("const TERRAIN_EXACT_WATER_TRANSITION_MIN_WEIGHT: f32 = 0.75;"));
        assert!(shader.contains("let water_tint = surface_water_tint(input);"));
        assert!(shader.contains("near_color = mix(far_color, exact_water_albedo, water_tint.a);"));
        assert!(shader.contains("appearance_weight = smoothstep("));
        assert!(shader.contains("TERRAIN_EXACT_WATER_TRANSITION_MIN_WEIGHT,"));
    }

    #[test]
    fn horizon_shader_welds_fine_outer_edges_to_coarse_interpolation() {
        let shader = super::super::TERRAIN_PREVIEW_RENDER_WGSL;
        assert!(shader.contains("fn terrain_horizon_geometry_height("));
        assert!(shader.contains("let rendered_x = sample_x / cell_stride;"));
        assert!(shader.contains("let rendered_z = sample_z / cell_stride;"));
        assert!(shader.contains("west_or_east && (rendered_z & 1) != 0"));
        assert!(shader.contains("north_or_south && (rendered_x & 1) != 0"));
        assert!(shader.contains("let vertex_world_y = mix("));
        assert!(shader.contains("let tile_overlap = f32(params.origin_spacing_cells.z) * 0.5;"));
        assert!(shader.contains("vertex_world_x -= tile_overlap;"));
        assert!(shader.contains("vertex_world_x += tile_overlap;"));
        assert!(shader.contains("vertex_world_z -= tile_overlap;"));
        assert!(shader.contains("vertex_world_z += tile_overlap;"));
        assert_eq!(
            shader
                .matches("let left = terrain_horizon_geometry_height(")
                .count(),
            1
        );
        assert_eq!(
            shader
                .matches("let wide_left = terrain_horizon_geometry_height(")
                .count(),
            1
        );
    }

    #[test]
    fn horizon_shader_uses_biome_ground_color_and_interpolated_pool_water() {
        let shader = super::super::TERRAIN_PREVIEW_RENDER_WGSL;
        for mapping in [
            "case 0u: { return 0u; }",  // ocean
            "case 1u: { return 16u; }", // shore
            "case 2u: { return 7u; }",  // river
            "case 3u: { return 13u; }", // snowy alpine
            "case 4u: { return 5u; }",  // conifer
            "case 5u: { return 35u; }", // steppe
            "case 6u: { return 4u; }",  // woodland
            "default: { return 1u; }",  // meadow
        ] {
            assert!(shader.contains(mapping), "missing biome mapping {mapping}");
        }
        assert!(shader.contains("mclone_grass_biome(u32(round(sample.semantics.y)))"));
        assert!(shader.contains("let pool_anti_alias = max(fwidth(input.semantics.y), 0.01);"));
        assert!(shader.contains("&& input.material != 2u"));
        assert!(shader.contains("let water_alpha = max(river_alpha, pool_alpha);"));
        assert!(shader.contains("water_surface_color(input.river.w, presentation_light)"));
        assert!(shader.contains("sample.terrain.x,"));
        assert!(!shader.contains("63.0 - input.position.z"));
        assert!(!shader.contains("let dry = vec3<f32>(0.63, 0.54, 0.29);"));
    }

    #[test]
    fn tree_instances_preserve_semantics_for_both_compare_panels() {
        let request = TerrainPreviewRequest::new(12_345, -80, 48, 4)
            .with_content_stage(TerrainPreviewContentStage::Cover);
        let product = TerrainPreviewVegetationProduct::compile(request).unwrap();
        let (bytes, instance_count) = tree_instance_bytes(&product).unwrap();

        assert!(instance_count > 0);
        assert_eq!(
            bytes.len(),
            usize::try_from(instance_count).unwrap()
                * TERRAIN_PREVIEW_TREE_INSTANCE_BYTES as usize
                * 2
        );
        let values = bytes
            .chunks_exact(size_of::<f32>())
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();
        let panel_stride =
            usize::try_from(instance_count).unwrap() * TERRAIN_PREVIEW_TREE_INSTANCE_FLOATS;
        assert_eq!(values[8], 0.0);
        assert_eq!(values[panel_stride + 8], 1.0);
        assert!((1.0..=3.0).contains(&values[6]));
        assert_eq!(values[9], 3.0);
    }

    #[test]
    fn exact_owned_tree_is_removed_as_one_complete_proxy_instance() {
        let request = TerrainPreviewRequest::new(12_345, -80, 48, 4)
            .with_content_stage(TerrainPreviewContentStage::Cover);
        let product = TerrainPreviewVegetationProduct::compile(request).unwrap();
        let exact_owned = McloneTreeOccurrenceId::from(product.occurrences()[0]);
        let (bytes, instance_count, suppressed_instance_count) =
            tree_instance_bytes_filtered(&product, &BTreeSet::from([exact_owned])).unwrap();

        assert_eq!(suppressed_instance_count, 1);
        assert_eq!(
            instance_count.saturating_add(suppressed_instance_count),
            product.occurrences().len() as u32
        );
        assert_eq!(
            bytes.len(),
            usize::try_from(instance_count).unwrap()
                * TERRAIN_PREVIEW_TREE_INSTANCE_BYTES as usize
                * 2
        );
    }

    #[test]
    fn source_lanes_are_independent_except_for_error_comparison() {
        let mut options = TerrainPreviewDrawOptions::default();
        assert!(source_needs_cpu(
            options,
            TerrainPreviewContentStage::Base,
            1
        ));
        assert!(!source_needs_gpu(
            options,
            TerrainPreviewProfile::McloneOverworldV1,
        ));

        options.source = TerrainPreviewSource::Gpu;
        assert!(!source_needs_cpu(
            options,
            TerrainPreviewContentStage::Base,
            1
        ));
        assert!(source_needs_gpu(
            options,
            TerrainPreviewProfile::McloneOverworldV1,
        ));

        options.source = TerrainPreviewSource::Split;
        assert!(source_needs_cpu(
            options,
            TerrainPreviewContentStage::Base,
            1
        ));
        assert!(source_needs_gpu(
            options,
            TerrainPreviewProfile::McloneOverworldV1,
        ));

        options.source = TerrainPreviewSource::Reference;
        options.layer = TerrainPreviewLayer::Error;
        assert!(source_needs_cpu(
            options,
            TerrainPreviewContentStage::Base,
            1
        ));
        assert!(source_needs_gpu(
            options,
            TerrainPreviewProfile::McloneOverworldV1,
        ));
        assert!(source_needs_gpu(
            options,
            TerrainPreviewProfile::VanillaOverworld,
        ));
        options.layer = TerrainPreviewLayer::Terrain;
        options.source = TerrainPreviewSource::Macro;
        assert!(!source_needs_cpu(
            options,
            TerrainPreviewContentStage::Surface,
            32,
        ));
        assert!(source_needs_gpu(
            options,
            TerrainPreviewProfile::VanillaOverworld,
        ));
        assert!(source_needs_cpu(
            TerrainPreviewDrawOptions {
                source: TerrainPreviewSource::Gpu,
                ..TerrainPreviewDrawOptions::default()
            },
            TerrainPreviewContentStage::Structured,
            4,
        ));
        assert!(!source_needs_cpu(
            TerrainPreviewDrawOptions {
                source: TerrainPreviewSource::Gpu,
                ..TerrainPreviewDrawOptions::default()
            },
            TerrainPreviewContentStage::Cover,
            8,
        ));
    }

    #[test]
    fn horizon_far_cull_rejects_only_tiles_wholly_beyond_the_radius() {
        let near = TerrainClipmapTile {
            level: 0,
            tile_x: 0,
            tile_z: 0,
            sample_spacing: 1,
            physical_x: 0,
            physical_z: 0,
            physical_slot: 0,
        };
        let far = TerrainClipmapTile { tile_x: 10, ..near };
        let camera = glam::Vec3::new(16.0, 300.0, 16.0);

        assert!(!terrain_horizon_tile_beyond_distance(near, camera, 32.0));
        assert!(terrain_horizon_tile_beyond_distance(far, camera, 32.0));
    }

    #[test]
    fn incomplete_exact_frontier_is_nonfatal_only_while_work_remains() {
        assert!(exact_frontier_warming(
            true, false, true, false, false, false
        ));
        assert!(exact_frontier_warming(
            true, false, false, true, false, false
        ));
        assert!(exact_frontier_warming(
            true, false, false, false, true, false
        ));
        assert!(exact_frontier_warming(
            true, false, false, false, false, true
        ));
        assert!(!exact_frontier_warming(
            true, false, false, false, false, false
        ));
        assert!(!exact_frontier_warming(true, true, true, true, true, true));
        assert!(!exact_frontier_warming(
            false, false, true, true, true, true
        ));
        assert!(!exact_frontier_preferred_support_allowed(true, true));
        assert!(exact_frontier_preferred_support_allowed(true, false));
        assert!(!exact_frontier_preferred_support_allowed(false, false));
    }

    #[test]
    fn horizon_multiview_culling_keeps_union_but_masks_each_eye() {
        let tile = TerrainClipmapTile {
            level: 0,
            tile_x: 0,
            tile_z: 0,
            sample_spacing: 1,
            physical_x: 0,
            physical_z: 0,
            physical_slot: 0,
        };
        let presentation = crate::TerrainPreviewUniformPresentation::integer(0, 0, 256, 256);
        let near = glam::Vec3::new(16.0, 300.0, 16.0);
        let far = glam::Vec3::new(1_000.0, 300.0, 1_000.0);

        assert_eq!(
            terrain_horizon_tile_visibility_for_views(
                tile,
                None,
                0.0,
                [Some((32.0, near)), Some((32.0, far))],
                [None, None],
                presentation,
            ),
            TerrainHorizonTileVisibility::Visible,
            "a tile visible to either eye remains admitted to the shared draw"
        );
        assert_eq!(
            terrain_horizon_tile_view_mask(
                tile,
                None,
                0.0,
                [Some((32.0, near)), Some((32.0, far))],
                [None, None],
                presentation,
            ),
            0b01
        );
        assert_eq!(
            terrain_horizon_tile_view_mask(
                tile,
                None,
                0.0,
                [Some((32.0, far)), Some((32.0, near))],
                [None, None],
                presentation,
            ),
            0b10
        );
        assert_eq!(
            terrain_horizon_tile_visibility_for_views(
                tile,
                None,
                0.0,
                [Some((32.0, far)), Some((32.0, far))],
                [None, None],
                presentation,
            ),
            TerrainHorizonTileVisibility::Far
        );
    }

    #[test]
    fn horizon_tile_culling_preserves_partial_hole_and_tree_margin() {
        let tile = TerrainClipmapTile {
            level: 0,
            tile_x: 0,
            tile_z: 0,
            sample_spacing: 1,
            physical_x: 0,
            physical_z: 0,
            physical_slot: 0,
        };
        let presentation = crate::TerrainPreviewUniformPresentation::integer(0, 0, 256, 256);
        let exact_tile_hole = crate::TerrainClipmapBounds {
            min_x: 0,
            min_z: 0,
            max_x: 64,
            max_z: 64,
        };
        assert_eq!(
            terrain_horizon_tile_visibility(
                tile,
                Some(exact_tile_hole),
                0.0,
                None,
                None,
                presentation,
            ),
            TerrainHorizonTileVisibility::InnerHole
        );
        assert_eq!(
            terrain_horizon_tile_visibility(
                tile,
                Some(exact_tile_hole),
                TERRAIN_HORIZON_TREE_CULL_MARGIN_BLOCKS,
                None,
                None,
                presentation,
            ),
            TerrainHorizonTileVisibility::Visible,
            "tree crowns crossing the hole edge must remain drawable"
        );

        let partial_hole = crate::TerrainClipmapBounds {
            min_x: 16,
            min_z: 16,
            max_x: 64,
            max_z: 64,
        };
        assert_eq!(
            terrain_horizon_tile_visibility(
                tile,
                Some(partial_hole),
                0.0,
                None,
                None,
                presentation,
            ),
            TerrainHorizonTileVisibility::Visible
        );
    }

    #[test]
    fn horizon_clip_aabb_rejects_only_wholly_outside_boxes() {
        let identity = glam::Mat4::IDENTITY;
        assert!(terrain_horizon_clip_aabb_visible(
            identity,
            glam::Vec3::splat(-0.5),
            glam::Vec3::splat(0.5),
        ));
        assert!(!terrain_horizon_clip_aabb_visible(
            identity,
            glam::Vec3::new(2.0, -0.5, -0.5),
            glam::Vec3::new(3.0, 0.5, 0.5),
        ));
        assert!(terrain_horizon_clip_aabb_visible(
            identity,
            glam::Vec3::new(0.5, -0.5, -0.5),
            glam::Vec3::new(1.5, 0.5, 0.5),
        ));
    }
}
