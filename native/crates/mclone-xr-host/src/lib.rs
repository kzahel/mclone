#![deny(unsafe_code)]

mod actions;

use anyhow::{Context, Result};
use glam::{Mat4, Quat, Vec3, Vec4};
use openxr as xr;
use std::marker::PhantomData;
use std::num::NonZeroU32;

pub use actions::{OpenXrControllerActions, XrControllerSnapshot, XrHand};

pub const PRIMARY_STEREO_VIEW_TYPE: xr::ViewConfigurationType =
    xr::ViewConfigurationType::PRIMARY_STEREO;
pub const XR_REFRESH_RATE_MATCH_EPSILON_HZ: f32 = 0.05;

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

#[derive(Clone, Copy, Debug)]
pub struct XrViewPose {
    pub position: Vec3,
    pub orientation: Quat,
}

#[derive(Clone, Copy, Debug)]
pub struct XrRenderView {
    pub view: Mat4,
    pub projection: Mat4,
    pub view_projection: Mat4,
    pub camera_position: Vec3,
    pub camera_forward: Vec3,
    pub camera_right: Vec3,
    pub camera_up: Vec3,
    pub aspect: f32,
    pub fov_y_radians: f32,
    pub z_near: f32,
    pub z_far: f32,
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
            .context("release OpenXR swapchain image")
    }
}

pub struct XrAcquiredStereoTarget<'a, G, S>
where
    G: xr::Graphics,
    S: XrStereoSwapchain<G> + ?Sized,
{
    stereo_state: &'a mut S,
    color_array_view: wgpu::TextureView,
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

    pub fn release(self) -> Result<()> {
        self.stereo_state
            .swapchain_mut()
            .release_image()
            .context("release OpenXR stereo array swapchain image")
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

pub fn poll_openxr_events<G, F>(
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

pub fn wait_begin_frame<G>(
    frame_wait: &mut xr::FrameWaiter,
    frame_stream: &mut xr::FrameStream<G>,
    frame_stats: &mut XrFrameStats,
) -> Result<xr::FrameState>
where
    G: xr::Graphics,
{
    let frame_state = frame_wait.wait().context("wait OpenXR frame")?;
    frame_stream.begin().context("begin OpenXR frame")?;
    frame_stats.record_runtime_frame();
    Ok(frame_state)
}

pub fn end_skipped_frame<G>(
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

pub fn end_frame_with_layers<G>(
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
    let color_array_view = {
        let texture = stereo_state
            .textures()
            .get(image_index as usize)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "OpenXR returned out-of-range stereo array image index {image_index}"
                )
            })?;
        texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("mclone_xr_stereo_array_swapchain_view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_array_layer: 0,
            array_layer_count: Some(stereo_state.array_size()),
            ..Default::default()
        })
    };
    Ok(XrAcquiredStereoTarget {
        stereo_state,
        color_array_view,
        _graphics: PhantomData,
    })
}

