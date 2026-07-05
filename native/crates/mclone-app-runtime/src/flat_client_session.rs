use crate::session::{RemoteSessionEndpoint, SessionStartRequest};
use mclone_ui::GameUiAction;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
