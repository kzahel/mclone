use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use std::collections::{BTreeMap, BTreeSet};

use super::{SMOKE_INITIAL_CENTER, SMOKE_RADIUS_CHUNKS, SMOKE_SEED, WebRuntime};
use mclone_assets::PackedAssetSource;
#[cfg(test)]
use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockStateId, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, chunk_section_index,
};
use mclone_core::{ChunkPos, ChunkSnapshot, SECTION_HEIGHT};
use mclone_mesh::{
    RenderSectionKey, TextureAtlasImage, TexturedMeshCatalog, TexturedRenderSectionBuildReport,
    load_textured_terrain_assets,
};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, ChunkTextureAtlas,
    TexturedSectionDrawResources, TexturedSectionUploadReport,
};
use mclone_render_session::{
    CachedTexturedRenderSections, PackedRenderSectionBuildReportSummary, RenderSectionCacheUpdate,
    RenderSectionCompileResult, build_client_textured_sections,
    decode_textured_render_section_build_report, encode_textured_render_section_build_report,
    summarize_textured_render_section_build_report,
};

const CANVAS_OK_BIT: u32 = 1 << 0;
const CANVAS_RENDERED_BIT: u32 = 1 << 1;
const CANVAS_CONFIGURED_BIT: u32 = 1 << 2;
const CANVAS_WIDTH_SHIFT: u32 = 8;
const CANVAS_HEIGHT_SHIFT: u32 = 20;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeneratedChunkRenderReport {
    center: ChunkPos,
    radius_chunks: u32,
    width: u32,
    height: u32,
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
struct WebChunkViewSync {
    current_loaded_chunks: BTreeSet<ChunkPos>,
    dirty_chunks: BTreeSet<ChunkPos>,
    removal_chunks: BTreeSet<ChunkPos>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WebPendingCompileRequest {
    center: ChunkPos,
    radius_chunks: u32,
    sync: WebChunkViewSync,
    target_section_keys: BTreeSet<RenderSectionKey>,
    section_revisions: BTreeMap<RenderSectionKey, u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WebCompileAcceptanceReport {
    request_id: u32,
    submitted_section_count: usize,
    accepted_section_count: usize,
    stale_section_count: usize,
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
    runtime: WebRuntime,
    mesh_assets: WebTexturedMeshAssets,
    draw: Option<TexturedSectionDrawResources>,
    render_sections: CachedTexturedRenderSections,
    loaded_chunk_positions: BTreeSet<ChunkPos>,
    asset_pack_parse_count: usize,
    terrain_asset_load_count: usize,
    atlas_upload_count: usize,
    mesh_build_count: usize,
    mesh_upload_count: usize,
    render_count: usize,
    next_compile_request_id: u32,
    pending_compile_requests: BTreeMap<u32, WebPendingCompileRequest>,
    render_section_revisions: BTreeMap<RenderSectionKey, u64>,
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
        self.pending_compile_requests.len()
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

        Ok(Self {
            context,
            depth,
            runtime: WebRuntime::local_integrated(SMOKE_SEED),
            mesh_assets,
            draw: None,
            render_sections: CachedTexturedRenderSections::default(),
            loaded_chunk_positions: BTreeSet::new(),
            asset_pack_parse_count: 1,
            terrain_asset_load_count: 1,
            atlas_upload_count: 0,
            mesh_build_count: 0,
            mesh_upload_count: 0,
            render_count: 0,
            next_compile_request_id: 1,
            pending_compile_requests: BTreeMap::new(),
            render_section_revisions: BTreeMap::new(),
        })
    }

    fn begin_chunk_render_compile_request_for_center(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
    ) -> Result<JsValue, String> {
        if !self.pending_compile_requests.is_empty() {
            return Err(format!(
                "web render compile request already pending ({} pending)",
                self.pending_compile_requests.len()
            ));
        }
        let sync = self.prepare_chunk_view(center, radius_chunks)?;
        let target_section_keys = self.target_section_keys_for_dirty_chunks(&sync.dirty_chunks);
        let request_id = self.take_next_compile_request_id();
        self.bump_render_section_revisions(&target_section_keys);
        let section_revisions = target_section_keys
            .iter()
            .map(|key| (*key, self.render_section_revision(*key)))
            .collect::<BTreeMap<_, _>>();
        let pending = WebPendingCompileRequest {
            center,
            radius_chunks,
            sync,
            target_section_keys,
            section_revisions,
        };
        let submitted_section_count = pending.target_section_keys.len();
        self.pending_compile_requests.insert(request_id, pending);
        self.compile_request_to_js_value(request_id, center, radius_chunks, submitted_section_count)
    }

    fn finish_chunk_render_compile_request_with_packed_report(
        &mut self,
        request_id: u32,
        packed_report: Vec<u8>,
    ) -> Result<GeneratedChunkRenderReport, String> {
        let pending = self
            .pending_compile_requests
            .remove(&request_id)
            .ok_or_else(|| format!("unknown web render compile request id {request_id}"))?;
        let packed_byte_length = packed_report.len();
        let section_report = decode_textured_render_section_build_report(&packed_report)
            .map_err(|error| format!("failed to decode packed render section report: {error:#}"))?;
        let summary = summarize_textured_render_section_build_report(&section_report);
        let completed = RenderSectionCompileResult {
            target_sections: pending.target_section_keys,
            section_revisions: pending.section_revisions,
            result: Ok(section_report),
        };
        let acceptance = completed.partition_by_revision(|key| self.render_section_revision(key));
        if acceptance.accepted_sections.is_empty() {
            return Err(format!(
                "web render compile request {request_id} had no accepted sections ({} stale)",
                acceptance.stale_section_count()
            ));
        }
        let build_report = completed
            .result
            .map_err(|error| format!("web render compile request {request_id} failed: {error}"))?;
        let acceptance_report = WebCompileAcceptanceReport {
            request_id,
            submitted_section_count: acceptance.accepted_section_count()
                + acceptance.stale_section_count(),
            accepted_section_count: acceptance.accepted_section_count(),
            stale_section_count: acceptance.stale_section_count(),
        };
        self.render_chunk_report_with_section_report(
            pending.center,
            pending.radius_chunks,
            pending.sync,
            acceptance.accepted_sections,
            build_report,
            WebSectionCompileReport::worker(packed_byte_length, summary),
            acceptance_report,
        )
    }

    fn render_chunk_report_for_center(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
    ) -> Result<GeneratedChunkRenderReport, String> {
        let sync = self.prepare_chunk_view(center, radius_chunks)?;
        let section_report =
            build_client_textured_sections(self.runtime.client(), &self.mesh_assets.catalog)
                .map_err(|error| {
                    format!("failed to build generated textured render sections: {error:#}")
                })?;
        let ready_section_keys = ready_section_keys_for_report(&section_report, &sync.dirty_chunks);
        self.render_chunk_report_with_section_report(
            center,
            radius_chunks,
            sync,
            ready_section_keys,
            section_report,
            WebSectionCompileReport::default(),
            WebCompileAcceptanceReport::default(),
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
        let sync = self.prepare_chunk_view(center, radius_chunks)?;
        let ready_section_keys = ready_section_keys_for_report(&section_report, &sync.dirty_chunks);
        self.render_chunk_report_with_section_report(
            center,
            radius_chunks,
            sync,
            ready_section_keys,
            section_report,
            WebSectionCompileReport::worker(packed_byte_length, summary),
            WebCompileAcceptanceReport::default(),
        )
    }

    fn prepare_chunk_view(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
    ) -> Result<WebChunkViewSync, String> {
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
        let current_loaded_chunks = self
            .runtime
            .client()
            .loaded_chunk_positions()
            .collect::<BTreeSet<_>>();
        let dirty_chunks = dirty_chunk_positions(&previous_loaded_chunks, &current_loaded_chunks);
        let removal_chunks = previous_loaded_chunks
            .difference(&current_loaded_chunks)
            .copied()
            .collect::<BTreeSet<_>>();

        Ok(WebChunkViewSync {
            current_loaded_chunks,
            dirty_chunks,
            removal_chunks,
        })
    }

    fn render_chunk_report_with_section_report(
        &mut self,
        center: ChunkPos,
        radius_chunks: u32,
        sync: WebChunkViewSync,
        ready_section_keys: BTreeSet<RenderSectionKey>,
        section_report: TexturedRenderSectionBuildReport,
        compile_report: WebSectionCompileReport,
        acceptance_report: WebCompileAcceptanceReport,
    ) -> Result<GeneratedChunkRenderReport, String> {
        let current_loaded_chunks = sync.current_loaded_chunks;
        let removal_chunks = sync.removal_chunks;
        self.mesh_build_count += 1;
        let current_section_keys = section_report
            .sections
            .iter()
            .map(|section| section.key)
            .collect::<BTreeSet<_>>();
        let cache_update = self.render_sections.apply_build_report(
            &ready_section_keys,
            section_report,
            &removal_chunks,
            &BTreeSet::new(),
        );
        let cached_sections = self.render_sections.sections();
        if cached_sections.iter().all(|section| section.is_empty()) {
            return Err(format!(
                "generated chunk view {},{} radius {} built no resident textured sections",
                center.x, center.z, radius_chunks
            ));
        }
        let upload_report = self.upload_sections(&cache_update, &cached_sections)?;

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
        let render_stats = draw
            .render(
                &self.context.queue,
                &mut encoder,
                ChunkRenderTarget::new(
                    &view,
                    &self.depth.view,
                    [self.context.width, self.context.height],
                    mclone_render::default_clear_color(),
                ),
                ChunkCamera::overview_for_chunk_area(center.x, center.z, radius_chunks as i32)
                    .render_view(self.context.width, self.context.height),
            )
            .map_err(|error| format!("failed to render generated section meshes: {error:#}"))?;
        self.context.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        self.render_count += 1;
        self.loaded_chunk_positions = current_loaded_chunks;

        Ok(GeneratedChunkRenderReport {
            center,
            radius_chunks,
            width: self.context.width,
            height: self.context.height,
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
            pending_compile_job_count: self.pending_compile_requests.len(),
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

    fn take_next_compile_request_id(&mut self) -> u32 {
        let request_id = self.next_compile_request_id;
        self.next_compile_request_id = self.next_compile_request_id.wrapping_add(1).max(1);
        request_id
    }

    fn target_section_keys_for_dirty_chunks(
        &self,
        dirty_chunks: &BTreeSet<ChunkPos>,
    ) -> BTreeSet<RenderSectionKey> {
        self.runtime
            .client()
            .chunk_snapshots()
            .filter(|snapshot| dirty_chunks.contains(&snapshot.pos))
            .flat_map(render_section_keys_for_snapshot)
            .collect()
    }

    fn bump_render_section_revisions(&mut self, keys: &BTreeSet<RenderSectionKey>) {
        for key in keys {
            let revision = self.render_section_revisions.entry(*key).or_default();
            *revision = revision.wrapping_add(1);
        }
    }

    fn render_section_revision(&self, key: RenderSectionKey) -> u64 {
        self.render_section_revisions
            .get(&key)
            .copied()
            .unwrap_or_default()
    }

    fn compile_request_to_js_value(
        &self,
        request_id: u32,
        center: ChunkPos,
        radius_chunks: u32,
        submitted_section_count: usize,
    ) -> Result<JsValue, String> {
        let object = js_sys::Object::new();
        set_bool(&object, "ok", true)?;
        set_number(&object, "requestId", f64::from(request_id))?;
        set_number(&object, "centerX", f64::from(center.x))?;
        set_number(&object, "centerZ", f64::from(center.z))?;
        set_number(&object, "radiusChunks", f64::from(radius_chunks))?;
        set_number(
            &object,
            "submittedCompileSectionCount",
            submitted_section_count as f64,
        )?;
        set_number(
            &object,
            "pendingCompileJobCount",
            self.pending_compile_requests.len() as f64,
        )?;
        Ok(object.into())
    }
}

impl Default for WebCompileAcceptanceReport {
    fn default() -> Self {
        Self {
            request_id: 0,
            submitted_section_count: 0,
            accepted_section_count: 0,
            stale_section_count: 0,
        }
    }
}

fn chunk_pos_for_section(key: RenderSectionKey) -> ChunkPos {
    ChunkPos {
        x: key.chunk_x,
        z: key.chunk_z,
    }
}

fn ready_section_keys_for_report(
    report: &TexturedRenderSectionBuildReport,
    dirty_chunks: &BTreeSet<ChunkPos>,
) -> BTreeSet<RenderSectionKey> {
    report
        .sections
        .iter()
        .map(|section| section.key)
        .filter(|key| dirty_chunks.contains(&chunk_pos_for_section(*key)))
        .collect()
}

fn render_section_keys_for_snapshot(snapshot: &ChunkSnapshot) -> Vec<RenderSectionKey> {
    let min_section_y = snapshot.min_y.div_euclid(SECTION_HEIGHT);
    let section_count = snapshot.height / SECTION_HEIGHT;
    (0..section_count)
        .map(|offset| RenderSectionKey::new(snapshot.pos.x, min_section_y + offset, snapshot.pos.z))
        .collect()
}

fn dirty_chunk_positions(
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

    Ok(WebTexturedMeshAssets {
        catalog: assets.catalog,
        atlas: assets.atlas,
        atlas_sprite_count: assets.atlas_sprite_count,
        asset_pack_file_count,
    })
}

struct WebCanvasContext {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
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
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
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
            surface,
            device,
            queue,
            format,
            width,
            height,
        })
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
