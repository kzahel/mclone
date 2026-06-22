use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use std::collections::{BTreeMap, BTreeSet};

use super::{
    SMOKE_INITIAL_CENTER, SMOKE_MOVED_CENTER, SMOKE_RADIUS_CHUNKS, SMOKE_SEED,
    WebIntegratedServerRunnerConfig, WebRuntime,
};
use mclone_assets::PackedAssetSource;
use mclone_client::{
    ActorInterpolationConfig, ActorInterpolationState, ActorPresentation, ActorPresentationKind,
    ClientInteractionController, ClientRuntime, LOCAL_PLAYER_STANDING_EYE_HEIGHT,
};
#[cfg(test)]
use mclone_core::{
    AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkRevision, ChunkStatus,
    chunk_section_index,
};
use mclone_core::{
    BlockHitResult, BlockPos, BlockStateId, ChunkPos, ChunkSnapshot, Direction, HitResultType,
    Vec3d,
};
use mclone_mesh::{
    RenderSectionKey, TextureAtlasImage, TexturedMeshCatalog, TexturedRenderSectionBuildReport,
    load_textured_terrain_assets,
};
use mclone_protocol::{EntityKind, ServerUpdate, decode_server_update, encode_server_update};
use mclone_render::actor_assets::{ActorTextureImage, load_actor_texture_assets};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, ChunkTextureAtlas,
    TexturedSectionDrawResources, TexturedSectionRenderOptions, TexturedSectionUploadReport,
};
use mclone_render::entity::{ActorDrawResources, ActorInstance};
use mclone_render::sky_render::SkyRenderer;
use mclone_render::target::RenderFrameTarget;
use mclone_render_session::{
    EngineCameraController, EngineCameraFrameState, EngineCameraInput, EngineCameraMovementImpulse,
    EngineRenderCamera, PackedRenderSectionBuildReportSummary, QueuedRenderViewCompile,
    RenderSectionCacheUpdate, RenderSectionCompileAcceptanceReport, RenderSectionCompileRequest,
    RenderSectionCompileRequestInfo, RenderSectionCompileRequestState, RenderSectionCompileResult,
    RenderSectionCompiler, RenderSectionNeighborReadiness, RenderSectionPendingCompileRequest,
    RenderSectionRemovalMode,
    RenderSectionSyncPlan, RenderSectionViewSync, RenderViewCompileQueue,
    RenderViewCompileQueueDecision, build_client_textured_sections,
    build_render_sections_from_snapshots, decode_textured_render_section_build_report,
    encode_textured_render_section_build_report, summarize_textured_render_section_build_report,
};
use mclone_server::ServerRunnerKind;

const CANVAS_OK_BIT: u32 = 1 << 0;
const CANVAS_RENDERED_BIT: u32 = 1 << 1;
const CANVAS_CONFIGURED_BIT: u32 = 1 << 2;
const CANVAS_WIDTH_SHIFT: u32 = 8;
const CANVAS_HEIGHT_SHIFT: u32 = 20;
const WEB_GROUND_PROBE_DISTANCE: f64 = 0.01;
const WEB_FRAME_UPDATE_DRAIN_BUDGET: usize = 1;
const WEB_COMPILE_WAIT_UPDATE_DRAIN_BUDGET: usize = 1;
const WEB_RENDER_COMPILE_INPUT_MAGIC: &[u8; 8] = b"MCWRCI1\0";

// 067 Stage 2: resident render-compiler shared ring ABI. These mirror the
// constants in `mclone-render-compiler-worker.js` (the producer) and the JS app /
// smoke glue; the worker writes the result control word + payload and main wasm
// reads them back here via `js_sys::Atomics`. Keep all three copies in lockstep
// until Stage 5 emits them from a single Rust source.
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

    #[wasm_bindgen(js_name = compileSnapshotSectionsForTargets)]
    pub fn compile_snapshot_sections_for_targets(
        &mut self,
        snapshot_input_bytes: js_sys::Uint8Array,
        target_sections: js_sys::Int32Array,
    ) -> Result<js_sys::Uint8Array, JsValue> {
        self.compile_count += 1;
        let snapshots = decode_web_render_compile_input(&snapshot_input_bytes.to_vec())
            .map_err(|error| JsValue::from_str(&error))?;
        let target_sections = render_section_keys_from_int32_array(&target_sections)
            .map_err(|error| JsValue::from_str(&error))?;
        let report = compile_snapshot_chunk_sections_with_catalog(
            &self.mesh_assets.catalog,
            &snapshots,
            &target_sections,
        )
        .map_err(JsValue::from)?;
        let packed = encode_textured_render_section_build_report(&report);
        Ok(js_sys::Uint8Array::from(packed.as_slice()))
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
        build_render_sections_from_snapshots(&snapshots, catalog, target_sections).map_err(
            |error| format!("failed to compile targeted generated render sections: {error:#}"),
        )
    } else {
        build_client_textured_sections(runtime.client(), catalog).map_err(|error| {
            format!("failed to compile generated textured render sections: {error:#}")
        })
    }
}

fn compile_snapshot_chunk_sections_with_catalog(
    catalog: &TexturedMeshCatalog,
    snapshots: &[ChunkSnapshot],
    target_sections: &BTreeSet<RenderSectionKey>,
) -> Result<mclone_mesh::TexturedRenderSectionBuildReport, String> {
    build_render_sections_from_snapshots(snapshots, catalog, target_sections)
        .map_err(|error| format!("failed to compile snapshot render sections: {error:#}"))
}

fn encode_web_render_compile_input(snapshots: &[ChunkSnapshot]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(WEB_RENDER_COMPILE_INPUT_MAGIC);
    write_web_compile_input_u32(&mut bytes, snapshots.len(), "snapshot count")?;
    for snapshot in snapshots {
        let frame = encode_server_update(&ServerUpdate::ChunkSnapshot(snapshot.clone())).map_err(
            |error| {
                format!(
                    "failed to encode compile snapshot {:?}: {error}",
                    snapshot.pos
                )
            },
        )?;
        write_web_compile_input_u32(&mut bytes, frame.len(), "snapshot frame length")?;
        bytes.extend_from_slice(&frame);
    }
    Ok(bytes)
}

