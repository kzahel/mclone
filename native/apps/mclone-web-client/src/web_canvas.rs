use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use super::{SMOKE_INITIAL_CENTER, SMOKE_RADIUS_CHUNKS, SMOKE_SEED, WebRuntime};
use mclone_assets::PackedAssetSource;
use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockStateId, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkSnapshot,
    SECTION_HEIGHT, chunk_section_index,
};
use mclone_mesh::{
    TextureAtlasImage, TexturedChunkMeshInput, TexturedMeshCatalog, TexturedVisibleChunkMesh,
    build_textured_visible_chunk_mesh, load_textured_terrain_assets, quad_face_count_from_indices,
};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, ChunkTextureAtlas, TexturedChunkDrawResources,
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
        let report = render_generated_chunk(canvas, asset_pack_bytes.to_vec())
            .await
            .map_err(JsValue::from)?;
        report.to_js_value().map_err(JsValue::from)
    })
}

#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&info.to_string()));
    }));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CanvasRenderReport {
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeneratedChunkRenderReport {
    width: u32,
    height: u32,
    command_count: usize,
    update_count: usize,
    loaded_chunk_count: usize,
    asset_pack_file_count: usize,
    atlas_width: u32,
    atlas_height: u32,
    atlas_sprite_count: usize,
    vertex_count: u32,
    index_count: u32,
    face_count: u32,
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
        set_number(&object, "width", f64::from(self.width))?;
        set_number(&object, "height", f64::from(self.height))?;
        set_number(&object, "commandCount", self.command_count as f64)?;
        set_number(&object, "updateCount", self.update_count as f64)?;
        set_number(&object, "loadedChunkCount", self.loaded_chunk_count as f64)?;
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

async fn render_generated_chunk(
    canvas: HtmlCanvasElement,
    asset_pack_bytes: Vec<u8>,
) -> Result<GeneratedChunkRenderReport, String> {
    let mesh_assets = load_textured_mesh_assets_from_pack(asset_pack_bytes)?;
    let mut runtime = WebRuntime::local_integrated(SMOKE_SEED);
    runtime
        .request_chunk_view(
            SMOKE_INITIAL_CENTER,
            SMOKE_RADIUS_CHUNKS,
            SMOKE_RADIUS_CHUNKS,
        )
        .map_err(|error| format!("failed to load generated chunk through web runtime: {error}"))?;
    let snapshot = runtime
        .client()
        .chunk_snapshot(SMOKE_INITIAL_CENTER)
        .ok_or_else(|| "web runtime did not publish the generated smoke chunk".to_owned())?;
    let mesh = build_textured_chunk_mesh(snapshot, &mesh_assets.catalog)?;
    if mesh.is_empty() {
        return Err("generated smoke chunk built an empty textured mesh".to_owned());
    }

    let stats = mesh.stats();
    let context = WebCanvasContext::new(canvas).await?;
    let frame = context
        .surface
        .get_current_texture()
        .map_err(|error| format!("failed to acquire WebGPU canvas frame: {error}"))?;
    let view = frame.texture.create_view(&Default::default());
    let depth = ChunkDepthTarget::new(&context.device, context.width, context.height);
    let draw = TexturedChunkDrawResources::new(
        &context.device,
        &context.queue,
        context.format,
        &mesh,
        atlas_upload(&mesh_assets.atlas),
    )
    .map_err(|error| format!("failed to upload generated textured chunk mesh: {error:#}"))?;
    let mut encoder = context
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_web_generated_textured_chunk_encoder"),
        });
    draw.render(
        &context.queue,
        &mut encoder,
        ChunkRenderTarget::new(
            &view,
            &depth.view,
            [context.width, context.height],
            mclone_render::default_clear_color(),
        ),
        ChunkCamera::overview_for_chunk(SMOKE_INITIAL_CENTER.x, SMOKE_INITIAL_CENTER.z)
            .render_view(context.width, context.height),
    )
    .map_err(|error| format!("failed to render generated chunk mesh: {error:#}"))?;
    context.queue.submit(std::iter::once(encoder.finish()));
    frame.present();

    Ok(GeneratedChunkRenderReport {
        width: context.width,
        height: context.height,
        command_count: runtime.command_count(),
        update_count: runtime.update_count(),
        loaded_chunk_count: runtime.client().loaded_chunk_count(),
        asset_pack_file_count: mesh_assets.asset_pack_file_count,
        atlas_width: mesh_assets.atlas.width,
        atlas_height: mesh_assets.atlas.height,
        atlas_sprite_count: mesh_assets.atlas_sprite_count,
        vertex_count: stats.vertex_count,
        index_count: stats.index_count,
        face_count: quad_face_count_from_indices(stats.index_count),
    })
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

fn build_textured_chunk_mesh(
    snapshot: &ChunkSnapshot,
    catalog: &TexturedMeshCatalog,
) -> Result<TexturedVisibleChunkMesh, String> {
    let blocks = snapshot_block_state_ids(snapshot)?;
    build_textured_visible_chunk_mesh(
        TexturedChunkMeshInput::new(
            snapshot.pos.x,
            snapshot.pos.z,
            snapshot.min_y,
            snapshot.height,
            &blocks,
        )
        .with_light_sections(&snapshot.light_sections),
        catalog,
    )
    .map_err(|error| format!("failed to build generated textured chunk mesh: {error}"))
}

fn snapshot_block_state_ids(snapshot: &ChunkSnapshot) -> Result<Vec<BlockStateId>, String> {
    if snapshot.height <= 0 || snapshot.height % SECTION_HEIGHT != 0 {
        return Err(format!(
            "chunk snapshot {:?} has invalid height {}",
            snapshot.pos, snapshot.height
        ));
    }
    let expected_len = snapshot.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
    let mut blocks = vec![AIR_BLOCK_STATE_ID; expected_len];
    let min_section_y = snapshot.min_y / SECTION_HEIGHT;
    let section_count = snapshot.height / SECTION_HEIGHT;

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
        for local_y in 0..SECTION_HEIGHT {
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    let section_index = chunk_section_index(local_x, local_y, local_z);
                    let chunk_local_y = section_offset * SECTION_HEIGHT + local_y;
                    let chunk_index = chunk_block_index(local_x, chunk_local_y, local_z);
                    blocks[chunk_index] = unpacked[section_index];
                }
            }
        }
    }

    Ok(blocks)
}

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
