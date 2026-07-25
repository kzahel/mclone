use mclone_core::BlockStateId;
use mclone_terrain_view::{
    CanonicalTerrainCompiler, CanonicalTerrainVisibility, TERRAIN_PREVIEW_GPU_EVALUATOR_REVISION,
    TERRAIN_PREVIEW_MATERIAL_UV_COUNT, TerrainPreviewCamera, TerrainPreviewLayer,
    TerrainPreviewMaterialAtlas, TerrainPreviewProjectionKind, TerrainPreviewSource,
    TerrainPreviewView, TerrainViewportCompletedComparison, TerrainViewportDetail,
    TerrainViewportExternalCpuRequest, TerrainViewportFrameStats, TerrainViewportRenderer,
    TerrainViewportRequest, TerrainViewportTileId, canonical_terrain_chunk_order,
    plan_terrain_viewport, terrain_preview_focus_y, terrain_preview_projection,
};
use mclone_worldgen::{
    levelgen::{
        MCLONE_OVERWORLD_DECORATION_REVISION, MCLONE_OVERWORLD_VEGETATION_REVISION,
        McloneOverworldDebugSample, McloneOverworldSampler, McloneOverworldSamplingTopology,
        VANILLA_OVERWORLD_MACRO_LOD_REVISION, VanillaOverworldLodSampler,
        VanillaOverworldMacroSampler, mclone_overworld_debug_sample_with_streams,
    },
    terrain_preview::{
        TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS, TERRAIN_PREVIEW_REFERENCE_SCHEMA_REVISION,
        TerrainPreviewContentStage, TerrainPreviewProfile, TerrainPreviewReferenceGrid,
        terrain_preview_field_revision,
    },
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
use web_sys::HtmlCanvasElement;

use crate::{
    canonical_mesh::{CanonicalMeshCoordinate, CanonicalMeshSession},
    canonical_terrain_stage, canonical_terrain_stage_label, terrain_preview_content_stage,
    terrain_preview_option_labels, terrain_preview_options, terrain_preview_projection_kind,
    terrain_preview_split_layout,
    visual_assets::load_terrain_lab_visual_assets,
};

#[wasm_bindgen(js_name = VanillaTerrainLodCompiler)]
pub struct TerrainLabVanillaLodCompiler {
    sampler: VanillaOverworldLodSampler,
}

#[wasm_bindgen(js_class = VanillaTerrainLodCompiler)]
impl TerrainLabVanillaLodCompiler {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: String) -> Result<TerrainLabVanillaLodCompiler, JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        Ok(Self {
            sampler: VanillaOverworldLodSampler::new(seed),
        })
    }

    pub fn compile(
        &mut self,
        tile_x: i32,
        tile_z: i32,
        sample_spacing: u32,
    ) -> Result<TerrainLabVanillaLodPayload, JsValue> {
        let tile = TerrainViewportTileId {
            profile: TerrainPreviewProfile::VanillaOverworld,
            seed: self.sampler.seed(),
            tile_x,
            tile_z,
            sample_spacing,
            content_stage: TerrainPreviewContentStage::Surface,
        };
        let generated_before = self.sampler.generated_density_columns();
        let reused_before = self.sampler.reused_density_columns();
        let reference = TerrainPreviewReferenceGrid::compile_with_vanilla_sampler(
            tile.preview_request(),
            &mut self.sampler,
        )
        .map_err(js_error)?;
        Ok(TerrainLabVanillaLodPayload {
            samples: reference.packed_f32(),
            generated_density_columns: self
                .sampler
                .generated_density_columns()
                .saturating_sub(generated_before),
            reused_density_columns: self
                .sampler
                .reused_density_columns()
                .saturating_sub(reused_before),
            retained_density_columns: self.sampler.retained_density_columns(),
        })
    }
}

#[wasm_bindgen(js_name = VanillaTerrainLodPayload)]
pub struct TerrainLabVanillaLodPayload {
    samples: Vec<f32>,
    generated_density_columns: u64,
    reused_density_columns: u64,
    retained_density_columns: usize,
}

#[wasm_bindgen(js_class = VanillaTerrainLodPayload)]
impl TerrainLabVanillaLodPayload {
    #[wasm_bindgen(getter)]
    pub fn samples(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(self.samples.as_slice())
    }

    #[wasm_bindgen(getter, js_name = generatedDensityColumns)]
    pub fn generated_density_columns(&self) -> f64 {
        self.generated_density_columns as f64
    }

    #[wasm_bindgen(getter, js_name = reusedDensityColumns)]
    pub fn reused_density_columns(&self) -> f64 {
        self.reused_density_columns as f64
    }

    #[wasm_bindgen(getter, js_name = retainedDensityColumns)]
    pub fn retained_density_columns(&self) -> u32 {
        self.retained_density_columns.min(u32::MAX as usize) as u32
    }
}

#[wasm_bindgen(js_name = VanillaTerrainMacroCompiler)]
pub struct TerrainLabVanillaMacroCompiler {
    sampler: VanillaOverworldMacroSampler,
}

#[wasm_bindgen(js_class = VanillaTerrainMacroCompiler)]
impl TerrainLabVanillaMacroCompiler {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: String) -> Result<TerrainLabVanillaMacroCompiler, JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        Ok(Self {
            sampler: VanillaOverworldMacroSampler::new(seed),
        })
    }

    pub fn compile(
        &mut self,
        tile_x: i32,
        tile_z: i32,
        sample_spacing: u32,
    ) -> Result<TerrainLabVanillaLodPayload, JsValue> {
        let tile = TerrainViewportTileId {
            profile: TerrainPreviewProfile::VanillaOverworld,
            seed: self.sampler.seed(),
            tile_x,
            tile_z,
            sample_spacing,
            content_stage: TerrainPreviewContentStage::Surface,
        };
        let generated_before = self.sampler.generated_density_columns();
        let reused_before = self.sampler.reused_density_columns();
        let reference = TerrainPreviewReferenceGrid::compile_with_vanilla_macro_sampler(
            tile.preview_request(),
            &mut self.sampler,
        )
        .map_err(js_error)?;
        Ok(TerrainLabVanillaLodPayload {
            samples: reference.packed_f32(),
            generated_density_columns: self
                .sampler
                .generated_density_columns()
                .saturating_sub(generated_before),
            reused_density_columns: self
                .sampler
                .reused_density_columns()
                .saturating_sub(reused_before),
            retained_density_columns: self.sampler.retained_density_columns(),
        })
    }
}

#[wasm_bindgen(js_name = CanonicalTerrainMeshSession)]
pub struct TerrainLabCanonicalMeshSession {
    session: CanonicalMeshSession,
}