fn decode_web_render_compile_input(bytes: &[u8]) -> Result<Vec<ChunkSnapshot>, String> {
    let mut reader = WebRenderCompileInputReader::new(bytes);
    let magic = reader.read_bytes(WEB_RENDER_COMPILE_INPUT_MAGIC.len(), "magic")?;
    if magic != WEB_RENDER_COMPILE_INPUT_MAGIC {
        return Err("web render compile input had an invalid magic header".to_string());
    }
    let snapshot_count = reader.read_u32("snapshot count")? as usize;
    let mut snapshots = Vec::with_capacity(snapshot_count);
    for index in 0..snapshot_count {
        let frame_len = reader.read_u32("snapshot frame length")? as usize;
        let frame = reader.read_bytes(frame_len, "snapshot frame")?;
        let update = decode_server_update(frame)
            .map_err(|error| format!("failed to decode compile snapshot frame {index}: {error}"))?;
        match update {
            ServerUpdate::ChunkSnapshot(snapshot) => snapshots.push(snapshot),
            _ => {
                return Err(format!(
                    "compile snapshot frame {index} did not contain a chunk snapshot"
                ));
            }
        }
    }
    reader.finish()?;
    Ok(snapshots)
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

impl WebSectionCompileReport {
    fn worker(packed_byte_length: usize, summary: PackedRenderSectionBuildReportSummary) -> Self {
        Self {
            worker_compile_used: true,
            packed_byte_length,
            section_count: summary.section_count,
            non_empty_section_count: summary.non_empty_section_count,
            visibility_graph_build_count: summary.visibility_graph_stats.build_count,
            visibility_graph_total_ms: summary.visibility_graph_stats.total_ms,
            visibility_graph_worst_ms: summary.visibility_graph_stats.worst_ms,
            vertex_count: summary.vertex_count,
            index_count: summary.index_count,
            face_count: summary.face_count(),
        }
    }
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct WebPendingCompileContext {
    center: ChunkPos,
    radius_chunks: u32,
    sync: RenderSectionViewSync,
    removal_chunks: BTreeSet<ChunkPos>,
    removal_sections: BTreeSet<RenderSectionKey>,
    scope: WebCompileScopeReport,
}

/// Per-compile context for the 067 Stage 2 shared-ring path, held by main wasm while
/// the worker compiles. `submit*` stores it keyed by the compiler's request id; the
/// matching `pollCameraRenderCompile` consumes it once the result control word flips
/// to COMPLETE/FAILED. Single compile in flight, so exactly one is live at a time.
struct WebSharedCompileContext {
    request_id: u32,
    center: ChunkPos,
    radius_chunks: u32,
    sync: RenderSectionViewSync,
    removal_chunks: BTreeSet<ChunkPos>,
    removal_sections: BTreeSet<RenderSectionKey>,
    scope: WebCompileScopeReport,
    frame_camera: WebFrameCamera,
}

#[derive(Clone, Debug, PartialEq)]
struct WebChunkRenderPlan {
    sync: RenderSectionViewSync,
    sync_plan: RenderSectionSyncPlan,
    scope: WebCompileScopeReport,
}

fn compile_result_from_worker_report(
    pending: RenderSectionPendingCompileRequest<WebPendingCompileContext>,
    section_report: TexturedRenderSectionBuildReport,
) -> (WebPendingCompileContext, RenderSectionCompileResult) {
    let RenderSectionPendingCompileRequest {
        context,
        target_sections,
        section_revisions,
        ..
    } = pending;
    let completed =
        render_section_compile_result_from_report(target_sections, section_revisions, section_report);
    (context, completed)
}

/// Build a [`RenderSectionCompileResult`] from a worker-produced section report.
///
/// The worker only emits sections it actually built, so any submitted target the
/// report omits (e.g. an all-air section) is marked with a revision one behind the
/// submitted revision. The shared render-session acceptance path then classifies it
/// as stale and drops it instead of resurrecting a section that was never compiled.
/// Shared by the legacy begin/finish worker path and the Stage-2
/// [`WebRenderSectionCompiler`] resident-ring transport.
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
        render_compiler_atomic_load(&self.result_control, RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX)
    }

    fn result_byte_length(&self) -> Result<u32, String> {
        let bytes =
            render_compiler_atomic_load(&self.result_control, RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX)?;
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

/// Web `RenderSectionCompiler` over the resident `SharedArrayBuffer` ring (067 Stage 2).
///
/// `submit` serializes the request into the resident input arena and arms the result
/// control word; JavaScript posts a tiny doorbell to the worker (JS still owns the
/// Worker lifecycle and diagnostics). `try_recv_completed` polls the result control
/// word from main wasm via `js_sys::Atomics` and decodes the packed report in place —
/// no JavaScript in the result data path. A single compile is in flight at a time,
/// matching the existing busy-flag streaming model; the whole-view budget is unchanged
/// at this stage so the transport swap is isolated from scheduling.
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
        }
    }

    fn shared_supported(&self) -> bool {
        self.shared.is_some()
    }

    fn in_flight_request_id(&self) -> Option<u32> {
        self.in_flight.as_ref().map(|in_flight| in_flight.request_id)
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
        render_compiler_set_value(object, "sharedInputControlBuffer", arena.input_control_buffer.as_ref())?;
        render_compiler_set_value(object, "sharedInputBuffer", arena.input_buffer.as_ref())?;
        set_number(object, "sharedInputByteLength", f64::from(arena.input_byte_length))?;
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
        let input_bytes = encode_web_render_compile_input(&request.snapshots)
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
        self.in_flight = Some(WebSharedCompileInFlight {
            request_id,
            target_sections: request.target_sections,
            section_revisions: request.section_revisions,
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
                let section_report = decode_textured_render_section_build_report(&packed)
                    .map_err(|error| {
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
    interaction_command_count: usize,
    interaction_update_count: usize,
    total_command_count: usize,
    total_update_count: usize,
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
    camera: EngineCameraController,
    interaction: ClientInteractionController,
    actor_interpolation: ActorInterpolationState,
    mesh_assets: WebTexturedMeshAssets,
    draw: Option<TexturedSectionDrawResources>,
    actors: ActorDrawResources,
    loaded_chunk_positions: BTreeSet<ChunkPos>,
    loaded_center: Option<ChunkPos>,
    compile_queue: RenderViewCompileQueue<String>,
    asset_pack_parse_count: usize,
    terrain_asset_load_count: usize,
    atlas_upload_count: usize,
    mesh_build_count: usize,
    mesh_upload_count: usize,
    render_count: usize,
    compile_requests: RenderSectionCompileRequestState<WebPendingCompileContext>,
    render_compiler: WebRenderSectionCompiler,
    shared_compile: Option<WebSharedCompileContext>,
}

#[wasm_bindgen]
impl WebChunkRenderSession {
    #[wasm_bindgen(js_name = renderChunkReport)]
    pub async fn render_chunk_report(
        &mut self,
        center_x: i32,
        center_z: i32,
        radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        self.render_chunk_report_for_center(
            ChunkPos {
                x: center_x,
                z: center_z,
            },
            radius_chunks,
        )
        .await
        .and_then(GeneratedChunkRenderReport::to_js_value)
        .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = beginChunkRenderCompileRequest)]
    pub async fn begin_chunk_render_compile_request(
        &mut self,
        center_x: i32,
        center_z: i32,
        radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        self.begin_chunk_render_compile_request_for_center(
            ChunkPos {
                x: center_x,
                z: center_z,
            },
            radius_chunks,
        )
        .await
        .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = finishChunkRenderCompileRequest)]
    pub fn finish_chunk_render_compile_request(
        &mut self,
        request_id: u32,
        packed_report: js_sys::Uint8Array,
    ) -> Result<JsValue, JsValue> {
        self.finish_chunk_render_compile_request_with_packed_report(
            request_id,
            packed_report.to_vec(),
        )
        .and_then(GeneratedChunkRenderReport::to_js_value)
        .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = pendingChunkRenderCompileJobCount)]
    pub fn pending_chunk_render_compile_job_count(&self) -> usize {
        self.compile_requests.pending_request_count() + self.render_compiler.pending_job_count()
    }

    /// Whether the resident shared-ring render-compile transport (067 Stage 2) is
    /// available. False on non-cross-origin-isolated browsers without
    /// `SharedArrayBuffer`/`Atomics`, where JS falls back to the labeled
    /// message-transfer begin/finish path.
    #[wasm_bindgen(js_name = renderCompilerSharedSupported)]
    pub fn render_compiler_shared_supported(&self) -> bool {
        self.render_compiler.shared_supported()
    }

    /// 067 Stage 2: submit the initial camera render compile through the resident
    /// shared ring. Loads the camera-centered view, fills the input arena, arms the
    /// result control word, and returns a doorbell descriptor (request id + the
    /// resident shared buffers) for JavaScript to post to the worker.
    #[wasm_bindgen(js_name = submitCameraRenderCompile)]
    pub async fn submit_camera_render_compile(
        &mut self,
        radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        self.submit_camera_render_compile_for_radius(radius_chunks)
            .await
            .map_err(JsValue::from)
    }

    /// 067 Stage 2: submit the deferred (movement) camera render compile through the
    /// resident shared ring. Drains pending runner updates and returns a waiting
    /// descriptor until the target center is loaded and the runner has settled, then
    /// submits and returns a doorbell descriptor.
    #[wasm_bindgen(js_name = submitDeferredCameraRenderCompile)]
    pub fn submit_deferred_camera_render_compile(
        &mut self,
        radius_chunks: u32,
        center_x: i32,
        center_z: i32,
    ) -> Result<JsValue, JsValue> {
        self.submit_deferred_camera_render_compile_for_center(
            radius_chunks,
            ChunkPos {
                x: center_x,
                z: center_z,
            },
        )
        .map_err(JsValue::from)
    }

    /// 067 Stage 2: poll the resident shared ring for a completed compile. Reads the
    /// result control word via `Atomics` from main wasm; returns `{pending:true}`
    /// while the worker is still compiling, or applies + renders the completed report
    /// and returns the frame report once COMPLETE.
    #[wasm_bindgen(js_name = pollCameraRenderCompile)]
    pub fn poll_camera_render_compile(&mut self) -> Result<JsValue, JsValue> {
        self.poll_camera_render_compile_result().map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = requestCameraRenderCompile)]
    pub fn request_camera_render_compile(
        &mut self,
        trigger: &str,
        force: bool,
        compile_busy: bool,
    ) -> Result<JsValue, JsValue> {
        self.request_camera_render_compile_decision(trigger, force, compile_busy)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = takeQueuedCameraRenderCompile)]
    pub fn take_queued_camera_render_compile(
        &mut self,
        compile_busy: bool,
    ) -> Result<JsValue, JsValue> {
        self.take_queued_camera_render_compile_decision(compile_busy)
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
            mouse_delta_x,
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
        };
        self.camera
            .apply_movement_input(self.runtime.client(), input);
        self.sync_camera_pose_to_server_deferred()
            .map_err(JsValue::from)?;
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
        }
        canvas_size_to_js_value(self.context.width, self.context.height, changed)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = beginCameraRenderCompileRequest)]
    pub async fn begin_camera_render_compile_request(
        &mut self,
        radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        self.begin_camera_render_compile_request_for_radius(radius_chunks)
            .await
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = requestCameraChunkView)]
    pub fn request_camera_chunk_view(&mut self, radius_chunks: u32) -> Result<JsValue, JsValue> {
        self.request_camera_chunk_view_for_radius(radius_chunks)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = tryBeginCameraRenderCompileRequest)]
    pub fn try_begin_camera_render_compile_request(
        &mut self,
        radius_chunks: u32,
        center_x: i32,
        center_z: i32,
    ) -> Result<JsValue, JsValue> {
        self.try_begin_camera_render_compile_request_for_center(
            radius_chunks,
            ChunkPos {
                x: center_x,
                z: center_z,
            },
        )
        .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = finishCameraRenderCompileRequest)]
    pub fn finish_camera_render_compile_request(
        &mut self,
        request_id: u32,
        packed_report: js_sys::Uint8Array,
    ) -> Result<JsValue, JsValue> {
        self.finish_camera_render_compile_request_with_packed_report(
            request_id,
            packed_report.to_vec(),
        )
        .and_then(GeneratedChunkRenderReport::to_js_value)
        .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = renderCameraFrame)]
    pub fn render_camera_frame(&mut self, radius_chunks: u32) -> Result<JsValue, JsValue> {
        self.render_camera_frame_for_radius(radius_chunks)
            .and_then(GeneratedChunkRenderReport::to_js_value)
            .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = shutdown)]
    pub fn shutdown(&mut self) -> Result<JsValue, JsValue> {
        self.runtime.request_shutdown();
        let object = js_sys::Object::new();
        set_bool(&object, "ok", true).map_err(JsValue::from)?;
        set_string(&object, "runnerKind", self.runtime.runner_kind().label())
            .map_err(JsValue::from)?;
        Ok(object.into())
    }

    #[wasm_bindgen(js_name = renderChunkReportFromPackedSections)]
    pub async fn render_chunk_report_from_packed_sections(
        &mut self,
        center_x: i32,
        center_z: i32,
        radius_chunks: u32,
        packed_report: js_sys::Uint8Array,
    ) -> Result<JsValue, JsValue> {
        self.render_chunk_report_for_center_from_packed_report(
            ChunkPos {
                x: center_x,
                z: center_z,
            },
            radius_chunks,
            packed_report.to_vec(),
        )
        .await
        .and_then(GeneratedChunkRenderReport::to_js_value)
        .map_err(JsValue::from)
    }
}

