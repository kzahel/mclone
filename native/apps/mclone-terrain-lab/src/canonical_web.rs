use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mclone_assets::{TexturePresentation, TextureVisualProfile};
use mclone_core::ChunkPos;
use mclone_mesh::{RenderSectionKey, unpack_textured_render_sections};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, ChunkRenderView, ChunkTextureAtlas,
    ChunkTextureSampling, TexturedSectionDrawResources, TexturedSectionRenderOptions,
};
use mclone_terrain_view::{
    TerrainPreviewCamera, TerrainPreviewProjectionKind, TerrainPreviewView,
    terrain_preview_focus_y_for_profile, terrain_preview_projection,
};
use mclone_worldgen::terrain_preview::TerrainPreviewProfile;
use serde::Serialize;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
use web_sys::HtmlCanvasElement;

use crate::canonical_coordinator_web::{
    CanonicalCoverageRequest, CanonicalTerrainWorkerCoordinator,
};
use crate::terrain_preview_projection_kind;
use crate::visual_assets::load_terrain_lab_visual_assets;

use crate::web::surface_configuration;

const CANONICAL_WARM_MESH_MAX_CHUNKS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanonicalChunkCoordinate {
    pub(crate) chunk_x: i32,
    pub(crate) chunk_z: i32,
}

pub(crate) struct CanonicalPackedPrepareReport {
    pub(crate) warm_chunks: usize,
    pub(crate) warm_available: Vec<CanonicalChunkCoordinate>,
    pub(crate) vertex_count: u32,
    pub(crate) index_count: u32,
    pub(crate) resident_mesh_used_bytes: u64,
}

pub(crate) struct CanonicalPackedAcceptReport {
    pub(crate) warm_chunks: usize,
    pub(crate) vertex_count: u32,
    pub(crate) index_count: u32,
    pub(crate) decode_ms: f64,
    pub(crate) mesh_upload_ms: f64,
    pub(crate) resident_mesh_used_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalRenderReport {
    resident_chunks: usize,
    vertex_count: u32,
    index_count: u32,
    resident_raw_bytes: u64,
    resident_mesh_used_bytes: u64,
    width: u32,
    height: u32,
    center_x: i32,
    center_z: i32,
    blocks_across: u32,
    view: String,
    water_visible: bool,
    vegetation_visible: bool,
    visual_profile: &'static str,
    texture_presentation: &'static str,
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
    profile: TerrainPreviewProfile,
    seed: i64,
    visual_profile: TextureVisualProfile,
    texture_presentation: TexturePresentation,
    packed_chunk_sections: BTreeMap<(i32, i32), BTreeSet<RenderSectionKey>>,
    active_packed_chunks: BTreeSet<(i32, i32)>,
    warm_packed_chunks: VecDeque<(i32, i32)>,
    water_visible: bool,
    vegetation_visible: bool,
    draw: TexturedSectionDrawResources,
    depth: ChunkDepthTarget,
    vertex_count: u32,
    index_count: u32,
    exact_coordinator: Option<CanonicalTerrainWorkerCoordinator>,
}

#[wasm_bindgen]
impl CanonicalTerrainLab {
    #[wasm_bindgen]
    pub fn resize(&mut self, width: u32, height: u32) {
        self.resize_surface(width, height);
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = beginExactCoverage)]
    pub fn begin_exact_coverage(
        &mut self,
        worker_transport: JsValue,
        authored_bytes: js_sys::Uint8Array,
        reference_bytes: js_sys::Uint8Array,
        provisional_bytes: js_sys::Uint8Array,
        diagnostic_bytes: js_sys::Uint8Array,
        center_x: i32,
        center_z: i32,
        radius: u32,
        profile: String,
        visual_profile: String,
        texture_presentation: String,
        seed: String,
        stage: String,
        water_visible: bool,
        vegetation_visible: bool,
        cache_enabled: bool,
        cache_epoch: u32,
    ) -> Result<String, JsValue> {
        let mut coordinator = match self.exact_coordinator.take() {
            Some(coordinator) => coordinator,
            None => CanonicalTerrainWorkerCoordinator::new(worker_transport).map_err(js_error)?,
        };
        let report = coordinator
            .begin(
                self,
                CanonicalCoverageRequest {
                    center_x,
                    center_z,
                    radius,
                    profile,
                    visual_profile,
                    texture_presentation,
                    seed,
                    stage,
                    water_visible,
                    vegetation_visible,
                    cache_enabled,
                    cache_epoch,
                    authored_bytes,
                    reference_bytes,
                    provisional_bytes,
                    diagnostic_bytes,
                },
            )
            .map_err(js_error);
        self.exact_coordinator = Some(coordinator);
        json(&report?)
    }

