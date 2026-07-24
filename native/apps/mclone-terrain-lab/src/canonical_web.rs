use std::collections::{BTreeMap, BTreeSet};

use mclone_assets::{AssetSourceChain, PackedAssetSource};
use mclone_core::BlockStateId;
use mclone_mesh::{
    TexturedChunkMeshInput, TexturedMeshCatalog, TexturedRenderSectionMesh,
    TexturedVisibleChunkMesh, build_textured_render_sections_for_chunk_set,
    load_first_party_textured_terrain_assets,
};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, ChunkTextureAtlas,
    TexturedSectionDrawResources, TexturedSectionRenderOptions,
};
use mclone_terrain_view::{
    CanonicalTerrainVisibility, TerrainPreviewCamera, canonical_terrain_presentation_blocks,
};
use serde::Serialize;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
use web_sys::HtmlCanvasElement;

use crate::web::surface_configuration;

#[derive(Clone, Debug)]
struct ResidentCanonicalChunk {
    min_y: i32,
    height: i32,
    blocks: Vec<u8>,
    biomes: Vec<i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalAcceptReport {
    chunk_x: i32,
    chunk_z: i32,
    fingerprint: String,
    resident_chunks: usize,
    vertex_count: u32,
    index_count: u32,
    mesh_upload_ms: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalRenderReport {
    resident_chunks: usize,
    vertex_count: u32,
    index_count: u32,
    width: u32,
    height: u32,
    center_x: i32,
    center_z: i32,
    blocks_across: u32,
    view: String,
    water_visible: bool,
    vegetation_visible: bool,
    preview_lighting: &'static str,
}

#[wasm_bindgen]
pub struct CanonicalTerrainLab {
    canvas: HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    present_mode: wgpu::PresentMode,
    alpha_mode: wgpu::CompositeAlphaMode,
    width: u32,
    height: u32,
    seed: i64,
    catalog: TexturedMeshCatalog,
    chunks: BTreeMap<(i32, i32), ResidentCanonicalChunk>,
    visibility: CanonicalTerrainVisibility,
    draw: TexturedSectionDrawResources,
    depth: ChunkDepthTarget,
    vertex_count: u32,
    index_count: u32,
}

#[wasm_bindgen]
impl CanonicalTerrainLab {
    #[wasm_bindgen]
    pub fn resize(&mut self, width: u32, height: u32) {
        self.resize_surface(width, height);
    }

    #[wasm_bindgen(js_name = resetChunks)]
    pub fn reset_chunks(&mut self, seed: String) -> Result<(), JsValue> {
        self.seed = parse_seed(&seed)?;
        self.chunks.clear();
        self.draw
            .update_sections(&self.device, &[])
            .map_err(|error| js_error(format!("failed to clear canonical terrain: {error}")))?;
        self.vertex_count = 0;
        self.index_count = 0;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = acceptChunk)]
    pub fn accept_chunk(
        &mut self,
        seed: String,
        chunk_x: i32,
        chunk_z: i32,
        min_y: i32,
        height: i32,
        blocks: js_sys::Uint8Array,
        biomes: js_sys::Int32Array,
        fingerprint: String,
        water_visible: bool,
        vegetation_visible: bool,
    ) -> Result<String, JsValue> {
        let seed = parse_seed(&seed)?;
        if self.seed != seed {
            self.seed = seed;
            self.chunks.clear();
        }
        let expected_blocks = usize::try_from(height)
            .ok()
            .and_then(|height| height.checked_mul(16 * 16))
            .ok_or_else(|| js_error(format!("invalid canonical chunk height {height}")))?;
        let blocks = blocks.to_vec();
        if blocks.len() != expected_blocks {
            return Err(js_error(format!(
                "canonical chunk ({chunk_x}, {chunk_z}) has {} blocks; expected {expected_blocks}",
                blocks.len()
            )));
        }
        self.visibility = CanonicalTerrainVisibility {
            water: water_visible,
            vegetation: vegetation_visible,
        };
        self.chunks.insert(
            (chunk_x, chunk_z),
            ResidentCanonicalChunk {
                min_y,
                height,
                blocks,
                biomes: biomes.to_vec(),
            },
        );
        let started = now_ms()?;
        let targets = canonical_remesh_targets(&self.chunks, chunk_x, chunk_z);
        let (vertex_count, index_count) = self.rebuild_chunks(&targets)?;
        let report = CanonicalAcceptReport {
            chunk_x,
            chunk_z,
            fingerprint,
            resident_chunks: self.chunks.len(),
            vertex_count,
            index_count,
            mesh_upload_ms: now_ms()? - started,
        };
        json(&report)
    }

    #[wasm_bindgen(js_name = setPresentation)]
    pub fn set_presentation(
        &mut self,
        water_visible: bool,
        vegetation_visible: bool,
    ) -> Result<String, JsValue> {
        let visibility = CanonicalTerrainVisibility {
            water: water_visible,
            vegetation: vegetation_visible,
        };
        if self.visibility != visibility {
            self.visibility = visibility;
            let targets = self.chunks.keys().copied().collect::<BTreeSet<_>>();
            self.rebuild_chunks(&targets)?;
        }
        json(&CanonicalAcceptReport {
            chunk_x: 0,
            chunk_z: 0,
            fingerprint: String::new(),
            resident_chunks: self.chunks.len(),
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            mesh_upload_ms: 0.0,
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen]
    pub fn render(
        &mut self,
        center_x: i32,
        center_z: i32,
        blocks_across: u32,
        view: String,
        camera_yaw: f32,
        camera_pitch: f32,
    ) -> Result<String, JsValue> {
        let camera = TerrainPreviewCamera::new(camera_yaw, camera_pitch).map_err(js_error)?;
        let render_camera = canonical_camera(
            center_x,
            center_z,
            blocks_across,
            &view,
            camera,
            self.width,
            self.height,
        )?;
        let frame = self.acquire_frame()?;
        let color_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_terrain_lab_canonical_encoder"),
            });
        self.draw
            .render_with_options(
                &self.queue,
                &mut encoder,
                ChunkRenderTarget::new(
                    &color_view,
                    &self.depth.view,
                    [self.width, self.height],
                    wgpu::Color {
                        r: 0.31,
                        g: 0.48,
                        b: 0.65,
                        a: 1.0,
                    },
                ),
                render_camera.render_view(self.width, self.height),
                TexturedSectionRenderOptions {
                    section_occlusion_culling: false,
                    force_fullbright: true,
                    ..TexturedSectionRenderOptions::default()
                },
            )
            .map_err(|error| js_error(format!("failed to draw canonical terrain: {error}")))?;
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        json(&CanonicalRenderReport {
            resident_chunks: self.chunks.len(),
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            width: self.width,
            height: self.height,
            center_x,
            center_z,
            blocks_across,
            view,
            water_visible: self.visibility.water,
            vegetation_visible: self.visibility.vegetation,
            preview_lighting: "fullbright + face shade + ambient occlusion",
        })
    }
}

impl CanonicalTerrainLab {
    async fn new(
        canvas: HtmlCanvasElement,
        authored_bytes: js_sys::Uint8Array,
        fallback_bytes: js_sys::Uint8Array,
    ) -> Result<Self, String> {
        let mut source = AssetSourceChain::new();
        source.push(
            PackedAssetSource::from_bytes(authored_bytes.to_vec())
                .map_err(|error| format!("failed to parse authored first-party pack: {error}"))?,
        );
        source.push(
            PackedAssetSource::from_bytes(fallback_bytes.to_vec())
                .map_err(|error| format!("failed to parse fallback first-party pack: {error}"))?,
        );
        let assets = load_first_party_textured_terrain_assets(&source)
            .map_err(|error| format!("failed to load canonical terrain assets: {error}"))?;

        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|error| format!("failed to create canonical WebGPU surface: {error}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| format!("failed to request canonical WebGPU adapter: {error}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("mclone_terrain_lab_canonical_device"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .map_err(|error| format!("failed to request canonical WebGPU device: {error}"))?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or("canonical WebGPU surface reported no color formats")?;
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

        let draw = TexturedSectionDrawResources::new(
            &device,
            &queue,
            format,
            &[],
            ChunkTextureAtlas {
                width: assets.atlas.width,
                height: assets.atlas.height,
                rgba: assets.atlas.rgba(),
            },
        )
        .map_err(|error| format!("failed to initialize canonical terrain renderer: {error}"))?;
        let depth = ChunkDepthTarget::new(&device, width, height);
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
            seed: 0,
            catalog: assets.catalog,
            chunks: BTreeMap::new(),
            visibility: CanonicalTerrainVisibility::default(),
            draw,
            depth,
            vertex_count: 0,
            index_count: 0,
        })
    }

