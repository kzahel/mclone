use mclone_terrain_view::{
    TERRAIN_PREVIEW_GPU_EVALUATOR_REVISION, TerrainPreviewCompletedComparison,
    TerrainPreviewFrameStats, TerrainPreviewRenderer,
};
use mclone_worldgen::{
    levelgen::McloneOverworldSamplingTopology,
    terrain_preview::{
        TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS, TERRAIN_PREVIEW_REFERENCE_SCHEMA_REVISION,
        TerrainPreviewReferenceGrid, TerrainPreviewRequest, terrain_preview_field_revision,
    },
};
use serde::Serialize;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
use web_sys::HtmlCanvasElement;

use crate::{terrain_preview_option_labels, terrain_preview_options};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainLabAdapterReport<'a> {
    name: &'a str,
    backend: &'a str,
    device_type: &'a str,
    driver: &'a str,
    driver_info: &'a str,
    field_revision: &'static str,
    reference_schema_revision: &'static str,
    gpu_evaluator_revision: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainLabRenderReport<'a> {
    revision: u64,
    field_revision: &'static str,
    reference_schema_revision: &'static str,
    gpu_evaluator_revision: &'static str,
    approximation: bool,
    seed: &'a str,
    center_x: i32,
    center_z: i32,
    sample_spacing: u32,
    cells_per_axis: u32,
    samples_per_axis: u32,
    sample_count: u32,
    vertex_count: u32,
    footprint_blocks: u32,
    footprint_chunks: u32,
    source: &'static str,
    view: &'static str,
    layer: &'static str,
    topology: &'static str,
    width: u32,
    height: u32,
    cpu_reference_ms: f64,
    encode_submit_ms: f64,
    request_ms: f64,
    reference_bytes: u64,
    gpu_sample_bytes: u64,
    readback_bytes: u64,
    resident_bytes: u64,
    comparison_pending: bool,
    stale_result_count: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerrainLabComparisonReport {
    revision: u64,
    sample_count: usize,
    max_absolute_surface_error: f32,
    mean_absolute_surface_error: f32,
    p95_absolute_surface_error: f32,
    water_presence_agreement: f32,
    stale_result_count: u64,
}

#[wasm_bindgen]
pub struct TerrainLab {
    canvas: HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    present_mode: wgpu::PresentMode,
    alpha_mode: wgpu::CompositeAlphaMode,
    width: u32,
    height: u32,
    adapter_name: String,
    adapter_backend: String,
    adapter_device_type: String,
    adapter_driver: String,
    adapter_driver_info: String,
    renderer: TerrainPreviewRenderer,
}

#[wasm_bindgen]
impl TerrainLab {
    #[wasm_bindgen(js_name = adapterReport)]
    pub fn adapter_report(&self) -> Result<String, JsValue> {
        json(&TerrainLabAdapterReport {
            name: &self.adapter_name,
            backend: &self.adapter_backend,
            device_type: &self.adapter_device_type,
            driver: &self.adapter_driver,
            driver_info: &self.adapter_driver_info,
            field_revision: terrain_preview_field_revision(),
            reference_schema_revision: TERRAIN_PREVIEW_REFERENCE_SCHEMA_REVISION,
            gpu_evaluator_revision: TERRAIN_PREVIEW_GPU_EVALUATOR_REVISION,
        })
    }

    #[wasm_bindgen]
    pub fn resize(&mut self, width: u32, height: u32) {
        self.resize_surface(width, height);
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen]
    pub fn render(
        &mut self,
        revision: u32,
        seed: String,
        center_x: i32,
        center_z: i32,
        sample_spacing: u32,
        source: String,
        view: String,
        layer: String,
    ) -> Result<String, JsValue> {
        let request_start = now_ms()?;
        let seed_value = seed
            .trim()
            .parse::<i64>()
            .map_err(|error| js_error(format!("invalid signed 64-bit seed {seed:?}: {error}")))?;
        let options = terrain_preview_options(&source, &view, &layer).map_err(js_error)?;
        let request = TerrainPreviewRequest {
            seed: seed_value,
            center_x,
            center_z,
            sample_spacing,
            cells_per_axis: TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS,
            topology: McloneOverworldSamplingTopology::Unbounded,
        };

        let reference_start = now_ms()?;
        let reference = TerrainPreviewReferenceGrid::compile(request).map_err(js_error)?;
        let reference_finished = now_ms()?;
        let cpu_reference_ms = reference_finished - reference_start;

        let frame = self.acquire_frame()?;
        let color_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encode_start = now_ms()?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mclone_terrain_lab_encoder"),
            });
        let (stats, readback) = self
            .renderer
            .encode(
                &self.device,
                &self.queue,
                &mut encoder,
                &color_view,
                self.width,
                self.height,
                u64::from(revision),
                &reference,
                options,
            )
            .map_err(js_error)?;
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        self.renderer.mark_submitted(readback);
        let finished = now_ms()?;
        let (source, view, layer) = terrain_preview_option_labels(options);
        let report = render_report(
            stats,
            &seed,
            center_x,
            center_z,
            sample_spacing,
            source,
            view,
            layer,
            self.width,
            self.height,
            cpu_reference_ms,
            finished - encode_start,
            finished - request_start,
            self.renderer.stale_result_count(),
        );
        json(&report)
    }

    #[wasm_bindgen(js_name = pollComparison)]
    pub fn poll_comparison(&mut self) -> Result<Option<String>, JsValue> {
        let mut latest = None;
        for result in self.renderer.poll_completed(&self.device) {
            let result = result.map_err(js_error)?;
            latest = Some(comparison_report(
                result,
                self.renderer.stale_result_count(),
            ));
        }
        latest.map(|report| json(&report)).transpose()
    }
}

