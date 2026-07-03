use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use std::collections::{BTreeMap, BTreeSet};

use super::{
    SMOKE_INITIAL_CENTER, SMOKE_MOVED_CENTER, SMOKE_RADIUS_CHUNKS, SMOKE_SEED,
    WebIntegratedServerRunnerConfig, WebRuntime,
};
use mclone_app_runtime::session::{
    ActiveSessionDescriptor, GameSessionCoordinator, GameSessionState, RemoteSessionEndpoint,
    SessionFailure, SessionStartRequest, SessionStartResult, StartedGameSession,
};
use mclone_app_runtime::startup_args::{
    RenderDistanceLimits, STARTUP_QUERY_KEYS, StartupArgState, StartupOptions, StartupSceneOptions,
};
use mclone_app_runtime::{
    debug_block_palette_overlay, debug_hotbar_icons, set_player_appearance_command_for_ui_model,
};
use mclone_assets::PackedAssetSource;
use mclone_client::{
    ActorInterpolationConfig, ActorInterpolationState, ClientInteractionController, ClientRuntime,
};
#[cfg(test)]
use mclone_core::{
    AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkStatus, chunk_section_index,
};
use mclone_core::{
    BlockHitResult, BlockPos, BlockStateId, ChunkPos, ChunkRevision, ChunkSnapshot, Direction,
    HitResultType, Vec3d, chunk_middle_block_coord,
};
use mclone_input::{
    InputCapabilities, InputCapabilityState, InputDeviceKind, InputPreferences, TouchControlsMode,
    keyboard_turn_mouse_delta,
};
use mclone_mesh::{
    RenderSectionKey, TextureAtlasImage, TexturedMeshCatalog, TexturedRenderSectionBuildReport,
    load_textured_terrain_assets,
};
use mclone_protocol::{ServerUpdate, decode_server_update, encode_server_update};
use mclone_render::actor_assets::{ActorTextureImage, load_actor_texture_assets};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, ChunkTextureAtlas,
    TexturedSectionDrawResources, TexturedSectionRenderOptions, TexturedSectionRenderPhase,
    TexturedSectionRenderStats, TexturedSectionUploadReport,
};
use mclone_render::color_profile::{RenderColorProfile, RenderConfig};
use mclone_render::entity::{ActorDrawResources, ActorFigureSet, ActorInstance};
use mclone_render::gui::{GuiRenderOptions, GuiRenderer};
use mclone_render::sky_render::SkyRenderer;
use mclone_render::target::RenderFrameTarget;
use mclone_render_session::{
    ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER, ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER, ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER, EngineCameraController, EngineCameraFrameState,
    EngineCameraInput, EngineCameraMovementImpulse, EngineCameraMovementMode, EngineRenderCamera,
    RenderSectionCacheUpdate, RenderSectionCompileAcceptanceReport, RenderSectionCompileRequest,
    RenderSectionCompileResult, RenderSectionCompiler, RenderSectionNeighborReadiness,
    RenderSectionRemovalMode, RenderSectionSyncPlan, RenderSectionViewSync,
    actor_instances_from_presentations, build_client_textured_sections,
    build_render_sections_from_snapshots_with_biome_zoom_seed,
    decode_textured_render_section_build_report, encode_textured_render_section_build_report,
    render_section_chunk_pos, render_section_neighbor_readiness, snapshot_contains_render_section,
    summarize_textured_render_section_build_report,
};
use mclone_server::ServerRunnerKind;
use mclone_ui::{
    DebugOverlay, FlatDebugActorCounts, FlatDebugChunkCounts, FlatDebugDrawCounts,
    FlatDebugMeshCounts, FlatDebugOverlay, FlatDebugRenderOptions, FlatDebugRunner,
    FlatDebugTarget, FlatDebugView, FlatHotbarOverlay, FlatHud, FlatHudDebugOverlay,
    GameFramePacingMode, GameHelpParent, GameMovementMode, GameOptionsParent, GamePlayerModel,
    GameScreen, GameTouchSettings, GameUiAction, GameUiHost, GameUiRenderState, GuiKey, GuiScale,
    Point, StatusOverlay, TouchJoystickOverlay, TouchOverlay,
};

const CANVAS_OK_BIT: u32 = 1 << 0;
const CANVAS_RENDERED_BIT: u32 = 1 << 1;
const CANVAS_CONFIGURED_BIT: u32 = 1 << 2;
const CANVAS_WIDTH_SHIFT: u32 = 8;
const CANVAS_HEIGHT_SHIFT: u32 = 20;
const WEB_GROUND_PROBE_DISTANCE: f64 = 0.01;
const WEB_FRAME_UPDATE_DRAIN_BUDGET: usize = 1;
const WEB_MIN_RENDER_DISTANCE: i32 = 1;
const WEB_MAX_RENDER_DISTANCE: i32 = 16;
const WEB_DEFAULT_RENDER_DISTANCE: u32 = 3;
const WEB_FIXED_FPS_CAP: u32 = 60;
const WEB_QUERY_WORLD_STORAGE: &str = "worldStorage";
const WEB_QUERY_WORLD_ID: &str = "worldId";
const WEB_QUERY_CLEAR_WORLD_STORAGE: &str = "clearWorldStorage";
const WEB_TOUCH_LOOK_SENSITIVITY_DEFAULT: f32 = 2.4;
const WEB_TOUCH_LOOK_SENSITIVITY_MIN: f32 = 0.5;
const WEB_TOUCH_LOOK_SENSITIVITY_MAX: f32 = 5.0;
// 067 Stage 3: web drives the shared streaming loop at the same per-frame increment as
// desktop (`DEFAULT_RENDER_CHUNK_MESH_BUDGET`). One small job in flight per frame; the
// resident cache stays visible while movement fills progressively.
const WEB_RENDER_CHUNK_MESH_BUDGET: usize = 1;
// Y the overview-frame camera looks down from when streaming a deterministic chunk view;
// only the horizontal component matters for near-camera readiness.
const WEB_OVERVIEW_CAMERA_EYE_Y: f32 = 256.0;
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
pub fn mclone_web_render_generated_chunk_report(
    canvas: HtmlCanvasElement,
    asset_pack_bytes: js_sys::Uint8Array,
) -> js_sys::Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        let mut session = WebChunkRenderSession::new(canvas, asset_pack_bytes.to_vec())
            .await
            .map_err(JsValue::from)?;
        let report = session
            .render_chunk_report_for_center(SMOKE_INITIAL_CENTER, SMOKE_RADIUS_CHUNKS)
            .await
            .map_err(JsValue::from)?;
        report.to_js_value().map_err(JsValue::from)
    })
}

#[wasm_bindgen]
pub async fn mclone_web_create_chunk_render_session(
    canvas: HtmlCanvasElement,
    asset_pack_bytes: js_sys::Uint8Array,
) -> Result<WebChunkRenderSession, JsValue> {
    WebChunkRenderSession::new(canvas, asset_pack_bytes.to_vec())
        .await
        .map_err(JsValue::from)
}

#[wasm_bindgen]
pub async fn mclone_web_create_worker_chunk_render_session(
    canvas: HtmlCanvasElement,
    asset_pack_bytes: js_sys::Uint8Array,
    server_worker_url: String,
    server_job_worker_url: String,
    bindgen_js_url: String,
    bindgen_wasm_url: String,
) -> Result<WebChunkRenderSession, JsValue> {
    WebChunkRenderSession::new_with_worker(
        canvas,
        asset_pack_bytes.to_vec(),
        WebIntegratedServerRunnerConfig::new(
            SMOKE_SEED,
            server_worker_url,
            server_job_worker_url,
            bindgen_js_url,
            bindgen_wasm_url,
        ),
    )
    .await
    .map_err(JsValue::from)
}

#[wasm_bindgen]
pub async fn mclone_web_create_worker_chunk_render_session_with_startup(
    canvas: HtmlCanvasElement,
    asset_pack_bytes: js_sys::Uint8Array,
    seed: i64,
    initial_center_x: i32,
    initial_center_z: i32,
    movement_speed_multiplier: f32,
    section_occlusion_culling: bool,
    force_fullbright: bool,
    render_color_profile: String,
    server_worker_url: String,
    server_job_worker_url: String,
    bindgen_js_url: String,
    bindgen_wasm_url: String,
    world_storage: String,
    world_id: String,
    clear_world_storage: bool,
) -> Result<WebChunkRenderSession, JsValue> {
    let render_options = startup_render_options(
        section_occlusion_culling,
        force_fullbright,
        &render_color_profile,
    )
    .map_err(JsValue::from)?;
    let runner_config = web_integrated_server_runner_config_with_storage(
        WebIntegratedServerRunnerConfig::new(
            seed,
            server_worker_url,
            server_job_worker_url,
            bindgen_js_url,
            bindgen_wasm_url,
        ),
        &world_storage,
        &world_id,
        clear_world_storage,
    )
    .map_err(JsValue::from)?;
    WebChunkRenderSession::new_with_worker_at(
        canvas,
        asset_pack_bytes.to_vec(),
        runner_config,
        ChunkPos {
            x: initial_center_x,
            z: initial_center_z,
        },
        movement_speed_multiplier,
        render_options,
    )
    .await
    .map_err(JsValue::from)
}

#[wasm_bindgen]
pub async fn mclone_web_create_remote_chunk_render_session(
    canvas: HtmlCanvasElement,
    asset_pack_bytes: js_sys::Uint8Array,
    websocket_url: String,
) -> Result<WebChunkRenderSession, JsValue> {
    WebChunkRenderSession::new_with_remote_websocket(
        canvas,
        asset_pack_bytes.to_vec(),
        websocket_url,
    )
    .await
    .map_err(JsValue::from)
}

