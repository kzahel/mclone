use std::time::Duration;

use mclone_core::BlockStateId;
use mclone_render::chunk::ChunkTextureAtlas;
use mclone_render_color::RenderColorProfile;
use mclone_terrain_view::{
    BrowserCanonicalExactExecutor, BrowserTerrainVegetationExecutor,
    TERRAIN_PREVIEW_MATERIAL_UV_COUNT, TerrainClipmapConfig, TerrainExactCoverageMode,
    TerrainHorizonRenderTarget, TerrainPreviewMaterialAtlas, TerrainRuntimeConfig,
    TerrainRuntimeExactAnchor, TerrainRuntimeExactRenderer, TerrainRuntimeExactStats,
    TerrainRuntimeSession,
};
use mclone_view_control::{WorldViewMode, WorldViewProjection, WorldViewState};
use serde::Serialize;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
use web_sys::HtmlCanvasElement;

use crate::visual_assets::load_terrain_lab_visual_assets;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainRuntimeReport {
    exact_desired_chunks: u32,
    exact_painted_chunks: u32,
    exact_queued_chunks: u32,
    exact_pending_admissions: u32,
    exact_in_flight: bool,
    exact_complete: bool,
    exact_natural_tree_records: u32,
    exact_owned_tree_records: u32,
    proxy_owned_tree_records: u32,
    exact_vertex_count: u32,
    exact_index_count: u32,
    horizon_ready_slots: u32,
    horizon_pending_refills: u32,
    vegetation_ready_tiles: u32,
    vegetation_pending_tiles: u32,
    tree_instances: u32,
    tree_proxy_suppressed_records: u32,
    coarse_ready: bool,
    target_ready: bool,
    needs_redraw: bool,
}

#[wasm_bindgen(js_name = TerrainRuntimeCompositionLab)]
pub struct TerrainRuntimeCompositionLab {
    canvas: HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    present_mode: wgpu::PresentMode,
    alpha_mode: wgpu::CompositeAlphaMode,
    width: u32,
    height: u32,
    frame_epoch_ms: Option<f64>,
    exact_radius: u32,
    session: TerrainRuntimeSession,
    exact: TerrainRuntimeExactRenderer,
}

#[wasm_bindgen(js_class = TerrainRuntimeCompositionLab)]
impl TerrainRuntimeCompositionLab {
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
        self.exact.resize(&self.device, width, height);
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = renderFrame)]
    pub fn render_frame(
        &mut self,
        frame_millis: f64,
        center_x: i32,
        center_z: i32,
        blocks_across: u32,
        view: String,
        yaw_radians: f64,
        pitch_radians: f64,
        projection: String,
    ) -> Result<String, JsValue> {
        let view_state = runtime_view_state(
            center_x,
            center_z,
            blocks_across,
            &view,
            yaw_radians,
            pitch_radians,
            &projection,
        )
        .map_err(js_error)?;
        self.session.set_view_state(view_state);

        let frame_millis = if frame_millis.is_finite() {
            frame_millis.max(0.0)
        } else {
            self.frame_epoch_ms.unwrap_or(0.0)
        };
        let frame_epoch_ms = *self.frame_epoch_ms.get_or_insert(frame_millis);
        let elapsed = Duration::from_secs_f64(
            ((frame_millis - frame_epoch_ms).max(0.0) / 1_000.0).min(f64::from(u32::MAX)),
        );
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(
                    &self.device,
                    &surface_configuration(
                        self.format,
                        self.width,
                        self.height,
                        self.present_mode,
                        self.alpha_mode,
                    ),
                );
                self.surface.get_current_texture().map_err(|error| {
                    js_error(format!(
                        "runtime composition surface frame failed after reconfigure: {error}"
                    ))
                })?
            }
            Err(error) => {
                return Err(js_error(format!(
                    "runtime composition surface frame failed: {error}"
                )));
            }
        };
        let color_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_terrain_lab_runtime_composition_frame"),
            });

        let exact_view = self
            .session
            .exact_composition_view(self.exact_radius, TerrainRuntimeExactAnchor::Focus)
            .map_err(js_error)?;
        self.session.apply_exact_composition_view(exact_view);
        self.exact
            .update_and_pump(
                &self.device,
                exact_view.residency_anchor[0],
                exact_view.residency_anchor[1],
            )
            .map_err(|error| js_error(error.to_string()))?;
        let prepared_exact = self.exact.prepared_frame().map_err(js_error)?;
        let mut horizon = self
            .session
            .encode_prepared_to_target(
                &self.device,
                &self.queue,
                &mut encoder,
                TerrainHorizonRenderTarget {
                    color_view: &color_view,
                    depth_view: &self.exact.depth().view,
                    color_load: wgpu::LoadOp::Clear(self.exact.clear_color()),
                    color_store: wgpu::StoreOp::Store,
                    depth_load: wgpu::LoadOp::Clear(0.0),
                    depth_store: wgpu::StoreOp::Store,
                },
                elapsed,
                Some((&prepared_exact, TerrainExactCoverageMode::DiscardPainted)),
                Some(self.exact.tree_ownership()),
            )
            .map_err(js_error)?;
        self.exact
            .render(
                &self.queue,
                &mut encoder,
                &color_view,
                exact_view.render_view,
                [self.width, self.height],
                true,
            )
            .map_err(|error| js_error(error.to_string()))?;
        let exact = self.exact.stats();
        horizon.target_ready &= exact.complete;
        horizon.needs_redraw |= !exact.complete;

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        report_json(runtime_report(exact, horizon))
    }

    pub fn shutdown(&mut self) {
        self.session.shutdown();
    }

    #[wasm_bindgen(js_name = shutdownComplete)]
    pub fn shutdown_complete(&self) -> bool {
        self.session.shutdown_complete()
    }
}

