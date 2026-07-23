//! Browser scene-driver mechanics around the shared scene host.
//!
//! Promise, Worker, WebSocket, and IndexedDB objects remain in the wasm
//! adapter. Session identity and lifecycle stay in the shared scene/session
//! coordinators; this module owns projected browser time, frame admission, and
//! the deferred catalog endpoint only.

use mclone_app_runtime::catalog_executor::{
    DeferredWorldCatalogOperationHandle, WorldCatalogOperation, WorldCatalogOperationService,
};
use mclone_app_runtime::monotonic::{
    MonotonicClock, MonotonicClockHandle, MonotonicInstant, ProjectedMonotonicClock,
};
use mclone_app_runtime::platform_operation::{PlatformOperation, PlatformOperationCompletion};
use mclone_app_runtime::world_catalog::{WorldCatalogError, WorldCatalogResponse};

/// Browser-side handles retained outside `McloneSceneHost` while the host owns
/// the corresponding neutral clock and catalog service.
#[derive(Debug)]
pub struct WebScenePlatformServices {
    clock: WebMonotonicClock,
    catalog: DeferredWorldCatalogOperationHandle,
}

#[derive(Clone, Debug, Default)]
struct WebMonotonicClock {
    projected: ProjectedMonotonicClock,
}

impl WebMonotonicClock {
    fn observe_millis(&self, millis: f64) -> MonotonicInstant {
        self.projected.observe_millis(millis)
    }
}

impl MonotonicClock for WebMonotonicClock {
    fn now(&self) -> MonotonicInstant {
        #[cfg(target_arch = "wasm32")]
        if let Some(performance) = web_sys::window().and_then(|window| window.performance()) {
            return self.projected.observe_millis(performance.now());
        }
        self.projected.now()
    }
}

/// Browser-frame lifecycle retained by the rAF driver rim.
///
/// Hidden pages do not step the scene and the first resumed delta is zero.
/// Visible elapsed time remains intact for the shared scene-owned movement
/// scheduler, which applies one cross-platform bounded catch-up policy.
/// Surface/device failure is explicit and terminal for this proof owner; the
/// caller must construct a fresh owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebSceneFrameState {
    Running,
    Hidden,
    RestartRequired { reason: String },
    Shutdown,
}

impl WebSceneFrameState {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Hidden => "hidden",
            Self::RestartRequired { .. } => "restart-required",
            Self::Shutdown => "shutdown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WebSceneFrameAdmission {
    Render {
        delta_seconds: f64,
        first_after_resume: bool,
    },
    Hidden,
    RestartRequired,
    Shutdown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WebSceneFrameDriverPolicy {
    state: WebSceneFrameState,
    last_frame_millis: Option<f64>,
    first_after_resume: bool,
    hidden_frame_skips: u64,
    resume_count: u64,
    max_delta_millis: f64,
}

impl Default for WebSceneFrameDriverPolicy {
    fn default() -> Self {
        Self::new(f64::INFINITY)
    }
}

impl WebSceneFrameDriverPolicy {
    pub fn new(max_delta_millis: f64) -> Self {
        Self {
            state: WebSceneFrameState::Running,
            last_frame_millis: None,
            first_after_resume: true,
            hidden_frame_skips: 0,
            resume_count: 0,
            max_delta_millis: max_delta_millis.max(0.0),
        }
    }

    pub fn state(&self) -> &WebSceneFrameState {
        &self.state
    }

    pub const fn hidden_frame_skips(&self) -> u64 {
        self.hidden_frame_skips
    }

    pub const fn resume_count(&self) -> u64 {
        self.resume_count
    }

    pub fn set_hidden(&mut self, hidden: bool) -> bool {
        match (&self.state, hidden) {
            (WebSceneFrameState::Running, true) => {
                self.state = WebSceneFrameState::Hidden;
                self.last_frame_millis = None;
                true
            }
            (WebSceneFrameState::Hidden, false) => {
                self.state = WebSceneFrameState::Running;
                self.last_frame_millis = None;
                self.first_after_resume = true;
                self.resume_count = self.resume_count.saturating_add(1);
                true
            }
            _ => false,
        }
    }