    #[wasm_bindgen(js_name = pumpExactCoverage)]
    pub fn pump_exact_coverage(&mut self) -> Result<String, JsValue> {
        let mut coordinator = self
            .exact_coordinator
            .take()
            .ok_or_else(|| js_error("canonical exact coordinator is not initialized"))?;
        let report = coordinator.pump(self).map_err(js_error);
        self.exact_coordinator = Some(coordinator);
        json(&report?)
    }

    #[wasm_bindgen(js_name = shutdownExactWorker)]
    pub fn shutdown_exact_worker(&mut self) -> Result<(), JsValue> {
        if let Some(coordinator) = self.exact_coordinator.take() {
            coordinator.terminate().map_err(js_error)?;
        }
        Ok(())
    }
}

impl CanonicalTerrainLab {
    pub(crate) fn reset_profile(&mut self, seed: String, profile: String) -> Result<(), JsValue> {
        self.profile = TerrainPreviewProfile::parse_label(&profile).map_err(js_error)?;
        self.seed = parse_seed(&seed)?;
        self.packed_chunk_sections.clear();
        self.active_packed_chunks.clear();
        self.warm_packed_chunks.clear();
        self.draw
            .update_sections(&self.device, &[])
            .map_err(|error| js_error(format!("failed to clear canonical terrain: {error}")))?;
        self.vertex_count = 0;
        self.index_count = 0;
        Ok(())
    }

    pub(crate) fn set_exact_visibility(&mut self, water_visible: bool, vegetation_visible: bool) {
        self.water_visible = water_visible;
        self.vegetation_visible = vegetation_visible;
    }

    pub(crate) fn prepare_packed_chunks(
        &mut self,
        coordinates: &[CanonicalChunkCoordinate],
    ) -> Result<CanonicalPackedPrepareReport, JsValue> {
        let desired = coordinates
            .iter()
            .map(|coordinate| (coordinate.chunk_x, coordinate.chunk_z))
            .collect::<BTreeSet<_>>();
        let departed = self
            .active_packed_chunks
            .iter()
            .copied()
            .filter(|position| !desired.contains(position))
            .collect::<Vec<_>>();
        for position in departed {
            self.active_packed_chunks.remove(&position);
            if self.packed_chunk_sections.contains_key(&position) {
                self.touch_warm_chunk(position);
            }
        }
        let warm_available = coordinates
            .iter()
            .copied()
            .filter(|coordinate| {
                self.warm_packed_chunks
                    .contains(&(coordinate.chunk_x, coordinate.chunk_z))
            })
            .collect::<Vec<_>>();
        let mut removed = BTreeSet::new();
        let mut eviction_attempts = self.warm_packed_chunks.len();
        while self.warm_packed_chunks.len() > CANONICAL_WARM_MESH_MAX_CHUNKS
            && eviction_attempts > 0
        {
            eviction_attempts -= 1;
            let Some(position) = self.warm_packed_chunks.pop_front() else {
                break;
            };
            if desired.contains(&position) {
                self.warm_packed_chunks.push_back(position);
                continue;
            }
            if let Some(keys) = self.packed_chunk_sections.remove(&position) {
                removed.extend(keys);
            }
        }
        self.draw
            .apply_section_updates(&self.device, &[], &removed)
            .map_err(|error| js_error(format!("failed to evict warm canonical meshes: {error}")))?;
        self.refresh_packed_readiness();
        self.vertex_count = self.draw.vertex_count();
        self.index_count = self.draw.index_count();
        Ok(CanonicalPackedPrepareReport {
            warm_chunks: self.warm_packed_chunks.len(),
            warm_available,
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            resident_mesh_used_bytes: self.draw.resident_mesh_used_bytes(),
        })
    }

