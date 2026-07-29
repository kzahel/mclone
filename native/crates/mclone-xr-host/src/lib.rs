#![deny(unsafe_code)]

mod actions;
mod frame_driver;
mod render_path;

use anyhow::{Context, Result};
use glam::{Mat4, Quat, Vec3};
use openxr as xr;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::sync::mpsc;

pub use actions::OpenXrControllerActions;
pub use frame_driver::*;
pub use mclone_input::{
    TrackedControllerState, XrControllerSpecificState, XrHand, XrInputFrame, XrSpecificInput,
};
pub use mclone_render_session::{
    XrFov, XrRenderView, XrView, XrViewPose, render_view_from_world_pose, xr_fov_aspect,
    xr_fov_to_projection_rh,
};
pub use render_path::*;

pub const PRIMARY_STEREO_VIEW_TYPE: xr::ViewConfigurationType =
    xr::ViewConfigurationType::PRIMARY_STEREO;
pub const XR_REFRESH_RATE_MATCH_EPSILON_HZ: f32 = 0.05;
const RGBA8_BYTES_PER_PIXEL: u32 = 4;
const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenXrPollStatus {
    Idle,
    Running,
    Exit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenXrHostEvent {
    SessionStateChanged(xr::SessionState),
    InstanceLossPending,
    EventsLost(u32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct XrFrameStats {
    pub submitted_frames: u64,
    pub runtime_frames: u64,
    pub skipped_frames: u64,
}

impl XrFrameStats {
    pub fn record_runtime_frame(&mut self) {
        self.runtime_frames += 1;
    }

    pub fn record_submitted_frame(&mut self) {
        self.submitted_frames += 1;
    }

    pub fn record_skipped_frame(&mut self) {
        self.skipped_frames += 1;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XrEyeConfig {
    pub width: u32,
    pub height: u32,
    pub recommended_sample_count: u32,
    pub max_sample_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XrStereoConfig {
    pub left: XrEyeConfig,
    pub right: XrEyeConfig,
}

#[derive(Clone, Copy)]
pub struct XrStereoFrameViews {
    pub left: xr::View,
    pub right: xr::View,
}

pub fn xr_fov_from_openxr(fov: xr::Fovf) -> XrFov {
    XrFov {
        angle_left: fov.angle_left,
        angle_right: fov.angle_right,
        angle_up: fov.angle_up,
        angle_down: fov.angle_down,
    }
}

pub fn xr_view_from_openxr(view: &xr::View) -> Result<XrView> {
    Ok(XrView {
        pose: view_pose(view)?,
        fov: xr_fov_from_openxr(view.fov),
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct XrDisplayRefreshSnapshot {
    pub extension_supported: bool,
    pub supported_rates: Vec<f32>,
    pub current_rate: Option<f32>,
}

pub trait XrEyeSwapchain<G>
where
    G: xr::Graphics,
{
    fn swapchain(&self) -> &xr::Swapchain<G>;
    fn swapchain_mut(&mut self) -> &mut xr::Swapchain<G>;
    fn textures(&self) -> &[wgpu::Texture];
    fn width(&self) -> u32;
    fn height(&self) -> u32;

    fn outstanding_image_count(&self) -> u32 {
        0
    }

    fn record_image_acquired(&mut self) {}

    fn record_image_released(&mut self) {}
}

pub trait XrStereoSwapchain<G>
where
    G: xr::Graphics,
{
    fn swapchain(&self) -> &xr::Swapchain<G>;
    fn swapchain_mut(&mut self) -> &mut xr::Swapchain<G>;
    fn textures(&self) -> &[wgpu::Texture];
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn array_size(&self) -> u32;

    fn outstanding_image_count(&self) -> u32 {
        0
    }

    fn record_image_acquired(&mut self) {}

    fn record_image_released(&mut self) {}
}

pub struct XrAcquiredEyeTarget<'a, G, E>
where
    G: xr::Graphics,
    E: XrEyeSwapchain<G> + ?Sized,
{
    eye_state: &'a mut E,
    color_view: wgpu::TextureView,
    _graphics: PhantomData<G>,
}

impl<'a, G, E> XrAcquiredEyeTarget<'a, G, E>
where
    G: xr::Graphics,
    E: XrEyeSwapchain<G> + ?Sized,
{
    pub fn eye(&self) -> &E {
        self.eye_state
    }

    pub fn color_view(&self) -> &wgpu::TextureView {
        &self.color_view
    }

    pub fn release(self) -> Result<()> {
        self.eye_state
            .swapchain_mut()
            .release_image()
            .context("release OpenXR swapchain image")?;
        self.eye_state.record_image_released();
        Ok(())
    }
}

pub struct XrAcquiredStereoTarget<'a, G, S>
where
    G: xr::Graphics,
    S: XrStereoSwapchain<G> + ?Sized,
{
    stereo_state: &'a mut S,
    image_index: u32,
    color_array_view: wgpu::TextureView,
    color_left_view: wgpu::TextureView,
    color_right_view: wgpu::TextureView,
    _graphics: PhantomData<G>,
}

impl<'a, G, S> XrAcquiredStereoTarget<'a, G, S>
where
    G: xr::Graphics,
    S: XrStereoSwapchain<G> + ?Sized,
{
    pub fn stereo(&self) -> &S {
        self.stereo_state
    }

    pub fn color_array_view(&self) -> &wgpu::TextureView {
        &self.color_array_view
    }

    pub fn color_left_view(&self) -> &wgpu::TextureView {
        &self.color_left_view
    }

    pub fn color_right_view(&self) -> &wgpu::TextureView {
        &self.color_right_view
    }

    pub fn color_texture(&self) -> Result<&wgpu::Texture> {
        self.stereo_state
            .textures()
            .get(self.image_index as usize)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "OpenXR returned out-of-range stereo array image index {}",
                    self.image_index
                )
            })
    }

    pub fn release(self) -> Result<()> {
        self.stereo_state
            .swapchain_mut()
            .release_image()
            .context("release OpenXR stereo array swapchain image")?;
        self.stereo_state.record_image_released();
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct XrClearTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub depth_view: Option<&'a wgpu::TextureView>,
}

impl XrStereoConfig {
    pub fn primary_eye_size(self) -> (u32, u32) {
        (self.left.width.max(1), self.left.height.max(1))
    }
}

pub fn selected_environment_blend_mode(
    blend_modes: &[xr::EnvironmentBlendMode],
) -> xr::EnvironmentBlendMode {
    blend_modes
        .iter()
        .copied()
        .find(|mode| *mode == xr::EnvironmentBlendMode::OPAQUE)
        .unwrap_or(blend_modes[0])
}

pub fn stereo_config(views: &[xr::ViewConfigurationView]) -> Result<XrStereoConfig> {
    if views.len() < 2 {
        anyhow::bail!(
            "OpenXR PRIMARY_STEREO reported {} view(s); mclone requires at least two",
            views.len()
        );
    }
    Ok(XrStereoConfig {
        left: eye_config(views[0]),
        right: eye_config(views[1]),
    })
}

pub fn format_debug_list<T: std::fmt::Debug>(values: &[T]) -> String {
    values
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn format_view_configurations(views: &[xr::ViewConfigurationView]) -> String {
    views
        .iter()
        .enumerate()
        .map(|(index, view)| {
            format!(
                "#{index} recommended={}x{} max={}x{} samples={}/{}",
                view.recommended_image_rect_width,
                view.recommended_image_rect_height,
                view.max_image_rect_width,
                view.max_image_rect_height,
                view.recommended_swapchain_sample_count,
                view.max_swapchain_sample_count
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn query_display_refresh_snapshot<G>(
    session: &xr::Session<G>,
    extension_supported: bool,
) -> XrDisplayRefreshSnapshot {
    if !extension_supported {
        return XrDisplayRefreshSnapshot::default();
    }
    XrDisplayRefreshSnapshot {
        extension_supported,
        supported_rates: query_supported_display_refresh_rates(session),
        current_rate: query_current_display_refresh_rate(session),
    }
}

pub fn query_supported_display_refresh_rates<G>(session: &xr::Session<G>) -> Vec<f32> {
    match session.enumerate_display_refresh_rates() {
        Ok(rates) => sorted_display_refresh_rates(rates),
        Err(err) => {
            log::warn!("OpenXR display refresh rate enumeration failed: {err:?}");
            Vec::new()
        }
    }
}

pub fn query_current_display_refresh_rate<G>(session: &xr::Session<G>) -> Option<f32> {
    match session.get_display_refresh_rate() {
        Ok(rate) if rate.is_finite() && rate > 0.0 => Some(rate),
        Ok(rate) => {
            log::warn!("OpenXR runtime returned invalid display refresh rate: {rate:?}");
            None
        }
        Err(err) => {
            log::warn!("OpenXR current display refresh query failed: {err:?}");
            None
        }
    }
}

pub fn sorted_display_refresh_rates(mut rates: Vec<f32>) -> Vec<f32> {
    rates.retain(|rate| rate.is_finite() && *rate > 0.0);
    rates.sort_by(|a, b| a.total_cmp(b));
    rates.dedup_by(|a, b| refresh_rates_match(*a, *b));
    rates
}

pub fn refresh_rates_match(a: f32, b: f32) -> bool {
    (a - b).abs() <= XR_REFRESH_RATE_MATCH_EPSILON_HZ
}

pub fn display_refresh_rates_label(rates: &[f32]) -> String {
    if rates.is_empty() {
        return "none reported".to_owned();
    }
    rates
        .iter()
        .map(|rate| format!("{rate:.1} Hz"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn eye_config(view: xr::ViewConfigurationView) -> XrEyeConfig {
    XrEyeConfig {
        width: view.recommended_image_rect_width.max(1),
        height: view.recommended_image_rect_height.max(1),
        recommended_sample_count: view.recommended_swapchain_sample_count,
        max_sample_count: view.max_swapchain_sample_count,
    }
}

fn poll_openxr_events<G, F>(
    session: &xr::Session<G>,
    event_storage: &mut xr::EventDataBuffer,
    session_running: &mut bool,
    view_type: xr::ViewConfigurationType,
    mut on_event: F,
) -> Result<OpenXrPollStatus>
where
    G: xr::Graphics,
    F: FnMut(OpenXrHostEvent),
{
    while let Some(event) = session
        .instance()
        .poll_event(event_storage)
        .context("poll OpenXR event")?
    {
        match event {
            xr::Event::SessionStateChanged(event) => {
                let state = event.state();
                on_event(OpenXrHostEvent::SessionStateChanged(state));
                match state {
                    xr::SessionState::READY if !*session_running => {
                        session.begin(view_type).context("begin OpenXR session")?;
                        *session_running = true;
                    }
                    xr::SessionState::STOPPING if *session_running => {
                        session.end().context("end OpenXR session")?;
                        *session_running = false;
                    }
                    xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                        return Ok(OpenXrPollStatus::Exit);
                    }
                    _ => {}
                }
            }
            xr::Event::InstanceLossPending(_) => {
                on_event(OpenXrHostEvent::InstanceLossPending);
                return Ok(OpenXrPollStatus::Exit);
            }
            xr::Event::EventsLost(event) => {
                on_event(OpenXrHostEvent::EventsLost(event.lost_event_count()));
            }
            _ => {}
        }
    }

    if *session_running {
        Ok(OpenXrPollStatus::Running)
    } else {
        Ok(OpenXrPollStatus::Idle)
    }
}

pub fn create_stage_reference_space<G>(session: &xr::Session<G>) -> Result<xr::Space>
where
    G: xr::Graphics,
{
    session
        .create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)
        .context("create OpenXR STAGE reference space")
}

fn end_skipped_frame<G>(
    frame_stream: &mut xr::FrameStream<G>,
    predicted_display_time: xr::Time,
    environment_blend_mode: xr::EnvironmentBlendMode,
    frame_stats: &mut XrFrameStats,
) -> Result<()>
where
    G: xr::Graphics,
{
    frame_stats.record_skipped_frame();
    frame_stream
        .end(predicted_display_time, environment_blend_mode, &[])
        .context("end skipped OpenXR frame")
}

fn end_frame_with_layers<G>(
    frame_stream: &mut xr::FrameStream<G>,
    predicted_display_time: xr::Time,
    environment_blend_mode: xr::EnvironmentBlendMode,
    layers: &[&xr::CompositionLayerBase<'_, G>],
) -> Result<()>
where
    G: xr::Graphics,
{
    frame_stream
        .end(predicted_display_time, environment_blend_mode, layers)
        .context("end OpenXR frame with projection layer")
}

pub fn acquire_eye_target<G, E>(eye_state: &mut E) -> Result<XrAcquiredEyeTarget<'_, G, E>>
where
    G: xr::Graphics,
    E: XrEyeSwapchain<G> + ?Sized,
{
    let image_index = eye_state
        .swapchain_mut()
        .acquire_image()
        .context("acquire OpenXR swapchain image")?;
    if let Err(err) = eye_state.swapchain_mut().wait_image(xr::Duration::INFINITE) {
        let _ = eye_state.swapchain_mut().release_image();
        return Err(err).context("wait for OpenXR swapchain image");
    }
    let color_view = {
        let texture = eye_state
            .textures()
            .get(image_index as usize)
            .ok_or_else(|| {
                anyhow::anyhow!("OpenXR returned out-of-range image index {image_index}")
            })?;
        texture.create_view(&Default::default())
    };
    eye_state.record_image_acquired();
    Ok(XrAcquiredEyeTarget {
        eye_state,
        color_view,
        _graphics: PhantomData,
    })
}

pub fn acquire_stereo_target<G, S>(stereo_state: &mut S) -> Result<XrAcquiredStereoTarget<'_, G, S>>
where
    G: xr::Graphics,
    S: XrStereoSwapchain<G> + ?Sized,
{
    if stereo_state.array_size() < 2 {
        anyhow::bail!(
            "OpenXR stereo array swapchain requires at least 2 layers, got {}",
            stereo_state.array_size()
        );
    }
    let image_index = stereo_state
        .swapchain_mut()
        .acquire_image()
        .context("acquire OpenXR stereo array swapchain image")?;
    if let Err(err) = stereo_state
        .swapchain_mut()
        .wait_image(xr::Duration::INFINITE)
    {
        let _ = stereo_state.swapchain_mut().release_image();
        return Err(err).context("wait for OpenXR stereo array swapchain image");
    }
    let (color_array_view, color_left_view, color_right_view) = {
        let texture = stereo_state
            .textures()
            .get(image_index as usize)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "OpenXR returned out-of-range stereo array image index {image_index}"
                )
            })?;
        (
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("mclone_xr_stereo_array_swapchain_view"),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                base_array_layer: 0,
                array_layer_count: Some(stereo_state.array_size()),
                ..Default::default()
            }),
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("mclone_xr_stereo_array_left_view"),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: 0,
                array_layer_count: Some(1),
                ..Default::default()
            }),
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("mclone_xr_stereo_array_right_view"),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: 1,
                array_layer_count: Some(1),
                ..Default::default()
            }),
        )
    };
    stereo_state.record_image_acquired();
    Ok(XrAcquiredStereoTarget {
        stereo_state,
        image_index,
        color_array_view,
        color_left_view,
        color_right_view,
        _graphics: PhantomData,
    })
}

fn end_stereo_projection_frame<G, L, R>(
    frame_stream: &mut xr::FrameStream<G>,
    predicted_display_time: xr::Time,
    environment_blend_mode: xr::EnvironmentBlendMode,
    stage: &xr::Space,
    stereo_views: XrStereoFrameViews,
    left_eye: &L,
    right_eye: &R,
) -> Result<()>
where
    G: xr::Graphics,
    L: XrEyeSwapchain<G> + ?Sized,
    R: XrEyeSwapchain<G> + ?Sized,
{
    let rect = xr::Rect2Di {
        offset: xr::Offset2Di { x: 0, y: 0 },
        extent: xr::Extent2Di {
            width: left_eye.width() as _,
            height: left_eye.height() as _,
        },
    };
    let projection_views = [
        xr::CompositionLayerProjectionView::new()
            .pose(stereo_views.left.pose)
            .fov(stereo_views.left.fov)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(left_eye.swapchain())
                    .image_array_index(0)
                    .image_rect(rect),
            ),
        xr::CompositionLayerProjectionView::new()
            .pose(stereo_views.right.pose)
            .fov(stereo_views.right.fov)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(right_eye.swapchain())
                    .image_array_index(0)
                    .image_rect(rect),
            ),
    ];
    let projection = xr::CompositionLayerProjection::new()
        .space(stage)
        .views(&projection_views);
    let layers: [&xr::CompositionLayerBase<'_, G>; 1] = [&projection];
    end_frame_with_layers(
        frame_stream,
        predicted_display_time,
        environment_blend_mode,
        &layers,
    )
}

fn end_multiview_projection_frame<G, S>(
    frame_stream: &mut xr::FrameStream<G>,
    predicted_display_time: xr::Time,
    environment_blend_mode: xr::EnvironmentBlendMode,
    stage: &xr::Space,
    stereo_views: XrStereoFrameViews,
    stereo_target: &S,
) -> Result<()>
where
    G: xr::Graphics,
    S: XrStereoSwapchain<G> + ?Sized,
{
    if stereo_target.array_size() < 2 {
        anyhow::bail!(
            "OpenXR multiview projection frame requires at least 2 array layers, got {}",
            stereo_target.array_size()
        );
    }
    let rect = xr::Rect2Di {
        offset: xr::Offset2Di { x: 0, y: 0 },
        extent: xr::Extent2Di {
            width: stereo_target.width() as _,
            height: stereo_target.height() as _,
        },
    };
    let projection_views = [
        xr::CompositionLayerProjectionView::new()
            .pose(stereo_views.left.pose)
            .fov(stereo_views.left.fov)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(stereo_target.swapchain())
                    .image_array_index(0)
                    .image_rect(rect),
            ),
        xr::CompositionLayerProjectionView::new()
            .pose(stereo_views.right.pose)
            .fov(stereo_views.right.fov)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(stereo_target.swapchain())
                    .image_array_index(1)
                    .image_rect(rect),
            ),
    ];
    let projection = xr::CompositionLayerProjection::new()
        .space(stage)
        .views(&projection_views);
    let layers: [&xr::CompositionLayerBase<'_, G>; 1] = [&projection];
    end_frame_with_layers(
        frame_stream,
        predicted_display_time,
        environment_blend_mode,
        &layers,
    )
}

pub fn clear_stereo_targets(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    left_target: XrClearTarget<'_>,
    right_target: XrClearTarget<'_>,
) -> Result<()> {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_xr_clear_encoder"),
    });
    clear_eye_target(&mut encoder, left_target, diagnostic_eye_clear_color(0));
    clear_eye_target(&mut encoder, right_target, diagnostic_eye_clear_color(1));
    let submission = queue.submit(Some(encoder.finish()));
    device
        .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
        .map(|_| ())
        .context("wait for OpenXR clear submission")?;
    device
        .poll(wgpu::PollType::Wait)
        .map(|_| ())
        .context("wait for OpenXR clear device idle")
}

pub fn render_multiview_layer_proof(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color_view: &wgpu::TextureView,
    color_format: wgpu::TextureFormat,
) -> Result<()> {
    const SHADER: &str = r#"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) view_index: i32,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(view_index) view_index: i32,
) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VertexOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.view_index = view_index;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    if (in.view_index == 0i) {
        return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    }
    return vec4<f32>(0.0, 1.0, 0.0, 1.0);
}
"#;

    if !device.features().contains(wgpu::Features::MULTIVIEW) {
        anyhow::bail!("wgpu device does not expose MULTIVIEW");
    }

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mclone_xr_multiview_proof_shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mclone_xr_multiview_proof_pipeline_layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mclone_xr_multiview_proof_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: NonZeroU32::new(2),
        cache: None,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_xr_multiview_proof_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_xr_multiview_proof_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.02,
                        g: 0.04,
                        b: 0.08,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
    }
    let submission = queue.submit(Some(encoder.finish()));
    device
        .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
        .map(|_| ())
        .context("wait for OpenXR multiview proof submission")?;
    device
        .poll(wgpu::PollType::Wait)
        .map(|_| ())
        .context("wait for OpenXR multiview proof device idle")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XrMultiviewLayerProof {
    pub width: u32,
    pub height: u32,
    pub left_red_pixels: u32,
    pub right_green_pixels: u32,
    pub minimum_expected_pixels: u32,
    pub left_first_pixel: [u8; 4],
    pub right_first_pixel: [u8; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XrMultiviewLayerDifferenceProof {
    pub width: u32,
    pub height: u32,
    pub different_pixels: u32,
    pub minimum_expected_different_pixels: u32,
    pub left_first_pixel: [u8; 4],
    pub right_first_pixel: [u8; 4],
}

pub fn render_private_multiview_readback_proof(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color_format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> Result<XrMultiviewLayerProof> {
    if !device.features().contains(wgpu::Features::MULTIVIEW) {
        anyhow::bail!("wgpu device does not expose MULTIVIEW");
    }
    if !matches!(
        color_format,
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb
    ) {
        anyhow::bail!("OpenXR multiview readback proof expects RGBA8 color, got {color_format:?}");
    }
    if width == 0 || height == 0 {
        anyhow::bail!("OpenXR multiview readback proof requires a non-empty target");
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mclone_xr_private_multiview_readback_target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 2,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: color_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("mclone_xr_private_multiview_readback_view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        base_array_layer: 0,
        array_layer_count: Some(2),
        ..Default::default()
    });

    render_multiview_clear_only_proof(device, queue, &view)
        .context("render private OpenXR multiview clear-only proof")?;
    read_multiview_clear_color_proof(device, queue, &texture, width, height, "private")
        .context("validate private OpenXR multiview clear-only proof")?;
    render_multiview_layer_proof(device, queue, &view, color_format)
        .context("render private OpenXR multiview readback proof")?;
    read_multiview_layer_color_proof(device, queue, &texture, width, height, "private")
}

fn render_multiview_clear_only_proof(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color_view: &wgpu::TextureView,
) -> Result<()> {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_xr_multiview_clear_only_encoder"),
    });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_xr_multiview_clear_only_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.02,
                        g: 0.04,
                        b: 0.08,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    }
    let submission = queue.submit(Some(encoder.finish()));
    device
        .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
        .map(|_| ())
        .context("wait for OpenXR multiview clear-only submission")?;
    device
        .poll(wgpu::PollType::Wait)
        .map(|_| ())
        .context("wait for OpenXR multiview clear-only device idle")
}

