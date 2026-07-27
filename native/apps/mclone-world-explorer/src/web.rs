use std::time::Duration;

use mclone_assets::{
    AUTHORED_FIRST_PARTY_PACK_ID, AssetPackId, AssetPackOrigin, AssetSourceChain,
    DIAGNOSTIC_MISSING_PACK_ID, PROVISIONAL_FIRST_PARTY_PACK_ID, PackedAssetSource,
    TexturePresentation,
};
use mclone_core::BlockStateId;
use mclone_mesh::load_first_party_textured_terrain_assets_with_presentation;
use mclone_render::chunk::ChunkTextureAtlas;
use mclone_render_color::{RenderColorProfile, RenderTargetColorTransform};
use mclone_terrain_view::{
    TERRAIN_PREVIEW_MATERIAL_UV_COUNT, TerrainClipmapConfig, TerrainExactCoverageMode,
    TerrainHorizonFrameStats, TerrainHorizonRenderTarget, TerrainPreviewMaterialAtlas,
    TerrainVegetationCoordinatorState, TerrainVegetationExecutorKind,
};
use mclone_view_control::{
    ContactButton, ContactEvent, ViewPoint, ViewportMetrics, WorldViewHeldDirection,
    WorldViewIntent, WorldViewMode, WorldViewProjection, WorldViewState, pointer_contact_purpose,
};
use serde::Serialize;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
use web_sys::{HtmlCanvasElement, UrlSearchParams};

use crate::{
    ExplorerExactStats, ExplorerExactTerrain, WorldExplorerCompositionMode, WorldExplorerConfig,
    WorldExplorerSession, web_exact::WebCanonicalExactExecutor,
    web_vegetation::WebTerrainVegetationExecutor,
};

const DEFAULT_SEED: i64 = 12_345;
const DEFAULT_BLOCKS_ACROSS: u32 = 4_096;
const DEFAULT_EXACT_RADIUS: u32 = 2;

#[derive(Clone, Copy, Debug)]
struct WebExplorerOptions {
    seed: i64,
    center_x: i32,
    center_z: i32,
    blocks_across: u32,
    mode: WorldViewMode,
    projection: WorldViewProjection,
    yaw_radians: f64,
    pitch_radians: f64,
    composition: WorldExplorerCompositionMode,
    source_colors: bool,
    exact_radius: u32,
    diagnostic_observer_enabled: bool,
    worker_overflow_probe_enabled: bool,
}

impl Default for WebExplorerOptions {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            center_x: 0,
            center_z: 0,
            blocks_across: DEFAULT_BLOCKS_ACROSS,
            mode: WorldViewMode::Orbit,
            projection: WorldViewProjection::Perspective,
            yaw_radians: std::f64::consts::FRAC_PI_4,
            pitch_radians: 0.52,
            composition: WorldExplorerCompositionMode::Horizon,
            source_colors: false,
            exact_radius: DEFAULT_EXACT_RADIUS,
            diagnostic_observer_enabled: false,
            worker_overflow_probe_enabled: false,
        }
    }
}

impl WebExplorerOptions {
    fn parse(search: &str) -> Result<Self, String> {
        let parameters = UrlSearchParams::new_with_str(search)
            .map_err(|error| format!("invalid World Explorer query: {error:?}"))?;
        let mut options = Self::default();
        options.seed = parse_parameter(&parameters, "seed", options.seed)?;
        options.center_x = parse_parameter(&parameters, "centerX", options.center_x)?;
        options.center_z = parse_parameter(&parameters, "centerZ", options.center_z)?;
        options.blocks_across =
            parse_parameter(&parameters, "blocksAcross", options.blocks_across)?;
        options.yaw_radians = parse_parameter(&parameters, "yaw", options.yaw_radians)?;
        options.pitch_radians = parse_parameter(&parameters, "pitch", options.pitch_radians)?;
        options.exact_radius = parse_parameter(&parameters, "exactRadius", options.exact_radius)?;
        if options.exact_radius > 8 {
            return Err("World Explorer exactRadius must be at most 8 chunks".to_owned());
        }
        if let Some(value) = parameters.get("composition") {
            options.composition = WorldExplorerCompositionMode::parse_label(&value)?;
        }
        options.source_colors = parameters.get("sourceColors").as_deref() == Some("1");
        if options.source_colors && options.composition == WorldExplorerCompositionMode::Horizon {
            return Err(
                "World Explorer sourceColors=1 requires exact, composed, or coverage composition"
                    .to_owned(),
            );
        }
        options.diagnostic_observer_enabled =
            parameters.get("smokeObserver").as_deref() == Some("1");
        options.worker_overflow_probe_enabled =
            parameters.get("workerOverflowProbe").as_deref() == Some("1");
        if options.worker_overflow_probe_enabled && !options.diagnostic_observer_enabled {
            return Err("World Explorer worker overflow probe requires smokeObserver=1".to_owned());
        }
        if let Some(value) = parameters.get("view") {
            options.mode = match value.as_str() {
                "map" | "2d" => WorldViewMode::Map,
                "3d" | "orbit" => WorldViewMode::Orbit,
                _ => return Err(format!("unsupported World Explorer view {value:?}")),
            };
        }
        if let Some(value) = parameters.get("projection") {
            options.projection = match value.as_str() {
                "orthographic" | "ortho" => WorldViewProjection::Orthographic,
                "perspective" => WorldViewProjection::Perspective,
                _ => {
                    return Err(format!("unsupported World Explorer projection {value:?}"));
                }
            };
        }
        Ok(options)
    }

