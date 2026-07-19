use wasm_bindgen::{JsCast, prelude::*};
use web_sys::HtmlCanvasElement;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

use crate::web_render_compiler_abi::*;
use crate::web_render_worker::{WebRenderWorkerCoordinator, WebRenderWorkerWorldHandle};
use crate::web_world_catalog_descriptor;

use super::{
    SMOKE_INITIAL_CENTER, SMOKE_MOVED_CENTER, SMOKE_RADIUS_CHUNKS, SMOKE_SEED, WebRuntime,
};
use mclone_app_runtime::deferred_drop::{
    BoundedDeferredDropQueue, DEFAULT_DEFERRED_DROP_MAX_ITEMS, DeferredDropService,
};
use mclone_app_runtime::far_lod::{
    FarTerrainLodBuildRequest, FarTerrainLodBuildResult, FarTerrainLodCache, FarTerrainLodCompiler,
    FarTerrainLodConfig, FarTerrainLodMaterialPalette, FarTerrainLodWorkerInput,
    compile_far_terrain_lod_worker_input,
};
use mclone_app_runtime::host_mode::SingleViewHostMode;
use mclone_app_runtime::lod_coverage::{LodCoverageCoordinator, LodTileAvailability};
use mclone_app_runtime::monotonic::{MonotonicClockHandle, MonotonicDeadline};
use mclone_app_runtime::prepared_assets::{
    AUTHORED_FIRST_PARTY_PACK_ID, AssetPackSourceRegistry, MINECRAFT_REFERENCE_PACK_ID,
    PreparedSceneAssets,
};
use mclone_app_runtime::render_asset_data::TexturedMeshAssets;
use mclone_app_runtime::render_compile_capacity::{
    RenderCompileCapacityHostKind, host_total_memory_bytes,
    preflight_render_compile_capacity_report,
};
use mclone_app_runtime::scenario::BuiltInScenarioId;
use mclone_app_runtime::scenario_content::{
    ManagedScenarioManifest, ManagedScenarioStoredRecord, ManagedScenarioStoredWorldMetadata,
    ManagedScenarioWorldRole, ManagedWorldKey, managed_scenario_world_payload,
    validate_managed_scenario_stored_world,
};
use mclone_app_runtime::scene_session_runtime::{
    FarLodRuntimeSettleSnapshot, RuntimeRenderPriority, SceneRuntimeService, SceneSessionRuntime,
    StartupReadinessPolicy,
};
use mclone_app_runtime::session::ActiveSessionDescriptor;
use mclone_app_runtime::startup_args::{
    RenderCompileCapacityRequest, RenderDistanceLimits, STARTUP_QUERY_KEYS, StartupArgState,
    StartupOptions, StartupSceneOptions,
};
use mclone_app_runtime::world_catalog::{
    LOCAL_WORLD_CATALOG_SCHEMA_VERSION, LOCAL_WORLD_TARGET_MINECRAFT_VERSION,
    LocalWorldCreateOptions, LocalWorldId, LocalWorldSummary, WorldCatalogCapabilities,
    WorldCatalogError, WorldCatalogErrorKind, WorldCatalogResponse, duplicate_world_id,
    sort_local_world_summaries, validate_delete_inactive_world, validate_local_world_compatible,
    world_not_found,
};
use mclone_app_runtime::{
    GameplayCommandSubmission, GameplayCommandTiming, GameplayCommandUpdatePolicy,
    RuntimePollTiming, RuntimeUpdatePumpBudget, TimedRenderSectionCacheUpdate,
    loading_progress_overlay_from_diagnostics, view_readiness_overlay_from_diagnostics,
};
use mclone_assets::{AssetPackId, AssetPackSelection, PackedAssetSource, SharedAssetSource};
use mclone_audio::PreparedAudioAssets;
use mclone_client::ClientRuntime;
#[cfg(test)]
use mclone_core::{
    AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkStatus, chunk_section_index,
};
use mclone_core::{
    AxisTopology, ChunkPos, ChunkRevision, ChunkSnapshot, HorizontalTopology, LodTileKey,
};
use mclone_mesh::{
    RenderSectionKey, TexturedMeshCatalog, TexturedRenderSectionBuildReport,
    TexturedRenderSectionMesh, TexturedRenderSectionMetadata, load_textured_terrain_assets,
};
use mclone_protocol::{ClientCommand, ServerUpdate, decode_server_update, encode_server_update};
use mclone_render::actor_assets::load_actor_texture_assets;
use mclone_render::chunk::TexturedSectionRenderOptions;
use mclone_render::color_profile::{RenderColorProfile, RenderConfig};
use mclone_render::far_lod::{FarTerrainLodFrameUpdate, FarTerrainLodTileMesh};
use mclone_render::screen_effect::load_screen_effect_texture_assets;
use mclone_render_session::{
    RenderSectionCacheUpdate, RenderSectionCompileQueueHealth, RenderSectionCompileRequest,
    RenderSectionCompileResult, RenderSectionCompiler, build_client_textured_sections,
    build_render_sections_from_snapshots_with_biome_zoom_seed,
    build_render_sections_from_snapshots_with_biome_zoom_seed_and_topology,
    decode_textured_render_section_build_report, encode_textured_render_section_build_report,
    summarize_textured_render_section_build_report,
};
use mclone_server::{ServerRunnerKind, SimulationCadenceConfig, WorldGenerationProfile};
use mclone_ui::{GameUiAction, GuiKey, LoadingProgressOverlay};

const CANVAS_OK_BIT: u32 = 1 << 0;
const CANVAS_RENDERED_BIT: u32 = 1 << 1;
const CANVAS_CONFIGURED_BIT: u32 = 1 << 2;
const CANVAS_WIDTH_SHIFT: u32 = 8;
const CANVAS_HEIGHT_SHIFT: u32 = 20;
const WEB_MIN_RENDER_DISTANCE: i32 = 1;
const WEB_MAX_RENDER_DISTANCE: i32 = 16;
const WEB_DEFAULT_RENDER_DISTANCE: u32 = 3;
const WEB_QUERY_WORLD_STORAGE: &str = "worldStorage";
const WEB_QUERY_WORLD_ID: &str = "worldId";
const WEB_QUERY_CLEAR_WORLD_STORAGE: &str = "clearWorldStorage";
const WEB_WORLD_BACKEND_LABEL: &str = "web-indexeddb";
// 067 Stage 3: web drives the shared streaming loop at the same per-frame increment as
// desktop (`DEFAULT_RENDER_CHUNK_MESH_BUDGET`). One small job in flight per frame; the
// resident cache stays visible while movement fills progressively.
const WEB_RENDER_CHUNK_MESH_BUDGET: usize = 1;
const WEB_DEFERRED_DROP_DRAIN_ITEM_BUDGET: usize = 256;
// Y the overview-frame camera looks down from when streaming a deterministic chunk view;
// only the horizontal component matters for near-camera readiness.
// 067 Stage 4: the render-compile input arena now carries a *delta* against the
// worker's resident snapshot mirror, not the whole loaded world. Distinct magic from the
// retired whole-world format so a stale producer/consumer is rejected loudly. Layout:
// magic, u32 generation, u32 flags (bit 0 = reset/full-resync), two fixed-width encoded
// topology axes, u32 upsert_count followed
// by that many length-prefixed `ServerUpdate::ChunkSnapshot` frames, then u32 evict_count
// followed by that many `[i32 x][i32 z]` chunk positions to drop from the mirror.
const WEB_RENDER_COMPILE_DELTA_MAGIC: &[u8; 8] = b"MCWRCD2\0";
const WEB_RENDER_COMPILE_DELTA_FLAG_RESET: u32 = 1;
const WEB_RENDER_COMPILE_DELTA_FLAG_BIOME_ZOOM_SEED: u32 = 2;
const WEB_FAR_LOD_TILE_MESH_MAGIC: &[u8; 8] = b"MCWLOD1\0";

#[wasm_bindgen]
pub fn mclone_web_canvas_clear_bits(width: u32, height: u32) -> u32 {
    let width = width.max(1);
    let height = height.max(1);
    CANVAS_OK_BIT
        | CANVAS_RENDERED_BIT
        | CANVAS_CONFIGURED_BIT
        | (packed_dimension(width) << CANVAS_WIDTH_SHIFT)
        | (packed_dimension(height) << CANVAS_HEIGHT_SHIFT)
}

#[wasm_bindgen]
pub fn mclone_web_render_canvas(canvas: HtmlCanvasElement) -> js_sys::Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        render_canvas_clear(canvas).await.map_err(JsValue::from)?;
        Ok(JsValue::from_bool(true))
    })
}

#[wasm_bindgen]
pub fn mclone_web_render_canvas_report(canvas: HtmlCanvasElement) -> js_sys::Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        let report = render_canvas_clear(canvas).await.map_err(JsValue::from)?;
        Ok(JsValue::from_f64(f64::from(report.packed_bits())))
    })
}

#[wasm_bindgen]
pub fn mclone_web_startup_options_from_query(search: String) -> Result<WebStartupConfig, JsValue> {
    let options = parse_startup_options_from_query(&search)?;
    let storage = parse_web_world_storage_from_query(&search, options.scene.seed)?;
    Ok(WebStartupConfig { options, storage })
}

#[wasm_bindgen]
pub struct WebStartupConfig {
    options: StartupOptions,
    storage: WebWorldStorageStartupOptions,
}

#[wasm_bindgen]
impl WebStartupConfig {
    #[wasm_bindgen(js_name = browserPlan)]
    pub fn browser_plan(&self) -> Result<JsValue, JsValue> {
        let object = js_sys::Object::new();
        set_number(
            &object,
            "renderDistance",
            f64::from(self.options.scene.render_distance),
        )
        .map_err(JsValue::from)?;
        set_bool(
            &object,
            "sectionOcclusionCulling",
            self.options.render_options.section_occlusion_culling,
        )
        .map_err(JsValue::from)?;
        set_bool(
            &object,
            "forceFullbright",
            self.options.render_options.force_fullbright,
        )
        .map_err(JsValue::from)?;
        set_string(
            &object,
            "renderColorProfile",
            self.options.render_options.color_profile.as_str(),
        )
        .map_err(JsValue::from)?;
        set_string(
            &object,
            "generationProfile",
            self.options.scene.world_generation_profile.label(),
        )
        .map_err(JsValue::from)?;
        if let Some(remote_addr) = &self.options.scene.remote_addr {
            set_string(&object, "remoteWebSocketUrl", remote_addr).map_err(JsValue::from)?;
        }
        Ok(object.into())
    }
}

impl WebStartupConfig {
    pub(super) fn into_parts(self) -> (StartupOptions, WebWorldStorageStartupOptions) {
        (self.options, self.storage)
    }
}

#[wasm_bindgen]
pub fn mclone_web_remote_websocket_smoke_report(websocket_url: String) -> js_sys::Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        let report = remote_websocket_smoke_report(websocket_url)
            .await
            .map_err(JsValue::from)?;
        Ok(report)
    })
}

#[wasm_bindgen]
pub fn mclone_web_compile_generated_chunk_sections(
    asset_pack_bytes: js_sys::Uint8Array,
    center_x: i32,
    center_z: i32,
    radius_chunks: u32,
) -> Result<js_sys::Uint8Array, JsValue> {
    let report = compile_generated_chunk_sections_from_pack(
        asset_pack_bytes.to_vec(),
        ChunkPos {
            x: center_x,
            z: center_z,
        },
        radius_chunks,
        None,
    )
    .map_err(JsValue::from)?;
    let packed = encode_textured_render_section_build_report(&report);
    Ok(js_sys::Uint8Array::from(packed.as_slice()))
}

#[wasm_bindgen]
pub fn mclone_web_compile_generated_chunk_sections_for_targets(
    asset_pack_bytes: js_sys::Uint8Array,
    center_x: i32,
    center_z: i32,
    radius_chunks: u32,
    target_sections: js_sys::Int32Array,
) -> Result<js_sys::Uint8Array, JsValue> {
    let target_sections = render_section_keys_from_int32_array(&target_sections)
        .map_err(|error| JsValue::from_str(&error))?;
    let report = compile_generated_chunk_sections_from_pack(
        asset_pack_bytes.to_vec(),
        ChunkPos {
            x: center_x,
            z: center_z,
        },
        radius_chunks,
        Some(&target_sections),
    )
    .map_err(JsValue::from)?;
    let packed = encode_textured_render_section_build_report(&report);
    Ok(js_sys::Uint8Array::from(packed.as_slice()))
}

#[wasm_bindgen]
pub struct WebRenderCompilerSession {
    mesh_assets: Rc<WebTexturedMeshAssets>,
    asset_pack_byte_length: usize,
    asset_load_count: usize,
    compile_count: usize,
    // 067 Stage 4: resident snapshot mirror. Created once with the session and persisted
    // across compiles, so the per-frame input is a delta (upserts + evictions) instead of
    // the whole loaded world. `compileSnapshotSectionsForTargets` applies each delta here
    // before compiling targets from the mirror's resident columns.
    snapshot_mirror: BTreeMap<ChunkPos, ChunkSnapshot>,
    // The mirror epoch this session currently holds. A non-reset delta whose generation does
    // not match is a desync (e.g. a worker that silently lost its mirror) and is rejected.
    mirror_generation: u32,
    biome_zoom_seed: Option<i64>,
    topology: HorizontalTopology,
    last_delta_upsert_count: usize,
    last_delta_eviction_count: usize,
}

#[wasm_bindgen]
impl WebRenderCompilerSession {
    #[wasm_bindgen(constructor)]
    pub fn new(asset_pack_bytes: js_sys::Uint8Array) -> Result<WebRenderCompilerSession, JsValue> {
        let asset_pack_byte_length = asset_pack_bytes.length() as usize;
        let mesh_assets = load_textured_mesh_assets_from_pack(asset_pack_bytes.to_vec())
            .map_err(JsValue::from)?;
        Ok(Self {
            mesh_assets: Rc::new(mesh_assets),
            asset_pack_byte_length,
            asset_load_count: 1,
            compile_count: 0,
            snapshot_mirror: BTreeMap::new(),
            mirror_generation: 0,
            biome_zoom_seed: None,
            topology: HorizontalTopology::UNBOUNDED,
            last_delta_upsert_count: 0,
            last_delta_eviction_count: 0,
        })
    }

    #[wasm_bindgen(js_name = newSelected)]
    pub fn new_selected(
        authored_pack_bytes: js_sys::Uint8Array,
        reference_pack_bytes: js_sys::Uint8Array,
        fallback_pack_bytes: js_sys::Uint8Array,
        authored_enabled: bool,
        reference_enabled: bool,
    ) -> Result<WebRenderCompilerSession, JsValue> {
        let asset_pack_byte_length = authored_pack_bytes.length() as usize
            + reference_pack_bytes.length() as usize
            + fallback_pack_bytes.length() as usize;
        let (mesh_assets, asset_pack_file_count) = load_selected_web_mesh_assets(
            authored_pack_bytes.to_vec(),
            reference_pack_bytes.to_vec(),
            fallback_pack_bytes.to_vec(),
            authored_enabled,
            reference_enabled,
        )
        .map_err(JsValue::from)?;
        Ok(Self {
            mesh_assets: Rc::new(WebTexturedMeshAssets {
                catalog: mesh_assets.catalog,
                far_lod_materials: mesh_assets.far_lod_materials,
                asset_pack_file_count,
            }),
            asset_pack_byte_length,
            asset_load_count: 3,
            compile_count: 0,
            snapshot_mirror: BTreeMap::new(),
            mirror_generation: 0,
            biome_zoom_seed: None,
            topology: HorizontalTopology::UNBOUNDED,
            last_delta_upsert_count: 0,
            last_delta_eviction_count: 0,
        })
    }

    #[wasm_bindgen(js_name = assetPackByteLength)]
    pub fn asset_pack_byte_length(&self) -> usize {
        self.asset_pack_byte_length
    }

    #[wasm_bindgen(js_name = assetPackFileCount)]
    pub fn asset_pack_file_count(&self) -> usize {
        self.mesh_assets.asset_pack_file_count
    }

    #[wasm_bindgen(js_name = assetLoadCount)]
    pub fn asset_load_count(&self) -> usize {
        self.asset_load_count
    }

    #[wasm_bindgen(js_name = compileCount)]
    pub fn compile_count(&self) -> usize {
        self.compile_count
    }

    /// Create an independent per-world snapshot mirror while retaining the
    /// one immutable parsed asset catalog owned by the Worker.
    #[wasm_bindgen(js_name = forkWorldSession)]
    pub fn fork_world_session(&self) -> WebRenderCompilerSession {
        Self {
            mesh_assets: Rc::clone(&self.mesh_assets),
            asset_pack_byte_length: self.asset_pack_byte_length,
            asset_load_count: 0,
            compile_count: 0,
            snapshot_mirror: BTreeMap::new(),
            mirror_generation: 0,
            biome_zoom_seed: None,
            topology: HorizontalTopology::UNBOUNDED,
            last_delta_upsert_count: 0,
            last_delta_eviction_count: 0,
        }
    }

    #[wasm_bindgen(js_name = compileFarLodTile)]
    pub fn compile_far_lod_tile(
        &mut self,
        seed_text: String,
        chunk_x: i32,
        chunk_z: i32,
        level: u8,
        sample_spacing_blocks: u32,
        west_sample_spacing_blocks: u32,
        east_sample_spacing_blocks: u32,
        north_sample_spacing_blocks: u32,
        south_sample_spacing_blocks: u32,
    ) -> Result<js_sys::Uint8Array, JsValue> {
        let packed = self.compile_far_lod_tile_bytes(
            seed_text,
            chunk_x,
            chunk_z,
            level,
            sample_spacing_blocks,
            west_sample_spacing_blocks,
            east_sample_spacing_blocks,
            north_sample_spacing_blocks,
            south_sample_spacing_blocks,
        )?;
        Ok(js_sys::Uint8Array::from(packed.as_slice()))
    }

    #[wasm_bindgen(js_name = compileGeneratedChunkSections)]
    pub fn compile_generated_chunk_sections(
        &mut self,
        center_x: i32,
        center_z: i32,
        radius_chunks: u32,
    ) -> Result<js_sys::Uint8Array, JsValue> {
        let packed =
            self.compile_generated_chunk_sections_bytes(center_x, center_z, radius_chunks, None)?;
        Ok(js_sys::Uint8Array::from(packed.as_slice()))
    }