#[wasm_bindgen]
pub async fn mclone_web_create_remote_chunk_render_session_with_startup(
    canvas: HtmlCanvasElement,
    asset_pack_bytes: js_sys::Uint8Array,
    websocket_url: String,
    initial_center_x: i32,
    initial_center_z: i32,
    movement_speed_multiplier: f32,
    section_occlusion_culling: bool,
    force_fullbright: bool,
    render_color_profile: String,
) -> Result<WebChunkRenderSession, JsValue> {
    let render_options = startup_render_options(
        section_occlusion_culling,
        force_fullbright,
        &render_color_profile,
    )
    .map_err(JsValue::from)?;
    WebChunkRenderSession::new_with_remote_websocket_at(
        canvas,
        asset_pack_bytes.to_vec(),
        websocket_url,
        ChunkPos {
            x: initial_center_x,
            z: initial_center_z,
        },
        movement_speed_multiplier,
        render_options,
    )
    .await
    .map_err(JsValue::from)
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct GeneratedChunkRenderReport {
    center: ChunkPos,
    radius_chunks: u32,
    camera_state: EngineCameraFrameState,
    width: u32,
    height: u32,
    day_time: u64,
    time_of_day: f32,
    sun_angle: f32,
    sky_rendered: bool,
    command_count: usize,
    update_count: usize,
    snapshot_update_count: usize,
    section_block_update_count: usize,
    unload_update_count: usize,
    loaded_chunk_count: usize,
    loaded_section_key_count: usize,
    resident_section_count: usize,
    drawn_section_count: usize,
    frustum_section_count: usize,
    asset_pack_file_count: usize,
    atlas_width: u32,
    atlas_height: u32,
    atlas_sprite_count: usize,
    vertex_count: u32,
    index_count: u32,
    face_count: u32,
    drawn_index_count: u32,
    drawn_face_count: u32,
    uploaded_section_count: usize,
    removed_section_count: usize,
    uploaded_vertex_count: u32,
    uploaded_index_count: u32,
    uploaded_face_count: u32,
    asset_pack_parse_count: usize,
    terrain_asset_load_count: usize,
    atlas_upload_count: usize,
    mesh_build_count: usize,
    mesh_upload_count: usize,
    render_count: usize,
    gui_command_count: usize,
    flat_hud_retained_rebuild_count: u64,
    flat_hud_retained_cache_hit_count: u64,
    ui_active: bool,
    ui_covers_world: bool,
    ui_screen: &'static str,
    ui_options_parent: Option<&'static str>,
    debug_overlay_visible: bool,
    status_overlay_visible: bool,
    section_occlusion_culling: bool,
    force_fullbright: bool,
    render_color_profile: &'static str,
    compile_request_id: u32,
    pending_compile_job_count: usize,
    view_dirty_chunk_count: usize,
    view_removal_chunk_count: usize,
    loaded_dirty_chunk_count: usize,
    removal_dirty_chunk_count: usize,
    stale_dirty_chunk_count: usize,
    loaded_dirty_section_count: usize,
    removal_dirty_section_count: usize,
    stale_dirty_section_count: usize,
    ready_compile_section_count: usize,
    deferred_compile_section_count: usize,
    budgeted_loaded_chunk_count: usize,
    budgeted_dirty_section_chunk_count: usize,
    submitted_compile_section_count: usize,
    accepted_compile_section_count: usize,
    stale_compile_section_count: usize,
    worker_compile_used: bool,
    worker_packed_byte_length: usize,
    worker_section_count: usize,
    worker_non_empty_section_count: usize,
    worker_visibility_graph_build_count: usize,
    worker_visibility_graph_total_ms: f64,
    worker_visibility_graph_worst_ms: f64,
    worker_vertex_count: u32,
    worker_index_count: u32,
    worker_face_count: u32,
    actor_count: usize,
    drawn_actor_count: usize,
    drawn_actor_index_count: u32,
    actor_atlas_width: u32,
    actor_atlas_height: u32,
    runner_kind: ServerRunnerKind,
    runner_running: bool,
    runner_command_queue_depth: usize,
    runner_update_queue_depth: usize,
    runner_pending_jobs: usize,
    runner_pending_publications: usize,
    worldgen_mailbox_kind: mclone_server::WorldgenMailboxKind,
    light_status_mailbox_kind: mclone_server::LightStatusMailboxKind,
    worldgen_mailbox_pending_jobs: usize,
    light_status_mailbox_pending_statuses: usize,
    runner_frame_metrics: mclone_server::WorkerFrameMetrics,
    worldgen_job_frame_metrics: mclone_server::WorkerFrameMetrics,
    light_status_job_frame_metrics: mclone_server::WorkerFrameMetrics,
    runner_last_simulation_tick: u64,
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

impl GeneratedChunkRenderReport {
    fn to_js_value(self) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        set_bool(&object, "ok", true)?;
        set_bool(&object, "rendered", true)?;
        set_bool(&object, "configured", true)?;
        set_bool(&object, "chunkLoaded", true)?;
        set_bool(&object, "meshBuilt", true)?;
        set_bool(&object, "assetPackLoaded", true)?;
        set_bool(&object, "textured", true)?;
        set_bool(&object, "workerCompileUsed", self.worker_compile_used)?;
        set_bool(&object, "skyRendered", self.sky_rendered)?;
        set_bool(&object, "actorRendered", self.drawn_actor_count > 0)?;
        set_string(&object, "runnerKind", self.runner_kind.label())?;
        set_bool(&object, "runnerRunning", self.runner_running)?;
        write_camera_frame_state_to_js(&object, self.camera_state)?;
        set_number(
            &object,
            "compileRequestId",
            f64::from(self.compile_request_id),
        )?;
        set_number(&object, "centerX", f64::from(self.center.x))?;
        set_number(&object, "centerZ", f64::from(self.center.z))?;
        set_number(&object, "radiusChunks", f64::from(self.radius_chunks))?;
        set_number(&object, "width", f64::from(self.width))?;
        set_number(&object, "height", f64::from(self.height))?;
        set_number(&object, "dayTime", self.day_time as f64)?;
        set_number(&object, "timeOfDay", f64::from(self.time_of_day))?;
        set_number(&object, "sunAngle", f64::from(self.sun_angle))?;
        set_number(&object, "commandCount", self.command_count as f64)?;
        set_number(&object, "updateCount", self.update_count as f64)?;
        set_number(
            &object,
            "snapshotUpdateCount",
            self.snapshot_update_count as f64,
        )?;
        set_number(
            &object,
            "sectionBlockUpdateCount",
            self.section_block_update_count as f64,
        )?;
        set_number(
            &object,
            "unloadUpdateCount",
            self.unload_update_count as f64,
        )?;
        set_number(&object, "loadedChunkCount", self.loaded_chunk_count as f64)?;
        set_number(
            &object,
            "loadedSectionKeyCount",
            self.loaded_section_key_count as f64,
        )?;
        set_number(
            &object,
            "residentSectionCount",
            self.resident_section_count as f64,
        )?;
        set_number(
            &object,
            "drawnSectionCount",
            self.drawn_section_count as f64,
        )?;
        set_number(
            &object,
            "frustumSectionCount",
            self.frustum_section_count as f64,
        )?;
        set_number(
            &object,
            "assetPackFileCount",
            self.asset_pack_file_count as f64,
        )?;
        set_number(&object, "atlasWidth", f64::from(self.atlas_width))?;
        set_number(&object, "atlasHeight", f64::from(self.atlas_height))?;
        set_number(&object, "atlasSpriteCount", self.atlas_sprite_count as f64)?;
        set_number(&object, "vertexCount", f64::from(self.vertex_count))?;
        set_number(&object, "indexCount", f64::from(self.index_count))?;
        set_number(&object, "faceCount", f64::from(self.face_count))?;
        set_number(
            &object,
            "drawnIndexCount",
            f64::from(self.drawn_index_count),
        )?;
        set_number(&object, "drawnFaceCount", f64::from(self.drawn_face_count))?;
        set_number(
            &object,
            "uploadedSectionCount",
            self.uploaded_section_count as f64,
        )?;
        set_number(
            &object,
            "removedSectionCount",
            self.removed_section_count as f64,
        )?;
        set_number(
            &object,
            "uploadedVertexCount",
            f64::from(self.uploaded_vertex_count),
        )?;
        set_number(
            &object,
            "uploadedIndexCount",
            f64::from(self.uploaded_index_count),
        )?;
        set_number(
            &object,
            "uploadedFaceCount",
            f64::from(self.uploaded_face_count),
        )?;
        set_number(
            &object,
            "assetPackParseCount",
            self.asset_pack_parse_count as f64,
        )?;
        set_number(
            &object,
            "terrainAssetLoadCount",
            self.terrain_asset_load_count as f64,
        )?;
        set_number(&object, "atlasUploadCount", self.atlas_upload_count as f64)?;
        set_number(&object, "meshBuildCount", self.mesh_build_count as f64)?;
        set_number(&object, "meshUploadCount", self.mesh_upload_count as f64)?;
        set_number(&object, "renderCount", self.render_count as f64)?;
        set_number(&object, "guiCommandCount", self.gui_command_count as f64)?;
        set_number(
            &object,
            "flatHudRetainedRebuilds",
            self.flat_hud_retained_rebuild_count as f64,
        )?;
        set_number(
            &object,
            "flatHudRetainedCacheHits",
            self.flat_hud_retained_cache_hit_count as f64,
        )?;
        set_bool(&object, "uiActive", self.ui_active)?;
        set_bool(&object, "uiCoversWorld", self.ui_covers_world)?;
        set_string(&object, "uiScreen", self.ui_screen)?;
        if let Some(parent) = self.ui_options_parent {
            set_string(&object, "uiOptionsParent", parent)?;
        }
        set_bool(&object, "debugOverlayVisible", self.debug_overlay_visible)?;
        set_bool(&object, "statusOverlayVisible", self.status_overlay_visible)?;
        set_bool(
            &object,
            "sectionOcclusionCulling",
            self.section_occlusion_culling,
        )?;
        set_bool(&object, "forceFullbright", self.force_fullbright)?;
        set_string(&object, "renderColorProfile", self.render_color_profile)?;
        set_number(
            &object,
            "pendingCompileJobCount",
            self.pending_compile_job_count as f64,
        )?;
        set_number(
            &object,
            "viewDirtyChunkCount",
            self.view_dirty_chunk_count as f64,
        )?;
        set_number(
            &object,
            "viewRemovalChunkCount",
            self.view_removal_chunk_count as f64,
        )?;
        set_number(
            &object,
            "loadedDirtyChunkCount",
            self.loaded_dirty_chunk_count as f64,
        )?;
        set_number(
            &object,
            "removalDirtyChunkCount",
            self.removal_dirty_chunk_count as f64,
        )?;
        set_number(
            &object,
            "staleDirtyChunkCount",
            self.stale_dirty_chunk_count as f64,
        )?;
        set_number(
            &object,
            "loadedDirtySectionCount",
            self.loaded_dirty_section_count as f64,
        )?;
        set_number(
            &object,
            "removalDirtySectionCount",
            self.removal_dirty_section_count as f64,
        )?;
        set_number(
            &object,
            "staleDirtySectionCount",
            self.stale_dirty_section_count as f64,
        )?;
        set_number(
            &object,
            "readyCompileSectionCount",
            self.ready_compile_section_count as f64,
        )?;
        set_number(
            &object,
            "deferredCompileSectionCount",
            self.deferred_compile_section_count as f64,
        )?;
        set_number(
            &object,
            "budgetedLoadedChunkCount",
            self.budgeted_loaded_chunk_count as f64,
        )?;
        set_number(
            &object,
            "budgetedDirtySectionChunkCount",
            self.budgeted_dirty_section_chunk_count as f64,
        )?;
        set_number(
            &object,
            "submittedCompileSectionCount",
            self.submitted_compile_section_count as f64,
        )?;
        set_number(
            &object,
            "acceptedCompileSectionCount",
            self.accepted_compile_section_count as f64,
        )?;
        set_number(
            &object,
            "staleCompileSectionCount",
            self.stale_compile_section_count as f64,
        )?;
        set_number(
            &object,
            "workerPackedByteLength",
            self.worker_packed_byte_length as f64,
        )?;
        set_number(
            &object,
            "workerSectionCount",
            self.worker_section_count as f64,
        )?;
        set_number(
            &object,
            "workerNonEmptySectionCount",
            self.worker_non_empty_section_count as f64,
        )?;
        set_number(
            &object,
            "workerVisibilityGraphBuildCount",
            self.worker_visibility_graph_build_count as f64,
        )?;
        set_number(
            &object,
            "workerVisibilityGraphTotalMs",
            self.worker_visibility_graph_total_ms,
        )?;
        set_number(
            &object,
            "workerVisibilityGraphWorstMs",
            self.worker_visibility_graph_worst_ms,
        )?;
        set_number(
            &object,
            "workerVertexCount",
            f64::from(self.worker_vertex_count),
        )?;
        set_number(
            &object,
            "workerIndexCount",
            f64::from(self.worker_index_count),
        )?;
        set_number(
            &object,
            "workerFaceCount",
            f64::from(self.worker_face_count),
        )?;
        set_number(&object, "actorCount", self.actor_count as f64)?;
        set_number(&object, "drawnActorCount", self.drawn_actor_count as f64)?;
        set_number(
            &object,
            "drawnActorIndexCount",
            f64::from(self.drawn_actor_index_count),
        )?;
        set_number(
            &object,
            "actorAtlasWidth",
            f64::from(self.actor_atlas_width),
        )?;
        set_number(
            &object,
            "actorAtlasHeight",
            f64::from(self.actor_atlas_height),
        )?;
        set_number(
            &object,
            "runnerCommandQueueDepth",
            self.runner_command_queue_depth as f64,
        )?;
        set_number(
            &object,
            "runnerUpdateQueueDepth",
            self.runner_update_queue_depth as f64,
        )?;
        set_number(
            &object,
            "runnerPendingJobs",
            self.runner_pending_jobs as f64,
        )?;
        set_number(
            &object,
            "runnerPendingPublications",
            self.runner_pending_publications as f64,
        )?;
        set_string(
            &object,
            "worldgenMailboxKind",
            self.worldgen_mailbox_kind.label(),
        )?;
        set_string(
            &object,
            "lightStatusMailboxKind",
            self.light_status_mailbox_kind.label(),
        )?;
        set_number(
            &object,
            "worldgenMailboxPendingJobs",
            self.worldgen_mailbox_pending_jobs as f64,
        )?;
        set_number(
            &object,
            "lightStatusMailboxPendingStatuses",
            self.light_status_mailbox_pending_statuses as f64,
        )?;
        set_worker_frame_metrics(&object, "runnerFrameMetrics", self.runner_frame_metrics)?;
        set_worker_frame_metrics(
            &object,
            "worldgenJobFrameMetrics",
            self.worldgen_job_frame_metrics,
        )?;
        set_worker_frame_metrics(
            &object,
            "lightStatusJobFrameMetrics",
            self.light_status_job_frame_metrics,
        )?;
        set_number(
            &object,
            "runnerLastSimulationTick",
            self.runner_last_simulation_tick as f64,
        )?;
        Ok(object.into())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct WebSectionCompileReport {
    worker_compile_used: bool,
    packed_byte_length: usize,
    section_count: usize,
    non_empty_section_count: usize,
    visibility_graph_build_count: usize,
    visibility_graph_total_ms: f64,
    visibility_graph_worst_ms: f64,
    vertex_count: u32,
    index_count: u32,
    face_count: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WebCompileScopeReport {
    view_dirty_chunk_count: usize,
    view_removal_chunk_count: usize,
    loaded_dirty_chunk_count: usize,
    removal_dirty_chunk_count: usize,
    stale_dirty_chunk_count: usize,
    loaded_dirty_section_count: usize,
    removal_dirty_section_count: usize,
    stale_dirty_section_count: usize,
    ready_compile_section_count: usize,
    deferred_compile_section_count: usize,
    budgeted_loaded_chunk_count: usize,
    budgeted_dirty_section_chunk_count: usize,
}

impl WebCompileScopeReport {
    fn from_plan(sync: &RenderSectionViewSync, sync_plan: &RenderSectionSyncPlan) -> Self {
        Self {
            view_dirty_chunk_count: sync.dirty_chunks.len(),
            view_removal_chunk_count: sync.removal_chunks.len(),
            loaded_dirty_chunk_count: sync_plan.dirty_work.loaded_dirty_chunks.len(),
            removal_dirty_chunk_count: sync_plan.dirty_work.removal_dirty_chunks.len(),
            stale_dirty_chunk_count: sync_plan.dirty_work.stale_dirty_chunks.len(),
            loaded_dirty_section_count: sync_plan
                .dirty_work
                .loaded_dirty_sections_by_chunk
                .values()
                .map(BTreeSet::len)
                .sum(),
            removal_dirty_section_count: sync_plan.dirty_work.removal_dirty_sections.len(),
            stale_dirty_section_count: sync_plan.dirty_work.stale_dirty_sections.len(),
            ready_compile_section_count: sync_plan.ready_plan.ready_section_keys.len(),
            deferred_compile_section_count: sync_plan.ready_plan.deferred_section_keys.len(),
            budgeted_loaded_chunk_count: sync_plan.ready_plan.budgeted_loaded_chunks.len(),
            budgeted_dirty_section_chunk_count: sync_plan
                .ready_plan
                .budgeted_dirty_section_chunks
                .len(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct WebChunkRenderPlan {
    sync: RenderSectionViewSync,
    sync_plan: RenderSectionSyncPlan,
    scope: WebCompileScopeReport,
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
    // The request id + outcome of the most recent compile drained by
    // `try_recv_completed`, consumed by the streaming frame so JS can correlate the
    // applied result with the doorbell it posted for that request.
    last_completed: Option<WebCompletedCompile>,
}

#[derive(Clone, Copy, Debug)]
struct WebCompletedCompile {
    request_id: u32,
    ok: bool,
    packed_byte_length: usize,
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
            last_completed: None,
        }
    }

    fn shared_supported(&self) -> bool {
        self.shared.is_some()
    }

    fn in_flight_request_id(&self) -> Option<u32> {
        self.in_flight
            .as_ref()
            .map(|in_flight| in_flight.request_id)
    }

    /// Take the most recently drained compile outcome, if any. The streaming frame reads
    /// this right after the shared sync to learn which doorbell-posted request finished
    /// this frame (single in flight, so at most one).
    fn take_last_completed(&mut self) -> Option<WebCompletedCompile> {
        self.last_completed.take()
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
                self.last_completed = Some(WebCompletedCompile {
                    request_id: in_flight.request_id,
                    ok: true,
                    packed_byte_length: packed.len(),
                });
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
                self.last_completed = Some(WebCompletedCompile {
                    request_id: in_flight.request_id,
                    ok: false,
                    packed_byte_length: 0,
                });
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
                self.last_completed = Some(WebCompletedCompile {
                    request_id: in_flight.request_id,
                    ok: false,
                    packed_byte_length: 0,
                });
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WebBlockInteractionKind {
    Break,
    Place,
}

impl WebBlockInteractionKind {
    fn from_label(label: &str) -> Result<Self, String> {
        match label {
            "break" => Ok(Self::Break),
            "place" => Ok(Self::Place),
            _ => Err(format!(
                "unknown browser block interaction {label:?}; expected \"break\" or \"place\""
            )),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Break => "break",
            Self::Place => "place",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct WebBlockInteractionReport {
    kind: WebBlockInteractionKind,
    hit: BlockHitResult,
    hit_block_state: Option<BlockStateId>,
    result_block_state: Option<BlockStateId>,
    selected_hotbar_slot: u8,
    carried_item_synced: bool,
    command_sent: bool,
    changed: bool,
    command_count_delta: usize,
    update_count_delta: usize,
    snapshot_update_count_delta: usize,
    section_block_update_count_delta: usize,
    unload_update_count_delta: usize,
    interaction_command_count: usize,
    interaction_update_count: usize,
    total_command_count: usize,
    total_update_count: usize,
    total_snapshot_update_count: usize,
    total_section_block_update_count: usize,
    total_unload_update_count: usize,
    pending_compile_job_count: usize,
}

impl WebBlockInteractionReport {
    fn to_js_value(self) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        let block_hit = self.hit.hit_type() == HitResultType::Block;
        set_bool(&object, "ok", true)?;
        set_string(&object, "action", self.kind.label())?;
        set_bool(&object, "hit", block_hit)?;
        set_string(&object, "hitType", if block_hit { "block" } else { "miss" })?;
        set_bool(&object, "inside", self.hit.inside)?;
        set_bool(&object, "carriedItemSynced", self.carried_item_synced)?;
        set_bool(&object, "commandSent", self.command_sent)?;
        set_bool(&object, "changed", self.changed)?;
        set_number(
            &object,
            "selectedHotbarSlot",
            f64::from(self.selected_hotbar_slot),
        )?;
        set_string(&object, "direction", direction_label(self.hit.direction))?;
        set_number(&object, "blockX", f64::from(self.hit.block_pos.x))?;
        set_number(&object, "blockY", f64::from(self.hit.block_pos.y))?;
        set_number(&object, "blockZ", f64::from(self.hit.block_pos.z))?;
        set_number(&object, "hitX", self.hit.location.x)?;
        set_number(&object, "hitY", self.hit.location.y)?;
        set_number(&object, "hitZ", self.hit.location.z)?;
        set_number(
            &object,
            "hitBlockStateId",
            optional_block_state_id(self.hit_block_state),
        )?;
        set_number(
            &object,
            "resultBlockStateId",
            optional_block_state_id(self.result_block_state),
        )?;
        set_number(
            &object,
            "commandCountDelta",
            self.command_count_delta as f64,
        )?;
        set_number(&object, "updateCountDelta", self.update_count_delta as f64)?;
        set_number(
            &object,
            "snapshotUpdateCountDelta",
            self.snapshot_update_count_delta as f64,
        )?;
        set_number(
            &object,
            "sectionBlockUpdateCountDelta",
            self.section_block_update_count_delta as f64,
        )?;
        set_number(
            &object,
            "unloadUpdateCountDelta",
            self.unload_update_count_delta as f64,
        )?;
        set_number(
            &object,
            "interactionCommandCount",
            self.interaction_command_count as f64,
        )?;
        set_number(
            &object,
            "interactionUpdateCount",
            self.interaction_update_count as f64,
        )?;
        set_number(&object, "commandCount", self.total_command_count as f64)?;
        set_number(&object, "updateCount", self.total_update_count as f64)?;
        set_number(
            &object,
            "snapshotUpdateCount",
            self.total_snapshot_update_count as f64,
        )?;
        set_number(
            &object,
            "sectionBlockUpdateCount",
            self.total_section_block_update_count as f64,
        )?;
        set_number(
            &object,
            "unloadUpdateCount",
            self.total_unload_update_count as f64,
        )?;
        set_number(
            &object,
            "pendingCompileJobCount",
            self.pending_compile_job_count as f64,
        )?;
        Ok(object.into())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct WebBlockTargetReport {
    hit: BlockHitResult,
    hit_block_state: Option<BlockStateId>,
    selected_hotbar_slot: u8,
    total_command_count: usize,
    total_update_count: usize,
    pending_compile_job_count: usize,
}

impl WebBlockTargetReport {
    fn to_js_value(self) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        let block_hit = self.hit.hit_type() == HitResultType::Block;
        set_bool(&object, "ok", true)?;
        set_bool(&object, "hit", block_hit)?;
        set_string(&object, "hitType", if block_hit { "block" } else { "miss" })?;
        set_bool(&object, "inside", self.hit.inside)?;
        set_number(
            &object,
            "selectedHotbarSlot",
            f64::from(self.selected_hotbar_slot),
        )?;
        set_string(&object, "direction", direction_label(self.hit.direction))?;
        set_number(&object, "blockX", f64::from(self.hit.block_pos.x))?;
        set_number(&object, "blockY", f64::from(self.hit.block_pos.y))?;
        set_number(&object, "blockZ", f64::from(self.hit.block_pos.z))?;
        set_number(&object, "hitX", self.hit.location.x)?;
        set_number(&object, "hitY", self.hit.location.y)?;
        set_number(&object, "hitZ", self.hit.location.z)?;
        set_number(
            &object,
            "hitBlockStateId",
            optional_block_state_id(self.hit_block_state),
        )?;
        set_number(&object, "commandCount", self.total_command_count as f64)?;
        set_number(&object, "updateCount", self.total_update_count as f64)?;
        set_number(
            &object,
            "pendingCompileJobCount",
            self.pending_compile_job_count as f64,
        )?;
        Ok(object.into())
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

#[wasm_bindgen]
pub struct WebChunkRenderSession {
    context: WebCanvasContext,
    depth: ChunkDepthTarget,
    sky: SkyRenderer,
    runtime: WebRuntime,
    session: GameSessionCoordinator<()>,
    camera: EngineCameraController,
    interaction: ClientInteractionController,
    actor_interpolation: ActorInterpolationState,
    mesh_assets: WebTexturedMeshAssets,
    draw: Option<TexturedSectionDrawResources>,
    actors: ActorDrawResources,
    gui: GuiRenderer,
    ui: GameUiHost,
    debug_overlay_visible: bool,
    status_overlay: StatusOverlay,
    section_occlusion_culling: bool,
    force_fullbright: bool,
    player_collision_box_visible: bool,
    crosshair_visible: bool,
    player_model: GamePlayerModel,
    render_color_profile: RenderColorProfile,
    input_preferences: InputPreferences,
    touch_look_sensitivity: f32,
    touch_settings_available: bool,
    touch_overlay: TouchOverlay,
    loaded_chunk_positions: BTreeSet<ChunkPos>,
    // 067 Stage 3: the last chunk-view center we asked the runner to stream. The
    // streaming loop only re-sends the deferred SetChunkView command when this changes,
    // so movement does not spam the runner with identical view requests every frame.
    interest_center: Option<ChunkPos>,
    asset_pack_parse_count: usize,
    terrain_asset_load_count: usize,
    atlas_upload_count: usize,
    mesh_build_count: usize,
    mesh_upload_count: usize,
    render_count: usize,
    render_compiler: WebRenderSectionCompiler,
}

#[wasm_bindgen]
impl WebChunkRenderSession {
    #[wasm_bindgen(js_name = pendingChunkRenderCompileJobCount)]
    pub fn pending_chunk_render_compile_job_count(&self) -> usize {
        self.render_compiler.pending_job_count()
    }

    /// Whether the resident shared-ring render-compile transport (067 Stage 2) is
    /// available. False on non-cross-origin-isolated browsers without
    /// `SharedArrayBuffer`/`Atomics`, where JS falls back to the labeled
    /// message-transfer begin/finish path.
    #[wasm_bindgen(js_name = renderCompilerSharedSupported)]
    pub fn render_compiler_shared_supported(&self) -> bool {
        self.render_compiler.shared_supported()
    }

    #[wasm_bindgen(js_name = uiStatus)]
    pub fn ui_status(&self) -> Result<JsValue, JsValue> {
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setDebugOverlayVisible)]
    pub fn set_debug_overlay_visible(&mut self, visible: bool) -> Result<JsValue, JsValue> {
        self.debug_overlay_visible = visible;
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setStatusOverlay)]
    pub fn set_status_overlay(
        &mut self,
        message: &str,
        ok: bool,
        visible: bool,
    ) -> Result<JsValue, JsValue> {
        self.status_overlay = if visible {
            StatusOverlay {
                message: message.to_owned(),
                ok,
                visible: true,
            }
        } else {
            StatusOverlay::hidden()
        };
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setTouchLookSensitivity)]
    pub fn set_touch_look_sensitivity(
        &mut self,
        look_sensitivity: f32,
        available: bool,
    ) -> Result<JsValue, JsValue> {
        self.touch_look_sensitivity = clamp_touch_look_sensitivity(look_sensitivity);
        self.touch_settings_available = available;
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setTouchControlsMode)]
    pub fn set_touch_controls_mode(&mut self, mode: &str) -> Result<JsValue, JsValue> {
        self.input_preferences.touch_controls = parse_touch_controls_mode(mode)
            .ok_or_else(|| JsValue::from(format!("invalid touch controls mode: {mode}")))?;
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setTouchControlsOverlay)]
    #[allow(clippy::too_many_arguments)]
    pub fn set_touch_controls_overlay(
        &mut self,
        visible: bool,
        movement_active: bool,
        base_pixel_x: f64,
        base_pixel_y: f64,
        thumb_pixel_x: f64,
        thumb_pixel_y: f64,
        jump_pressed: bool,
        sprint_pressed: bool,
        descend_pressed: bool,
        menu_pressed: bool,
    ) -> Result<JsValue, JsValue> {
        self.touch_overlay = TouchOverlay {
            visible,
            menu_pressed,
            movement: TouchJoystickOverlay {
                active: movement_active,
                base: self.ui_point_from_canvas_pixels(base_pixel_x, base_pixel_y),
                thumb: self.ui_point_from_canvas_pixels(thumb_pixel_x, thumb_pixel_y),
            },
            jump_pressed,
            sprint_pressed,
            descend_pressed,
            interaction_visible: false,
            attack_pressed: false,
            use_pressed: false,
            hotbar_visible: false,
            selected_hotbar_slot: self.interaction.selected_hotbar_slot(),
            hotbar_pressed_slot: None,
            hotbar_icons: mclone_ui::EMPTY_HOTBAR_ICONS,
        };
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = startLocalWorld)]
    pub async fn start_local_world(
        &mut self,
        seed: i64,
        worker_url: String,
        job_worker_url: String,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
    ) -> Result<JsValue, JsValue> {
        let request = SessionStartRequest::NewLocalWorld { seed };
        self.session.begin_start(request.clone());
        let config = WebIntegratedServerRunnerConfig::new(
            seed,
            worker_url,
            job_worker_url,
            bindgen_js_url,
            bindgen_wasm_url,
        );
        match WebRuntime::web_worker_integrated(config).await {
            Ok(runtime) => {
                self.install_started_runtime(request, runtime)
                    .map_err(JsValue::from)?;
            }
            Err(error) => {
                self.fail_session_start(request, error);
            }
        }
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = joinRemoteWebSocket)]
    pub async fn join_remote_websocket(
        &mut self,
        websocket_url: String,
    ) -> Result<JsValue, JsValue> {
        let request = SessionStartRequest::JoinRemote {
            endpoint: RemoteSessionEndpoint::new(websocket_url.clone()),
        };
        self.session.begin_start(request.clone());
        self.ui.set_join_remote_addr(websocket_url.clone());
        match WebRuntime::websocket_remote(websocket_url).await {
            Ok(runtime) => {
                self.install_started_runtime(request, runtime)
                    .map_err(JsValue::from)?;
            }
            Err(error) => {
                self.fail_session_start(request, error);
            }
        }
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = openTitleUi)]
    pub fn open_title_ui(&mut self) -> Result<JsValue, JsValue> {
        self.ui.set_screen(Some(GameScreen::Title));
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = openPauseUi)]
    pub fn open_pause_ui(&mut self) -> Result<JsValue, JsValue> {
        self.ui.set_screen(Some(GameScreen::Pause));
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = openHelpUi)]
    pub fn open_help_ui(&mut self) -> Result<JsValue, JsValue> {
        self.ui
            .apply_action(GameUiAction::OpenHelp(GameHelpParent::Game));
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = openOptionsUi)]
    pub fn open_options_ui(&mut self, parent: &str) -> Result<JsValue, JsValue> {
        let parent = match parent {
            "title" => GameOptionsParent::Title,
            "pause" => GameOptionsParent::Pause,
            other => {
                return Err(JsValue::from_str(&format!(
                    "unknown native UI options parent {other:?}"
                )));
            }
        };
        self.ui.set_screen(Some(GameScreen::Options { parent }));
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = closeUi)]
    pub fn close_ui(&mut self) -> Result<JsValue, JsValue> {
        self.ui.close();
        self.ui_status_to_js_value().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = handleUiKey)]
    pub fn handle_ui_key(&mut self, key: &str) -> Result<JsValue, JsValue> {
        let key = gui_key_from_label(key)
            .ok_or_else(|| JsValue::from_str(&format!("unknown native UI key {key:?}")))?;
        let (handled, action) = self.ui.key_pressed(key);
        self.apply_ui_event_result(handled, action)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = handleUiPointerMove)]
    pub fn handle_ui_pointer_move(
        &mut self,
        pixel_x: f64,
        pixel_y: f64,
        radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        let point = self.ui_point_from_canvas_pixels(pixel_x, pixel_y);
        let _ = radius_chunks;
        let (handled, action) = self.ui.pointer_move(point);
        self.apply_ui_event_result(handled, action)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = handleUiPointerDown)]
    pub fn handle_ui_pointer_down(
        &mut self,
        pixel_x: f64,
        pixel_y: f64,
        radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        let point = self.ui_point_from_canvas_pixels(pixel_x, pixel_y);
        let _ = radius_chunks;
        let handled = self.ui.pointer_down(point);
        self.apply_ui_event_result(handled, None)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = handleUiPointerUp)]
    pub fn handle_ui_pointer_up(
        &mut self,
        pixel_x: f64,
        pixel_y: f64,
        radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        let point = self.ui_point_from_canvas_pixels(pixel_x, pixel_y);
        let _ = radius_chunks;
        let (handled, action) = self.ui.pointer_up(point);
        self.apply_ui_event_result(handled, action)
            .map_err(JsValue::from)
    }

    /// 067 Stage 3 keystone: drive one frame of the shared per-frame streaming loop from
    /// the live camera. Updates the interest center, drains runner updates, runs the
    /// shared `sync_render_sections_with_budget` over the resident-ring compiler at budget
    /// 1, and renders the current cache. Returns the frame report plus, when a new compile
    /// was armed this frame, a `doorbell` for JS to post to the worker — the worker writes
    /// the packed result back into the resident ring that the next frame's poll drains.
    #[wasm_bindgen(js_name = syncCameraRenderFrame)]
    pub async fn sync_camera_render_frame(
        &mut self,
        radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        let snapshot = self.camera.snapshot();
        let camera_position = glam_vec3_from_vec3d(snapshot.eye);
        self.sync_render_streaming_frame_async(
            WebFrameCamera::Camera,
            snapshot.chunk_pos,
            camera_position,
            radius_chunks,
        )
        .await
        .map_err(JsValue::from)
    }

    /// 067 Stage 3: drive one frame of the shared streaming loop for a deterministic
    /// top-down overview at a fixed chunk center. The deterministic smoke pumps this to
    /// idle (the web analog of desktop `sync_all_render_sections`), replacing the begin/
    /// finish overview transaction while keeping the same compile topology as the app.
    #[wasm_bindgen(js_name = syncOverviewRenderFrame)]
    pub fn sync_overview_render_frame(
        &mut self,
        center_x: i32,
        center_z: i32,
        radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        let center = ChunkPos {
            x: center_x,
            z: center_z,
        };
        let camera_position = glam::Vec3::new(
            chunk_middle_block_coord(center.x) as f32,
            WEB_OVERVIEW_CAMERA_EYE_Y,
            chunk_middle_block_coord(center.z) as f32,
        );
        self.sync_render_streaming_frame_deferred(
            WebFrameCamera::Overview {
                center,
                radius_chunks,
            },
            center,
            camera_position,
            radius_chunks,
        )
        .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = cameraFrameState)]
    pub fn camera_frame_state(&self) -> Result<JsValue, JsValue> {
        camera_state_to_js_value(&self.camera, &self.interaction).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = advanceCameraFrame)]
    #[allow(clippy::too_many_arguments)]
    pub async fn advance_camera_frame(
        &mut self,
        dt_seconds: f64,
        mouse_delta_x: f64,
        mouse_delta_y: f64,
        keyboard_turn: f64,
        forward: bool,
        backward: bool,
        left: bool,
        right: bool,
        jump: bool,
        descend: bool,
        shift: bool,
        sprint: bool,
        movement_impulse_active: bool,
        movement_left_impulse: f64,
        movement_forward_impulse: f64,
    ) -> Result<JsValue, JsValue> {
        let movement_impulse = if movement_impulse_active {
            Some(EngineCameraMovementImpulse::new(
                movement_left_impulse as f32,
                movement_forward_impulse as f32,
            ))
        } else {
            None
        };
        let input = EngineCameraInput {
            dt_seconds,
            mouse_delta_x: mouse_delta_x
                + keyboard_turn_mouse_delta(keyboard_turn as f32, dt_seconds),
            mouse_delta_y,
            forward,
            backward,
            left,
            right,
            jump,
            descend,
            shift,
            sprint,
            movement_impulse,
            movement_yaw_radians: None,
            ..EngineCameraInput::default()
        };
        self.camera
            .apply_movement_input(self.runtime.client(), input);
        if self.is_remote_websocket_runtime() {
            self.sync_camera_pose_to_server()
                .await
                .map_err(JsValue::from)?;
        } else {
            self.sync_camera_pose_to_server_deferred()
                .map_err(JsValue::from)?;
        }
        camera_state_to_js_value(&self.camera, &self.interaction).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = adjustCameraSpeed)]
    pub fn adjust_camera_speed(&mut self, wheel_amount: f64) -> Result<JsValue, JsValue> {
        self.camera.adjust_speed(wheel_amount);
        camera_state_to_js_value(&self.camera, &self.interaction).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = toggleMovementMode)]
    pub fn toggle_movement_mode(&mut self) -> Result<JsValue, JsValue> {
        self.camera.toggle_movement_mode();
        camera_state_to_js_value(&self.camera, &self.interaction).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = selectHotbarSlot)]
    pub fn select_hotbar_slot(&mut self, slot: u8) -> Result<JsValue, JsValue> {
        self.interaction.select_hotbar_slot(slot);
        hotbar_state_to_js_value(&self.interaction).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = previewBlockTarget)]
    pub fn preview_block_target(&self) -> Result<JsValue, JsValue> {
        self.preview_block_target_report()
            .to_js_value()
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = interactBlock)]
    pub async fn interact_block(&mut self, kind: &str) -> Result<JsValue, JsValue> {
        let kind = WebBlockInteractionKind::from_label(kind).map_err(JsValue::from)?;
        self.interact_block_with_kind(kind)
            .await
            .and_then(WebBlockInteractionReport::to_js_value)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = resizeCanvas)]
    pub fn resize_canvas(&mut self, width: u32, height: u32) -> Result<JsValue, JsValue> {
        let changed = self.context.resize(width, height);
        if changed {
            self.depth.resize(
                &self.context.device,
                self.context.width,
                self.context.height,
            );
            self.ui.set_scale(GuiScale::from_pixels(
                self.context.width,
                self.context.height,
            ));
        }
        canvas_size_to_js_value(self.context.width, self.context.height, changed)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = blockStateAt)]
    pub fn block_state_at(&self, x: i32, y: i32, z: i32) -> Result<JsValue, JsValue> {
        let object = js_sys::Object::new();
        let state = self
            .runtime
            .client()
            .block_state_at_block_pos(BlockPos::new(x, y, z));
        set_bool(&object, "ok", true).map_err(JsValue::from)?;
        set_bool(&object, "loaded", state.is_some()).map_err(JsValue::from)?;
        set_number(&object, "blockX", f64::from(x)).map_err(JsValue::from)?;
        set_number(&object, "blockY", f64::from(y)).map_err(JsValue::from)?;
        set_number(&object, "blockZ", f64::from(z)).map_err(JsValue::from)?;
        set_number(&object, "blockStateId", optional_block_state_id(state))
            .map_err(JsValue::from)?;
        Ok(object.into())
    }

    #[wasm_bindgen(js_name = shutdown)]
    pub fn shutdown(&mut self) -> Result<JsValue, JsValue> {
        self.runtime.request_shutdown();
        shutdown_report_to_js(self.runtime.runner_kind())
    }

    #[wasm_bindgen(js_name = shutdownAsync)]
    pub async fn shutdown_async(&mut self) -> Result<JsValue, JsValue> {
        self.runtime
            .shutdown_gracefully()
            .await
            .map_err(JsValue::from)?;
        shutdown_report_to_js(self.runtime.runner_kind())
    }
}