    fn view_state(self) -> WorldViewState {
        WorldViewState {
            mode: self.mode,
            focus_x: f64::from(self.center_x),
            focus_z: f64::from(self.center_z),
            blocks_across: f64::from(self.blocks_across),
            yaw_radians: self.yaw_radians,
            pitch_radians: self.pitch_radians,
            projection: self.projection,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebExplorerReport {
    revision: u64,
    seed: String,
    center_x: i32,
    center_z: i32,
    focus_x: f64,
    focus_z: f64,
    blocks_across: u32,
    blocks_across_exact: f64,
    view: &'static str,
    projection: &'static str,
    color_profile: &'static str,
    color_format: String,
    color_transform: &'static str,
    yaw_radians: f64,
    pitch_radians: f64,
    composition: &'static str,
    source_colors: bool,
    exact_radius: u32,
    exact_anchor_x: i32,
    exact_anchor_z: i32,
    exact_desired_chunks: u32,
    exact_painted_chunks: u32,
    exact_queued_chunks: u32,
    exact_pending_admissions: u32,
    exact_in_flight: bool,
    exact_coverage_generation: u64,
    exact_admitted_chunks_total: u64,
    exact_stale_chunks_total: u64,
    exact_resident_mesh_bytes: u64,
    exact_vertex_count: u32,
    exact_index_count: u32,
    exact_drawn_sections: u32,
    exact_drawn_indices: u32,
    exact_natural_tree_records: u32,
    canonical_exact_owned_tree_records: u32,
    canonical_proxy_owned_tree_records: u32,
    exact_tree_sections: u32,
    exact_tree_indices: u32,
    exact_complete: bool,
    exact_coverage_mode: &'static str,
    procedural_coverage_generation: u64,
    procedural_painted_chunks: u32,
    exact_coverage_mask_bytes: u64,
    allocation_slots: u32,
    staging_slots: u32,
    normal_halo_radius: u32,
    normal_halo_samples_per_tile: u32,
    normal_halo_fixed_bytes: u64,
    normal_height_fixed_bytes: u64,
    ready_slots: u32,
    requested_levels: u32,
    staged_levels: u32,
    committed_levels: u32,
    vegetation_committed_levels: u32,
    atomic_level_commits: u64,
    deferred_transition_attempts: u64,
    pending_refills: u32,
    dispatched_refills: u32,
    dispatched_refills_total: u64,
    total_refills: u64,
    total_rebases: u64,
    retained_tiles: u32,
    drawn_levels: u32,
    drawn_tiles: u32,
    vertex_count: u32,
    vegetation_ready_tiles: u32,
    pending_vegetation_tiles: u32,
    tree_instance_count: u32,
    tree_proxy_suppressed_instances: u32,
    tree_proxy_suppressed_records: u32,
    tree_proxy_missing_exact_records: u32,
    tree_proxy_missing_proxy_records: u32,
    tree_proxy_vertex_count: u32,
    tree_ownership_generation: u64,
    tree_ownership_units: u32,
    exact_owned_tree_records: u32,
    proxy_owned_tree_records: u32,
    fixed_resident_bytes: u64,
    vegetation_bytes: u64,
    resident_bytes: u64,
    vegetation_coordinator_state: &'static str,
    vegetation_executor_kind: &'static str,
    vegetation_source_fingerprint: String,
    vegetation_terrain_source_revision: String,
    vegetation_compiler_source_revision: String,
    vegetation_plan_revision: String,
    vegetation_product_revision: u32,
    mchv_wire_version: u16,
    vegetation_record_hash: String,
    vegetation_family_counts: [u32; 3],
    vegetation_record_count: u32,
    vegetation_product_count: u32,
    vegetation_executor_generation: u32,
    vegetation_source_epoch: u32,
    vegetation_coverage_revision: u64,
    vegetation_desired_tiles: u32,
    vegetation_queued_tiles: u32,
    vegetation_resident_tiles: u32,
    vegetation_in_flight: bool,
    vegetation_submitted_jobs: u64,
    vegetation_completed_jobs: u64,
    vegetation_admitted_products: u64,
    vegetation_source_resets: u64,
    vegetation_transport_failures: u64,
    vegetation_executor_restarts: u64,
    vegetation_job_failures: u64,
    vegetation_stale_completions: u64,
    vegetation_superseded_completions: u64,
    vegetation_submit_full_count: u64,
    vegetation_compile_micros: u64,
    vegetation_cache_cell_requests: u64,
    vegetation_cache_cell_hits: u64,
    vegetation_cache_cell_misses: u64,
    vegetation_cache_retained_cells: u64,
    vegetation_cache_retained_preliminary_candidates: u64,
    worker_submitted_jobs: u64,
    worker_completed_jobs: u64,
    worker_transport_failures: u64,
    worker_restart_count: u64,
    worker_result_capacity_bytes: u64,
    worker_result_high_water_bytes: u64,
    worker_result_overflow_count: u64,
    worker_copied_result_bytes: u64,
    worker_main_decode_micros: u64,
    coarse_ready: bool,
    target_ready: bool,
    needs_redraw: bool,
    held_motion: bool,
}

#[wasm_bindgen(js_name = WebWorldExplorer)]
pub struct WebWorldExplorer {
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
    frame_epoch_ms: Option<f64>,
    session: WorldExplorerSession,
    composition: WorldExplorerCompositionMode,
    source_colors: bool,
    exact_radius: u32,
    exact: Option<ExplorerExactTerrain>,
    last_exact_stats: ExplorerExactStats,
    last_exact_anchor: [i32; 2],
    diagnostic_observer_enabled: bool,
    last_diagnostic_report: Option<WebExplorerReport>,
}

#[wasm_bindgen(js_class = WebWorldExplorer)]
impl WebWorldExplorer {
    pub fn resize(&mut self, width: u32, height: u32) {
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
        self.session.resize(&self.device, width, height);
        if let Some(exact) = &mut self.exact {
            exact.resize(&self.device, width, height);
        }
    }

    #[wasm_bindgen(js_name = renderFrame)]
    pub fn render_frame(&mut self, frame_millis: f64) -> Result<(), JsValue> {
        let frame_millis = if frame_millis.is_finite() {
            frame_millis.max(0.0)
        } else {
            self.frame_epoch_ms.unwrap_or(0.0)
        };
        let frame_epoch_ms = *self.frame_epoch_ms.get_or_insert(frame_millis);
        let elapsed = Duration::from_secs_f64(
            ((frame_millis - frame_epoch_ms).max(0.0) / 1_000.0).min(f64::from(u32::MAX)),
        );
        let frame = self
            .surface
            .get_current_texture()
            .map_err(|error| js_error(format!("World Explorer surface frame failed: {error}")))?;
        let color_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_world_explorer_web_frame"),
            });
        let mut stats = if self.composition == WorldExplorerCompositionMode::Horizon {
            self.session.restore_horizon_view();
            self.session
                .encode(
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    &color_view,
                    elapsed,
                )
                .map_err(js_error)?
        } else {
            self.encode_composed(&mut encoder, &color_view, elapsed)
                .map_err(js_error)?
        };
        if self.composition != WorldExplorerCompositionMode::Horizon {
            self.last_exact_stats = self
                .exact
                .as_ref()
                .expect("non-horizon browser composition owns exact terrain")
                .stats();
            stats.target_ready &= self.last_exact_stats.complete;
            stats.needs_redraw |= !self.last_exact_stats.complete;
        }
        if self.diagnostic_observer_enabled {
            self.last_diagnostic_report = Some(explorer_report(
                self.seed,
                self.session.view_state(),
                self.session.has_held_motion(),
                stats,
                self.session.color_profile(),
                self.session.color_format(),
                self.session.target_color_transform(),
                self.composition,
                self.source_colors,
                self.exact_radius,
                self.last_exact_anchor,
                self.last_exact_stats,
            ));
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }

    /// Rich semantic state for the explicit smoke observer.
    ///
    /// Ordinary browser cadence does not consume or return this projection.
    #[wasm_bindgen(js_name = diagnosticSnapshot)]
    pub fn diagnostic_snapshot(&self) -> Result<String, JsValue> {
        if !self.diagnostic_observer_enabled {
            return Err(js_error(
                "World Explorer diagnostic observer was not requested",
            ));
        }
        let report = self.last_diagnostic_report.as_ref().ok_or_else(|| {
            js_error("World Explorer diagnostic snapshot requested before the first frame")
        })?;
        report_json(report)
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = pointerDown)]
    pub fn pointer_down(
        &mut self,
        pointer_id: u32,
        button: i16,
        shift_key: bool,
        x: f64,
        y: f64,
        time_seconds: f64,
        width: f64,
        height: f64,
    ) -> bool {
        let Some(button) = web_contact_button(button) else {
            return false;
        };
        self.session.contact(ContactEvent::Down {
            id: u64::from(pointer_id),
            position: ViewPoint::new(x, y),
            purpose: pointer_contact_purpose(button, shift_key),
            time_seconds,
            viewport: ViewportMetrics::new(width, height),
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = pointerMove)]
    pub fn pointer_move(
        &mut self,
        pointer_id: u32,
        x: f64,
        y: f64,
        time_seconds: f64,
        width: f64,
        height: f64,
    ) -> bool {
        self.session.contact(ContactEvent::Moved {
            id: u64::from(pointer_id),
            position: ViewPoint::new(x, y),
            time_seconds,
            viewport: ViewportMetrics::new(width, height),
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = pointerUp)]
    pub fn pointer_up(
        &mut self,
        pointer_id: u32,
        x: f64,
        y: f64,
        time_seconds: f64,
        width: f64,
        height: f64,
    ) -> bool {
        self.session.contact(ContactEvent::Up {
            id: u64::from(pointer_id),
            position: ViewPoint::new(x, y),
            time_seconds,
            viewport: ViewportMetrics::new(width, height),
        })
    }

    #[wasm_bindgen(js_name = cancelInput)]
    pub fn cancel_input(&mut self) -> bool {
        self.session.cancel_input()
    }

    pub fn shutdown(&mut self) {
        self.session.shutdown();
        self.exact = None;
    }

    #[wasm_bindgen(js_name = shutdownComplete)]
    pub fn shutdown_complete(&self) -> bool {
        self.session.shutdown_complete()
    }

    pub fn wheel(
        &mut self,
        delta_y: f64,
        delta_mode: u32,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> bool {
        let line_scale = match delta_mode {
            1 => 16.0,
            2 => height.max(1.0),
            _ => 1.0,
        };
        let state = self.session.view_state();
        let viewport = ViewportMetrics::new(width, height);
        self.session.apply_intent(WorldViewIntent::AnchoredZoom {
            log_delta: delta_y * line_scale * 0.0015,
            normalized_anchor: if state.mode == WorldViewMode::Map {
                viewport.normalized_anchor(ViewPoint::new(x, y))
            } else {
                ViewPoint::default()
            },
            viewport,
        })
    }

    #[wasm_bindgen(js_name = rawKey)]
    pub fn raw_key(&mut self, code: String, pressed: bool, repeat: bool) -> bool {
        if let Some(direction) = web_held_direction(&code) {
            self.session.set_held_motion(direction, pressed);
            return true;
        }
        let recognized = matches!(code.as_str(), "KeyM" | "KeyP");
        if !pressed || repeat {
            return recognized;
        }
        let intent = match code.as_str() {
            "KeyM" => Some(WorldViewIntent::SetMode(WorldViewMode::Map)),
            "KeyP" => Some(WorldViewIntent::SetMode(WorldViewMode::Orbit)),
            _ => None,
        };
        if let Some(intent) = intent {
            self.session.apply_intent(intent);
        }
        recognized
    }

    #[wasm_bindgen(js_name = recenterForSmoke)]
    pub fn recenter_for_smoke(&mut self, world_x: f64, world_z: f64) -> Result<bool, JsValue> {
        if !self.diagnostic_observer_enabled {
            return Err(js_error("World Explorer smoke commands were not requested"));
        }
        Ok(self
            .session
            .apply_intent(WorldViewIntent::FocusAt { world_x, world_z }))
    }
}

impl WebWorldExplorer {
    fn encode_composed(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        elapsed: Duration,
    ) -> Result<TerrainHorizonFrameStats, String> {
        let exact_view = self.session.exact_composition_view(self.exact_radius)?;
        self.session.apply_exact_composition_view(exact_view);
        self.last_exact_anchor = exact_view.residency_anchor;
        let exact = self
            .exact
            .as_mut()
            .ok_or("non-horizon browser composition has no exact renderer")?;
        exact
            .update_and_pump(
                &self.device,
                exact_view.residency_anchor[0],
                exact_view.residency_anchor[1],
            )
            .map_err(|error| error.to_string())?;
        let coverage = exact.coverage_snapshot()?;
        let coverage_mode = match self.composition {
            WorldExplorerCompositionMode::Composed => {
                Some((&coverage, TerrainExactCoverageMode::DiscardPainted))
            }
            WorldExplorerCompositionMode::Coverage => {
                Some((&coverage, TerrainExactCoverageMode::VisualizePainted))
            }
            WorldExplorerCompositionMode::Horizon | WorldExplorerCompositionMode::Exact => None,
        };
        let tree_ownership = matches!(
            self.composition,
            WorldExplorerCompositionMode::Composed | WorldExplorerCompositionMode::Coverage
        )
        .then_some(exact.tree_ownership());
        let mut stats = self.session.encode_to_target(
            &self.device,
            &self.queue,
            encoder,
            TerrainHorizonRenderTarget {
                color_view,
                depth_view: &exact.depth().view,
                color_load: wgpu::LoadOp::Clear(exact.clear_color()),
                color_store: wgpu::StoreOp::Store,
                depth_load: wgpu::LoadOp::Clear(0.0),
                depth_store: wgpu::StoreOp::Store,
            },
            elapsed,
            coverage_mode,
            tree_ownership,
        )?;
        if matches!(
            self.composition,
            WorldExplorerCompositionMode::Exact | WorldExplorerCompositionMode::Composed
        ) {
            exact
                .render(
                    &self.queue,
                    encoder,
                    color_view,
                    exact_view.render_view,
                    [self.width, self.height],
                    self.composition == WorldExplorerCompositionMode::Composed,
                )
                .map_err(|error| error.to_string())?;
        }
        let exact_stats = exact.stats();
        stats.target_ready &= exact_stats.complete;
        stats.needs_redraw |= !exact_stats.complete;
        Ok(stats)
    }

    async fn new(
        canvas: HtmlCanvasElement,
        authored_bytes: js_sys::Uint8Array,
        provisional_bytes: js_sys::Uint8Array,
        diagnostic_bytes: js_sys::Uint8Array,
        search: String,
        vegetation_worker_transport_factory: JsValue,
        exact_worker_transport_factory: JsValue,
    ) -> Result<Self, String> {
        let options = WebExplorerOptions::parse(&search)?;
        let vegetation_executor = if options.worker_overflow_probe_enabled {
            WebTerrainVegetationExecutor::with_initial_capacity(
                vegetation_worker_transport_factory,
                1_024,
            )?
        } else {
            WebTerrainVegetationExecutor::new(vegetation_worker_transport_factory)?
        };
        let authored_asset_bytes = authored_bytes.to_vec();
        let provisional_asset_bytes = provisional_bytes.to_vec();
        let diagnostic_asset_bytes = diagnostic_bytes.to_vec();
        let assets = load_web_assets(
            authored_asset_bytes.clone(),
            provisional_asset_bytes.clone(),
            diagnostic_asset_bytes.clone(),
        )?;
        let mut material_uvs = [[0.0_f32, 0.0, 1.0, 1.0]; TERRAIN_PREVIEW_MATERIAL_UV_COUNT];
        for (raw_id, target) in material_uvs.iter_mut().enumerate() {
            if let Some(sprite) = assets.catalog.gui_icon_uv(BlockStateId(raw_id as u32)) {
                *target = [sprite.u0, sprite.v0, sprite.u1, sprite.v1];
            }
        }

        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|error| format!("failed to create World Explorer WebGPU surface: {error}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| format!("failed to request World Explorer adapter: {error}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("mclone_world_explorer_web_device"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .map_err(|error| format!("failed to request World Explorer device: {error}"))?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or("World Explorer WebGPU surface reported no color formats")?;
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::Fifo)
            .or_else(|| capabilities.present_modes.first().copied())
            .unwrap_or(wgpu::PresentMode::Fifo);
        let alpha_mode = capabilities
            .alpha_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
            .or_else(|| capabilities.alpha_modes.first().copied())
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        surface.configure(
            &device,
            &surface_configuration(format, width, height, present_mode, alpha_mode),
        );

        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let session = WorldExplorerSession::new(
            &device,
            &queue,
            format,
            WorldExplorerConfig {
                width,
                height,
                seed: options.seed,
                initial_view: options.view_state(),
                clipmap: TerrainClipmapConfig::default(),
                vegetation_enabled: true,
                color_profile: RenderColorProfile::Vanilla,
            },
            TerrainPreviewMaterialAtlas {
                width: assets.atlas.width,
                height: assets.atlas.height,
                rgba: assets.atlas.rgba(),
                material_uvs: &material_uvs,
            },
            Some(Box::new(vegetation_executor)),
        )?;
        let exact = if options.composition == WorldExplorerCompositionMode::Horizon {
            None
        } else {
            let executor = WebCanonicalExactExecutor::new(
                exact_worker_transport_factory,
                options.seed,
                authored_asset_bytes,
                provisional_asset_bytes,
                diagnostic_asset_bytes,
            )?;
            Some(
                ExplorerExactTerrain::new_with_executor(
                    &device,
                    &queue,
                    format,
                    width,
                    height,
                    options.seed,
                    options.exact_radius,
                    Box::new(executor),
                    ChunkTextureAtlas {
                        width: assets.atlas.width,
                        height: assets.atlas.height,
                        rgba: assets.atlas.rgba(),
                    },
                    options.source_colors,
                    RenderColorProfile::Vanilla.target_color_transform(format),
                )
                .map_err(|error| error.to_string())?,
            )
        };
        if let Some(error) = device.pop_error_scope().await {
            return Err(format!(
                "failed to initialize World Explorer GPU pipelines: {error}"
            ));
        }

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
            seed: options.seed,
            frame_epoch_ms: None,
            session,
            composition: options.composition,
            source_colors: options.source_colors,
            exact_radius: options.exact_radius,
            exact,
            last_exact_stats: ExplorerExactStats::default(),
            last_exact_anchor: [options.center_x, options.center_z],
            diagnostic_observer_enabled: options.diagnostic_observer_enabled,
            last_diagnostic_report: None,
        })
    }
}

#[wasm_bindgen]
pub fn mclone_world_explorer_create(
    canvas: HtmlCanvasElement,
    authored_bytes: js_sys::Uint8Array,
    provisional_bytes: js_sys::Uint8Array,
    diagnostic_bytes: js_sys::Uint8Array,
    search: String,
    vegetation_worker_transport_factory: JsValue,
    exact_worker_transport_factory: JsValue,
) -> js_sys::Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        WebWorldExplorer::new(
            canvas,
            authored_bytes,
            provisional_bytes,
            diagnostic_bytes,
            search,
            vegetation_worker_transport_factory,
            exact_worker_transport_factory,
        )
        .await
        .map(JsValue::from)
        .map_err(JsValue::from)
    })
}