    #[wasm_bindgen(js_name = compileGeneratedChunkSectionsForTargets)]
    pub fn compile_generated_chunk_sections_for_targets(
        &mut self,
        center_x: i32,
        center_z: i32,
        radius_chunks: u32,
        target_sections: js_sys::Int32Array,
    ) -> Result<js_sys::Uint8Array, JsValue> {
        let target_sections = render_section_keys_from_int32_array(&target_sections)
            .map_err(|error| JsValue::from_str(&error))?;
        let packed = self.compile_generated_chunk_sections_bytes(
            center_x,
            center_z,
            radius_chunks,
            Some(&target_sections),
        )?;
        Ok(js_sys::Uint8Array::from(packed.as_slice()))
    }

    /// 067 Stage 4: apply a per-frame input delta to the resident snapshot mirror, then
    /// compile the target sections from the mirror's resident columns. The input bytes are a
    /// `WebRenderCompileDelta` (upserts + evictions against the mirror), not the whole loaded
    /// world. The mirror is applied *before* the compile (and before any result-buffer
    /// overflow can be detected downstream), so the advance survives a requeue.
    #[wasm_bindgen(js_name = compileSnapshotSectionsForTargets)]
    pub fn compile_snapshot_sections_for_targets(
        &mut self,
        snapshot_input_bytes: js_sys::Uint8Array,
        target_sections: js_sys::Int32Array,
    ) -> Result<js_sys::Uint8Array, JsValue> {
        let packed = self
            .compile_snapshot_sections_for_targets_bytes(&snapshot_input_bytes, &target_sections)?;
        Ok(js_sys::Uint8Array::from(packed.as_slice()))
    }

    /// Resident snapshot mirror size after the last applied delta — the worker's view of the
    /// loaded world. Stays bounded to the loaded set because each delta evicts the columns the
    /// client unloaded, so this validates the mirror does not grow unbounded over a session.
    #[wasm_bindgen(js_name = mirrorChunkCount)]
    pub fn mirror_chunk_count(&self) -> usize {
        self.snapshot_mirror.len()
    }

    #[wasm_bindgen(js_name = lastDeltaUpsertCount)]
    pub fn last_delta_upsert_count(&self) -> usize {
        self.last_delta_upsert_count
    }

    #[wasm_bindgen(js_name = lastDeltaEvictionCount)]
    pub fn last_delta_eviction_count(&self) -> usize {
        self.last_delta_eviction_count
    }
}

impl WebRenderCompilerSession {
    pub(crate) fn compile_far_lod_tile_bytes(
        &mut self,
        seed_text: String,
        chunk_x: i32,
        chunk_z: i32,
        level: u8,
        sample_spacing_blocks: u32,
        west_sample_spacing_blocks: u32,
        east_sample_spacing_blocks: u32,
        north_sample_spacing_blocks: u32,
        south_sample_spacing_blocks: u32,
    ) -> Result<Vec<u8>, JsValue> {
        self.compile_count += 1;
        let seed = seed_text
            .parse::<i64>()
            .map_err(|error| JsValue::from_str(&format!("invalid far LOD seed: {error}")))?;
        let mesh = compile_far_terrain_lod_worker_input(
            FarTerrainLodWorkerInput {
                key: LodTileKey::new(ChunkPos::new(chunk_x, chunk_z), level),
                seed,
                sample_spacing_blocks,
                neighbor_sample_spacings: [
                    west_sample_spacing_blocks,
                    east_sample_spacing_blocks,
                    north_sample_spacing_blocks,
                    south_sample_spacing_blocks,
                ],
            },
            self.mesh_assets.far_lod_materials.as_ref(),
        );
        encode_web_far_lod_tile_mesh(&mesh).map_err(JsValue::from)
    }

    pub(crate) fn compile_generated_chunk_sections_bytes(
        &mut self,
        center_x: i32,
        center_z: i32,
        radius_chunks: u32,
        target_sections: Option<&BTreeSet<RenderSectionKey>>,
    ) -> Result<Vec<u8>, JsValue> {
        self.compile_count += 1;
        let report = compile_generated_chunk_sections_with_catalog(
            &self.mesh_assets.catalog,
            ChunkPos {
                x: center_x,
                z: center_z,
            },
            radius_chunks,
            target_sections,
        )
        .map_err(JsValue::from)?;
        Ok(encode_textured_render_section_build_report(&report))
    }

    pub(crate) fn compile_snapshot_sections_for_targets_bytes(
        &mut self,
        snapshot_input_bytes: &js_sys::Uint8Array,
        target_sections: &js_sys::Int32Array,
    ) -> Result<Vec<u8>, JsValue> {
        self.compile_count += 1;
        let delta = decode_web_render_compile_delta(&snapshot_input_bytes.to_vec())
            .map_err(|error| JsValue::from_str(&error))?;
        self.apply_render_compile_delta(delta)
            .map_err(|error| JsValue::from_str(&error))?;
        let target_sections = render_section_keys_from_int32_array(&target_sections)
            .map_err(|error| JsValue::from_str(&error))?;
        let snapshots = self.snapshot_mirror.values().collect::<Vec<_>>();
        let report = compile_snapshot_chunk_sections_with_catalog(
            &self.mesh_assets.catalog,
            &snapshots,
            &target_sections,
            self.biome_zoom_seed,
            self.topology,
        )
        .map_err(JsValue::from)?;
        Ok(encode_textured_render_section_build_report(&report))
    }

    /// Apply a render-compile delta to the resident mirror. A `reset` delta clears the mirror
    /// and adopts the delta's generation (a full resync); otherwise the delta's generation must
    /// match the mirror's, else it is a desync (a worker that lost its mirror would see a
    /// non-reset delta against an empty mirror) and is rejected loudly rather than compiled
    /// against a partial mirror. Upserts then overwrite/insert and evictions remove, leaving the
    /// mirror equal to the client's currently-loaded set.
    fn apply_render_compile_delta(&mut self, delta: WebRenderCompileDelta) -> Result<(), String> {
        if delta.reset {
            self.snapshot_mirror.clear();
            self.mirror_generation = delta.generation;
        } else if delta.generation != self.mirror_generation {
            return Err(format!(
                "render compile delta generation {} does not match worker mirror generation {} \
                 (snapshot mirror desynced; a full resync is required)",
                delta.generation, self.mirror_generation
            ));
        }
        self.last_delta_upsert_count = delta.upserts.len();
        self.last_delta_eviction_count = delta.evictions.len();
        self.biome_zoom_seed = delta.biome_zoom_seed;
        self.topology = delta.topology;
        for pos in &delta.evictions {
            self.snapshot_mirror.remove(pos);
        }
        for snapshot in delta.upserts {
            self.snapshot_mirror.insert(snapshot.pos, snapshot);
        }
        Ok(())
    }
}

async fn remote_websocket_smoke_report(websocket_url: String) -> Result<JsValue, String> {
    let mut runtime = WebRuntime::websocket_remote(websocket_url.clone()).await?;
    let first = runtime
        .request_chunk_view_async(
            SMOKE_INITIAL_CENTER,
            SMOKE_RADIUS_CHUNKS,
            SMOKE_RADIUS_CHUNKS,
        )
        .await?;
    for _ in 0..500 {
        runtime.drain_pending_runner_updates_with_budget(RuntimeUpdatePumpBudget::unlimited())?;
        if runtime
            .client()
            .chunk_snapshot(SMOKE_INITIAL_CENTER)
            .is_some()
        {
            break;
        }
        wait_for_remote_worker_turn(10).await?;
    }
    let center_chunk_loaded = runtime
        .client()
        .chunk_snapshot(SMOKE_INITIAL_CENTER)
        .is_some();
    let second = runtime
        .request_chunk_view_async(SMOKE_MOVED_CENTER, SMOKE_RADIUS_CHUNKS, SMOKE_RADIUS_CHUNKS)
        .await?;
    for _ in 0..500 {
        runtime.drain_pending_runner_updates_with_budget(RuntimeUpdatePumpBudget::unlimited())?;
        let diagnostics = runtime.runner_diagnostics();
        if runtime
            .client()
            .chunk_snapshot(SMOKE_MOVED_CENTER)
            .is_some()
            && runtime
                .client()
                .chunk_snapshot(SMOKE_INITIAL_CENTER)
                .is_none()
            && diagnostics.command_queue_depth == 0
            && diagnostics.update_queue_depth == 0
        {
            break;
        }
        wait_for_remote_worker_turn(10).await?;
    }
    let moved_chunk_loaded = runtime
        .client()
        .chunk_snapshot(SMOKE_MOVED_CENTER)
        .is_some();
    let previous_chunk_unloaded = runtime
        .client()
        .chunk_snapshot(SMOKE_INITIAL_CENTER)
        .is_none();
    let command_count_before_idle = runtime.command_count();
    let day_time_before_idle = runtime.client().day_time();
    let inbound_frames_before_idle = runtime
        .runner_diagnostics()
        .runner_frame_metrics
        .inbound_frames;
    let mut unsolicited_publication = false;
    for _ in 0..250 {
        wait_for_remote_worker_turn(10).await?;
        runtime.drain_pending_runner_updates_with_budget(RuntimeUpdatePumpBudget::unlimited())?;
        let idle_diagnostics = runtime.runner_diagnostics();
        if runtime.command_count() == command_count_before_idle
            && idle_diagnostics.runner_frame_metrics.inbound_frames > inbound_frames_before_idle
            && runtime.client().day_time() != day_time_before_idle
        {
            unsolicited_publication = true;
            break;
        }
    }
    let diagnostics = runtime.runner_diagnostics();
    let metrics = diagnostics.runner_frame_metrics;
    let ok = runtime.client().host() == mclone_client::ClientHost::RemoteDedicated
        && diagnostics.kind == ServerRunnerKind::RemoteWebSocket
        && metrics.transport_kind == mclone_server::WorkerFrameTransportKind::WebSocket
        && metrics.request_frames >= 2
        && metrics.inbound_frames >= 3
        && center_chunk_loaded
        && moved_chunk_loaded
        && previous_chunk_unloaded
        && runtime.transport_drained()
        && runtime.protocol_codec_roundtrip()
        && diagnostics.command_queue_depth == 0
        && diagnostics.update_queue_depth == 0
        && runtime.command_count() == 2
        && unsolicited_publication
        && runtime.update_count() > 0;
    runtime.request_shutdown();

    let object = js_sys::Object::new();
    set_bool(&object, "ok", ok)?;
    set_string(&object, "url", &websocket_url)?;
    set_string(&object, "clientHost", "remote-dedicated")?;
    set_string(&object, "runnerKind", diagnostics.kind.label())?;
    set_bool(&object, "runnerRunning", diagnostics.running)?;
    set_bool(&object, "centerChunkLoaded", center_chunk_loaded)?;
    set_bool(&object, "movedChunkLoaded", moved_chunk_loaded)?;
    set_bool(&object, "previousChunkUnloaded", previous_chunk_unloaded)?;
    set_bool(&object, "unsolicitedPublication", unsolicited_publication)?;
    set_number(
        &object,
        "idleCommandCount",
        command_count_before_idle as f64,
    )?;
    set_number(
        &object,
        "idleResponseFramesBefore",
        inbound_frames_before_idle as f64,
    )?;
    set_number(
        &object,
        "idleResponseFramesAfter",
        metrics.inbound_frames as f64,
    )?;
    set_bool(&object, "transportDrained", runtime.transport_drained())?;
    set_bool(
        &object,
        "protocolCodecRoundtrip",
        runtime.protocol_codec_roundtrip(),
    )?;
    set_number(&object, "commandCount", runtime.command_count() as f64)?;
    set_number(&object, "updateCount", runtime.update_count() as f64)?;
    set_number(
        &object,
        "loadedChunkCount",
        runtime.client().loaded_chunk_count() as f64,
    )?;
    set_number(
        &object,
        "runnerCommandQueueDepth",
        diagnostics.command_queue_depth as f64,
    )?;
    set_number(
        &object,
        "runnerUpdateQueueDepth",
        diagnostics.update_queue_depth as f64,
    )?;
    set_worker_frame_metrics(&object, "runnerFrameMetrics", metrics)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("first"),
        &web_runtime_step_report_to_js(first)?,
    )
    .map_err(|error| format!("failed to attach first remote websocket report: {error:?}"))?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("second"),
        &web_runtime_step_report_to_js(second)?,
    )
    .map_err(|error| format!("failed to attach second remote websocket report: {error:?}"))?;
    Ok(object.into())
}

pub(crate) async fn wait_for_remote_worker_turn(timeout_ms: i32) -> Result<(), String> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let callback = wasm_bindgen::closure::Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::NULL);
        });
        let Some(window) = web_sys::window() else {
            let _ = reject.call1(
                &JsValue::NULL,
                &JsValue::from_str("browser window is unavailable"),
            );
            return;
        };
        if let Err(error) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.unchecked_ref(),
            timeout_ms,
        ) {
            let _ = reject.call1(&JsValue::NULL, &error);
        }
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map(|_| ())
        .map_err(|error| format!("remote worker turn failed: {error:?}"))
}

fn web_runtime_step_report_to_js(report: super::WebRuntimeStepReport) -> Result<JsValue, String> {
    let object = js_sys::Object::new();
    set_number(&object, "commandCount", report.command_count as f64)?;
    set_number(&object, "updateCount", report.update_count as f64)?;
    set_number(
        &object,
        "loadedChunkCount",
        report.loaded_chunk_count as f64,
    )?;
    set_bool(
        &object,
        "protocolCodecRoundtrip",
        report.protocol_codec_roundtrip,
    )?;
    set_bool(&object, "transportDrained", report.transport_drained)?;
    Ok(object.into())
}

#[wasm_bindgen]
pub fn mclone_web_packed_compile_report_summary(
    packed_report: js_sys::Uint8Array,
) -> Result<JsValue, JsValue> {
    let packed = packed_report.to_vec();
    packed_compile_report_summary_from_bytes(&packed).map_err(JsValue::from)
}

pub(crate) fn packed_compile_report_summary_from_bytes(packed: &[u8]) -> Result<JsValue, String> {
    let report = decode_textured_render_section_build_report(packed)
        .map_err(|error| format!("{error:#}"))?;
    packed_compile_report_summary_to_js(packed.len(), &report)
}

#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&info.to_string()));
    }));
}

fn compile_generated_chunk_sections_from_pack(
    asset_pack_bytes: Vec<u8>,
    center: ChunkPos,
    radius_chunks: u32,
    target_sections: Option<&BTreeSet<RenderSectionKey>>,
) -> Result<mclone_mesh::TexturedRenderSectionBuildReport, String> {
    let mesh_assets = load_textured_mesh_assets_from_pack(asset_pack_bytes)?;
    compile_generated_chunk_sections_with_catalog(
        &mesh_assets.catalog,
        center,
        radius_chunks,
        target_sections,
    )
}

fn compile_generated_chunk_sections_with_catalog(
    catalog: &TexturedMeshCatalog,
    center: ChunkPos,
    radius_chunks: u32,
    target_sections: Option<&BTreeSet<RenderSectionKey>>,
) -> Result<mclone_mesh::TexturedRenderSectionBuildReport, String> {
    let mut runtime = WebRuntime::local_integrated(SMOKE_SEED);
    runtime
        .request_chunk_view(center, radius_chunks, radius_chunks)
        .map_err(|error| {
            format!("failed to load generated chunk through web compiler runtime: {error}")
        })?;
    if runtime.client().chunk_snapshot(center).is_none() {
        return Err(format!(
            "web compiler runtime did not publish generated chunk {},{}",
            center.x, center.z
        ));
    }

    if let Some(target_sections) = target_sections {
        let snapshots = runtime
            .client()
            .chunk_snapshots()
            .cloned()
            .collect::<Vec<_>>();
        build_render_sections_from_snapshots_with_biome_zoom_seed(
            &snapshots,
            catalog,
            target_sections,
            runtime.client().biome_zoom_seed(),
        )
        .map_err(|error| format!("failed to compile targeted generated render sections: {error:#}"))
    } else {
        build_client_textured_sections(runtime.client(), catalog).map_err(|error| {
            format!("failed to compile generated textured render sections: {error:#}")
        })
    }
}

fn compile_snapshot_chunk_sections_with_catalog<S: std::borrow::Borrow<ChunkSnapshot>>(
    catalog: &TexturedMeshCatalog,
    snapshots: &[S],
    target_sections: &BTreeSet<RenderSectionKey>,
    biome_zoom_seed: Option<i64>,
    topology: HorizontalTopology,
) -> Result<mclone_mesh::TexturedRenderSectionBuildReport, String> {
    build_render_sections_from_snapshots_with_biome_zoom_seed_and_topology(
        snapshots,
        catalog,
        target_sections,
        biome_zoom_seed,
        topology,
    )
    .map_err(|error| format!("failed to compile snapshot render sections: {error:#}"))
}

/// A per-frame render-compile input delta against the worker's resident snapshot mirror
/// (067 Stage 4). `reset` means the worker must clear its mirror and adopt `generation`
/// before applying the upserts (a full resync — the natural shape of the first compile,
/// or recovery after a desync); otherwise `generation` must match the worker's mirror.
struct WebRenderCompileDelta {
    generation: u32,
    reset: bool,
    biome_zoom_seed: Option<i64>,
    topology: HorizontalTopology,
    upserts: Vec<ChunkSnapshot>,
    evictions: Vec<ChunkPos>,
}

fn encode_web_render_compile_delta(
    generation: u32,
    reset: bool,
    biome_zoom_seed: Option<i64>,
    topology: HorizontalTopology,
    upserts: &[ChunkSnapshot],
    evictions: &[ChunkPos],
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(WEB_RENDER_COMPILE_DELTA_MAGIC);
    bytes.extend_from_slice(&generation.to_le_bytes());
    let mut flags = if reset {
        WEB_RENDER_COMPILE_DELTA_FLAG_RESET
    } else {
        0
    };
    if biome_zoom_seed.is_some() {
        flags |= WEB_RENDER_COMPILE_DELTA_FLAG_BIOME_ZOOM_SEED;
    }
    bytes.extend_from_slice(&flags.to_le_bytes());
    encode_web_axis_topology(&mut bytes, topology.x);
    encode_web_axis_topology(&mut bytes, topology.z);
    if let Some(seed) = biome_zoom_seed {
        bytes.extend_from_slice(&seed.to_le_bytes());
    }
    write_web_compile_input_u32(&mut bytes, upserts.len(), "delta upsert count")?;
    for snapshot in upserts {
        let frame = encode_server_update(&ServerUpdate::ChunkSnapshot(snapshot.clone())).map_err(
            |error| {
                format!(
                    "failed to encode compile upsert snapshot {:?}: {error}",
                    snapshot.pos
                )
            },
        )?;
        write_web_compile_input_u32(&mut bytes, frame.len(), "delta upsert frame length")?;
        bytes.extend_from_slice(&frame);
    }
    write_web_compile_input_u32(&mut bytes, evictions.len(), "delta eviction count")?;
    for pos in evictions {
        bytes.extend_from_slice(&pos.x.to_le_bytes());
        bytes.extend_from_slice(&pos.z.to_le_bytes());
    }
    Ok(bytes)
}