#[wasm_bindgen(js_class = CanonicalTerrainMeshSession)]
impl TerrainLabCanonicalMeshSession {
    #[wasm_bindgen(js_name = withProfile)]
    pub fn with_profile(
        authored_bytes: js_sys::Uint8Array,
        reference_bytes: js_sys::Uint8Array,
        provisional_bytes: js_sys::Uint8Array,
        diagnostic_bytes: js_sys::Uint8Array,
        visual_profile: String,
        texture_presentation: String,
        seed: String,
        profile: String,
        stage: String,
    ) -> Result<TerrainLabCanonicalMeshSession, JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        let profile = TerrainPreviewProfile::parse_label(&profile).map_err(js_error)?;
        let stage = canonical_terrain_stage(&stage).map_err(js_error)?;
        let assets = load_terrain_lab_visual_assets(
            authored_bytes.to_vec(),
            reference_bytes.to_vec(),
            provisional_bytes.to_vec(),
            diagnostic_bytes.to_vec(),
            &visual_profile,
            &texture_presentation,
        )
        .map_err(js_error)?;
        Ok(Self {
            session: CanonicalMeshSession::new(profile, seed, stage, assets.terrain.catalog),
        })
    }

    pub fn begin(
        &mut self,
        coordinates_json: String,
        water_visible: bool,
        vegetation_visible: bool,
        cache_enabled: bool,
    ) -> Result<(), JsValue> {
        let coordinates =
            serde_json::from_str::<Vec<TerrainLabCanonicalChunkCoordinate>>(&coordinates_json)
                .map_err(|error| {
                    js_error(format!(
                        "invalid canonical Worker desired coordinates: {error}"
                    ))
                })?;
        self.session.begin(
            coordinates.into_iter().map(|coordinate| {
                CanonicalMeshCoordinate::new(coordinate.chunk_x, coordinate.chunk_z)
            }),
            CanonicalTerrainVisibility {
                water: water_visible,
                vegetation: vegetation_visible,
            },
            cache_enabled,
        );
        Ok(())
    }

    #[wasm_bindgen(js_name = compileBatch)]
    pub fn compile_batch(
        &mut self,
        coordinates_json: String,
    ) -> Result<TerrainLabCanonicalMeshBatchPayload, JsValue> {
        let coordinates =
            serde_json::from_str::<Vec<TerrainLabCanonicalChunkCoordinate>>(&coordinates_json)
                .map_err(|error| {
                    js_error(format!(
                        "invalid canonical Worker batch coordinates: {error}"
                    ))
                })?;
        let batch = self
            .session
            .compile_batch(
                &coordinates
                    .into_iter()
                    .map(|coordinate| {
                        CanonicalMeshCoordinate::new(coordinate.chunk_x, coordinate.chunk_z)
                    })
                    .collect::<Vec<_>>(),
            )
            .map_err(js_error)?;
        Ok(TerrainLabCanonicalMeshBatchPayload { batch })
    }

    #[wasm_bindgen(getter, js_name = rawCacheChunks)]
    pub fn raw_cache_chunks(&self) -> usize {
        self.session.raw_cache_chunks()
    }

    #[wasm_bindgen(getter, js_name = rawCacheBytes)]
    pub fn raw_cache_bytes(&self) -> u64 {
        self.session.raw_cache_bytes()
    }
}

#[wasm_bindgen(js_name = CanonicalTerrainMeshBatchPayload)]
pub struct TerrainLabCanonicalMeshBatchPayload {
    batch: crate::canonical_mesh::CanonicalMeshBatch,
}

#[wasm_bindgen(js_class = CanonicalTerrainMeshBatchPayload)]
impl TerrainLabCanonicalMeshBatchPayload {
    #[wasm_bindgen(getter, js_name = admissionCount)]
    pub fn admission_count(&self) -> u32 {
        self.batch.admissions.len().min(u32::MAX as usize) as u32
    }

    #[wasm_bindgen(getter, js_name = generationMs)]
    pub fn generation_ms(&self) -> f64 {
        self.batch.generation_ms
    }

    #[wasm_bindgen(getter, js_name = presentationMs)]
    pub fn presentation_ms(&self) -> f64 {
        self.batch.presentation_ms
    }

    #[wasm_bindgen(getter, js_name = meshMs)]
    pub fn mesh_ms(&self) -> f64 {
        self.batch.mesh_ms
    }

    #[wasm_bindgen(getter, js_name = packMs)]
    pub fn pack_ms(&self) -> f64 {
        self.batch.pack_ms
    }

    #[wasm_bindgen(getter, js_name = deduplicatedTargetChunks)]
    pub fn deduplicated_target_chunks(&self) -> u32 {
        self.batch.deduplicated_target_chunks.min(u32::MAX as usize) as u32
    }

    #[wasm_bindgen(getter, js_name = rawCacheChunks)]
    pub fn raw_cache_chunks(&self) -> u32 {
        self.batch.raw_cache_chunks.min(u32::MAX as usize) as u32
    }

    #[wasm_bindgen(getter, js_name = rawCacheBytes)]
    pub fn raw_cache_bytes(&self) -> f64 {
        self.batch.raw_cache_bytes as f64
    }

    #[wasm_bindgen(js_name = admissionChunkX)]
    pub fn admission_chunk_x(&self, index: u32) -> Result<i32, JsValue> {
        Ok(self.admission(index)?.requested.coordinate.chunk_x)
    }

    #[wasm_bindgen(js_name = admissionChunkZ)]
    pub fn admission_chunk_z(&self, index: u32) -> Result<i32, JsValue> {
        Ok(self.admission(index)?.requested.coordinate.chunk_z)
    }

    #[wasm_bindgen(js_name = admissionFingerprint)]
    pub fn admission_fingerprint(&self, index: u32) -> Result<String, JsValue> {
        Ok(format!(
            "{:016x}",
            self.admission(index)?.requested.fingerprint
        ))
    }

    #[wasm_bindgen(js_name = admissionRawCacheHit)]
    pub fn admission_raw_cache_hit(&self, index: u32) -> Result<bool, JsValue> {
        Ok(self.admission(index)?.requested.raw_cache_hit)
    }

    #[wasm_bindgen(js_name = admissionDependencyCacheHits)]
    pub fn admission_dependency_cache_hits(&self, index: u32) -> Result<u32, JsValue> {
        Ok(self
            .admission(index)?
            .requested
            .dependency_cache_hits
            .min(u32::MAX as usize) as u32)
    }

    #[wasm_bindgen(js_name = admissionGeneratedDependencyChunks)]
    pub fn admission_generated_dependency_chunks(&self, index: u32) -> Result<u32, JsValue> {
        Ok(self
            .admission(index)?
            .requested
            .generated_dependency_chunks
            .min(u32::MAX as usize) as u32)
    }

    #[wasm_bindgen(js_name = admissionRetainedDependencyChunks)]
    pub fn admission_retained_dependency_chunks(&self, index: u32) -> Result<u32, JsValue> {
        Ok(self
            .admission(index)?
            .requested
            .retained_dependency_chunks
            .min(u32::MAX as usize) as u32)
    }

    #[wasm_bindgen(js_name = admissionTargetChunks)]
    pub fn admission_target_chunks(&self, index: u32) -> Result<u32, JsValue> {
        Ok(self
            .admission(index)?
            .target_chunk_count
            .min(u32::MAX as usize) as u32)
    }

    #[wasm_bindgen(js_name = admissionSectionCount)]
    pub fn admission_section_count(&self, index: u32) -> Result<u32, JsValue> {
        Ok(self.admission(index)?.section_count.min(u32::MAX as usize) as u32)
    }

    #[wasm_bindgen(js_name = admissionVertexCount)]
    pub fn admission_vertex_count(&self, index: u32) -> Result<u32, JsValue> {
        Ok(self.admission(index)?.vertex_count.min(u32::MAX as usize) as u32)
    }

    #[wasm_bindgen(js_name = admissionIndexCount)]
    pub fn admission_index_count(&self, index: u32) -> Result<u32, JsValue> {
        Ok(self.admission(index)?.index_count.min(u32::MAX as usize) as u32)
    }

    #[wasm_bindgen(js_name = admissionPackedSections)]
    pub fn admission_packed_sections(&self, index: u32) -> Result<js_sys::Uint8Array, JsValue> {
        Ok(js_sys::Uint8Array::from(
            self.admission(index)?.packed_sections.as_slice(),
        ))
    }
}

impl TerrainLabCanonicalMeshBatchPayload {
    fn admission(
        &self,
        index: u32,
    ) -> Result<&crate::canonical_mesh::CanonicalPackedAdmission, JsValue> {
        self.batch.admissions.get(index as usize).ok_or_else(|| {
            js_error(format!(
                "canonical admission index {index} is out of bounds"
            ))
        })
    }
}

#[wasm_bindgen(js_name = CanonicalTerrainCompiler)]
pub struct TerrainLabCanonicalCompiler {
    compiler: CanonicalTerrainCompiler,
}