#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&info.to_string()));
    }));
}

pub(crate) fn load_web_assets(
    authored_bytes: Vec<u8>,
    provisional_bytes: Vec<u8>,
    diagnostic_bytes: Vec<u8>,
) -> Result<mclone_mesh::TexturedTerrainAssets, String> {
    let mut source = AssetSourceChain::new();
    source.push_named(
        AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID),
        AssetPackOrigin::FirstParty,
        PackedAssetSource::from_bytes(authored_bytes)
            .map_err(|error| format!("failed to parse authored asset pack: {error}"))?,
    );
    source.push_named(
        AssetPackId::new(PROVISIONAL_FIRST_PARTY_PACK_ID),
        AssetPackOrigin::FirstPartyProvisional,
        PackedAssetSource::from_bytes(provisional_bytes)
            .map_err(|error| format!("failed to parse provisional asset pack: {error}"))?,
    );
    if !diagnostic_bytes.is_empty() {
        source.push_named(
            AssetPackId::new(DIAGNOSTIC_MISSING_PACK_ID),
            AssetPackOrigin::Diagnostic,
            PackedAssetSource::from_bytes(diagnostic_bytes)
                .map_err(|error| format!("failed to parse diagnostic asset pack: {error}"))?,
        );
    }
    load_first_party_textured_terrain_assets_with_presentation(
        &source,
        TexturePresentation::Textured,
    )
    .map_err(|error| format!("failed to prepare World Explorer browser materials: {error}"))
}