fn decode_web_render_compile_delta(bytes: &[u8]) -> Result<WebRenderCompileDelta, String> {
    let mut reader = WebRenderCompileInputReader::new(bytes);
    let magic = reader.read_bytes(WEB_RENDER_COMPILE_DELTA_MAGIC.len(), "magic")?;
    if magic != WEB_RENDER_COMPILE_DELTA_MAGIC {
        return Err("web render compile delta had an invalid magic header".to_string());
    }
    let generation = reader.read_u32("delta generation")?;
    let flags = reader.read_u32("delta flags")?;
    let reset = flags & WEB_RENDER_COMPILE_DELTA_FLAG_RESET != 0;
    let topology = HorizontalTopology::new(
        decode_web_axis_topology(&mut reader, "x")?,
        decode_web_axis_topology(&mut reader, "z")?,
    );
    topology
        .validate()
        .map_err(|error| format!("invalid web render compile topology: {error}"))?;
    let biome_zoom_seed = if flags & WEB_RENDER_COMPILE_DELTA_FLAG_BIOME_ZOOM_SEED != 0 {
        Some(reader.read_i64("delta biome zoom seed")?)
    } else {
        None
    };
    let upsert_count = reader.read_u32("delta upsert count")? as usize;
    let mut upserts = Vec::with_capacity(upsert_count);
    for index in 0..upsert_count {
        let frame_len = reader.read_u32("delta upsert frame length")? as usize;
        let frame = reader.read_bytes(frame_len, "delta upsert frame")?;
        let update = decode_server_update(frame)
            .map_err(|error| format!("failed to decode compile upsert frame {index}: {error}"))?;
        match update {
            ServerUpdate::ChunkSnapshot(snapshot) => upserts.push(snapshot),
            _ => {
                return Err(format!(
                    "compile upsert frame {index} did not contain a chunk snapshot"
                ));
            }
        }
    }
    let eviction_count = reader.read_u32("delta eviction count")? as usize;
    let mut evictions = Vec::with_capacity(eviction_count);
    for _ in 0..eviction_count {
        let x = reader.read_i32("delta eviction x")?;
        let z = reader.read_i32("delta eviction z")?;
        evictions.push(ChunkPos { x, z });
    }
    reader.finish()?;
    Ok(WebRenderCompileDelta {
        generation,
        reset,
        biome_zoom_seed,
        topology,
        upserts,
        evictions,
    })
}

fn encode_web_axis_topology(bytes: &mut Vec<u8>, axis: AxisTopology) {
    let (tag, first, second) = match axis {
        AxisTopology::Unbounded => (0_u32, 0_i32, 0_u32),
        AxisTopology::Finite {
            minimum_chunk,
            maximum_chunk_exclusive,
        } => (1, minimum_chunk, maximum_chunk_exclusive as u32),
        AxisTopology::Periodic {
            minimum_chunk,
            period_chunks,
        } => (2, minimum_chunk, period_chunks),
    };
    bytes.extend_from_slice(&tag.to_le_bytes());
    bytes.extend_from_slice(&first.to_le_bytes());
    bytes.extend_from_slice(&second.to_le_bytes());
}

fn decode_web_axis_topology(
    reader: &mut WebRenderCompileInputReader<'_>,
    label: &str,
) -> Result<AxisTopology, String> {
    let tag = reader.read_u32("topology axis tag")?;
    let first = reader.read_i32("topology axis first value")?;
    let second = reader.read_u32("topology axis second value")?;
    match tag {
        0 if first == 0 && second == 0 => Ok(AxisTopology::Unbounded),
        1 => Ok(AxisTopology::finite(first, second as i32)),
        2 => Ok(AxisTopology::periodic(first, second)),
        0 => Err(format!(
            "web render compile {label} unbounded axis carried nonzero values"
        )),
        _ => Err(format!(
            "web render compile {label} axis had unknown tag {tag}"
        )),
    }
}

fn write_web_compile_input_u32(
    bytes: &mut Vec<u8>,
    value: usize,
    label: &str,
) -> Result<(), String> {
    let value = u32::try_from(value)
        .map_err(|_| format!("web render compile input {label} {value} exceeded u32"))?;
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

struct WebRenderCompileInputReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> WebRenderCompileInputReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_u32(&mut self, label: &str) -> Result<u32, String> {
        let bytes = self.read_bytes(4, label)?;
        Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| {
            format!("web render compile input {label} was not 4 bytes")
        })?))
    }

    fn read_i32(&mut self, label: &str) -> Result<i32, String> {
        let bytes = self.read_bytes(4, label)?;
        Ok(i32::from_le_bytes(bytes.try_into().map_err(|_| {
            format!("web render compile input {label} was not 4 bytes")
        })?))
    }

    fn read_i64(&mut self, label: &str) -> Result<i64, String> {
        let bytes = self.read_bytes(8, label)?;
        Ok(i64::from_le_bytes(bytes.try_into().map_err(|_| {
            format!("web render compile input {label} was not 8 bytes")
        })?))
    }

    fn read_bytes(&mut self, len: usize, label: &str) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| format!("web render compile input {label} length overflowed"))?;
        let bytes = self.bytes.get(self.offset..end).ok_or_else(|| {
            format!(
                "web render compile input truncated while reading {label}: need {len} bytes at offset {} from {} bytes",
                self.offset,
                self.bytes.len()
            )
        })?;
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), String> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(format!(
                "web render compile input had {} trailing bytes",
                self.bytes.len() - self.offset
            ))
        }
    }
}

fn encode_web_far_lod_tile_mesh(mesh: &FarTerrainLodTileMesh) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(
        WEB_FAR_LOD_TILE_MESH_MAGIC.len()
            + 20
            + mesh.vertices().len() * std::mem::size_of::<f32>()
            + mesh.indices().len() * std::mem::size_of::<u32>(),
    );
    bytes.extend_from_slice(WEB_FAR_LOD_TILE_MESH_MAGIC);
    bytes.extend_from_slice(&mesh.key().chunk.x.to_le_bytes());
    bytes.extend_from_slice(&mesh.key().chunk.z.to_le_bytes());
    bytes.extend_from_slice(&u32::from(mesh.key().level).to_le_bytes());
    write_web_compile_input_u32(
        &mut bytes,
        mesh.vertices().len(),
        "far LOD vertex float count",
    )?;
    write_web_compile_input_u32(&mut bytes, mesh.indices().len(), "far LOD index count")?;
    for value in mesh.vertices() {
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    for index in mesh.indices() {
        bytes.extend_from_slice(&index.to_le_bytes());
    }
    Ok(bytes)
}

fn decode_web_far_lod_tile_mesh(bytes: &[u8]) -> Result<FarTerrainLodTileMesh, String> {
    let mut reader = WebRenderCompileInputReader::new(bytes);
    let magic = reader.read_bytes(WEB_FAR_LOD_TILE_MESH_MAGIC.len(), "far LOD magic")?;
    if magic != WEB_FAR_LOD_TILE_MESH_MAGIC {
        return Err("web far LOD tile mesh had an invalid magic header".to_owned());
    }
    let key = LodTileKey::new(
        ChunkPos::new(
            reader.read_i32("far LOD chunk x")?,
            reader.read_i32("far LOD chunk z")?,
        ),
        u8::try_from(reader.read_u32("far LOD level")?)
            .map_err(|_| "web far LOD level exceeded u8".to_owned())?,
    );
    let vertex_float_count = reader.read_u32("far LOD vertex float count")? as usize;
    let index_count = reader.read_u32("far LOD index count")? as usize;
    let mut vertices = Vec::with_capacity(vertex_float_count);
    for _ in 0..vertex_float_count {
        vertices.push(f32::from_bits(reader.read_u32("far LOD vertex float")?));
    }
    let mut indices = Vec::with_capacity(index_count);
    for _ in 0..index_count {
        indices.push(reader.read_u32("far LOD index")?);
    }
    reader.finish()?;
    Ok(FarTerrainLodTileMesh::new(key, vertices, indices))
}