    pub(crate) fn activate_packed_chunk(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<CanonicalPackedAcceptReport, JsValue> {
        let position = (chunk_x, chunk_z);
        if !self.packed_chunk_sections.contains_key(&position) {
            return Err(js_error(format!(
                "canonical warm chunk ({chunk_x}, {chunk_z}) is not resident"
            )));
        }
        self.warm_packed_chunks
            .retain(|candidate| *candidate != position);
        self.active_packed_chunks.insert(position);
        self.refresh_packed_readiness();
        Ok(CanonicalPackedAcceptReport {
            warm_chunks: self.warm_packed_chunks.len(),
            vertex_count: self.draw.vertex_count(),
            index_count: self.draw.index_count(),
            decode_ms: 0.0,
            mesh_upload_ms: 0.0,
            resident_mesh_used_bytes: self.draw.resident_mesh_used_bytes(),
        })
    }

    pub(crate) fn accept_packed_mesh_bytes(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        packed_sections: Vec<u8>,
    ) -> Result<CanonicalPackedAcceptReport, JsValue> {
        let decode_started = now_ms()?;
        let sections = unpack_textured_render_sections(&packed_sections)
            .map_err(|error| js_error(format!("invalid canonical packed mesh: {error}")))?;
        let decode_ms = now_ms()? - decode_started;
        let target_chunks = sections
            .iter()
            .map(|section| (section.key.chunk_x, section.key.chunk_z))
            .collect::<BTreeSet<_>>();
        let mut removed = BTreeSet::new();
        for target in &target_chunks {
            let next_keys = sections
                .iter()
                .filter(|section| (section.key.chunk_x, section.key.chunk_z) == *target)
                .map(|section| section.key)
                .collect::<BTreeSet<_>>();
            if let Some(previous) = self
                .packed_chunk_sections
                .insert(*target, next_keys.clone())
            {
                removed.extend(previous.difference(&next_keys).copied());
            }
        }
        let upload_started = now_ms()?;
        self.draw
            .apply_section_updates(&self.device, &sections, &removed)
            .map_err(|error| {
                js_error(format!("failed to upload canonical packed mesh: {error}"))
            })?;
        let mesh_upload_ms = now_ms()? - upload_started;
        let position = (chunk_x, chunk_z);
        self.warm_packed_chunks
            .retain(|candidate| *candidate != position);
        self.active_packed_chunks.insert(position);
        self.refresh_packed_readiness();
        self.vertex_count = self.draw.vertex_count();
        self.index_count = self.draw.index_count();
        Ok(CanonicalPackedAcceptReport {
            warm_chunks: self.warm_packed_chunks.len(),
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            decode_ms,
            mesh_upload_ms,
            resident_mesh_used_bytes: self.draw.resident_mesh_used_bytes(),
        })
    }
}

#[wasm_bindgen]
impl CanonicalTerrainLab {
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
        projection: String,
    ) -> Result<String, JsValue> {
        let projection_kind = terrain_preview_projection_kind(&projection).map_err(js_error)?;
        let camera = TerrainPreviewCamera::new(camera_yaw, camera_pitch, projection_kind)
            .map_err(js_error)?;
        let focus_y =
            terrain_preview_focus_y_for_profile(self.profile, self.seed, center_x, center_z);
        let render_view = canonical_render_view(
            center_x,
            center_z,
            blocks_across,
            &view,
            camera,
            self.width,
            self.height,
            focus_y,
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
                render_view,
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
            resident_chunks: self.active_packed_chunks.len(),
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            resident_raw_bytes: 0,
            resident_mesh_used_bytes: self.draw.resident_mesh_used_bytes(),
            width: self.width,
            height: self.height,
            center_x,
            center_z,
            blocks_across,
            view,
            water_visible: self.water_visible,
            vegetation_visible: self.vegetation_visible,
            visual_profile: self.visual_profile.id(),
            texture_presentation: self.texture_presentation.id(),
            preview_lighting: "fullbright + face shade + ambient occlusion",
        })
    }
}

