#![deny(unsafe_code)]

use anyhow::{Context, Result};
use openxr as xr;

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