#[wasm_bindgen(js_class = CanonicalTerrainCompiler)]
impl TerrainLabCanonicalCompiler {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: String, stage: String) -> Result<TerrainLabCanonicalCompiler, JsValue> {
        Self::with_profile(
            seed,
            TerrainPreviewProfile::McloneOverworldV1.label().to_owned(),
            stage,
        )
    }

    #[wasm_bindgen(js_name = withProfile)]
    pub fn with_profile(
        seed: String,
        profile: String,
        stage: String,
    ) -> Result<TerrainLabCanonicalCompiler, JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        let profile = TerrainPreviewProfile::parse_label(&profile).map_err(js_error)?;
        let stage = canonical_terrain_stage(&stage).map_err(js_error)?;
        Ok(Self {
            compiler: CanonicalTerrainCompiler::new_with_profile(profile, seed, stage),
        })
    }

    #[wasm_bindgen(getter)]
    pub fn profile(&self) -> String {
        self.compiler.profile().label().to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn stage(&self) -> String {
        canonical_terrain_stage_label(self.compiler.stage()).to_owned()
    }

    #[wasm_bindgen(getter, js_name = retainedDependencyChunks)]
    pub fn retained_dependency_chunks(&self) -> u32 {
        self.compiler
            .retained_dependency_chunks()
            .min(u32::MAX as usize) as u32
    }

    #[wasm_bindgen(js_name = clearCache)]
    pub fn clear_cache(&mut self) {
        self.compiler.clear_cache();
    }

    pub fn compile(&mut self, chunk_x: i32, chunk_z: i32) -> TerrainLabCanonicalChunkPayload {
        TerrainLabCanonicalChunkPayload {
            chunk: self.compiler.compile(chunk_x, chunk_z),
        }
    }
}

#[wasm_bindgen(js_name = CanonicalTerrainChunkPayload)]
pub struct TerrainLabCanonicalChunkPayload {
    chunk: mclone_terrain_view::CanonicalTerrainChunk,
}

#[wasm_bindgen(js_class = CanonicalTerrainChunkPayload)]
impl TerrainLabCanonicalChunkPayload {
    #[wasm_bindgen(getter)]
    pub fn profile(&self) -> String {
        self.chunk.profile.label().to_owned()
    }

    #[wasm_bindgen(getter, js_name = chunkX)]
    pub fn chunk_x(&self) -> i32 {
        self.chunk.chunk_x
    }

    #[wasm_bindgen(getter, js_name = chunkZ)]
    pub fn chunk_z(&self) -> i32 {
        self.chunk.chunk_z
    }

    #[wasm_bindgen(getter, js_name = minY)]
    pub fn min_y(&self) -> i32 {
        self.chunk.min_y
    }

    #[wasm_bindgen(getter)]
    pub fn height(&self) -> i32 {
        self.chunk.height
    }