pub fn end_stereo_projection_frame<G, L, R>(
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

pub fn end_multiview_projection_frame<G, S>(
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
    @location(0) @interpolate(flat) view_index: u32,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(view_index) view_index: u32,
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
    if (in.view_index == 0u) {
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
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
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
                    load: wgpu::LoadOp::Clear(1.0),
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

pub fn render_view_from_world_pose(
    pose: XrViewPose,
    fov: xr::Fovf,
    near: f32,
    far: f32,
) -> Result<XrRenderView> {
    if !pose.position.is_finite()
        || !finite_quat(pose.orientation)
        || pose.orientation.length_squared() < 0.5
    {
        anyhow::bail!("OpenXR render view received an invalid world pose");
    }
    let orientation = pose.orientation.normalize();
    let camera_forward = (orientation * Vec3::NEG_Z).normalize_or_zero();
    let camera_right = (orientation * Vec3::X).normalize_or_zero();
    let camera_up = (orientation * Vec3::Y).normalize_or_zero();
    if camera_forward.length_squared() < 1.0e-6 || camera_up.length_squared() < 1.0e-6 {
        anyhow::bail!("OpenXR returned an invalid view orientation");
    }
    let view = Mat4::from_rotation_translation(orientation, pose.position).inverse();
    let projection = xr_fov_to_projection_rh(fov, near, far)?;
    Ok(XrRenderView {
        view,
        projection,
        view_projection: projection * view,
        camera_position: pose.position,
        camera_forward,
        camera_right,
        camera_up,
        aspect: xr_fov_aspect(fov),
        fov_y_radians: (fov.angle_up - fov.angle_down).abs(),
        z_near: near,
        z_far: far,
    })
}

pub fn xr_fov_to_projection_rh(fov: xr::Fovf, near: f32, far: f32) -> Result<Mat4> {
    if !near.is_finite() || !far.is_finite() || near <= 0.0 || far <= near {
        anyhow::bail!("invalid XR projection clipping planes near={near} far={far}");
    }
    let tan_left = fov.angle_left.tan();
    let tan_right = fov.angle_right.tan();
    let tan_up = fov.angle_up.tan();
    let tan_down = fov.angle_down.tan();
    let tan_width = tan_right - tan_left;
    let tan_height = tan_up - tan_down;
    if !tan_width.is_finite()
        || !tan_height.is_finite()
        || tan_width.abs() <= f32::EPSILON
        || tan_height.abs() <= f32::EPSILON
    {
        anyhow::bail!("invalid OpenXR FOV {:?}", fov);
    }
    Ok(Mat4::from_cols(
        Vec4::new(2.0 / tan_width, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 2.0 / tan_height, 0.0, 0.0),
        Vec4::new(
            (tan_right + tan_left) / tan_width,
            (tan_up + tan_down) / tan_height,
            -far / (far - near),
            -1.0,
        ),
        Vec4::new(0.0, 0.0, -(near * far) / (far - near), 0.0),
    ))
}

pub fn xr_fov_aspect(fov: xr::Fovf) -> f32 {
    let width = fov.angle_right.tan() - fov.angle_left.tan();
    let height = fov.angle_up.tan() - fov.angle_down.tan();
    if width.is_finite() && height.is_finite() && height.abs() > f32::EPSILON {
        (width / height).abs().max(0.01)
    } else {
        1.0
    }
}

fn finite_quat(value: Quat) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite() && value.w.is_finite()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_mat4_close(actual: Mat4, expected: Mat4) {
        for (actual, expected) in actual
            .to_cols_array()
            .into_iter()
            .zip(expected.to_cols_array())
        {
            assert!(
                (actual - expected).abs() < 1.0e-5,
                "matrix mismatch: actual={actual} expected={expected}"
            );
        }
    }

    #[test]
    fn symmetric_xr_fov_matches_chunk_projection_convention() {
        let fov_y = 58.0_f32.to_radians();
        let aspect = 1.25;
        let near = 0.05;
        let far = 700.0;
        let tan_y = (fov_y * 0.5).tan();
        let tan_x = tan_y * aspect;
        let fov = xr::Fovf {
            angle_left: -tan_x.atan(),
            angle_right: tan_x.atan(),
            angle_up: tan_y.atan(),
            angle_down: -tan_y.atan(),
        };

        let actual = xr_fov_to_projection_rh(fov, near, far).unwrap();
        let expected = Mat4::perspective_rh(fov_y, aspect, near, far);

        assert_mat4_close(actual, expected);
    }

    #[test]
    fn render_view_from_world_pose_builds_camera_vectors() {
        let fov = xr::Fovf {
            angle_left: -0.5,
            angle_right: 0.5,
            angle_up: 0.5,
            angle_down: -0.5,
        };
        let pose = XrViewPose {
            position: Vec3::new(4.0, 5.0, 6.0),
            orientation: Quat::IDENTITY,
        };

        let view = render_view_from_world_pose(pose, fov, 0.05, 700.0).unwrap();

        assert!((view.camera_position - pose.position).length() < 1.0e-6);
        assert!((view.camera_forward - Vec3::NEG_Z).length() < 1.0e-6);
        assert!((view.camera_right - Vec3::X).length() < 1.0e-6);
        assert!((view.camera_up - Vec3::Y).length() < 1.0e-6);
        assert_mat4_close(view.view_projection, view.projection * view.view);
    }

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
