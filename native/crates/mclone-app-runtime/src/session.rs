#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameSessionCoordinator<P> {
    state: GameSessionState,
    pending_start: Option<PendingSessionStart<P>>,
}

impl<P> Default for GameSessionCoordinator<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P> GameSessionCoordinator<P> {
    pub fn new() -> Self {
        Self {
            state: GameSessionState::NoSession,
            pending_start: None,
        }
    }

    pub fn state(&self) -> &GameSessionState {
        &self.state
    }

    pub fn is_starting(&self) -> bool {
        matches!(self.state, GameSessionState::Starting { .. })
    }

    pub fn begin_start(&mut self, request: SessionStartRequest) {
        self.state = GameSessionState::Starting {
            request: request.clone(),
        };
        self.pending_start = None;
    }

    pub fn request_start(&mut self, request: SessionStartRequest, payload: P) {
        self.state = GameSessionState::Starting {
            request: request.clone(),
        };
        self.pending_start = Some(PendingSessionStart { request, payload });
    }

    pub fn take_pending_start(&mut self) -> Option<PendingSessionStart<P>> {
        self.pending_start.take()
    }

    pub fn complete_start(&mut self, session: ActiveSessionDescriptor) {
        self.state = GameSessionState::Active { session };
        self.pending_start = None;
    }

    pub fn fail_start(&mut self, error: SessionFailure) {
        let request = match &self.state {
            GameSessionState::Starting { request } => request.clone(),
            GameSessionState::Failed { request, .. } => request.clone(),
            GameSessionState::Active { session } => session.start_request(),
            GameSessionState::NoSession => SessionStartRequest::Unknown,
        };
        self.state = GameSessionState::Failed { request, error };
        self.pending_start = None;
    }

    pub fn clear(&mut self) {
        self.state = GameSessionState::NoSession;
        self.pending_start = None;
    }

    pub fn status(&self) -> Option<SessionStatus> {
        match &self.state {
            GameSessionState::Starting { request } => Some(SessionStatus {
                message: request.starting_message().to_owned(),
                ok: true,
            }),
            GameSessionState::Failed { error, .. } => Some(SessionStatus {
                message: error.message.clone(),
                ok: false,
            }),
            GameSessionState::NoSession | GameSessionState::Active { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameSessionState {
    NoSession,
    Starting {
        request: SessionStartRequest,
    },
    Active {
        session: ActiveSessionDescriptor,
    },
    Failed {
        request: SessionStartRequest,
        error: SessionFailure,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingSessionStart<P> {
    pub request: SessionStartRequest,
    pub payload: P,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionStartRequest {
    NewLocalWorld { seed: i64 },
    JoinRemote { endpoint: RemoteSessionEndpoint },
    Unknown,
}

impl SessionStartRequest {
    pub fn starting_message(&self) -> &'static str {
        match self {
            Self::NewLocalWorld { .. } => "Creating world...",
            Self::JoinRemote { .. } => "Connecting...",
            Self::Unknown => "Starting session...",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActiveSessionDescriptor {
    LocalWorld { seed: i64 },
    Remote { endpoint: RemoteSessionEndpoint },
}

impl ActiveSessionDescriptor {
    pub fn start_request(&self) -> SessionStartRequest {
        match self {
            Self::LocalWorld { seed } => SessionStartRequest::NewLocalWorld { seed: *seed },
            Self::Remote { endpoint } => SessionStartRequest::JoinRemote {
                endpoint: endpoint.clone(),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteSessionEndpoint {
    pub address: String,
}

impl RemoteSessionEndpoint {
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionFailure {
    pub message: String,
}

impl SessionFailure {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionStatus {
    pub message: String,
    pub ok: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_session_start_transitions_through_loading_to_active() {
        let mut coordinator = GameSessionCoordinator::new();

        coordinator.request_start(SessionStartRequest::NewLocalWorld { seed: 42 }, "payload");

        assert_eq!(
            coordinator.state(),
            &GameSessionState::Starting {
                request: SessionStartRequest::NewLocalWorld { seed: 42 }
            }
        );
        assert_eq!(
            coordinator.status(),
            Some(SessionStatus {
                message: "Creating world...".to_owned(),
                ok: true,
            })
        );
        assert_eq!(
            coordinator.take_pending_start(),
            Some(PendingSessionStart {
                request: SessionStartRequest::NewLocalWorld { seed: 42 },
                payload: "payload",
            })
        );

        coordinator.complete_start(ActiveSessionDescriptor::LocalWorld { seed: 42 });

        assert_eq!(
            coordinator.state(),
            &GameSessionState::Active {
                session: ActiveSessionDescriptor::LocalWorld { seed: 42 }
            }
        );
        assert_eq!(coordinator.status(), None);
    }

    #[test]
    fn remote_session_start_uses_connecting_status_and_records_failure() {
        let mut coordinator = GameSessionCoordinator::<()>::new();
        let request = SessionStartRequest::JoinRemote {
            endpoint: RemoteSessionEndpoint::new("127.0.0.1:25565"),
        };

        coordinator.begin_start(request.clone());
        assert_eq!(
            coordinator.status(),
            Some(SessionStatus {
                message: "Connecting...".to_owned(),
                ok: true,
            })
        );

        coordinator.fail_start(SessionFailure::new("Connection failed"));

        assert_eq!(
            coordinator.state(),
            &GameSessionState::Failed {
                request,
                error: SessionFailure::new("Connection failed"),
            }
        );
        assert_eq!(
            coordinator.status(),
            Some(SessionStatus {
                message: "Connection failed".to_owned(),
                ok: false,
            })
        );
    }

    #[test]
    fn clear_drops_pending_start() {
        let mut coordinator = GameSessionCoordinator::new();

        coordinator.request_start(SessionStartRequest::NewLocalWorld { seed: 7 }, 9);
        coordinator.clear();

        assert_eq!(coordinator.state(), &GameSessionState::NoSession);
        assert_eq!(coordinator.take_pending_start(), None);
    }
}