    #[wasm_bindgen(getter)]
    pub fn stage(&self) -> String {
        canonical_terrain_stage_label(self.chunk.stage).to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn fingerprint(&self) -> String {
        format!("{:016x}", self.chunk.fingerprint)
    }

    #[wasm_bindgen(getter)]
    pub fn blocks(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(self.chunk.blocks.as_slice())
    }

    #[wasm_bindgen(getter)]
    pub fn biomes(&self) -> js_sys::Int32Array {
        js_sys::Int32Array::from(self.chunk.biomes.as_slice())
    }

    #[wasm_bindgen(getter, js_name = dependencyCacheHits)]
    pub fn dependency_cache_hits(&self) -> u32 {
        self.chunk
            .dependency_cache
            .cache_hits
            .min(u32::MAX as usize) as u32
    }

    #[wasm_bindgen(getter, js_name = generatedDependencyChunks)]
    pub fn generated_dependency_chunks(&self) -> u32 {
        self.chunk
            .dependency_cache
            .generated_dependency_chunks
            .min(u32::MAX as usize) as u32
    }

    #[wasm_bindgen(getter, js_name = retainedDependencyChunks)]
    pub fn retained_dependency_chunks(&self) -> u32 {
        self.chunk
            .dependency_cache
            .retained_dependency_chunks
            .min(u32::MAX as usize) as u32
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainLabCanonicalChunkCoordinate {
    chunk_x: i32,
    chunk_z: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainLabExternalCpuTileRequest {
    revision: u64,
    profile: &'static str,
    seed: String,
    tile_x: i32,
    tile_z: i32,
    sample_spacing: u32,
}

#[wasm_bindgen(js_name = canonicalTerrainChunkOrder)]
pub fn canonical_terrain_chunk_order_json(
    center_x: i32,
    center_z: i32,
    radius: u32,
) -> Result<String, JsValue> {
    let coordinates = canonical_terrain_chunk_order(center_x, center_z, radius)
        .into_iter()
        .map(|position| TerrainLabCanonicalChunkCoordinate {
            chunk_x: position.x,
            chunk_z: position.z,
        })
        .collect::<Vec<_>>();
    json(&coordinates)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainLabAdapterReport<'a> {
    name: &'a str,
    backend: &'a str,
    device_type: &'a str,
    driver: &'a str,
    driver_info: &'a str,
    field_revision: &'static str,
    reference_schema_revision: &'static str,
    gpu_evaluator_revision: &'static str,
    macro_evaluator_revision: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainLabRenderReport<'a> {
    revision: u64,
    profile: &'static str,
    field_revision: &'static str,
    reference_schema_revision: &'static str,
    gpu_evaluator_revision: &'static str,
    macro_evaluator_revision: &'static str,
    vegetation_revision: &'static str,
    approximation: bool,
    seed: &'a str,
    center_x: i32,
    center_z: i32,
    requested_detail: &'a str,
    requested_spacing: u32,
    effective_spacing: u32,
    published_spacing: u32,
    cpu_published_spacing: u32,
    gpu_published_spacing: u32,
    sample_spacing: u32,
    cells_per_axis: u32,
    samples_per_axis: u32,
    sample_count: u32,
    vertex_count: u32,
    vegetation_summary_tile_count: u32,
    cpu_vegetation_summary_tile_count: u32,
    gpu_vegetation_summary_tile_count: u32,
    vegetation_record_tile_count: u32,
    vegetation_aggregated_tile_count: u32,
    tree_instance_count: u32,
    tree_instance_bytes: u64,
    tree_proxy_vertex_count: u32,
    vegetation_cell_requests: u64,
    vegetation_cell_hits: u64,
    vegetation_cell_misses: u64,
    retained_vegetation_cells: u32,
    footprint_blocks: u32,
    footprint_chunks: u32,
    view_width_blocks: u32,
    view_height_blocks: u32,
    level_count: u32,
    visible_tile_count: u32,
    published_tile_count: u32,
    cpu_published_tile_count: u32,
    gpu_published_tile_count: u32,
    resident_tile_count: u32,
    queued_tile_count: u32,
    cpu_queued_tile_count: u32,
    gpu_queued_tile_count: u32,
    pending_readback_count: u32,
    cpu_compiled_tiles: u32,
    cpu_compiled_tiles_total: u64,
    gpu_dispatched_tiles: u32,
    gpu_dispatched_tiles_total: u64,
    macro_compiled_tiles_total: u64,
    request_cpu_compiled_tiles: u32,
    request_gpu_dispatched_tiles: u32,
    request_macro_compiled_tiles: u32,
    request_cache_hit_tiles: u32,
    request_cpu_sample_lattice_points: u64,
    request_cpu_terrain_sample_evaluations: u64,
    request_cpu_forest_intent_evaluations: u64,
    request_cpu_forest_footprint_summaries: u64,
    request_gpu_sample_lattice_points: u64,
    request_gpu_terrain_sample_evaluations: u64,
    request_gpu_forest_intent_evaluations: u64,
    request_gpu_forest_footprint_summaries: u64,
    evicted_tiles_total: u64,
    source: &'static str,
    view: &'static str,
    layer: &'static str,
    content_stage: &'static str,
    structured_hydrology_available: bool,
    topology: &'static str,
    width: u32,
    height: u32,
    camera_yaw: f32,
    camera_pitch: f32,
    cpu_reference_ms: f64,
    cpu_vegetation_ms: f64,
    cpu_pack_upload_ms: f64,
    encode_submit_ms: f64,
    request_ms: f64,
    coarse_ready_ms: Option<f64>,
    target_ready_ms: Option<f64>,
    cpu_coarse_ready_ms: Option<f64>,
    cpu_target_ready_ms: Option<f64>,
    gpu_coarse_ready_ms: Option<f64>,
    gpu_target_ready_ms: Option<f64>,
    request_cpu_reference_ms: f64,
    request_cpu_vegetation_ms: f64,
    request_cpu_pack_upload_ms: f64,
    request_macro_compile_ms: f64,
    reference_bytes: u64,
    gpu_sample_bytes: u64,
    readback_bytes: u64,
    request_readback_bytes: u64,
    resident_bytes: u64,
    comparison_pending: bool,
    stale_result_count: u64,
    coarse_ready: bool,
    target_ready: bool,
    cpu_coarse_ready: bool,
    cpu_target_ready: bool,
    gpu_coarse_ready: bool,
    gpu_target_ready: bool,
    cache_enabled: bool,
    budget_limited: bool,
    needs_redraw: bool,
    gpu_execution_timing_available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainLabComparisonReport {
    revision: u64,
    sample_spacing: u32,
    tile_count: u32,
    sample_count: usize,
    max_absolute_surface_error: f32,
    mean_absolute_surface_error: f32,
    p95_absolute_surface_error: f32,
    max_absolute_display_error: f32,
    mean_absolute_display_error: f32,
    p95_absolute_display_error: f32,
    water_presence_agreement: f32,
    max_absolute_base_surface_error: f32,
    mean_absolute_base_surface_error: f32,
    p95_absolute_base_surface_error: f32,
    ocean_water_presence_agreement: f32,
    mean_absolute_continentalness_error: f32,
    mean_absolute_relief_error: f32,
    mean_absolute_temperature_error: f32,
    mean_absolute_moisture_error: f32,
    mean_absolute_ruggedness_error: f32,
    macro_surface_material_agreement: f32,
    channel_presence_agreement: f32,
    mean_absolute_river_signed_distance_error: f32,
    mean_absolute_channel_influence_error: f32,
    mean_absolute_bank_influence_error: f32,
    mean_absolute_wetland_influence_error: f32,
    visible_surface_material_agreement: f32,
    landform_kind_agreement: f32,
    biome_recipe_agreement: f32,
    surface_recipe_agreement: f32,
    stale_result_count: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainLabPointReceipt {
    field_revision: &'static str,
    preview_schema_revision: &'static str,
    gpu_evaluator_revision: &'static str,
    decoration_revision: &'static str,
    world_x: i32,
    world_z: i32,
    chunk_x: i32,
    chunk_z: i32,
    quart_x: i32,
    quart_z: i32,
    landform: &'static str,
    hydrology: &'static str,
    biome_recipe: &'static str,
    biome_reason: &'static str,
    surface_recipe: &'static str,
    planned_stream_start: Option<TerrainLabCanonicalChunkCoordinate>,
    base_surface_y: i32,
    surface_y: i32,
    carve_delta: i32,
    slope: f64,
    mountain_strength: f64,
    exposure: f64,
    continentalness: f64,
    relief: f64,
    ruggedness: f64,
    ridges: f64,
    mountain_detail: f64,
    temperature: f64,
    adjusted_temperature: f64,
    moisture: f64,
    river_signed_distance: f64,
    river_distance: f64,
    river_half_width: f64,
    channel_influence: f64,
    major_channel_influence: f64,
    bank_influence: f64,
    wetland_influence: f64,
    wetland_pool_influence: f64,
    submerged_outlet_influence: f64,
    planned_stream_influence: f64,
    water_surface_y: i32,
    bed_y: i32,
    flow_x: f64,
    flow_z: f64,
    grade: f64,
}

#[wasm_bindgen]
pub struct TerrainLab {
    canvas: HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    present_mode: wgpu::PresentMode,
    alpha_mode: wgpu::CompositeAlphaMode,
    width: u32,
    height: u32,
    adapter_name: String,
    adapter_backend: String,
    adapter_device_type: String,
    adapter_driver: String,
    adapter_driver_info: String,
    renderer: TerrainViewportRenderer,
    active_revision: u64,
    request_started_ms: f64,
    coarse_ready_ms: Option<f64>,
    target_ready_ms: Option<f64>,
    cpu_coarse_ready_ms: Option<f64>,
    cpu_target_ready_ms: Option<f64>,
    gpu_coarse_ready_ms: Option<f64>,
    gpu_target_ready_ms: Option<f64>,
}

#[wasm_bindgen]
impl TerrainLab {
    #[wasm_bindgen(js_name = adapterReport)]
    pub fn adapter_report(&self) -> Result<String, JsValue> {
        json(&TerrainLabAdapterReport {
            name: &self.adapter_name,
            backend: &self.adapter_backend,
            device_type: &self.adapter_device_type,
            driver: &self.adapter_driver,
            driver_info: &self.adapter_driver_info,
            field_revision: terrain_preview_field_revision(),
            reference_schema_revision: TERRAIN_PREVIEW_REFERENCE_SCHEMA_REVISION,
            gpu_evaluator_revision: TERRAIN_PREVIEW_GPU_EVALUATOR_REVISION,
            macro_evaluator_revision: VANILLA_OVERWORLD_MACRO_LOD_REVISION,
        })
    }

    #[wasm_bindgen]
    pub fn resize(&mut self, width: u32, height: u32) {
        self.resize_surface(width, height);
    }

    #[wasm_bindgen(js_name = setCacheEnabled)]
    pub fn set_cache_enabled(&mut self, enabled: bool) {
        self.renderer.set_cache_enabled(enabled);
    }

    #[wasm_bindgen(js_name = clearCache)]
    pub fn clear_cache(&mut self) {
        self.renderer.clear_cache();
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen]
    pub fn render(
        &mut self,
        revision: u32,
        profile: String,
        seed: String,
        center_x: i32,
        center_z: i32,
        blocks_across: u32,
        detail: String,
        panel_width_css: u32,
        panel_height_css: u32,
        max_visible_tiles_per_axis: u32,
        source: String,
        view: String,
        layer: String,
        content_stage: String,
        camera_yaw: f32,
        camera_pitch: f32,
        projection: String,
        split_layout: String,
    ) -> Result<String, JsValue> {
        let request_start = now_ms()?;
        let seed_value = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        let profile = TerrainPreviewProfile::parse_label(&profile).map_err(js_error)?;
        let mut options = terrain_preview_options(&source, &view, &layer).map_err(js_error)?;
        options.split_layout = terrain_preview_split_layout(&split_layout).map_err(js_error)?;
        let content_stage = terrain_preview_content_stage(&content_stage).map_err(js_error)?;
        if profile == TerrainPreviewProfile::VanillaOverworld {
            if options.source == TerrainPreviewSource::Gpu {
                return Err(js_error(
                    "vanilla overworld LOD supports sampled exact, fast macro, or compare",
                ));
            }
            if content_stage != TerrainPreviewContentStage::Surface {
                return Err(js_error(
                    "vanilla overworld LOD supports only the surface checkpoint",
                ));
            }
            if !matches!(
                options.layer,
                TerrainPreviewLayer::Terrain
                    | TerrainPreviewLayer::Height
                    | TerrainPreviewLayer::Error
                    | TerrainPreviewLayer::Biomes
                    | TerrainPreviewLayer::SurfaceRecipe
            ) {
                return Err(js_error(
                    "vanilla overworld LOD supports terrain, height, biome, and surface layers",
                ));
            }
        }
        let projection_kind = terrain_preview_projection_kind(&projection).map_err(js_error)?;
        let camera = TerrainPreviewCamera::new(camera_yaw, camera_pitch, projection_kind)
            .map_err(js_error)?;
        let viewport_detail = parse_viewport_detail(&detail).map_err(js_error)?;
        let plan = plan_terrain_viewport(TerrainViewportRequest {
            profile,
            seed: seed_value,
            center_x,
            center_z,
            blocks_across,
            panel_width_css,
            panel_height_css,
            detail: viewport_detail,
            max_visible_tiles_per_axis,
            content_stage,
        })
        .map_err(js_error)?;
        let revision = u64::from(revision);
        if self.active_revision != revision {
            self.active_revision = revision;
            self.request_started_ms = request_start;
            self.coarse_ready_ms = None;
            self.target_ready_ms = None;
            self.cpu_coarse_ready_ms = None;
            self.cpu_target_ready_ms = None;
            self.gpu_coarse_ready_ms = None;
            self.gpu_target_ready_ms = None;
        }
        self.renderer.set_viewport(revision, plan);

        let frame = self.acquire_frame()?;
        let color_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encode_start = now_ms()?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_terrain_lab_encoder"),
            });
        let stats = self
            .renderer
            .encode(
                &self.device,
                &self.queue,
                &mut encoder,
                &color_view,
                self.width,
                self.height,
                options,
                camera,
                performance_now,
            )
            .map_err(js_error)?;
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        let finished = now_ms()?;
        if stats.coarse_ready && self.coarse_ready_ms.is_none() {
            self.coarse_ready_ms = Some(finished - self.request_started_ms);
        }
        if stats.target_ready && self.target_ready_ms.is_none() {
            self.target_ready_ms = Some(finished - self.request_started_ms);
        }
        if stats.cpu_coarse_ready && self.cpu_coarse_ready_ms.is_none() {
            self.cpu_coarse_ready_ms = Some(finished - self.request_started_ms);
        }
        if stats.cpu_target_ready && self.cpu_target_ready_ms.is_none() {
            self.cpu_target_ready_ms = Some(finished - self.request_started_ms);
        }
        if stats.gpu_coarse_ready && self.gpu_coarse_ready_ms.is_none() {
            self.gpu_coarse_ready_ms = Some(finished - self.request_started_ms);
        }
        if stats.gpu_target_ready && self.gpu_target_ready_ms.is_none() {
            self.gpu_target_ready_ms = Some(finished - self.request_started_ms);
        }
        let (source, view, layer) = terrain_preview_option_labels(options);
        let report = render_report(
            stats,
            profile,
            &seed,
            center_x,
            center_z,
            &detail,
            source,
            view,
            layer,
            content_stage.label(),
            self.width,
            self.height,
            camera.yaw_radians,
            camera.pitch_radians,
            finished - encode_start,
            finished - self.request_started_ms,
            self.coarse_ready_ms,
            self.target_ready_ms,
            self.cpu_coarse_ready_ms,
            self.cpu_target_ready_ms,
            self.gpu_coarse_ready_ms,
            self.gpu_target_ready_ms,
        );
        json(&report)
    }

    #[wasm_bindgen(js_name = pollComparison)]
    pub fn poll_comparison(&mut self) -> Result<Option<String>, JsValue> {
        let mut latest = None;
        for result in self.renderer.poll_completed(&self.device) {
            let result = result.map_err(js_error)?;
            latest = Some(comparison_report(
                result,
                self.renderer.stale_result_count(),
            ));
        }
        latest.map(|report| json(&report)).transpose()
    }

    #[wasm_bindgen(js_name = nextCpuTileRequest)]
    pub fn next_cpu_tile_request(&mut self) -> Result<Option<String>, JsValue> {
        let Some(request) = self.renderer.take_external_cpu_request() else {
            return Ok(None);
        };
        json(&TerrainLabExternalCpuTileRequest {
            revision: request.revision,
            profile: request.tile.profile.label(),
            seed: request.tile.seed.to_string(),
            tile_x: request.tile.tile_x,
            tile_z: request.tile.tile_z,
            sample_spacing: request.tile.sample_spacing,
        })
        .map(Some)
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = acceptCpuTile)]
    pub fn accept_cpu_tile(
        &mut self,
        revision: u32,
        seed: String,
        tile_x: i32,
        tile_z: i32,
        sample_spacing: u32,
        samples: js_sys::Float32Array,
        compile_ms: f64,
    ) -> Result<bool, JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        let tile = TerrainViewportTileId {
            profile: TerrainPreviewProfile::VanillaOverworld,
            seed,
            tile_x,
            tile_z,
            sample_spacing,
            content_stage: TerrainPreviewContentStage::Surface,
        };
        let reference =
            TerrainPreviewReferenceGrid::from_packed_f32(tile.preview_request(), &samples.to_vec())
                .map_err(js_error)?;
        let compile_micros = (compile_ms.max(0.0) * 1_000.0).round().min(u64::MAX as f64) as u64;
        self.renderer
            .accept_external_cpu_tile(
                &self.device,
                &self.queue,
                TerrainViewportExternalCpuRequest {
                    revision: u64::from(revision),
                    tile,
                },
                reference,
                compile_micros,
            )
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = rejectCpuTile)]
    pub fn reject_cpu_tile(
        &mut self,
        revision: u32,
        seed: String,
        tile_x: i32,
        tile_z: i32,
        sample_spacing: u32,
    ) -> Result<(), JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        self.renderer
            .reject_external_cpu_tile(TerrainViewportExternalCpuRequest {
                revision: u64::from(revision),
                tile: TerrainViewportTileId {
                    profile: TerrainPreviewProfile::VanillaOverworld,
                    seed,
                    tile_x,
                    tile_z,
                    sample_spacing,
                    content_stage: TerrainPreviewContentStage::Surface,
                },
            });
        Ok(())
    }