    pub fn admit_frame(&mut self, now_millis: f64) -> WebSceneFrameAdmission {
        match &self.state {
            WebSceneFrameState::Hidden => {
                self.hidden_frame_skips = self.hidden_frame_skips.saturating_add(1);
                WebSceneFrameAdmission::Hidden
            }
            WebSceneFrameState::RestartRequired { .. } => WebSceneFrameAdmission::RestartRequired,
            WebSceneFrameState::Shutdown => WebSceneFrameAdmission::Shutdown,
            WebSceneFrameState::Running => {
                let now_millis = if now_millis.is_finite() {
                    now_millis.max(0.0)
                } else {
                    self.last_frame_millis.unwrap_or(0.0)
                };
                let first_after_resume = self.first_after_resume;
                let delta_millis = if first_after_resume {
                    0.0
                } else {
                    self.last_frame_millis
                        .map_or(0.0, |previous| (now_millis - previous).max(0.0))
                        .min(self.max_delta_millis)
                };
                self.first_after_resume = false;
                self.last_frame_millis = Some(now_millis);
                WebSceneFrameAdmission::Render {
                    delta_seconds: delta_millis / 1_000.0,
                    first_after_resume,
                }
            }
        }
    }

    pub fn require_restart(&mut self, reason: impl Into<String>) {
        self.state = WebSceneFrameState::RestartRequired {
            reason: reason.into(),
        };
        self.last_frame_millis = None;
    }

    pub fn shutdown(&mut self) {
        self.state = WebSceneFrameState::Shutdown;
        self.last_frame_millis = None;
    }
}

impl WebScenePlatformServices {
    pub fn new() -> (Self, MonotonicClockHandle, WorldCatalogOperationService) {
        let clock = WebMonotonicClock::default();
        let clock_handle = MonotonicClockHandle::new(clock.clone());
        let (catalog_operations, catalog) = WorldCatalogOperationService::deferred();
        (Self { clock, catalog }, clock_handle, catalog_operations)
    }

    pub fn observe_frame_time_millis(&self, millis: f64) -> MonotonicInstant {
        self.clock.observe_millis(millis)
    }

    pub fn clock_handle(&self) -> MonotonicClockHandle {
        MonotonicClockHandle::new(self.clock.clone())
    }

    pub fn take_catalog_operation(&self) -> Option<PlatformOperation<WorldCatalogOperation>> {
        self.catalog.take_submitted()
    }

    pub fn complete_catalog_operation(
        &self,
        completion: PlatformOperationCompletion<WorldCatalogResponse, WorldCatalogError>,
    ) {
        self.catalog.submit_completion(completion);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_frames_pause_and_resume_with_a_clamped_delta() {
        let mut policy = WebSceneFrameDriverPolicy::new(50.0);
        assert_eq!(
            policy.admit_frame(100.0),
            WebSceneFrameAdmission::Render {
                delta_seconds: 0.0,
                first_after_resume: true,
            }
        );
        assert!(policy.set_hidden(true));
        assert_eq!(policy.admit_frame(10_000.0), WebSceneFrameAdmission::Hidden);
        assert!(policy.set_hidden(false));
        assert_eq!(
            policy.admit_frame(10_100.0),
            WebSceneFrameAdmission::Render {
                delta_seconds: 0.0,
                first_after_resume: true,
            }
        );
        assert_eq!(
            policy.admit_frame(10_500.0),
            WebSceneFrameAdmission::Render {
                delta_seconds: 0.05,
                first_after_resume: false,
            }
        );
        assert_eq!(policy.hidden_frame_skips(), 1);
        assert_eq!(policy.resume_count(), 1);
    }

    #[test]
    fn default_policy_preserves_visible_elapsed_time_for_shared_catch_up() {
        let mut policy = WebSceneFrameDriverPolicy::default();
        assert!(matches!(
            policy.admit_frame(100.0),
            WebSceneFrameAdmission::Render {
                delta_seconds: 0.0,
                first_after_resume: true,
            }
        ));
        assert_eq!(
            policy.admit_frame(300.0),
            WebSceneFrameAdmission::Render {
                delta_seconds: 0.2,
                first_after_resume: false,
            }
        );
    }

    #[test]
    fn surface_failure_requires_restart_and_shutdown_is_terminal() {
        let mut policy = WebSceneFrameDriverPolicy::default();
        policy.require_restart("surface lost");
        assert_eq!(
            policy.admit_frame(1.0),
            WebSceneFrameAdmission::RestartRequired
        );
        assert!(matches!(
            policy.state(),
            WebSceneFrameState::RestartRequired { reason } if reason == "surface lost"
        ));
        policy.shutdown();
        assert_eq!(policy.admit_frame(2.0), WebSceneFrameAdmission::Shutdown);
    }
}
