#![deny(unsafe_code)]

use anyhow::{Context, Result};
use openxr as xr;

pub const PRIMARY_STEREO_VIEW_TYPE: xr::ViewConfigurationType =
    xr::ViewConfigurationType::PRIMARY_STEREO;

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
