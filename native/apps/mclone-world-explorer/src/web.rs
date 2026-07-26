use std::time::Duration;

use mclone_assets::{
    AUTHORED_FIRST_PARTY_PACK_ID, AssetPackId, AssetPackOrigin, AssetSourceChain,
    DIAGNOSTIC_MISSING_PACK_ID, PROVISIONAL_FIRST_PARTY_PACK_ID, PackedAssetSource,
    TexturePresentation,
};
use mclone_core::BlockStateId;
use mclone_mesh::load_first_party_textured_terrain_assets_with_presentation;
use mclone_terrain_view::{
    TERRAIN_PREVIEW_MATERIAL_UV_COUNT, TerrainClipmapConfig, TerrainHorizonFrameStats,
    TerrainPreviewMaterialAtlas,
};
use mclone_view_control::{
    ContactEvent, ContactPurpose, ViewPoint, ViewportMetrics, WorldViewHeldDirection,
    WorldViewIntent, WorldViewMode, WorldViewProjection, WorldViewState,
};
use serde::Serialize;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
use web_sys::{HtmlCanvasElement, UrlSearchParams};

use crate::{WorldExplorerConfig, WorldExplorerSession};

const DEFAULT_SEED: i64 = 12_345;
const DEFAULT_BLOCKS_ACROSS: u32 = 4_096;

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
    yaw_radians: f64,
    pitch_radians: f64,
    allocation_slots: u32,
    ready_slots: u32,
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
    tree_proxy_vertex_count: u32,
    fixed_resident_bytes: u64,
    vegetation_bytes: u64,
    resident_bytes: u64,
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
    }

    #[wasm_bindgen(js_name = renderFrame)]
    pub fn render_frame(&mut self, frame_millis: f64) -> Result<String, JsValue> {
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
        let stats = self
            .session
            .encode(
                &self.device,
                &self.queue,
                &mut encoder,
                &color_view,
                elapsed,
            )
            .map_err(js_error)?;
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        report_json(
            self.seed,
            self.session.view_state(),
            self.session.has_held_motion(),
            stats,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = pointerDown)]
    pub fn pointer_down(
        &mut self,
        pointer_id: u32,
        button: i16,
        x: f64,
        y: f64,
        time_seconds: f64,
        width: f64,
        height: f64,
    ) -> bool {
        self.session.contact(ContactEvent::Down {
            id: u64::from(pointer_id),
            position: ViewPoint::new(x, y),
            purpose: if button == 2 {
                ContactPurpose::Pan
            } else {
                ContactPurpose::ViewDefault
            },
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

    pub fn recenter(&mut self, world_x: f64, world_z: f64) -> bool {
        self.session
            .apply_intent(WorldViewIntent::FocusAt { world_x, world_z })
    }

    pub fn diagnostics(&self) -> String {
        self.session.diagnostics()
    }
}

impl WebWorldExplorer {
    async fn new(
        canvas: HtmlCanvasElement,
        authored_bytes: js_sys::Uint8Array,
        provisional_bytes: js_sys::Uint8Array,
        diagnostic_bytes: js_sys::Uint8Array,
        search: String,
    ) -> Result<Self, String> {
        let options = WebExplorerOptions::parse(&search)?;
        let assets = load_web_assets(
            authored_bytes.to_vec(),
            provisional_bytes.to_vec(),
            diagnostic_bytes.to_vec(),
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
            .first()
            .copied()
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
                vegetation_enabled: false,
            },
            TerrainPreviewMaterialAtlas {
                width: assets.atlas.width,
                height: assets.atlas.height,
                rgba: assets.atlas.rgba(),
                material_uvs: &material_uvs,
            },
        )?;
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
) -> js_sys::Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        WebWorldExplorer::new(
            canvas,
            authored_bytes,
            provisional_bytes,
            diagnostic_bytes,
            search,
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

fn load_web_assets(
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

fn report_json(
    seed: i64,
    state: WorldViewState,
    held_motion: bool,
    stats: TerrainHorizonFrameStats,
) -> Result<String, JsValue> {
    serde_json::to_string(&WebExplorerReport {
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
        yaw_radians: state.yaw_radians,
        pitch_radians: state.pitch_radians,
        allocation_slots: stats.allocation_slots,
        ready_slots: stats.ready_slots,
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
        tree_proxy_vertex_count: stats.tree_proxy_vertex_count,
        fixed_resident_bytes: stats.fixed_resident_bytes,
        vegetation_bytes: stats.vegetation_bytes,
        resident_bytes: stats.resident_bytes,
        coarse_ready: stats.coarse_ready,
        target_ready: stats.target_ready,
        needs_redraw: stats.needs_redraw,
        held_motion,
    })
    .map_err(|error| {
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