fn explorer_report(
    seed: i64,
    state: WorldViewState,
    held_motion: bool,
    stats: TerrainHorizonFrameStats,
    color_profile: RenderColorProfile,
    color_format: wgpu::TextureFormat,
    color_transform: RenderTargetColorTransform,
    composition: WorldExplorerCompositionMode,
    source_colors: bool,
    exact_radius: u32,
    exact_anchor: [i32; 2],
    exact: ExplorerExactStats,
) -> WebExplorerReport {
    WebExplorerReport {
        revision: stats.revision,
        seed: seed.to_string(),
        center_x: state.center_x_i32(),
        center_z: state.center_z_i32(),
        focus_x: state.focus_x,
        focus_z: state.focus_z,
        blocks_across: state.blocks_across_u32(),
        blocks_across_exact: state.blocks_across,
        view: match state.mode {
            WorldViewMode::Map => "map",
            WorldViewMode::Orbit => "3d",
        },
        projection: match state.projection {
            WorldViewProjection::Orthographic => "orthographic",
            WorldViewProjection::Perspective => "perspective",
        },
        color_profile: color_profile.as_str(),
        color_format: format!("{color_format:?}"),
        color_transform: color_transform.as_str(),
        yaw_radians: state.yaw_radians,
        pitch_radians: state.pitch_radians,
        composition: composition.label(),
        source_colors,
        exact_radius,
        exact_anchor_x: exact_anchor[0],
        exact_anchor_z: exact_anchor[1],
        exact_desired_chunks: exact.desired_chunks,
        exact_painted_chunks: exact.painted_chunks,
        exact_queued_chunks: exact.queued_chunks,
        exact_pending_admissions: exact.pending_admissions,
        exact_in_flight: exact.in_flight,
        exact_coverage_generation: exact.coverage_generation,
        exact_admitted_chunks_total: exact.admitted_chunks_total,
        exact_stale_chunks_total: exact.stale_chunks_total,
        exact_resident_mesh_bytes: exact.resident_mesh_bytes,
        exact_vertex_count: exact.vertex_count,
        exact_index_count: exact.index_count,
        exact_drawn_sections: exact.drawn_sections,
        exact_drawn_indices: exact.drawn_indices,
        exact_natural_tree_records: exact.natural_tree_records,
        canonical_exact_owned_tree_records: exact.exact_owned_tree_records,
        canonical_proxy_owned_tree_records: exact.proxy_owned_tree_records,
        exact_tree_sections: exact.exact_tree_sections,
        exact_tree_indices: exact.exact_tree_indices,
        exact_complete: exact.complete,
        exact_coverage_mode: coverage_mode_label(stats.exact_coverage_mode),
        procedural_coverage_generation: stats.exact_coverage_generation,
        procedural_painted_chunks: stats.exact_painted_chunks,
        exact_coverage_mask_bytes: stats.exact_coverage_mask_bytes,
        allocation_slots: stats.allocation_slots,
        staging_slots: stats.staging_slots,
        normal_halo_radius: stats.normal_halo_radius,
        normal_halo_samples_per_tile: stats.normal_halo_samples_per_tile,
        normal_halo_fixed_bytes: stats.normal_halo_fixed_bytes,
        normal_height_fixed_bytes: stats.normal_height_fixed_bytes,
        ready_slots: stats.ready_slots,
        requested_levels: stats.requested_levels,
        staged_levels: stats.staged_levels,
        committed_levels: stats.committed_levels,
        vegetation_committed_levels: stats.vegetation_committed_levels,
        atomic_level_commits: stats.atomic_level_commits,
        deferred_transition_attempts: stats.deferred_transition_attempts,
        pending_refills: stats.pending_refills,
        dispatched_refills: stats.dispatched_refills,
        dispatched_refills_total: stats.dispatched_refills_total,
        total_refills: stats.residency.total_refills,
        total_rebases: stats.residency.total_rebases,
        retained_tiles: stats.residency.retained_tiles,
        drawn_levels: stats.drawn_levels,
        drawn_tiles: stats.drawn_tiles,
        vertex_count: stats.vertex_count,
        vegetation_ready_tiles: stats.vegetation_ready_tiles,
        pending_vegetation_tiles: stats.pending_vegetation_tiles,
        tree_instance_count: stats.tree_instance_count,
        tree_proxy_suppressed_instances: stats.tree_proxy_suppressed_instances,
        tree_proxy_suppressed_records: stats.tree_proxy_suppressed_records,
        tree_proxy_missing_exact_records: stats.tree_proxy_missing_exact_records,
        tree_proxy_missing_proxy_records: stats.tree_proxy_missing_proxy_records,
        tree_proxy_vertex_count: stats.tree_proxy_vertex_count,
        tree_ownership_generation: stats.tree_ownership_generation,
        tree_ownership_units: stats.tree_ownership_units,
        exact_owned_tree_records: stats.exact_owned_tree_records,
        proxy_owned_tree_records: stats.proxy_owned_tree_records,
        fixed_resident_bytes: stats.fixed_resident_bytes,
        vegetation_bytes: stats.vegetation_bytes,
        resident_bytes: stats.resident_bytes,
        vegetation_coordinator_state: coordinator_state_label(
            stats.vegetation_service.coordinator_state,
        ),
        vegetation_executor_kind: executor_kind_label(stats.vegetation_service.executor_kind),
        vegetation_source_fingerprint: stable_hex(stats.vegetation_service.source_fingerprint),
        vegetation_terrain_source_revision: stable_hex(
            stats.vegetation_service.terrain_source_revision,
        ),
        vegetation_compiler_source_revision: stable_hex(
            stats.vegetation_service.compiler_source_revision,
        ),
        vegetation_plan_revision: stable_hex(stats.vegetation_service.vegetation_plan_revision),
        vegetation_product_revision: stats.vegetation_service.product_revision,
        mchv_wire_version: mclone_worldgen::terrain_vegetation::MCHV_WIRE_VERSION,
        vegetation_record_hash: stable_hex(stats.vegetation_service.record_hash),
        vegetation_family_counts: stats.vegetation_service.family_counts,
        vegetation_record_count: stats.vegetation_service.record_count,
        vegetation_product_count: stats.vegetation_service.product_count,
        vegetation_executor_generation: stats.vegetation_service.executor_generation,
        vegetation_source_epoch: stats.vegetation_service.source_epoch,
        vegetation_coverage_revision: stats.vegetation_service.coverage_revision,
        vegetation_desired_tiles: stats.vegetation_service.desired_tiles,
        vegetation_queued_tiles: stats.vegetation_service.queued_tiles,
        vegetation_resident_tiles: stats.vegetation_service.resident_tiles,
        vegetation_in_flight: stats.vegetation_service.in_flight,
        vegetation_submitted_jobs: stats.vegetation_service.submitted_jobs,
        vegetation_completed_jobs: stats.vegetation_service.completed_jobs,
        vegetation_admitted_products: stats.vegetation_service.admitted_products,
        vegetation_source_resets: stats.vegetation_service.source_resets,
        vegetation_transport_failures: stats.vegetation_service.transport_failures,
        vegetation_executor_restarts: stats.vegetation_service.executor_restarts,
        vegetation_job_failures: stats.vegetation_service.job_failures,
        vegetation_stale_completions: stats.vegetation_service.stale_completions,
        vegetation_superseded_completions: stats.vegetation_service.superseded_completions,
        vegetation_submit_full_count: stats.vegetation_service.submit_full_count,
        vegetation_compile_micros: stats.vegetation_service.compile_micros,
        vegetation_cache_cell_requests: stats.vegetation_service.cache_cell_requests,
        vegetation_cache_cell_hits: stats.vegetation_service.cache_cell_hits,
        vegetation_cache_cell_misses: stats.vegetation_service.cache_cell_misses,
        vegetation_cache_retained_cells: stats.vegetation_service.cache_retained_cells,
        vegetation_cache_retained_preliminary_candidates: stats
            .vegetation_service
            .cache_retained_preliminary_candidates,
        worker_submitted_jobs: stats.vegetation_service.executor_submitted_jobs,
        worker_completed_jobs: stats.vegetation_service.executor_completed_jobs,
        worker_transport_failures: stats.vegetation_service.executor_transport_failures,
        worker_restart_count: stats.vegetation_service.executor_restart_count,
        worker_result_capacity_bytes: stats.vegetation_service.result_capacity_bytes,
        worker_result_high_water_bytes: stats.vegetation_service.result_high_water_bytes,
        worker_result_overflow_count: stats.vegetation_service.result_overflow_count,
        worker_copied_result_bytes: stats.vegetation_service.copied_result_bytes,
        worker_main_decode_micros: stats.vegetation_service.main_decode_micros,
        coarse_ready: stats.coarse_ready,
        target_ready: stats.target_ready,
        needs_redraw: stats.needs_redraw,
        held_motion,
    }
}