fn shutdown_report_to_js(runner_kind: ServerRunnerKind) -> Result<JsValue, JsValue> {
    let object = js_sys::Object::new();
    set_bool(&object, "ok", true).map_err(JsValue::from)?;
    set_string(&object, "runnerKind", runner_kind.label()).map_err(JsValue::from)?;
    Ok(object.into())
}

impl WebChunkRenderSession {
    fn ui_status_to_js_value(&self) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        self.write_ui_status_to_js_object(&object)?;
        Ok(object.into())
    }

    fn write_ui_status_to_js_object(&self, object: &js_sys::Object) -> Result<(), String> {
        set_bool(object, "ok", true)?;
        set_bool(object, "active", self.ui.is_active())?;
        set_bool(object, "coversWorld", self.ui.covers_world())?;
        set_string(object, "screen", ui_screen_label(self.ui.screen()))?;
        set_string(object, "uiScreen", ui_screen_label(self.ui.screen()))?;
        if let Some(GameScreen::Options { parent }) = self.ui.screen() {
            set_string(object, "optionsParent", options_parent_label(parent))?;
            set_string(object, "uiOptionsParent", options_parent_label(parent))?;
        }
        set_bool(
            object,
            "sectionOcclusionCulling",
            self.section_occlusion_culling,
        )?;
        set_bool(object, "forceFullbright", self.force_fullbright)?;
        set_bool(object, "crosshairVisible", self.crosshair_visible)?;
        set_string(object, "playerModel", self.player_model.label())?;
        set_string(
            object,
            "renderColorProfile",
            self.render_color_profile.as_str(),
        )?;
        set_bool(object, "debugOverlayVisible", self.debug_overlay_visible)?;
        let status_overlay = self.effective_status_overlay();
        set_bool(
            object,
            "statusOverlayVisible",
            status_overlay.visible && !status_overlay.message.is_empty(),
        )?;
        set_bool(object, "statusOverlayOk", status_overlay.ok)?;
        if status_overlay.visible && !status_overlay.message.is_empty() {
            set_string(object, "statusOverlayMessage", &status_overlay.message)?;
        }
        write_web_session_status_to_js_object(object, &self.session)?;
        set_bool(
            object,
            "touchLookSensitivityAvailable",
            self.touch_settings_available,
        )?;
        set_bool(
            object,
            "touchControlsOverlayVisible",
            self.touch_overlay.visible,
        )?;
        set_string(
            object,
            "touchControlsMode",
            touch_controls_mode_js_label(self.input_preferences.touch_controls),
        )?;
        set_bool(
            object,
            "touchControlsVisible",
            self.resolved_flat_input_for_hud().touch_controls_visible,
        )?;
        if self.touch_settings_available {
            set_number(
                object,
                "touchLookSensitivity",
                f64::from(self.touch_look_sensitivity),
            )?;
        }
        Ok(())
    }

    fn ui_point_from_canvas_pixels(&self, pixel_x: f64, pixel_y: f64) -> Point {
        self.ui.scale().client_to_gui(pixel_x, pixel_y)
    }

    fn apply_ui_event_result(
        &mut self,
        handled: bool,
        action: Option<GameUiAction>,
    ) -> Result<JsValue, String> {
        if let Some(action) = action {
            self.apply_web_ui_action(action);
        }
        self.ui_event_result_to_js_value(handled, action)
    }

    fn apply_web_ui_action(&mut self, action: GameUiAction) {
        match action {
            GameUiAction::ToggleSectionOcclusion => {
                self.section_occlusion_culling = !self.section_occlusion_culling;
            }
            GameUiAction::ToggleFullbright => {
                self.force_fullbright = !self.force_fullbright;
            }
            GameUiAction::ToggleFarLod => {}
            GameUiAction::SetFarLodRange(_) => {}
            GameUiAction::TogglePlayerCollisionBox => {
                self.player_collision_box_visible = !self.player_collision_box_visible;
            }
            GameUiAction::ToggleCrosshair => {
                self.crosshair_visible = !self.crosshair_visible;
            }
            GameUiAction::ToggleFirstPersonPlayer => {
                let visible = !self.camera.first_person_player_visible();
                self.camera.set_first_person_player_visible(visible);
            }
            GameUiAction::SetTouchLookSensitivity(look_sensitivity) => {
                self.touch_look_sensitivity = clamp_touch_look_sensitivity(look_sensitivity);
            }
            GameUiAction::SetTouchControlsMode(mode) => {
                self.input_preferences.touch_controls = mode;
            }
            GameUiAction::SetMovementMode(movement_mode) => {
                self.camera
                    .set_movement_mode(engine_movement_mode(movement_mode));
            }
            GameUiAction::SetXrTurnMode(_) => {}
            GameUiAction::SetFlySpeed(multiplier) => {
                self.camera.set_fly_speed_multiplier(f64::from(multiplier));
            }
            GameUiAction::SetMovementSpeed(multiplier) => {
                self.camera
                    .set_movement_speed_multiplier(f64::from(multiplier));
            }
            GameUiAction::SetPlayerModel(model) => {
                self.player_model = model;
                if let Err(error) = self.sync_player_appearance_deferred() {
                    web_sys::console::warn_1(
                        &format!("failed to sync browser player appearance: {error}").into(),
                    );
                }
            }
            GameUiAction::AssignHotbarBlock { slot, block_state } => {
                if let Some(command) = self
                    .interaction
                    .set_debug_hotbar_slot(slot, Some(BlockStateId(block_state)))
                {
                    let _ = self.runtime.send_gameplay_command_deferred(command);
                }
            }
            GameUiAction::CreateWorld(seed) => {
                self.session
                    .begin_start(SessionStartRequest::NewLocalWorld { seed });
                self.status_overlay = StatusOverlay::hidden();
            }
            GameUiAction::JoinRemote => {
                self.session.begin_start(SessionStartRequest::JoinRemote {
                    endpoint: RemoteSessionEndpoint::new(self.ui.join_remote_addr().to_owned()),
                });
                self.status_overlay = StatusOverlay::hidden();
            }
            GameUiAction::Quit => {
                self.runtime.request_shutdown();
            }
            GameUiAction::StartWorld
            | GameUiAction::OpenBlockPalette
            | GameUiAction::OpenHelp(_)
            | GameUiAction::CloseHelp(_)
            | GameUiAction::OpenNewWorld
            | GameUiAction::OpenJoinRemote
            | GameUiAction::OpenServerSettings(_)
            | GameUiAction::RerollSeed
            | GameUiAction::Resume
            | GameUiAction::OpenOptions(_)
            | GameUiAction::BackToTitle
            | GameUiAction::BackToPause
            | GameUiAction::QuitToTitle
            | GameUiAction::CycleFramePacing
            | GameUiAction::CycleFpsCap
            | GameUiAction::SetServerSimulationCadence(_)
            | GameUiAction::SetRenderDistance(_) => {}
        }
        self.ui.apply_action(action);
    }

    fn ui_event_result_to_js_value(
        &self,
        handled: bool,
        action: Option<GameUiAction>,
    ) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        self.write_ui_status_to_js_object(&object)?;
        set_bool(&object, "handled", handled)?;
        if let Some(action) = action {
            set_string(&object, "action", ui_action_label(action))?;
            match action {
                GameUiAction::OpenOptions(parent) => {
                    set_string(&object, "actionParent", options_parent_label(parent))?;
                }
                GameUiAction::CreateWorld(seed) => {
                    set_number(&object, "worldSeed", seed as f64)?;
                    set_string(&object, "worldSeedText", &seed.to_string())?;
                }
                GameUiAction::JoinRemote => {
                    set_string(&object, "remoteEndpoint", self.ui.join_remote_addr())?;
                }
                GameUiAction::SetRenderDistance(render_distance) => {
                    set_number(&object, "renderDistance", f64::from(render_distance))?;
                }
                GameUiAction::SetTouchLookSensitivity(look_sensitivity) => {
                    set_number(
                        &object,
                        "touchLookSensitivity",
                        f64::from(clamp_touch_look_sensitivity(look_sensitivity)),
                    )?;
                }
                GameUiAction::SetTouchControlsMode(mode) => {
                    set_string(
                        &object,
                        "touchControlsMode",
                        touch_controls_mode_js_label(mode),
                    )?;
                }
                GameUiAction::SetServerSimulationCadence(cadence) => {
                    set_string(&object, "serverCadence", &cadence.label())?;
                }
                GameUiAction::AssignHotbarBlock { slot, block_state } => {
                    set_number(&object, "slot", f64::from(slot))?;
                    set_number(&object, "blockState", f64::from(block_state))?;
                }
                GameUiAction::Quit => {
                    set_bool(&object, "shutdownRequested", true)?;
                }
                GameUiAction::SetMovementSpeed(multiplier) => {
                    set_number(&object, "movementSpeedMultiplier", f64::from(multiplier))?;
                }
                GameUiAction::StartWorld
                | GameUiAction::OpenBlockPalette
                | GameUiAction::OpenHelp(_)
                | GameUiAction::CloseHelp(_)
                | GameUiAction::OpenNewWorld
                | GameUiAction::OpenJoinRemote
                | GameUiAction::OpenServerSettings(_)
                | GameUiAction::RerollSeed
                | GameUiAction::Resume
                | GameUiAction::BackToTitle
                | GameUiAction::BackToPause
                | GameUiAction::QuitToTitle
                | GameUiAction::ToggleSectionOcclusion
                | GameUiAction::ToggleFullbright
                | GameUiAction::ToggleFarLod
                | GameUiAction::SetFarLodRange(_)
                | GameUiAction::TogglePlayerCollisionBox
                | GameUiAction::ToggleCrosshair
                | GameUiAction::ToggleFirstPersonPlayer
                | GameUiAction::SetPlayerModel(_)
                | GameUiAction::SetMovementMode(_)
                | GameUiAction::SetXrTurnMode(_)
                | GameUiAction::SetFlySpeed(_)
                | GameUiAction::CycleFramePacing
                | GameUiAction::CycleFpsCap => {}
            }
        }
        Ok(object.into())
    }

    fn ui_render_state(&self, radius_chunks: u32) -> GameUiRenderState {
        GameUiRenderState {
            render_distance: i32::try_from(radius_chunks)
                .unwrap_or(i32::MAX)
                .clamp(WEB_MIN_RENDER_DISTANCE, WEB_MAX_RENDER_DISTANCE),
            min_render_distance: WEB_MIN_RENDER_DISTANCE,
            max_render_distance: WEB_MAX_RENDER_DISTANCE,
            section_occlusion_culling: self.section_occlusion_culling,
            force_fullbright: self.force_fullbright,
            far_lod_enabled: false,
            far_lod_range_chunks:
                mclone_app_runtime::far_lod::DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS as i32,
            min_far_lod_range_chunks:
                mclone_app_runtime::far_lod::MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS as i32,
            max_far_lod_range_chunks:
                mclone_app_runtime::far_lod::MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS as i32,
            player_collision_box_visible: self.player_collision_box_visible,
            first_person_player_visible: self.camera.first_person_player_visible(),
            crosshair_visible: Some(self.crosshair_visible),
            player_model: self.player_model,
            movement_mode: game_movement_mode(self.camera.movement_mode()),
            xr_turn_mode: None,
            fly_speed_multiplier: self.camera.fly_speed_multiplier() as f32,
            min_fly_speed_multiplier: ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER as f32,
            max_fly_speed_multiplier: ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER as f32,
            movement_speed_multiplier: self.camera.movement_speed_multiplier() as f32,
            min_movement_speed_multiplier: ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER as f32,
            max_movement_speed_multiplier: ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER as f32,
            frame_pacing_mode: GameFramePacingMode::Vsync,
            fps_cap: WEB_FIXED_FPS_CAP,
            server_cadence: None,
            touch_controls_mode: Some(self.input_preferences.touch_controls),
            touch_settings: self
                .touch_settings_available
                .then_some(GameTouchSettings::new(
                    self.touch_look_sensitivity,
                    WEB_TOUCH_LOOK_SENSITIVITY_MIN,
                    WEB_TOUCH_LOOK_SENSITIVITY_MAX,
                )),
            block_palette: debug_block_palette_overlay(
                &self.mesh_assets.catalog,
                self.interaction.selected_hotbar_slot(),
            ),
        }
    }

    async fn new(canvas: HtmlCanvasElement, asset_pack_bytes: Vec<u8>) -> Result<Self, String> {
        Self::new_with_runtime(
            canvas,
            asset_pack_bytes,
            SessionStartRequest::NewLocalWorld { seed: SMOKE_SEED },
            WebRuntime::local_integrated(SMOKE_SEED),
            SMOKE_INITIAL_CENTER,
            ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER as f32,
            TexturedSectionRenderOptions::default(),
        )
        .await
    }

    async fn new_with_worker(
        canvas: HtmlCanvasElement,
        asset_pack_bytes: Vec<u8>,
        config: WebIntegratedServerRunnerConfig,
    ) -> Result<Self, String> {
        let seed = config.seed;
        let runtime = WebRuntime::web_worker_integrated(config).await?;
        Self::new_with_runtime(
            canvas,
            asset_pack_bytes,
            SessionStartRequest::NewLocalWorld { seed },
            runtime,
            SMOKE_INITIAL_CENTER,
            ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER as f32,
            TexturedSectionRenderOptions::default(),
        )
        .await
    }

    async fn new_with_worker_at(
        canvas: HtmlCanvasElement,
        asset_pack_bytes: Vec<u8>,
        config: WebIntegratedServerRunnerConfig,
        initial_center: ChunkPos,
        movement_speed_multiplier: f32,
        render_options: TexturedSectionRenderOptions,
    ) -> Result<Self, String> {
        let seed = config.seed;
        let runtime = WebRuntime::web_worker_integrated_at(config, initial_center).await?;
        Self::new_with_runtime(
            canvas,
            asset_pack_bytes,
            SessionStartRequest::NewLocalWorld { seed },
            runtime,
            initial_center,
            movement_speed_multiplier,
            render_options,
        )
        .await
    }

    async fn new_with_remote_websocket(
        canvas: HtmlCanvasElement,
        asset_pack_bytes: Vec<u8>,
        websocket_url: String,
    ) -> Result<Self, String> {
        let runtime = WebRuntime::websocket_remote(websocket_url.clone()).await?;
        Self::new_with_runtime(
            canvas,
            asset_pack_bytes,
            SessionStartRequest::JoinRemote {
                endpoint: RemoteSessionEndpoint::new(websocket_url),
            },
            runtime,
            SMOKE_INITIAL_CENTER,
            ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER as f32,
            TexturedSectionRenderOptions::default(),
        )
        .await
    }

    async fn new_with_remote_websocket_at(
        canvas: HtmlCanvasElement,
        asset_pack_bytes: Vec<u8>,
        websocket_url: String,
        initial_center: ChunkPos,
        movement_speed_multiplier: f32,
        render_options: TexturedSectionRenderOptions,
    ) -> Result<Self, String> {
        let runtime =
            WebRuntime::websocket_remote_at(websocket_url.clone(), initial_center).await?;
        Self::new_with_runtime(
            canvas,
            asset_pack_bytes,
            SessionStartRequest::JoinRemote {
                endpoint: RemoteSessionEndpoint::new(websocket_url),
            },
            runtime,
            initial_center,
            movement_speed_multiplier,
            render_options,
        )
        .await
    }

    async fn new_with_runtime(
        canvas: HtmlCanvasElement,
        asset_pack_bytes: Vec<u8>,
        session_request: SessionStartRequest,
        runtime: WebRuntime,
        initial_center: ChunkPos,
        movement_speed_multiplier: f32,
        render_options: TexturedSectionRenderOptions,
    ) -> Result<Self, String> {
        let mesh_assets = load_textured_mesh_assets_from_pack(asset_pack_bytes)?;
        let render_color_profile = render_options.color_profile;
        let context =
            WebCanvasContext::new_with_color_profile(canvas, render_color_profile).await?;
        let depth = ChunkDepthTarget::new(&context.device, context.width, context.height);
        let sky = SkyRenderer::new_with_color_profile(
            &context.device,
            context.format,
            render_color_profile,
        );
        let actors = ActorDrawResources::new(
            &context.device,
            &context.queue,
            context.format,
            mesh_assets.actor_atlas.as_upload(),
            Some(&mesh_assets.actor_figures),
        )
        .map_err(|error| format!("failed to upload packed actor textures: {error:#}"))?;
        let mut gui = GuiRenderer::new(&context.device, context.format);
        gui.upload_texture_atlas(
            &context.device,
            &context.queue,
            atlas_upload(&mesh_assets.atlas),
        )
        .map_err(|error| format!("failed to upload GUI atlas: {error:#}"))?;
        let mut ui = GameUiHost::new_ingame();
        match &session_request {
            SessionStartRequest::NewLocalWorld { seed } => {
                ui.set_new_world_seed(*seed);
            }
            SessionStartRequest::JoinRemote { endpoint } => {
                ui.set_new_world_seed(SMOKE_SEED);
                ui.set_join_remote_addr(endpoint.address.clone());
            }
            SessionStartRequest::Unknown => {
                ui.set_new_world_seed(SMOKE_SEED);
            }
        }
        ui.set_scale(GuiScale::from_pixels(context.width, context.height));
        let session = started_web_session_coordinator(session_request)?;
        let mut camera = EngineCameraController::spawn_for_chunk(initial_center);
        camera.set_movement_speed_multiplier(f64::from(movement_speed_multiplier));

        Ok(Self {
            context,
            depth,
            sky,
            runtime,
            session,
            camera,
            interaction: ClientInteractionController::new(),
            actor_interpolation: ActorInterpolationState::new(),
            mesh_assets,
            draw: None,
            actors,
            gui,
            ui,
            debug_overlay_visible: false,
            status_overlay: StatusOverlay::hidden(),
            section_occlusion_culling: render_options.section_occlusion_culling,
            force_fullbright: render_options.force_fullbright,
            player_collision_box_visible: false,
            crosshair_visible: true,
            player_model: GamePlayerModel::default(),
            render_color_profile,
            input_preferences: InputPreferences::AUTO,
            touch_look_sensitivity: WEB_TOUCH_LOOK_SENSITIVITY_DEFAULT,
            touch_settings_available: false,
            touch_overlay: TouchOverlay::hidden(),
            loaded_chunk_positions: BTreeSet::new(),
            interest_center: None,
            asset_pack_parse_count: 1,
            terrain_asset_load_count: 1,
            atlas_upload_count: 0,
            mesh_build_count: 0,
            mesh_upload_count: 0,
            render_count: 0,
            render_compiler: WebRenderSectionCompiler::new(),
        })
    }

    fn is_remote_websocket_runtime(&self) -> bool {
        self.runtime.runner_kind() == ServerRunnerKind::RemoteWebSocket
    }

    fn effective_status_overlay(&self) -> StatusOverlay {
        self.session.status().map_or_else(
            || self.status_overlay.clone(),
            |status| StatusOverlay::new(status.message, status.ok),
        )
    }

    fn resolved_flat_input_for_hud(&self) -> mclone_input::ResolvedFlatInput {
        let mut state = InputCapabilityState::new(InputCapabilities {
            keyboard: true,
            mouse: true,
            touch: self.touch_overlay.visible,
            ..InputCapabilities::NONE
        });
        if self.touch_overlay.visible {
            state.note_activity(InputDeviceKind::Touch);
        }
        state.resolve(self.input_preferences)
    }

    fn install_started_runtime(
        &mut self,
        request: SessionStartRequest,
        runtime: WebRuntime,
    ) -> Result<(), String> {
        let descriptor = request
            .active_descriptor()
            .ok_or_else(|| "web session request did not describe an active session".to_owned())?;
        self.runtime.request_shutdown();
        self.runtime = runtime;
        self.session
            .apply_start_result(&Ok(StartedGameSession::new(descriptor, ())));
        self.reset_runtime_view_state(&request);
        self.sync_player_appearance_deferred()?;
        Ok(())
    }

    fn fail_session_start(&mut self, request: SessionStartRequest, error: String) {
        web_sys::console::error_1(
            &format!("failed to start web session {request:?}: {error}").into(),
        );
        self.session
            .fail_start(SessionFailure::new(request.default_failure_message()));
    }

    fn reset_runtime_view_state(&mut self, request: &SessionStartRequest) {
        match request {
            SessionStartRequest::NewLocalWorld { seed } => {
                self.ui.set_new_world_seed(*seed);
            }
            SessionStartRequest::JoinRemote { endpoint } => {
                self.ui.set_join_remote_addr(endpoint.address.clone());
            }
            SessionStartRequest::Unknown => {}
        }
        self.status_overlay = StatusOverlay::hidden();
        let movement_speed_multiplier = self.camera.movement_speed_multiplier();
        self.camera = EngineCameraController::spawn_for_chunk(SMOKE_INITIAL_CENTER);
        self.camera
            .set_movement_speed_multiplier(movement_speed_multiplier);
        self.interaction = ClientInteractionController::new();
        self.actor_interpolation = ActorInterpolationState::new();
        self.draw = None;
        self.loaded_chunk_positions.clear();
        self.interest_center = None;
        self.render_compiler = WebRenderSectionCompiler::new();
        self.mesh_build_count = 0;
        self.mesh_upload_count = 0;
        self.render_count = 0;
    }

    async fn interact_block_with_kind(
        &mut self,
        kind: WebBlockInteractionKind,
    ) -> Result<WebBlockInteractionReport, String> {
        let command_count_before = self.runtime.command_count();
        let update_count_before = self.runtime.update_count();
        let snapshot_update_count_before = self.runtime.snapshot_update_count();
        let section_block_update_count_before = self.runtime.section_block_update_count();
        let unload_update_count_before = self.runtime.unload_update_count();
        self.sync_camera_pose_to_server().await?;
        let carried_item_synced = self.sync_carried_item().await?;

        let hit = self
            .camera
            .pick_block(self.runtime.client(), &self.interaction);
        let hit_block_state = self
            .runtime
            .client()
            .block_state_at_block_pos(hit.block_pos);
        let command = match kind {
            WebBlockInteractionKind::Break => self.interaction.debug_instant_break_command(hit),
            WebBlockInteractionKind::Place => self.interaction.use_item_on_command(hit),
        };
        let Some(command) = command else {
            return Ok(WebBlockInteractionReport {
                kind,
                hit,
                hit_block_state,
                result_block_state: hit_block_state,
                selected_hotbar_slot: self.interaction.selected_hotbar_slot(),
                carried_item_synced,
                command_sent: false,
                changed: false,
                command_count_delta: self.runtime.command_count() - command_count_before,
                update_count_delta: self.runtime.update_count() - update_count_before,
                snapshot_update_count_delta: self.runtime.snapshot_update_count()
                    - snapshot_update_count_before,
                section_block_update_count_delta: self.runtime.section_block_update_count()
                    - section_block_update_count_before,
                unload_update_count_delta: self.runtime.unload_update_count()
                    - unload_update_count_before,
                interaction_command_count: 0,
                interaction_update_count: 0,
                total_command_count: self.runtime.command_count(),
                total_update_count: self.runtime.update_count(),
                total_snapshot_update_count: self.runtime.snapshot_update_count(),
                total_section_block_update_count: self.runtime.section_block_update_count(),
                total_unload_update_count: self.runtime.unload_update_count(),
                pending_compile_job_count: self.render_compiler.pending_job_count(),
            });
        };

        let step = self
            .runtime
            .send_gameplay_command_async(command)
            .await
            .map_err(|error| {
                format!("failed to send browser block interaction command: {error}")
            })?;
        let result_block_state = self
            .runtime
            .client()
            .block_state_at_block_pos(hit.block_pos);
        Ok(WebBlockInteractionReport {
            kind,
            hit,
            hit_block_state,
            result_block_state,
            selected_hotbar_slot: self.interaction.selected_hotbar_slot(),
            carried_item_synced,
            command_sent: true,
            changed: step.update_count > 0,
            command_count_delta: self.runtime.command_count() - command_count_before,
            update_count_delta: self.runtime.update_count() - update_count_before,
            snapshot_update_count_delta: self.runtime.snapshot_update_count()
                - snapshot_update_count_before,
            section_block_update_count_delta: self.runtime.section_block_update_count()
                - section_block_update_count_before,
            unload_update_count_delta: self.runtime.unload_update_count()
                - unload_update_count_before,
            interaction_command_count: step.command_count,
            interaction_update_count: step.update_count,
            total_command_count: self.runtime.command_count(),
            total_update_count: self.runtime.update_count(),
            total_snapshot_update_count: self.runtime.snapshot_update_count(),
            total_section_block_update_count: self.runtime.section_block_update_count(),
            total_unload_update_count: self.runtime.unload_update_count(),
            pending_compile_job_count: self.render_compiler.pending_job_count(),
        })
    }

    fn preview_block_target_report(&self) -> WebBlockTargetReport {
        let hit = self
            .camera
            .pick_block(self.runtime.client(), &self.interaction);
        WebBlockTargetReport {
            hit,
            hit_block_state: self
                .runtime
                .client()
                .block_state_at_block_pos(hit.block_pos),
            selected_hotbar_slot: self.interaction.selected_hotbar_slot(),
            total_command_count: self.runtime.command_count(),
            total_update_count: self.runtime.update_count(),
            pending_compile_job_count: self.render_compiler.pending_job_count(),
        }
    }

    fn debug_overlay(
        &self,
        center: ChunkPos,
        radius_chunks: u32,
        day_time: u64,
        time_of_day: f32,
        render_stats: TexturedSectionRenderStats,
        actor_count: usize,
        actor_stats: mclone_render::entity::ActorRenderStats,
        runner_diagnostics: &mclone_server::ServerRunnerDiagnostics,
    ) -> DebugOverlay {
        let camera_state = self.camera.frame_state(&self.interaction);
        let camera = camera_state.camera;
        let target = self.preview_block_target_report();
        let target = if target.hit.hit_type() == HitResultType::Block {
            Some(FlatDebugTarget::Block {
                x: target.hit.block_pos.x,
                y: target.hit.block_pos.y,
                z: target.hit.block_pos.z,
            })
        } else {
            Some(FlatDebugTarget::Miss)
        };

        let mut overlay = FlatDebugOverlay::new(
            [
                camera.eye.x as f32,
                camera.eye.y as f32,
                camera.eye.z as f32,
            ],
            [camera.chunk_pos.x, camera.chunk_pos.z],
            camera.speed_blocks_per_second as f32,
            camera_state.movement_mode_label(),
            camera_state.on_ground,
            FlatDebugView::with_center(radius_chunks as i32, [center.x, center.z]),
        );
        overlay.runner = Some(FlatDebugRunner::new(
            runner_diagnostics.kind.label().to_ascii_uppercase(),
            runner_diagnostics.command_queue_depth,
            runner_diagnostics.update_queue_depth,
        ));
        overlay.chunks = Some(FlatDebugChunkCounts::loaded_visible(
            self.runtime.client().loaded_chunk_count(),
            self.loaded_chunk_positions.len(),
        ));
        overlay.draw = Some(FlatDebugDrawCounts {
            drawn_sections: render_stats.drawn_section_count,
            section_count: render_stats.loaded_section_count,
            drawn_faces: render_stats.drawn_face_count(),
            face_count: render_stats.loaded_face_count(),
        });
        overlay.actors = Some(FlatDebugActorCounts {
            drawn_actors: actor_stats.drawn_actor_count,
            actor_count,
            drawn_actor_indices: actor_stats.index_count,
        });
        overlay.mesh = Some(FlatDebugMeshCounts {
            build_count: self.mesh_build_count,
            upload_count: self.mesh_upload_count,
            render_count: self.render_count,
        });
        overlay.pending_compile_jobs = Some(self.render_compiler.pending_job_count());
        overlay.day_time = Some(day_time);
        overlay.time_of_day = Some(time_of_day);
        overlay.selected_hotbar_slot = Some(camera_state.selected_hotbar_slot);
        overlay.target = target;
        overlay.render_options = Some(FlatDebugRenderOptions {
            section_occlusion_culling: self.section_occlusion_culling,
            force_fullbright: self.force_fullbright,
            color_profile: self.render_color_profile.label(),
        });
        overlay.to_debug_overlay()
    }

    async fn sync_carried_item(&mut self) -> Result<bool, String> {
        let Some(command) = self.interaction.ensure_has_sent_carried_item() else {
            return Ok(false);
        };
        self.runtime
            .send_gameplay_command_async(command)
            .await
            .map_err(|error| format!("failed to sync browser carried item: {error}"))?;
        Ok(true)
    }

    fn sync_player_appearance_deferred(&mut self) -> Result<bool, String> {
        self.runtime
            .send_gameplay_command_deferred(set_player_appearance_command_for_ui_model(
                self.player_model,
            ))
            .map_err(|error| format!("failed to sync browser player appearance: {error}"))?;
        Ok(true)
    }

    async fn sync_camera_pose_to_server(&mut self) -> Result<(), String> {
        self.apply_pending_camera_position_updates().await?;
        if let Some(report) = self.camera.next_pose_sync_command() {
            self.runtime
                .send_gameplay_command_async(report.command)
                .await
                .map_err(|error| format!("failed to sync browser camera pose: {error}"))?;
            self.apply_pending_camera_position_updates().await?;
        }
        Ok(())
    }

    fn sync_camera_pose_to_server_deferred(&mut self) -> Result<(), String> {
        self.runtime
            .drain_pending_runner_updates_budgeted(WEB_FRAME_UPDATE_DRAIN_BUDGET)
            .map_err(|error| format!("failed to drain browser camera updates: {error}"))?;
        self.apply_pending_camera_position_updates_deferred()?;
        if let Some(report) = self.camera.next_pose_sync_command() {
            self.runtime
                .send_gameplay_command_deferred(report.command)
                .map_err(|error| format!("failed to queue browser camera pose: {error}"))?;
        }
        self.runtime
            .drain_pending_runner_updates_budgeted(WEB_FRAME_UPDATE_DRAIN_BUDGET)
            .map_err(|error| format!("failed to drain browser camera updates: {error}"))?;
        self.apply_pending_camera_position_updates_deferred()?;
        Ok(())
    }

    async fn apply_pending_camera_position_updates(&mut self) -> Result<(), String> {
        let updates = self
            .runtime
            .drain_player_position_updates()
            .collect::<Vec<_>>();
        for update in updates {
            let accepted = self.camera.accept_position_update(update);
            self.camera
                .probe_ground(self.runtime.client(), WEB_GROUND_PROBE_DISTANCE);
            self.runtime
                .send_gameplay_command_async(accepted.accept_command)
                .await
                .map_err(|error| format!("failed to accept browser camera correction: {error}"))?;
            let resync = self.camera.corrected_pose_sync_command();
            self.runtime
                .send_gameplay_command_async(resync.command)
                .await
                .map_err(|error| {
                    format!("failed to resync corrected browser camera pose: {error}")
                })?;
        }
        Ok(())
    }

    fn apply_pending_camera_position_updates_deferred(&mut self) -> Result<(), String> {
        let updates = self
            .runtime
            .drain_player_position_updates()
            .collect::<Vec<_>>();
        for update in updates {
            let accepted = self.camera.accept_position_update(update);
            self.camera
                .probe_ground(self.runtime.client(), WEB_GROUND_PROBE_DISTANCE);
            self.runtime
                .send_gameplay_command_deferred(accepted.accept_command)
                .map_err(|error| format!("failed to queue browser camera correction: {error}"))?;
            let resync = self.camera.corrected_pose_sync_command();
            self.runtime
                .send_gameplay_command_deferred(resync.command)
                .map_err(|error| {
                    format!("failed to queue corrected browser camera pose: {error}")
                })?;
        }
        Ok(())
    }

    async fn render_chunk_report_for_center(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
    ) -> Result<GeneratedChunkRenderReport, String> {
        let render_plan = self
            .prepare_chunk_render_plan(center, radius_chunks)
            .await?;
        let section_report =
            build_client_textured_sections(self.runtime.client(), &self.mesh_assets.catalog)
                .map_err(|error| {
                    format!("failed to build generated textured render sections: {error:#}")
                })?;
        self.render_prepared_section_report(
            center,
            radius_chunks,
            render_plan,
            section_report,
            WebSectionCompileReport::default(),
        )
    }

    fn render_prepared_section_report(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
        render_plan: WebChunkRenderPlan,
        section_report: TexturedRenderSectionBuildReport,
        compile_report: WebSectionCompileReport,
    ) -> Result<GeneratedChunkRenderReport, String> {
        let removal_chunks = render_plan
            .sync_plan
            .dirty_work
            .removal_dirty_chunks
            .clone();
        let removal_sections = render_plan
            .sync_plan
            .dirty_work
            .removal_dirty_sections
            .clone();
        let submission = self.runtime.engine_mut().submit_prepared_sync_plan(
            &render_plan.sync_plan,
            Vec::new(),
            |_sync_plan, request| {
                Ok::<_, String>(RenderSectionCompileResult {
                    target_sections: request.target_sections,
                    section_revisions: request.section_revisions,
                    result: Ok(section_report),
                })
            },
        )?;
        let Some(submission) = submission.submission else {
            return Err(format!(
                "generated chunk view {},{} radius {} had no ready render sections",
                center.x, center.z, radius_chunks
            ));
        };
        let completed = submission.output;
        self.finish_render_compile_result(
            center,
            radius_chunks,
            render_plan.sync,
            removal_chunks,
            removal_sections,
            render_plan.scope,
            completed,
            compile_report,
            0,
            WebFrameCamera::Overview {
                center,
                radius_chunks,
            },
        )
    }

    fn finish_render_compile_result(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
        sync: RenderSectionViewSync,
        removal_chunks: BTreeSet<ChunkPos>,
        removal_sections: BTreeSet<RenderSectionKey>,
        compile_scope: WebCompileScopeReport,
        completed: RenderSectionCompileResult,
        compile_report: WebSectionCompileReport,
        request_id: u32,
        frame_camera: WebFrameCamera,
    ) -> Result<GeneratedChunkRenderReport, String> {
        let current_section_keys = completed
            .result
            .as_ref()
            .ok()
            .map(|report| {
                report
                    .sections
                    .iter()
                    .map(|section| section.key)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let has_removals = !removal_chunks.is_empty() || !removal_sections.is_empty();
        let finished = self
            .runtime
            .engine_mut()
            .finish_compile_update(completed, request_id, &removal_chunks, &removal_sections)
            .map_err(|error| error.to_string())?;
        if finished.accepted_sections.is_empty()
            && !has_removals
            && finished.acceptance_report.stale_section_count == 0
        {
            return Err(format!(
                "web render compile request {request_id} had no accepted sections ({} stale)",
                finished.acceptance_report.stale_section_count
            ));
        }
        self.render_chunk_report_with_cache_update(
            center,
            radius_chunks,
            sync.current_loaded_chunks,
            current_section_keys,
            finished.cache_update,
            compile_report,
            compile_scope,
            finished.acceptance_report,
            frame_camera,
            true,
        )
    }

    async fn prepare_chunk_render_plan(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
    ) -> Result<WebChunkRenderPlan, String> {
        let sync = self.prepare_chunk_view(center, radius_chunks).await?;
        self.prepare_render_plan_from_sync(sync)
    }

    fn prepare_render_plan_from_sync(
        &mut self,
        sync: RenderSectionViewSync,
    ) -> Result<WebChunkRenderPlan, String> {
        let sync_update = self.runtime.engine_mut().prepare_sync_update(
            |dirty_work| {
                sort_chunk_positions_by_coordinate(dirty_work.loaded_dirty_chunks.iter().copied())
            },
            |dirty_work| {
                sort_dirty_section_chunks_by_coordinate(&dirty_work.loaded_dirty_sections_by_chunk)
            },
            usize::MAX,
            |_client, _key| RenderSectionNeighborReadiness::ReadyWithNeighbors,
            RenderSectionRemovalMode::Defer,
        );
        Ok(WebChunkRenderPlan {
            scope: WebCompileScopeReport::from_plan(&sync, &sync_update.sync_plan),
            sync,
            sync_plan: sync_update.sync_plan,
        })
    }

    async fn sync_render_streaming_frame_async(
        &mut self,
        frame_camera: WebFrameCamera,
        center: ChunkPos,
        camera_position: glam::Vec3,
        radius_chunks: u32,
    ) -> Result<JsValue, String> {
        if !self.is_remote_websocket_runtime() {
            return self.sync_render_streaming_frame_deferred(
                frame_camera,
                center,
                camera_position,
                radius_chunks,
            );
        }

        self.ensure_streaming_compile_supported()?;

        // Remote websocket exchanges are awaited command/response round trips. Unlike the
        // worker-integrated path there is no deferred command queue to drain after the fact;
        // the returned updates are applied by `request_chunk_view_async`.
        if self.interest_center != Some(center) {
            self.runtime
                .request_chunk_view_async(center, radius_chunks, radius_chunks)
                .await
                .map_err(|error| {
                    format!("failed to request remote streaming chunk view: {error}")
                })?;
            self.interest_center = Some(center);
        }

        self.runtime
            .drain_pending_runner_updates_budgeted(WEB_FRAME_UPDATE_DRAIN_BUDGET)
            .map_err(|error| format!("failed to drain web server updates: {error}"))?;

        self.finish_render_streaming_frame(frame_camera, center, camera_position, radius_chunks)
    }

    fn sync_render_streaming_frame_deferred(
        &mut self,
        frame_camera: WebFrameCamera,
        center: ChunkPos,
        camera_position: glam::Vec3,
        radius_chunks: u32,
    ) -> Result<JsValue, String> {
        self.ensure_streaming_compile_supported()?;

        // 1. Keep the runner's streaming interest centered on the camera. Deferred send,
        //    only when the center actually moved, so movement does not re-issue the view
        //    every frame.
        if self.interest_center != Some(center) {
            self.runtime
                .request_chunk_view_deferred(center, radius_chunks, radius_chunks)
                .map_err(|error| format!("failed to request streaming chunk view: {error}"))?;
            self.interest_center = Some(center);
        }

        // 2. Drain runner updates so freshly published snapshots are marked render-dirty
        //    (ALL policy) before this frame's streaming plan runs.
        self.runtime
            .drain_pending_runner_updates_budgeted(WEB_FRAME_UPDATE_DRAIN_BUDGET)
            .map_err(|error| format!("failed to drain web server worker updates: {error}"))?;

        self.finish_render_streaming_frame(frame_camera, center, camera_position, radius_chunks)
    }

    fn ensure_streaming_compile_supported(&self) -> Result<(), String> {
        if self.render_compiler.shared_supported() {
            return Ok(());
        }
        Err(
            "shared render compile transport unavailable: cross-origin isolation \
             (SharedArrayBuffer/Atomics) is required for the streaming render loop"
                .into(),
        )
    }

    fn finish_render_streaming_frame(
        &mut self,
        frame_camera: WebFrameCamera,
        center: ChunkPos,
        camera_position: glam::Vec3,
        radius_chunks: u32,
    ) -> Result<JsValue, String> {
        // 3. Shared per-frame streaming sync (budget 1) over the resident-ring compiler.
        let prev_in_flight = self.render_compiler.in_flight_request_id();
        let render_compiler = &mut self.render_compiler;
        let cache_update = self
            .runtime
            .sync_render_sections_with_budget(
                render_compiler,
                camera_position,
                WEB_RENDER_CHUNK_MESH_BUDGET,
                // 067 follow-up 1: instead of deep-cloning every loaded column each frame, the
                // compiler diffs the live snapshots (borrowed) against its own mirror shadow and
                // clones only the changed columns; the eviction/reset/generation parts of the
                // delta are staged on the compiler for the `submit` that follows in this same
                // streaming step. Web-only — desktop's collector still clones for the `mpsc` move.
                |client, compiler| compiler.stage_streaming_delta(client),
            )
            .map_err(|error| format!("web render streaming sync failed: {error:#}"))?;

        // A new compile was armed this frame whenever the in-flight request id advanced to
        // a fresh value — including the common case where this same frame both drained the
        // previous compile (in_flight -> None) and submitted the next one (-> Some(new)).
        // Comparing against the pre-sync id (rather than only matching None -> Some) is what
        // keeps the doorbell from being dropped on those apply+submit frames.
        let new_in_flight = self.render_compiler.in_flight_request_id();
        let armed_doorbell = new_in_flight.is_some() && new_in_flight != prev_in_flight;
        let applied = self.render_compiler.take_last_completed();
        let pending_compile_jobs = self.render_compiler.pending_job_count();
        let render_pending_work = self.runtime.has_pending_render_work(0, camera_position);
        let streaming_idle = pending_compile_jobs == 0 && !render_pending_work;
        let dirty = self.runtime.engine().render_session().dirty();
        let render_dirty_chunk_count = dirty.dirty_chunks.len();
        let render_dirty_section_count = dirty.dirty_sections.len();
        let render_inflight_section_count = dirty.inflight_sections.len();
        let dirty_work = {
            let client = self.runtime.client();
            let render_session = self.runtime.engine().render_session();
            render_session.classify_dirty_work(
                |pos| client.chunk_snapshot(pos).is_some(),
                |pos| render_session.contains_chunk(pos),
                |key| {
                    let pos = render_section_chunk_pos(key);
                    client
                        .chunk_snapshot(pos)
                        .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
                },
                |key| render_session.contains_section(key),
            )
        };
        let loaded_dirty_section_count: usize = dirty_work
            .loaded_dirty_sections_by_chunk
            .values()
            .map(BTreeSet::len)
            .sum();
        let (ready_dirty_section_count, deferred_dirty_section_count) = dirty_work
            .loaded_dirty_sections_by_chunk
            .values()
            .flat_map(|keys| keys.iter().copied())
            .fold((0usize, 0usize), |(ready, deferred), key| {
                if render_section_neighbor_readiness(self.runtime.client(), key, camera_position)
                    .is_ready()
                {
                    (ready + 1, deferred)
                } else {
                    (ready, deferred + 1)
                }
            });

        // 4. Synthesize this frame's compile diagnostics from the streaming cache update.
        let acceptance_report = RenderSectionCompileAcceptanceReport {
            request_id: applied.map(|completed| completed.request_id).unwrap_or(0),
            submitted_section_count: cache_update.submitted_compile_section_count,
            accepted_section_count: cache_update.completed_compile_section_count,
            stale_section_count: cache_update.stale_compile_section_count,
        };
        let compile_report = match applied {
            Some(completed) if completed.ok => WebSectionCompileReport {
                worker_compile_used: true,
                packed_byte_length: completed.packed_byte_length,
                ..WebSectionCompileReport::default()
            },
            _ => WebSectionCompileReport::default(),
        };

        let current_loaded_chunks = self
            .runtime
            .client()
            .loaded_chunk_positions()
            .collect::<BTreeSet<_>>();
        let current_section_keys = self
            .runtime
            .engine()
            .render_session()
            .section_keys()
            .collect::<BTreeSet<_>>();

        // Count a mesh build for any frame that merged rebuilt sections (streaming drives
        // `render_chunk_report_with_cache_update` with count_mesh_build=false to keep the
        // budgeted drain + no loaded-center semantics, so track the build signal here).
        if cache_update.rebuilt_section_count() > 0 {
            self.mesh_build_count += 1;
        }

        // 5. Render the current cache (sky-only until sections stream in) and serialize.
        let report = self.render_chunk_report_with_cache_update(
            center,
            radius_chunks,
            current_loaded_chunks,
            current_section_keys,
            cache_update,
            compile_report,
            WebCompileScopeReport::default(),
            acceptance_report,
            frame_camera,
            false,
        )?;
        let object: js_sys::Object = report.to_js_value()?.unchecked_into();
        set_bool(&object, "shared", true)?;
        set_bool(&object, "streamingIdle", streaming_idle)?;
        set_bool(&object, "renderPendingWork", render_pending_work)?;
        set_number(
            &object,
            "renderDirtyChunkCount",
            render_dirty_chunk_count as f64,
        )?;
        set_number(
            &object,
            "renderDirtySectionCount",
            render_dirty_section_count as f64,
        )?;
        set_number(
            &object,
            "renderInflightSectionCount",
            render_inflight_section_count as f64,
        )?;
        set_number(
            &object,
            "loadedDirtyChunkCount",
            dirty_work.loaded_dirty_chunks.len() as f64,
        )?;
        set_number(
            &object,
            "removalDirtyChunkCount",
            dirty_work.removal_dirty_chunks.len() as f64,
        )?;
        set_number(
            &object,
            "staleDirtyChunkCount",
            dirty_work.stale_dirty_chunks.len() as f64,
        )?;
        set_number(
            &object,
            "loadedDirtySectionCount",
            loaded_dirty_section_count as f64,
        )?;
        set_number(
            &object,
            "removalDirtySectionCount",
            dirty_work.removal_dirty_sections.len() as f64,
        )?;
        set_number(
            &object,
            "staleDirtySectionCount",
            dirty_work.stale_dirty_sections.len() as f64,
        )?;
        set_number(
            &object,
            "readyCompileSectionCount",
            ready_dirty_section_count as f64,
        )?;
        set_number(
            &object,
            "deferredCompileSectionCount",
            deferred_dirty_section_count as f64,
        )?;
        set_number(
            &object,
            "pendingCompileJobCount",
            pending_compile_jobs as f64,
        )?;
        if let Some(completed) = applied {
            set_number(&object, "appliedRequestId", f64::from(completed.request_id))?;
            set_bool(&object, "appliedOk", completed.ok)?;
            set_number(
                &object,
                "appliedPackedByteLength",
                completed.packed_byte_length as f64,
            )?;
        }
        if armed_doorbell {
            let doorbell = js_sys::Object::new();
            if self.render_compiler.write_doorbell(&doorbell)? {
                set_number(&doorbell, "centerX", f64::from(center.x))?;
                set_number(&doorbell, "centerZ", f64::from(center.z))?;
                set_number(&doorbell, "radiusChunks", f64::from(radius_chunks))?;
                js_sys::Reflect::set(&object, &JsValue::from_str("doorbell"), &doorbell)
                    .map_err(|_| "failed to attach streaming render doorbell".to_string())?;
            }
        }
        Ok(object.into())
    }

    async fn prepare_chunk_view(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
    ) -> Result<RenderSectionViewSync, String> {
        let previous_loaded_chunks = self.loaded_chunk_positions.clone();
        self.runtime
            .request_chunk_view_async(center, radius_chunks, radius_chunks)
            .await
            .map_err(|error| {
                format!("failed to load generated chunk through web runtime: {error}")
            })?;
        if self.runtime.client().chunk_snapshot(center).is_none() {
            return Err(format!(
                "web runtime did not publish generated chunk {},{}",
                center.x, center.z
            ));
        }
        Ok(self
            .runtime
            .engine_mut()
            .apply_current_loaded_view_sync(&previous_loaded_chunks))
    }

    fn render_chunk_report_with_cache_update(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
        current_loaded_chunks: BTreeSet<ChunkPos>,
        current_section_keys: BTreeSet<RenderSectionKey>,
        cache_update: RenderSectionCacheUpdate,
        compile_report: WebSectionCompileReport,
        compile_scope: WebCompileScopeReport,
        acceptance_report: RenderSectionCompileAcceptanceReport,
        frame_camera: WebFrameCamera,
        count_mesh_build: bool,
    ) -> Result<GeneratedChunkRenderReport, String> {
        if count_mesh_build {
            self.mesh_build_count += 1;
        }
        let drain_result = if count_mesh_build {
            self.runtime.drain_pending_runner_updates()
        } else {
            self.runtime
                .drain_pending_runner_updates_budgeted(WEB_FRAME_UPDATE_DRAIN_BUDGET)
        };
        drain_result
            .map_err(|error| format!("failed to drain web server worker updates: {error}"))?;
        let cached_sections = self.runtime.engine().sections();
        // 067 Stage 3: the streaming loop renders every frame, so the cache can be empty
        // on the first frames after a view loads (sections still compiling). Sky-only
        // frames are expected then — the desktop window shows the same while terrain
        // streams in. Draw the section pass only once non-empty sections exist.
        let has_sections = !cached_sections.iter().all(|section| section.is_empty());
        let upload_report = if has_sections {
            self.upload_sections(&cache_update, &cached_sections)?
        } else {
            TexturedSectionUploadReport::default()
        };
        let frame_camera = chunk_camera_for_frame(frame_camera, &self.camera, radius_chunks);
        let render_view = frame_camera.render_view(self.context.width, self.context.height);
        let day_time = self.runtime.client().day_time();
        let time_of_day = self.runtime.client().time_of_day();
        let sun_angle = self.runtime.client().sun_angle();
        let runner_diagnostics = self.runtime.runner_diagnostics();
        let sky_clear_color = mclone_render::sky::overworld_clear_color(time_of_day);
        let mut render_options = TexturedSectionRenderOptions::default()
            .with_sky_darken(mclone_render::light_texture::sky_darken(time_of_day))
            .with_color_profile(self.render_color_profile);
        render_options.section_occlusion_culling = self.section_occlusion_culling;
        render_options.force_fullbright = self.force_fullbright;
        let actor_instances = self.interpolated_actor_instances();
        let ui_active = self.ui.is_active();
        let ui_covers_world = self.ui.covers_world();
        let ui_render_state = self.ui_render_state(radius_chunks);

        let frame = self
            .context
            .surface
            .get_current_texture()
            .map_err(|error| format!("failed to acquire WebGPU canvas frame: {error}"))?;
        let view = frame.texture.create_view(&Default::default());
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("mclone_web_generated_textured_chunk_encoder"),
                });
        let mut actor_stats = mclone_render::entity::ActorRenderStats::default();
        let split_translucent_terrain = !actor_instances.is_empty();
        let render_stats = if ui_covers_world {
            TexturedSectionRenderStats::default()
        } else {
            self.sky.render(
                &self.context.queue,
                &mut encoder,
                &view,
                sky_clear_color,
                render_view.sky_view_projection(),
                time_of_day,
                sun_angle,
            );
            let render_stats = if has_sections {
                let draw = self
                    .draw
                    .as_ref()
                    .ok_or_else(|| "textured draw resources were not initialized".to_owned())?;
                let render_target = ChunkRenderTarget::new(
                    &view,
                    &self.depth.view,
                    [self.context.width, self.context.height],
                    sky_clear_color,
                )
                .with_loaded_color();
                draw.render_with_options_phase_in_slot(
                    &self.context.queue,
                    &mut encoder,
                    render_target,
                    render_view,
                    render_options,
                    mclone_render::uniform::SINGLE_VIEW_SLOT,
                    if split_translucent_terrain {
                        TexturedSectionRenderPhase::Opaque
                    } else {
                        TexturedSectionRenderPhase::All
                    },
                )
                .map_err(|error| format!("failed to render generated section meshes: {error:#}"))?
            } else {
                TexturedSectionRenderStats::default()
            };
            let frame_target =
                RenderFrameTarget::color(&view, [self.context.width, self.context.height])
                    .with_depth(&self.depth.view);
            actor_stats = self
                .actors
                .render(
                    &self.context.device,
                    &self.context.queue,
                    &mut encoder,
                    frame_target,
                    render_view,
                    render_options,
                    &actor_instances,
                )
                .map_err(|error| format!("failed to render browser actor meshes: {error:#}"))?;
            if has_sections && split_translucent_terrain {
                let draw = self
                    .draw
                    .as_ref()
                    .ok_or_else(|| "textured draw resources were not initialized".to_owned())?;
                let translucent_target = ChunkRenderTarget::new(
                    &view,
                    &self.depth.view,
                    [self.context.width, self.context.height],
                    sky_clear_color,
                )
                .with_loaded_color()
                .with_loaded_depth();
                draw.render_with_options_phase_in_slot(
                    &self.context.queue,
                    &mut encoder,
                    translucent_target,
                    render_view,
                    render_options,
                    mclone_render::uniform::SINGLE_VIEW_SLOT,
                    TexturedSectionRenderPhase::Translucent,
                )
                .map_err(|error| {
                    format!("failed to render generated translucent section meshes: {error:#}")
                })?;
            }
            render_stats
        };
        let mut ui_draw = self.ui.render_draw_list(ui_render_state);
        let debug_overlay = (!ui_active && self.debug_overlay_visible).then(|| {
            FlatHudDebugOverlay::at(
                self.debug_overlay(
                    center,
                    radius_chunks,
                    day_time,
                    time_of_day,
                    render_stats,
                    actor_instances.len(),
                    actor_stats,
                    &runner_diagnostics,
                ),
                Point { x: 4.0, y: 52.0 },
            )
        });
        let mut hud = FlatHud::new(self.resolved_flat_input_for_hud());
        hud.world_hud_visible = !ui_active;
        hud.crosshair_visible = self.crosshair_visible && !ui_active;
        let hotbar_icons =
            debug_hotbar_icons(self.interaction.hotbar_items(), &self.mesh_assets.catalog);
        hud.hotbar = FlatHotbarOverlay::selected_with_icons(
            self.interaction.selected_hotbar_slot(),
            hotbar_icons,
        );
        let mut touch = self.touch_overlay;
        touch.hotbar_icons = hotbar_icons;
        hud.touch = touch;
        hud.status = self.effective_status_overlay();
        hud.debug = debug_overlay;
        let flat_hud_retained_cache =
            self.ui
                .append_flat_hud_draw(self.ui.scale(), &mut ui_draw, &hud);
        let gui_command_count = ui_draw.commands().len();
        if gui_command_count > 0 {
            self.gui
                .render(
                    &self.context.device,
                    &self.context.queue,
                    &mut encoder,
                    RenderFrameTarget::color(&view, [self.context.width, self.context.height]),
                    [self.ui.scale().width, self.ui.scale().height],
                    &ui_draw,
                    if ui_covers_world {
                        GuiRenderOptions::clear(mclone_render::default_clear_color())
                    } else {
                        GuiRenderOptions::overlay()
                    },
                )
                .map_err(|error| format!("failed to render browser GUI overlay: {error:#}"))?;
        }
        self.context.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        self.render_count += 1;
        self.loaded_chunk_positions = current_loaded_chunks;
        let effective_status_overlay = self.effective_status_overlay();

        Ok(GeneratedChunkRenderReport {
            center,
            radius_chunks,
            camera_state: self.camera.frame_state(&self.interaction),
            width: self.context.width,
            height: self.context.height,
            day_time,
            time_of_day,
            sun_angle,
            sky_rendered: !ui_covers_world,
            command_count: self.runtime.command_count(),
            update_count: self.runtime.update_count(),
            snapshot_update_count: self.runtime.snapshot_update_count(),
            section_block_update_count: self.runtime.section_block_update_count(),
            unload_update_count: self.runtime.unload_update_count(),
            loaded_chunk_count: self.runtime.client().loaded_chunk_count(),
            loaded_section_key_count: current_section_keys.len(),
            resident_section_count: render_stats.loaded_section_count,
            drawn_section_count: render_stats.drawn_section_count,
            frustum_section_count: render_stats.frustum_section_count,
            asset_pack_file_count: self.mesh_assets.asset_pack_file_count,
            atlas_width: self.mesh_assets.atlas.width,
            atlas_height: self.mesh_assets.atlas.height,
            atlas_sprite_count: self.mesh_assets.atlas_sprite_count,
            vertex_count: section_vertex_count(&cached_sections),
            index_count: render_stats.loaded_index_count,
            face_count: render_stats.loaded_face_count(),
            drawn_index_count: render_stats.drawn_index_count,
            drawn_face_count: render_stats.drawn_face_count(),
            uploaded_section_count: upload_report.uploaded_section_count,
            removed_section_count: upload_report.removed_section_count,
            uploaded_vertex_count: upload_report.uploaded_vertex_count,
            uploaded_index_count: upload_report.uploaded_index_count,
            uploaded_face_count: upload_report.uploaded_face_count(),
            asset_pack_parse_count: self.asset_pack_parse_count,
            terrain_asset_load_count: self.terrain_asset_load_count,
            atlas_upload_count: self.atlas_upload_count,
            mesh_build_count: self.mesh_build_count,
            mesh_upload_count: self.mesh_upload_count,
            render_count: self.render_count,
            gui_command_count,
            flat_hud_retained_rebuild_count: flat_hud_retained_cache.rebuild_count,
            flat_hud_retained_cache_hit_count: flat_hud_retained_cache.cache_hit_count,
            ui_active,
            ui_covers_world,
            ui_screen: ui_screen_label(self.ui.screen()),
            ui_options_parent: match self.ui.screen() {
                Some(GameScreen::Options { parent }) => Some(options_parent_label(parent)),
                _ => None,
            },
            debug_overlay_visible: self.debug_overlay_visible,
            status_overlay_visible: effective_status_overlay.visible
                && !effective_status_overlay.message.is_empty(),
            section_occlusion_culling: self.section_occlusion_culling,
            force_fullbright: self.force_fullbright,
            render_color_profile: self.render_color_profile.as_str(),
            compile_request_id: acceptance_report.request_id,
            pending_compile_job_count: self.render_compiler.pending_job_count(),
            view_dirty_chunk_count: compile_scope.view_dirty_chunk_count,
            view_removal_chunk_count: compile_scope.view_removal_chunk_count,
            loaded_dirty_chunk_count: compile_scope.loaded_dirty_chunk_count,
            removal_dirty_chunk_count: compile_scope.removal_dirty_chunk_count,
            stale_dirty_chunk_count: compile_scope.stale_dirty_chunk_count,
            loaded_dirty_section_count: compile_scope.loaded_dirty_section_count,
            removal_dirty_section_count: compile_scope.removal_dirty_section_count,
            stale_dirty_section_count: compile_scope.stale_dirty_section_count,
            ready_compile_section_count: compile_scope.ready_compile_section_count,
            deferred_compile_section_count: compile_scope.deferred_compile_section_count,
            budgeted_loaded_chunk_count: compile_scope.budgeted_loaded_chunk_count,
            budgeted_dirty_section_chunk_count: compile_scope.budgeted_dirty_section_chunk_count,
            submitted_compile_section_count: acceptance_report.submitted_section_count,
            accepted_compile_section_count: acceptance_report.accepted_section_count,
            stale_compile_section_count: acceptance_report.stale_section_count,
            worker_compile_used: compile_report.worker_compile_used,
            worker_packed_byte_length: compile_report.packed_byte_length,
            worker_section_count: compile_report.section_count,
            worker_non_empty_section_count: compile_report.non_empty_section_count,
            worker_visibility_graph_build_count: compile_report.visibility_graph_build_count,
            worker_visibility_graph_total_ms: compile_report.visibility_graph_total_ms,
            worker_visibility_graph_worst_ms: compile_report.visibility_graph_worst_ms,
            worker_vertex_count: compile_report.vertex_count,
            worker_index_count: compile_report.index_count,
            worker_face_count: compile_report.face_count,
            actor_count: actor_instances.len(),
            drawn_actor_count: actor_stats.drawn_actor_count,
            drawn_actor_index_count: actor_stats.index_count,
            actor_atlas_width: self.mesh_assets.actor_atlas.width,
            actor_atlas_height: self.mesh_assets.actor_atlas.height,
            runner_kind: runner_diagnostics.kind,
            runner_running: runner_diagnostics.running,
            runner_command_queue_depth: runner_diagnostics.command_queue_depth,
            runner_update_queue_depth: runner_diagnostics.update_queue_depth,
            runner_pending_jobs: runner_diagnostics.pending_jobs,
            runner_pending_publications: runner_diagnostics.pending_publications,
            worldgen_mailbox_kind: runner_diagnostics.worldgen_mailbox_kind,
            light_status_mailbox_kind: runner_diagnostics.light_status_mailbox_kind,
            worldgen_mailbox_pending_jobs: runner_diagnostics.worldgen_mailbox_pending_jobs,
            light_status_mailbox_pending_statuses: runner_diagnostics
                .light_status_mailbox_pending_statuses,
            runner_frame_metrics: runner_diagnostics.runner_frame_metrics,
            worldgen_job_frame_metrics: runner_diagnostics.worldgen_job_frame_metrics,
            light_status_job_frame_metrics: runner_diagnostics.light_status_job_frame_metrics,
            runner_last_simulation_tick: runner_diagnostics.last_tick.simulation_tick,
        })
    }

    fn upload_sections(
        &mut self,
        update: &RenderSectionCacheUpdate,
        cached_sections: &[mclone_mesh::TexturedRenderSectionMesh],
    ) -> Result<TexturedSectionUploadReport, String> {
        let first_upload = self.draw.is_none();

        let upload_report = if first_upload {
            let draw = TexturedSectionDrawResources::new(
                &self.context.device,
                &self.context.queue,
                self.context.format,
                cached_sections,
                atlas_upload(&self.mesh_assets.atlas),
            )
            .map_err(|error| {
                format!("failed to upload generated textured section meshes: {error:#}")
            })?;
            self.draw = Some(draw);
            self.atlas_upload_count += 1;
            upload_report_for_sections(cached_sections)
        } else {
            let draw = self
                .draw
                .as_mut()
                .ok_or_else(|| "textured section resources were not initialized".to_owned())?;
            draw.apply_section_updates(
                &self.context.device,
                &update.rebuilt_sections,
                &update.removed_section_keys,
            )
            .map_err(|error| {
                format!("failed to upload generated textured section mesh updates: {error:#}")
            })?
        };
        if first_upload || update.rebuilt_section_count() > 0 || update.removed_section_count() > 0
        {
            self.mesh_upload_count += 1;
        }
        Ok(upload_report)
    }

    fn interpolated_actor_instances(&mut self) -> Vec<ActorInstance> {
        self.actor_interpolation
            .reconcile_authoritative(self.runtime.client().actor_presentations());
        self.actor_interpolation
            .step(0.05, ActorInterpolationConfig::default());
        actor_instances_from_presentations(
            &self.actor_interpolation.presentations(),
            self.runtime.client(),
        )
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WebFrameCamera {
    Overview {
        center: ChunkPos,
        radius_chunks: u32,
    },
    Camera,
}

fn chunk_camera_for_frame(
    frame_camera: WebFrameCamera,
    camera: &EngineCameraController,
    radius_chunks: u32,
) -> ChunkCamera {
    match frame_camera {
        WebFrameCamera::Overview {
            center,
            radius_chunks,
        } => ChunkCamera::overview_for_chunk_area(center.x, center.z, radius_chunks as i32),
        WebFrameCamera::Camera => chunk_camera_from_engine(camera.render_camera(radius_chunks)),
    }
}

fn chunk_camera_from_engine(camera: EngineRenderCamera) -> ChunkCamera {
    ChunkCamera {
        eye: camera.eye,
        target: camera.target,
        up: camera.up,
        fov_y_radians: camera.fov_y_radians,
        z_near: camera.z_near,
        z_far: camera.z_far,
    }
}

fn glam_vec3_from_vec3d(value: Vec3d) -> glam::Vec3 {
    glam::Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

fn camera_state_to_js_value(
    camera: &EngineCameraController,
    interaction: &ClientInteractionController,
) -> Result<JsValue, String> {
    let object = js_sys::Object::new();
    set_bool(&object, "ok", true)?;
    write_camera_frame_state_to_js(&object, camera.frame_state(interaction))?;
    Ok(object.into())
}

fn write_camera_frame_state_to_js(
    object: &js_sys::Object,
    state: EngineCameraFrameState,
) -> Result<(), String> {
    let snapshot = state.camera;
    set_bool(object, "onGround", state.on_ground)?;
    set_bool(object, "horizontalCollision", state.horizontal_collision)?;
    set_bool(object, "verticalCollision", state.vertical_collision)?;
    set_string(object, "movementMode", state.movement_mode_label())?;
    set_number(
        object,
        "selectedHotbarSlot",
        f64::from(state.selected_hotbar_slot),
    )?;
    set_number(object, "cameraX", snapshot.eye.x)?;
    set_number(object, "cameraY", snapshot.eye.y)?;
    set_number(object, "cameraZ", snapshot.eye.z)?;
    set_number(object, "cameraYawRadians", snapshot.yaw_radians)?;
    set_number(object, "cameraPitchRadians", snapshot.pitch_radians)?;
    set_number(
        object,
        "cameraSpeedBlocksPerSecond",
        snapshot.speed_blocks_per_second,
    )?;
    set_number(object, "centerX", f64::from(snapshot.chunk_pos.x))?;
    set_number(object, "centerZ", f64::from(snapshot.chunk_pos.z))?;
    Ok(())
}

fn hotbar_state_to_js_value(interaction: &ClientInteractionController) -> Result<JsValue, String> {
    let object = js_sys::Object::new();
    set_bool(&object, "ok", true)?;
    set_number(
        &object,
        "selectedHotbarSlot",
        f64::from(interaction.selected_hotbar_slot()),
    )?;
    Ok(object.into())
}

fn canvas_size_to_js_value(width: u32, height: u32, changed: bool) -> Result<JsValue, String> {
    let object = js_sys::Object::new();
    set_bool(&object, "ok", true)?;
    set_bool(&object, "changed", changed)?;
    set_number(&object, "width", f64::from(width))?;
    set_number(&object, "height", f64::from(height))?;
    Ok(object.into())
}

fn upload_report_for_sections(
    sections: &[mclone_mesh::TexturedRenderSectionMesh],
) -> TexturedSectionUploadReport {
    let mut report = TexturedSectionUploadReport::default();
    for section in sections {
        if section.is_empty() {
            continue;
        }
        let stats = section.stats();
        report.uploaded_section_count += 1;
        report.uploaded_vertex_count += stats.vertex_count;
        report.uploaded_index_count += stats.index_count;
    }
    report
}

fn sort_chunk_positions_by_coordinate(
    positions: impl IntoIterator<Item = ChunkPos>,
) -> Vec<ChunkPos> {
    let mut positions = positions.into_iter().collect::<Vec<_>>();
    positions.sort_by(|left, right| left.x.cmp(&right.x).then_with(|| left.z.cmp(&right.z)));
    positions
}

fn sort_dirty_section_chunks_by_coordinate(
    sections_by_chunk: &BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
) -> Vec<ChunkPos> {
    sort_chunk_positions_by_coordinate(sections_by_chunk.keys().copied())
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

fn section_vertex_count(sections: &[mclone_mesh::TexturedRenderSectionMesh]) -> u32 {
    sections
        .iter()
        .filter(|section| !section.is_empty())
        .map(|section| section.stats().vertex_count)
        .sum()
}

#[derive(Clone, Debug)]
struct WebTexturedMeshAssets {
    catalog: TexturedMeshCatalog,
    atlas: TextureAtlasImage,
    actor_atlas: ActorTextureImage,
    actor_figures: ActorFigureSet,
    atlas_sprite_count: usize,
    asset_pack_file_count: usize,
}

fn atlas_upload(atlas: &TextureAtlasImage) -> ChunkTextureAtlas<'_> {
    ChunkTextureAtlas {
        width: atlas.width,
        height: atlas.height,
        rgba: atlas.rgba(),
    }
}

fn load_textured_mesh_assets_from_pack(bytes: Vec<u8>) -> Result<WebTexturedMeshAssets, String> {
    let source = PackedAssetSource::from_bytes(bytes)
        .map_err(|error| format!("failed to parse packed Minecraft assets: {error}"))?;
    let asset_pack_file_count = source.file_count();
    let assets = load_textured_terrain_assets(&source)
        .map_err(|error| format!("failed to load packed textured terrain assets: {error}"))?;
    let actor_assets = load_actor_texture_assets(&source)
        .map_err(|error| format!("failed to load packed actor texture assets: {error}"))?;

    Ok(WebTexturedMeshAssets {
        catalog: assets.catalog,
        atlas: assets.atlas,
        actor_atlas: actor_assets.atlas,
        actor_figures: actor_assets.figures,
        atlas_sprite_count: assets.atlas_sprite_count,
        asset_pack_file_count,
    })
}

fn started_web_session_coordinator(
    request: SessionStartRequest,
) -> Result<GameSessionCoordinator<()>, String> {
    let descriptor = request
        .active_descriptor()
        .ok_or_else(|| "web session request did not describe an active session".to_owned())?;
    let result: SessionStartResult<()> = Ok(StartedGameSession::new(descriptor, ()));
    let mut coordinator = GameSessionCoordinator::new();
    coordinator.begin_start(request);
    coordinator.apply_start_result(&result);
    Ok(coordinator)
}

fn gui_key_from_label(label: &str) -> Option<GuiKey> {
    match label {
        "escape" | "Escape" => Some(GuiKey::Escape),
        "f1" | "F1" => Some(GuiKey::F1),
        _ => None,
    }
}

fn ui_screen_label(screen: Option<GameScreen>) -> &'static str {
    match screen {
        Some(GameScreen::Title) => "title",
        Some(GameScreen::NewWorld) => "newWorld",
        Some(GameScreen::JoinRemote) => "joinRemote",
        Some(GameScreen::Pause) => "pause",
        Some(GameScreen::Help { .. }) => "help",
        Some(GameScreen::BlockPalette) => "blockPalette",
        Some(GameScreen::Options { .. }) => "options",
        Some(GameScreen::ServerSettings { .. }) => "serverSettings",
        None => "none",
    }
}

fn options_parent_label(parent: GameOptionsParent) -> &'static str {
    match parent {
        GameOptionsParent::Title => "title",
        GameOptionsParent::Pause => "pause",
    }
}