fn read_multiview_clear_color_proof(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    label: &'static str,
) -> Result<()> {
    let left_pixels = read_rgba8_layer(device, queue, texture, width, height, 0)?;
    let right_pixels = read_rgba8_layer(device, queue, texture, width, height, 1)?;
    let left_clear_pixels = pixel_count_matching(&left_pixels, is_clear_pixel);
    let right_clear_pixels = pixel_count_matching(&right_pixels, is_clear_pixel);
    let minimum_expected_pixels = ((width as u64 * height as u64) * 9 / 10) as u32;
    let left_first_pixel = first_pixel_rgba(&left_pixels);
    let right_first_pixel = first_pixel_rgba(&right_pixels);
    log::info!(
        "OpenXR multiview {label} clear-only readback: left_clear_pixels={} right_clear_pixels={} minimum_expected_pixels={} left_first={:?} right_first={:?}",
        left_clear_pixels,
        right_clear_pixels,
        minimum_expected_pixels,
        left_first_pixel,
        right_first_pixel
    );
    if left_clear_pixels < minimum_expected_pixels || right_clear_pixels < minimum_expected_pixels {
        anyhow::bail!(
            "OpenXR multiview {label} clear-only readback failed: left_clear_pixels={} right_clear_pixels={} minimum_expected_pixels={} left_first={:?} right_first={:?}",
            left_clear_pixels,
            right_clear_pixels,
            minimum_expected_pixels,
            left_first_pixel,
            right_first_pixel
        );
    }
    Ok(())
}