fn packed_compile_report_summary_to_js(
    byte_length: usize,
    report: &mclone_mesh::TexturedRenderSectionBuildReport,
) -> Result<JsValue, String> {
    let summary = summarize_textured_render_section_build_report(report);
    let object = js_sys::Object::new();
    set_bool(&object, "ok", true)?;
    set_number(&object, "byteLength", byte_length as f64)?;
    set_number(&object, "sectionCount", summary.section_count as f64)?;
    set_number(
        &object,
        "nonEmptySectionCount",
        summary.non_empty_section_count as f64,
    )?;
    set_number(&object, "vertexCount", f64::from(summary.vertex_count))?;
    set_number(&object, "indexCount", f64::from(summary.index_count))?;
    set_number(&object, "faceCount", f64::from(summary.face_count()))?;
    set_number(
        &object,
        "visibilityGraphBuildCount",
        summary.visibility_graph_stats.build_count as f64,
    )?;
    set_number(
        &object,
        "visibilityGraphTotalMs",
        summary.visibility_graph_stats.total_ms,
    )?;
    set_number(
        &object,
        "visibilityGraphWorstMs",
        summary.visibility_graph_stats.worst_ms,
    )?;
    Ok(object.into())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CanvasRenderReport {
    width: u32,
    height: u32,
}

impl CanvasRenderReport {
    fn packed_bits(self) -> u32 {
        CANVAS_OK_BIT
            | CANVAS_RENDERED_BIT
            | CANVAS_CONFIGURED_BIT
            | (packed_dimension(self.width) << CANVAS_WIDTH_SHIFT)
            | (packed_dimension(self.height) << CANVAS_HEIGHT_SHIFT)
    }
}

fn render_section_compile_result_from_report(
    target_sections: BTreeSet<RenderSectionKey>,
    mut section_revisions: BTreeMap<RenderSectionKey, u64>,
    section_report: TexturedRenderSectionBuildReport,
) -> RenderSectionCompileResult {
    let reported_sections = section_report
        .sections
        .iter()
        .map(|section| section.key)
        .collect::<BTreeSet<_>>();
    for missing_key in target_sections.difference(&reported_sections) {
        let submitted_revision = section_revisions
            .get(missing_key)
            .copied()
            .unwrap_or_default();
        section_revisions.insert(*missing_key, submitted_revision.wrapping_sub(1));
    }
    RenderSectionCompileResult {
        target_sections,
        section_revisions,
        result: Ok(section_report),
    }
}

/// Resident render-compiler shared ring (067 Stage 2), owned by main wasm.
///
/// The control buffers carry the atomic status / byte-count / capacity words; the
/// data buffers carry the encoded snapshot input and the packed section report.
/// Allocated once per session and re-armed per compile, so there is no per-request
/// `SharedArrayBuffer` churn. The worker reads the input + writes the result through
/// the same buffers handed to it on the JS doorbell.
struct WebRenderCompilerSharedArena {
    result_control_buffer: js_sys::SharedArrayBuffer,
    result_control: js_sys::Int32Array,
    result_buffer: js_sys::SharedArrayBuffer,
    result_capacity: u32,
    input_control_buffer: js_sys::SharedArrayBuffer,
    input_control: js_sys::Int32Array,
    input_buffer: js_sys::SharedArrayBuffer,
    input_capacity: u32,
    input_byte_length: u32,
}

impl WebRenderCompilerSharedArena {
    fn new() -> Result<Self, String> {
        let result_control_buffer =
            js_sys::SharedArrayBuffer::new(RENDER_COMPILER_SHARED_RESULT_CONTROL_WORDS * 4);
        let result_control = js_sys::Int32Array::new(result_control_buffer.as_ref());
        let result_capacity = RENDER_COMPILER_DEFAULT_SHARED_RESULT_CAPACITY;
        let result_buffer = js_sys::SharedArrayBuffer::new(result_capacity);
        let input_control_buffer =
            js_sys::SharedArrayBuffer::new(RENDER_COMPILER_SHARED_INPUT_CONTROL_WORDS * 4);
        let input_control = js_sys::Int32Array::new(input_control_buffer.as_ref());
        let input_capacity = RENDER_COMPILER_DEFAULT_SHARED_INPUT_CAPACITY;
        let input_buffer = js_sys::SharedArrayBuffer::new(input_capacity);
        let arena = Self {
            result_control_buffer,
            result_control,
            result_buffer,
            result_capacity,
            input_control_buffer,
            input_control,
            input_buffer,
            input_capacity,
            input_byte_length: 0,
        };
        atomic_store(
            &arena.result_control,
            RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX,
            RENDER_COMPILER_SHARED_RESULT_PENDING,
        )?;
        atomic_store(
            &arena.result_control,
            RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX,
            0,
        )?;
        atomic_store(
            &arena.result_control,
            RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX,
            arena.result_capacity as i32,
        )?;
        atomic_store(
            &arena.input_control,
            RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX,
            RENDER_COMPILER_SHARED_INPUT_READY,
        )?;
        atomic_store(
            &arena.input_control,
            RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX,
            0,
        )?;
        atomic_store(
            &arena.input_control,
            RENDER_COMPILER_SHARED_INPUT_CAPACITY_INDEX,
            arena.input_capacity as i32,
        )?;
        Ok(arena)
    }

    /// Fill the input arena with `input_bytes` and arm both control words for a fresh
    /// compile. Returns `true` when the resident input buffer had to grow in place.
    fn arm_for_submit(&mut self, input_bytes: &[u8]) -> Result<bool, String> {
        let byte_length = u32::try_from(input_bytes.len()).map_err(|_| {
            format!(
                "render compile input of {} bytes exceeded u32",
                input_bytes.len()
            )
        })?;
        let mut grew = false;
        if byte_length > self.input_capacity {
            self.input_buffer = js_sys::SharedArrayBuffer::new(byte_length);
            self.input_capacity = byte_length;
            grew = true;
        }
        if byte_length > 0 {
            let view = js_sys::Uint8Array::new_with_byte_offset_and_length(
                self.input_buffer.as_ref(),
                0,
                byte_length,
            );
            view.copy_from(input_bytes);
        }
        self.input_byte_length = byte_length;
        atomic_store(
            &self.input_control,
            RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX,
            byte_length as i32,
        )?;
        atomic_store(
            &self.input_control,
            RENDER_COMPILER_SHARED_INPUT_CAPACITY_INDEX,
            self.input_capacity as i32,
        )?;
        atomic_store(
            &self.input_control,
            RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX,
            RENDER_COMPILER_SHARED_INPUT_READY,
        )?;
        // Arm the result control word last so the worker only observes PENDING after
        // the input payload is fully published into shared memory.
        self.arm_result()?;
        Ok(grew)
    }

    fn arm_result(&self) -> Result<(), String> {
        atomic_store(
            &self.result_control,
            RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX,
            0,
        )?;
        atomic_store(
            &self.result_control,
            RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX,
            self.result_capacity as i32,
        )?;
        atomic_store(
            &self.result_control,
            RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX,
            RENDER_COMPILER_SHARED_RESULT_PENDING,
        )?;
        Ok(())
    }

    fn poll_result_status(&self) -> Result<i32, String> {
        atomic_load(
            &self.result_control,
            RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX,
        )
    }

    fn result_byte_length(&self) -> Result<u32, String> {
        let bytes = atomic_load(
            &self.result_control,
            RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX,
        )?;
        u32::try_from(bytes)
            .map_err(|_| format!("render compile result reported a negative byte count {bytes}"))
    }

    fn result_capacity_word(&self) -> Result<u32, String> {
        let capacity = atomic_load(
            &self.result_control,
            RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX,
        )?;
        u32::try_from(capacity)
            .map_err(|_| format!("render compile result reported a negative capacity {capacity}"))
    }

    fn read_result_bytes(&self) -> Result<Vec<u8>, String> {
        let byte_length = self.result_byte_length()?;
        if byte_length > self.result_buffer.byte_length() {
            return Err(format!(
                "render compile result byte length {byte_length} exceeds resident buffer {}",
                self.result_buffer.byte_length()
            ));
        }
        if byte_length == 0 {
            return Ok(Vec::new());
        }
        let view = js_sys::Uint8Array::new_with_byte_offset_and_length(
            self.result_buffer.as_ref(),
            0,
            byte_length,
        );
        Ok(view.to_vec())
    }

    /// Grow the resident result buffer so a future compile of `byte_length` fits.
    /// Used after a worker overflow (its one-off larger buffer is not reachable by
    /// main wasm), so the retry lands back in the resident ring.
    fn grow_result(&mut self, byte_length: u32) {
        if byte_length > self.result_capacity {
            self.result_buffer = js_sys::SharedArrayBuffer::new(byte_length);
            self.result_capacity = byte_length;
        }
    }
}

enum WebSharedCompileInFlight {
    Sections {
        request_id: u32,
        target_sections: BTreeSet<RenderSectionKey>,
        section_revisions: BTreeMap<RenderSectionKey, u64>,
    },
    FarLod {
        request_id: u32,
        request: FarTerrainLodBuildRequest,
    },
}

impl WebSharedCompileInFlight {
    fn request_id(&self) -> u32 {
        match self {
            Self::Sections { request_id, .. } | Self::FarLod { request_id, .. } => *request_id,
        }
    }
}

/// Web `RenderSectionCompiler` over the resident `SharedArrayBuffer` ring (067 Stage 2),
/// shipping a per-frame *delta* against the worker's resident snapshot mirror (067 Stage 4).
///
/// `submit` diffs the request's loaded snapshots against `mirror_tracking` — a cheap
/// `(pos, revision)` shadow of what the worker mirror holds — to compute the changed
/// columns (upserts) and unloaded columns (evictions), encodes only that delta into the
/// resident input arena, and arms the result control word; the Rust coordinator posts
/// a tiny doorbell through the generic browser Worker transport.
/// `try_recv_completed` polls the result control word from main wasm via `js_sys::Atomics`
/// and decodes the packed report in place — no JavaScript in the result data path. A
/// single compile is in flight at a time, matching the existing busy-flag streaming model,
/// so the mirror is mutated by exactly one compile at a time and needs no locking.
struct WebRenderSectionCompiler {
    shared: Option<WebRenderCompilerSharedArena>,
    in_flight: Option<WebSharedCompileInFlight>,
    completed_sections: VecDeque<RenderSectionCompileResult>,
    completed_far_lod: VecDeque<FarTerrainLodBuildResult>,
    far_lod_error: Option<String>,
    next_request_id: u32,
    pending_jobs: usize,
    compile_count: usize,
    shared_result_response_count: usize,
    shared_result_byte_count: usize,
    shared_result_overflow_count: usize,
    shared_input_grow_count: usize,
    last_packed_byte_length: usize,
    last_result_capacity_bytes: usize,
    last_result_overflow: bool,
    // 067 Stage 4: `(pos, revision)` shadow of what the worker's resident snapshot mirror
    // holds. `submit` diffs the loaded snapshots against this to build the delta and then
    // advances it on submit (the worker applies the same delta on receipt, so the two stay
    // in lockstep). Cleared on a worker-reported failure to force a full resync next submit;
    // empty means "the next submit is a full resync" (the first compile, and recovery).
    mirror_tracking: BTreeMap<ChunkPos, ChunkRevision>,
    // Monotonic mirror epoch bumped on every full resync. The worker rejects a non-reset
    // delta whose generation does not match its mirror's, turning a latent desync (e.g. a
    // restarted worker that silently lost its mirror) into a loud failure instead of a
    // compile against a partial mirror.
    mirror_generation: u32,
    // Count of changed columns shipped (upserts) for the in-flight request, surfaced on the
    // doorbell as the delta width. The worker reports the matching eviction + resident-mirror
    // counts from its own session getters, so they are not duplicated here.
    last_input_upsert_count: usize,
    // 067 follow-up 1: the eviction/reset/generation parts of the delta, computed by
    // `stage_streaming_delta` against the live snapshots and consumed by the `submit` that
    // follows it in the same streaming step. `submit` requires this to be present; the shared
    // streaming loop always stages immediately before submitting.
    staged_delta: Option<WebStagedSubmitDelta>,
    // 067 follow-up 1: number of `ChunkSnapshot` clones the most recent `stage_streaming_delta`
    // performed for submit — counted at the clone site so it stays a true clone count. It must
    // equal the upserts shipped (`last_input_upsert_count`); a whole-world clone that ships only
    // a delta would make it diverge, which the smoke asserts as a regression fence.
    last_submit_cloned_column_count: usize,
}

/// The non-snapshot parts of a per-frame compile delta, computed by `stage_streaming_delta`
/// (which has the live `&ClientRuntime` borrows) and consumed by the `submit` that follows in
/// the same streaming step. The changed columns themselves ride along as `request.snapshots`
/// (the upserts the collector returned); only the evictions + reset/generation need staging,
/// because the shared `RenderSectionCompileRequest` carries no eviction/generation fields.
#[derive(Clone, Debug, Default)]
struct WebStagedSubmitDelta {
    evictions: Vec<ChunkPos>,
    reset: bool,
    generation: u32,
}

impl WebRenderSectionCompiler {
    fn new() -> Self {
        let shared = if shared_memory_supported() {
            WebRenderCompilerSharedArena::new().ok()
        } else {
            None
        };
        Self {
            last_result_capacity_bytes: shared
                .as_ref()
                .map(|arena| arena.result_capacity as usize)
                .unwrap_or(0),
            shared,
            in_flight: None,
            completed_sections: VecDeque::new(),
            completed_far_lod: VecDeque::new(),
            far_lod_error: None,
            next_request_id: 1,
            pending_jobs: 0,
            compile_count: 0,
            shared_result_response_count: 0,
            shared_result_byte_count: 0,
            shared_result_overflow_count: 0,
            shared_input_grow_count: 0,
            last_packed_byte_length: 0,
            last_result_overflow: false,
            mirror_tracking: BTreeMap::new(),
            mirror_generation: 0,
            last_input_upsert_count: 0,
            staged_delta: None,
            last_submit_cloned_column_count: 0,
        }
    }

    fn in_flight_request_id(&self) -> Option<u32> {
        self.in_flight
            .as_ref()
            .map(WebSharedCompileInFlight::request_id)
    }

    fn allocate_request_id(&mut self) -> u32 {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        request_id
    }

    fn take_in_flight(&mut self) -> Option<WebSharedCompileInFlight> {
        self.pending_jobs = 0;
        self.in_flight.take()
    }

    fn fail_in_flight(&mut self, message: String) {
        let Some(in_flight) = self.take_in_flight() else {
            return;
        };
        self.mirror_tracking.clear();
        self.staged_delta = None;
        match in_flight {
            WebSharedCompileInFlight::Sections {
                target_sections,
                section_revisions,
                ..
            } => self
                .completed_sections
                .push_back(RenderSectionCompileResult {
                    target_sections,
                    section_revisions,
                    result: Err(message),
                }),
            WebSharedCompileInFlight::FarLod { .. } => {
                self.far_lod_error = Some(message);
            }
        }
    }

    fn reset_worker_mirror(&mut self) {
        self.mirror_tracking.clear();
        self.staged_delta = None;
    }

    /// Attach the resident shared buffers + byte counts to a worker doorbell message.
    /// The Rust coordinator adds transport metadata and posts it through the generic
    /// browser Worker transport; the worker reads the input and writes the packed result
    /// into the same buffers this compiler polls.
    fn write_doorbell_arenas(&self, object: &js_sys::Object) -> Result<(), String> {
        let Some(arena) = self.shared.as_ref() else {
            return Ok(());
        };
        render_compiler_set_value(
            object,
            "sharedInputControlBuffer",
            arena.input_control_buffer.as_ref(),
        )?;
        render_compiler_set_value(object, "sharedInputBuffer", arena.input_buffer.as_ref())?;
        set_number(
            object,
            "sharedInputByteLength",
            f64::from(arena.input_byte_length),
        )?;
        set_number(
            object,
            "sharedInputBufferCapacityBytes",
            f64::from(arena.input_capacity),
        )?;
        render_compiler_set_value(
            object,
            "sharedResultControlBuffer",
            arena.result_control_buffer.as_ref(),
        )?;
        render_compiler_set_value(
            object,
            "sharedResultResponseBuffer",
            arena.result_buffer.as_ref(),
        )?;
        set_number(
            object,
            "sharedResultBufferCapacityBytes",
            f64::from(arena.result_capacity),
        )?;
        Ok(())
    }

    fn write_doorbell_result_arena(&self, object: &js_sys::Object) -> Result<(), String> {
        let Some(arena) = self.shared.as_ref() else {
            return Ok(());
        };
        render_compiler_set_value(
            object,
            "sharedResultControlBuffer",
            arena.result_control_buffer.as_ref(),
        )?;
        render_compiler_set_value(
            object,
            "sharedResultResponseBuffer",
            arena.result_buffer.as_ref(),
        )?;
        set_number(
            object,
            "sharedResultBufferCapacityBytes",
            f64::from(arena.result_capacity),
        )
    }

    /// Build a complete worker doorbell for the in-flight compile (067 Stage 3): the
    /// request id, the budget's target section keys (so the worker compiles only those —
    /// the per-frame increment, not a whole-view job), the delta input width, and the
    /// resident shared buffers. The Rust coordinator posts it through the generic
    /// transport; the worker writes the packed result back into the same buffers
    /// `try_recv_completed` polls. Returns whether a doorbell was written (false when no
    /// compile is in flight).
    fn write_doorbell(&self, object: &js_sys::Object) -> Result<bool, String> {
        let Some(in_flight) = self.in_flight.as_ref() else {
            return Ok(false);
        };
        set_number(object, "requestId", f64::from(in_flight.request_id()))?;
        match in_flight {
            WebSharedCompileInFlight::Sections {
                target_sections, ..
            } => {
                set_string(object, "workKind", "render-sections")?;
                write_target_sections(object, target_sections)?;
                set_number(
                    object,
                    "submittedCompileSectionCount",
                    target_sections.len() as f64,
                )?;
                set_number(
                    object,
                    "snapshotInputChunkCount",
                    self.last_input_upsert_count as f64,
                )?;
                set_number(
                    object,
                    "snapshotInputClonedColumnCount",
                    self.last_submit_cloned_column_count as f64,
                )?;
                self.write_doorbell_arenas(object)?;
            }
            WebSharedCompileInFlight::FarLod { request, .. } => {
                let input = request.worker_input();
                set_string(object, "workKind", "far-lod")?;
                set_string(object, "farLodSeed", &input.seed.to_string())?;
                set_number(object, "farLodChunkX", f64::from(input.key.chunk.x))?;
                set_number(object, "farLodChunkZ", f64::from(input.key.chunk.z))?;
                set_number(object, "farLodLevel", f64::from(input.key.level))?;
                set_number(
                    object,
                    "farLodSampleSpacingBlocks",
                    f64::from(input.sample_spacing_blocks),
                )?;
                for (name, spacing) in [
                    (
                        "farLodWestSampleSpacingBlocks",
                        input.neighbor_sample_spacings[0],
                    ),
                    (
                        "farLodEastSampleSpacingBlocks",
                        input.neighbor_sample_spacings[1],
                    ),
                    (
                        "farLodNorthSampleSpacingBlocks",
                        input.neighbor_sample_spacings[2],
                    ),
                    (
                        "farLodSouthSampleSpacingBlocks",
                        input.neighbor_sample_spacings[3],
                    ),
                ] {
                    set_number(object, name, f64::from(spacing))?;
                }
                self.write_doorbell_result_arena(object)?;
            }
        }
        Ok(true)
    }

    /// Compute this frame's compile delta against the worker-mirror shadow from the *borrowed*
    /// live snapshots, cloning only the changed columns (067 follow-up 1). Returns the upsert
    /// snapshots (which become `request.snapshots` for the `submit` that follows in the same
    /// streaming step) and stages the evictions + reset/generation on `self` for that submit.
    ///
    /// This does **not** mutate the mirror shadow or the generation: the streaming loop runs
    /// this every frame the budget gate opens, but only actually submits when there is a ready
    /// plan, so the shadow advance (and the generation bump) is deferred to `submit`, which runs
    /// exactly when a delta is armed. A frame that stages but does not submit simply has its
    /// staged delta overwritten by the next frame's stage before that frame's submit.
    fn stage_streaming_delta(&mut self, client: &ClientRuntime) -> Vec<ChunkSnapshot> {
        // An empty shadow is a full resync (the first compile, and post-failure recovery): every
        // loaded column ships as an upsert under a freshly bumped mirror epoch, with no evictions.
        // The generation is computed prospectively here and only committed by `submit`.
        let reset = self.mirror_tracking.is_empty();
        let generation = if reset {
            self.mirror_generation.wrapping_add(1).max(1)
        } else {
            self.mirror_generation
        };
        let mut upserts: Vec<ChunkSnapshot> = Vec::new();
        let mut cloned_columns = 0usize;
        let mut current_positions: BTreeSet<ChunkPos> = BTreeSet::new();
        for snapshot in client.chunk_snapshots() {
            current_positions.insert(snapshot.pos);
            if self.mirror_tracking.get(&snapshot.pos) != Some(&snapshot.revision) {
                upserts.push(snapshot.clone());
                cloned_columns += 1;
            }
        }
        let evictions: Vec<ChunkPos> = self
            .mirror_tracking
            .keys()
            .copied()
            .filter(|pos| !current_positions.contains(pos))
            .collect();
        self.last_submit_cloned_column_count = cloned_columns;
        self.staged_delta = Some(WebStagedSubmitDelta {
            evictions,
            reset,
            generation,
        });
        upserts
    }

    fn poll_completed_work(&mut self) -> anyhow::Result<()> {
        if self.in_flight.is_none() {
            return Ok(());
        }
        let Some(arena) = self.shared.as_ref() else {
            return Ok(());
        };
        let status = arena.poll_result_status().map_err(anyhow::Error::msg)?;
        match status {
            RENDER_COMPILER_SHARED_RESULT_COMPLETE => {
                let packed = self
                    .shared
                    .as_ref()
                    .expect("shared arena present")
                    .read_result_bytes()
                    .map_err(anyhow::Error::msg)?;
                let in_flight = self
                    .take_in_flight()
                    .expect("in-flight compile present after status check");
                self.last_packed_byte_length = packed.len();
                self.last_result_overflow = false;
                self.shared_result_response_count += 1;
                self.shared_result_byte_count += packed.len();
                if let Some(arena) = self.shared.as_ref() {
                    self.last_result_capacity_bytes = arena.result_capacity as usize;
                }
                match in_flight {
                    WebSharedCompileInFlight::Sections {
                        target_sections,
                        section_revisions,
                        ..
                    } => {
                        let section_report = decode_textured_render_section_build_report(&packed)
                            .map_err(|error| {
                            anyhow::anyhow!(
                                "failed to decode packed render section report: {error:#}"
                            )
                        })?;
                        self.completed_sections.push_back(
                            render_section_compile_result_from_report(
                                target_sections,
                                section_revisions,
                                section_report,
                            ),
                        );
                    }
                    WebSharedCompileInFlight::FarLod { request, .. } => {
                        let mesh =
                            decode_web_far_lod_tile_mesh(&packed).map_err(anyhow::Error::msg)?;
                        self.completed_far_lod
                            .push_back(request.complete_worker_mesh(mesh)?);
                    }
                }
            }
            RENDER_COMPILER_SHARED_RESULT_OVERFLOW => {
                let required = self
                    .shared
                    .as_ref()
                    .expect("shared arena present")
                    .result_capacity_word()
                    .map_err(anyhow::Error::msg)?;
                if let Some(arena) = self.shared.as_mut() {
                    arena.grow_result(required);
                    self.last_result_capacity_bytes = arena.result_capacity as usize;
                }
                self.shared_result_overflow_count += 1;
                self.last_result_overflow = true;
                let in_flight = self.take_in_flight().expect("in-flight compile present");
                let message = format!(
                    "render compile result overflowed the resident shared buffer ({required} bytes required)"
                );
                match in_flight {
                    WebSharedCompileInFlight::Sections {
                        target_sections,
                        section_revisions,
                        ..
                    } => self
                        .completed_sections
                        .push_back(RenderSectionCompileResult {
                            target_sections,
                            section_revisions,
                            result: Err(message),
                        }),
                    WebSharedCompileInFlight::FarLod { .. } => {
                        self.far_lod_error = Some(message);
                    }
                }
            }
            RENDER_COMPILER_SHARED_RESULT_FAILED => {
                self.last_result_overflow = false;
                let in_flight = self.take_in_flight().expect("in-flight compile present");
                match in_flight {
                    WebSharedCompileInFlight::Sections {
                        target_sections,
                        section_revisions,
                        ..
                    } => {
                        self.mirror_tracking.clear();
                        self.completed_sections
                            .push_back(RenderSectionCompileResult {
                                target_sections,
                                section_revisions,
                                result: Err(
                                    "render compiler worker reported a failed compile".to_owned()
                                ),
                            });
                    }
                    WebSharedCompileInFlight::FarLod { .. } => {
                        self.far_lod_error = Some(
                            "render compiler worker reported a failed far LOD compile".to_owned(),
                        );
                    }
                }
            }
            RENDER_COMPILER_SHARED_RESULT_PENDING | 0 => {}
            other => {
                return Err(anyhow::anyhow!(
                    "render compile result control word had unexpected status {other}"
                ));
            }
        }
        Ok(())
    }
}

fn render_compiler_set_value(
    object: &js_sys::Object,
    name: &str,
    value: &JsValue,
) -> Result<(), String> {
    js_sys::Reflect::set(object, &JsValue::from_str(name), value)
        .map(|_| ())
        .map_err(|error| format!("failed to set render compile doorbell field {name}: {error:?}"))
}

impl RenderSectionCompiler for WebRenderSectionCompiler {
    fn submit(&mut self, request: RenderSectionCompileRequest) -> anyhow::Result<()> {
        if self.shared.is_none() {
            return Err(anyhow::anyhow!(
                "web render compiler has no shared arena (SharedArrayBuffer unavailable)"
            ));
        }
        if self.in_flight.is_some() {
            return Err(anyhow::anyhow!(
                "web render compiler already has a compile in flight"
            ));
        }
        let request_id = self.allocate_request_id();
        let staged_delta = self.staged_delta.take().ok_or_else(|| {
            anyhow::anyhow!("web render compiler submit called without a staged streaming delta")
        })?;
        let RenderSectionCompileRequest {
            target_sections,
            section_revisions,
            snapshots: upserts,
            biome_zoom_seed,
            topology,
        } = request;

        if staged_delta.reset {
            self.mirror_generation = staged_delta.generation;
        } else if staged_delta.generation != self.mirror_generation {
            return Err(anyhow::anyhow!(
                "web render compiler staged generation {} did not match mirror generation {}",
                staged_delta.generation,
                self.mirror_generation
            ));
        }

        let input_bytes = encode_web_render_compile_delta(
            staged_delta.generation,
            staged_delta.reset,
            biome_zoom_seed,
            topology,
            &upserts,
            &staged_delta.evictions,
        )
        .map_err(|error| anyhow::anyhow!(error))?;
        let arena = self
            .shared
            .as_mut()
            .expect("shared arena present after is_none check");
        let grew = arena
            .arm_for_submit(&input_bytes)
            .map_err(|error| anyhow::anyhow!(error))?;
        if grew {
            self.shared_input_grow_count += 1;
        }

        // Advance the shadow to match what the worker will hold once it applies this delta
        // (apply-on-submit). The worker applies the same upserts/evictions on receipt — before
        // it can fail — so a compile requeued after a result-buffer overflow still finds those
        // columns resident and re-ships nothing. `try_recv_completed` clears the shadow on a
        // worker failure, which forces the next submit back into a full resync.
        for pos in &staged_delta.evictions {
            self.mirror_tracking.remove(pos);
        }
        for snapshot in &upserts {
            self.mirror_tracking.insert(snapshot.pos, snapshot.revision);
        }

        self.last_input_upsert_count = upserts.len();
        debug_assert_eq!(
            self.last_submit_cloned_column_count, self.last_input_upsert_count,
            "web render compiler should clone exactly the upsert columns it submits"
        );
        self.in_flight = Some(WebSharedCompileInFlight::Sections {
            request_id,
            target_sections,
            section_revisions,
        });
        self.pending_jobs = 1;
        self.compile_count += 1;
        Ok(())
    }

    fn try_recv_completed(&mut self) -> anyhow::Result<Vec<RenderSectionCompileResult>> {
        self.poll_completed_work()?;
        Ok(self.completed_sections.drain(..).collect())
    }

    fn pending_job_count(&self) -> usize {
        self.pending_jobs
    }
}

impl FarTerrainLodCompiler for WebRenderSectionCompiler {
    fn available_far_lod_job_slots(&self) -> usize {
        usize::from(self.shared.is_some() && self.in_flight.is_none())
    }

    fn submit_far_lod(&mut self, request: FarTerrainLodBuildRequest) -> anyhow::Result<()> {
        if self.shared.is_none() {
            return Err(anyhow::anyhow!(
                "web render compiler has no shared arena (SharedArrayBuffer unavailable)"
            ));
        }
        if self.in_flight.is_some() {
            return Err(anyhow::anyhow!(
                "web render compiler already has a compile in flight"
            ));
        }
        self.shared
            .as_ref()
            .expect("shared arena present after is_none check")
            .arm_result()
            .map_err(anyhow::Error::msg)?;
        let request_id = self.allocate_request_id();
        self.in_flight = Some(WebSharedCompileInFlight::FarLod {
            request_id,
            request,
        });
        self.pending_jobs = 1;
        self.compile_count += 1;
        Ok(())
    }

    fn try_recv_completed_far_lod(&mut self) -> anyhow::Result<Vec<FarTerrainLodBuildResult>> {
        self.poll_completed_work()?;
        if let Some(error) = self.far_lod_error.take() {
            return Err(anyhow::anyhow!(error));
        }
        Ok(self.completed_far_lod.drain(..).collect())
    }

    fn release_completed_far_lod_jobs(&mut self, count: usize) -> usize {
        count
    }
}

/// Browser implementation of the neutral scene runtime boundary.
///
/// It reuses the resident server worker/WebSocket connection and render
/// compiler. Production browser hosts inject this service into
/// `McloneSceneHost`; the Rust coordinator owns the browser worker lifecycle.
pub struct WebSceneRuntimeService {
    runtime: WebRuntime,
    mesh_assets: TexturedMeshAssets,
    render_compiler: WebRenderSectionCompiler,
    far_lod_cache: FarTerrainLodCache,
    lod_coverage: LodCoverageCoordinator,
    clock: MonotonicClockHandle,
    render_worker: WebRenderWorkerWorldHandle,
    worker_generation: u64,
    deferred_chunk_drops: BoundedDeferredDropQueue,
}

impl std::fmt::Debug for WebSceneRuntimeService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WebSceneRuntimeService")
            .field("runner_kind", &self.runtime.runner_kind())
            .field(
                "compiler_pending",
                &self.render_compiler.pending_job_count(),
            )
            .field("deferred_drop", &self.deferred_chunk_drops.backlog())
            .finish_non_exhaustive()
    }
}