fn ui_action_label(action: GameUiAction) -> &'static str {
    match action {
        GameUiAction::StartWorld => "startWorld",
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
        GameUiAction::OpenServerSettings(_) => "openServerSettings",
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
        GameUiAction::SetPlayerModel(_) => "setPlayerModel",
        GameUiAction::SetMovementMode(_) => "setMovementMode",
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

fn touch_controls_mode_js_label(mode: TouchControlsMode) -> &'static str {
    match mode {
        TouchControlsMode::Auto => "auto",
        TouchControlsMode::On => "on",
        TouchControlsMode::Off => "off",
    }
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

fn web_integrated_server_runner_config_with_storage(
    config: WebIntegratedServerRunnerConfig,
    world_storage: &str,
    world_id: &str,
    clear_world_storage: bool,
) -> Result<WebIntegratedServerRunnerConfig, String> {
    match normalize_web_world_storage_label(world_storage)?.as_str() {
        "transient" => Ok(config),
        "indexeddb" => {
            let world_id = world_id.trim();
            let world_id = if world_id.is_empty() {
                default_web_world_id(config.seed)
            } else {
                world_id.to_owned()
            };
            Ok(config.with_indexed_db_world(world_id, clear_world_storage))
        }
        _ => unreachable!("normalize_web_world_storage_label returned an unknown label"),
    }
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

fn startup_render_options(
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

fn parse_touch_controls_mode(mode: &str) -> Option<TouchControlsMode> {
    match mode {
        "auto" => Some(TouchControlsMode::Auto),
        "on" => Some(TouchControlsMode::On),
        "off" => Some(TouchControlsMode::Off),
        _ => None,
    }
}

fn write_web_session_status_to_js_object(
    object: &js_sys::Object,
    coordinator: &GameSessionCoordinator<()>,
) -> Result<(), String> {
    match coordinator.state() {
        GameSessionState::NoSession => {
            set_string(object, "sessionState", "none")?;
        }
        GameSessionState::Starting { request } => {
            set_string(object, "sessionState", "starting")?;
            write_session_request_to_js_object(object, request)?;
        }
        GameSessionState::Active { session } => {
            set_string(object, "sessionState", "active")?;
            write_active_session_to_js_object(object, session)?;
        }
        GameSessionState::Failed { request, error } => {
            set_string(object, "sessionState", "failed")?;
            write_session_request_to_js_object(object, request)?;
            set_string(object, "sessionFailureMessage", &error.message)?;
        }
    }

    if let Some(status) = coordinator.status() {
        set_bool(object, "sessionStatusVisible", true)?;
        set_bool(object, "sessionStatusOk", status.ok)?;
        set_string(object, "sessionStatusMessage", &status.message)?;
    } else {
        set_bool(object, "sessionStatusVisible", false)?;
        set_bool(object, "sessionStatusOk", true)?;
    }
    Ok(())
}

fn write_session_request_to_js_object(
    object: &js_sys::Object,
    request: &SessionStartRequest,
) -> Result<(), String> {
    match request {
        SessionStartRequest::NewLocalWorld { seed } => {
            set_string(object, "sessionKind", "localWorld")?;
            set_number(object, "sessionSeed", *seed as f64)?;
            set_string(object, "sessionSeedText", &seed.to_string())?;
        }
        SessionStartRequest::JoinRemote { endpoint } => {
            set_string(object, "sessionKind", "remote")?;
            set_string(object, "sessionRemoteEndpoint", &endpoint.address)?;
        }
        SessionStartRequest::Unknown => {
            set_string(object, "sessionKind", "unknown")?;
        }
    }
    Ok(())
}

fn write_active_session_to_js_object(
    object: &js_sys::Object,
    session: &ActiveSessionDescriptor,
) -> Result<(), String> {
    match session {
        ActiveSessionDescriptor::LocalWorld { seed } => {
            set_string(object, "sessionKind", "localWorld")?;
            set_number(object, "sessionSeed", *seed as f64)?;
            set_string(object, "sessionSeedText", &seed.to_string())?;
        }
        ActiveSessionDescriptor::Remote { endpoint } => {
            set_string(object, "sessionKind", "remote")?;
            set_string(object, "sessionRemoteEndpoint", &endpoint.address)?;
        }
    }
    Ok(())
}

fn clamp_touch_look_sensitivity(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(
            WEB_TOUCH_LOOK_SENSITIVITY_MIN,
            WEB_TOUCH_LOOK_SENSITIVITY_MAX,
        )
    } else {
        WEB_TOUCH_LOOK_SENSITIVITY_DEFAULT
    }
}

fn game_movement_mode(mode: EngineCameraMovementMode) -> GameMovementMode {
    match mode {
        EngineCameraMovementMode::Walking => GameMovementMode::Walk,
        EngineCameraMovementMode::NoClip => GameMovementMode::Fly,
        EngineCameraMovementMode::HandPush => GameMovementMode::HandPush,
    }
}

fn engine_movement_mode(mode: GameMovementMode) -> EngineCameraMovementMode {
    match mode {
        GameMovementMode::Walk => EngineCameraMovementMode::Walking,
        GameMovementMode::Fly => EngineCameraMovementMode::NoClip,
        GameMovementMode::HandPush => EngineCameraMovementMode::HandPush,
    }
}

struct WebCanvasContext {
    canvas: HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    present_mode: wgpu::PresentMode,
    alpha_mode: wgpu::CompositeAlphaMode,
    width: u32,
    height: u32,
}

impl WebCanvasContext {
    async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        Self::new_with_color_profile(canvas, RenderColorProfile::default()).await
    }

    async fn new_with_color_profile(
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

    fn resize(&mut self, width: u32, height: u32) -> bool {
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

fn set_string(object: &js_sys::Object, key: &str, value: &str) -> Result<(), String> {
    js_sys::Reflect::set(object, &JsValue::from_str(key), &JsValue::from_str(value))
        .map(|_| ())
        .map_err(|_| format!("failed to set generated chunk report key {key}"))
}

fn set_worker_frame_metrics(
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

fn direction_label(direction: Direction) -> &'static str {
    match direction {
        Direction::Down => "down",
        Direction::Up => "up",
        Direction::North => "north",
        Direction::South => "south",
        Direction::West => "west",
        Direction::East => "east",
    }
}

fn optional_block_state_id(block_state: Option<BlockStateId>) -> f64 {
    block_state.map(|state| state.0 as f64).unwrap_or(-1.0)
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
