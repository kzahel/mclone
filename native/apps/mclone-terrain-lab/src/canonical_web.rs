use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mclone_assets::{TexturePresentation, TextureVisualProfile};
use mclone_core::{BlockStateId, ChunkPos};
use mclone_mesh::{
    RENDER_SECTION_HEIGHT, RenderSectionKey, TexturedChunkMeshInput, TexturedMeshCatalog,
    TexturedRenderSectionMesh, TexturedVisibleChunkMesh,
    build_textured_render_sections_for_chunk_set, unpack_textured_render_sections,
};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, ChunkRenderView, ChunkTextureAtlas,
    ChunkTextureSampling, TexturedSectionDrawResources, TexturedSectionRenderOptions,
};
use mclone_terrain_view::{
    CanonicalTerrainVisibility, TerrainPreviewCamera, TerrainPreviewProjectionKind,
    TerrainPreviewView, canonical_terrain_presentation_blocks, terrain_preview_focus_y_for_profile,
    terrain_preview_projection,
};
use mclone_worldgen::terrain_preview::TerrainPreviewProfile;
use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
use web_sys::HtmlCanvasElement;

use crate::canonical_coordinator_web::{
    CanonicalCoverageRequest, CanonicalTerrainWorkerCoordinator,
};
use crate::terrain_preview_projection_kind;
use crate::visual_assets::load_terrain_lab_visual_assets;

use crate::web::surface_configuration;