impl TerrainRuntimeCompositionLab {
    #[allow(clippy::too_many_arguments)]
    async fn new(
        canvas: HtmlCanvasElement,
        authored_bytes: js_sys::Uint8Array,
        reference_bytes: js_sys::Uint8Array,
        provisional_bytes: js_sys::Uint8Array,
        diagnostic_bytes: js_sys::Uint8Array,
        visual_profile: String,
        texture_presentation: String,
        seed: i64,
        exact_radius: u32,
        initial_view: WorldViewState,
        vegetation_worker_transport_factory: JsValue,
        exact_worker_transport_factory: JsValue,
    ) -> Result<Self, String> {
        if exact_radius > 15 {
            return Err("Terrain Lab runtime exact radius must be at most 15 chunks".to_owned());
        }
        let authored = authored_bytes.to_vec();
        let reference = reference_bytes.to_vec();
        let provisional = provisional_bytes.to_vec();
        let diagnostic = diagnostic_bytes.to_vec();
        let visual_assets = load_terrain_lab_visual_assets(
            authored.clone(),
            reference.clone(),
            provisional.clone(),
            diagnostic.clone(),
            &visual_profile,
            &texture_presentation,
        )?;
        let assets = visual_assets.terrain;
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
            .map_err(|error| {
                format!("failed to create Terrain Lab runtime WebGPU surface: {error}")
            })?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| {
                format!("failed to request Terrain Lab runtime WebGPU adapter: {error}")
            })?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("mclone_terrain_lab_runtime_device"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .map_err(|error| {
                format!("failed to request Terrain Lab runtime WebGPU device: {error}")
            })?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or("Terrain Lab runtime WebGPU surface reported no color formats")?;
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

        let vegetation_executor =
            BrowserTerrainVegetationExecutor::new(vegetation_worker_transport_factory)?;
        let session = TerrainRuntimeSession::new(
            &device,
            &queue,
            format,
            TerrainRuntimeConfig {
                width,
                height,
                seed,
                initial_view,
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
        let exact_executor = BrowserCanonicalExactExecutor::new_with_visual_assets(
            exact_worker_transport_factory,
            seed,
            authored,
            reference,
            provisional,
            diagnostic,
            &visual_profile,
            &texture_presentation,
        )?;
        let exact = TerrainRuntimeExactRenderer::new_with_executor(
            &device,
            &queue,
            format,
            width,
            height,
            seed,
            exact_radius,
            Box::new(exact_executor),
            ChunkTextureAtlas {
                width: assets.atlas.width,
                height: assets.atlas.height,
                rgba: assets.atlas.rgba(),
            },
            false,
            RenderColorProfile::Vanilla.target_color_transform(format),
        )
        .map_err(|error| error.to_string())?;
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
            frame_epoch_ms: None,
            exact_radius,
            session,
            exact,
        })
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn mclone_terrain_lab_create_runtime_composition(
    canvas: HtmlCanvasElement,
    authored_bytes: js_sys::Uint8Array,
    reference_bytes: js_sys::Uint8Array,
    provisional_bytes: js_sys::Uint8Array,
    diagnostic_bytes: js_sys::Uint8Array,
    visual_profile: String,
    texture_presentation: String,
    seed: String,
    exact_radius: u32,
    center_x: i32,
    center_z: i32,
    blocks_across: u32,
    view: String,
    yaw_radians: f64,
    pitch_radians: f64,
    projection: String,
    vegetation_worker_transport_factory: JsValue,
    exact_worker_transport_factory: JsValue,
) -> js_sys::Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        let seed = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        let initial_view = runtime_view_state(
            center_x,
            center_z,
            blocks_across,
            &view,
            yaw_radians,
            pitch_radians,
            &projection,
        )
        .map_err(js_error)?;
        TerrainRuntimeCompositionLab::new(
            canvas,
            authored_bytes,
            reference_bytes,
            provisional_bytes,
            diagnostic_bytes,
            visual_profile,
            texture_presentation,
            seed,
            exact_radius,
            initial_view,
            vegetation_worker_transport_factory,
            exact_worker_transport_factory,
        )
        .await
        .map(JsValue::from)
        .map_err(JsValue::from)
    })
}