impl WebChunkRenderSession {
    async fn new(canvas: HtmlCanvasElement, asset_pack_bytes: Vec<u8>) -> Result<Self, String> {
        Self::new_with_runtime(
            canvas,
            asset_pack_bytes,
            WebRuntime::local_integrated(SMOKE_SEED),
        )
        .await
    }

    async fn new_with_worker(
        canvas: HtmlCanvasElement,
        asset_pack_bytes: Vec<u8>,
        config: WebIntegratedServerRunnerConfig,
    ) -> Result<Self, String> {
        let runtime = WebRuntime::web_worker_integrated(config).await?;
        Self::new_with_runtime(canvas, asset_pack_bytes, runtime).await
    }

    async fn new_with_runtime(
        canvas: HtmlCanvasElement,
        asset_pack_bytes: Vec<u8>,
        runtime: WebRuntime,
    ) -> Result<Self, String> {
        let mesh_assets = load_textured_mesh_assets_from_pack(asset_pack_bytes)?;
        let context = WebCanvasContext::new(canvas).await?;
        let depth = ChunkDepthTarget::new(&context.device, context.width, context.height);
        let sky = SkyRenderer::new(&context.device, context.format);
        let actors = ActorDrawResources::new(
            &context.device,
            &context.queue,
            context.format,
            mesh_assets.actor_atlas.as_upload(),
        )
        .map_err(|error| format!("failed to upload packed actor textures: {error:#}"))?;

        Ok(Self {
            context,
            depth,
            sky,
            runtime,
            camera: EngineCameraController::spawn_for_chunk(SMOKE_INITIAL_CENTER),
            interaction: ClientInteractionController::new(),
            actor_interpolation: ActorInterpolationState::new(),
            mesh_assets,
            draw: None,
            actors,
            loaded_chunk_positions: BTreeSet::new(),
            loaded_center: None,
            compile_queue: RenderViewCompileQueue::default(),
            asset_pack_parse_count: 1,
            terrain_asset_load_count: 1,
            atlas_upload_count: 0,
            mesh_build_count: 0,
            mesh_upload_count: 0,
            render_count: 0,
            compile_requests: RenderSectionCompileRequestState::default(),
            render_compiler: WebRenderSectionCompiler::new(),
            shared_compile: None,
        })
    }