    fn rebuild_chunks(&mut self, targets: &BTreeSet<(i32, i32)>) -> Result<(u32, u32), JsValue> {
        if targets.is_empty() {
            return Ok((self.vertex_count, self.index_count));
        }
        let presented = self
            .chunks
            .values()
            .map(|chunk| {
                canonical_terrain_presentation_blocks(&chunk.blocks, self.visibility)
                    .into_iter()
                    .map(|block| BlockStateId(u32::from(block)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let inputs = self
            .chunks
            .iter()
            .zip(presented.iter())
            .map(|(((chunk_x, chunk_z), chunk), blocks)| {
                TexturedChunkMeshInput::new(*chunk_x, *chunk_z, chunk.min_y, chunk.height, blocks)
                    .with_biomes(&chunk.biomes)
                    .with_world_seed(self.seed)
                    .with_fluids_visible(self.visibility.water)
            })
            .collect::<Vec<_>>();
        let mut sections =
            build_textured_render_sections_for_chunk_set(&inputs, &self.catalog, targets)
                .map_err(|error| js_error(format!("failed to mesh canonical terrain: {error}")))?;
        suppress_missing_footprint_walls(&mut sections, &self.chunks);
        self.draw
            .apply_section_updates(&self.device, &sections, &BTreeSet::new())
            .map_err(|error| js_error(format!("failed to upload canonical terrain: {error}")))?;
        let vertex_count = self.draw.vertex_count();
        let index_count = self.draw.index_count();
        self.vertex_count = vertex_count;
        self.index_count = index_count;
        Ok((vertex_count, index_count))
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
        self.depth.resize(&self.device, width, height);
    }

    fn acquire_frame(&mut self) -> Result<wgpu::SurfaceTexture, JsValue> {
        match self.surface.get_current_texture() {
            Ok(frame) => Ok(frame),
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.resize_surface(self.width, self.height);
                self.surface.get_current_texture().map_err(|error| {
                    js_error(format!(
                        "failed to acquire canonical WebGPU frame after reconfigure: {error}"
                    ))
                })
            }
            Err(error) => Err(js_error(format!(
                "failed to acquire canonical WebGPU frame: {error}"
            ))),
        }
    }
}

#[wasm_bindgen]
pub fn mclone_terrain_lab_create_canonical(
    canvas: HtmlCanvasElement,
    authored_bytes: js_sys::Uint8Array,
    fallback_bytes: js_sys::Uint8Array,
) -> js_sys::Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        CanonicalTerrainLab::new(canvas, authored_bytes, fallback_bytes)
            .await
            .map(JsValue::from)
            .map_err(JsValue::from)
    })
}

fn canonical_camera(
    center_x: i32,
    center_z: i32,
    blocks_across: u32,
    view: &str,
    camera: TerrainPreviewCamera,
    width: u32,
    height: u32,
) -> Result<ChunkCamera, JsValue> {
    let center_x = center_x as f32;
    let center_z = center_z as f32;
    let target_y = 54.0;
    let aspect = width.max(1) as f32 / height.max(1) as f32;
    let vertical_blocks = blocks_across.max(16) as f32 / aspect.max(0.2);
    let fov = 58.0_f32.to_radians();
    let (eye, up) = match view.trim().to_ascii_lowercase().as_str() {
        "map" | "2d" => {
            let distance = vertical_blocks * 0.5 / (fov * 0.5).tan() + 96.0;
            ([center_x, target_y + distance, center_z], [0.0, 0.0, -1.0])
        }
        "3d" | "terrain" => {
            let distance = blocks_across.max(96) as f32 * 0.9 + 64.0;
            let horizontal = camera.pitch_radians.cos() * distance;
            (
                [
                    center_x + camera.yaw_radians.cos() * horizontal,
                    target_y + camera.pitch_radians.sin() * distance,
                    center_z - camera.yaw_radians.sin() * horizontal,
                ],
                [0.0, 1.0, 0.0],
            )
        }
        other => {
            return Err(js_error(format!(
                "unsupported canonical terrain view {other:?}; expected map or 3d"
            )));
        }
    };
    let distance = blocks_across.max(128) as f32 * 6.0 + 512.0;
    Ok(ChunkCamera {
        eye,
        target: [center_x, target_y, center_z],
        up,
        fov_y_radians: fov,
        z_near: 0.25,
        z_far: distance,
    })
}

fn canonical_remesh_targets(
    chunks: &BTreeMap<(i32, i32), ResidentCanonicalChunk>,
    chunk_x: i32,
    chunk_z: i32,
) -> BTreeSet<(i32, i32)> {
    [
        (chunk_x, chunk_z),
        (chunk_x - 1, chunk_z),
        (chunk_x + 1, chunk_z),
        (chunk_x, chunk_z - 1),
        (chunk_x, chunk_z + 1),
    ]
    .into_iter()
    .filter(|position| chunks.contains_key(position))
    .collect()
}

fn suppress_missing_footprint_walls(
    sections: &mut [TexturedRenderSectionMesh],
    chunks: &BTreeMap<(i32, i32), ResidentCanonicalChunk>,
) {
    let Some(min_chunk_x) = chunks.keys().map(|(chunk_x, _)| *chunk_x).min() else {
        return;
    };
    let Some(max_chunk_x) = chunks.keys().map(|(chunk_x, _)| *chunk_x).max() else {
        return;
    };
    let Some(min_chunk_z) = chunks.keys().map(|(_, chunk_z)| *chunk_z).min() else {
        return;
    };
    let Some(max_chunk_z) = chunks.keys().map(|(_, chunk_z)| *chunk_z).max() else {
        return;
    };
    let bounds = [
        min_chunk_x as f32 * 16.0,
        (max_chunk_x + 1) as f32 * 16.0,
        min_chunk_z as f32 * 16.0,
        (max_chunk_z + 1) as f32 * 16.0,
    ];
    for section in sections {
        suppress_mesh_boundary_quads(&mut section.mesh, bounds);
    }
}

fn suppress_mesh_boundary_quads(mesh: &mut TexturedVisibleChunkMesh, bounds: [f32; 4]) {
    let old_indices = std::mem::take(&mut mesh.indices);
    let solid_end = mesh.solid_index_count.min(old_indices.len() as u32) as usize;
    let opaque_end = mesh.opaque_index_count.min(old_indices.len() as u32) as usize;
    append_non_boundary_quads(mesh, &old_indices[..solid_end], bounds);
    mesh.solid_index_count = mesh.indices.len() as u32;
    append_non_boundary_quads(mesh, &old_indices[solid_end..opaque_end], bounds);
    mesh.opaque_index_count = mesh.indices.len() as u32;
    append_non_boundary_quads(mesh, &old_indices[opaque_end..], bounds);
}

fn append_non_boundary_quads(
    mesh: &mut TexturedVisibleChunkMesh,
    indices: &[u32],
    [min_x, max_x, min_z, max_z]: [f32; 4],
) {
    for quad in indices.chunks_exact(6) {
        let positions = quad.iter().filter_map(|index| {
            mesh.vertices
                .get(*index as usize)
                .map(|vertex| vertex.position)
        });
        let mut count = 0;
        let mut on_min_x = true;
        let mut on_max_x = true;
        let mut on_min_z = true;
        let mut on_max_z = true;
        for position in positions {
            count += 1;
            on_min_x &= position[0] == min_x;
            on_max_x &= position[0] == max_x;
            on_min_z &= position[2] == min_z;
            on_max_z &= position[2] == max_z;
        }
        if count == 6 && (on_min_x || on_max_x || on_min_z || on_max_z) {
            continue;
        }
        mesh.indices.extend_from_slice(quad);
    }
}

fn parse_seed(seed: &str) -> Result<i64, JsValue> {
    seed.trim()
        .parse::<i64>()
        .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))
}

fn now_ms() -> Result<f64, JsValue> {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .ok_or_else(|| js_error("Terrain Lab requires window.performance"))
}

fn json(value: &impl Serialize) -> Result<String, JsValue> {
    serde_json::to_string(value)
        .map_err(|error| js_error(format!("failed to encode report: {error}")))
}

fn js_error(message: impl AsRef<str>) -> JsValue {
    js_sys::Error::new(message.as_ref()).into()
}