pub fn read_multiview_layer_color_proof(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    label: &'static str,
) -> Result<XrMultiviewLayerProof> {
    if width == 0 || height == 0 {
        anyhow::bail!("OpenXR multiview readback proof requires a non-empty target");
    }
    let left_pixels = read_rgba8_layer(device, queue, texture, width, height, 0)?;
    let right_pixels = read_rgba8_layer(device, queue, texture, width, height, 1)?;
    let left_red_pixels = pixel_count_matching(&left_pixels, is_red_layer_pixel);
    let right_green_pixels = pixel_count_matching(&right_pixels, is_green_layer_pixel);
    let minimum_expected_pixels = ((width as u64 * height as u64) * 9 / 10) as u32;
    let left_first_pixel = first_pixel_rgba(&left_pixels);
    let right_first_pixel = first_pixel_rgba(&right_pixels);
    log::info!(
        "OpenXR multiview {label} readback: left_red_pixels={} right_green_pixels={} minimum_expected_pixels={} left_first={:?} right_first={:?}",
        left_red_pixels,
        right_green_pixels,
        minimum_expected_pixels,
        left_first_pixel,
        right_first_pixel
    );
    if left_red_pixels < minimum_expected_pixels || right_green_pixels < minimum_expected_pixels {
        anyhow::bail!(
            "OpenXR multiview {label} readback failed: left_red_pixels={} right_green_pixels={} minimum_expected_pixels={} left_first={:?} right_first={:?}",
            left_red_pixels,
            right_green_pixels,
            minimum_expected_pixels,
            left_first_pixel,
            right_first_pixel
        );
    }
    Ok(XrMultiviewLayerProof {
        width,
        height,
        left_red_pixels,
        right_green_pixels,
        minimum_expected_pixels,
        left_first_pixel,
        right_first_pixel,
    })
}