    async fn begin_chunk_render_compile_request_for_center(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
    ) -> Result<JsValue, String> {
        if self.compile_requests.has_pending_requests() {
            return Err(format!(
                "web render compile request already pending ({} pending)",
                self.compile_requests.pending_request_count()
            ));
        }
        let render_plan = self
            .prepare_chunk_render_plan(center, radius_chunks)
            .await?;
        let context = WebPendingCompileContext {
            center,
            radius_chunks,
            sync: render_plan.sync,
            removal_chunks: render_plan
                .sync_plan
                .dirty_work
                .removal_dirty_chunks
                .clone(),
            removal_sections: render_plan
                .sync_plan
                .dirty_work
                .removal_dirty_sections
                .clone(),
            scope: render_plan.scope,
        };
        let snapshots = self
            .runtime
            .client()
            .chunk_snapshots()
            .cloned()
            .collect::<Vec<_>>();
        let snapshot_input_chunk_count = snapshots.len();
        let snapshot_input_bytes = encode_web_render_compile_input(&snapshots)?;
        let compile_requests = &mut self.compile_requests;
        let submission = self.runtime.engine_mut().submit_prepared_sync_plan(
            &render_plan.sync_plan,
            snapshots,
            |_sync_plan, request| {
                let target_sections = request.target_sections.clone();
                let info = compile_requests.begin_compile_request(context, request);
                Ok::<_, String>((info, target_sections))
            },
        )?;
        let Some(submission) = submission.submission else {
            return Err(format!(
                "web render compile request for {},{} radius {} had no ready sections",
                center.x, center.z, radius_chunks
            ));
        };
        let (info, target_sections) = submission.output;
        self.compile_request_to_js_value(
            info,
            center,
            radius_chunks,
            render_plan.scope,
            &target_sections,
            &snapshot_input_bytes,
            snapshot_input_chunk_count,
        )
    }

    fn finish_chunk_render_compile_request_with_packed_report(
        &mut self,
        request_id: u32,
        packed_report: Vec<u8>,
    ) -> Result<GeneratedChunkRenderReport, String> {
        let pending = self
            .compile_requests
            .remove_pending_request(request_id)
            .ok_or_else(|| format!("unknown web render compile request id {request_id}"))?;
        let packed_byte_length = packed_report.len();
        let section_report = decode_textured_render_section_build_report(&packed_report)
            .map_err(|error| format!("failed to decode packed render section report: {error:#}"))?;
        let summary = summarize_textured_render_section_build_report(&section_report);
        let (context, completed) = compile_result_from_worker_report(pending, section_report);
        self.finish_render_compile_result(
            context.center,
            context.radius_chunks,
            context.sync,
            context.removal_chunks,
            context.removal_sections,
            context.scope,
            completed,
            WebSectionCompileReport::worker(packed_byte_length, summary),
            request_id,
            WebFrameCamera::Overview {
                center: context.center,
                radius_chunks: context.radius_chunks,
            },
        )
    }

    async fn begin_camera_render_compile_request_for_radius(
        &mut self,
        radius_chunks: u32,
    ) -> Result<JsValue, String> {
        let center = self.camera.snapshot().chunk_pos;
        self.begin_chunk_render_compile_request_for_center(center, radius_chunks)
            .await
    }

    fn request_camera_render_compile_decision(
        &mut self,
        trigger: &str,
        force: bool,
        compile_busy: bool,
    ) -> Result<JsValue, String> {
        let center = self.camera.snapshot().chunk_pos;
        let request = QueuedRenderViewCompile::new(center, force, trigger.to_owned());
        let busy = self.compile_queue_busy(compile_busy);
        let decision = self
            .compile_queue
            .request(request, self.loaded_center, busy);
        self.compile_queue_decision_to_js_value(decision)
    }

    fn take_queued_camera_render_compile_decision(
        &mut self,
        compile_busy: bool,
    ) -> Result<JsValue, String> {
        let busy = self.compile_queue_busy(compile_busy);
        let decision = self.compile_queue.take_next(self.loaded_center, busy);
        self.compile_queue_decision_to_js_value(decision)
    }

    fn compile_queue_busy(&self, compile_busy: bool) -> bool {
        compile_busy || self.compile_requests.has_pending_requests()
    }

    fn request_camera_chunk_view_for_radius(
        &mut self,
        radius_chunks: u32,
    ) -> Result<JsValue, String> {
        let center = self.camera.snapshot().chunk_pos;
        let step =
            self.runtime
                .request_chunk_view_deferred(center, radius_chunks, radius_chunks)?;
        self.chunk_view_request_to_js_value(center, radius_chunks, step)
    }

    fn try_begin_camera_render_compile_request_for_center(
        &mut self,
        radius_chunks: u32,
        center: ChunkPos,
    ) -> Result<JsValue, String> {
        let step = self
            .runtime
            .drain_pending_runner_updates_budgeted(WEB_COMPILE_WAIT_UPDATE_DRAIN_BUDGET)?;
        let diagnostics = self.runtime.runner_diagnostics();
        let center_loaded = self.runtime.client().chunk_snapshot(center).is_some();
        if !center_loaded || !web_runner_diagnostics_settled(&diagnostics) {
            return self.waiting_compile_request_to_js_value(
                center,
                radius_chunks,
                center_loaded,
                step,
                diagnostics,
            );
        }
        self.begin_loaded_chunk_render_compile_request_for_center(
            center,
            radius_chunks,
            step,
            diagnostics,
        )
    }

