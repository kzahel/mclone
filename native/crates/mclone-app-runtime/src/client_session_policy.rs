use crate::session::{GameSessionState, RemoteSessionEndpoint, SessionStartRequest, SessionStatus};
use mclone_ui::{GameScreen, GameUiAction, LoadingProgressOverlay, StatusOverlay};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientSessionActionContext<'a> {
    pub next_new_world_seed: Option<i64>,
    pub current_join_remote_addr: &'a str,
    pub fallback_remote_addr: Option<&'a str>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientSessionEffects {
    pub new_world_seed: Option<i64>,
    pub join_remote_addr: Option<String>,
    pub session_start: Option<SessionStartRequest>,
    pub host_action: Option<ClientSessionHostAction>,
    pub clear_inactive_session_status: bool,
}

impl ClientSessionEffects {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientSessionHostAction {
    QuitToTitle,
    Quit,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientSessionUiEffects {
    pub new_world_seed: Option<i64>,
    pub join_remote_addr: Option<String>,
    pub screen: Option<GameScreen>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientSessionTransitionEffects {
    pub teardown_world: bool,
    pub clear_session: bool,
    pub ui: ClientSessionUiEffects,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientSessionStatusProjection {
    pub status_overlay: StatusOverlay,
    pub loading_progress_overlay: Option<LoadingProgressOverlay>,
}

pub fn client_session_effects_for_action(
    action: GameUiAction,
    context: ClientSessionActionContext<'_>,
) -> ClientSessionEffects {
    match action {
        GameUiAction::OpenNewWorld | GameUiAction::RerollSeed => ClientSessionEffects {
            new_world_seed: context.next_new_world_seed,
            clear_inactive_session_status: true,
            ..ClientSessionEffects::default()
        },
        GameUiAction::OpenJoinRemote => ClientSessionEffects {
            join_remote_addr: Some(
                context
                    .fallback_remote_addr
                    .unwrap_or(context.current_join_remote_addr)
                    .to_owned(),
            ),
            clear_inactive_session_status: true,
            ..ClientSessionEffects::default()
        },
        GameUiAction::CreateWorld(seed) => ClientSessionEffects {
            session_start: Some(SessionStartRequest::new_seed_local_world(seed)),
            ..ClientSessionEffects::default()
        },
        GameUiAction::JoinRemote => ClientSessionEffects {
            session_start: Some(SessionStartRequest::JoinRemote {
                endpoint: RemoteSessionEndpoint::new(context.current_join_remote_addr.to_owned()),
            }),
            ..ClientSessionEffects::default()
        },
        GameUiAction::BackToTitle => ClientSessionEffects {
            clear_inactive_session_status: true,
            ..ClientSessionEffects::default()
        },
        GameUiAction::QuitToTitle => ClientSessionEffects {
            host_action: Some(ClientSessionHostAction::QuitToTitle),
            ..ClientSessionEffects::default()
        },
        GameUiAction::Quit => ClientSessionEffects {
            host_action: Some(ClientSessionHostAction::Quit),
            ..ClientSessionEffects::default()
        },
        _ => ClientSessionEffects::default(),
    }
}

pub fn client_session_ui_effects_for_request(
    request: &SessionStartRequest,
) -> ClientSessionUiEffects {
    match request {
        SessionStartRequest::CreateLocalWorld { options } => ClientSessionUiEffects {
            new_world_seed: Some(options.seed),
            ..ClientSessionUiEffects::default()
        },
        SessionStartRequest::OpenLocalWorld { .. } => ClientSessionUiEffects::default(),
        SessionStartRequest::JoinRemote { endpoint } => ClientSessionUiEffects {
            join_remote_addr: Some(endpoint.address.clone()),
            ..ClientSessionUiEffects::default()
        },
        SessionStartRequest::Unknown => ClientSessionUiEffects::default(),
    }
}

pub fn client_session_failed_start_ui_effects(
    request: &SessionStartRequest,
    show_title_on_failure: bool,
) -> ClientSessionUiEffects {
    let mut effects = client_session_ui_effects_for_request(request);
    if show_title_on_failure {
        effects.screen = Some(GameScreen::Title);
    }
    effects
}

pub fn client_session_status_overlay(status: Option<SessionStatus>) -> StatusOverlay {
    client_session_status_projection(status, StatusOverlay::hidden(), None).status_overlay
}

pub fn client_session_effective_status_overlay(
    status: Option<SessionStatus>,
    fallback: StatusOverlay,
) -> StatusOverlay {
    status.map_or(fallback, |status| {
        StatusOverlay::new(status.message, status.ok)
    })
}

pub fn client_session_status_projection(
    status: Option<SessionStatus>,
    fallback_status: StatusOverlay,
    loading_progress_overlay: Option<LoadingProgressOverlay>,
) -> ClientSessionStatusProjection {
    ClientSessionStatusProjection {
        status_overlay: client_session_effective_status_overlay(status, fallback_status),
        loading_progress_overlay,
    }
}

pub fn client_session_should_clear_inactive_status(state: &GameSessionState) -> bool {
    !matches!(state, GameSessionState::Active { .. })
}

pub fn client_session_start_transition(state: &GameSessionState) -> ClientSessionTransitionEffects {
    ClientSessionTransitionEffects {
        teardown_world: client_session_has_teardownable_runtime(state),
        ..ClientSessionTransitionEffects::default()
    }
}

pub fn client_session_quit_to_title_transition(
    state: &GameSessionState,
) -> ClientSessionTransitionEffects {
    ClientSessionTransitionEffects {
        teardown_world: client_session_has_teardownable_runtime(state),
        clear_session: true,
        ui: ClientSessionUiEffects {
            screen: Some(GameScreen::Title),
            ..ClientSessionUiEffects::default()
        },
    }
}

fn client_session_has_teardownable_runtime(state: &GameSessionState) -> bool {
    matches!(
        state,
        GameSessionState::Active { .. } | GameSessionState::Starting { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{ActiveSessionDescriptor, SessionFailure, SessionStatus};

    fn context() -> ClientSessionActionContext<'static> {
        ClientSessionActionContext {
            next_new_world_seed: Some(42),
            current_join_remote_addr: "127.0.0.1:25565",
            fallback_remote_addr: Some("10.0.0.9:25565"),
        }
    }

    #[test]
    fn open_new_world_sets_next_seed_and_clears_inactive_status() {
        let effects = client_session_effects_for_action(GameUiAction::OpenNewWorld, context());
        assert_eq!(effects.new_world_seed, Some(42));
        assert!(effects.clear_inactive_session_status);
        assert_eq!(effects.session_start, None);
    }

    #[test]
    fn open_join_remote_uses_fallback_endpoint() {
        let effects = client_session_effects_for_action(GameUiAction::OpenJoinRemote, context());
        assert_eq!(effects.join_remote_addr.as_deref(), Some("10.0.0.9:25565"));
        assert!(effects.clear_inactive_session_status);
    }

    #[test]
    fn create_world_emits_seed_local_session_start() {
        let effects = client_session_effects_for_action(GameUiAction::CreateWorld(1234), context());
        assert_eq!(
            effects.session_start,
            Some(SessionStartRequest::new_seed_local_world(1234))
        );
        assert_eq!(effects.host_action, None);
    }

    #[test]
    fn join_remote_emits_remote_session_start() {
        let effects = client_session_effects_for_action(GameUiAction::JoinRemote, context());
        assert_eq!(
            effects.session_start,
            Some(SessionStartRequest::JoinRemote {
                endpoint: RemoteSessionEndpoint::new("127.0.0.1:25565")
            })
        );
    }

    #[test]
    fn quit_actions_emit_host_actions() {
        assert_eq!(
            client_session_effects_for_action(GameUiAction::QuitToTitle, context()).host_action,
            Some(ClientSessionHostAction::QuitToTitle)
        );
        assert_eq!(
            client_session_effects_for_action(GameUiAction::Quit, context()).host_action,
            Some(ClientSessionHostAction::Quit)
        );
    }

    #[test]
    fn failed_start_restores_request_fields_and_optional_title_screen() {
        let effects = client_session_failed_start_ui_effects(
            &SessionStartRequest::new_seed_local_world(3456),
            true,
        );
        assert_eq!(effects.new_world_seed, Some(3456));
        assert_eq!(effects.join_remote_addr, None);
        assert_eq!(effects.screen, Some(GameScreen::Title));

        let effects = client_session_failed_start_ui_effects(
            &SessionStartRequest::JoinRemote {
                endpoint: RemoteSessionEndpoint::new("example.test:25565"),
            },
            false,
        );
        assert_eq!(effects.new_world_seed, None);
        assert_eq!(
            effects.join_remote_addr.as_deref(),
            Some("example.test:25565")
        );
        assert_eq!(effects.screen, None);
    }

    #[test]
    fn status_overlay_prefers_session_status_over_fallback() {
        let fallback = StatusOverlay::new("browser status", true);
        let overlay = client_session_effective_status_overlay(None, fallback.clone());
        assert_eq!(overlay, fallback);

        let overlay = client_session_effective_status_overlay(
            Some(SessionStatus {
                message: "Connecting...".to_owned(),
                ok: true,
            }),
            fallback,
        );
        assert_eq!(overlay, StatusOverlay::new("Connecting...", true));
    }

    #[test]
    fn session_projection_combines_status_and_loading_progress() {
        let progress = LoadingProgressOverlay::new(1, 2, 9, false, []);
        let projection = client_session_status_projection(
            Some(SessionStatus {
                message: "Loading world...".to_owned(),
                ok: true,
            }),
            StatusOverlay::new("browser status", true),
            Some(progress.clone()),
        );

        assert_eq!(
            projection.status_overlay,
            StatusOverlay::new("Loading world...", true)
        );
        assert_eq!(projection.loading_progress_overlay, Some(progress));

        let fallback = StatusOverlay::new("browser status", true);
        let projection = client_session_status_projection(None, fallback.clone(), None);
        assert_eq!(projection.status_overlay, fallback);
        assert_eq!(projection.loading_progress_overlay, None);
    }

    #[test]
    fn inactive_status_clear_policy_keeps_active_sessions() {
        assert!(client_session_should_clear_inactive_status(
            &GameSessionState::Failed {
                request: SessionStartRequest::new_seed_local_world(1),
                error: SessionFailure::new("failed")
            }
        ));
        assert!(!client_session_should_clear_inactive_status(
            &GameSessionState::Active {
                session: ActiveSessionDescriptor::new_seed_local_world(1)
            }
        ));
    }

    #[test]
    fn start_transition_tears_down_active_or_starting_sessions_only() {
        assert!(
            client_session_start_transition(&GameSessionState::Active {
                session: ActiveSessionDescriptor::new_seed_local_world(1)
            })
            .teardown_world
        );
        assert!(
            client_session_start_transition(&GameSessionState::Starting {
                request: SessionStartRequest::new_seed_local_world(2)
            })
            .teardown_world
        );
        assert!(
            !client_session_start_transition(&GameSessionState::Failed {
                request: SessionStartRequest::new_seed_local_world(3),
                error: SessionFailure::new("failed")
            })
            .teardown_world
        );
        assert!(!client_session_start_transition(&GameSessionState::NoSession).teardown_world);
    }

    #[test]
    fn quit_to_title_transition_clears_session_and_opens_title() {
        let transition = client_session_quit_to_title_transition(&GameSessionState::Active {
            session: ActiveSessionDescriptor::new_seed_local_world(1),
        });
        assert!(transition.teardown_world);
        assert!(transition.clear_session);
        assert_eq!(transition.ui.screen, Some(GameScreen::Title));

        let transition = client_session_quit_to_title_transition(&GameSessionState::Failed {
            request: SessionStartRequest::new_seed_local_world(2),
            error: SessionFailure::new("failed"),
        });
        assert!(!transition.teardown_world);
        assert!(transition.clear_session);
        assert_eq!(transition.ui.screen, Some(GameScreen::Title));
    }
}