fn stable_hex(value: u64) -> String {
    format!("{value:016x}")
}

const fn coordinator_state_label(state: Option<TerrainVegetationCoordinatorState>) -> &'static str {
    match state {
        None => "disabled",
        Some(TerrainVegetationCoordinatorState::Starting) => "starting",
        Some(TerrainVegetationCoordinatorState::Running) => "running",
        Some(TerrainVegetationCoordinatorState::Failed) => "failed",
        Some(TerrainVegetationCoordinatorState::ShuttingDown) => "shutting-down",
        Some(TerrainVegetationCoordinatorState::Terminated) => "terminated",
    }
}

const fn executor_kind_label(kind: Option<TerrainVegetationExecutorKind>) -> &'static str {
    match kind {
        None => "disabled",
        Some(TerrainVegetationExecutorKind::InlineTest) => "inline-test",
        Some(TerrainVegetationExecutorKind::NativeThread) => "native-thread",
        Some(TerrainVegetationExecutorKind::BrowserWorker) => "browser-worker",
    }
}

const fn coverage_mode_label(mode: TerrainExactCoverageMode) -> &'static str {
    match mode {
        TerrainExactCoverageMode::Disabled => "disabled",
        TerrainExactCoverageMode::DiscardPainted => "discard-painted",
        TerrainExactCoverageMode::VisualizePainted => "visualize-painted",
    }
}