    fn finish_camera_render_compile_request_with_packed_report(
        &mut self,
        request_id: u32,
        packed_report: Vec<u8>,
    ) -> Result<GeneratedChunkRenderReport, String> {
        let pending = self
            .compile_requests
            .remove_pending_request(request_id)
            .ok_or_else(|| format!("unknown web render compile request id {request_id}"))?;
        let packed_byte_length = packed_report.len();
        let section_report = decode_textured_render_section_build_report(&packed_report)
            .map_err(|error| format!("failed to decode packed render section report: {error:#}"))?;
        let summary = summarize_textured_render_section_build_report(&section_report);
        let (context, completed) = compile_result_from_worker_report(pending, section_report);
        self.finish_render_compile_result(
            context.center,
            context.radius_chunks,
            context.sync,
            context.removal_chunks,
            context.removal_sections,
            context.scope,
            completed,
            WebSectionCompileReport::worker(packed_byte_length, summary),
            request_id,
            WebFrameCamera::Camera,
        )
    }

    fn render_camera_frame_for_radius(
        &mut self,
        radius_chunks: u32,
    ) -> Result<GeneratedChunkRenderReport, String> {
        if self.draw.is_none() {
            return Err(
                "camera frame requested before initial render sections were uploaded".into(),
            );
        }
        let center = self.camera.snapshot().chunk_pos;
        let cached_sections = self.runtime.engine().sections();
        let current_section_keys = cached_sections
            .iter()
            .map(|section| section.key)
            .collect::<BTreeSet<_>>();
        if cached_sections.iter().all(|section| section.is_empty()) {
            return Err("camera frame has no resident textured sections".into());
        }
        self.render_chunk_report_with_cache_update(
            center,
            radius_chunks,
            self.loaded_chunk_positions.clone(),
            current_section_keys,
            RenderSectionCacheUpdate::default(),
            WebSectionCompileReport::default(),
            WebCompileScopeReport::default(),
            RenderSectionCompileAcceptanceReport::default(),
            WebFrameCamera::Camera,
            false,
        )
    }

    async fn interact_block_with_kind(
        &mut self,
        kind: WebBlockInteractionKind,
    ) -> Result<WebBlockInteractionReport, String> {
        let command_count_before = self.runtime.command_count();
        let update_count_before = self.runtime.update_count();
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
                interaction_command_count: 0,
                interaction_update_count: 0,
                total_command_count: self.runtime.command_count(),
                total_update_count: self.runtime.update_count(),
                pending_compile_job_count: self.compile_requests.pending_request_count(),
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
            interaction_command_count: step.command_count,
            interaction_update_count: step.update_count,
            total_command_count: self.runtime.command_count(),
            total_update_count: self.runtime.update_count(),
            pending_compile_job_count: self.compile_requests.pending_request_count(),
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
            pending_compile_job_count: self.compile_requests.pending_request_count(),
        }
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