    #[wasm_bindgen(js_name = nextMacroTileRequest)]
    pub fn next_macro_tile_request(&mut self) -> Result<Option<String>, JsValue> {
        let Some(request) = self.renderer.take_external_macro_request() else {
            return Ok(None);
        };
        json(&TerrainLabExternalCpuTileRequest {
            revision: request.revision,
            profile: request.tile.profile.label(),
            seed: request.tile.seed.to_string(),
            tile_x: request.tile.tile_x,
            tile_z: request.tile.tile_z,
            sample_spacing: request.tile.sample_spacing,
        })
        .map(Some)
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = acceptMacroTile)]
    pub fn accept_macro_tile(
        &mut self,
        revision: u32,
        seed: String,
        tile_x: i32,
        tile_z: i32,
        sample_spacing: u32,
        samples: js_sys::Float32Array,
        compile_ms: f64,
    ) -> Result<bool, JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        let tile = TerrainViewportTileId {
            profile: TerrainPreviewProfile::VanillaOverworld,
            seed,
            tile_x,
            tile_z,
            sample_spacing,
            content_stage: TerrainPreviewContentStage::Surface,
        };
        let macro_grid =
            TerrainPreviewReferenceGrid::from_packed_f32(tile.preview_request(), &samples.to_vec())
                .map_err(js_error)?;
        let compile_micros = (compile_ms.max(0.0) * 1_000.0).round().min(u64::MAX as f64) as u64;
        self.renderer
            .accept_external_macro_tile(
                &self.device,
                &self.queue,
                TerrainViewportExternalCpuRequest {
                    revision: u64::from(revision),
                    tile,
                },
                macro_grid,
                compile_micros,
            )
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = rejectMacroTile)]
    pub fn reject_macro_tile(
        &mut self,
        revision: u32,
        seed: String,
        tile_x: i32,
        tile_z: i32,
        sample_spacing: u32,
    ) -> Result<(), JsValue> {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        self.renderer
            .reject_external_macro_tile(TerrainViewportExternalCpuRequest {
                revision: u64::from(revision),
                tile: TerrainViewportTileId {
                    profile: TerrainPreviewProfile::VanillaOverworld,
                    seed,
                    tile_x,
                    tile_z,
                    sample_spacing,
                    content_stage: TerrainPreviewContentStage::Surface,
                },
            });
        Ok(())
    }
}

