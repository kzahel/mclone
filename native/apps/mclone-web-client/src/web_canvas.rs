use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use std::collections::{BTreeMap, BTreeSet};

use super::{
    SMOKE_INITIAL_CENTER, SMOKE_MOVED_CENTER, SMOKE_RADIUS_CHUNKS, SMOKE_SEED,
    WebIntegratedServerRunnerConfig, WebRuntime,
};
use mclone_app_runtime::deferred_drop::{
    BoundedDeferredDropQueue, DEFAULT_DEFERRED_DROP_MAX_ITEMS, DeferredDropService,
};
use mclone_app_runtime::far_lod::{FarTerrainLodConfig, FarTerrainLodMaterialPalette};
use mclone_app_runtime::host_mode::SingleViewHostMode;
use mclone_app_runtime::monotonic::MonotonicDeadline;
use mclone_app_runtime::prepared_assets::{
    AUTHORED_FIRST_PARTY_PACK_ID, AssetPackSourceRegistry, MINECRAFT_REFERENCE_PACK_ID,
    PreparedSceneAssets,
};
use mclone_app_runtime::render_asset_data::TexturedMeshAssets;
use mclone_app_runtime::render_compile_capacity::{
    RenderCompileCapacityHostKind, host_total_memory_bytes,
    preflight_render_compile_capacity_report,
};
use mclone_app_runtime::scene_session_runtime::{
    SceneRuntimeService, SceneSessionRuntime, StartupReadinessPolicy,
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
    GameplayCommandTiming, GameplayCommandUpdatePolicy, RuntimePollTiming, RuntimeUpdatePumpBudget,
    TimedRenderSectionCacheUpdate, loading_progress_overlay_from_diagnostics,
    view_readiness_overlay_from_diagnostics,
};
use mclone_assets::{AssetPackId, AssetPackSelection, PackedAssetSource, SharedAssetSource};
use mclone_audio::PreparedAudioAssets;
use mclone_client::ClientRuntime;
#[cfg(test)]
use mclone_core::{
    AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkStatus, chunk_section_index,
};
use mclone_core::{ChunkPos, ChunkRevision, ChunkSnapshot};
use mclone_mesh::{
    RenderSectionKey, TexturedMeshCatalog, TexturedRenderSectionBuildReport,
    TexturedRenderSectionMesh, TexturedRenderSectionMetadata, load_textured_terrain_assets,
};
use mclone_protocol::{ClientCommand, ServerUpdate, decode_server_update, encode_server_update};
use mclone_render::actor_assets::load_actor_texture_assets;
use mclone_render::chunk::TexturedSectionRenderOptions;
use mclone_render::color_profile::{RenderColorProfile, RenderConfig};
use mclone_render::far_lod::FarTerrainLodMesh;
use mclone_render::screen_effect::load_screen_effect_texture_assets;
use mclone_render_session::{
    RenderSectionCacheUpdate, RenderSectionCompileQueueHealth, RenderSectionCompileRequest,
    RenderSectionCompileResult, RenderSectionCompiler, build_client_textured_sections,
    build_render_sections_from_snapshots_with_biome_zoom_seed,
    decode_textured_render_section_build_report, encode_textured_render_section_build_report,
    summarize_textured_render_section_build_report,
};
use mclone_server::{ServerRunnerKind, SimulationCadenceConfig};
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
// magic, u32 generation, u32 flags (bit 0 = reset/full-resync), u32 upsert_count followed
// by that many length-prefixed `ServerUpdate::ChunkSnapshot` frames, then u32 evict_count
// followed by that many `[i32 x][i32 z]` chunk positions to drop from the mirror.
const WEB_RENDER_COMPILE_DELTA_MAGIC: &[u8; 8] = b"MCWRCD1\0";
const WEB_RENDER_COMPILE_DELTA_FLAG_RESET: u32 = 1;
const WEB_RENDER_COMPILE_DELTA_FLAG_BIOME_ZOOM_SEED: u32 = 2;