    async fn render_chunk_report_for_center_from_packed_report(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
        packed_report: Vec<u8>,
    ) -> Result<GeneratedChunkRenderReport, String> {
        let packed_byte_length = packed_report.len();
        let section_report = decode_textured_render_section_build_report(&packed_report)
            .map_err(|error| format!("failed to decode packed render section report: {error:#}"))?;
        let summary = summarize_textured_render_section_build_report(&section_report);
        let render_plan = self
            .prepare_chunk_render_plan(center, radius_chunks)
            .await?;
        self.render_prepared_section_report(
            center,
            radius_chunks,
            render_plan,
            section_report,
            WebSectionCompileReport::worker(packed_byte_length, summary),
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

    fn prepare_loaded_chunk_render_plan(
        &mut self,
        center: ChunkPos,
    ) -> Result<RenderSectionViewSync, String> {
        if self.runtime.client().chunk_snapshot(center).is_none() {
            return Err(format!(
                "web runtime did not publish generated chunk {},{}",
                center.x, center.z
            ));
        }
        let previous_loaded_chunks = self.loaded_chunk_positions.clone();
        Ok(self
            .runtime
            .engine_mut()
            .apply_current_loaded_view_sync(&previous_loaded_chunks))
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

    fn begin_loaded_chunk_render_compile_request_for_center(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
        step: super::WebRuntimeStepReport,
        diagnostics: mclone_server::ServerRunnerDiagnostics,
    ) -> Result<JsValue, String> {
        if self.compile_requests.pending_request_count() != 0 {
            return Err(format!(
                "web render compile request already pending ({} pending)",
                self.compile_requests.pending_request_count()
            ));
        }
        let sync = self.prepare_loaded_chunk_render_plan(center)?;
        let render_plan = self.prepare_render_plan_from_sync(sync)?;
        if render_plan
            .sync_plan
            .ready_plan
            .ready_section_keys
            .is_empty()
        {
            return self.waiting_compile_request_to_js_value(
                center,
                radius_chunks,
                true,
                step,
                diagnostics,
            );
        }
        let context = WebPendingCompileContext {
            center,
            radius_chunks,
            sync: render_plan.sync,
            removal_chunks: render_plan
                .sync_plan
                .dirty_work
                .removal_dirty_chunks
                .clone(),
            removal_sections: render_plan
                .sync_plan
                .dirty_work
                .removal_dirty_sections
                .clone(),
            scope: render_plan.scope,
        };
        let snapshots = self
            .runtime
            .client()
            .chunk_snapshots()
            .cloned()
            .collect::<Vec<_>>();
        let snapshot_input_chunk_count = snapshots.len();
        let snapshot_input_bytes = encode_web_render_compile_input(&snapshots)?;
        let compile_requests = &mut self.compile_requests;
        let submission = self.runtime.engine_mut().submit_prepared_sync_plan(
            &render_plan.sync_plan,
            snapshots,
            |_sync_plan, request| {
                let target_sections = request.target_sections.clone();
                let info = compile_requests.begin_compile_request(context, request);
                Ok::<_, String>((info, target_sections))
            },
        )?;
        let Some(submission) = submission.submission else {
            return Err(format!(
                "web render compile request for {},{} radius {} had no ready sections",
                center.x, center.z, radius_chunks
            ));
        };
        let (info, target_sections) = submission.output;
        self.compile_request_to_js_value(
            info,
            center,
            radius_chunks,
            render_plan.scope,
            &target_sections,
            &snapshot_input_bytes,
            snapshot_input_chunk_count,
        )
    }

    // ---- 067 Stage 2: shared-ring camera render compile (submit + poll) ----

    async fn submit_camera_render_compile_for_radius(
        &mut self,
        radius_chunks: u32,
    ) -> Result<JsValue, String> {
        self.ensure_shared_compile_ready()?;
        let center = self.camera.snapshot().chunk_pos;
        let render_plan = self.prepare_chunk_render_plan(center, radius_chunks).await?;
        self.submit_shared_render_compile(render_plan, center, radius_chunks, WebFrameCamera::Camera)
    }

    fn submit_deferred_camera_render_compile_for_center(
        &mut self,
        radius_chunks: u32,
        center: ChunkPos,
    ) -> Result<JsValue, String> {
        self.ensure_shared_compile_ready()?;
        let step = self
            .runtime
            .drain_pending_runner_updates_budgeted(WEB_COMPILE_WAIT_UPDATE_DRAIN_BUDGET)?;
        let diagnostics = self.runtime.runner_diagnostics();
        let center_loaded = self.runtime.client().chunk_snapshot(center).is_some();
        if !center_loaded || !web_runner_diagnostics_settled(&diagnostics) {
            return self.waiting_compile_request_to_js_value(
                center,
                radius_chunks,
                center_loaded,
                step,
                diagnostics,
            );
        }
        let sync = self.prepare_loaded_chunk_render_plan(center)?;
        let render_plan = self.prepare_render_plan_from_sync(sync)?;
        if render_plan
            .sync_plan
            .ready_plan
            .ready_section_keys
            .is_empty()
        {
            return self.waiting_compile_request_to_js_value(
                center,
                radius_chunks,
                true,
                step,
                diagnostics,
            );
        }
        self.submit_shared_render_compile(render_plan, center, radius_chunks, WebFrameCamera::Camera)
    }

    fn ensure_shared_compile_ready(&self) -> Result<(), String> {
        if !self.render_compiler.shared_supported() {
            return Err(
                "shared render compile transport unavailable (no SharedArrayBuffer/Atomics)".into(),
            );
        }
        if self.shared_compile.is_some() {
            return Err("web shared render compile already in flight".into());
        }
        Ok(())
    }

    fn submit_shared_render_compile(
        &mut self,
        render_plan: WebChunkRenderPlan,
        center: ChunkPos,
        radius_chunks: u32,
        frame_camera: WebFrameCamera,
    ) -> Result<JsValue, String> {
        let WebChunkRenderPlan {
            sync,
            sync_plan,
            scope,
        } = render_plan;
        let removal_chunks = sync_plan.dirty_work.removal_dirty_chunks.clone();
        let removal_sections = sync_plan.dirty_work.removal_dirty_sections.clone();
        let snapshots = self
            .runtime
            .client()
            .chunk_snapshots()
            .cloned()
            .collect::<Vec<_>>();
        let snapshot_input_chunk_count = snapshots.len();
        let render_compiler = &mut self.render_compiler;
        let submission = self.runtime.engine_mut().submit_prepared_sync_plan(
            &sync_plan,
            snapshots,
            |_sync_plan, request| {
                let target_sections = request.target_sections.clone();
                render_compiler
                    .submit(request)
                    .map_err(|error| format!("{error:#}"))?;
                Ok::<_, String>(target_sections)
            },
        )?;
        let Some(submission) = submission.submission else {
            return Err(format!(
                "web render compile request for {},{} radius {} had no ready sections",
                center.x, center.z, radius_chunks
            ));
        };
        let target_sections = submission.output;
        let request_id = self
            .render_compiler
            .in_flight_request_id()
            .ok_or_else(|| "shared render compile submit recorded no in-flight request".to_string())?;
        self.shared_compile = Some(WebSharedCompileContext {
            request_id,
            center,
            radius_chunks,
            sync,
            removal_chunks,
            removal_sections,
            scope,
            frame_camera,
        });
        self.shared_compile_request_to_js_value(
            request_id,
            center,
            radius_chunks,
            scope,
            &target_sections,
            snapshot_input_chunk_count,
        )
    }

    fn shared_compile_request_to_js_value(
        &self,
        request_id: u32,
        center: ChunkPos,
        radius_chunks: u32,
        scope: WebCompileScopeReport,
        target_sections: &BTreeSet<RenderSectionKey>,
        snapshot_input_chunk_count: usize,
    ) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        set_bool(&object, "ok", true)?;
        set_bool(&object, "waiting", false)?;
        set_bool(&object, "shared", true)?;
        set_number(&object, "requestId", f64::from(request_id))?;
        set_number(&object, "centerX", f64::from(center.x))?;
        set_number(&object, "centerZ", f64::from(center.z))?;
        set_number(&object, "radiusChunks", f64::from(radius_chunks))?;
        write_compile_scope_report(&object, scope)?;
        write_target_sections(&object, target_sections)?;
        set_number(
            &object,
            "snapshotInputChunkCount",
            snapshot_input_chunk_count as f64,
        )?;
        set_number(
            &object,
            "submittedCompileSectionCount",
            target_sections.len() as f64,
        )?;
        set_number(
            &object,
            "pendingCompileJobCount",
            self.render_compiler.pending_job_count() as f64,
        )?;
        // Resident shared buffers + byte counts for the worker doorbell. JavaScript
        // relays this message verbatim (plus kind + bindgen URLs); the result data
        // path stays in main wasm.
        self.render_compiler.write_doorbell_arenas(&object)?;
        Ok(object.into())
    }

    fn poll_camera_render_compile_result(&mut self) -> Result<JsValue, String> {
        let mut completed = self
            .render_compiler
            .try_recv_completed()
            .map_err(|error| format!("{error:#}"))?;
        let Some(result) = completed.pop() else {
            return self.pending_shared_compile_to_js_value();
        };
        let Some(context) = self.shared_compile.take() else {
            return Err(
                "shared render compile poll received a result with no in-flight context".into(),
            );
        };
        let compile_report = match &result.result {
            Ok(report) => WebSectionCompileReport::worker(
                self.render_compiler.last_packed_byte_length,
                summarize_textured_render_section_build_report(report),
            ),
            Err(_) => WebSectionCompileReport::default(),
        };
        self.finish_render_compile_result(
            context.center,
            context.radius_chunks,
            context.sync,
            context.removal_chunks,
            context.removal_sections,
            context.scope,
            result,
            compile_report,
            context.request_id,
            context.frame_camera,
        )
        .and_then(GeneratedChunkRenderReport::to_js_value)
    }

    fn pending_shared_compile_to_js_value(&self) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        let in_flight = self.shared_compile.is_some();
        set_bool(&object, "ok", true)?;
        set_bool(&object, "pending", in_flight)?;
        set_bool(&object, "idle", !in_flight)?;
        if let Some(context) = self.shared_compile.as_ref() {
            set_number(&object, "requestId", f64::from(context.request_id))?;
            set_number(&object, "centerX", f64::from(context.center.x))?;
            set_number(&object, "centerZ", f64::from(context.center.z))?;
        }
        set_number(
            &object,
            "pendingCompileJobCount",
            self.render_compiler.pending_job_count() as f64,
        )?;
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
        if cached_sections.iter().all(|section| section.is_empty()) {
            return Err(format!(
                "generated chunk view {},{} radius {} built no resident textured sections",
                center.x, center.z, radius_chunks
            ));
        }
        let upload_report = self.upload_sections(&cache_update, &cached_sections)?;
        let frame_camera = chunk_camera_for_frame(frame_camera, &self.camera, radius_chunks);
        let render_view = frame_camera.render_view(self.context.width, self.context.height);
        let day_time = self.runtime.client().day_time();
        let time_of_day = self.runtime.client().time_of_day();
        let sun_angle = self.runtime.client().sun_angle();
        let runner_diagnostics = self.runtime.runner_diagnostics();
        let sky_clear_color = mclone_render::sky::overworld_clear_color(time_of_day);
        let render_options = TexturedSectionRenderOptions::default()
            .with_sky_darken(mclone_render::light_texture::sky_darken(time_of_day));
        let actor_instances = self.interpolated_actor_instances();

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
        let draw = self
            .draw
            .as_ref()
            .ok_or_else(|| "textured draw resources were not initialized".to_owned())?;
        self.sky.render(
            &self.context.queue,
            &mut encoder,
            &view,
            sky_clear_color,
            render_view.sky_view_projection(),
            time_of_day,
            sun_angle,
        );
        let render_target = ChunkRenderTarget::new(
            &view,
            &self.depth.view,
            [self.context.width, self.context.height],
            sky_clear_color,
        )
        .with_loaded_color();
        let render_stats = draw
            .render_with_options(
                &self.context.queue,
                &mut encoder,
                render_target,
                render_view,
                render_options,
            )
            .map_err(|error| format!("failed to render generated section meshes: {error:#}"))?;
        let frame_target =
            RenderFrameTarget::color(&view, [self.context.width, self.context.height])
                .with_depth(&self.depth.view);
        let actor_stats = self
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
        self.context.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        self.render_count += 1;
        self.loaded_chunk_positions = current_loaded_chunks;
        if count_mesh_build {
            self.loaded_center = Some(center);
        }

        Ok(GeneratedChunkRenderReport {
            center,
            radius_chunks,
            camera_state: self.camera.frame_state(&self.interaction),
            width: self.context.width,
            height: self.context.height,
            day_time,
            time_of_day,
            sun_angle,
            sky_rendered: true,
            command_count: self.runtime.command_count(),
            update_count: self.runtime.update_count(),
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
            compile_request_id: acceptance_report.request_id,
            pending_compile_job_count: self.compile_requests.pending_request_count(),
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

    fn compile_request_to_js_value(
        &self,
        info: RenderSectionCompileRequestInfo,
        center: ChunkPos,
        radius_chunks: u32,
        scope: WebCompileScopeReport,
        target_sections: &BTreeSet<RenderSectionKey>,
        snapshot_input_bytes: &[u8],
        snapshot_input_chunk_count: usize,
    ) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        set_bool(&object, "ok", true)?;
        set_number(&object, "requestId", f64::from(info.request_id))?;
        set_number(&object, "centerX", f64::from(center.x))?;
        set_number(&object, "centerZ", f64::from(center.z))?;
        set_number(&object, "radiusChunks", f64::from(radius_chunks))?;
        write_compile_scope_report(&object, scope)?;
        write_target_sections(&object, target_sections)?;
        let snapshot_input = js_sys::Uint8Array::from(snapshot_input_bytes);
        js_sys::Reflect::set(
            &object,
            &JsValue::from_str("snapshotInputBytes"),
            &snapshot_input,
        )
        .map(|_| ())
        .map_err(|_| "failed to set render compile request snapshot input bytes".to_string())?;
        set_number(
            &object,
            "snapshotInputByteLength",
            snapshot_input_bytes.len() as f64,
        )?;
        set_number(
            &object,
            "snapshotInputChunkCount",
            snapshot_input_chunk_count as f64,
        )?;
        set_number(
            &object,
            "submittedCompileSectionCount",
            info.submitted_section_count as f64,
        )?;
        set_number(
            &object,
            "pendingCompileJobCount",
            info.pending_compile_jobs as f64,
        )?;
        Ok(object.into())
    }

    fn compile_queue_decision_to_js_value(
        &self,
        decision: RenderViewCompileQueueDecision<String>,
    ) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        set_bool(&object, "ok", true)?;
        set_number(
            &object,
            "pendingCompileJobCount",
            self.compile_requests.pending_request_count() as f64,
        )?;
        match decision {
            RenderViewCompileQueueDecision::Start(request) => {
                write_compile_queue_request(&object, "start", true, request)?;
            }
            RenderViewCompileQueueDecision::Queued(request) => {
                write_compile_queue_request(&object, "queued", false, request)?;
            }
            RenderViewCompileQueueDecision::Skipped => {
                set_bool(&object, "startNow", false)?;
                set_bool(&object, "queued", false)?;
                set_bool(&object, "skipped", true)?;
            }
            RenderViewCompileQueueDecision::Idle => {
                set_bool(&object, "startNow", false)?;
                set_bool(&object, "queued", false)?;
                set_bool(&object, "skipped", false)?;
            }
        }
        if let Some(center) = self.loaded_center {
            set_number(&object, "loadedCenterX", f64::from(center.x))?;
            set_number(&object, "loadedCenterZ", f64::from(center.z))?;
        }
        Ok(object.into())
    }

    fn chunk_view_request_to_js_value(
        &self,
        center: ChunkPos,
        radius_chunks: u32,
        step: super::WebRuntimeStepReport,
    ) -> Result<JsValue, String> {
        let diagnostics = self.runtime.runner_diagnostics();
        let object = js_sys::Object::new();
        set_bool(&object, "ok", true)?;
        set_bool(&object, "waiting", true)?;
        set_bool(&object, "centerLoaded", false)?;
        set_number(&object, "centerX", f64::from(center.x))?;
        set_number(&object, "centerZ", f64::from(center.z))?;
        set_number(&object, "radiusChunks", f64::from(radius_chunks))?;
        set_number(
            &object,
            "pendingCompileJobCount",
            self.compile_requests.pending_request_count() as f64,
        )?;
        set_number(&object, "commandCountDelta", step.command_count as f64)?;
        set_number(&object, "updateCountDelta", step.update_count as f64)?;
        set_bool(&object, "transportDrained", step.transport_drained)?;
        write_runner_wait_diagnostics(&object, diagnostics)?;
        Ok(object.into())
    }

    fn waiting_compile_request_to_js_value(
        &self,
        center: ChunkPos,
        radius_chunks: u32,
        center_loaded: bool,
        step: super::WebRuntimeStepReport,
        diagnostics: mclone_server::ServerRunnerDiagnostics,
    ) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        set_bool(&object, "ok", true)?;
        set_bool(&object, "waiting", true)?;
        set_bool(&object, "centerLoaded", center_loaded)?;
        set_number(&object, "centerX", f64::from(center.x))?;
        set_number(&object, "centerZ", f64::from(center.z))?;
        set_number(&object, "radiusChunks", f64::from(radius_chunks))?;
        set_number(
            &object,
            "pendingCompileJobCount",
            self.compile_requests.pending_request_count() as f64,
        )?;
        set_number(&object, "commandCountDelta", step.command_count as f64)?;
        set_number(&object, "updateCountDelta", step.update_count as f64)?;
        set_number(
            &object,
            "loadedChunkCount",
            self.runtime.client().loaded_chunk_count() as f64,
        )?;
        set_bool(&object, "transportDrained", step.transport_drained)?;
        write_runner_wait_diagnostics(&object, diagnostics)?;
        Ok(object.into())
    }
}

fn web_runner_diagnostics_settled(diagnostics: &mclone_server::ServerRunnerDiagnostics) -> bool {
    diagnostics.command_queue_depth == 0
        && diagnostics.update_queue_depth == 0
        && diagnostics.pending_jobs == 0
        && diagnostics.pending_publications == 0
        && diagnostics.worldgen_mailbox_pending_jobs == 0
        && diagnostics.light_status_mailbox_pending_statuses == 0
}

fn write_compile_queue_request(
    object: &js_sys::Object,
    status: &str,
    start_now: bool,
    request: QueuedRenderViewCompile<String>,
) -> Result<(), String> {
    set_bool(object, "startNow", start_now)?;
    set_bool(object, "queued", !start_now)?;
    set_bool(object, "skipped", false)?;
    set_string(object, "status", status)?;
    set_string(object, "trigger", &request.metadata)?;
    set_bool(object, "force", request.force)?;
    set_number(object, "centerX", f64::from(request.center.x))?;
    set_number(object, "centerZ", f64::from(request.center.z))?;
    Ok(())
}

fn write_compile_scope_report(
    object: &js_sys::Object,
    scope: WebCompileScopeReport,
) -> Result<(), String> {
    set_number(
        object,
        "viewDirtyChunkCount",
        scope.view_dirty_chunk_count as f64,
    )?;
    set_number(
        object,
        "viewRemovalChunkCount",
        scope.view_removal_chunk_count as f64,
    )?;
    set_number(
        object,
        "loadedDirtyChunkCount",
        scope.loaded_dirty_chunk_count as f64,
    )?;
    set_number(
        object,
        "removalDirtyChunkCount",
        scope.removal_dirty_chunk_count as f64,
    )?;
    set_number(
        object,
        "staleDirtyChunkCount",
        scope.stale_dirty_chunk_count as f64,
    )?;
    set_number(
        object,
        "loadedDirtySectionCount",
        scope.loaded_dirty_section_count as f64,
    )?;
    set_number(
        object,
        "removalDirtySectionCount",
        scope.removal_dirty_section_count as f64,
    )?;
    set_number(
        object,
        "staleDirtySectionCount",
        scope.stale_dirty_section_count as f64,
    )?;
    set_number(
        object,
        "readyCompileSectionCount",
        scope.ready_compile_section_count as f64,
    )?;
    set_number(
        object,
        "deferredCompileSectionCount",
        scope.deferred_compile_section_count as f64,
    )?;
    set_number(
        object,
        "budgetedLoadedChunkCount",
        scope.budgeted_loaded_chunk_count as f64,
    )?;
    set_number(
        object,
        "budgetedDirtySectionChunkCount",
        scope.budgeted_dirty_section_chunk_count as f64,
    )?;
    Ok(())
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

fn write_runner_wait_diagnostics(
    object: &js_sys::Object,
    diagnostics: mclone_server::ServerRunnerDiagnostics,
) -> Result<(), String> {
    set_bool(
        object,
        "runnerSettled",
        web_runner_diagnostics_settled(&diagnostics),
    )?;
    set_string(object, "runnerKind", diagnostics.kind.label())?;
    set_number(
        object,
        "runnerCommandQueueDepth",
        diagnostics.command_queue_depth as f64,
    )?;
    set_number(
        object,
        "runnerUpdateQueueDepth",
        diagnostics.update_queue_depth as f64,
    )?;
    set_number(object, "runnerPendingJobs", diagnostics.pending_jobs as f64)?;
    set_number(
        object,
        "runnerPendingPublications",
        diagnostics.pending_publications as f64,
    )?;
    set_number(
        object,
        "worldgenMailboxPendingJobs",
        diagnostics.worldgen_mailbox_pending_jobs as f64,
    )?;
    set_number(
        object,
        "lightStatusMailboxPendingStatuses",
        diagnostics.light_status_mailbox_pending_statuses as f64,
    )?;
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

fn actor_instances_from_presentations(
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
            }
        })
        .collect()
}