impl TerrainLab {
    async fn new(
        canvas: HtmlCanvasElement,
        authored_bytes: js_sys::Uint8Array,
        reference_bytes: js_sys::Uint8Array,
        provisional_bytes: js_sys::Uint8Array,
        diagnostic_bytes: js_sys::Uint8Array,
        visual_profile: String,
        texture_presentation: String,
    ) -> Result<Self, String> {
        let assets = load_terrain_lab_visual_assets(
            authored_bytes.to_vec(),
            reference_bytes.to_vec(),
            provisional_bytes.to_vec(),
            diagnostic_bytes.to_vec(),
            &visual_profile,
            &texture_presentation,
        )?
        .terrain;
        let mut material_uvs = [[0.0_f32, 0.0, 1.0, 1.0]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT];
        for (raw_id, target) in material_uvs.iter_mut().enumerate() {
            if let Some(sprite) = assets.catalog.gui_icon_uv(BlockStateId(raw_id as u32)) {
                *target = [sprite.u0, sprite.v0, sprite.u1, sprite.v1];
            }
        }
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|error| format!("failed to create Terrain Lab WebGPU surface: {error}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| format!("failed to request Terrain Lab WebGPU adapter: {error}"))?;
        let adapter_info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("mclone_terrain_lab_device"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .map_err(|error| format!("failed to request Terrain Lab WebGPU device: {error}"))?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or("Terrain Lab WebGPU surface reported no color formats")?;
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::Fifo)
            .or_else(|| capabilities.present_modes.first().copied())
            .unwrap_or(wgpu::PresentMode::Fifo);
        let alpha_mode = capabilities
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        surface.configure(
            &device,
            &surface_configuration(format, width, height, present_mode, alpha_mode),
        );

        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let renderer = TerrainViewportRenderer::new(
            &device,
            &queue,
            format,
            width,
            height,
            TerrainPreviewMaterialAtlas {
                width: assets.atlas.width,
                height: assets.atlas.height,
                rgba: assets.atlas.rgba(),
                material_uvs: &material_uvs,
            },
        )?;
        if let Some(error) = device.pop_error_scope().await {
            return Err(format!(
                "failed to initialize Terrain Lab GPU pipelines: {error}"
            ));
        }

        Ok(Self {
            canvas,
            surface,
            device,
            queue,
            format,
            present_mode,
            alpha_mode,
            width,
            height,
            adapter_name: adapter_info.name,
            adapter_backend: format!("{:?}", adapter_info.backend),
            adapter_device_type: format!("{:?}", adapter_info.device_type),
            adapter_driver: adapter_info.driver,
            adapter_driver_info: adapter_info.driver_info,
            renderer,
            active_revision: 0,
            request_started_ms: 0.0,
            coarse_ready_ms: None,
            target_ready_ms: None,
            cpu_coarse_ready_ms: None,
            cpu_target_ready_ms: None,
            gpu_coarse_ready_ms: None,
            gpu_target_ready_ms: None,
        })
    }

    fn resize_surface(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        self.surface.configure(
            &self.device,
            &surface_configuration(
                self.format,
                width,
                height,
                self.present_mode,
                self.alpha_mode,
            ),
        );
        self.renderer.resize(&self.device, width, height);
    }

    fn acquire_frame(&mut self) -> Result<wgpu::SurfaceTexture, JsValue> {
        match self.surface.get_current_texture() {
            Ok(frame) => Ok(frame),
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.resize_surface(self.width, self.height);
                self.surface.get_current_texture().map_err(|error| {
                    js_error(format!(
                        "failed to acquire Terrain Lab WebGPU frame after reconfigure: {error}"
                    ))
                })
            }
            Err(error) => Err(js_error(format!(
                "failed to acquire Terrain Lab WebGPU frame: {error}"
            ))),
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = terrainLabInspectPoint)]
pub fn terrain_lab_inspect_point(
    seed: String,
    center_x: i32,
    center_z: i32,
    blocks_across: u32,
    panel_width_css: u32,
    panel_height_css: u32,
    source: String,
    view: String,
    camera_yaw: f32,
    camera_pitch: f32,
    projection: String,
    split_layout: String,
    pointer_x_css: f32,
    pointer_y_css: f32,
) -> Result<String, JsValue> {
    let seed = seed
        .trim()
        .parse::<i64>()
        .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
    let mut options = terrain_preview_options(&source, &view, "terrain").map_err(js_error)?;
    options.split_layout = terrain_preview_split_layout(&split_layout).map_err(js_error)?;
    let projection_kind = terrain_preview_projection_kind(&projection).map_err(js_error)?;
    let camera =
        TerrainPreviewCamera::new(camera_yaw, camera_pitch, projection_kind).map_err(js_error)?;
    let (panel_width, panel_height, local_x, local_y) = local_inspection_panel(
        options.source,
        options.split_layout,
        panel_width_css.max(1),
        panel_height_css.max(1),
        pointer_x_css,
        pointer_y_css,
    );
    let (world_x, world_z) = inspection_world_coordinate(
        seed,
        center_x,
        center_z,
        blocks_across,
        panel_width,
        panel_height,
        options.view,
        camera,
        local_x,
        local_y,
    );
    let sample = mclone_overworld_debug_sample_with_streams(
        seed,
        McloneOverworldSamplingTopology::Unbounded,
        world_x,
        world_z,
    )
    .map_err(js_error)?;
    json(&point_receipt(sample))
}

fn local_inspection_panel(
    source: TerrainPreviewSource,
    split_layout: mclone_terrain_view::TerrainPreviewSplitLayout,
    width: u32,
    height: u32,
    pointer_x: f32,
    pointer_y: f32,
) -> (u32, u32, f32, f32) {
    if source != TerrainPreviewSource::Split {
        return (
            width,
            height,
            pointer_x.clamp(0.0, width as f32),
            pointer_y.clamp(0.0, height as f32),
        );
    }
    if split_layout == mclone_terrain_view::TerrainPreviewSplitLayout::Rows {
        let panel_height = (height / 2).max(1);
        (
            width,
            panel_height,
            pointer_x.clamp(0.0, width as f32),
            pointer_y.rem_euclid(panel_height as f32),
        )
    } else {
        let panel_width = (width / 2).max(1);
        (
            panel_width,
            height,
            pointer_x.rem_euclid(panel_width as f32),
            pointer_y.clamp(0.0, height as f32),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn inspection_world_coordinate(
    seed: i64,
    center_x: i32,
    center_z: i32,
    blocks_across: u32,
    panel_width: u32,
    panel_height: u32,
    view: TerrainPreviewView,
    camera: TerrainPreviewCamera,
    pointer_x: f32,
    pointer_y: f32,
) -> (i32, i32) {
    let focus_y = terrain_preview_focus_y(seed, center_x, center_z);
    let projection = terrain_preview_projection(
        blocks_across,
        view,
        camera,
        panel_width,
        panel_height,
        focus_y,
    );
    let normalized_x = pointer_x / panel_width.max(1) as f32;
    let normalized_y = pointer_y / panel_height.max(1) as f32;
    if view == TerrainPreviewView::Map {
        let width = blocks_across as f32;
        let height = width / projection.aspect.max(0.01);
        return (
            saturating_world_coordinate(center_x, (normalized_x * 2.0 - 1.0) * width * 0.5),
            saturating_world_coordinate(center_z, (normalized_y * 2.0 - 1.0) * height * 0.5),
        );
    }

    let eye = [
        projection.eye_offset[0],
        projection.target_y + projection.eye_offset[1],
        projection.eye_offset[2],
    ];
    let forward = normalize3([-eye[0], projection.target_y - eye[1], -eye[2]]);
    let right = normalize3(cross3(forward, projection.up));
    let camera_up = normalize3(cross3(right, forward));
    let ndc_x = normalized_x * 2.0 - 1.0;
    let ndc_y = 1.0 - normalized_y * 2.0;
    let (eye, direction) = match projection.kind {
        TerrainPreviewProjectionKind::Orthographic => (
            add3(
                eye,
                add3(
                    scale3(
                        right,
                        ndc_x * projection.vertical_half_extent * projection.aspect,
                    ),
                    scale3(camera_up, ndc_y * projection.vertical_half_extent),
                ),
            ),
            forward,
        ),
        TerrainPreviewProjectionKind::Perspective => {
            let half_height = (projection.fov_y_radians * 0.5).tan();
            (
                eye,
                normalize3(add3(
                    forward,
                    add3(
                        scale3(right, ndc_x * half_height * projection.aspect),
                        scale3(camera_up, ndc_y * half_height),
                    ),
                )),
            )
        }
    };
    let sampler =
        McloneOverworldSampler::new_with_topology(seed, McloneOverworldSamplingTopology::Unbounded);
    let ray_height = |distance: f32| {
        let relative_x = eye[0] + direction[0] * distance;
        let relative_y = eye[1] + direction[1] * distance;
        let relative_z = eye[2] + direction[2] * distance;
        let world_x = saturating_world_coordinate(center_x, relative_x);
        let world_z = saturating_world_coordinate(center_z, relative_z);
        let terrain = sampler.sample(world_x, world_z);
        let display_y = if terrain.watercourse.is_water()
            || terrain.surface_y < mclone_worldgen::levelgen::MCLONE_OVERWORLD_SEA_LEVEL
        {
            terrain.surface_y.max(terrain.watercourse.water_surface_y)
        } else {
            terrain.surface_y
        };
        (relative_y - (display_y as f32 + 1.0), world_x, world_z)
    };
    let mut previous_distance = projection.z_near;
    let mut previous = ray_height(previous_distance);
    for step in 1..=128 {
        let distance =
            projection.z_near + (projection.z_far - projection.z_near) * step as f32 / 128.0;
        let current = ray_height(distance);
        if previous.0 >= 0.0 && current.0 <= 0.0 {
            let mut low = previous_distance;
            let mut high = distance;
            for _ in 0..10 {
                let middle = (low + high) * 0.5;
                if ray_height(middle).0 >= 0.0 {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            let hit = ray_height((low + high) * 0.5);
            return (hit.1, hit.2);
        }
        previous_distance = distance;
        previous = current;
    }
    let plane_distance = ((projection.target_y - eye[1]) / direction[1]).max(0.0);
    (
        saturating_world_coordinate(center_x, eye[0] + direction[0] * plane_distance),
        saturating_world_coordinate(center_z, eye[2] + direction[2] * plane_distance),
    )
}

fn point_receipt(sample: McloneOverworldDebugSample) -> TerrainLabPointReceipt {
    let landform = sample.landform_sample;
    let terrain = landform.terrain;
    let watercourse = terrain.watercourse;
    TerrainLabPointReceipt {
        field_revision: terrain_preview_field_revision(),
        preview_schema_revision: TERRAIN_PREVIEW_REFERENCE_SCHEMA_REVISION,
        gpu_evaluator_revision: TERRAIN_PREVIEW_GPU_EVALUATOR_REVISION,
        decoration_revision: MCLONE_OVERWORLD_DECORATION_REVISION,
        world_x: sample.world_x,
        world_z: sample.world_z,
        chunk_x: sample.world_x.div_euclid(16),
        chunk_z: sample.world_z.div_euclid(16),
        quart_x: sample.quart_x,
        quart_z: sample.quart_z,
        landform: sample.landform.label(),
        hydrology: sample.hydrology.label(),
        biome_recipe: sample.biome.recipe.label(),
        biome_reason: sample.biome.reason.label(),
        surface_recipe: sample.surface.label(),
        planned_stream_start: sample.planned_stream_start.map(|start| {
            TerrainLabCanonicalChunkCoordinate {
                chunk_x: start.x,
                chunk_z: start.z,
            }
        }),
        base_surface_y: terrain.base_surface_y,
        surface_y: terrain.surface_y,
        carve_delta: terrain.base_surface_y - terrain.surface_y,
        slope: landform.slope,
        mountain_strength: terrain.mountain_strength(),
        exposure: landform.exposure(),
        continentalness: terrain.continentalness,
        relief: terrain.relief,
        ruggedness: terrain.ruggedness,
        ridges: terrain.ridges,
        mountain_detail: terrain.mountain_detail,
        temperature: terrain.climate.temperature,
        adjusted_temperature: terrain
            .climate
            .altitude_adjusted_temperature(terrain.surface_y),
        moisture: terrain.climate.moisture,
        river_signed_distance: watercourse.signed_distance,
        river_distance: watercourse.distance,
        river_half_width: watercourse.half_width,
        channel_influence: watercourse.channel_influence,
        major_channel_influence: watercourse.major_channel_influence,
        bank_influence: watercourse.bank_influence,
        wetland_influence: watercourse.wetland_influence,
        wetland_pool_influence: watercourse.wetland_pool_influence,
        submerged_outlet_influence: watercourse.submerged_outlet_influence,
        planned_stream_influence: watercourse.planned_stream_influence,
        water_surface_y: watercourse.water_surface_y,
        bed_y: watercourse.bed_y,
        flow_x: watercourse.flow_x,
        flow_z: watercourse.flow_z,
        grade: watercourse.grade,
    }
}

fn saturating_world_coordinate(center: i32, offset: f32) -> i32 {
    (center as f64 + f64::from(offset.round())).clamp(f64::from(i32::MIN), f64::from(i32::MAX))
        as i32
}

fn add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn scale3(vector: [f32; 3], factor: f32) -> [f32; 3] {
    [vector[0] * factor, vector[1] * factor, vector[2] * factor]
}

fn cross3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length = (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt();
    if length <= f32::EPSILON {
        return [0.0, 1.0, 0.0];
    }
    scale3(vector, length.recip())
}

#[wasm_bindgen]
pub fn mclone_terrain_lab_create(
    canvas: HtmlCanvasElement,
    authored_bytes: js_sys::Uint8Array,
    reference_bytes: js_sys::Uint8Array,
    provisional_bytes: js_sys::Uint8Array,
    diagnostic_bytes: js_sys::Uint8Array,
    visual_profile: String,
    texture_presentation: String,
) -> js_sys::Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        TerrainLab::new(
            canvas,
            authored_bytes,
            reference_bytes,
            provisional_bytes,
            diagnostic_bytes,
            visual_profile,
            texture_presentation,
        )
        .await
        .map(JsValue::from)
        .map_err(JsValue::from)
    })
}

#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&info.to_string()));
    }));
}