const CANONICAL_WARM_MESH_MAX_CHUNKS: usize = 64;

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
    resident_raw_bytes: u64,
    resident_mesh_used_bytes: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanonicalChunkCoordinate {
    pub(crate) chunk_x: i32,
    pub(crate) chunk_z: i32,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanonicalPackedPrepareReport {
    pub(crate) active_chunks: usize,
    pub(crate) warm_chunks: usize,
    pub(crate) warm_available: Vec<CanonicalChunkCoordinate>,
    pub(crate) evicted_chunks: usize,
    pub(crate) removed_sections: usize,
    pub(crate) vertex_count: u32,
    pub(crate) index_count: u32,
    pub(crate) resident_mesh_used_bytes: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanonicalPackedAcceptReport {
    pub(crate) chunk_x: i32,
    pub(crate) chunk_z: i32,
    pub(crate) fingerprint: String,
    pub(crate) active_chunks: usize,
    pub(crate) warm_chunks: usize,
    pub(crate) target_chunks: usize,
    pub(crate) section_count: usize,
    pub(crate) vertex_count: u32,
    pub(crate) index_count: u32,
    pub(crate) decode_ms: f64,
    pub(crate) mesh_upload_ms: f64,
    pub(crate) resident_mesh_used_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalRetainReport {
    resident_chunks: usize,
    removed_chunks: usize,
    removed_sections: usize,
    vertex_count: u32,
    index_count: u32,
    resident_raw_bytes: u64,
    resident_mesh_used_bytes: u64,
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
    catalog: TexturedMeshCatalog,
    chunks: BTreeMap<(i32, i32), ResidentCanonicalChunk>,
    packed_chunk_sections: BTreeMap<(i32, i32), BTreeSet<RenderSectionKey>>,
    active_packed_chunks: BTreeSet<(i32, i32)>,
    warm_packed_chunks: VecDeque<(i32, i32)>,
    visibility: CanonicalTerrainVisibility,
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

    #[wasm_bindgen(js_name = resetChunks)]
    pub fn reset_chunks(&mut self, seed: String) -> Result<(), JsValue> {
        self.reset_profile(
            seed,
            TerrainPreviewProfile::McloneOverworldV1.label().to_owned(),
        )
    }

    #[wasm_bindgen(js_name = resetProfile)]
    pub fn reset_profile(&mut self, seed: String, profile: String) -> Result<(), JsValue> {
        self.profile = TerrainPreviewProfile::parse_label(&profile).map_err(js_error)?;
        self.seed = parse_seed(&seed)?;
        self.chunks.clear();
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
        let mut coordinator = self
            .exact_coordinator
            .take()
            .unwrap_or_else(|| CanonicalTerrainWorkerCoordinator::new(worker_transport));
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

    #[wasm_bindgen(js_name = preparePackedChunks)]
    pub fn prepare_packed_chunks(&mut self, coordinates_json: String) -> Result<String, JsValue> {
        let coordinates = serde_json::from_str::<Vec<CanonicalChunkCoordinate>>(&coordinates_json)
            .map_err(|error| {
                js_error(format!(
                    "invalid packed canonical desired coordinates: {error}"
                ))
            })?;
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
        let mut evicted_chunks = 0;
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
                evicted_chunks += 1;
            }
        }
        self.draw
            .apply_section_updates(&self.device, &[], &removed)
            .map_err(|error| js_error(format!("failed to evict warm canonical meshes: {error}")))?;
        self.refresh_packed_readiness();
        self.vertex_count = self.draw.vertex_count();
        self.index_count = self.draw.index_count();
        json(&CanonicalPackedPrepareReport {
            active_chunks: self.active_packed_chunks.len(),
            warm_chunks: self.warm_packed_chunks.len(),
            warm_available,
            evicted_chunks,
            removed_sections: removed.len(),
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            resident_mesh_used_bytes: self.draw.resident_mesh_used_bytes(),
        })
    }

    #[wasm_bindgen(js_name = activatePackedChunk)]
    pub fn activate_packed_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> Result<String, JsValue> {
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
        json(&CanonicalPackedAcceptReport {
            chunk_x,
            chunk_z,
            fingerprint: String::new(),
            active_chunks: self.active_packed_chunks.len(),
            warm_chunks: self.warm_packed_chunks.len(),
            target_chunks: 0,
            section_count: 0,
            vertex_count: self.draw.vertex_count(),
            index_count: self.draw.index_count(),
            decode_ms: 0.0,
            mesh_upload_ms: 0.0,
            resident_mesh_used_bytes: self.draw.resident_mesh_used_bytes(),
        })
    }

    #[wasm_bindgen(js_name = acceptPackedMesh)]
    pub fn accept_packed_mesh(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        fingerprint: String,
        packed_sections: js_sys::Uint8Array,
    ) -> Result<String, JsValue> {
        let decode_started = now_ms()?;
        let sections = unpack_textured_render_sections(&packed_sections.to_vec())
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
        json(&CanonicalPackedAcceptReport {
            chunk_x,
            chunk_z,
            fingerprint,
            active_chunks: self.active_packed_chunks.len(),
            warm_chunks: self.warm_packed_chunks.len(),
            target_chunks: target_chunks.len(),
            section_count: sections.len(),
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            decode_ms,
            mesh_upload_ms,
            resident_mesh_used_bytes: self.draw.resident_mesh_used_bytes(),
        })
    }

    #[wasm_bindgen(js_name = retainChunks)]
    pub fn retain_chunks(&mut self, coordinates_json: String) -> Result<String, JsValue> {
        let coordinates = serde_json::from_str::<Vec<CanonicalChunkCoordinate>>(&coordinates_json)
            .map_err(|error| js_error(format!("invalid canonical retain coordinates: {error}")))?;
        let retained = coordinates
            .into_iter()
            .map(|coordinate| (coordinate.chunk_x, coordinate.chunk_z))
            .collect::<BTreeSet<_>>();
        let removed_chunks = self
            .chunks
            .keys()
            .copied()
            .filter(|position| !retained.contains(position))
            .collect::<Vec<_>>();
        let mut removed_sections = BTreeSet::new();
        for position in &removed_chunks {
            let Some(chunk) = self.chunks.remove(position) else {
                continue;
            };
            for local_y_start in (0..chunk.height).step_by(RENDER_SECTION_HEIGHT as usize) {
                removed_sections.insert(RenderSectionKey::new(
                    position.0,
                    (chunk.min_y + local_y_start).div_euclid(RENDER_SECTION_HEIGHT),
                    position.1,
                ));
            }
        }
        self.draw
            .apply_section_updates(&self.device, &[], &removed_sections)
            .map_err(|error| js_error(format!("failed to retain canonical terrain: {error}")))?;
        self.vertex_count = self.draw.vertex_count();
        self.index_count = self.draw.index_count();
        json(&CanonicalRetainReport {
            resident_chunks: self.chunks.len(),
            removed_chunks: removed_chunks.len(),
            removed_sections: removed_sections.len(),
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            resident_raw_bytes: self.resident_raw_bytes(),
            resident_mesh_used_bytes: self.draw.resident_mesh_used_bytes(),
        })
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
            resident_raw_bytes: self.resident_raw_bytes(),
            resident_mesh_used_bytes: self.draw.resident_mesh_used_bytes(),
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
            resident_raw_bytes: self.resident_raw_bytes(),
            resident_mesh_used_bytes: self.draw.resident_mesh_used_bytes(),
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
            resident_chunks: self.chunks.len(),
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            resident_raw_bytes: self.resident_raw_bytes(),
            resident_mesh_used_bytes: self.draw.resident_mesh_used_bytes(),
            width: self.width,
            height: self.height,
            center_x,
            center_z,
            blocks_across,
            view,
            water_visible: self.visibility.water,
            vegetation_visible: self.visibility.vegetation,
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
            catalog: assets.catalog,
            chunks: BTreeMap::new(),
            packed_chunk_sections: BTreeMap::new(),
            active_packed_chunks: BTreeSet::new(),
            warm_packed_chunks: VecDeque::new(),
            visibility: CanonicalTerrainVisibility::default(),
            draw,
            depth,
            vertex_count: 0,
            index_count: 0,
            exact_coordinator: None,
        })
    }

    fn resident_raw_bytes(&self) -> u64 {
        self.chunks.values().fold(0_u64, |bytes, chunk| {
            bytes
                .saturating_add(chunk.blocks.len() as u64)
                .saturating_add(
                    (chunk.biomes.len() as u64).saturating_mul(std::mem::size_of::<i32>() as u64),
                )
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

    fn rebuild_chunks(&mut self, targets: &BTreeSet<(i32, i32)>) -> Result<(u32, u32), JsValue> {
        if targets.is_empty() {
            return Ok((self.vertex_count, self.index_count));
        }
        let input_positions = targets
            .iter()
            .flat_map(|(chunk_x, chunk_z)| {
                [
                    (*chunk_x, *chunk_z),
                    (*chunk_x - 1, *chunk_z),
                    (*chunk_x + 1, *chunk_z),
                    (*chunk_x, *chunk_z - 1),
                    (*chunk_x, *chunk_z + 1),
                ]
            })
            .filter(|position| self.chunks.contains_key(position))
            .collect::<BTreeSet<_>>();
        let presented = input_positions
            .iter()
            .filter_map(|position| {
                let chunk = self.chunks.get(position)?;
                let blocks = canonical_terrain_presentation_blocks(&chunk.blocks, self.visibility)
                    .into_iter()
                    .map(|block| BlockStateId(u32::from(block)))
                    .collect::<Vec<_>>();
                Some((*position, blocks))
            })
            .collect::<Vec<_>>();
        let inputs = presented
            .iter()
            .filter_map(|((chunk_x, chunk_z), blocks)| {
                let chunk = self.chunks.get(&(*chunk_x, *chunk_z))?;
                Some(
                    TexturedChunkMeshInput::new(
                        *chunk_x,
                        *chunk_z,
                        chunk.min_y,
                        chunk.height,
                        blocks,
                    )
                    .with_biomes(&chunk.biomes)
                    .with_world_seed(self.seed)
                    .with_fluids_visible(self.visibility.water),
                )
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
