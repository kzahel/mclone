use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use std::collections::{BTreeMap, BTreeSet};

use super::{SMOKE_INITIAL_CENTER, SMOKE_RADIUS_CHUNKS, SMOKE_SEED, WebRuntime};
use mclone_assets::PackedAssetSource;
use mclone_client::{
    ActorInterpolationConfig, ActorInterpolationState, ActorPresentation, ActorPresentationKind,
    ClientInteractionController, ClientRuntime, LOCAL_PLAYER_STANDING_EYE_HEIGHT,
};
#[cfg(test)]
use mclone_core::{AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, chunk_section_index};
use mclone_core::{
    BlockHitResult, BlockPos, BlockStateId, ChunkPos, Direction, HitResultType, Vec3d,
};
use mclone_mesh::{
    RenderSectionKey, TextureAtlasImage, TexturedMeshCatalog, TexturedRenderSectionBuildReport,
    load_textured_terrain_assets,
};
use mclone_protocol::EntityKind;
use mclone_render::actor_assets::{ActorTextureImage, load_actor_texture_assets};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, ChunkTextureAtlas,
    TexturedSectionDrawResources, TexturedSectionRenderOptions, TexturedSectionUploadReport,
};
use mclone_render::entity::{ActorDrawResources, ActorInstance};
use mclone_render::sky_render::SkyRenderer;
use mclone_render::target::RenderFrameTarget;
use mclone_render_session::{
    EngineCameraController, EngineCameraFrameState, EngineCameraInput, EngineRenderCamera,
    PackedRenderSectionBuildReportSummary, RenderSectionCacheUpdate,
    RenderSectionCompileAcceptanceReport, RenderSectionCompileRequestInfo,
    RenderSectionCompileRequestState, RenderSectionCompileResult, RenderSectionNeighborReadiness,
    RenderSectionRemovalMode, RenderSectionSyncPlan, RenderSectionViewSync,
    build_client_textured_sections, decode_textured_render_section_build_report,
    encode_textured_render_section_build_report, summarize_textured_render_section_build_report,
};