fn report_json(report: &WebExplorerReport) -> Result<String, JsValue> {
    serde_json::to_string(report).map_err(|error| {
        js_error(format!(
            "failed to serialize World Explorer report: {error}"
        ))
    })
}

fn web_held_direction(code: &str) -> Option<WorldViewHeldDirection> {
    match code {
        "ArrowUp" | "KeyW" => Some(WorldViewHeldDirection::Forward),
        "ArrowDown" | "KeyS" => Some(WorldViewHeldDirection::Backward),
        "ArrowLeft" | "KeyA" => Some(WorldViewHeldDirection::Left),
        "ArrowRight" | "KeyD" => Some(WorldViewHeldDirection::Right),
        _ => None,
    }
}

fn web_contact_button(button: i16) -> Option<ContactButton> {
    match button {
        0 => Some(ContactButton::Primary),
        1 => Some(ContactButton::Auxiliary),
        2 => Some(ContactButton::Secondary),
        _ => None,
    }
}

fn surface_configuration(
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    present_mode: wgpu::PresentMode,
    alpha_mode: wgpu::CompositeAlphaMode,
) -> wgpu::SurfaceConfiguration {
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: width.max(1),
        height: height.max(1),
        present_mode,
        alpha_mode,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    }
}

fn parse_parameter<T>(parameters: &UrlSearchParams, name: &str, default: T) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let Some(value) = parameters.get(name) else {
        return Ok(default);
    };
    value
        .parse()
        .map_err(|error| format!("invalid World Explorer {name}={value:?}: {error}"))
}

fn js_error(message: impl Into<String>) -> JsValue {
    js_sys::Error::new(&message.into()).into()
}