impl WebSceneRuntimeService {
    pub fn new(
        runtime: WebRuntime,
        mesh_assets: TexturedMeshAssets,
        render_worker: WebRenderWorkerCoordinator,
        clock: MonotonicClockHandle,
        world_instance_id: u64,
        priority: RuntimeRenderPriority,
    ) -> Self {
        let render_worker = render_worker.world_handle(world_instance_id, priority);
        let worker_generation = render_worker.active_generation();
        Self {
            runtime,
            mesh_assets,
            render_compiler: WebRenderSectionCompiler::new(),
            far_lod_cache: FarTerrainLodCache::new(),
            lod_coverage: LodCoverageCoordinator::new(),
            clock,
            render_worker,
            worker_generation,
            deferred_chunk_drops: BoundedDeferredDropQueue::new(DEFAULT_DEFERRED_DROP_MAX_ITEMS),
        }
    }

    pub fn into_scene_session_runtime(
        self,
        descriptor: ActiveSessionDescriptor,
    ) -> SceneSessionRuntime {
        SceneSessionRuntime::from_active_service(descriptor, Box::new(self))
    }

    fn host_mode(&self) -> SingleViewHostMode {
        if self.runtime.runner_kind() == ServerRunnerKind::RemoteWebSocket {
            SingleViewHostMode::RemoteDedicated
        } else {
            SingleViewHostMode::LocalIntegrated
        }
    }

    fn handoff_deferred_drops(&mut self) -> (usize, usize) {
        let mut handed_off = 0_usize;
        while let Some((snapshot, item_count)) = self
            .runtime
            .scene_core_mut()
            .take_deferred_client_chunk_drop_snapshot()
        {
            handed_off = handed_off.saturating_add(item_count);
            let _ = self.deferred_chunk_drops.enqueue(snapshot, item_count);
        }
        let _ = self
            .deferred_chunk_drops
            .drain(WEB_DEFERRED_DROP_DRAIN_ITEM_BUDGET);
        (
            handed_off,
            self.deferred_chunk_drops.backlog().pending_items,
        )
    }

    fn sync_render_sections_once(
        &mut self,
        camera_position: glam::Vec3,
        chunk_budget: usize,
        completed_result_accept_budget: Option<usize>,
    ) -> anyhow::Result<RenderSectionCacheUpdate> {
        self.reconcile_render_worker();
        let previous_request = self.render_compiler.in_flight_request_id();
        let compiler = &mut self.render_compiler;
        let update = self
            .runtime
            .scene_core_mut()
            .sync_render_sections_with_budget_and_completed_result_acceptance(
                compiler,
                camera_position,
                chunk_budget,
                completed_result_accept_budget,
                |client, compiler| compiler.stage_streaming_delta(client),
            )?;
        let current_request = self.render_compiler.in_flight_request_id();
        if current_request.is_some() && current_request != previous_request {
            let doorbell = js_sys::Object::new();
            if self
                .render_compiler
                .write_doorbell(&doorbell)
                .map_err(anyhow::Error::msg)?
            {
                self.render_worker
                    .wake(&doorbell)
                    .map_err(anyhow::Error::msg)?;
            }
        }
        Ok(update)
    }

    fn reconcile_render_worker(&mut self) {
        if let Some(request_id) = self.render_compiler.in_flight_request_id()
            && let Some(message) = self.render_worker.take_failure(request_id)
        {
            self.render_compiler.fail_in_flight(message);
        }
        let generation = self.render_worker.active_generation();
        if generation != self.worker_generation {
            self.worker_generation = generation;
            self.render_compiler.reset_worker_mirror();
        }
    }

    fn timed_sync_once(
        &mut self,
        camera_position: glam::Vec3,
        chunk_budget: usize,
        completed_result_accept_budget: Option<usize>,
    ) -> anyhow::Result<TimedRenderSectionCacheUpdate> {
        Ok(TimedRenderSectionCacheUpdate {
            cache_update: self.sync_render_sections_once(
                camera_position,
                chunk_budget,
                completed_result_accept_budget,
            )?,
            timing: Default::default(),
        })
    }

    fn diagnostics(&self) -> mclone_server::ServerRunnerDiagnostics {
        self.runtime.runner_diagnostics()
    }

    fn resolve_lod_coverage(&mut self, normal_drawable: &BTreeSet<ChunkPos>) -> BTreeSet<ChunkPos> {
        let normal_loaded = self
            .runtime
            .client()
            .loaded_chunk_positions()
            .collect::<BTreeSet<_>>();
        let synthetic = self
            .far_lod_cache
            .drawable_lod_tiles()
            .map(LodTileAvailability::synthetic)
            .collect::<Vec<_>>();
        self.lod_coverage
            .resolve(normal_drawable, &normal_loaded, synthetic)
            .visible_lod_tiles
    }
}

impl SceneRuntimeService for WebSceneRuntimeService {
    fn host_mode(&self) -> SingleViewHostMode {
        self.host_mode()
    }

    fn host_label(&self) -> &'static str {
        match self.host_mode() {
            SingleViewHostMode::LocalIntegrated => "web-worker-integrated",
            SingleViewHostMode::RemoteDedicated => "websocket-remote",
        }
    }

    fn set_render_priority(&mut self, priority: RuntimeRenderPriority) {
        self.render_worker.set_priority(priority);
    }

    fn core(&self) -> &mclone_app_runtime::SingleViewRuntime {
        self.runtime.scene_core()
    }

    fn core_mut(&mut self) -> &mut mclone_app_runtime::SingleViewRuntime {
        self.runtime.scene_core_mut()
    }

    fn mesh_assets(&self) -> &TexturedMeshAssets {
        &self.mesh_assets
    }

    fn replace_asset_epoch(
        &mut self,
        epoch: u64,
        mesh_assets: TexturedMeshAssets,
        sections: TexturedRenderSectionBuildReport,
    ) -> anyhow::Result<()> {
        let _ = self
            .runtime
            .scene_core_mut()
            .replace_asset_epoch_sections(epoch, sections);
        self.clear_far_lod();
        self.render_compiler = WebRenderSectionCompiler::new();
        self.worker_generation = self.render_worker.active_generation();
        self.mesh_assets = mesh_assets;
        Ok(())
    }

    fn clear_far_lod(&mut self) {
        let abandoned = self.far_lod_cache.clear();
        self.render_compiler
            .release_completed_far_lod_jobs(abandoned);
        self.lod_coverage.clear();
    }

    fn prepare_far_lod_frame(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        center: ChunkPos,
        camera_position: glam::Vec3,
        build_budget: usize,
        upload_budget: usize,
    ) -> anyhow::Result<Option<&FarTerrainLodFrameUpdate>> {
        self.reconcile_render_worker();
        if !config.enabled {
            self.clear_far_lod();
            return Ok(None);
        }
        let normal_terrain_chunks = self
            .runtime
            .scene_core()
            .traversal_ready_render_section_keys(camera_position)
            .into_iter()
            .map(|section| ChunkPos::new(section.chunk_x, section.chunk_z))
            .collect::<BTreeSet<_>>();
        let previous_request = self.render_compiler.in_flight_request_id();
        self.far_lod_cache.advance_for_camera_at(
            self.clock.now(),
            config,
            seed,
            center,
            self.runtime.scene_core().render_distance(),
            self.mesh_assets.far_lod_materials.as_ref(),
            &mut self.render_compiler,
            build_budget,
        )?;
        let current_request = self.render_compiler.in_flight_request_id();
        if current_request.is_some() && current_request != previous_request {
            let doorbell = js_sys::Object::new();
            if self
                .render_compiler
                .write_doorbell(&doorbell)
                .map_err(anyhow::Error::msg)?
            {
                self.render_worker
                    .wake(&doorbell)
                    .map_err(anyhow::Error::msg)?;
            }
        }
        self.far_lod_cache
            .drain_render_uploads(upload_budget, &mut self.render_compiler);
        let visible = self.resolve_lod_coverage(&normal_terrain_chunks);
        Ok(Some(self.far_lod_cache.prepare_render_update(&visible)))
    }

    fn far_lod_stats(&self) -> mclone_app_runtime::far_lod::FarTerrainLodProducerStats {
        self.far_lod_cache.stats()
    }

    fn far_lod_settle_snapshot(&self, camera_position: glam::Vec3) -> FarLodRuntimeSettleSnapshot {
        FarLodRuntimeSettleSnapshot {
            producer: self.far_lod_cache.settle_snapshot(),
            loaded_chunks: self.runtime.client().loaded_chunk_positions().collect(),
            traversal_ready_sections: self
                .runtime
                .scene_core()
                .traversal_ready_render_section_keys(camera_position),
            suppressed_chunks: self
                .lod_coverage
                .normal_coverage()
                .drawable_chunks()
                .collect(),
        }
    }

    fn lod_coverage_counters(&self) -> mclone_app_runtime::lod_coverage::LodReplacementCounters {
        self.lod_coverage.counters()
    }

    fn release_render_compile_jobs(&mut self, count: usize) -> usize {
        self.render_compiler.release_completed_jobs(count)
    }

    fn simulation_cadence(&self) -> Option<SimulationCadenceConfig> {
        (self.host_mode() == SingleViewHostMode::LocalIntegrated)
            .then(|| self.diagnostics().simulation_cadence)
    }

    fn set_simulation_cadence(&mut self, cadence: SimulationCadenceConfig) -> anyhow::Result<bool> {
        if self.simulation_cadence() == Some(cadence) {
            return Ok(false);
        }
        anyhow::bail!(
            "browser simulation cadence changes require the typed worker operation adapter"
        )
    }

    fn loaded_chunk_count(&self) -> usize {
        self.runtime.client().loaded_chunk_count()
    }

    fn set_chunk_view_with_update_policy_timed(
        &mut self,
        center: ChunkPos,
        render_distance: u32,
        chunk_tracking_radius: u32,
        policy: GameplayCommandUpdatePolicy,
    ) -> anyhow::Result<(bool, GameplayCommandTiming)> {
        let changed = self.runtime.scene_core().interest_center() != center
            || self.runtime.scene_core().render_distance() != render_distance
            || self.runtime.scene_core().chunk_tracking_radius() != chunk_tracking_radius;
        if !changed {
            return Ok((false, GameplayCommandTiming::default()));
        }
        let mut report = self
            .runtime
            .request_chunk_view_deferred(center, render_distance, chunk_tracking_radius)
            .map_err(anyhow::Error::msg)?;
        if policy == GameplayCommandUpdatePolicy::DrainImmediately {
            let drained = self
                .runtime
                .drain_pending_runner_updates_with_budget(RuntimeUpdatePumpBudget::unlimited())
                .map_err(anyhow::Error::msg)?;
            report.update_count = report.update_count.saturating_add(drained.update_count);
        }
        Ok((
            true,
            GameplayCommandTiming {
                updates: report.update_count,
                ..GameplayCommandTiming::default()
            },
        ))
    }

    fn send_gameplay_command_with_update_policy_timed(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> anyhow::Result<GameplayCommandSubmission> {
        let mut report = self
            .runtime
            .send_gameplay_command_deferred(command)
            .map_err(anyhow::Error::msg)?;
        if policy == GameplayCommandUpdatePolicy::DrainImmediately {
            let drained = self
                .runtime
                .drain_pending_runner_updates_with_budget(RuntimeUpdatePumpBudget::unlimited())
                .map_err(anyhow::Error::msg)?;
            report.update_count = report.update_count.saturating_add(drained.update_count);
        }
        Ok(GameplayCommandSubmission {
            timing: GameplayCommandTiming {
                updates: report.update_count,
                ..GameplayCommandTiming::default()
            },
        })
    }

    fn poll_with_update_budget(&mut self, budget: RuntimeUpdatePumpBudget) -> anyhow::Result<bool> {
        let pump = self
            .runtime
            .drain_pending_runner_updates_report(budget)
            .map_err(anyhow::Error::msg)?;
        let (drop_items, drop_backlog) = self.handoff_deferred_drops();
        let runner = self.diagnostics();
        self.runtime.scene_core_mut().finish_poll_diagnostics(
            RuntimePollTiming {
                drain_updates_ms: pump.drain_updates_ms,
                producer_read_ms: pump.producer_read_ms,
                producer_decode_ms: pump.producer_decode_ms,
                producer_inbound_frame_sequence: pump.producer_inbound_frame_sequence,
                client_deferred_chunk_drop_items: drop_items,
                client_deferred_chunk_drop_backlog_items: drop_backlog,
                update_pump_stalled: pump.stalled,
                update_pump_stall_count: pump.stall_count,
                server_update_queue_depth: pump.remaining_queue_depth,
                server_update_queue_bytes: pump.remaining_queue_bytes,
                server_update_applied_bytes: pump.update_bytes,
                server_update_oldest_applied_age_ms: pump.oldest_applied_update_age_ms,
                diagnostics_refreshed: true,
                ..RuntimePollTiming::default()
            },
            pump.apply_report,
            Some(&runner),
        );
        Ok(pump.apply_report.changed)
    }

    fn poll_until_idle_with_timeout(
        &mut self,
        _timeout: std::time::Duration,
    ) -> anyhow::Result<(usize, f64)> {
        let _ = self.poll_with_update_budget(RuntimeUpdatePumpBudget::unlimited())?;
        let diagnostics = self.diagnostics();
        if diagnostics.command_queue_depth == 0
            && diagnostics.update_queue_depth == 0
            && diagnostics.pending_jobs == 0
            && diagnostics.pending_publications == 0
            && self.render_compiler.pending_job_count() == 0
            && self.deferred_chunk_drops.backlog().pending_items == 0
        {
            Ok((1, 0.0))
        } else {
            anyhow::bail!(
                "browser runtime is not idle; continue stepping from requestAnimationFrame"
            )
        }
    }

    fn sync_render_sections_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: glam::Vec3,
        completed_result_accept_budget: Option<usize>,
    ) -> anyhow::Result<TimedRenderSectionCacheUpdate> {
        self.timed_sync_once(
            camera_position,
            WEB_RENDER_CHUNK_MESH_BUDGET,
            completed_result_accept_budget,
        )
    }

    fn sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
        &mut self,
        camera_position: glam::Vec3,
        deadline: MonotonicDeadline,
        completed_result_accept_budget: Option<usize>,
    ) -> anyhow::Result<TimedRenderSectionCacheUpdate> {
        if deadline.is_reached() {
            return Ok(TimedRenderSectionCacheUpdate::default());
        }
        self.sync_render_sections_with_completed_result_acceptance_timed(
            camera_position,
            completed_result_accept_budget,
        )
    }

    fn sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
        &mut self,
        camera_position: glam::Vec3,
        deadline: MonotonicDeadline,
        max_compile_requests: usize,
        completed_result_accept_budget: Option<usize>,
    ) -> anyhow::Result<TimedRenderSectionCacheUpdate> {
        if deadline.is_reached() || max_compile_requests == 0 {
            return Ok(TimedRenderSectionCacheUpdate::default());
        }
        self.sync_render_sections_with_completed_result_acceptance_timed(
            camera_position,
            completed_result_accept_budget,
        )
    }

    fn sync_all_render_sections(
        &mut self,
        camera_position: glam::Vec3,
    ) -> anyhow::Result<RenderSectionCacheUpdate> {
        self.sync_render_sections_once(camera_position, usize::MAX, None)
    }

    fn resident_section_metadata(&self) -> Vec<TexturedRenderSectionMetadata> {
        self.runtime.scene_core().resident_section_metadata()
    }

    fn cached_section_count(&self) -> usize {
        self.runtime.scene_core().cached_section_count()
    }

    fn mark_all_render_sections_dirty_for_resource_rebuild(&mut self) -> usize {
        self.runtime
            .scene_core_mut()
            .mark_all_render_sections_dirty_for_resource_rebuild()
    }

    fn recompile_all_render_section_meshes_for_resource_rebuild(
        &mut self,
        camera_position: glam::Vec3,
    ) -> anyhow::Result<Vec<TexturedRenderSectionMesh>> {
        self.mark_all_render_sections_dirty_for_resource_rebuild();
        Ok(self
            .sync_render_sections_once(camera_position, usize::MAX, None)?
            .rebuilt_sections)
    }

    fn traversal_ready_render_section_keys(
        &self,
        camera_position: glam::Vec3,
    ) -> BTreeSet<RenderSectionKey> {
        self.runtime
            .scene_core()
            .traversal_ready_render_section_keys(camera_position)
    }

    fn sky_clear_color(&self) -> wgpu::Color {
        mclone_render::sky::overworld_clear_color(self.runtime.scene_core().time_of_day())
    }

    fn render_compile_pending_job_count(&self) -> usize {
        self.render_compiler.pending_job_count()
    }

    fn render_compile_max_pending_job_count(&self) -> usize {
        self.render_compiler.max_pending_job_count()
    }

    fn render_compile_available_pending_job_slots(&self) -> usize {
        self.render_compiler.available_pending_job_slots()
    }

    fn render_compile_queue_health(&self) -> RenderSectionCompileQueueHealth {
        RenderSectionCompileQueueHealth::from_compiler(&self.render_compiler)
    }

    fn view_readiness_overlay(&self) -> Option<LoadingProgressOverlay> {
        view_readiness_overlay_from_diagnostics(&self.diagnostics())
    }

    fn flush_persistence(&mut self) -> anyhow::Result<usize> {
        self.runtime.flush_persistence().map_err(anyhow::Error::msg)
    }

    fn promote_observer_to_player(&mut self) -> anyhow::Result<()> {
        self.runtime
            .promote_observer_to_player()
            .map_err(anyhow::Error::msg)
    }

    fn demote_player_to_observer(&mut self) -> anyhow::Result<()> {
        self.runtime
            .demote_player_to_observer()
            .map_err(anyhow::Error::msg)
    }

    fn refresh_startup_diagnostics(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn startup_progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        let diagnostics = self.diagnostics();
        loading_progress_overlay_from_diagnostics(&diagnostics)
            .or_else(|| view_readiness_overlay_from_diagnostics(&diagnostics))
    }

    fn startup_host_ready(
        &self,
        policy: StartupReadinessPolicy,
        _camera_position: glam::Vec3,
    ) -> bool {
        let diagnostics = self.diagnostics();
        let interest_center = self.runtime.scene_core().interest_center();
        let interest_chunk_ready = self
            .runtime
            .client()
            .chunk_snapshot(interest_center)
            .is_some();
        let host_ready = match self.host_mode() {
            SingleViewHostMode::LocalIntegrated => {
                interest_chunk_ready
                    && diagnostics
                        .view_readiness_snapshot
                        .as_ref()
                        .is_none_or(|snapshot| {
                            snapshot.stats.playable_chunk_ready
                                && self
                                    .runtime
                                    .client()
                                    .chunk_snapshot(snapshot.stats.playable_chunk)
                                    .is_some()
                        })
            }
            SingleViewHostMode::RemoteDedicated => {
                interest_chunk_ready
                    && diagnostics.command_queue_depth == 0
                    && diagnostics.update_queue_depth == 0
            }
        };
        match policy {
            StartupReadinessPolicy::Playable => host_ready,
            StartupReadinessPolicy::Idle => {
                host_ready
                    && diagnostics.command_queue_depth == 0
                    && diagnostics.update_queue_depth == 0
                    && diagnostics.pending_jobs == 0
                    && diagnostics.pending_publications == 0
                    && self.render_compiler.pending_job_count() == 0
            }
        }
    }

    fn stats(&self) -> mclone_app_runtime::SingleViewRuntimeStats {
        self.runtime.scene_core().stats(
            self.host_mode(),
            Some(&self.diagnostics()),
            self.render_compiler.pending_job_count(),
        )
    }
}