pub(crate) fn surface_configuration(
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    present_mode: wgpu::PresentMode,
    alpha_mode: wgpu::CompositeAlphaMode,
) -> wgpu::SurfaceConfiguration {
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: width.max(1),
        height: height.max(1),
        present_mode,
        alpha_mode,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_report<'a>(
    stats: TerrainViewportFrameStats,
    profile: TerrainPreviewProfile,
    seed: &'a str,
    center_x: i32,
    center_z: i32,
    requested_detail: &'a str,
    source: &'static str,
    view: &'static str,
    layer: &'static str,
    content_stage: &'static str,
    width: u32,
    height: u32,
    camera_yaw: f32,
    camera_pitch: f32,
    encode_submit_ms: f64,
    request_ms: f64,
    coarse_ready_ms: Option<f64>,
    target_ready_ms: Option<f64>,
    cpu_coarse_ready_ms: Option<f64>,
    cpu_target_ready_ms: Option<f64>,
    gpu_coarse_ready_ms: Option<f64>,
    gpu_target_ready_ms: Option<f64>,
) -> TerrainLabRenderReport<'a> {
    TerrainLabRenderReport {
        revision: stats.revision,
        profile: profile.label(),
        field_revision: profile.source_revision(),
        reference_schema_revision: TERRAIN_PREVIEW_REFERENCE_SCHEMA_REVISION,
        gpu_evaluator_revision: TERRAIN_PREVIEW_GPU_EVALUATOR_REVISION,
        macro_evaluator_revision: VANILLA_OVERWORLD_MACRO_LOD_REVISION,
        vegetation_revision: MCLONE_OVERWORLD_VEGETATION_REVISION,
        approximation: true,
        seed,
        center_x,
        center_z,
        requested_detail,
        requested_spacing: stats.requested_spacing,
        effective_spacing: stats.effective_spacing,
        published_spacing: stats.published_spacing,
        cpu_published_spacing: stats.cpu_published_spacing,
        gpu_published_spacing: stats.gpu_published_spacing,
        sample_spacing: stats.effective_spacing,
        cells_per_axis: TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS,
        samples_per_axis: TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS + 1,
        sample_count: stats.sample_count,
        vertex_count: stats.vertex_count,
        vegetation_summary_tile_count: stats.vegetation_summary_tile_count,
        cpu_vegetation_summary_tile_count: stats.cpu_vegetation_summary_tile_count,
        gpu_vegetation_summary_tile_count: stats.gpu_vegetation_summary_tile_count,
        vegetation_record_tile_count: stats.vegetation_record_tile_count,
        vegetation_aggregated_tile_count: stats.vegetation_aggregated_tile_count,
        tree_instance_count: stats.tree_instance_count,
        tree_instance_bytes: stats.tree_instance_bytes,
        tree_proxy_vertex_count: stats.tree_proxy_vertex_count,
        vegetation_cell_requests: stats.vegetation_cell_requests,
        vegetation_cell_hits: stats.vegetation_cell_hits,
        vegetation_cell_misses: stats.vegetation_cell_misses,
        retained_vegetation_cells: stats.retained_vegetation_cells,
        footprint_blocks: stats.view_width_blocks,
        footprint_chunks: stats.view_width_blocks / 16,
        view_width_blocks: stats.view_width_blocks,
        view_height_blocks: stats.view_height_blocks,
        level_count: stats.level_count,
        visible_tile_count: stats.visible_tile_count,
        published_tile_count: stats.published_tile_count,
        cpu_published_tile_count: stats.cpu_published_tile_count,
        gpu_published_tile_count: stats.gpu_published_tile_count,
        resident_tile_count: stats.resident_tile_count,
        queued_tile_count: stats.queued_tile_count,
        cpu_queued_tile_count: stats.cpu_queued_tile_count,
        gpu_queued_tile_count: stats.gpu_queued_tile_count,
        pending_readback_count: stats.pending_readback_count,
        cpu_compiled_tiles: stats.cpu_compiled_tiles,
        cpu_compiled_tiles_total: stats.cpu_compiled_tiles_total,
        gpu_dispatched_tiles: stats.gpu_dispatched_tiles,
        gpu_dispatched_tiles_total: stats.gpu_dispatched_tiles_total,
        macro_compiled_tiles_total: stats.macro_compiled_tiles_total,
        request_cpu_compiled_tiles: stats.request_cpu_compiled_tiles,
        request_gpu_dispatched_tiles: stats.request_gpu_dispatched_tiles,
        request_macro_compiled_tiles: stats.request_macro_compiled_tiles,
        request_cache_hit_tiles: stats.request_cache_hit_tiles,
        request_cpu_sample_lattice_points: stats.request_cpu_compile_work.sample_lattice_points,
        request_cpu_terrain_sample_evaluations: stats
            .request_cpu_compile_work
            .terrain_sample_evaluations,
        request_cpu_forest_intent_evaluations: stats
            .request_cpu_compile_work
            .forest_intent_evaluations,
        request_cpu_forest_footprint_summaries: stats
            .request_cpu_compile_work
            .forest_footprint_summaries,
        request_gpu_sample_lattice_points: stats.request_gpu_compile_work.sample_lattice_points,
        request_gpu_terrain_sample_evaluations: stats
            .request_gpu_compile_work
            .terrain_sample_evaluations,
        request_gpu_forest_intent_evaluations: stats
            .request_gpu_compile_work
            .forest_intent_evaluations,
        request_gpu_forest_footprint_summaries: stats
            .request_gpu_compile_work
            .forest_footprint_summaries,
        evicted_tiles_total: stats.evicted_tiles_total,
        source,
        view,
        layer,
        content_stage,
        structured_hydrology_available: profile == TerrainPreviewProfile::McloneOverworldV1
            && stats.effective_spacing <= 4
            && matches!(content_stage, "structured" | "surface" | "cover"),
        topology: McloneOverworldSamplingTopology::Unbounded.label(),
        width,
        height,
        camera_yaw,
        camera_pitch,
        cpu_reference_ms: stats.cpu_reference_micros as f64 / 1_000.0,
        cpu_vegetation_ms: stats.cpu_vegetation_micros as f64 / 1_000.0,
        cpu_pack_upload_ms: stats.cpu_pack_upload_micros as f64 / 1_000.0,
        encode_submit_ms,
        request_ms,
        coarse_ready_ms,
        target_ready_ms,
        cpu_coarse_ready_ms,
        cpu_target_ready_ms,
        gpu_coarse_ready_ms,
        gpu_target_ready_ms,
        request_cpu_reference_ms: stats.request_cpu_reference_micros as f64 / 1_000.0,
        request_cpu_vegetation_ms: stats.request_cpu_vegetation_micros as f64 / 1_000.0,
        request_cpu_pack_upload_ms: stats.request_cpu_pack_upload_micros as f64 / 1_000.0,
        request_macro_compile_ms: stats.request_macro_compile_micros as f64 / 1_000.0,
        reference_bytes: stats.reference_bytes,
        gpu_sample_bytes: stats.gpu_sample_bytes,
        readback_bytes: stats.readback_bytes,
        request_readback_bytes: stats.request_readback_bytes,
        resident_bytes: stats.resident_bytes,
        comparison_pending: stats.pending_readback_count > 0,
        stale_result_count: stats.stale_result_count,
        coarse_ready: stats.coarse_ready,
        target_ready: stats.target_ready,
        cpu_coarse_ready: stats.cpu_coarse_ready,
        cpu_target_ready: stats.cpu_target_ready,
        gpu_coarse_ready: stats.gpu_coarse_ready,
        gpu_target_ready: stats.gpu_target_ready,
        cache_enabled: stats.cache_enabled,
        budget_limited: stats.budget_limited,
        needs_redraw: stats.needs_redraw,
        gpu_execution_timing_available: false,
    }
}