impl CanonicalTerrainLab {
    async fn new(
        canvas: HtmlCanvasElement,
        authored_bytes: js_sys::Uint8Array,
        reference_bytes: js_sys::Uint8Array,
        provisional_bytes: js_sys::Uint8Array,
        diagnostic_bytes: js_sys::Uint8Array,
        visual_profile: String,
        texture_presentation: String,
    ) -> Result<Self, String> {
        let visual_assets = load_terrain_lab_visual_assets(
            authored_bytes.to_vec(),
            reference_bytes.to_vec(),
            provisional_bytes.to_vec(),
            diagnostic_bytes.to_vec(),
            &visual_profile,
            &texture_presentation,
        )?;
        let assets = visual_assets.terrain;

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

        let draw = TexturedSectionDrawResources::new_with_texture_sampling(
            &device,
            &queue,
            format,
            &[],
            ChunkTextureAtlas {
                width: assets.atlas.width,
                height: assets.atlas.height,
                rgba: assets.atlas.rgba(),
            },
            ChunkTextureSampling::TerrainOverview,
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
            profile: TerrainPreviewProfile::McloneOverworldV1,
            seed: 0,
            visual_profile: visual_assets.profile,
            texture_presentation: visual_assets.presentation,
            packed_chunk_sections: BTreeMap::new(),
            active_packed_chunks: BTreeSet::new(),
            warm_packed_chunks: VecDeque::new(),
            water_visible: true,
            vegetation_visible: true,
            draw,
            depth,
            vertex_count: 0,
            index_count: 0,
            exact_coordinator: None,
        })
    }

    fn touch_warm_chunk(&mut self, position: (i32, i32)) {
        self.warm_packed_chunks
            .retain(|candidate| *candidate != position);
        self.warm_packed_chunks.push_back(position);
    }

    fn refresh_packed_readiness(&mut self) {
        let ready = self
            .active_packed_chunks
            .iter()
            .map(|(chunk_x, chunk_z)| ChunkPos::new(*chunk_x, *chunk_z))
            .collect::<BTreeSet<_>>();
        self.draw
            .set_traversal_ready_columns_with_context(&ready, false);
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
    reference_bytes: js_sys::Uint8Array,
    provisional_bytes: js_sys::Uint8Array,
    diagnostic_bytes: js_sys::Uint8Array,
    visual_profile: String,
    texture_presentation: String,
) -> js_sys::Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        CanonicalTerrainLab::new(
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

fn canonical_render_view(
    center_x: i32,
    center_z: i32,
    blocks_across: u32,
    view: &str,
    camera: TerrainPreviewCamera,
    width: u32,
    height: u32,
    focus_y: f32,
) -> Result<ChunkRenderView, JsValue> {
    // Integer Terrain Lab centers identify block columns. Aim exact geometry
    // at that column's center so a one-block viewport frames one complete top
    // face rather than four quarters around a block boundary.
    let center_x = center_x as f32 + 0.5;
    let center_z = center_z as f32 + 0.5;
    let preview_view = match view.trim().to_ascii_lowercase().as_str() {
        "map" | "2d" => TerrainPreviewView::Map,
        "3d" | "terrain" => TerrainPreviewView::ThreeDimensional,
        other => {
            return Err(js_error(format!(
                "unsupported canonical terrain view {other:?}; expected map or 3d"
            )));
        }
    };
    let projection =
        terrain_preview_projection(blocks_across, preview_view, camera, width, height, focus_y);
    let render_camera = ChunkCamera {
        eye: [
            center_x + projection.eye_offset[0],
            projection.target_y + projection.eye_offset[1],
            center_z + projection.eye_offset[2],
        ],
        target: [center_x, projection.target_y, center_z],
        up: projection.up,
        fov_y_radians: projection.fov_y_radians,
        z_near: projection.z_near,
        z_far: projection.z_far,
    };
    Ok(match projection.kind {
        TerrainPreviewProjectionKind::Orthographic => render_camera.render_orthographic_view(
            width,
            height,
            projection.vertical_half_extent * 2.0,
        ),
        TerrainPreviewProjectionKind::Perspective => render_camera.render_view(width, height),
    })
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