async fn render_canvas_clear(canvas: HtmlCanvasElement) -> Result<CanvasRenderReport, String> {
    let context = WebCanvasContext::new(canvas).await?;
    let frame = context
        .surface
        .get_current_texture()
        .map_err(|error| format!("failed to acquire WebGPU canvas frame: {error}"))?;
    let view = frame.texture.create_view(&Default::default());
    let mut encoder = context
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_web_canvas_clear_encoder"),
        });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_web_canvas_clear_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(mclone_render::default_clear_color()),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
    }
    context.queue.submit(std::iter::once(encoder.finish()));
    frame.present();

    Ok(CanvasRenderReport {
        width: context.width,
        height: context.height,
    })
}

#[wasm_bindgen(js_name = mclone_web_catalog_validate_world_id)]
pub fn mclone_web_catalog_validate_world_id(id: String) -> Result<String, JsValue> {
    LocalWorldId::new(id)
        .map(LocalWorldId::into_string)
        .map_err(|error| JsValue::from(error.message))
}

#[wasm_bindgen(js_name = mclone_web_catalog_prepare_world_list)]
pub fn mclone_web_catalog_prepare_world_list(records: JsValue) -> Result<JsValue, JsValue> {
    let mut worlds = decode_web_local_world_summaries(&records).map_err(JsValue::from)?;
    sort_local_world_summaries(&mut worlds);
    encode_web_local_world_summaries(&worlds).map_err(JsValue::from)
}

#[wasm_bindgen(js_name = mclone_web_catalog_prepare_create_world)]
pub fn mclone_web_catalog_prepare_create_world(
    options: JsValue,
    existing_records: JsValue,
    now_unix_millis: f64,
) -> Result<JsValue, JsValue> {
    let options = decode_web_local_world_create_options(&options).map_err(JsValue::from)?;
    let existing = decode_web_local_world_summaries(&existing_records).map_err(JsValue::from)?;
    let id = options
        .resolve_id(existing.iter().map(|world| &world.id))
        .map_err(|error| JsValue::from(error.message))?;
    if existing.iter().any(|world| world.id == id) {
        return Err(JsValue::from(duplicate_world_id(&id).message));
    }

    let now = parse_web_unix_millis(now_unix_millis, "nowUnixMillis").map_err(JsValue::from)?;
    let mut summary = LocalWorldSummary::new(id, options.display_name, options.seed, now)
        .map_err(|error| JsValue::from(error.message))?;
    summary.world_generation_profile = options.world_generation_profile;
    summary.last_played_unix_millis = Some(now);
    summary.backend_label = Some(WEB_WORLD_BACKEND_LABEL.to_owned());
    encode_web_catalog_mutation(&summary).map_err(JsValue::from)
}

#[wasm_bindgen(js_name = mclone_web_catalog_prepare_open_world)]
pub fn mclone_web_catalog_prepare_open_world(
    id: String,
    record: JsValue,
    now_unix_millis: f64,
) -> Result<JsValue, JsValue> {
    let id = LocalWorldId::new(id).map_err(|error| JsValue::from(error.message))?;
    let mut summary =
        decode_required_web_local_world_summary(&record, &id).map_err(JsValue::from)?;
    validate_local_world_compatible(&summary).map_err(|error| JsValue::from(error.message))?;
    summary.last_played_unix_millis =
        Some(parse_web_unix_millis(now_unix_millis, "nowUnixMillis").map_err(JsValue::from)?);
    encode_web_catalog_mutation(&summary).map_err(JsValue::from)
}

#[wasm_bindgen(js_name = mclone_web_catalog_prepare_record_world_played)]
pub fn mclone_web_catalog_prepare_record_world_played(
    id: String,
    record: JsValue,
    now_unix_millis: f64,
) -> Result<JsValue, JsValue> {
    // Recording active play has the same metadata validation and timestamp
    // transition as an ordinary open, but the IndexedDB adapter never opens a
    // runtime or world store for this operation.
    mclone_web_catalog_prepare_open_world(id, record, now_unix_millis)
}

#[wasm_bindgen(js_name = mclone_web_catalog_prepare_delete_world)]
pub fn mclone_web_catalog_prepare_delete_world(
    id: String,
    active_world_id: String,
    record: JsValue,
) -> Result<JsValue, JsValue> {
    let id = LocalWorldId::new(id).map_err(|error| JsValue::from(error.message))?;
    let active_world = if active_world_id.is_empty() {
        None
    } else {
        Some(LocalWorldId::new(active_world_id).map_err(|error| JsValue::from(error.message))?)
    };
    validate_delete_inactive_world(&id, active_world.as_ref())
        .map_err(|error| JsValue::from(error.message))?;
    let summary = decode_required_web_local_world_summary(&record, &id).map_err(JsValue::from)?;
    encode_web_local_world_summary(&summary).map_err(JsValue::from)
}

/// Produce one storage-neutral managed-world publication payload. The browser
/// adapter only carries these opaque Rust-authored bytes into IndexedDB.
#[wasm_bindgen(js_name = mclone_web_managed_scenario_prepare_world)]
pub fn mclone_web_managed_scenario_prepare_world(
    scenario_id: String,
    role: String,
) -> Result<JsValue, JsValue> {
    let scenario_id = parse_web_managed_scenario_id(&scenario_id).map_err(JsValue::from)?;
    let role = parse_web_managed_world_role(&role).map_err(JsValue::from)?;
    let manifest = match scenario_id {
        BuiltInScenarioId::LobbyPreview => ManagedScenarioManifest::current_lobby_preview(),
    };
    let payload = managed_scenario_world_payload(&manifest, role)
        .map_err(|error| JsValue::from(error.to_string()))?;
    encode_web_managed_scenario_payload(&payload).map_err(JsValue::from)
}

/// Apply the shared missing/partial/incompatible/corrupt/valid vocabulary to
/// records read by the thin IndexedDB adapter.
#[wasm_bindgen(js_name = mclone_web_managed_scenario_validate_world)]
pub fn mclone_web_managed_scenario_validate_world(
    scenario_id: String,
    role: String,
    metadata: JsValue,
    chunk_records: JsValue,
    entity_chunk_records: JsValue,
) -> Result<JsValue, JsValue> {
    let scenario_id = parse_web_managed_scenario_id(&scenario_id).map_err(JsValue::from)?;
    let role = parse_web_managed_world_role(&role).map_err(JsValue::from)?;
    let manifest = match scenario_id {
        BuiltInScenarioId::LobbyPreview => ManagedScenarioManifest::current_lobby_preview(),
    };
    let payload = managed_scenario_world_payload(&manifest, role)
        .map_err(|error| JsValue::from(error.to_string()))?;
    let metadata = decode_web_managed_scenario_metadata(&metadata).map_err(JsValue::from)?;
    let chunk_records =
        decode_web_managed_scenario_records(&chunk_records).map_err(JsValue::from)?;
    let entity_chunk_records =
        decode_web_managed_scenario_records(&entity_chunk_records).map_err(JsValue::from)?;
    let validation = validate_managed_scenario_stored_world(
        &payload,
        metadata.as_ref(),
        &chunk_records,
        &entity_chunk_records,
    );
    let result = js_sys::Object::new();
    set_string(&result, "status", validation.status.label()).map_err(JsValue::from)?;
    set_string(&result, "detail", &validation.detail).map_err(JsValue::from)?;
    Ok(result.into())
}

fn parse_web_managed_scenario_id(value: &str) -> Result<BuiltInScenarioId, String> {
    match value {
        "lobbyPreview" => Ok(BuiltInScenarioId::LobbyPreview),
        _ => Err(format!("unsupported managed scenario id `{value}`")),
    }
}

fn parse_web_managed_world_role(value: &str) -> Result<ManagedScenarioWorldRole, String> {
    match value {
        "primary" => Ok(ManagedScenarioWorldRole::Primary),
        "destination" => Ok(ManagedScenarioWorldRole::Destination),
        _ => Err(format!("unsupported managed world role `{value}`")),
    }
}

fn web_managed_world_role_label(role: ManagedScenarioWorldRole) -> &'static str {
    match role {
        ManagedScenarioWorldRole::Primary => "primary",
        ManagedScenarioWorldRole::Destination => "destination",
    }
}

fn encode_web_managed_scenario_payload(
    payload: &mclone_app_runtime::scenario_content::ManagedScenarioWorldPayload,
) -> Result<JsValue, String> {
    let result = js_sys::Object::new();
    let metadata = ManagedScenarioStoredWorldMetadata::for_payload(payload);
    let metadata_object = js_sys::Object::new();
    set_string(&metadata_object, "worldId", metadata.key.as_str())?;
    set_number(
        &metadata_object,
        "schemaVersion",
        f64::from(metadata.schema_version),
    )?;
    set_string(
        &metadata_object,
        "role",
        web_managed_world_role_label(metadata.role),
    )?;
    set_string(&metadata_object, "contentId", &metadata.content_id)?;
    set_number(
        &metadata_object,
        "contentVersion",
        f64::from(metadata.content_version),
    )?;
    set_string(
        &metadata_object,
        "authoredPayloadFingerprint",
        &metadata.authored_payload_fingerprint.to_string(),
    )?;

    let chunk_records = js_sys::Array::new();
    for record in &payload.chunk_records {
        let object = js_sys::Object::new();
        set_number(&object, "x", f64::from(record.chunk_x))?;
        set_number(&object, "z", f64::from(record.chunk_z))?;
        js_sys::Reflect::set(
            &object,
            &JsValue::from_str("record"),
            js_sys::Uint8Array::from(record.bytes.as_slice()).as_ref(),
        )
        .map_err(|error| format!("failed to attach managed chunk bytes: {error:?}"))?;
        chunk_records.push(&object);
    }

    set_string(&result, "worldId", payload.key.as_str())?;
    set_string(&result, "role", web_managed_world_role_label(payload.role))?;
    js_sys::Reflect::set(&result, &JsValue::from_str("metadata"), &metadata_object)
        .map_err(|error| format!("failed to attach managed metadata: {error:?}"))?;
    js_sys::Reflect::set(&result, &JsValue::from_str("chunks"), &chunk_records)
        .map_err(|error| format!("failed to attach managed chunks: {error:?}"))?;
    let entity_chunk_records = js_sys::Array::new();
    for record in &payload.entity_chunk_records {
        let object = js_sys::Object::new();
        set_number(&object, "x", f64::from(record.chunk_x))?;
        set_number(&object, "z", f64::from(record.chunk_z))?;
        js_sys::Reflect::set(
            &object,
            &JsValue::from_str("record"),
            js_sys::Uint8Array::from(record.bytes.as_slice()).as_ref(),
        )
        .map_err(|error| format!("failed to attach managed entity chunk bytes: {error:?}"))?;
        entity_chunk_records.push(&object);
    }
    js_sys::Reflect::set(
        &result,
        &JsValue::from_str("entityChunks"),
        &entity_chunk_records,
    )
    .map_err(|error| format!("failed to attach managed entity chunks: {error:?}"))?;
    Ok(result.into())
}

fn decode_web_managed_scenario_metadata(
    value: &JsValue,
) -> Result<Option<ManagedScenarioStoredWorldMetadata>, String> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    let schema_version = u32::try_from(js_u64_property(value, "schemaVersion")?)
        .map_err(|_| "managed schemaVersion must fit u32".to_owned())?;
    let role = parse_web_managed_world_role(&js_string_property(value, "role")?)?;
    let key = ManagedWorldKey::new(js_string_property(value, "worldId")?)
        .map_err(|error| error.to_string())?;
    let content_version = u32::try_from(js_u64_property(value, "contentVersion")?)
        .map_err(|_| "managed contentVersion must fit u32".to_owned())?;
    let authored_payload_fingerprint = js_string_property(value, "authoredPayloadFingerprint")?
        .parse::<u64>()
        .map_err(|_| "managed authoredPayloadFingerprint must be a u64 string".to_owned())?;
    Ok(Some(ManagedScenarioStoredWorldMetadata {
        schema_version,
        role,
        key,
        content_id: js_string_property(value, "contentId")?,
        content_version,
        authored_payload_fingerprint,
    }))
}

fn decode_web_managed_scenario_records(
    value: &JsValue,
) -> Result<Vec<ManagedScenarioStoredRecord>, String> {
    if !js_sys::Array::is_array(value) {
        return Err("managed world records must be an array".to_owned());
    }
    let array = js_sys::Array::from(value);
    array
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let chunk_x = i32::try_from(js_i64_property(&value, "x")?)
                .map_err(|_| format!("managed record {index} x must fit i32"))?;
            let chunk_z = i32::try_from(js_i64_property(&value, "z")?)
                .map_err(|_| format!("managed record {index} z must fit i32"))?;
            let bytes = js_sys::Uint8Array::new(&js_property(&value, "record")?).to_vec();
            Ok(ManagedScenarioStoredRecord {
                chunk_x,
                chunk_z,
                bytes,
            })
        })
        .collect()
}

fn write_target_sections(
    object: &js_sys::Object,
    target_sections: &BTreeSet<RenderSectionKey>,
) -> Result<(), String> {
    let flat = target_sections
        .iter()
        .flat_map(|key| [key.chunk_x, key.section_y, key.chunk_z])
        .collect::<Vec<_>>();
    let array = js_sys::Int32Array::from(flat.as_slice());
    js_sys::Reflect::set(object, &JsValue::from_str("targetSections"), array.as_ref())
        .map_err(|error| format!("failed to set targetSections: {error:?}"))?;
    Ok(())
}

pub(crate) fn render_section_keys_from_int32_array(
    target_sections: &js_sys::Int32Array,
) -> Result<BTreeSet<RenderSectionKey>, String> {
    let values = target_sections.to_vec();
    if values.len() % 3 != 0 {
        return Err(format!(
            "targetSections length {} is not a multiple of 3",
            values.len()
        ));
    }
    Ok(values
        .chunks_exact(3)
        .map(|chunk| RenderSectionKey::new(chunk[0], chunk[1], chunk[2]))
        .collect())
}

#[derive(Clone, Debug)]
struct WebTexturedMeshAssets {
    catalog: TexturedMeshCatalog,
    far_lod_materials: Option<FarTerrainLodMaterialPalette>,
    asset_pack_file_count: usize,
}

fn load_textured_mesh_assets_from_pack(bytes: Vec<u8>) -> Result<WebTexturedMeshAssets, String> {
    let source = PackedAssetSource::from_bytes(bytes)
        .map_err(|error| format!("failed to parse packed Minecraft assets: {error}"))?;
    let asset_pack_file_count = source.file_count();
    let assets = load_textured_terrain_assets(&source)
        .map_err(|error| format!("failed to load packed textured terrain assets: {error}"))?;
    let far_lod_materials = FarTerrainLodMaterialPalette::load_from_asset_source(&source)
        .map_err(|error| format!("failed to load packed far-LOD assets: {error}"))?;

    Ok(WebTexturedMeshAssets {
        catalog: assets.catalog,
        far_lod_materials,
        asset_pack_file_count,
    })
}