impl TerrainLab {
    async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|error| format!("failed to create Terrain Lab WebGPU surface: {error}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| format!("failed to request Terrain Lab WebGPU adapter: {error}"))?;
        let adapter_info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("mclone_terrain_lab_device"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .map_err(|error| format!("failed to request Terrain Lab WebGPU device: {error}"))?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or("Terrain Lab WebGPU surface reported no color formats")?;
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
        let renderer = TerrainPreviewRenderer::new(
            &device,
            format,
            width,
            height,
            TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS,
        )?;
        if let Some(error) = device.pop_error_scope().await {
            return Err(format!(
                "failed to initialize Terrain Lab GPU pipelines: {error}"
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
            adapter_name: adapter_info.name,
            adapter_backend: format!("{:?}", adapter_info.backend),
            adapter_device_type: format!("{:?}", adapter_info.device_type),
            adapter_driver: adapter_info.driver,
            adapter_driver_info: adapter_info.driver_info,
            renderer,
        })
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
        self.renderer.resize(&self.device, width, height);
    }

    fn acquire_frame(&mut self) -> Result<wgpu::SurfaceTexture, JsValue> {
        match self.surface.get_current_texture() {
            Ok(frame) => Ok(frame),
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.resize_surface(self.width, self.height);
                self.surface.get_current_texture().map_err(|error| {
                    js_error(format!(
                        "failed to acquire Terrain Lab WebGPU frame after reconfigure: {error}"
                    ))
                })
            }
            Err(error) => Err(js_error(format!(
                "failed to acquire Terrain Lab WebGPU frame: {error}"
            ))),
        }
    }
}

#[wasm_bindgen]
pub fn mclone_terrain_lab_create(canvas: HtmlCanvasElement) -> js_sys::Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        TerrainLab::new(canvas)
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

#[allow(clippy::too_many_arguments)]
fn render_report<'a>(
    stats: TerrainPreviewFrameStats,
    seed: &'a str,
    center_x: i32,
    center_z: i32,
    sample_spacing: u32,
    source: &'static str,
    view: &'static str,
    layer: &'static str,
    width: u32,
    height: u32,
    cpu_reference_ms: f64,
    encode_submit_ms: f64,
    request_ms: f64,
    stale_result_count: u64,
) -> TerrainLabRenderReport<'a> {
    TerrainLabRenderReport {
        revision: stats.revision,
        field_revision: terrain_preview_field_revision(),
        reference_schema_revision: TERRAIN_PREVIEW_REFERENCE_SCHEMA_REVISION,
        gpu_evaluator_revision: TERRAIN_PREVIEW_GPU_EVALUATOR_REVISION,
        approximation: true,
        seed,
        center_x,
        center_z,
        sample_spacing,
        cells_per_axis: stats.cells_per_axis,
        samples_per_axis: stats.samples_per_axis,
        sample_count: stats.sample_count,
        vertex_count: stats.vertex_count,
        footprint_blocks: stats.footprint_blocks,
        footprint_chunks: stats.footprint_blocks / 16,
        source,
        view,
        layer,
        topology: McloneOverworldSamplingTopology::Unbounded.label(),
        width,
        height,
        cpu_reference_ms,
        encode_submit_ms,
        request_ms,
        reference_bytes: stats.reference_bytes,
        gpu_sample_bytes: stats.gpu_sample_bytes,
        readback_bytes: stats.readback_bytes,
        resident_bytes: stats.resident_bytes,
        comparison_pending: true,
        stale_result_count,
    }
}

fn comparison_report(
    completed: TerrainPreviewCompletedComparison,
    stale_result_count: u64,
) -> TerrainLabComparisonReport {
    TerrainLabComparisonReport {
        revision: completed.revision,
        sample_count: completed.comparison.sample_count,
        max_absolute_surface_error: completed.comparison.max_absolute_surface_error,
        mean_absolute_surface_error: completed.comparison.mean_absolute_surface_error,
        p95_absolute_surface_error: completed.comparison.p95_absolute_surface_error,
        water_presence_agreement: completed.comparison.water_presence_agreement,
        stale_result_count,
    }
}

fn now_ms() -> Result<f64, JsValue> {
    web_sys::window()
        .ok_or_else(|| js_error("Terrain Lab has no Window"))?
        .performance()
        .ok_or_else(|| js_error("Terrain Lab has no Performance clock"))
        .map(|performance| performance.now())
}

fn json(value: &impl Serialize) -> Result<String, JsValue> {
    serde_json::to_string(value)
        .map_err(|error| js_error(format!("failed to serialize Terrain Lab report: {error}")))
}

fn js_error(message: impl AsRef<str>) -> JsValue {
    JsValue::from_str(message.as_ref())
}