pub fn read_multiview_layer_difference_proof(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    label: &'static str,
) -> Result<XrMultiviewLayerDifferenceProof> {
    if width == 0 || height == 0 {
        anyhow::bail!("OpenXR multiview layer difference proof requires a non-empty target");
    }
    let left_pixels = read_rgba8_layer(device, queue, texture, width, height, 0)?;
    let right_pixels = read_rgba8_layer(device, queue, texture, width, height, 1)?;
    let different_pixels = left_pixels
        .chunks_exact(4)
        .zip(right_pixels.chunks_exact(4))
        .filter(|(left, right)| left != right)
        .count() as u32;
    let pixel_count = width.saturating_mul(height);
    let minimum_expected_different_pixels = (pixel_count / 1000).max(64).min(pixel_count);
    let left_first_pixel = first_pixel_rgba(&left_pixels);
    let right_first_pixel = first_pixel_rgba(&right_pixels);
    log::info!(
        "OpenXR multiview {label} layer difference readback: different_pixels={} minimum_expected_different_pixels={} left_first={:?} right_first={:?}",
        different_pixels,
        minimum_expected_different_pixels,
        left_first_pixel,
        right_first_pixel
    );
    if different_pixels < minimum_expected_different_pixels {
        anyhow::bail!(
            "OpenXR multiview {label} layer difference readback failed: different_pixels={} minimum_expected_different_pixels={} left_first={:?} right_first={:?}",
            different_pixels,
            minimum_expected_different_pixels,
            left_first_pixel,
            right_first_pixel
        );
    }
    Ok(XrMultiviewLayerDifferenceProof {
        width,
        height,
        different_pixels,
        minimum_expected_different_pixels,
        left_first_pixel,
        right_first_pixel,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XrProjectionMarkerSample {
    pub expected_px: [f32; 2],
    pub actual_px: [f32; 2],
    pub error_px: f32,
    pub pixel_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XrStereoProjectionProof {
    pub left: XrProjectionMarkerSample,
    pub right: XrProjectionMarkerSample,
    pub expected_disparity_px: f32,
    pub tolerance_px: f32,
}

pub fn render_stereo_projection_readback_proof(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color_format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    stereo_views: XrStereoFrameViews,
) -> Result<XrStereoProjectionProof> {
    const SHADER: &str = r#"
struct ProjectionProbe {
    view_projection: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> probe: ProjectionProbe;

struct VertexIn {
    @location(0) position: vec3<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.position = probe.view_projection * vec4<f32>(input.position, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
"#;

    if width == 0 || height == 0 {
        anyhow::bail!("OpenXR stereo projection proof requires a non-empty target");
    }
    let proof_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 2,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: color_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let left_layer_view = proof_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_left_view"),
        dimension: Some(wgpu::TextureViewDimension::D2),
        base_array_layer: 0,
        array_layer_count: Some(1),
        ..Default::default()
    });
    let right_layer_view = proof_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_right_view"),
        dimension: Some(wgpu::TextureViewDimension::D2),
        base_array_layer: 1,
        array_layer_count: Some(1),
        ..Default::default()
    });

    let left_view = render_view_from_world_pose(
        view_pose(&stereo_views.left)?,
        xr_fov_from_openxr(stereo_views.left.fov),
        0.05,
        100.0,
    )?;
    let right_view = render_view_from_world_pose(
        view_pose(&stereo_views.right)?,
        xr_fov_from_openxr(stereo_views.right.fov),
        0.05,
        100.0,
    )?;
    let marker = stereo_projection_marker(left_view, right_view)?;
    let expected_left = project_to_pixel(left_view.view_projection, marker.center, width, height)
        .context("left projection marker is outside the left eye view")?;
    let expected_right = project_to_pixel(right_view.view_projection, marker.center, width, height)
        .context("right projection marker is outside the right eye view")?;
    let expected_disparity_px = distance2(expected_left, expected_right);
    let tolerance_px = (width.max(height) as f32 * 0.015).max(8.0);
    if expected_disparity_px <= tolerance_px * 1.5 {
        anyhow::bail!(
            "OpenXR stereo projection proof fixture produced insufficient eye separation: expected_disparity_px={expected_disparity_px:.2} tolerance_px={tolerance_px:.2}"
        );
    }
    log::info!(
        "OpenXR stereo projection proof fixture: left_expected=({:.1},{:.1}) right_expected=({:.1},{:.1}) expected_disparity_px={:.1} tolerance_px={:.1}",
        expected_left[0],
        expected_left[1],
        expected_right[0],
        expected_right[1],
        expected_disparity_px,
        tolerance_px
    );

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let left_uniform_bytes = projection_probe_uniform_bytes(left_view.view_projection);
    let left_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_left_uniform"),
        size: left_uniform_bytes.len() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&left_uniform_buffer, 0, &left_uniform_bytes);
    let left_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_left_bind_group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: left_uniform_buffer.as_entire_binding(),
        }],
    });
    let right_uniform_bytes = projection_probe_uniform_bytes(right_view.view_projection);
    let right_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_right_uniform"),
        size: right_uniform_bytes.len() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&right_uniform_buffer, 0, &right_uniform_bytes);
    let right_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_right_bind_group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: right_uniform_buffer.as_entire_binding(),
        }],
    });
    let vertex_bytes = projection_marker_vertex_bytes(marker);
    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_vertices"),
        size: vertex_bytes.len() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&vertex_buffer, 0, &vertex_bytes);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_pipeline_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 12,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                }],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_xr_stereo_projection_probe_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_xr_stereo_projection_probe_left_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &left_layer_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.02,
                        g: 0.04,
                        b: 0.08,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &left_bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..6, 0..1);
    }
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mclone_xr_stereo_projection_probe_right_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &right_layer_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.02,
                        g: 0.04,
                        b: 0.08,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &right_bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..6, 0..1);
    }
    let submission = queue.submit(Some(encoder.finish()));
    device
        .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
        .map(|_| ())
        .context("wait for OpenXR stereo projection proof submission")?;
    device
        .poll(wgpu::PollType::Wait)
        .map(|_| ())
        .context("wait for OpenXR stereo projection proof device idle")?;

    let left_pixels = read_rgba8_layer(device, queue, &proof_texture, width, height, 0)?;
    let right_pixels = read_rgba8_layer(device, queue, &proof_texture, width, height, 1)?;
    let left_pixel_count = marker_pixel_count(&left_pixels);
    let right_pixel_count = marker_pixel_count(&right_pixels);
    let left_first = left_pixels.get(0..4).unwrap_or(&[]);
    let right_first = right_pixels.get(0..4).unwrap_or(&[]);
    log::info!(
        "OpenXR stereo projection proof readback: left_marker_pixels={} right_marker_pixels={} left_first={:?} right_first={:?}",
        left_pixel_count,
        right_pixel_count,
        left_first,
        right_first
    );
    let left_actual =
        find_marker_center(&left_pixels, width, height).context("left marker was not rendered")?;
    let right_actual = find_marker_center(&right_pixels, width, height)
        .context("right marker was not rendered")?;
    let left_error_px = distance2(left_actual, expected_left);
    let right_error_px = distance2(right_actual, expected_right);
    if left_error_px > tolerance_px || right_error_px > tolerance_px {
        anyhow::bail!(
            "OpenXR stereo projection proof failed: left_actual=({:.1},{:.1}) left_expected=({:.1},{:.1}) left_error={:.1}px right_actual=({:.1},{:.1}) right_expected=({:.1},{:.1}) right_error={:.1}px tolerance={:.1}px",
            left_actual[0],
            left_actual[1],
            expected_left[0],
            expected_left[1],
            left_error_px,
            right_actual[0],
            right_actual[1],
            expected_right[0],
            expected_right[1],
            right_error_px,
            tolerance_px
        );
    }

    Ok(XrStereoProjectionProof {
        left: XrProjectionMarkerSample {
            expected_px: expected_left,
            actual_px: left_actual,
            error_px: left_error_px,
            pixel_count: left_pixel_count,
        },
        right: XrProjectionMarkerSample {
            expected_px: expected_right,
            actual_px: right_actual,
            error_px: right_error_px,
            pixel_count: right_pixel_count,
        },
        expected_disparity_px,
        tolerance_px,
    })
}

