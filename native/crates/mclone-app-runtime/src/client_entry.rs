//! Shared interactive-client entry and host-lifecycle policy.
//!
//! Platform adapters normalize CLI arguments, activity intents, browser
//! locations, managed-launch options, and automation inputs into one
//! [`ClientEntryResolution`]. This module never performs I/O and never depends
//! on a platform event-loop type.

use crate::scenario::ScenarioLaunchIntent;
use crate::session::SessionStartRequest;

/// The initial product destination requested for an interactive client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientEntryIntent {
    /// Enter a session-free title menu.
    Title,
    /// Start a local, catalog, or remote session through the shared session
    /// request vocabulary.
    StartSession(SessionStartRequest),
    /// Launch one of the shared, path-free authored scenarios.
    LaunchScenario(ScenarioLaunchIntent),
}

impl Default for ClientEntryIntent {
    fn default() -> Self {
        Self::Title
    }
}

impl ClientEntryIntent {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::StartSession(_) => "start-session",
            Self::LaunchScenario(_) => "launch-scenario",
        }
    }
}

/// Provenance retained for diagnostics. Shared policy must not select behavior
/// by matching this value.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ClientEntrySource {
    #[default]
    ProductDefault,
    CommandLine,
    ManagedLaunch,
    ActivityIntent,
    BrowserLocation,
    XrLaunchIntent,
    Automation,
}

impl ClientEntrySource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProductDefault => "product-default",
            Self::CommandLine => "command-line",
            Self::ManagedLaunch => "managed-launch",
            Self::ActivityIntent => "activity-intent",
            Self::BrowserLocation => "browser-location",
            Self::XrLaunchIntent => "xr-launch-intent",
            Self::Automation => "automation",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientEntryStatus {
    pub message: String,
    pub ok: bool,
}

impl ClientEntryStatus {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ok: false,
        }
    }
}

/// Normalized result of platform parsing and shared fallback resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientEntryResolution {
    pub intent: ClientEntryIntent,
    pub source: ClientEntrySource,
    pub title_status: Option<ClientEntryStatus>,
}

impl Default for ClientEntryResolution {
    fn default() -> Self {
        Self::ordinary()
    }
}

impl ClientEntryResolution {
    /// Ordinary interactive product launch: a session-free title menu.
    pub const fn ordinary() -> Self {
        Self {
            intent: ClientEntryIntent::Title,
            source: ClientEntrySource::ProductDefault,
            title_status: None,
        }
    }

    pub const fn explicit(intent: ClientEntryIntent, source: ClientEntrySource) -> Self {
        Self {
            intent,
            source,
            title_status: None,
        }
    }