fn runtime_view_state(
    center_x: i32,
    center_z: i32,
    blocks_across: u32,
    view: &str,
    yaw_radians: f64,
    pitch_radians: f64,
    projection: &str,
) -> Result<WorldViewState, String> {
    let mode = match view.trim().to_ascii_lowercase().as_str() {
        "map" | "2d" => WorldViewMode::Map,
        "3d" | "terrain" | "orbit" => WorldViewMode::Orbit,
        other => return Err(format!("unsupported runtime terrain view {other:?}")),
    };
    let projection = match projection.trim().to_ascii_lowercase().as_str() {
        "orthographic" | "ortho" => WorldViewProjection::Orthographic,
        "perspective" => WorldViewProjection::Perspective,
        other => return Err(format!("unsupported runtime terrain projection {other:?}")),
    };
    Ok(WorldViewState {
        mode,
        focus_x: f64::from(center_x),
        focus_z: f64::from(center_z),
        blocks_across: f64::from(blocks_across.max(1)),
        yaw_radians,
        pitch_radians,
        projection,
    })
}

fn runtime_report(
    exact: TerrainRuntimeExactStats,
    horizon: mclone_terrain_view::TerrainHorizonFrameStats,
) -> TerrainRuntimeReport {
    TerrainRuntimeReport {
        exact_desired_chunks: exact.desired_chunks,
        exact_painted_chunks: exact.painted_chunks,
        exact_queued_chunks: exact.queued_chunks,
        exact_pending_admissions: exact.pending_admissions,
        exact_in_flight: exact.in_flight,
        exact_complete: exact.complete,
        exact_natural_tree_records: exact.natural_tree_records,
        exact_owned_tree_records: exact.exact_owned_tree_records,
        proxy_owned_tree_records: exact.proxy_owned_tree_records,
        exact_vertex_count: exact.vertex_count,
        exact_index_count: exact.index_count,
        horizon_ready_slots: horizon.ready_slots,
        horizon_pending_refills: horizon.pending_refills,
        vegetation_ready_tiles: horizon.vegetation_ready_tiles,
        vegetation_pending_tiles: horizon.pending_vegetation_tiles,
        tree_instances: horizon.tree_instance_count,
        tree_proxy_suppressed_records: horizon.tree_proxy_suppressed_records,
        coarse_ready: horizon.coarse_ready,
        target_ready: horizon.target_ready && exact.complete,
        needs_redraw: horizon.needs_redraw || !exact.complete,
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
        desired_maximum_frame_latency: 2,
        alpha_mode,
        view_formats: vec![],
    }
}

fn report_json(report: TerrainRuntimeReport) -> Result<String, JsValue> {
    serde_json::to_string(&report).map_err(|error| {
        js_error(format!(
            "failed to encode runtime composition report: {error}"
        ))
    })
}

fn js_error(message: impl AsRef<str>) -> JsValue {
    js_sys::Error::new(message.as_ref()).into()
}
