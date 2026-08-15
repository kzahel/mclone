use std::path::{Path, PathBuf};

use mclone_server::{StarterContentDescriptor, WorldGenerationProfile};

use crate::world_catalog::{
    DEFAULT_LOCAL_WORLD_GENERATION_PROFILE, LocalWorldCreateOptions, LocalWorldId,
    LocalWorldSummary,
};

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

    pub fn pending_start(&self) -> Option<&PendingSessionStart<P>> {
        self.pending_start.as_ref()
    }

    pub fn complete_start(&mut self, session: ActiveSessionDescriptor) {
        self.state = GameSessionState::Active { session };
        self.pending_start = None;
    }

    pub fn apply_start_result<S>(&mut self, result: &SessionStartResult<S>) {
        match result {
            Ok(started) => self.complete_start(started.descriptor.clone()),
            Err(error) => self.fail_start(error.clone()),
        }
    }

    pub fn start_pending_with<S>(
        &mut self,
        start: impl FnOnce(PendingSessionStart<P>) -> SessionStartResult<S>,
    ) -> Option<SessionStartResult<S>> {
        let pending = self.take_pending_start()?;
        let result = start(pending);
        self.apply_start_result(&result);
        Some(result)
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

    pub fn start_phase(&self) -> Option<SessionStartPhase> {
        match &self.state {
            GameSessionState::Starting { request } | GameSessionState::Failed { request, .. } => {
                Some(request.start_phase())
            }
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

pub type SessionStartResult<S> = Result<StartedGameSession<S>, SessionFailure>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartedGameSession<S> {
    descriptor: ActiveSessionDescriptor,
    session: S,
}

impl<S> StartedGameSession<S> {
    pub fn new(descriptor: ActiveSessionDescriptor, session: S) -> Self {
        Self {
            descriptor,
            session,
        }
    }

    pub fn descriptor(&self) -> &ActiveSessionDescriptor {
        &self.descriptor
    }

    pub fn session(&self) -> &S {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut S {
        &mut self.session
    }

    pub fn into_parts(self) -> (ActiveSessionDescriptor, S) {
        (self.descriptor, self.session)
    }

    pub fn map_session<T>(self, map: impl FnOnce(S) -> T) -> StartedGameSession<T> {
        StartedGameSession {
            descriptor: self.descriptor,
            session: map(self.session),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionStartRequest {
    CreateLocalWorld { options: LocalWorldCreateOptions },
    OpenLocalWorld { id: LocalWorldId },
    JoinRemote { endpoint: RemoteSessionEndpoint },
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionStartPhase {
    CreatingWorld,
    PlanningHomestead,
    LoadingWorld,
    Connecting,
    StartingSession,
}

impl SessionStartPhase {
    pub const fn message(self) -> &'static str {
        match self {
            Self::CreatingWorld => "Creating world...",
            Self::PlanningHomestead => "Planning homestead site...",
            Self::LoadingWorld => "Loading world...",
            Self::Connecting => "Connecting...",
            Self::StartingSession => "Starting session...",
        }
    }
}

impl SessionStartRequest {
    pub fn new_seed_local_world(seed: i64) -> Self {
        Self::new_seed_local_world_with_generation_profile(
            seed,
            DEFAULT_LOCAL_WORLD_GENERATION_PROFILE,
        )
    }

    pub fn new_seed_local_world_with_generation_profile(
        seed: i64,
        profile: WorldGenerationProfile,
    ) -> Self {
        Self::new_seed_local_world_with_generation_profile_and_starter_content(
            seed,
            profile,
            StarterContentDescriptor::Wild,
        )
    }

    pub fn new_seed_local_world_with_generation_profile_and_starter_content(
        seed: i64,
        profile: WorldGenerationProfile,
        starter_content: StarterContentDescriptor,
    ) -> Self {
        Self::CreateLocalWorld {
            options: LocalWorldCreateOptions::new(
                default_seed_local_world_display_name(seed),
                seed,
            )
            .expect("default seed local world display name is valid")
            .with_world_generation_profile(profile)
            .with_starter_content(starter_content),
        }
    }

    pub fn create_local_world(options: LocalWorldCreateOptions) -> Self {
        Self::CreateLocalWorld { options }
    }

    pub fn open_local_world(id: LocalWorldId) -> Self {
        Self::OpenLocalWorld { id }
    }

    pub const fn start_phase(&self) -> SessionStartPhase {
        match self {
            Self::CreateLocalWorld { options }
                if matches!(
                    options.starter_content,
                    StarterContentDescriptor::IntroHomesteadV1
                ) =>
            {
                SessionStartPhase::PlanningHomestead
            }
            Self::CreateLocalWorld { .. } => SessionStartPhase::CreatingWorld,
            Self::OpenLocalWorld { .. } => SessionStartPhase::LoadingWorld,
            Self::JoinRemote { .. } => SessionStartPhase::Connecting,
            Self::Unknown => SessionStartPhase::StartingSession,
        }
    }

    pub fn starting_message(&self) -> &'static str {
        self.start_phase().message()
    }

    pub fn default_failure_message(&self) -> &'static str {
        match self {
            Self::CreateLocalWorld { options }
                if matches!(
                    options.starter_content,
                    StarterContentDescriptor::IntroHomesteadV1
                ) =>
            {
                "Homestead planning failed; choose another seed or Wild Start"
            }
            Self::CreateLocalWorld { .. } => "World creation failed; see log",
            Self::OpenLocalWorld { .. } => "World load failed; see log",
            Self::JoinRemote { .. } => "Connection failed; see log",
            Self::Unknown => "Session start failed; see log",
        }
    }

    pub fn active_descriptor(&self) -> Option<ActiveSessionDescriptor> {
        match self {
            Self::CreateLocalWorld { options } => Some(ActiveSessionDescriptor::LocalWorld {
                seed: options.seed,
                id: options.requested_id.clone(),
                display_name: Some(options.display_name.clone()),
            }),
            Self::OpenLocalWorld { .. } => None,
            Self::JoinRemote { endpoint } => Some(ActiveSessionDescriptor::Remote {
                endpoint: endpoint.clone(),
            }),
            Self::Unknown => None,
        }
    }

    pub fn local_seed(&self) -> Option<i64> {
        match self {
            Self::CreateLocalWorld { options } => Some(options.seed),
            Self::OpenLocalWorld { .. } | Self::JoinRemote { .. } | Self::Unknown => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRuntimeKind {
    Local,
    Remote,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionStartPayload<O> {
    pub runtime_kind: SessionRuntimeKind,
    pub options: O,
    pub descriptor: ActiveSessionDescriptor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionStartPlan<O> {
    pub request: SessionStartRequest,
    pub payload: SessionStartPayload<O>,
}

/// Resolve every session request variant into host-specific scene/runtime
/// options through one exhaustive shared dispatch.
///
/// Hosts supply only the construction details for transient local, catalog
/// local, and remote options. This keeps request classification and descriptor
/// selection out of platform/app loops.
pub fn plan_session_start<O, E>(
    request: SessionStartRequest,
    create_local: impl FnOnce(&LocalWorldCreateOptions) -> Result<O, E>,
    open_local: impl FnOnce(&LocalWorldId) -> Result<(O, ActiveSessionDescriptor), E>,
    join_remote: impl FnOnce(&RemoteSessionEndpoint) -> Result<O, E>,
    unknown_error: impl FnOnce() -> E,
) -> Result<SessionStartPlan<O>, E> {
    let payload = match &request {
        SessionStartRequest::CreateLocalWorld { options } => SessionStartPayload {
            runtime_kind: SessionRuntimeKind::Local,
            options: create_local(options)?,
            descriptor: request
                .active_descriptor()
                .expect("create-local request always has an active descriptor"),
        },
        SessionStartRequest::OpenLocalWorld { id } => {
            let (options, descriptor) = open_local(id)?;
            SessionStartPayload {
                runtime_kind: SessionRuntimeKind::Local,
                options,
                descriptor,
            }
        }
        SessionStartRequest::JoinRemote { endpoint } => SessionStartPayload {
            runtime_kind: SessionRuntimeKind::Remote,
            options: join_remote(endpoint)?,
            descriptor: request
                .active_descriptor()
                .expect("join-remote request always has an active descriptor"),
        },
        SessionStartRequest::Unknown => return Err(unknown_error()),
    };
    Ok(SessionStartPlan { request, payload })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionStorageIntent {
    seed: Option<i64>,
    world_generation_profile: WorldGenerationProfile,
    starter_content: StarterContentDescriptor,
    remote_addr: Option<String>,
    world_dir: Option<PathBuf>,
    suppress_adaptive_chunk_publication_budget: bool,
}

impl SessionStorageIntent {
    pub fn transient_local_world(seed: i64) -> Self {
        Self {
            seed: Some(seed),
            world_generation_profile: DEFAULT_LOCAL_WORLD_GENERATION_PROFILE,
            starter_content: StarterContentDescriptor::Wild,
            remote_addr: None,
            world_dir: None,
            suppress_adaptive_chunk_publication_budget: false,
        }
    }

    pub fn transient_local_world_with_generation_profile(
        seed: i64,
        profile: WorldGenerationProfile,
    ) -> Self {
        Self {
            seed: Some(seed),
            world_generation_profile: profile,
            starter_content: StarterContentDescriptor::Wild,
            remote_addr: None,
            world_dir: None,
            suppress_adaptive_chunk_publication_budget: false,
        }
    }

    pub fn transient_local_world_with_generation_profile_and_starter_content(
        seed: i64,
        profile: WorldGenerationProfile,
        starter_content: StarterContentDescriptor,
    ) -> Self {
        Self {
            seed: Some(seed),
            world_generation_profile: profile,
            starter_content,
            remote_addr: None,
            world_dir: None,
            suppress_adaptive_chunk_publication_budget: false,
        }
    }

    pub fn catalog_world(summary: &LocalWorldSummary, world_dir: PathBuf) -> Self {
        Self {
            seed: Some(summary.seed),
            world_generation_profile: summary.world_generation_profile,
            starter_content: summary.starter_content,
            remote_addr: None,
            world_dir: Some(world_dir),
            suppress_adaptive_chunk_publication_budget: false,
        }
    }

    pub fn remote_session(remote_addr: impl Into<String>) -> Self {
        Self {
            seed: None,
            world_generation_profile: WorldGenerationProfile::default(),
            starter_content: StarterContentDescriptor::Wild,
            remote_addr: Some(remote_addr.into()),
            world_dir: None,
            suppress_adaptive_chunk_publication_budget: true,
        }
    }

    pub fn seed(&self) -> Option<i64> {
        self.seed
    }

    pub fn remote_addr(&self) -> Option<&str> {
        self.remote_addr.as_deref()
    }

    pub const fn world_generation_profile(&self) -> WorldGenerationProfile {
        self.world_generation_profile
    }

    pub const fn starter_content(&self) -> StarterContentDescriptor {
        self.starter_content
    }

    pub fn world_dir(&self) -> Option<&Path> {
        self.world_dir.as_deref()
    }

    pub fn suppress_adaptive_chunk_publication_budget(&self) -> bool {
        self.suppress_adaptive_chunk_publication_budget
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActiveSessionDescriptor {
    LocalWorld {
        seed: i64,
        id: Option<LocalWorldId>,
        display_name: Option<String>,
    },
    Remote {
        endpoint: RemoteSessionEndpoint,
    },
}

impl ActiveSessionDescriptor {
    pub fn new_seed_local_world(seed: i64) -> Self {
        Self::LocalWorld {
            seed,
            id: None,
            display_name: Some(default_seed_local_world_display_name(seed)),
        }
    }

    pub fn from_local_world_summary(summary: &LocalWorldSummary) -> Self {
        Self::LocalWorld {
            seed: summary.seed,
            id: Some(summary.id.clone()),
            display_name: Some(summary.display_name.clone()),
        }
    }

    pub fn start_request(&self) -> SessionStartRequest {
        match self {
            Self::LocalWorld { seed, id, .. } => id.clone().map_or_else(
                || SessionStartRequest::new_seed_local_world(*seed),
                SessionStartRequest::open_local_world,
            ),
            Self::Remote { endpoint } => SessionStartRequest::JoinRemote {
                endpoint: endpoint.clone(),
            },
        }
    }

    pub fn local_seed(&self) -> Option<i64> {
        match self {
            Self::LocalWorld { seed, .. } => Some(*seed),
            Self::Remote { .. } => None,
        }
    }

    pub fn local_world_id(&self) -> Option<&LocalWorldId> {
        match self {
            Self::LocalWorld { id, .. } => id.as_ref(),
            Self::Remote { .. } => None,
        }
    }
}

pub fn default_seed_local_world_display_name(seed: i64) -> String {
    format!("Seed {seed}")
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
    pub kind: SessionFailureKind,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionFailureKind {
    General,
    HomesteadPlanning,
}

impl SessionFailure {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            kind: SessionFailureKind::General,
            message: message.into(),
        }
    }

    pub fn for_request(request: &SessionStartRequest, message: impl Into<String>) -> Self {
        let kind = match request.start_phase() {
            SessionStartPhase::PlanningHomestead => SessionFailureKind::HomesteadPlanning,
            _ => SessionFailureKind::General,
        };
        Self {
            kind,
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
        let request = SessionStartRequest::new_seed_local_world(42);
        let descriptor = ActiveSessionDescriptor::new_seed_local_world(42);

        coordinator.request_start(request.clone(), "payload");

        assert_eq!(
            coordinator.state(),
            &GameSessionState::Starting {
                request: request.clone()
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
                request,
                payload: "payload",
            })
        );

        coordinator.complete_start(descriptor.clone());

        assert_eq!(
            coordinator.state(),
            &GameSessionState::Active {
                session: descriptor
            }
        );
        assert_eq!(coordinator.status(), None);
    }

    #[test]
    fn homestead_start_projects_typed_planning_progress_and_failure() {
        let mut coordinator = GameSessionCoordinator::<()>::new();
        let request =
            SessionStartRequest::new_seed_local_world_with_generation_profile_and_starter_content(
                0,
                WorldGenerationProfile::McloneOverworldV1,
                StarterContentDescriptor::IntroHomesteadV1,
            );

        coordinator.begin_start(request.clone());
        assert_eq!(
            coordinator.start_phase(),
            Some(SessionStartPhase::PlanningHomestead)
        );
        assert_eq!(
            coordinator.status(),
            Some(SessionStatus {
                message: "Planning homestead site...".to_owned(),
                ok: true,
            })
        );

        let failure = SessionFailure::for_request(&request, request.default_failure_message());
        assert_eq!(failure.kind, SessionFailureKind::HomesteadPlanning);
        assert_eq!(
            failure.message,
            "Homestead planning failed; choose another seed or Wild Start"
        );
        coordinator.fail_start(failure.clone());
        assert_eq!(
            coordinator.start_phase(),
            Some(SessionStartPhase::PlanningHomestead)
        );
        assert_eq!(
            coordinator.state(),
            &GameSessionState::Failed {
                request,
                error: failure,
            }
        );
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
    fn start_result_updates_state_for_success_and_failure() {
        let mut coordinator = GameSessionCoordinator::<()>::new();

        coordinator.begin_start(SessionStartRequest::new_seed_local_world(3));
        let success: SessionStartResult<&str> = Ok(StartedGameSession::new(
            ActiveSessionDescriptor::new_seed_local_world(3),
            "started",
        ));
        coordinator.apply_start_result(&success);

        assert_eq!(
            coordinator.state(),
            &GameSessionState::Active {
                session: ActiveSessionDescriptor::new_seed_local_world(3)
            }
        );
        assert_eq!(
            success.unwrap().into_parts(),
            (ActiveSessionDescriptor::new_seed_local_world(3), "started")
        );

        let failed_request = SessionStartRequest::new_seed_local_world(4);
        coordinator.begin_start(failed_request.clone());
        let failure: SessionStartResult<()> = Err(SessionFailure::new("nope"));
        coordinator.apply_start_result(&failure);

        assert_eq!(
            coordinator.state(),
            &GameSessionState::Failed {
                request: failed_request,
                error: SessionFailure::new("nope"),
            }
        );
    }

    #[test]
    fn start_pending_with_consumes_pending_and_applies_result() {
        let mut coordinator = GameSessionCoordinator::new();
        let request = SessionStartRequest::new_seed_local_world(8);
        let descriptor = ActiveSessionDescriptor::new_seed_local_world(8);

        coordinator.request_start(request.clone(), 11);
        let result = coordinator.start_pending_with(|pending| {
            assert_eq!(pending.request, request);
            assert_eq!(pending.payload, 11);
            Ok(StartedGameSession::new(descriptor.clone(), 12))
        });

        assert_eq!(
            result,
            Some(Ok(StartedGameSession::new(descriptor.clone(), 12)))
        );
        assert_eq!(coordinator.take_pending_start(), None);
        assert_eq!(
            coordinator.state(),
            &GameSessionState::Active {
                session: descriptor
            }
        );
    }

    #[test]
    fn session_start_request_derives_descriptor_and_default_messages() {
        let endpoint = RemoteSessionEndpoint::new("localhost:25565");
        let local = SessionStartRequest::new_seed_local_world(99);
        let remote = SessionStartRequest::JoinRemote {
            endpoint: endpoint.clone(),
        };

        assert_eq!(
            local.active_descriptor(),
            Some(ActiveSessionDescriptor::new_seed_local_world(99))
        );
        let SessionStartRequest::CreateLocalWorld { options } = &local else {
            panic!("seed-local request should create a local world");
        };
        assert_eq!(
            options.world_generation_profile,
            WorldGenerationProfile::McloneOverworldV1
        );
        assert_eq!(
            local.default_failure_message(),
            "World creation failed; see log"
        );
        assert_eq!(
            remote.active_descriptor(),
            Some(ActiveSessionDescriptor::Remote { endpoint })
        );
        assert_eq!(
            remote.default_failure_message(),
            "Connection failed; see log"
        );
        assert_eq!(SessionStartRequest::Unknown.active_descriptor(), None);
    }

    #[test]
    fn create_local_world_request_carries_requested_catalog_identity() {
        let id = LocalWorldId::new("my-world").unwrap();
        let options = LocalWorldCreateOptions::new("My World", 123)
            .unwrap()
            .with_requested_id(id.clone());
        let request = SessionStartRequest::create_local_world(options);

        assert_eq!(
            request.active_descriptor(),
            Some(ActiveSessionDescriptor::LocalWorld {
                seed: 123,
                id: Some(id),
                display_name: Some("My World".to_owned()),
            })
        );
    }

    #[test]
    fn open_local_world_request_waits_for_catalog_summary_descriptor() {
        let id = LocalWorldId::new("my-world").unwrap();
        let request = SessionStartRequest::open_local_world(id.clone());
        let summary = LocalWorldSummary::new(id.clone(), "My World", 456, 100).unwrap();
        let descriptor = ActiveSessionDescriptor::from_local_world_summary(&summary);

        assert_eq!(request.starting_message(), "Loading world...");
        assert_eq!(
            request.default_failure_message(),
            "World load failed; see log"
        );
        assert_eq!(request.active_descriptor(), None);
        assert_eq!(
            descriptor,
            ActiveSessionDescriptor::LocalWorld {
                seed: 456,
                id: Some(id),
                display_name: Some("My World".to_owned()),
            }
        );
        assert_eq!(descriptor.start_request(), request);
    }

    #[test]
    fn shared_session_plan_classifies_all_supported_request_kinds() {
        let create = plan_session_start(
            SessionStartRequest::new_seed_local_world(12),
            |options| Ok::<_, &'static str>(format!("local:{}", options.seed)),
            |_| Err("unexpected open"),
            |_| Err("unexpected remote"),
            || "unknown",
        )
        .unwrap();
        assert_eq!(create.payload.runtime_kind, SessionRuntimeKind::Local);
        assert_eq!(create.payload.options, "local:12");
        assert_eq!(
            create.payload.descriptor,
            ActiveSessionDescriptor::new_seed_local_world(12)
        );

        let id = LocalWorldId::new("planned-world").unwrap();
        let summary = LocalWorldSummary::new(id.clone(), "Planned World", 34, 1).unwrap();
        let expected_descriptor = ActiveSessionDescriptor::from_local_world_summary(&summary);
        let open = plan_session_start(
            SessionStartRequest::open_local_world(id.clone()),
            |_| Err("unexpected create"),
            |opened_id| {
                assert_eq!(opened_id, &id);
                Ok::<_, &'static str>(("catalog".to_owned(), expected_descriptor.clone()))
            },
            |_| Err("unexpected remote"),
            || "unknown",
        )
        .unwrap();
        assert_eq!(open.payload.runtime_kind, SessionRuntimeKind::Local);
        assert_eq!(open.payload.options, "catalog");
        assert_eq!(open.payload.descriptor, expected_descriptor);

        let endpoint = RemoteSessionEndpoint::new("127.0.0.1:25565");
        let remote = plan_session_start(
            SessionStartRequest::JoinRemote {
                endpoint: endpoint.clone(),
            },
            |_| Err("unexpected create"),
            |_| Err("unexpected open"),
            |joined| Ok::<_, &'static str>(format!("remote:{}", joined.address)),
            || "unknown",
        )
        .unwrap();
        assert_eq!(remote.payload.runtime_kind, SessionRuntimeKind::Remote);
        assert_eq!(remote.payload.options, "remote:127.0.0.1:25565");
        assert_eq!(
            remote.payload.descriptor,
            ActiveSessionDescriptor::Remote { endpoint }
        );

        let unknown = plan_session_start(
            SessionStartRequest::Unknown,
            |_| Ok::<_, &'static str>(String::new()),
            |_| unreachable!(),
            |_| unreachable!(),
            || "unknown session",
        );
        assert_eq!(unknown.unwrap_err(), "unknown session");
    }

    #[test]
    fn session_storage_intent_encodes_local_catalog_and_remote_policy() {
        let local = SessionStorageIntent::transient_local_world(123);
        assert_eq!(local.seed(), Some(123));
        assert_eq!(
            local.world_generation_profile(),
            WorldGenerationProfile::McloneOverworldV1
        );
        assert_eq!(local.remote_addr(), None);
        assert_eq!(local.world_dir(), None);
        assert!(!local.suppress_adaptive_chunk_publication_budget());

        let id = LocalWorldId::new("my-world").unwrap();
        let summary = LocalWorldSummary::new(id, "My World", 456, 100).unwrap();
        let world_dir = PathBuf::from("/tmp/mclone-worlds/my-world");
        let catalog = SessionStorageIntent::catalog_world(&summary, world_dir.clone());
        assert_eq!(catalog.seed(), Some(456));
        assert_eq!(catalog.remote_addr(), None);
        assert_eq!(catalog.world_dir(), Some(world_dir.as_path()));
        assert!(!catalog.suppress_adaptive_chunk_publication_budget());

        let remote = SessionStorageIntent::remote_session("127.0.0.1:25565");
        assert_eq!(remote.seed(), None);
        assert_eq!(remote.remote_addr(), Some("127.0.0.1:25565"));
        assert_eq!(remote.world_dir(), None);
        assert!(remote.suppress_adaptive_chunk_publication_budget());
    }

    #[test]
    fn clear_drops_pending_start() {
        let mut coordinator = GameSessionCoordinator::new();

        coordinator.request_start(SessionStartRequest::new_seed_local_world(7), 9);
        coordinator.clear();

        assert_eq!(coordinator.state(), &GameSessionState::NoSession);
        assert_eq!(coordinator.take_pending_start(), None);
    }
}