    /// Invalid, stale, or unavailable explicit destinations fail safely to the
    /// title menu with a visible reason.
    pub fn fallback_to_title(source: ClientEntrySource, reason: impl Into<String>) -> Self {
        Self {
            intent: ClientEntryIntent::Title,
            source,
            title_status: Some(ClientEntryStatus::error(reason)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClientHostAvailability {
    pub bootstrapped: bool,
    pub foreground: bool,
    pub presentation_available: bool,
}

impl ClientHostAvailability {
    pub const fn interactive_ready(self) -> bool {
        self.bootstrapped && self.foreground && self.presentation_available
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ClientEntryLifecycleState {
    #[default]
    AwaitingHost,
    Title,
    StartingSession,
    ActiveSession,
    Suspended,
}

/// Shared statement of useful work. Adapters map this onto their own event,
/// activity, browser-frame, or OpenXR cadence mechanics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ClientActivityDemand {
    #[default]
    Dormant,
    StaticUi,
    AnimatedUi {
        maximum_hz: u16,
    },
    ActiveSession,
    Suspended,
}

impl ClientActivityDemand {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dormant => "dormant",
            Self::StaticUi => "static-ui",
            Self::AnimatedUi { .. } => "animated-ui",
            Self::ActiveSession => "active-session",
            Self::Suspended => "suspended",
        }
    }

    pub const fn requires_continuous_frames(self) -> bool {
        matches!(self, Self::AnimatedUi { .. } | Self::ActiveSession)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientEntryEffect {
    EnterTitle { status: Option<ClientEntryStatus> },
    StartSession(SessionStartRequest),
    LaunchScenario(ScenarioLaunchIntent),
}

/// Idempotent launch/lifecycle reducer.
///
/// Presentation loss and activity recreation may repeat. The normalized entry
/// intent is emitted at most once, when bootstrap, foreground, and presentation
/// availability are all true.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientEntryController {
    resolution: ClientEntryResolution,
    availability: ClientHostAvailability,
    product_state: ClientEntryLifecycleState,
    entry_dispatched: bool,
}

impl Default for ClientEntryController {
    fn default() -> Self {
        Self::new(ClientEntryResolution::ordinary())
    }
}

impl ClientEntryController {
    pub fn new(resolution: ClientEntryResolution) -> Self {
        Self {
            resolution,
            availability: ClientHostAvailability::default(),
            product_state: ClientEntryLifecycleState::AwaitingHost,
            entry_dispatched: false,
        }
    }

    pub fn resolution(&self) -> &ClientEntryResolution {
        &self.resolution
    }

    pub const fn availability(&self) -> ClientHostAvailability {
        self.availability
    }

    pub fn update_host(
        &mut self,
        availability: ClientHostAvailability,
    ) -> Option<ClientEntryEffect> {
        self.availability = availability;
        self.dispatch_entry_if_ready()
    }

    pub fn set_bootstrapped(&mut self, ready: bool) -> Option<ClientEntryEffect> {
        self.availability.bootstrapped = ready;
        self.dispatch_entry_if_ready()
    }

    pub fn set_foreground(&mut self, foreground: bool) -> Option<ClientEntryEffect> {
        self.availability.foreground = foreground;
        self.dispatch_entry_if_ready()
    }

    pub fn set_presentation_available(&mut self, available: bool) -> Option<ClientEntryEffect> {
        self.availability.presentation_available = available;
        self.dispatch_entry_if_ready()
    }

    pub fn state(&self) -> ClientEntryLifecycleState {
        if self.entry_dispatched && !self.availability.interactive_ready() {
            ClientEntryLifecycleState::Suspended
        } else {
            self.product_state
        }
    }

    pub const fn entry_dispatched(&self) -> bool {
        self.entry_dispatched
    }

    pub fn mark_session_active(&mut self) {
        self.product_state = ClientEntryLifecycleState::ActiveSession;
    }

    pub fn mark_session_starting(&mut self) {
        self.product_state = ClientEntryLifecycleState::StartingSession;
    }

    pub fn mark_session_ended(&mut self) -> ClientEntryEffect {
        self.product_state = ClientEntryLifecycleState::Title;
        ClientEntryEffect::EnterTitle { status: None }
    }

    pub fn mark_session_failed(&mut self, reason: impl Into<String>) -> ClientEntryEffect {
        self.product_state = ClientEntryLifecycleState::Title;
        ClientEntryEffect::EnterTitle {
            status: Some(ClientEntryStatus::error(reason)),
        }
    }

    pub fn activity_demand(&self, title_animation_active: bool) -> ClientActivityDemand {
        match self.state() {
            ClientEntryLifecycleState::AwaitingHost => ClientActivityDemand::Dormant,
            ClientEntryLifecycleState::Title if title_animation_active => {
                ClientActivityDemand::AnimatedUi { maximum_hz: 30 }
            }
            ClientEntryLifecycleState::Title => ClientActivityDemand::StaticUi,
            ClientEntryLifecycleState::StartingSession
            | ClientEntryLifecycleState::ActiveSession => ClientActivityDemand::ActiveSession,
            ClientEntryLifecycleState::Suspended => ClientActivityDemand::Suspended,
        }
    }

    fn dispatch_entry_if_ready(&mut self) -> Option<ClientEntryEffect> {
        if self.entry_dispatched || !self.availability.interactive_ready() {
            return None;
        }
        self.entry_dispatched = true;
        Some(match &self.resolution.intent {
            ClientEntryIntent::Title => {
                self.product_state = ClientEntryLifecycleState::Title;
                ClientEntryEffect::EnterTitle {
                    status: self.resolution.title_status.clone(),
                }
            }
            ClientEntryIntent::StartSession(request) => {
                self.product_state = ClientEntryLifecycleState::StartingSession;
                ClientEntryEffect::StartSession(request.clone())
            }
            ClientEntryIntent::LaunchScenario(intent) => {
                self.product_state = ClientEntryLifecycleState::StartingSession;
                ClientEntryEffect::LaunchScenario(*intent)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::RemoteSessionEndpoint;

    fn ready_host() -> ClientHostAvailability {
        ClientHostAvailability {
            bootstrapped: true,
            foreground: true,
            presentation_available: true,
        }
    }

    #[test]
    fn ordinary_launch_enters_a_static_session_free_title() {
        let mut controller = ClientEntryController::default();
        assert_eq!(
            controller.activity_demand(false),
            ClientActivityDemand::Dormant
        );
        assert_eq!(
            controller.update_host(ready_host()),
            Some(ClientEntryEffect::EnterTitle { status: None })
        );
        assert_eq!(controller.state(), ClientEntryLifecycleState::Title);
        assert_eq!(
            controller.activity_demand(false),
            ClientActivityDemand::StaticUi
        );
        assert_eq!(ClientActivityDemand::StaticUi.label(), "static-ui");
        assert!(!ClientActivityDemand::StaticUi.requires_continuous_frames());
        assert!(ClientActivityDemand::ActiveSession.requires_continuous_frames());
    }

    #[test]
    fn every_explicit_source_has_identical_session_policy() {
        for source in [
            ClientEntrySource::CommandLine,
            ClientEntrySource::ManagedLaunch,
            ClientEntrySource::ActivityIntent,
            ClientEntrySource::BrowserLocation,
            ClientEntrySource::XrLaunchIntent,
            ClientEntrySource::Automation,
        ] {
            let request = SessionStartRequest::JoinRemote {
                endpoint: RemoteSessionEndpoint::new("example.invalid:25565"),
            };
            let mut controller = ClientEntryController::new(ClientEntryResolution::explicit(
                ClientEntryIntent::StartSession(request.clone()),
                source,
            ));
            assert_eq!(
                controller.update_host(ready_host()),
                Some(ClientEntryEffect::StartSession(request))
            );
            assert_eq!(
                controller.activity_demand(false),
                ClientActivityDemand::ActiveSession
            );
        }
    }

    #[test]
    fn invalid_explicit_entry_falls_back_to_title_with_visible_status() {
        let mut controller = ClientEntryController::new(ClientEntryResolution::fallback_to_title(
            ClientEntrySource::BrowserLocation,
            "Unknown world `missing`",
        ));
        assert_eq!(
            controller.update_host(ready_host()),
            Some(ClientEntryEffect::EnterTitle {
                status: Some(ClientEntryStatus::error("Unknown world `missing`")),
            })
        );
        assert_eq!(controller.state(), ClientEntryLifecycleState::Title);
    }

    #[test]
    fn repeated_presentation_and_foreground_events_never_duplicate_entry() {
        let mut controller = ClientEntryController::new(ClientEntryResolution::explicit(
            ClientEntryIntent::LaunchScenario(ScenarioLaunchIntent::lobby_preview()),
            ClientEntrySource::XrLaunchIntent,
        ));
        assert_eq!(controller.set_bootstrapped(true), None);
        assert_eq!(controller.set_foreground(true), None);
        assert_eq!(
            controller.set_presentation_available(true),
            Some(ClientEntryEffect::LaunchScenario(
                ScenarioLaunchIntent::lobby_preview()
            ))
        );

        assert_eq!(controller.set_presentation_available(false), None);
        assert_eq!(controller.state(), ClientEntryLifecycleState::Suspended);
        assert_eq!(
            controller.activity_demand(true),
            ClientActivityDemand::Suspended
        );
        assert_eq!(controller.set_presentation_available(true), None);
        assert_eq!(
            controller.state(),
            ClientEntryLifecycleState::StartingSession
        );
        assert_eq!(controller.set_foreground(false), None);
        assert_eq!(controller.set_foreground(true), None);
    }

    #[test]
    fn session_failure_returns_to_title_without_relaunching() {
        let request = SessionStartRequest::new_seed_local_world(7);
        let mut controller = ClientEntryController::new(ClientEntryResolution::explicit(
            ClientEntryIntent::StartSession(request.clone()),
            ClientEntrySource::Automation,
        ));
        assert_eq!(
            controller.update_host(ready_host()),
            Some(ClientEntryEffect::StartSession(request))
        );
        assert_eq!(
            controller.mark_session_failed("World failed"),
            ClientEntryEffect::EnterTitle {
                status: Some(ClientEntryStatus::error("World failed")),
            }
        );
        assert_eq!(controller.state(), ClientEntryLifecycleState::Title);
        assert_eq!(controller.update_host(ready_host()), None);
    }
}