// 067 Stage 2 ABI / Stage 5 lock: resident render-compiler shared ring constants.
// The worker writes the result control word + payload and main wasm reads them back
// here via `js_sys::Atomics`. The JS side now authors these once in
// `www/mclone-render-compiler-abi.js` (imported by the worker producer + the app/smoke
// consumers); this Rust block is the main-wasm reader's copy. The two authored copies
// are kept in lockstep by the host test `tests/render_compiler_abi_lock.rs`, which parses
// both files and fails on any drift — so a mismatched status-word index is a failing test,
// not a silent SAB corruption. (A `build.rs`/codegen single-source was considered but
// rejected: there is no codegen precedent in this crate, and a parse-and-assert lock has a
// far smaller blast radius than wiring a generated, committed JS artifact.)
const RENDER_COMPILER_SHARED_RESULT_CONTROL_WORDS: u32 = 4;
const RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX: u32 = 0;
const RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX: u32 = 1;
const RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX: u32 = 2;
const RENDER_COMPILER_SHARED_RESULT_PENDING: i32 = 1;
const RENDER_COMPILER_SHARED_RESULT_COMPLETE: i32 = 2;
const RENDER_COMPILER_SHARED_RESULT_OVERFLOW: i32 = 3;
const RENDER_COMPILER_SHARED_RESULT_FAILED: i32 = 4;
const RENDER_COMPILER_SHARED_INPUT_CONTROL_WORDS: u32 = 4;
const RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX: u32 = 0;
const RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX: u32 = 1;
const RENDER_COMPILER_SHARED_INPUT_CAPACITY_INDEX: u32 = 2;
const RENDER_COMPILER_SHARED_INPUT_READY: i32 = 2;
const RENDER_COMPILER_DEFAULT_SHARED_RESULT_CAPACITY: u32 = 16 * 1024 * 1024;
const RENDER_COMPILER_DEFAULT_SHARED_INPUT_CAPACITY: u32 = 1024 * 1024;

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
pub fn mclone_web_startup_options_from_query(search: String) -> Result<JsValue, JsValue> {
    let options = parse_startup_options_from_query(&search)?;
    let storage = parse_web_world_storage_from_query(&search, options.scene.seed)?;
    let value = startup_options_to_js_value(&options).map_err(JsValue::from)?;
    let object: js_sys::Object = value.unchecked_into();
    attach_web_world_storage_startup_options(&object, &storage).map_err(JsValue::from)?;
    Ok(object.into())
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
    mesh_assets: WebTexturedMeshAssets,
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
            mesh_assets,
            asset_pack_byte_length,
            asset_load_count: 1,
            compile_count: 0,
            snapshot_mirror: BTreeMap::new(),
            mirror_generation: 0,
            biome_zoom_seed: None,
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
            mesh_assets: WebTexturedMeshAssets {
                catalog: mesh_assets.catalog,
                asset_pack_file_count,
            },
            asset_pack_byte_length,
            asset_load_count: 3,
            compile_count: 0,
            snapshot_mirror: BTreeMap::new(),
            mirror_generation: 0,
            biome_zoom_seed: None,
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

    #[wasm_bindgen(js_name = compileGeneratedChunkSections)]
    pub fn compile_generated_chunk_sections(
        &mut self,
        center_x: i32,
        center_z: i32,
        radius_chunks: u32,
    ) -> Result<js_sys::Uint8Array, JsValue> {
        self.compile_count += 1;
        let report = compile_generated_chunk_sections_with_catalog(
            &self.mesh_assets.catalog,
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

    #[wasm_bindgen(js_name = compileGeneratedChunkSectionsForTargets)]
    pub fn compile_generated_chunk_sections_for_targets(
        &mut self,
        center_x: i32,
        center_z: i32,
        radius_chunks: u32,
        target_sections: js_sys::Int32Array,
    ) -> Result<js_sys::Uint8Array, JsValue> {
        self.compile_count += 1;
        let target_sections = render_section_keys_from_int32_array(&target_sections)
            .map_err(|error| JsValue::from_str(&error))?;
        let report = compile_generated_chunk_sections_with_catalog(
            &self.mesh_assets.catalog,
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
        )
        .map_err(JsValue::from)?;
        let packed = encode_textured_render_section_build_report(&report);
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
    let center_chunk_loaded = runtime
        .client()
        .chunk_snapshot(SMOKE_INITIAL_CENTER)
        .is_some();
    let second = runtime
        .request_chunk_view_async(SMOKE_MOVED_CENTER, SMOKE_RADIUS_CHUNKS, SMOKE_RADIUS_CHUNKS)
        .await?;
    let moved_chunk_loaded = runtime
        .client()
        .chunk_snapshot(SMOKE_MOVED_CENTER)
        .is_some();
    let previous_chunk_unloaded = runtime
        .client()
        .chunk_snapshot(SMOKE_INITIAL_CENTER)
        .is_none();
    let diagnostics = runtime.runner_diagnostics();
    let metrics = diagnostics.runner_frame_metrics;
    let ok = runtime.client().host() == mclone_client::ClientHost::RemoteDedicated
        && diagnostics.kind == ServerRunnerKind::RemoteWebSocket
        && metrics.transport_kind == mclone_server::WorkerFrameTransportKind::WebSocket
        && metrics.request_frames >= 3
        && metrics.response_frames >= 3
        && center_chunk_loaded
        && moved_chunk_loaded
        && previous_chunk_unloaded
        && runtime.transport_drained()
        && runtime.protocol_codec_roundtrip()
        && diagnostics.command_queue_depth == 0
        && diagnostics.update_queue_depth == 0
        && runtime.command_count() == 2
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
    let report = decode_textured_render_section_build_report(&packed)
        .map_err(|error| JsValue::from_str(&format!("{error:#}")))?;
    packed_compile_report_summary_to_js(packed.len(), &report).map_err(JsValue::from)
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
) -> Result<mclone_mesh::TexturedRenderSectionBuildReport, String> {
    build_render_sections_from_snapshots_with_biome_zoom_seed(
        snapshots,
        catalog,
        target_sections,
        biome_zoom_seed,
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
    upserts: Vec<ChunkSnapshot>,
    evictions: Vec<ChunkPos>,
}

fn encode_web_render_compile_delta(
    generation: u32,
    reset: bool,
    biome_zoom_seed: Option<i64>,
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
        upserts,
        evictions,
    })
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

fn render_compiler_atomic_store(
    control: &js_sys::Int32Array,
    index: u32,
    value: i32,
) -> Result<(), String> {
    js_sys::Atomics::store(control, index, value)
        .map(|_| ())
        .map_err(|error| format!("failed to store render-compile control word {index}: {error:?}"))
}

fn render_compiler_atomic_load(control: &js_sys::Int32Array, index: u32) -> Result<i32, String> {
    js_sys::Atomics::load(control, index)
        .map_err(|error| format!("failed to load render-compile control word {index}: {error:?}"))
}

fn render_compiler_shared_memory_supported() -> bool {
    let global = js_sys::global();
    js_global_is_function(&global, "SharedArrayBuffer")
        && js_global_method_is_function(&global, "Atomics", "load")
        && js_global_method_is_function(&global, "Atomics", "store")
        && js_global_method_is_function(&global, "Atomics", "notify")
}

fn js_global_is_function(global: &JsValue, name: &str) -> bool {
    js_sys::Reflect::get(global, &JsValue::from_str(name))
        .ok()
        .is_some_and(|value| value.is_function())
}

fn js_global_method_is_function(global: &JsValue, object_name: &str, method_name: &str) -> bool {
    js_sys::Reflect::get(global, &JsValue::from_str(object_name))
        .ok()
        .and_then(|object| js_sys::Reflect::get(&object, &JsValue::from_str(method_name)).ok())
        .is_some_and(|value| value.is_function())
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
        render_compiler_atomic_store(
            &arena.result_control,
            RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX,
            RENDER_COMPILER_SHARED_RESULT_PENDING,
        )?;
        render_compiler_atomic_store(
            &arena.result_control,
            RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX,
            0,
        )?;
        render_compiler_atomic_store(
            &arena.result_control,
            RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX,
            arena.result_capacity as i32,
        )?;
        render_compiler_atomic_store(
            &arena.input_control,
            RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX,
            RENDER_COMPILER_SHARED_INPUT_READY,
        )?;
        render_compiler_atomic_store(
            &arena.input_control,
            RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX,
            0,
        )?;
        render_compiler_atomic_store(
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
        render_compiler_atomic_store(
            &self.input_control,
            RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX,
            byte_length as i32,
        )?;
        render_compiler_atomic_store(
            &self.input_control,
            RENDER_COMPILER_SHARED_INPUT_CAPACITY_INDEX,
            self.input_capacity as i32,
        )?;
        render_compiler_atomic_store(
            &self.input_control,
            RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX,
            RENDER_COMPILER_SHARED_INPUT_READY,
        )?;
        // Arm the result control word last so the worker only observes PENDING after
        // the input payload is fully published into shared memory.
        render_compiler_atomic_store(
            &self.result_control,
            RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX,
            0,
        )?;
        render_compiler_atomic_store(
            &self.result_control,
            RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX,
            self.result_capacity as i32,
        )?;
        render_compiler_atomic_store(
            &self.result_control,
            RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX,
            RENDER_COMPILER_SHARED_RESULT_PENDING,
        )?;
        Ok(grew)
    }

    fn poll_result_status(&self) -> Result<i32, String> {
        render_compiler_atomic_load(
            &self.result_control,
            RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX,
        )
    }

    fn result_byte_length(&self) -> Result<u32, String> {
        let bytes = render_compiler_atomic_load(
            &self.result_control,
            RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX,
        )?;
        u32::try_from(bytes)
            .map_err(|_| format!("render compile result reported a negative byte count {bytes}"))
    }

    fn result_capacity_word(&self) -> Result<u32, String> {
        let capacity = render_compiler_atomic_load(
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

struct WebSharedCompileInFlight {
    request_id: u32,
    target_sections: BTreeSet<RenderSectionKey>,
    section_revisions: BTreeMap<RenderSectionKey, u64>,
}

/// Web `RenderSectionCompiler` over the resident `SharedArrayBuffer` ring (067 Stage 2),
/// shipping a per-frame *delta* against the worker's resident snapshot mirror (067 Stage 4).
///
/// `submit` diffs the request's loaded snapshots against `mirror_tracking` — a cheap
/// `(pos, revision)` shadow of what the worker mirror holds — to compute the changed
/// columns (upserts) and unloaded columns (evictions), encodes only that delta into the
/// resident input arena, and arms the result control word; JavaScript posts a tiny
/// doorbell to the worker (JS still owns the Worker lifecycle and diagnostics).
/// `try_recv_completed` polls the result control word from main wasm via `js_sys::Atomics`
/// and decodes the packed report in place — no JavaScript in the result data path. A
/// single compile is in flight at a time, matching the existing busy-flag streaming model,
/// so the mirror is mutated by exactly one compile at a time and needs no locking.
struct WebRenderSectionCompiler {
    shared: Option<WebRenderCompilerSharedArena>,
    in_flight: Option<WebSharedCompileInFlight>,
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
        let shared = if render_compiler_shared_memory_supported() {
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
            .map(|in_flight| in_flight.request_id)
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

    /// Attach the resident shared buffers + byte counts to a worker doorbell message.
    /// JavaScript adds the message kind / bindgen URLs and posts it; the worker reads
    /// the input and writes the packed result into the same buffers this compiler polls.
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

    /// Build a complete worker doorbell for the in-flight compile (067 Stage 3): the
    /// request id, the budget's target section keys (so the worker compiles only those —
    /// the per-frame increment, not a whole-view job), the delta input width, and the
    /// resident shared buffers. JS posts it verbatim; the worker writes the packed result
    /// back into the same buffers `try_recv_completed` polls. Returns whether a doorbell
    /// was written (false when no compile is in flight).
    fn write_doorbell(&self, object: &js_sys::Object) -> Result<bool, String> {
        let Some(in_flight) = self.in_flight.as_ref() else {
            return Ok(false);
        };
        set_number(object, "requestId", f64::from(in_flight.request_id))?;
        write_target_sections(object, &in_flight.target_sections)?;
        set_number(
            object,
            "submittedCompileSectionCount",
            in_flight.target_sections.len() as f64,
        )?;
        // 067 Stage 4: the input is now a delta, so the "chunk count" reported up the existing
        // diagnostics chain is the number of columns shipped this compile (upserts). A
        // neighbor-dirtied target legitimately ships 0 upserts (the worker mirror already holds
        // it), while the delta byte length stays > 0 (fixed header). The worker surfaces the
        // matching upsert/eviction/mirror counts from its own session getters.
        set_number(
            object,
            "snapshotInputChunkCount",
            self.last_input_upsert_count as f64,
        )?;
        // 067 follow-up 1: columns actually cloned on the main thread for this submit, counted
        // at the clone site. It must equal `snapshotInputChunkCount` (the upserts shipped); the
        // smoke asserts the equality so a reintroduced whole-world clone (clone all, ship delta)
        // shows up as a divergence instead of silently regressing main-thread allocation.
        set_number(
            object,
            "snapshotInputClonedColumnCount",
            self.last_submit_cloned_column_count as f64,
        )?;
        self.write_doorbell_arenas(object)?;
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
        self.in_flight = Some(WebSharedCompileInFlight {
            request_id,
            target_sections,
            section_revisions,
        });
        self.pending_jobs = 1;
        self.compile_count += 1;
        Ok(())
    }

    fn try_recv_completed(&mut self) -> anyhow::Result<Vec<RenderSectionCompileResult>> {
        if self.in_flight.is_none() {
            return Ok(Vec::new());
        }
        let Some(arena) = self.shared.as_ref() else {
            return Ok(Vec::new());
        };
        let status = arena
            .poll_result_status()
            .map_err(|error| anyhow::anyhow!(error))?;
        match status {
            RENDER_COMPILER_SHARED_RESULT_COMPLETE => {
                let packed = self
                    .shared
                    .as_ref()
                    .expect("shared arena present")
                    .read_result_bytes()
                    .map_err(|error| anyhow::anyhow!(error))?;
                let in_flight = self
                    .take_in_flight()
                    .expect("in-flight compile present after status check");
                let section_report =
                    decode_textured_render_section_build_report(&packed).map_err(|error| {
                        anyhow::anyhow!("failed to decode packed render section report: {error:#}")
                    })?;
                self.last_packed_byte_length = packed.len();
                self.last_result_overflow = false;
                self.shared_result_response_count += 1;
                self.shared_result_byte_count += packed.len();
                if let Some(arena) = self.shared.as_ref() {
                    self.last_result_capacity_bytes = arena.result_capacity as usize;
                }
                Ok(vec![render_section_compile_result_from_report(
                    in_flight.target_sections,
                    in_flight.section_revisions,
                    section_report,
                )])
            }
            RENDER_COMPILER_SHARED_RESULT_OVERFLOW => {
                // Labeled hard-diagnostic fallback: the worker had to allocate a
                // one-off larger buffer (only reachable through its postMessage), so
                // main wasm cannot read it from the resident ring. Record it, grow the
                // resident buffer so the retry fits, and fail this compile so the
                // sections requeue. With a 16 MB resident buffer vs ~11.6 MB worst case
                // this path stays dormant in the validated lanes.
                let required = self
                    .shared
                    .as_ref()
                    .expect("shared arena present")
                    .result_capacity_word()
                    .map_err(|error| anyhow::anyhow!(error))?;
                if let Some(arena) = self.shared.as_mut() {
                    arena.grow_result(required);
                    self.last_result_capacity_bytes = arena.result_capacity as usize;
                }
                self.shared_result_overflow_count += 1;
                self.last_result_overflow = true;
                // 067 Stage 4: do NOT clear the mirror shadow here. The worker applied this
                // delta to its mirror before encoding the (oversized) result, so the mirror is
                // correctly advanced; keeping the shadow lets the requeued compile ship a
                // minimal delta instead of re-shipping the whole world.
                let in_flight = self.take_in_flight().expect("in-flight compile present");
                Ok(vec![RenderSectionCompileResult {
                    target_sections: in_flight.target_sections,
                    section_revisions: in_flight.section_revisions,
                    result: Err(format!(
                        "render compile result overflowed the resident shared buffer ({required} bytes required)"
                    )),
                }])
            }
            RENDER_COMPILER_SHARED_RESULT_FAILED => {
                self.last_result_overflow = false;
                // 067 Stage 4: a worker-reported failure (decode error, or the mirror-generation
                // desync tripwire) leaves the worker mirror in an unknown state, so drop the
                // shadow. The next submit then sees an empty shadow and ships a full resync,
                // rebuilding the worker mirror from scratch.
                self.mirror_tracking.clear();
                let in_flight = self.take_in_flight().expect("in-flight compile present");
                Ok(vec![RenderSectionCompileResult {
                    target_sections: in_flight.target_sections,
                    section_revisions: in_flight.section_revisions,
                    result: Err("render compiler worker reported a failed compile".to_string()),
                }])
            }
            RENDER_COMPILER_SHARED_RESULT_PENDING | 0 => Ok(Vec::new()),
            other => Err(anyhow::anyhow!(
                "render compile result control word had unexpected status {other}"
            )),
        }
    }

    fn pending_job_count(&self) -> usize {
        self.pending_jobs
    }
}

/// Browser-owned compiler wake capability. The shared scene sees only
/// `RenderSectionCompiler`; the JavaScript callback and doorbell object remain
/// private to this adapter.
#[derive(Clone)]
struct WebRenderCompilerWakeSink {
    callback: js_sys::Function,
}

impl std::fmt::Debug for WebRenderCompilerWakeSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WebRenderCompilerWakeSink")
            .finish_non_exhaustive()
    }
}

impl WebRenderCompilerWakeSink {
    fn wake(&self, compiler: &WebRenderSectionCompiler) -> anyhow::Result<()> {
        let doorbell = js_sys::Object::new();
        if compiler
            .write_doorbell(&doorbell)
            .map_err(anyhow::Error::msg)?
        {
            self.callback
                .call1(&JsValue::NULL, &doorbell)
                .map_err(|error| anyhow::anyhow!("render-compiler wake failed: {error:?}"))?;
        }
        Ok(())
    }
}

/// Browser implementation of the neutral scene runtime boundary.
///
/// It reuses the resident server worker/WebSocket connection and render
/// compiler. Production browser hosts inject this service into
/// `McloneSceneHost`; JavaScript sees only the private wake callback.
pub struct WebSceneRuntimeService {
    runtime: WebRuntime,
    mesh_assets: TexturedMeshAssets,
    render_compiler: WebRenderSectionCompiler,
    compiler_wake: WebRenderCompilerWakeSink,
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
    pub async fn local_worker(
        config: WebIntegratedServerRunnerConfig,
        mesh_assets: TexturedMeshAssets,
        compiler_wake: js_sys::Function,
    ) -> Result<Self, String> {
        let runtime = WebRuntime::web_worker_integrated(config).await?;
        Ok(Self::new(runtime, mesh_assets, compiler_wake))
    }

    pub async fn remote_websocket(
        url: impl Into<String>,
        mesh_assets: TexturedMeshAssets,
        compiler_wake: js_sys::Function,
    ) -> Result<Self, String> {
        let runtime = WebRuntime::websocket_remote(url).await?;
        Ok(Self::new(runtime, mesh_assets, compiler_wake))
    }

    pub fn new(
        runtime: WebRuntime,
        mesh_assets: TexturedMeshAssets,
        compiler_wake: js_sys::Function,
    ) -> Self {
        Self {
            runtime,
            mesh_assets,
            render_compiler: WebRenderSectionCompiler::new(),
            compiler_wake: WebRenderCompilerWakeSink {
                callback: compiler_wake,
            },
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
            self.compiler_wake.wake(&self.render_compiler)?;
        }
        Ok(update)
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
        self.render_compiler = WebRenderSectionCompiler::new();
        self.mesh_assets = mesh_assets;
        Ok(())
    }

    fn clear_far_lod(&mut self) {}

    fn prepare_far_lod_mesh(
        &mut self,
        _config: FarTerrainLodConfig,
        _seed: i64,
        _center: ChunkPos,
        _camera_position: glam::Vec3,
    ) -> Option<&FarTerrainLodMesh> {
        None
    }

    fn lod_coverage_counters(&self) -> mclone_app_runtime::lod_coverage::LodReplacementCounters {
        Default::default()
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
    ) -> anyhow::Result<(bool, GameplayCommandTiming)> {
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
        Ok((
            true,
            GameplayCommandTiming {
                updates: report.update_count,
                ..GameplayCommandTiming::default()
            },
        ))
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
                producer_response_sequence: pump.producer_response_sequence,
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
        // IndexedDB writes are already owned by the integrated-server worker;
        // graceful shutdown is a separate typed browser operation.
        Ok(0)
    }

    fn refresh_startup_diagnostics(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn startup_progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        loading_progress_overlay_from_diagnostics(&self.diagnostics())
    }

    fn startup_host_ready(
        &self,
        policy: StartupReadinessPolicy,
        camera_position: glam::Vec3,
    ) -> bool {
        let view_ready = !self
            .traversal_ready_render_section_keys(camera_position)
            .is_empty();
        match policy {
            StartupReadinessPolicy::Playable => self.loaded_chunk_count() > 0 && view_ready,
            StartupReadinessPolicy::Idle => {
                let diagnostics = self.diagnostics();
                view_ready
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
    summary.last_played_unix_millis = Some(now);
    summary.backend_label = Some(WEB_WORLD_BACKEND_LABEL.to_owned());
    encode_web_local_world_summary(&summary).map_err(JsValue::from)
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
    encode_web_local_world_summary(&summary).map_err(JsValue::from)
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

fn render_section_keys_from_int32_array(
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
    asset_pack_file_count: usize,
}

fn load_textured_mesh_assets_from_pack(bytes: Vec<u8>) -> Result<WebTexturedMeshAssets, String> {
    let source = PackedAssetSource::from_bytes(bytes)
        .map_err(|error| format!("failed to parse packed Minecraft assets: {error}"))?;
    let asset_pack_file_count = source.file_count();
    let assets = load_textured_terrain_assets(&source)
        .map_err(|error| format!("failed to load packed textured terrain assets: {error}"))?;

    Ok(WebTexturedMeshAssets {
        catalog: assets.catalog,
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
        GameUiAction::BackToTitle => "backToTitle",
        GameUiAction::BackToPause => "backToPause",
        GameUiAction::QuitToTitle => "quitToTitle",
        GameUiAction::ToggleSectionOcclusion => "toggleSectionOcclusion",
        GameUiAction::ToggleFullbright => "toggleFullbright",
        GameUiAction::ToggleFarLod => "toggleFarLod",
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
        "deleteWorld" => Ok(WorldCatalogResponse::WorldDeleted {
            id: LocalWorldId::new(js_string_property(payload, "id")?)
                .map_err(|error| error.message)?,
        }),
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
    let id = LocalWorldId::new(js_string_property(value, "id")?).map_err(|error| error.message)?;
    let mut summary = LocalWorldSummary::new(
        id,
        js_string_property(value, "displayName")?,
        js_i64_property(value, "seed")?,
        js_u64_property(value, "createdUnixMillis")?,
    )
    .map_err(|error| error.message)?;
    summary.last_played_unix_millis = js_optional_u64_property(value, "lastPlayedUnixMillis")?;
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
    set_number(
        &object,
        "createdUnixMillis",
        summary.created_unix_millis as f64,
    )?;
    set_optional_number(
        &object,
        "lastPlayedUnixMillis",
        summary.last_played_unix_millis.map(|millis| millis as f64),
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

fn js_u64_property(value: &JsValue, key: &str) -> Result<u64, String> {
    let number = js_f64_property(value, key)?;
    if number.fract() != 0.0 || number < 0.0 || number > u64::MAX as f64 {
        return Err(format!("JS property {key} must be a u64 integer"));
    }
    Ok(number as u64)
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
struct WebWorldStorageStartupOptions {
    world_storage: String,
    world_id: String,
    clear_world_storage: bool,
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

fn attach_web_world_storage_startup_options(
    object: &js_sys::Object,
    options: &WebWorldStorageStartupOptions,
) -> Result<(), String> {
    set_string(object, "worldStorage", &options.world_storage)?;
    set_string(object, "worldId", &options.world_id)?;
    set_bool(object, "clearWorldStorage", options.clear_world_storage)
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

pub(super) fn startup_render_options(
    section_occlusion_culling: bool,
    force_fullbright: bool,
    render_color_profile: &str,
) -> Result<TexturedSectionRenderOptions, String> {
    let color_profile = render_color_profile
        .parse::<RenderColorProfile>()
        .map_err(|message| format!("renderColorProfile {message}"))?;
    Ok(TexturedSectionRenderOptions {
        section_occlusion_culling,
        force_fullbright,
        color_profile,
        ..TexturedSectionRenderOptions::default()
    })
}

fn startup_options_to_js_value(options: &StartupOptions) -> Result<JsValue, String> {
    let object = js_sys::Object::new();
    set_bool(&object, "ok", true)?;
    set_string(&object, "seedText", &options.scene.seed.to_string())?;
    set_number(&object, "seed", options.scene.seed as f64)?;
    set_number(&object, "chunkX", f64::from(options.scene.chunk_x))?;
    set_number(&object, "chunkZ", f64::from(options.scene.chunk_z))?;
    set_number(
        &object,
        "renderDistance",
        f64::from(options.scene.render_distance),
    )?;
    set_number(
        &object,
        "movementSpeedMultiplier",
        f64::from(options.scene.movement_speed_multiplier),
    )?;
    if let Some(remote_addr) = &options.scene.remote_addr {
        set_string(&object, "remoteWebSocketUrl", remote_addr)?;
    }
    if let Some(day_time) = options.scene.day_time_override {
        set_number(&object, "dayTime", day_time as f64)?;
    }
    set_bool(&object, "freezeTime", options.scene.freeze_time)?;
    set_bool(
        &object,
        "debugPassiveShowcase",
        options.scene.debug_passive_showcase,
    )?;
    set_bool(&object, "lightingEnabled", options.scene.lighting_enabled)?;
    set_number(
        &object,
        "lightStatusBatchSize",
        options.scene.light_status_batch_size as f64,
    )?;
    set_bool(
        &object,
        "sectionOcclusionCulling",
        options.render_options.section_occlusion_culling,
    )?;
    set_bool(
        &object,
        "forceFullbright",
        options.render_options.force_fullbright,
    )?;
    set_string(
        &object,
        "renderColorProfile",
        options.render_options.color_profile.as_str(),
    )?;
    Ok(object.into())
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
        "responseFrames",
        metrics.response_frames as f64,
    )?;
    set_number(
        &metrics_object,
        "responseBytes",
        metrics.response_bytes as f64,
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
        "sharedBufferPooledResponseFrames",
        metrics.shared_buffer_pooled_response_frames as f64,
    )?;
    set_number(
        &metrics_object,
        "sharedBufferFallbackResponseFrames",
        metrics.shared_buffer_fallback_response_frames as f64,
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
        let bytes =
            encode_web_render_compile_delta(7, true, Some(1124), &upserts, &evictions).unwrap();
        assert!(bytes.len() > WEB_RENDER_COMPILE_DELTA_MAGIC.len());
        let decoded = decode_web_render_compile_delta(&bytes).unwrap();
        assert_eq!(decoded.generation, 7);
        assert!(decoded.reset);
        assert_eq!(decoded.biome_zoom_seed, Some(1124));
        assert_eq!(decoded.upserts, upserts);
        assert_eq!(decoded.evictions, evictions);

        // Incremental delta (reset = false) preserves the generation and the empty-set cases.
        let incremental =
            encode_web_render_compile_delta(8, false, None, &upserts[..1], &[]).unwrap();
        let decoded = decode_web_render_compile_delta(&incremental).unwrap();
        assert_eq!(decoded.generation, 8);
        assert!(!decoded.reset);
        assert_eq!(decoded.biome_zoom_seed, None);
        assert_eq!(decoded.upserts, upserts[..1]);
        assert!(decoded.evictions.is_empty());

        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode_web_render_compile_delta(&trailing).is_err());
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