#[derive(Clone, Copy, Debug)]
struct ProjectionMarker {
    center: Vec3,
    right_extent: Vec3,
    up_extent: Vec3,
}

fn stereo_projection_marker(
    left_view: XrRenderView,
    right_view: XrRenderView,
) -> Result<ProjectionMarker> {
    let head_center = (left_view.camera_position + right_view.camera_position) * 0.5;
    let forward = (left_view.camera_forward + right_view.camera_forward).normalize_or_zero();
    let right = (left_view.camera_right + right_view.camera_right).normalize_or_zero();
    let up = (left_view.camera_up + right_view.camera_up).normalize_or_zero();
    if forward.length_squared() < 1.0e-6
        || right.length_squared() < 1.0e-6
        || up.length_squared() < 1.0e-6
    {
        anyhow::bail!("OpenXR stereo projection proof received degenerate eye poses");
    }
    Ok(ProjectionMarker {
        center: head_center + forward * 1.5 + right * 0.18 + up * 0.03,
        right_extent: right * 0.035,
        up_extent: up * 0.035,
    })
}

fn projection_probe_uniform_bytes(view_projection: Mat4) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64);
    push_mat4_bytes(&mut bytes, view_projection);
    bytes
}

fn projection_marker_vertex_bytes(marker: ProjectionMarker) -> Vec<u8> {
    let offsets = [
        [-1.0_f32, -1.0_f32],
        [1.0, -1.0],
        [-1.0, 1.0],
        [-1.0, 1.0],
        [1.0, -1.0],
        [1.0, 1.0],
    ];
    let mut bytes = Vec::with_capacity(offsets.len() * 12);
    for [right_scale, up_scale] in offsets {
        let position =
            marker.center + marker.right_extent * right_scale + marker.up_extent * up_scale;
        for value in position.to_array() {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

fn push_mat4_bytes(bytes: &mut Vec<u8>, matrix: Mat4) {
    for value in matrix.to_cols_array() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn project_to_pixel(
    view_projection: Mat4,
    world_position: Vec3,
    width: u32,
    height: u32,
) -> Option<[f32; 2]> {
    let clip = view_projection * world_position.extend(1.0);
    if !clip.is_finite() || clip.w.abs() <= f32::EPSILON {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !ndc.is_finite() || ndc.z < -1.0 || ndc.z > 1.0 {
        return None;
    }
    Some([
        (ndc.x * 0.5 + 0.5) * width as f32,
        (0.5 - ndc.y * 0.5) * height as f32,
    ])
}

fn find_marker_center(pixels: &[u8], width: u32, height: u32) -> Option<[f32; 2]> {
    let mut count = 0_u32;
    let mut sum_x = 0.0_f32;
    let mut sum_y = 0.0_f32;
    for y in 0..height {
        for x in 0..width {
            let index = ((y * width + x) * RGBA8_BYTES_PER_PIXEL) as usize;
            let pixel = pixels.get(index..index + 4)?;
            if is_marker_pixel(pixel) {
                count += 1;
                sum_x += x as f32 + 0.5;
                sum_y += y as f32 + 0.5;
            }
        }
    }
    (count > 0).then_some([sum_x / count as f32, sum_y / count as f32])
}

fn marker_pixel_count(pixels: &[u8]) -> u32 {
    pixels
        .chunks_exact(RGBA8_BYTES_PER_PIXEL as usize)
        .filter(|pixel| is_marker_pixel(pixel))
        .count() as u32
}

fn pixel_count_matching(pixels: &[u8], predicate: fn(&[u8]) -> bool) -> u32 {
    pixels
        .chunks_exact(RGBA8_BYTES_PER_PIXEL as usize)
        .filter(|pixel| predicate(pixel))
        .count() as u32
}

fn is_marker_pixel(pixel: &[u8]) -> bool {
    pixel[0] >= 200 && pixel[1] >= 200 && pixel[2] >= 200
}

fn is_clear_pixel(pixel: &[u8]) -> bool {
    (3..=7).contains(&pixel[0])
        && (8..=12).contains(&pixel[1])
        && (18..=24).contains(&pixel[2])
        && pixel[3] >= 200
}

fn is_red_layer_pixel(pixel: &[u8]) -> bool {
    pixel[0] >= 200 && pixel[1] <= 40 && pixel[2] <= 40 && pixel[3] >= 200
}

fn is_green_layer_pixel(pixel: &[u8]) -> bool {
    pixel[0] <= 40 && pixel[1] >= 200 && pixel[2] <= 40 && pixel[3] >= 200
}

fn first_pixel_rgba(pixels: &[u8]) -> [u8; 4] {
    pixels
        .get(0..4)
        .and_then(|pixel| pixel.try_into().ok())
        .unwrap_or([0, 0, 0, 0])
}

fn read_rgba8_layer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    layer: u32,
) -> Result<Vec<u8>> {
    let unpadded_row_bytes = width * RGBA8_BYTES_PER_PIXEL;
    let padded_row_bytes = padded_row_bytes(unpadded_row_bytes);
    let buffer_size = padded_row_bytes as u64 * height as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mclone_xr_stereo_projection_readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mclone_xr_stereo_projection_readback_encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
            aspect: Default::default(),
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::Wait)
        .context("wait for OpenXR stereo projection readback")?;
    receiver
        .recv()
        .context("failed to receive OpenXR stereo projection readback map result")?
        .context("failed to map OpenXR stereo projection readback buffer")?;

    let mapped = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((width * height * RGBA8_BYTES_PER_PIXEL) as usize);
    for row in 0..height {
        let start = (row * padded_row_bytes) as usize;
        let end = start + unpadded_row_bytes as usize;
        pixels.extend_from_slice(&mapped[start..end]);
    }
    drop(mapped);
    staging.unmap();
    Ok(pixels)
}

fn padded_row_bytes(unpadded_row_bytes: u32) -> u32 {
    unpadded_row_bytes.div_ceil(COPY_BYTES_PER_ROW_ALIGNMENT) * COPY_BYTES_PER_ROW_ALIGNMENT
}

fn distance2(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (dx * dx + dy * dy).sqrt()
}

pub fn diagnostic_eye_clear_color(eye: usize) -> wgpu::Color {
    match eye {
        0 => wgpu::Color {
            r: 0.04,
            g: 0.10,
            b: 0.35,
            a: 1.0,
        },
        _ => wgpu::Color {
            r: 0.05,
            g: 0.28,
            b: 0.12,
            a: 1.0,
        },
    }
}

fn clear_eye_target(
    encoder: &mut wgpu::CommandEncoder,
    target: XrClearTarget<'_>,
    color: wgpu::Color,
) {
    let color_attachments = [Some(wgpu::RenderPassColorAttachment {
        view: target.color_view,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(color),
            store: wgpu::StoreOp::Store,
        },
    })];
    let depth_stencil_attachment =
        target
            .depth_view
            .map(|view| wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations {
                    // Reversed-Z: far plane is 0.0 (see tactical 158).
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            });
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("mclone_xr_clear_pass"),
        color_attachments: &color_attachments,
        depth_stencil_attachment,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
}

pub fn locate_stereo_views<G>(
    session: &xr::Session<G>,
    stage: &xr::Space,
    predicted_display_time: xr::Time,
) -> Result<XrStereoFrameViews>
where
    G: xr::Graphics,
{
    let (_, views) = session
        .locate_views(PRIMARY_STEREO_VIEW_TYPE, predicted_display_time, stage)
        .context("locate OpenXR stereo views")?;
    if views.len() < 2 {
        anyhow::bail!("OpenXR runtime returned fewer than two stereo views");
    }
    Ok(XrStereoFrameViews {
        left: views[0],
        right: views[1],
    })
}

pub fn view_pose(view: &xr::View) -> Result<XrViewPose> {
    let position = Vec3::new(
        view.pose.position.x,
        view.pose.position.y,
        view.pose.position.z,
    );
    let orientation = Quat::from_xyzw(
        view.pose.orientation.x,
        view.pose.orientation.y,
        view.pose.orientation.z,
        view.pose.orientation.w,
    );
    if !position.is_finite() || !finite_quat(orientation) || orientation.length_squared() < 0.5 {
        anyhow::bail!("OpenXR returned an invalid view pose");
    }
    Ok(XrViewPose {
        position,
        orientation: orientation.normalize(),
    })
}

fn finite_quat(value: Quat) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite() && value.w.is_finite()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_refresh_rates_are_sorted_filtered_and_deduped() {
        let rates = sorted_display_refresh_rates(vec![120.0, f32::NAN, 72.0, -1.0, 89.98, 90.0]);

        assert_eq!(rates, vec![72.0, 89.98, 120.0]);
        assert!(refresh_rates_match(89.98, 90.0));
        assert!(!refresh_rates_match(89.9, 90.0));
        assert_eq!(
            display_refresh_rates_label(&rates),
            "72.0 Hz, 90.0 Hz, 120.0 Hz"
        );
        assert_eq!(display_refresh_rates_label(&[]), "none reported");
    }
}