const CANVAS_OK_BIT: u32 = 1 << 0;
const CANVAS_RENDERED_BIT: u32 = 1 << 1;
const CANVAS_CONFIGURED_BIT: u32 = 1 << 2;
const CANVAS_WIDTH_SHIFT: u32 = 8;
const CANVAS_HEIGHT_SHIFT: u32 = 20;
const WEB_GROUND_PROBE_DISTANCE: f64 = 0.01;

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
    )
    .map_err(JsValue::from)?;
    let packed = encode_textured_render_section_build_report(&report);
    Ok(js_sys::Uint8Array::from(packed.as_slice()))
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
) -> Result<mclone_mesh::TexturedRenderSectionBuildReport, String> {
    let mesh_assets = load_textured_mesh_assets_from_pack(asset_pack_bytes)?;
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

    build_client_textured_sections(runtime.client(), &mesh_assets.catalog)
        .map_err(|error| format!("failed to compile generated textured render sections: {error:#}"))
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
    submitted_compile_section_count: usize,
    accepted_compile_section_count: usize,
    stale_compile_section_count: usize,
    worker_compile_used: bool,
    worker_packed_byte_length: usize,
    worker_section_count: usize,
    worker_non_empty_section_count: usize,
    worker_vertex_count: u32,
    worker_index_count: u32,
    worker_face_count: u32,
    actor_count: usize,
    drawn_actor_count: usize,
    drawn_actor_index_count: u32,
    actor_atlas_width: u32,
    actor_atlas_height: u32,
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
        Ok(object.into())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WebSectionCompileReport {
    worker_compile_used: bool,
    packed_byte_length: usize,
    section_count: usize,
    non_empty_section_count: usize,
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
            vertex_count: summary.vertex_count,
            index_count: summary.index_count,
            face_count: summary.face_count(),
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WebChunkRenderPlan {
    sync: RenderSectionViewSync,
    sync_plan: RenderSectionSyncPlan,
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
    asset_pack_parse_count: usize,
    terrain_asset_load_count: usize,
    atlas_upload_count: usize,
    mesh_build_count: usize,
    mesh_upload_count: usize,
    render_count: usize,
    compile_requests: RenderSectionCompileRequestState<WebPendingCompileContext>,
}

#[wasm_bindgen]
impl WebChunkRenderSession {
    #[wasm_bindgen(js_name = renderChunkReport)]
    pub fn render_chunk_report(
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
        .and_then(GeneratedChunkRenderReport::to_js_value)
        .map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = beginChunkRenderCompileRequest)]
    pub fn begin_chunk_render_compile_request(
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
        self.compile_requests.pending_request_count()
    }

    #[wasm_bindgen(js_name = cameraFrameState)]
    pub fn camera_frame_state(&self) -> Result<JsValue, JsValue> {
        camera_state_to_js_value(&self.camera, &self.interaction).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = advanceCameraFrame)]
    #[allow(clippy::too_many_arguments)]
    pub fn advance_camera_frame(
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
    ) -> Result<JsValue, JsValue> {
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
        };
        self.camera
            .apply_movement_input(self.runtime.client(), input);
        self.sync_camera_pose_to_server().map_err(JsValue::from)?;
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
    pub fn interact_block(&mut self, kind: &str) -> Result<JsValue, JsValue> {
        let kind = WebBlockInteractionKind::from_label(kind).map_err(JsValue::from)?;
        self.interact_block_with_kind(kind)
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
    pub fn begin_camera_render_compile_request(
        &mut self,
        radius_chunks: u32,
    ) -> Result<JsValue, JsValue> {
        self.begin_camera_render_compile_request_for_radius(radius_chunks)
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

    #[wasm_bindgen(js_name = renderChunkReportFromPackedSections)]
    pub fn render_chunk_report_from_packed_sections(
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
        .and_then(GeneratedChunkRenderReport::to_js_value)
        .map_err(JsValue::from)
    }
}

impl WebChunkRenderSession {
    async fn new(canvas: HtmlCanvasElement, asset_pack_bytes: Vec<u8>) -> Result<Self, String> {
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
            runtime: WebRuntime::local_integrated(SMOKE_SEED),
            camera: EngineCameraController::spawn_for_chunk(SMOKE_INITIAL_CENTER),
            interaction: ClientInteractionController::new(),
            actor_interpolation: ActorInterpolationState::new(),
            mesh_assets,
            draw: None,
            actors,
            loaded_chunk_positions: BTreeSet::new(),
            asset_pack_parse_count: 1,
            terrain_asset_load_count: 1,
            atlas_upload_count: 0,
            mesh_build_count: 0,
            mesh_upload_count: 0,
            render_count: 0,
            compile_requests: RenderSectionCompileRequestState::default(),
        })
    }

    fn begin_chunk_render_compile_request_for_center(
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
        let render_plan = self.prepare_chunk_render_plan(center, radius_chunks)?;
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
        };
        let compile_requests = &mut self.compile_requests;
        let submission = self.runtime.engine_mut().submit_prepared_sync_plan(
            &render_plan.sync_plan,
            Vec::new(),
            |_sync_plan, request| {
                Ok::<_, String>(compile_requests.begin_compile_request(context, request))
            },
        )?;
        let Some(submission) = submission.submission else {
            return Err(format!(
                "web render compile request for {},{} radius {} had no ready sections",
                center.x, center.z, radius_chunks
            ));
        };
        let info = submission.output;
        self.compile_request_to_js_value(info, center, radius_chunks)
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
        let (context, completed) = pending.into_compile_result(Ok(section_report));
        self.finish_render_compile_result(
            context.center,
            context.radius_chunks,
            context.sync,
            context.removal_chunks,
            context.removal_sections,
            completed,
            WebSectionCompileReport::worker(packed_byte_length, summary),
            request_id,
            WebFrameCamera::Overview {
                center: context.center,
                radius_chunks: context.radius_chunks,
            },
        )
    }

    fn begin_camera_render_compile_request_for_radius(
        &mut self,
        radius_chunks: u32,
    ) -> Result<JsValue, String> {
        let center = self.camera.snapshot().chunk_pos;
        self.begin_chunk_render_compile_request_for_center(center, radius_chunks)
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
        let (context, completed) = pending.into_compile_result(Ok(section_report));
        self.finish_render_compile_result(
            context.center,
            context.radius_chunks,
            context.sync,
            context.removal_chunks,
            context.removal_sections,
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
            RenderSectionCompileAcceptanceReport::default(),
            WebFrameCamera::Camera,
            false,
        )
    }

    fn interact_block_with_kind(
        &mut self,
        kind: WebBlockInteractionKind,
    ) -> Result<WebBlockInteractionReport, String> {
        let command_count_before = self.runtime.command_count();
        let update_count_before = self.runtime.update_count();
        self.sync_camera_pose_to_server()?;
        let carried_item_synced = self.sync_carried_item()?;

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
            .send_gameplay_command(command)
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

    fn sync_carried_item(&mut self) -> Result<bool, String> {
        let Some(command) = self.interaction.ensure_has_sent_carried_item() else {
            return Ok(false);
        };
        self.runtime
            .send_gameplay_command(command)
            .map_err(|error| format!("failed to sync browser carried item: {error}"))?;
        Ok(true)
    }

    fn sync_camera_pose_to_server(&mut self) -> Result<(), String> {
        self.apply_pending_camera_position_updates()?;
        if let Some(report) = self.camera.next_pose_sync_command() {
            self.runtime
                .send_gameplay_command(report.command)
                .map_err(|error| format!("failed to sync browser camera pose: {error}"))?;
            self.apply_pending_camera_position_updates()?;
        }
        Ok(())
    }

    fn apply_pending_camera_position_updates(&mut self) -> Result<(), String> {
        let updates = self
            .runtime
            .drain_player_position_updates()
            .collect::<Vec<_>>();
        for update in updates {
            let accepted = self.camera.accept_position_update(update);
            self.camera
                .probe_ground(self.runtime.client(), WEB_GROUND_PROBE_DISTANCE);
            self.runtime
                .send_gameplay_command(accepted.accept_command)
                .map_err(|error| format!("failed to accept browser camera correction: {error}"))?;
            let resync = self.camera.corrected_pose_sync_command();
            self.runtime
                .send_gameplay_command(resync.command)
                .map_err(|error| {
                    format!("failed to resync corrected browser camera pose: {error}")
                })?;
        }
        Ok(())
    }

    fn render_chunk_report_for_center(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
    ) -> Result<GeneratedChunkRenderReport, String> {
        let render_plan = self.prepare_chunk_render_plan(center, radius_chunks)?;
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
        if finished.accepted_sections.is_empty() && !has_removals {
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
            finished.acceptance_report,
            frame_camera,
            true,
        )
    }

    fn render_chunk_report_for_center_from_packed_report(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
        packed_report: Vec<u8>,
    ) -> Result<GeneratedChunkRenderReport, String> {
        let packed_byte_length = packed_report.len();
        let section_report = decode_textured_render_section_build_report(&packed_report)
            .map_err(|error| format!("failed to decode packed render section report: {error:#}"))?;
        let summary = summarize_textured_render_section_build_report(&section_report);
        let render_plan = self.prepare_chunk_render_plan(center, radius_chunks)?;
        self.render_prepared_section_report(
            center,
            radius_chunks,
            render_plan,
            section_report,
            WebSectionCompileReport::worker(packed_byte_length, summary),
        )
    }

    fn prepare_chunk_render_plan(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
    ) -> Result<WebChunkRenderPlan, String> {
        let sync = self.prepare_chunk_view(center, radius_chunks)?;
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
            sync,
            sync_plan: sync_update.sync_plan,
        })
    }

    fn prepare_chunk_view(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
    ) -> Result<RenderSectionViewSync, String> {
        let previous_loaded_chunks = self.loaded_chunk_positions.clone();
        self.runtime
            .request_chunk_view(center, radius_chunks, radius_chunks)
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
        acceptance_report: RenderSectionCompileAcceptanceReport,
        frame_camera: WebFrameCamera,
        count_mesh_build: bool,
    ) -> Result<GeneratedChunkRenderReport, String> {
        if count_mesh_build {
            self.mesh_build_count += 1;
        }
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
            submitted_compile_section_count: acceptance_report.submitted_section_count,
            accepted_compile_section_count: acceptance_report.accepted_section_count,
            stale_compile_section_count: acceptance_report.stale_section_count,
            worker_compile_used: compile_report.worker_compile_used,
            worker_packed_byte_length: compile_report.packed_byte_length,
            worker_section_count: compile_report.section_count,
            worker_non_empty_section_count: compile_report.non_empty_section_count,
            worker_vertex_count: compile_report.vertex_count,
            worker_index_count: compile_report.index_count,
            worker_face_count: compile_report.face_count,
            actor_count: actor_instances.len(),
            drawn_actor_count: actor_stats.drawn_actor_count,
            drawn_actor_index_count: actor_stats.index_count,
            actor_atlas_width: self.mesh_assets.actor_atlas.width,
            actor_atlas_height: self.mesh_assets.actor_atlas.height,
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
    ) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        set_bool(&object, "ok", true)?;
        set_number(&object, "requestId", f64::from(info.request_id))?;
        set_number(&object, "centerX", f64::from(center.x))?;
        set_number(&object, "centerZ", f64::from(center.z))?;
        set_number(&object, "radiusChunks", f64::from(radius_chunks))?;
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