/// CPU-side browser asset preparation for the neutral scene-host seam.
///
/// The caller runs this in the existing asset/worker lifecycle before handing
/// the result to `McloneSceneHost::with_scene_runtime`. GPU upload remains in
/// the browser driver, and browser audio stays explicitly absent.
pub fn prepare_web_scene_assets_from_pack(bytes: Vec<u8>) -> Result<PreparedSceneAssets, String> {
    let source = PackedAssetSource::from_bytes(bytes)
        .map_err(|error| format!("failed to parse packed scene assets: {error}"))?;
    let terrain = load_textured_terrain_assets(&source)
        .map_err(|error| format!("failed to prepare browser terrain assets: {error}"))?;
    let far_lod_materials = FarTerrainLodMaterialPalette::load_from_asset_source(&source)
        .map_err(|error| format!("failed to prepare browser far-LOD assets: {error}"))?;
    let actors = load_actor_texture_assets(&source)
        .map_err(|error| format!("failed to prepare browser actor assets: {error}"))?;
    let screen_effects = load_screen_effect_texture_assets(&source)
        .map_err(|error| format!("failed to prepare browser screen-effect assets: {error}"))?;
    Ok(PreparedSceneAssets::startup(
        0,
        TexturedMeshAssets {
            catalog: terrain.catalog,
            atlas: terrain.atlas.into(),
            far_lod_materials,
        },
        actors,
        screen_effects,
        PreparedAudioAssets::silent(),
    ))
}

pub fn prepare_web_scene_assets_from_selection(
    epoch: u64,
    authored_bytes: Vec<u8>,
    reference_bytes: Vec<u8>,
    fallback_bytes: Vec<u8>,
    authored_enabled: bool,
    reference_enabled: bool,
) -> Result<PreparedSceneAssets, String> {
    let authored = PackedAssetSource::from_bytes(authored_bytes)
        .map_err(|error| format!("failed to parse authored browser pack: {error}"))?;
    let reference = PackedAssetSource::from_bytes(reference_bytes)
        .map_err(|error| format!("failed to parse reference browser pack: {error}"))?;
    let fallback = PackedAssetSource::from_bytes(fallback_bytes)
        .map_err(|error| format!("failed to parse generated browser pack: {error}"))?;
    let registry = AssetPackSourceRegistry::from_packed_with_reference(
        Some(authored),
        fallback,
        SharedAssetSource::new(reference),
    )
    .map_err(|error| format!("failed to compose browser asset catalog: {error:#}"))?;
    registry
        .prepare(
            epoch,
            web_asset_pack_selection(authored_enabled, reference_enabled),
        )
        .map_err(|error| format!("failed to prepare browser asset selection: {error:#}"))
}

pub fn web_asset_pack_catalog(
    authored_bytes: Vec<u8>,
    reference_bytes: Vec<u8>,
    fallback_bytes: Vec<u8>,
) -> Result<mclone_assets::AssetPackCatalog, String> {
    let authored = PackedAssetSource::from_bytes(authored_bytes)
        .map_err(|error| format!("failed to parse authored browser pack: {error}"))?;
    let reference = PackedAssetSource::from_bytes(reference_bytes)
        .map_err(|error| format!("failed to parse reference browser pack: {error}"))?;
    let fallback = PackedAssetSource::from_bytes(fallback_bytes)
        .map_err(|error| format!("failed to parse generated browser pack: {error}"))?;
    AssetPackSourceRegistry::from_packed_with_reference(
        Some(authored),
        fallback,
        SharedAssetSource::new(reference),
    )
    .map(|registry| registry.catalog().clone())
    .map_err(|error| format!("failed to compose browser asset catalog: {error:#}"))
}

fn load_selected_web_mesh_assets(
    authored_bytes: Vec<u8>,
    reference_bytes: Vec<u8>,
    fallback_bytes: Vec<u8>,
    authored_enabled: bool,
    reference_enabled: bool,
) -> Result<(TexturedMeshAssets, usize), String> {
    let file_count = [
        authored_enabled.then_some(&authored_bytes),
        reference_enabled.then_some(&reference_bytes),
        Some(&fallback_bytes),
    ]
    .into_iter()
    .flatten()
    .map(|bytes| {
        PackedAssetSource::from_bytes(bytes.clone())
            .map(|source| source.file_count())
            .map_err(|error| format!("failed to count selected browser pack: {error}"))
    })
    .collect::<Result<Vec<_>, _>>()?
    .into_iter()
    .sum();
    let assets = prepare_web_scene_assets_from_selection(
        1,
        authored_bytes,
        reference_bytes,
        fallback_bytes,
        authored_enabled,
        reference_enabled,
    )?;
    Ok((assets.mesh, file_count))
}

fn web_asset_pack_selection(authored_enabled: bool, reference_enabled: bool) -> AssetPackSelection {
    let mut enabled = Vec::new();
    if authored_enabled {
        enabled.push(AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID));
    }
    if reference_enabled {
        enabled.push(AssetPackId::new(MINECRAFT_REFERENCE_PACK_ID));
    }
    AssetPackSelection::new(enabled)
}

pub(super) fn gui_key_from_label(label: &str) -> Option<GuiKey> {
    match label {
        "escape" | "Escape" => Some(GuiKey::Escape),
        "f1" | "F1" => Some(GuiKey::F1),
        _ => None,
    }
}

pub(super) fn ui_action_label(action: GameUiAction) -> &'static str {
    match action {
        GameUiAction::StartWorld => "startWorld",
        GameUiAction::EnterScenario(_) => "enterScenario",
        GameUiAction::OpenWorldList => "openWorldList",
        GameUiAction::OpenWorldCreate => "openWorldCreate",
        GameUiAction::SelectWorld(_) => "selectWorld",
        GameUiAction::OpenWorld(_) => "openWorld",
        GameUiAction::CreateCatalogWorld => "createCatalogWorld",
        GameUiAction::ConfirmDeleteWorld(_) => "confirmDeleteWorld",
        GameUiAction::DeleteWorld(_) => "deleteWorld",
        GameUiAction::CancelDeleteWorld => "cancelDeleteWorld",
        GameUiAction::OpenBlockPalette => "openBlockPalette",
        GameUiAction::OpenHelp(_) => "openHelp",
        GameUiAction::CloseHelp(_) => "closeHelp",
        GameUiAction::OpenNewWorld => "openNewWorld",
        GameUiAction::OpenJoinRemote => "openJoinRemote",
        GameUiAction::RerollSeed => "rerollSeed",
        GameUiAction::CycleWorldGenerationProfile => "cycleWorldGenerationProfile",
        GameUiAction::CreateWorld(_) => "createWorld",
        GameUiAction::JoinRemote => "joinRemote",
        GameUiAction::Resume => "resume",
        GameUiAction::OpenOptions(_) => "openOptions",
        GameUiAction::OpenOptionsCategory(_, _) => "openOptionsCategory",
        GameUiAction::OpenServerSettings(_) => "openServerSettings",
        GameUiAction::OpenAssetPacks(_) => "openAssetPacks",
        GameUiAction::ToggleAssetPack(_) => "toggleAssetPack",
        GameUiAction::ApplyAssetPacks => "applyAssetPacks",
        GameUiAction::CancelAssetPacks => "cancelAssetPacks",
        GameUiAction::ConfirmStorageAction(_, _) => "confirmStorageAction",
        GameUiAction::ExecuteStorageAction(_, _) => "executeStorageAction",
        GameUiAction::CancelStorageAction(_) => "cancelStorageAction",
        GameUiAction::ClearRebuildableCache => "clearRebuildableCache",
        GameUiAction::BackToTitle => "backToTitle",
        GameUiAction::BackToPause => "backToPause",
        GameUiAction::QuitToTitle => "quitToTitle",
        GameUiAction::Respawn => "respawn",
        GameUiAction::ToggleSectionOcclusion => "toggleSectionOcclusion",
        GameUiAction::ToggleFullbright => "toggleFullbright",
        GameUiAction::ToggleFarLod => "toggleFarLod",
        GameUiAction::CycleFarLodDetail => "cycleFarLodDetail",
        GameUiAction::SetFarLodRange(_) => "setFarLodRange",
        GameUiAction::TogglePlayerCollisionBox => "togglePlayerCollisionBox",
        GameUiAction::ToggleCrosshair => "toggleCrosshair",
        GameUiAction::ToggleFirstPersonPlayer => "toggleFirstPersonPlayer",
        GameUiAction::ToggleFramePipelineOverlay => "toggleFramePipelineOverlay",
        GameUiAction::ToggleDebugDiagnostics => "toggleDebugDiagnostics",
        GameUiAction::SetPlayerModel(_) => "setPlayerModel",
        GameUiAction::SetMovementMode(_) => "setMovementMode",
        GameUiAction::SetCollisionMode(_) => "setCollisionMode",
        GameUiAction::SetTravelAssistMode(_) => "setTravelAssistMode",
        GameUiAction::SetTurnMode(_) => "setTurnMode",
        GameUiAction::SetXrTurnMode(_) => "setXrTurnMode",
        GameUiAction::CycleFramePacing => "cycleFramePacing",
        GameUiAction::CycleFpsCap => "cycleFpsCap",
        GameUiAction::SetRenderDistance(_) => "setRenderDistance",
        GameUiAction::SetFlySpeed(_) => "setFlySpeed",
        GameUiAction::SetMovementSpeed(_) => "setMovementSpeed",
        GameUiAction::SetTouchLookSensitivity(_) => "setTouchLookSensitivity",
        GameUiAction::SetTouchControlsMode(_) => "setTouchControlsMode",
        GameUiAction::SetServerSimulationCadence(_) => "setServerSimulationCadence",
        GameUiAction::AssignHotbarBlock { .. } => "assignHotbarBlock",
        GameUiAction::AssignHotbarActor { .. } => "assignHotbarActor",
        GameUiAction::Quit => "quit",
    }
}

pub(super) fn decode_world_catalog_response(
    operation: &str,
    payload: &JsValue,
) -> Result<WorldCatalogResponse, String> {
    match operation {
        "listWorlds" => Ok(WorldCatalogResponse::WorldList {
            capabilities: WorldCatalogCapabilities::persistent_local(),
            worlds: {
                let mut worlds = decode_web_local_world_summaries(payload)?;
                sort_local_world_summaries(&mut worlds);
                worlds
            },
        }),
        "createWorld" => Ok(WorldCatalogResponse::WorldCreated {
            summary: decode_web_local_world_summary(payload)?,
        }),
        "openWorld" => Ok(WorldCatalogResponse::WorldOpened {
            summary: decode_web_local_world_summary(payload)?,
        }),
        "recordWorldPlayed" => Ok(WorldCatalogResponse::WorldPlayRecorded {
            summary: decode_web_local_world_summary(payload)?,
        }),
        "deleteWorld" => Ok(WorldCatalogResponse::WorldDeleted {
            id: LocalWorldId::new(js_string_property(payload, "id")?)
                .map_err(|error| error.message)?,
        }),
        "deleteAllLocalWorlds" | "factoryResetLocalData" => {
            Ok(WorldCatalogResponse::AllLocalWorldsDeleted {
                deleted_count: js_u64_property(payload, "deletedCount")? as usize,
            })
        }
        other => Err(format!(
            "unsupported world catalog response operation {other:?}"
        )),
    }
}

fn decode_web_local_world_create_options(
    value: &JsValue,
) -> Result<LocalWorldCreateOptions, String> {
    let mut options = LocalWorldCreateOptions::new(
        js_string_property(value, "displayName")?,
        js_i64_property(value, "seed")?,
    )
    .map_err(|error| error.message)?;
    if let Some(requested_id) = js_optional_string_property(value, "requestedId")?
        && !requested_id.is_empty()
    {
        options = options
            .with_requested_id(LocalWorldId::new(requested_id).map_err(|error| error.message)?);
    }
    if let Some(profile) = js_optional_string_property(value, "generationProfile")? {
        options = options.with_world_generation_profile(
            WorldGenerationProfile::parse_label(&profile).map_err(|error| error.to_string())?,
        );
    }
    Ok(options)
}

fn decode_web_local_world_summaries(value: &JsValue) -> Result<Vec<LocalWorldSummary>, String> {
    if !js_sys::Array::is_array(value) {
        return Err("world catalog list response must be an array".to_owned());
    }
    js_sys::Array::from(value)
        .iter()
        .enumerate()
        .map(|(index, value)| {
            decode_web_local_world_summary(&value)
                .map_err(|error| format!("world catalog list entry {index}: {error}"))
        })
        .collect()
}

fn decode_required_web_local_world_summary(
    value: &JsValue,
    id: &LocalWorldId,
) -> Result<LocalWorldSummary, String> {
    if value.is_null() || value.is_undefined() {
        return Err(world_not_found(id).message);
    }
    let summary = decode_web_local_world_summary(value)?;
    if &summary.id != id {
        return Err(WorldCatalogError::new(
            WorldCatalogErrorKind::StorageFailure,
            format!("local world record `{}` identified `{}`", id, summary.id),
        )
        .message);
    }
    Ok(summary)
}

fn decode_web_local_world_summary(value: &JsValue) -> Result<LocalWorldSummary, String> {
    let descriptor = js_property(value, "descriptor")?;
    if !descriptor.is_null() && !descriptor.is_undefined() {
        if !descriptor.is_instance_of::<js_sys::Uint8Array>() {
            return Err("world catalog descriptor must be a Uint8Array".to_owned());
        }
        let summary =
            web_world_catalog_descriptor::decode(&js_sys::Uint8Array::new(&descriptor).to_vec())?;
        let envelope_id =
            LocalWorldId::new(js_string_property(value, "id")?).map_err(|error| error.message)?;
        if summary.id != envelope_id {
            return Err(format!(
                "world catalog descriptor `{}` was stored under envelope id `{envelope_id}`",
                summary.id
            ));
        }
        return Ok(summary);
    }

    let id = LocalWorldId::new(js_string_property(value, "id")?).map_err(|error| error.message)?;
    let mut summary = LocalWorldSummary::new(
        id,
        js_string_property(value, "displayName")?,
        js_i64_text_or_number_property(value, "seedText", "seed")?,
        js_u64_text_or_number_property(value, "createdUnixMillisText", "createdUnixMillis")?,
    )
    .map_err(|error| error.message)?;
    summary.world_generation_profile = js_optional_string_property(value, "generationProfile")?
        .map(|profile| WorldGenerationProfile::parse_label(&profile))
        .transpose()?
        .unwrap_or_default();
    summary.last_played_unix_millis = js_optional_u64_text_or_number_property(
        value,
        "lastPlayedUnixMillisText",
        "lastPlayedUnixMillis",
    )?;
    summary.storage_schema_version =
        js_optional_u32_property(value, "storageSchemaVersion")?.unwrap_or(0);
    summary.target_minecraft_version =
        js_optional_string_property(value, "targetMinecraftVersion")?
            .unwrap_or_else(|| LOCAL_WORLD_TARGET_MINECRAFT_VERSION.to_owned());
    summary.mclone_version = js_optional_string_property(value, "mcloneVersion")?;
    summary.backend_label = js_optional_string_property(value, "backendLabel")?
        .or_else(|| Some(WEB_WORLD_BACKEND_LABEL.to_owned()));
    summary.locked = js_optional_bool_property(value, "locked")?.unwrap_or(false);
    summary.compatible = summary.storage_schema_version == LOCAL_WORLD_CATALOG_SCHEMA_VERSION
        && summary.target_minecraft_version == LOCAL_WORLD_TARGET_MINECRAFT_VERSION;
    Ok(summary)
}

fn encode_web_local_world_summaries(worlds: &[LocalWorldSummary]) -> Result<JsValue, String> {
    let array = js_sys::Array::new();
    for summary in worlds {
        array.push(&encode_web_local_world_summary(summary)?);
    }
    Ok(array.into())
}

fn encode_web_local_world_summary(summary: &LocalWorldSummary) -> Result<JsValue, String> {
    let object = js_sys::Object::new();
    set_string(&object, "id", summary.id.as_str())?;
    set_string(&object, "displayName", &summary.display_name)?;
    set_number(&object, "seed", summary.seed as f64)?;
    set_string(&object, "seedText", &summary.seed.to_string())?;
    set_string(
        &object,
        "generationProfile",
        summary.world_generation_profile.label(),
    )?;
    set_number(
        &object,
        "createdUnixMillis",
        summary.created_unix_millis as f64,
    )?;
    set_string(
        &object,
        "createdUnixMillisText",
        &summary.created_unix_millis.to_string(),
    )?;
    set_optional_number(
        &object,
        "lastPlayedUnixMillis",
        summary.last_played_unix_millis.map(|millis| millis as f64),
    )?;
    set_optional_string(
        &object,
        "lastPlayedUnixMillisText",
        summary
            .last_played_unix_millis
            .map(|millis| millis.to_string())
            .as_deref(),
    )?;
    set_number(
        &object,
        "storageSchemaVersion",
        f64::from(summary.storage_schema_version),
    )?;
    set_string(
        &object,
        "targetMinecraftVersion",
        &summary.target_minecraft_version,
    )?;
    set_optional_string(&object, "mcloneVersion", summary.mclone_version.as_deref())?;
    set_optional_string(&object, "backendLabel", summary.backend_label.as_deref())?;
    set_bool(&object, "locked", summary.locked)?;
    set_bool(&object, "compatible", summary.compatible)?;
    Ok(object.into())
}

fn encode_web_catalog_record(summary: &LocalWorldSummary) -> Result<JsValue, String> {
    let object = js_sys::Object::new();
    let descriptor = web_world_catalog_descriptor::encode(summary)?;
    set_string(&object, "id", summary.id.as_str())?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("descriptor"),
        js_sys::Uint8Array::from(descriptor.as_slice()).as_ref(),
    )
    .map_err(|error| format!("failed to attach world catalog descriptor: {error:?}"))?;
    Ok(object.into())
}

fn encode_web_catalog_mutation(summary: &LocalWorldSummary) -> Result<JsValue, String> {
    let object = js_sys::Object::new();
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("record"),
        &encode_web_catalog_record(summary)?,
    )
    .map_err(|error| format!("failed to attach world catalog record: {error:?}"))?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("summary"),
        &encode_web_local_world_summary(summary)?,
    )
    .map_err(|error| format!("failed to attach world catalog UI summary: {error:?}"))?;
    Ok(object.into())
}

fn parse_web_unix_millis(value: f64, label: &str) -> Result<u64, String> {
    if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > u64::MAX as f64 {
        return Err(format!("{label} must be a non-negative integer timestamp"));
    }
    Ok(value as u64)
}

fn js_property(value: &JsValue, key: &str) -> Result<JsValue, String> {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .map_err(|error| format!("failed to read JS property {key}: {error:?}"))
}

fn js_string_property(value: &JsValue, key: &str) -> Result<String, String> {
    let property = js_property(value, key)?;
    property
        .as_string()
        .ok_or_else(|| format!("JS property {key} must be a string"))
}

fn js_optional_string_property(value: &JsValue, key: &str) -> Result<Option<String>, String> {
    let property = js_property(value, key)?;
    if property.is_null() || property.is_undefined() {
        return Ok(None);
    }
    property
        .as_string()
        .map(Some)
        .ok_or_else(|| format!("JS property {key} must be a string or null"))
}