fn glam_vec3_from_vec3d(value: Vec3d) -> glam::Vec3 {
    glam::Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

fn actor_light_probe_block_pos(actor: &ActorPresentation) -> BlockPos {
    BlockPos::containing(actor.feet_position.add(Vec3d::new(
        0.0,
        actor_light_probe_height(actor),
        0.0,
    )))
}

fn actor_light_probe_height(actor: &ActorPresentation) -> f64 {
    match actor.kind {
        ActorPresentationKind::RemotePlayer => LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        ActorPresentationKind::Entity(EntityKind::Cow) => 1.3,
        ActorPresentationKind::Entity(EntityKind::Chicken) => f64::from(actor.height) * 0.92,
    }
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
        atlas_sprite_count: assets.atlas_sprite_count,
        asset_pack_file_count,
    })
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
        let Some(format) = preferred_surface_format(&caps) else {
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
    fn web_render_compile_input_roundtrips_snapshots() {
        let mut first_blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        first_blocks[chunk_section_index(1, 2, 3)] = BlockStateId(42);
        let mut second_blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        second_blocks[chunk_section_index(4, 5, 6)] = BlockStateId(7);
        let snapshots = vec![
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

        let bytes = encode_web_render_compile_input(&snapshots).unwrap();
        assert!(bytes.len() > WEB_RENDER_COMPILE_INPUT_MAGIC.len());
        let decoded = decode_web_render_compile_input(&bytes).unwrap();
        assert_eq!(decoded, snapshots);

        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode_web_render_compile_input(&trailing).is_err());
    }
}

fn preferred_surface_format(caps: &wgpu::SurfaceCapabilities) -> Option<wgpu::TextureFormat> {
    [
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
    ]
    .into_iter()
    .find(|format| caps.formats.contains(format))
    .or_else(|| caps.formats.iter().copied().find(|format| format.is_srgb()))
    .or_else(|| caps.formats.first().copied())
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