fn comparison_report(
    completed: TerrainViewportCompletedComparison,
    stale_result_count: u64,
) -> TerrainLabComparisonReport {
    TerrainLabComparisonReport {
        revision: completed.revision,
        sample_spacing: completed.sample_spacing,
        tile_count: completed.tile_count,
        sample_count: completed.comparison.sample_count,
        max_absolute_surface_error: completed.comparison.max_absolute_surface_error,
        mean_absolute_surface_error: completed.comparison.mean_absolute_surface_error,
        p95_absolute_surface_error: completed.comparison.p95_absolute_surface_error,
        max_absolute_display_error: completed.comparison.max_absolute_display_error,
        mean_absolute_display_error: completed.comparison.mean_absolute_display_error,
        p95_absolute_display_error: completed.comparison.p95_absolute_display_error,
        water_presence_agreement: completed.comparison.water_presence_agreement,
        max_absolute_base_surface_error: completed.comparison.max_absolute_base_surface_error,
        mean_absolute_base_surface_error: completed.comparison.mean_absolute_base_surface_error,
        p95_absolute_base_surface_error: completed.comparison.p95_absolute_base_surface_error,
        ocean_water_presence_agreement: completed.comparison.ocean_water_presence_agreement,
        mean_absolute_continentalness_error: completed
            .comparison
            .mean_absolute_continentalness_error,
        mean_absolute_relief_error: completed.comparison.mean_absolute_relief_error,
        mean_absolute_temperature_error: completed.comparison.mean_absolute_temperature_error,
        mean_absolute_moisture_error: completed.comparison.mean_absolute_moisture_error,
        mean_absolute_ruggedness_error: completed.comparison.mean_absolute_ruggedness_error,
        macro_surface_material_agreement: completed.comparison.macro_surface_material_agreement,
        channel_presence_agreement: completed.comparison.channel_presence_agreement,
        mean_absolute_river_signed_distance_error: completed
            .comparison
            .mean_absolute_river_signed_distance_error,
        mean_absolute_channel_influence_error: completed
            .comparison
            .mean_absolute_channel_influence_error,
        mean_absolute_bank_influence_error: completed.comparison.mean_absolute_bank_influence_error,
        mean_absolute_wetland_influence_error: completed
            .comparison
            .mean_absolute_wetland_influence_error,
        visible_surface_material_agreement: completed.comparison.visible_surface_material_agreement,
        landform_kind_agreement: completed.comparison.landform_kind_agreement,
        biome_recipe_agreement: completed.comparison.biome_recipe_agreement,
        surface_recipe_agreement: completed.comparison.surface_recipe_agreement,
        stale_result_count,
    }
}

fn parse_viewport_detail(detail: &str) -> Result<TerrainViewportDetail, String> {
    if detail == "auto" {
        return Ok(TerrainViewportDetail::Auto);
    }
    detail
        .parse::<u32>()
        .map(TerrainViewportDetail::Manual)
        .map_err(|error| format!("invalid terrain viewport detail {detail:?}: {error}"))
}

fn now_ms() -> Result<f64, JsValue> {
    web_sys::window()
        .ok_or_else(|| js_error("Terrain Lab has no Window"))?
        .performance()
        .ok_or_else(|| js_error("Terrain Lab has no Performance clock"))
        .map(|performance| performance.now())
}

fn performance_now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .unwrap_or(0.0)
}

fn json(value: &impl Serialize) -> Result<String, JsValue> {
    serde_json::to_string(value)
        .map_err(|error| js_error(format!("failed to serialize Terrain Lab report: {error}")))
}

fn js_error(message: impl AsRef<str>) -> JsValue {
    JsValue::from_str(message.as_ref())
}
