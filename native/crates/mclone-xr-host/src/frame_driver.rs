use std::fmt;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use openxr as xr;

use crate::{
    OpenXrHostEvent, OpenXrPollStatus, XrEyeSwapchain, XrFrameStats, XrStereoFrameViews,
    XrStereoSwapchain, end_frame_with_layers, end_multiview_projection_frame, end_skipped_frame,
    end_stereo_projection_frame, poll_openxr_events,
};

pub const SESSION_IDLE_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenXrFrameLoopControl {
    Continue,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenXrRunExit {
    FrameLimit,
    Platform,
    Runtime,
    Handler,
}

#[derive(Clone, Copy, Debug)]
pub struct OpenXrFrameLoopPolicy {
    pub view_type: xr::ViewConfigurationType,
    pub environment_blend_mode: xr::EnvironmentBlendMode,
    pub frame_limit: Option<u64>,
    pub session_ready_timeout: Option<Duration>,
    pub submitted_frame_progress_timeout: Option<Duration>,
    pub runtime_exit_is_error: bool,
    pub idle_poll_interval: Duration,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OpenXrFrameTiming {
    pub frame_wall: Duration,
    pub wait_frame: Duration,
    pub begin_frame: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenXrFrameDisposition {
    Submitted,
    Skipped,
}

#[derive(Debug)]
pub struct OpenXrFrameOutcome<T> {
    pub session_state: Option<xr::SessionState>,
    pub predicted_display_time: xr::Time,
    pub should_render: bool,
    pub disposition: OpenXrFrameDisposition,
    pub timing: OpenXrFrameTiming,
    pub stats: XrFrameStats,
    pub render_output: Option<T>,
}

#[derive(Clone, Copy, Debug)]
pub struct OpenXrRunOutcome {
    pub exit: OpenXrRunExit,
    pub session_state: Option<xr::SessionState>,
    pub stats: XrFrameStats,
}

pub trait OpenXrFrameLoopHandler<G>
where
    G: xr::Graphics,
{
    type RenderOutput;

    /// Pump platform events. `timeout` is zero during the active loop and the
    /// shared idle interval while waiting for the OpenXR session to reach READY.
    fn pump_platform_events(&mut self, timeout: Duration) -> Result<OpenXrFrameLoopControl>;

    fn on_openxr_event(&mut self, _event: OpenXrHostEvent) {}

    /// Runs after a live OpenXR session was observed and before `xrWaitFrame`.
    fn before_frame(&mut self, _stats: XrFrameStats) -> Result<()> {
        Ok(())
    }

    /// Runs after `xrWaitFrame` and immediately before `xrBeginFrame`.
    fn after_wait_frame(&mut self) -> Result<()> {
        Ok(())
    }

    fn render_frame(&mut self, frame: &mut OpenXrRenderFrame<'_, G>) -> Result<Self::RenderOutput>;

    fn after_frame(
        &mut self,
        _outcome: OpenXrFrameOutcome<Self::RenderOutput>,
    ) -> Result<OpenXrFrameLoopControl> {
        Ok(OpenXrFrameLoopControl::Continue)
    }
}

/// The only object allowed to end an active OpenXR frame.
///
/// Render callbacks acquire/render/release their concrete swapchain targets,
/// then select one of these submission methods. The driver verifies exactly one
/// submission and closes the frame with no layers on callback failure.
pub struct OpenXrRenderFrame<'a, G>
where
    G: xr::Graphics,
{
    session: &'a xr::Session<G>,
    frame_stream: &'a mut xr::FrameStream<G>,
    predicted_display_time: xr::Time,
    environment_blend_mode: xr::EnvironmentBlendMode,
    ended: bool,
}

impl<'a, G> OpenXrRenderFrame<'a, G>
where
    G: xr::Graphics,
{
    pub fn session(&self) -> &xr::Session<G> {
        self.session
    }

    pub const fn predicted_display_time(&self) -> xr::Time {
        self.predicted_display_time
    }

    pub fn submit_empty(&mut self) -> Result<()> {
        self.ensure_not_ended()?;
        end_frame_with_layers(
            self.frame_stream,
            self.predicted_display_time,
            self.environment_blend_mode,
            &[],
        )?;
        self.ended = true;
        Ok(())
    }

    pub fn submit_stereo_projection<L, R>(
        &mut self,
        stage: &xr::Space,
        stereo_views: XrStereoFrameViews,
        left_eye: &L,
        right_eye: &R,
    ) -> Result<()>
    where
        L: XrEyeSwapchain<G> + ?Sized,
        R: XrEyeSwapchain<G> + ?Sized,
    {
        self.ensure_not_ended()?;
        end_stereo_projection_frame(
            self.frame_stream,
            self.predicted_display_time,
            self.environment_blend_mode,
            stage,
            stereo_views,
            left_eye,
            right_eye,
        )?;
        self.ended = true;
        Ok(())
    }

    pub fn submit_multiview_projection<S>(
        &mut self,
        stage: &xr::Space,
        stereo_views: XrStereoFrameViews,
        stereo_target: &S,
    ) -> Result<()>
    where
        S: XrStereoSwapchain<G> + ?Sized,
    {
        self.ensure_not_ended()?;
        end_multiview_projection_frame(
            self.frame_stream,
            self.predicted_display_time,
            self.environment_blend_mode,
            stage,
            stereo_views,
            stereo_target,
        )?;
        self.ended = true;
        Ok(())
    }

    /// Submit two physical-eye projection views backed by layers zero and one
    /// of one stereo-array swapchain.
    ///
    /// The compositor submission is identical for array-per-eye and
    /// array-multiview rendering; only the render-pass encoding differs.
    pub fn submit_stereo_array_projection<S>(
        &mut self,
        stage: &xr::Space,
        stereo_views: XrStereoFrameViews,
        stereo_target: &S,
    ) -> Result<()>
    where
        S: XrStereoSwapchain<G> + ?Sized,
    {
        self.submit_multiview_projection(stage, stereo_views, stereo_target)
    }

    fn ensure_not_ended(&self) -> Result<()> {
        if self.ended {
            bail!("OpenXR frame was submitted more than once");
        }
        Ok(())
    }

    fn finish_after_error(&mut self) {
        if !self.ended {
            let _ = self.submit_empty();
        }
    }
}

pub struct OpenXrFrameDriver<'a, G>
where
    G: xr::Graphics,
{
    session: &'a xr::Session<G>,
    frame_wait: &'a mut xr::FrameWaiter,
    frame_stream: &'a mut xr::FrameStream<G>,
    policy: OpenXrFrameLoopPolicy,
    event_storage: xr::EventDataBuffer,
    session_running: bool,
    session_state: Option<xr::SessionState>,
    stats: XrFrameStats,
}

impl<'a, G> OpenXrFrameDriver<'a, G>
where
    G: xr::Graphics,
{
    pub fn new(
        session: &'a xr::Session<G>,
        frame_wait: &'a mut xr::FrameWaiter,
        frame_stream: &'a mut xr::FrameStream<G>,
        policy: OpenXrFrameLoopPolicy,
    ) -> Self {
        Self {
            session,
            frame_wait,
            frame_stream,
            policy,
            event_storage: xr::EventDataBuffer::new(),
            session_running: false,
            session_state: None,
            stats: XrFrameStats::default(),
        }
    }

    pub const fn stats(&self) -> XrFrameStats {
        self.stats
    }

    pub const fn session_running(&self) -> bool {
        self.session_running
    }

    pub fn run<H>(&mut self, handler: &mut H) -> Result<OpenXrRunOutcome>
    where
        H: OpenXrFrameLoopHandler<G>,
    {
        let ready_deadline = self
            .policy
            .session_ready_timeout
            .map(|timeout| Instant::now() + timeout);
        let mut progress_deadline = self
            .policy
            .submitted_frame_progress_timeout
            .map(|timeout| Instant::now() + timeout);

        loop {
            if self
                .policy
                .frame_limit
                .is_some_and(|limit| self.stats.submitted_frames >= limit)
            {
                return Ok(self.outcome(OpenXrRunExit::FrameLimit));
            }

            if handler.pump_platform_events(Duration::ZERO)? == OpenXrFrameLoopControl::Complete {
                return Ok(self.outcome(OpenXrRunExit::Platform));
            }

            match self.poll_events(handler).context("poll OpenXR events")? {
                OpenXrPollStatus::Exit if self.policy.runtime_exit_is_error => {
                    bail!("OpenXR session requested exit before frame loop completed")
                }
                OpenXrPollStatus::Exit => {
                    return Ok(self.outcome(OpenXrRunExit::Runtime));
                }
                OpenXrPollStatus::Idle if !self.session_running => {
                    if ready_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                        bail!("timed out waiting for OpenXR session READY state");
                    }
                    if handler.pump_platform_events(self.policy.idle_poll_interval)?
                        == OpenXrFrameLoopControl::Complete
                    {
                        return Ok(self.outcome(OpenXrRunExit::Platform));
                    }
                    continue;
                }
                OpenXrPollStatus::Idle | OpenXrPollStatus::Running => {}
            }

            let frame_wall_start = Instant::now();
            handler.before_frame(self.stats)?;

            let wait_start = Instant::now();
            let frame_state = self.frame_wait.wait().context("wait OpenXR frame")?;
            let wait_frame = wait_start.elapsed();
            handler.after_wait_frame()?;

            let begin_start = Instant::now();
            self.frame_stream.begin().context("begin OpenXR frame")?;
            let begin_frame = begin_start.elapsed();
            self.stats.record_runtime_frame();

            let (disposition, render_output) = if frame_state.should_render {
                let mut frame = OpenXrRenderFrame {
                    session: self.session,
                    frame_stream: self.frame_stream,
                    predicted_display_time: frame_state.predicted_display_time,
                    environment_blend_mode: self.policy.environment_blend_mode,
                    ended: false,
                };
                let output = match handler.render_frame(&mut frame) {
                    Ok(output) => output,
                    Err(error) => {
                        frame.finish_after_error();
                        return Err(error);
                    }
                };
                if !frame.ended {
                    frame.finish_after_error();
                    bail!("OpenXR render callback returned without submitting the frame");
                }
                self.stats.record_submitted_frame();
                (OpenXrFrameDisposition::Submitted, Some(output))
            } else {
                end_skipped_frame(
                    self.frame_stream,
                    frame_state.predicted_display_time,
                    self.policy.environment_blend_mode,
                    &mut self.stats,
                )?;
                (OpenXrFrameDisposition::Skipped, None)
            };

            if let Some(deadline) = progress_deadline.as_mut() {
                if disposition == OpenXrFrameDisposition::Submitted {
                    *deadline = Instant::now()
                        + self
                            .policy
                            .submitted_frame_progress_timeout
                            .expect("deadline implies configured progress timeout");
                } else if Instant::now() >= *deadline {
                    bail!(
                        "timed out waiting for OpenXR submitted-frame progress: submitted={} runtime_frames={} skipped={}",
                        self.stats.submitted_frames,
                        self.stats.runtime_frames,
                        self.stats.skipped_frames
                    );
                }
            }

            let outcome = OpenXrFrameOutcome {
                session_state: self.session_state,
                predicted_display_time: frame_state.predicted_display_time,
                should_render: frame_state.should_render,
                disposition,
                timing: OpenXrFrameTiming {
                    frame_wall: frame_wall_start.elapsed(),
                    wait_frame,
                    begin_frame,
                },
                stats: self.stats,
                render_output,
            };
            if handler.after_frame(outcome)? == OpenXrFrameLoopControl::Complete {
                return Ok(self.outcome(OpenXrRunExit::Handler));
            }
        }
    }

    pub fn request_exit_and_drain<F>(&mut self, timeout: Duration, mut on_event: F) -> Result<()>
    where
        F: FnMut(OpenXrHostEvent),
    {
        if !self.session_running {
            return Ok(());
        }
        match self.session.request_exit() {
            Ok(()) => {}
            Err(xr::sys::Result::ERROR_SESSION_NOT_RUNNING) => {
                self.session_running = false;
                return Ok(());
            }
            Err(error) => {
                log::warn!("OpenXR session exit request failed: {error:?}");
            }
        }

        let deadline = Instant::now() + timeout;
        while self.session_running && Instant::now() < deadline {
            let mut events = Vec::new();
            let status = poll_openxr_events(
                self.session,
                &mut self.event_storage,
                &mut self.session_running,
                self.policy.view_type,
                |event| events.push(event),
            )
            .context("poll OpenXR events during shutdown")?;
            for event in events {
                self.record_event(event);
                on_event(event);
            }
            if status == OpenXrPollStatus::Exit {
                break;
            }
            thread::sleep(self.policy.idle_poll_interval);
        }

        if self.session_running {
            match self.session.end() {
                Ok(_) | Err(xr::sys::Result::ERROR_SESSION_NOT_RUNNING) => {
                    self.session_running = false;
                }
                Err(error) => {
                    log::warn!("OpenXR session end during shutdown failed: {error:?}");
                }
            }
        }
        Ok(())
    }

    fn poll_events<H>(&mut self, handler: &mut H) -> Result<OpenXrPollStatus>
    where
        H: OpenXrFrameLoopHandler<G>,
    {
        let mut events = Vec::new();
        let status = poll_openxr_events(
            self.session,
            &mut self.event_storage,
            &mut self.session_running,
            self.policy.view_type,
            |event| events.push(event),
        )?;
        for event in events {
            self.record_event(event);
            handler.on_openxr_event(event);
        }
        Ok(status)
    }

    fn record_event(&mut self, event: OpenXrHostEvent) {
        if let OpenXrHostEvent::SessionStateChanged(state) = event {
            self.session_state = Some(state);
        }
    }

    const fn outcome(&self, exit: OpenXrRunExit) -> OpenXrRunOutcome {
        OpenXrRunOutcome {
            exit,
            session_state: self.session_state,
            stats: self.stats,
        }
    }
}

impl fmt::Display for OpenXrHostEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionStateChanged(state) => {
                write!(formatter, "OpenXR session state: {state:?}")
            }
            Self::InstanceLossPending => formatter.write_str("OpenXR instance loss pending"),
            Self::EventsLost(count) => write!(formatter, "OpenXR events lost: {count}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_event_display_preserves_validation_markers() {
        assert_eq!(
            OpenXrHostEvent::SessionStateChanged(xr::SessionState::READY).to_string(),
            "OpenXR session state: READY"
        );
        assert_eq!(
            OpenXrHostEvent::InstanceLossPending.to_string(),
            "OpenXR instance loss pending"
        );
        assert_eq!(
            OpenXrHostEvent::EventsLost(3).to_string(),
            "OpenXR events lost: 3"
        );
    }
}
