use crate::session::{GameSessionState, RemoteSessionEndpoint, SessionStartRequest, SessionStatus};
use mclone_ui::{GameScreen, GameUiAction, StatusOverlay};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlatClientSessionActionContext<'a> {
    pub next_new_world_seed: Option<i64>,
    pub current_join_remote_addr: &'a str,
    pub fallback_remote_addr: Option<&'a str>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FlatClientSessionEffects {
    pub new_world_seed: Option<i64>,
    pub join_remote_addr: Option<String>,
    pub session_start: Option<SessionStartRequest>,
    pub host_action: Option<FlatClientSessionHostAction>,
    pub clear_inactive_session_status: bool,
}

impl FlatClientSessionEffects {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlatClientSessionHostAction {
    QuitToTitle,
    Quit,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FlatClientSessionUiEffects {
    pub new_world_seed: Option<i64>,
    pub join_remote_addr: Option<String>,
    pub screen: Option<GameScreen>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FlatClientSessionTransitionEffects {
    pub teardown_world: bool,
    pub clear_session: bool,
    pub ui: FlatClientSessionUiEffects,
}

pub fn flat_client_session_effects_for_action(
    action: GameUiAction,
    context: FlatClientSessionActionContext<'_>,
) -> FlatClientSessionEffects {
    match action {
        GameUiAction::OpenNewWorld | GameUiAction::RerollSeed => FlatClientSessionEffects {
            new_world_seed: context.next_new_world_seed,
            clear_inactive_session_status: true,
            ..FlatClientSessionEffects::default()
        },
        GameUiAction::OpenJoinRemote => FlatClientSessionEffects {
            join_remote_addr: Some(
                context
                    .fallback_remote_addr
                    .unwrap_or(context.current_join_remote_addr)
                    .to_owned(),
            ),
            clear_inactive_session_status: true,
            ..FlatClientSessionEffects::default()
        },
        GameUiAction::CreateWorld(seed) => FlatClientSessionEffects {
            session_start: Some(SessionStartRequest::new_seed_local_world(seed)),
            ..FlatClientSessionEffects::default()
        },
        GameUiAction::JoinRemote => FlatClientSessionEffects {
            session_start: Some(SessionStartRequest::JoinRemote {
                endpoint: RemoteSessionEndpoint::new(context.current_join_remote_addr.to_owned()),
            }),
            ..FlatClientSessionEffects::default()
        },
        GameUiAction::BackToTitle => FlatClientSessionEffects {
            clear_inactive_session_status: true,
            ..FlatClientSessionEffects::default()
        },
        GameUiAction::QuitToTitle => FlatClientSessionEffects {
            host_action: Some(FlatClientSessionHostAction::QuitToTitle),
            ..FlatClientSessionEffects::default()
        },
        GameUiAction::Quit => FlatClientSessionEffects {
            host_action: Some(FlatClientSessionHostAction::Quit),
            ..FlatClientSessionEffects::default()
        },
        _ => FlatClientSessionEffects::default(),
    }
}

pub fn flat_client_session_ui_effects_for_request(
    request: &SessionStartRequest,
) -> FlatClientSessionUiEffects {
    match request {
        SessionStartRequest::CreateLocalWorld { options } => FlatClientSessionUiEffects {
            new_world_seed: Some(options.seed),
            ..FlatClientSessionUiEffects::default()
        },
        SessionStartRequest::OpenLocalWorld { .. } => FlatClientSessionUiEffects::default(),
        SessionStartRequest::JoinRemote { endpoint } => FlatClientSessionUiEffects {
            join_remote_addr: Some(endpoint.address.clone()),
            ..FlatClientSessionUiEffects::default()
        },
        SessionStartRequest::Unknown => FlatClientSessionUiEffects::default(),
    }
}

pub fn flat_client_failed_start_ui_effects(
    request: &SessionStartRequest,
    show_title_on_failure: bool,
) -> FlatClientSessionUiEffects {
    let mut effects = flat_client_session_ui_effects_for_request(request);
    if show_title_on_failure {
        effects.screen = Some(GameScreen::Title);
    }
    effects
}

pub fn flat_client_session_status_overlay(status: Option<SessionStatus>) -> StatusOverlay {
    flat_client_effective_status_overlay(status, StatusOverlay::hidden())
}

pub fn flat_client_effective_status_overlay(
    status: Option<SessionStatus>,
    fallback: StatusOverlay,
) -> StatusOverlay {
    status.map_or(fallback, |status| {
        StatusOverlay::new(status.message, status.ok)
    })
}

pub fn flat_client_should_clear_inactive_session_status(state: &GameSessionState) -> bool {
    !matches!(state, GameSessionState::Active { .. })
}

pub fn flat_client_start_session_transition(
    state: &GameSessionState,
) -> FlatClientSessionTransitionEffects {
    FlatClientSessionTransitionEffects {
        teardown_world: flat_client_session_has_teardownable_runtime(state),
        ..FlatClientSessionTransitionEffects::default()
    }
}

pub fn flat_client_quit_to_title_transition(
    state: &GameSessionState,
) -> FlatClientSessionTransitionEffects {
    FlatClientSessionTransitionEffects {
        teardown_world: flat_client_session_has_teardownable_runtime(state),
        clear_session: true,
        ui: FlatClientSessionUiEffects {
            screen: Some(GameScreen::Title),
            ..FlatClientSessionUiEffects::default()
        },
    }
}

fn flat_client_session_has_teardownable_runtime(state: &GameSessionState) -> bool {
    matches!(
        state,
        GameSessionState::Active { .. } | GameSessionState::Starting { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{ActiveSessionDescriptor, SessionFailure, SessionStatus};

    fn context() -> FlatClientSessionActionContext<'static> {
        FlatClientSessionActionContext {
            next_new_world_seed: Some(42),
            current_join_remote_addr: "127.0.0.1:25565",
            fallback_remote_addr: Some("10.0.0.9:25565"),
        }
    }

    #[test]
    fn open_new_world_sets_next_seed_and_clears_inactive_status() {
        let effects = flat_client_session_effects_for_action(GameUiAction::OpenNewWorld, context());
        assert_eq!(effects.new_world_seed, Some(42));
        assert!(effects.clear_inactive_session_status);
        assert_eq!(effects.session_start, None);
    }

    #[test]
    fn open_join_remote_uses_fallback_endpoint() {
        let effects =
            flat_client_session_effects_for_action(GameUiAction::OpenJoinRemote, context());
        assert_eq!(effects.join_remote_addr.as_deref(), Some("10.0.0.9:25565"));
        assert!(effects.clear_inactive_session_status);
    }

    #[test]
    fn create_world_emits_seed_local_session_start() {
        let effects =
            flat_client_session_effects_for_action(GameUiAction::CreateWorld(1234), context());
        assert_eq!(
            effects.session_start,
            Some(SessionStartRequest::new_seed_local_world(1234))
        );
        assert_eq!(effects.host_action, None);
    }

    #[test]
    fn join_remote_emits_remote_session_start() {
        let effects = flat_client_session_effects_for_action(GameUiAction::JoinRemote, context());
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
            flat_client_session_effects_for_action(GameUiAction::QuitToTitle, context())
                .host_action,
            Some(FlatClientSessionHostAction::QuitToTitle)
        );
        assert_eq!(
            flat_client_session_effects_for_action(GameUiAction::Quit, context()).host_action,
            Some(FlatClientSessionHostAction::Quit)
        );
    }

    #[test]
    fn failed_start_restores_request_fields_and_optional_title_screen() {
        let effects = flat_client_failed_start_ui_effects(
            &SessionStartRequest::new_seed_local_world(3456),
            true,
        );
        assert_eq!(effects.new_world_seed, Some(3456));
        assert_eq!(effects.join_remote_addr, None);
        assert_eq!(effects.screen, Some(GameScreen::Title));

        let effects = flat_client_failed_start_ui_effects(
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
        let overlay = flat_client_effective_status_overlay(None, fallback.clone());
        assert_eq!(overlay, fallback);

        let overlay = flat_client_effective_status_overlay(
            Some(SessionStatus {
                message: "Connecting...".to_owned(),
                ok: true,
            }),
            fallback,
        );
        assert_eq!(overlay, StatusOverlay::new("Connecting...", true));
    }

    #[test]
    fn inactive_status_clear_policy_keeps_active_sessions() {
        assert!(flat_client_should_clear_inactive_session_status(
            &GameSessionState::Failed {
                request: SessionStartRequest::new_seed_local_world(1),
                error: SessionFailure::new("failed")
            }
        ));
        assert!(!flat_client_should_clear_inactive_session_status(
            &GameSessionState::Active {
                session: ActiveSessionDescriptor::new_seed_local_world(1)
            }
        ));
    }

    #[test]
    fn start_transition_tears_down_active_or_starting_sessions_only() {
        assert!(
            flat_client_start_session_transition(&GameSessionState::Active {
                session: ActiveSessionDescriptor::new_seed_local_world(1)
            })
            .teardown_world
        );
        assert!(
            flat_client_start_session_transition(&GameSessionState::Starting {
                request: SessionStartRequest::new_seed_local_world(2)
            })
            .teardown_world
        );
        assert!(
            !flat_client_start_session_transition(&GameSessionState::Failed {
                request: SessionStartRequest::new_seed_local_world(3),
                error: SessionFailure::new("failed")
            })
            .teardown_world
        );
        assert!(!flat_client_start_session_transition(&GameSessionState::NoSession).teardown_world);
    }

    #[test]
    fn quit_to_title_transition_clears_session_and_opens_title() {
        let transition = flat_client_quit_to_title_transition(&GameSessionState::Active {
            session: ActiveSessionDescriptor::new_seed_local_world(1),
        });
        assert!(transition.teardown_world);
        assert!(transition.clear_session);
        assert_eq!(transition.ui.screen, Some(GameScreen::Title));

        let transition = flat_client_quit_to_title_transition(&GameSessionState::Failed {
            request: SessionStartRequest::new_seed_local_world(2),
            error: SessionFailure::new("failed"),
        });
        assert!(!transition.teardown_world);
        assert!(transition.clear_session);
        assert_eq!(transition.ui.screen, Some(GameScreen::Title));
    }
}