fn js_i64_property(value: &JsValue, key: &str) -> Result<i64, String> {
    let number = js_f64_property(value, key)?;
    if number.fract() != 0.0 || number < i64::MIN as f64 || number > i64::MAX as f64 {
        return Err(format!("JS property {key} must be an i64 integer"));
    }
    Ok(number as i64)
}

fn js_i64_text_or_number_property(
    value: &JsValue,
    text_key: &str,
    number_key: &str,
) -> Result<i64, String> {
    if let Some(text) = js_optional_string_property(value, text_key)? {
        return text
            .parse::<i64>()
            .map_err(|_| format!("JS property {text_key} must be an i64 integer string"));
    }
    js_i64_property(value, number_key)
}

fn js_u64_property(value: &JsValue, key: &str) -> Result<u64, String> {
    let number = js_f64_property(value, key)?;
    if number.fract() != 0.0 || number < 0.0 || number > u64::MAX as f64 {
        return Err(format!("JS property {key} must be a u64 integer"));
    }
    Ok(number as u64)
}

fn js_u64_text_or_number_property(
    value: &JsValue,
    text_key: &str,
    number_key: &str,
) -> Result<u64, String> {
    if let Some(text) = js_optional_string_property(value, text_key)? {
        return text
            .parse::<u64>()
            .map_err(|_| format!("JS property {text_key} must be a u64 integer string"));
    }
    js_u64_property(value, number_key)
}

fn js_optional_u64_text_or_number_property(
    value: &JsValue,
    text_key: &str,
    number_key: &str,
) -> Result<Option<u64>, String> {
    if let Some(text) = js_optional_string_property(value, text_key)? {
        return text
            .parse::<u64>()
            .map(Some)
            .map_err(|_| format!("JS property {text_key} must be a u64 integer string or null"));
    }
    js_optional_u64_property(value, number_key)
}

fn js_optional_u64_property(value: &JsValue, key: &str) -> Result<Option<u64>, String> {
    let property = js_property(value, key)?;
    if property.is_null() || property.is_undefined() {
        return Ok(None);
    }
    let number = property
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("JS property {key} must be a number or null"))?;
    if number.fract() != 0.0 || number < 0.0 || number > u64::MAX as f64 {
        return Err(format!("JS property {key} must be a u64 integer or null"));
    }
    Ok(Some(number as u64))
}

fn js_optional_u32_property(value: &JsValue, key: &str) -> Result<Option<u32>, String> {
    let Some(number) = js_optional_u64_property(value, key)? else {
        return Ok(None);
    };
    u32::try_from(number)
        .map(Some)
        .map_err(|_| format!("JS property {key} must fit u32"))
}

fn js_optional_bool_property(value: &JsValue, key: &str) -> Result<Option<bool>, String> {
    let property = js_property(value, key)?;
    if property.is_null() || property.is_undefined() {
        return Ok(None);
    }
    property
        .as_bool()
        .map(Some)
        .ok_or_else(|| format!("JS property {key} must be a boolean or null"))
}

fn js_f64_property(value: &JsValue, key: &str) -> Result<f64, String> {
    js_property(value, key)?
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("JS property {key} must be a finite number"))
}

fn parse_startup_options_from_query(search: &str) -> Result<StartupOptions, JsValue> {
    let params = web_sys::UrlSearchParams::new_with_str(search).map_err(|error| {
        JsValue::from_str(&format!("failed to parse startup query string: {error:?}"))
    })?;
    let mut scene = StartupSceneOptions::default();
    scene.render_distance = WEB_DEFAULT_RENDER_DISTANCE;
    let mut state = StartupArgState::new(scene, TexturedSectionRenderOptions::default());
    for key in STARTUP_QUERY_KEYS {
        if params.has(key) {
            state
                .parse_query_param(key, params.get(key), web_render_distance_limits())
                .map_err(|error| JsValue::from_str(&format!("{error:#}")))?;
        }
    }
    if state.scene().render_compile_capacity_request == RenderCompileCapacityRequest::Derived
        && state.scene().remote_addr.is_some()
    {
        return Err(JsValue::from_str(
            "--render-compile-capacity derived applies only to local integrated worlds",
        ));
    }
    if state.scene().render_compile_capacity_request == RenderCompileCapacityRequest::Derived {
        let report = preflight_render_compile_capacity_report(
            RenderCompileCapacityHostKind::Flat,
            host_total_memory_bytes(),
        );
        state.apply_render_compile_capacity_report(&report);
    }
    Ok(state.finish())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WebWorldStorageStartupOptions {
    pub(super) world_storage: String,
    pub(super) world_id: String,
    pub(super) clear_world_storage: bool,
}

fn parse_web_world_storage_from_query(
    search: &str,
    seed: i64,
) -> Result<WebWorldStorageStartupOptions, JsValue> {
    let params = web_sys::UrlSearchParams::new_with_str(search).map_err(|error| {
        JsValue::from_str(&format!(
            "failed to parse web world storage query string: {error:?}"
        ))
    })?;
    let raw_storage = params
        .get(WEB_QUERY_WORLD_STORAGE)
        .unwrap_or_else(|| "transient".to_owned());
    let world_storage = normalize_web_world_storage_label(&raw_storage).map_err(JsValue::from)?;
    let raw_world_id = params
        .get(WEB_QUERY_WORLD_ID)
        .unwrap_or_default()
        .trim()
        .to_owned();
    let world_id = if world_storage == "indexeddb" && raw_world_id.is_empty() {
        default_web_world_id(seed)
    } else {
        raw_world_id
    };
    Ok(WebWorldStorageStartupOptions {
        world_storage,
        world_id,
        clear_world_storage: query_truthy(&params, WEB_QUERY_CLEAR_WORLD_STORAGE),
    })
}

fn normalize_web_world_storage_label(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" | "transient" | "memory" => Ok("transient".to_owned()),
        "indexeddb" | "indexed-db" | "persistent" => Ok("indexeddb".to_owned()),
        other => Err(format!(
            "unsupported web worldStorage {other:?}; expected transient or indexeddb"
        )),
    }
}

fn default_web_world_id(seed: i64) -> String {
    format!("seed-{seed}")
}

fn query_truthy(params: &web_sys::UrlSearchParams, key: &str) -> bool {
    if !params.has(key) {
        return false;
    }
    let value = params.get(key).unwrap_or_default();
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => true,
    }
}

fn web_render_distance_limits() -> RenderDistanceLimits {
    RenderDistanceLimits::new(
        WEB_MIN_RENDER_DISTANCE as u32,
        WEB_MAX_RENDER_DISTANCE as u32,
    )
}

pub(super) struct WebCanvasContext {
    pub(super) canvas: HtmlCanvasElement,
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) format: wgpu::TextureFormat,
    present_mode: wgpu::PresentMode,
    alpha_mode: wgpu::CompositeAlphaMode,
    pub(super) width: u32,
    pub(super) height: u32,
}

impl WebCanvasContext {
    pub(super) async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        Self::new_with_color_profile(canvas, RenderColorProfile::default()).await
    }

    pub(super) async fn new_with_color_profile(
        canvas: HtmlCanvasElement,
        color_profile: RenderColorProfile,
    ) -> Result<Self, String> {
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|error| format!("failed to create WebGPU canvas surface: {error}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| format!("failed to request WebGPU adapter: {error}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("mclone_web_canvas_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|error| format!("failed to request WebGPU device: {error}"))?;

        let caps = surface.get_capabilities(&adapter);
        let Some(format) = RenderConfig::preferred_surface_format_for_profile(&caps, color_profile)
        else {
            return Err("WebGPU canvas surface reported no supported formats".to_owned());
        };
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        let present_mode = selected_present_mode(&caps.present_modes);
        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width,
                height,
                present_mode,
                alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            },
        );

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
        })
    }

    pub(super) fn resize(&mut self, width: u32, height: u32) -> bool {
        let width = width.max(1);
        let height = height.max(1);
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        if self.width == width && self.height == height {
            return false;
        }
        self.width = width;
        self.height = height;
        self.surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.format,
                width,
                height,
                present_mode: self.present_mode,
                alpha_mode: self.alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            },
        );
        true
    }
}

#[cfg(test)]
fn chunk_block_index(local_x: i32, local_y: i32, local_z: i32) -> usize {
    debug_assert!((0..CHUNK_WIDTH).contains(&local_x));
    debug_assert!(local_y >= 0);
    debug_assert!((0..CHUNK_WIDTH).contains(&local_z));
    (local_y as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize)
        + (local_z as usize * CHUNK_WIDTH as usize)
        + local_x as usize
}

fn set_bool(object: &js_sys::Object, key: &str, value: bool) -> Result<(), String> {
    js_sys::Reflect::set(object, &JsValue::from_str(key), &JsValue::from_bool(value))
        .map(|_| ())
        .map_err(|_| format!("failed to set generated chunk report key {key}"))
}

fn set_number(object: &js_sys::Object, key: &str, value: f64) -> Result<(), String> {
    js_sys::Reflect::set(object, &JsValue::from_str(key), &JsValue::from_f64(value))
        .map(|_| ())
        .map_err(|_| format!("failed to set generated chunk report key {key}"))
}

fn set_optional_number(
    object: &js_sys::Object,
    key: &str,
    value: Option<f64>,
) -> Result<(), String> {
    let value = value.map(JsValue::from_f64).unwrap_or(JsValue::NULL);
    js_sys::Reflect::set(object, &JsValue::from_str(key), &value)
        .map(|_| ())
        .map_err(|_| format!("failed to set generated chunk report key {key}"))
}

fn set_string(object: &js_sys::Object, key: &str, value: &str) -> Result<(), String> {
    js_sys::Reflect::set(object, &JsValue::from_str(key), &JsValue::from_str(value))
        .map(|_| ())
        .map_err(|_| format!("failed to set generated chunk report key {key}"))
}

fn set_optional_string(
    object: &js_sys::Object,
    key: &str,
    value: Option<&str>,
) -> Result<(), String> {
    let value = value.map(JsValue::from_str).unwrap_or(JsValue::NULL);
    js_sys::Reflect::set(object, &JsValue::from_str(key), &value)
        .map(|_| ())
        .map_err(|_| format!("failed to set generated chunk report key {key}"))
}

pub(super) fn set_worker_frame_metrics(
    object: &js_sys::Object,
    key: &str,
    metrics: mclone_server::WorkerFrameMetrics,
) -> Result<(), String> {
    let metrics_object = js_sys::Object::new();
    set_string(
        &metrics_object,
        "transportKind",
        metrics.transport_kind.label(),
    )?;
    set_number(
        &metrics_object,
        "requestFrames",
        metrics.request_frames as f64,
    )?;
    set_number(
        &metrics_object,
        "requestBytes",
        metrics.request_bytes as f64,
    )?;
    set_number(
        &metrics_object,
        "inboundFrames",
        metrics.inbound_frames as f64,
    )?;
    set_number(
        &metrics_object,
        "inboundBytes",
        metrics.inbound_bytes as f64,
    )?;
    set_number(
        &metrics_object,
        "maxPendingFrames",
        metrics.max_pending_frames as f64,
    )?;
    set_number(
        &metrics_object,
        "lastRequestUs",
        metrics.last_request_us as f64,
    )?;
    set_number(
        &metrics_object,
        "totalRequestUs",
        metrics.total_request_us as f64,
    )?;
    set_number(
        &metrics_object,
        "maxRequestUs",
        metrics.max_request_us as f64,
    )?;
    set_number(
        &metrics_object,
        "sharedBufferPoolHits",
        metrics.shared_buffer_pool_hits as f64,
    )?;
    set_number(
        &metrics_object,
        "sharedBufferPoolMisses",
        metrics.shared_buffer_pool_misses as f64,
    )?;
    set_number(
        &metrics_object,
        "sharedBufferPoolDrops",
        metrics.shared_buffer_pool_drops as f64,
    )?;
    set_number(
        &metrics_object,
        "sharedBufferCapacityBytes",
        metrics.shared_buffer_capacity_bytes as f64,
    )?;
    set_number(
        &metrics_object,
        "maxSharedBufferCapacityBytes",
        metrics.max_shared_buffer_capacity_bytes as f64,
    )?;
    set_number(
        &metrics_object,
        "sharedBufferPooledInboundFrames",
        metrics.shared_buffer_pooled_inbound_frames as f64,
    )?;
    set_number(
        &metrics_object,
        "sharedBufferFallbackInboundFrames",
        metrics.shared_buffer_fallback_inbound_frames as f64,
    )?;
    js_sys::Reflect::set(object, &JsValue::from_str(key), &metrics_object)
        .map(|_| ())
        .map_err(|_| format!("failed to set generated chunk report key {key}"))
}

#[cfg(test)]
fn build_flat_chunk_mesh(
    snapshot: &ChunkSnapshot,
) -> Result<mclone_mesh::VisibleChunkMesh, String> {
    use mclone_mesh::{ChunkMeshInput, build_visible_chunk_mesh};

    let blocks = snapshot_debug_mesh_blocks(snapshot)?;
    Ok(build_visible_chunk_mesh(ChunkMeshInput::new(
        snapshot.pos.x,
        snapshot.pos.z,
        snapshot.min_y,
        snapshot.height,
        &blocks,
    )))
}

#[cfg(test)]
fn snapshot_debug_mesh_blocks(snapshot: &ChunkSnapshot) -> Result<Vec<u8>, String> {
    if snapshot.height <= 0 || snapshot.height % mclone_core::SECTION_HEIGHT != 0 {
        return Err(format!(
            "chunk snapshot {:?} has invalid height {}",
            snapshot.pos, snapshot.height
        ));
    }
    let expected_len = snapshot.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
    let mut blocks = vec![0; expected_len];
    let min_section_y = snapshot.min_y / mclone_core::SECTION_HEIGHT;
    let section_count = snapshot.height / mclone_core::SECTION_HEIGHT;

    for section in &snapshot.sections {
        let section_offset = section.section_y - min_section_y;
        if !(0..section_count).contains(&section_offset) {
            return Err(format!(
                "chunk snapshot {:?} contains section {} outside {}..{}",
                snapshot.pos,
                section.section_y,
                min_section_y,
                min_section_y + section_count - 1
            ));
        }
        let unpacked = section.unpack_block_state_ids();
        if unpacked.len() != CHUNK_SECTION_VOLUME {
            return Err(format!(
                "chunk snapshot {:?} section {} unpacked to {} blocks",
                snapshot.pos,
                section.section_y,
                unpacked.len()
            ));
        }
        for local_y in 0..mclone_core::SECTION_HEIGHT {
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let section_index = chunk_section_index(local_x, local_y, local_z);
                    let chunk_local_y = section_offset * mclone_core::SECTION_HEIGHT + local_y;
                    let chunk_index = chunk_block_index(local_x, chunk_local_y, local_z);
                    blocks[chunk_index] = debug_mesh_id_for_block_state(unpacked[section_index]);
                }
            }
        }
    }

    Ok(blocks)
}

#[cfg(test)]
fn debug_mesh_id_for_block_state(state_id: BlockStateId) -> u8 {
    if state_id == AIR_BLOCK_STATE_ID {
        0
    } else {
        u8::try_from(state_id.0).unwrap_or(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_render_compile_delta_roundtrips_upserts_and_evictions() {
        let mut first_blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        first_blocks[chunk_section_index(1, 2, 3)] = BlockStateId(42);
        let mut second_blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        second_blocks[chunk_section_index(4, 5, 6)] = BlockStateId(7);
        let upserts = vec![
            ChunkSnapshot::from_block_state_ids(
                ChunkPos::new(0, 0),
                ChunkStatus::Full,
                ChunkRevision(1),
                0,
                16,
                &first_blocks,
            ),
            ChunkSnapshot::from_block_state_ids(
                ChunkPos::new(1, 0),
                ChunkStatus::Light,
                ChunkRevision(2),
                0,
                16,
                &second_blocks,
            ),
        ];
        let evictions = vec![ChunkPos::new(-3, 4), ChunkPos::new(5, -6)];

        // Full-resync delta (reset = true) carrying upserts and evictions together.
        let topology = HorizontalTopology::cylinder_x(-2, 32);
        let bytes =
            encode_web_render_compile_delta(7, true, Some(1124), topology, &upserts, &evictions)
                .unwrap();
        assert!(bytes.len() > WEB_RENDER_COMPILE_DELTA_MAGIC.len());
        let decoded = decode_web_render_compile_delta(&bytes).unwrap();
        assert_eq!(decoded.generation, 7);
        assert!(decoded.reset);
        assert_eq!(decoded.biome_zoom_seed, Some(1124));
        assert_eq!(decoded.topology, topology);
        assert_eq!(decoded.upserts, upserts);
        assert_eq!(decoded.evictions, evictions);

        // Incremental delta (reset = false) preserves the generation and the empty-set cases.
        let incremental = encode_web_render_compile_delta(
            8,
            false,
            None,
            HorizontalTopology::UNBOUNDED,
            &upserts[..1],
            &[],
        )
        .unwrap();
        let decoded = decode_web_render_compile_delta(&incremental).unwrap();
        assert_eq!(decoded.generation, 8);
        assert!(!decoded.reset);
        assert_eq!(decoded.biome_zoom_seed, None);
        assert_eq!(decoded.topology, HorizontalTopology::UNBOUNDED);
        assert_eq!(decoded.upserts, upserts[..1]);
        assert!(decoded.evictions.is_empty());

        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode_web_render_compile_delta(&trailing).is_err());
    }

    #[test]
    fn web_far_lod_tile_mesh_roundtrips_worker_payload() {
        let mesh = FarTerrainLodTileMesh::new(
            LodTileKey::new(ChunkPos::new(-7, 11), 1),
            vec![1.0, 2.0, 3.0, 0.25, 0.5, 0.75, 1.0],
            vec![0, 0, 0],
        );
        let packed = encode_web_far_lod_tile_mesh(&mesh).unwrap();
        assert_eq!(decode_web_far_lod_tile_mesh(&packed).unwrap(), mesh);

        let mut trailing = packed;
        trailing.push(0);
        assert!(decode_web_far_lod_tile_mesh(&trailing).is_err());
    }
}

fn selected_present_mode(supported: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    if supported.contains(&wgpu::PresentMode::Fifo) {
        wgpu::PresentMode::Fifo
    } else {
        supported
            .first()
            .copied()
            .unwrap_or(wgpu::PresentMode::AutoVsync)
    }
}

fn packed_dimension(value: u32) -> u32 {
    value.min(0x0fff)
}
