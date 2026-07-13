//! Shared typed world-catalog operation executor.
//!
//! The host issues tokened operations and folds typed completions through the
//! shared catalog controller. It does **not** own filesystem or IndexedDB work,
//! and it does not apply session/GPU lifecycle effects.
//!
//! Native assembly supplies an immediate filesystem executor. Browser assembly
//! can supply a deferred IndexedDB executor without changing controller policy.

use std::collections::VecDeque;

use crate::client_catalog_policy::{
    ClientCatalogController, ClientCatalogEffects, ClientCatalogRequest,
};
use crate::platform_operation::{
    DeferredPlatformOperationHandle, PlatformOperation, PlatformOperationCompletion,
    PlatformOperationExecutor, PlatformOperationResolution, PlatformOperationService,
    deferred_platform_operation_executor,
};
#[cfg(test)]
use crate::world_catalog::WorldCatalogCapabilities;
use crate::world_catalog::{LocalWorldId, WorldCatalog, WorldCatalogError, WorldCatalogResponse};
#[cfg(test)]
use mclone_ui::WorldCatalogUiStatus;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldCatalogOperation {
    pub request: ClientCatalogRequest,
    pub active_world: Option<LocalWorldId>,
}

#[derive(Debug)]
pub struct ImmediateWorldCatalogExecutor {
    catalog: Box<dyn WorldCatalog>,
    ready: VecDeque<PlatformOperationCompletion<WorldCatalogResponse, WorldCatalogError>>,
}

impl ImmediateWorldCatalogExecutor {
    pub fn new(catalog: Box<dyn WorldCatalog>) -> Self {
        Self {
            catalog,
            ready: VecDeque::new(),
        }
    }
}

impl PlatformOperationExecutor<WorldCatalogOperation, WorldCatalogResponse, WorldCatalogError>
    for ImmediateWorldCatalogExecutor
{
    fn submit(&mut self, operation: PlatformOperation<WorldCatalogOperation>) {
        let result = self.catalog.handle_request(
            operation.kind.request.request.clone(),
            operation.kind.active_world.as_ref(),
        );
        self.ready.push_back(PlatformOperationCompletion {
            token: operation.token,
            result,
        });
    }

    fn try_recv_completion(
        &mut self,
    ) -> Option<PlatformOperationCompletion<WorldCatalogResponse, WorldCatalogError>> {
        self.ready.pop_front()
    }
}

impl std::fmt::Debug for dyn WorldCatalog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("WorldCatalog")
    }
}

#[derive(Debug)]
pub struct WorldCatalogOperationService {
    operations: PlatformOperationService<
        WorldCatalogOperation,
        ClientCatalogRequest,
        WorldCatalogResponse,
        WorldCatalogError,
    >,
}

impl WorldCatalogOperationService {
    pub fn new(
        executor: Box<
            dyn PlatformOperationExecutor<
                    WorldCatalogOperation,
                    WorldCatalogResponse,
                    WorldCatalogError,
                >,
        >,
    ) -> Self {
        Self {
            operations: PlatformOperationService::new(executor),
        }
    }

    pub fn immediate(catalog: Box<dyn WorldCatalog>) -> Self {
        Self::new(Box::new(ImmediateWorldCatalogExecutor::new(catalog)))
    }

    pub fn deferred() -> (Self, DeferredWorldCatalogOperationHandle) {
        let (executor, handle) = deferred_platform_operation_executor();
        (Self::new(Box::new(executor)), handle)
    }

    pub fn submit(&mut self, request: ClientCatalogRequest, active_world: Option<LocalWorldId>) {
        self.operations.issue(
            WorldCatalogOperation {
                request: request.clone(),
                active_world,
            },
            request,
        );
    }

    pub fn poll(&mut self, controller: &mut ClientCatalogController) -> ClientCatalogEffects {
        let mut effects = ClientCatalogEffects::default();
        for resolution in self.operations.poll() {
            let next = match resolution {
                PlatformOperationResolution::Applied { kind, value, .. } => {
                    controller.apply_catalog_response(kind.request.id, value)
                }
                PlatformOperationResolution::Failed {
                    error,
                    failure_restore,
                    ..
                } => {
                    log::warn!("world catalog action failed: {error}");
                    controller.apply_catalog_error(failure_restore.id, error)
                }
                PlatformOperationResolution::Stale(_)
                | PlatformOperationResolution::Duplicate(_)
                | PlatformOperationResolution::Unknown(_) => continue,
            };
            effects.session_starts.extend(next.session_starts);
            effects.catalog_requests.extend(next.catalog_requests);
        }
        effects
    }

    pub fn pending_len(&self) -> usize {
        self.operations.pending_len()
    }

    pub fn begin_epoch(&mut self) -> Vec<ClientCatalogRequest> {
        self.operations
            .begin_epoch()
            .into_iter()
            .map(|cancelled| cancelled.failure_restore)
            .collect()
    }
}

pub type DeferredWorldCatalogOperationHandle =
    DeferredPlatformOperationHandle<WorldCatalogOperation, WorldCatalogResponse, WorldCatalogError>;

/// Run one `ClientCatalogRequest` against a native catalog backend and fold the
/// result back through the shared `ClientCatalogController`.
///
/// When `catalog` is `None` (genuinely transient targets: headless, initial
/// local session), the request fails with the shared "Persistent worlds
/// unavailable" error, exactly as the per-app copies did.
#[cfg(test)]
fn execute_world_catalog_request(
    catalog: Option<&dyn WorldCatalog>,
    controller: &mut ClientCatalogController,
    active_world: Option<&LocalWorldId>,
    request: ClientCatalogRequest,
) -> ClientCatalogEffects {
    let Some(catalog) = catalog else {
        let error = WorldCatalogError::unsupported("Persistent worlds unavailable");
        log::warn!("world catalog action failed: {error}");
        return controller.apply_catalog_error(request.id, error);
    };
    match catalog.handle_request(request.request, active_world) {
        Ok(response) => controller.apply_catalog_response(request.id, response),
        Err(error) => {
            log::warn!("world catalog action failed: {error}");
            controller.apply_catalog_error(request.id, error)
        }
    }
}

/// Refresh a native catalog controller from the current backend list state.
#[cfg(test)]
fn refresh_world_catalog_controller(
    catalog: Option<&dyn WorldCatalog>,
    controller: &mut ClientCatalogController,
    status: WorldCatalogUiStatus,
    log_context: &str,
) {
    let Some(catalog) = catalog else {
        controller.set_worlds(WorldCatalogCapabilities::default(), Vec::new(), status);
        return;
    };

    match catalog.list_worlds() {
        Ok(worlds) => {
            controller.set_worlds(catalog.capabilities(), worlds, status);
        }
        Err(error) => {
            log::warn!("failed to refresh {log_context} world catalog: {error}");
            controller.set_worlds(
                catalog.capabilities(),
                Vec::new(),
                WorldCatalogUiStatus::new(&error.message, false),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::client_catalog_policy::ClientCatalogActionContext;
    use crate::world_catalog::{
        LocalWorldSummary, WorldCatalogCapabilities, WorldCatalogErrorKind, WorldCatalogRequest,
        WorldCatalogRequestId, WorldCatalogResponse, WorldCatalogResult,
        sort_local_world_summaries, validate_delete_inactive_world, world_not_found,
    };
    use mclone_ui::GameUiAction;

    /// Deterministic in-memory `WorldCatalog` for behavior-equivalence testing.
    ///
    /// Mirrors `NativeWorldCatalog::handle_request` semantics without touching
    /// the filesystem or the clock, and records the last outcome so tests can
    /// assert the exact `WorldCatalogResponse` produced at each step.
    struct FakeWorldCatalog {
        worlds: RefCell<Vec<LocalWorldSummary>>,
        last: RefCell<Option<WorldCatalogResult<WorldCatalogResponse>>>,
    }

    impl FakeWorldCatalog {
        fn new() -> Self {
            Self {
                worlds: RefCell::new(Vec::new()),
                last: RefCell::new(None),
            }
        }

        fn take_last(&self) -> WorldCatalogResult<WorldCatalogResponse> {
            self.last
                .borrow_mut()
                .take()
                .expect("catalog handled at least one request")
        }

        fn record(
            &self,
            result: WorldCatalogResult<WorldCatalogResponse>,
        ) -> WorldCatalogResult<WorldCatalogResponse> {
            *self.last.borrow_mut() = Some(result.clone());
            result
        }

        fn summary(&self, id: &LocalWorldId) -> Option<LocalWorldSummary> {
            self.worlds
                .borrow()
                .iter()
                .find(|summary| &summary.id == id)
                .cloned()
        }
    }

    impl WorldCatalog for FakeWorldCatalog {
        fn capabilities(&self) -> WorldCatalogCapabilities {
            WorldCatalogCapabilities::persistent_local()
        }

        fn list_worlds(&self) -> WorldCatalogResult<Vec<LocalWorldSummary>> {
            let mut worlds = self.worlds.borrow().clone();
            sort_local_world_summaries(&mut worlds);
            Ok(worlds)
        }

        fn handle_request(
            &self,
            request: WorldCatalogRequest,
            active_world: Option<&LocalWorldId>,
        ) -> WorldCatalogResult<WorldCatalogResponse> {
            match request {
                WorldCatalogRequest::ListWorlds => {
                    self.record(Ok(WorldCatalogResponse::WorldList {
                        capabilities: self.capabilities(),
                        worlds: self.list_worlds()?,
                    }))
                }
                WorldCatalogRequest::CreateWorld { options } => {
                    let existing = self
                        .worlds
                        .borrow()
                        .iter()
                        .map(|summary| summary.id.clone())
                        .collect::<Vec<_>>();
                    let id = match options.resolve_id(existing.iter()) {
                        Ok(id) => id,
                        Err(error) => return self.record(Err(error)),
                    };
                    let mut summary =
                        match LocalWorldSummary::new(id, options.display_name, options.seed, 1_000)
                        {
                            Ok(summary) => summary,
                            Err(error) => return self.record(Err(error)),
                        };
                    summary.world_generation_profile = options.world_generation_profile;
                    summary.last_played_unix_millis = Some(1_000);
                    self.worlds.borrow_mut().push(summary.clone());
                    self.record(Ok(WorldCatalogResponse::WorldCreated { summary }))
                }
                WorldCatalogRequest::OpenWorld { id } => match self.summary(&id) {
                    Some(mut summary) => {
                        summary.last_played_unix_millis = Some(2_000);
                        if let Some(existing) = self
                            .worlds
                            .borrow_mut()
                            .iter_mut()
                            .find(|world| world.id == id)
                        {
                            *existing = summary.clone();
                        }
                        self.record(Ok(WorldCatalogResponse::WorldOpened { summary }))
                    }
                    None => self.record(Err(world_not_found(&id))),
                },
                WorldCatalogRequest::DeleteWorld { id } => match self.summary(&id) {
                    Some(_) => {
                        if let Err(error) = validate_delete_inactive_world(&id, active_world) {
                            return self.record(Err(error));
                        }
                        self.worlds.borrow_mut().retain(|world| world.id != id);
                        self.record(Ok(WorldCatalogResponse::WorldDeleted { id }))
                    }
                    None => self.record(Err(world_not_found(&id))),
                },
            }
        }
    }

    fn persistent_controller() -> ClientCatalogController {
        let mut controller = ClientCatalogController::new();
        controller.set_capabilities(WorldCatalogCapabilities::persistent_local());
        controller
    }

    #[derive(Debug)]
    struct ListOnlyCatalog {
        world: LocalWorldSummary,
    }

    impl WorldCatalog for ListOnlyCatalog {
        fn capabilities(&self) -> WorldCatalogCapabilities {
            WorldCatalogCapabilities::persistent_local()
        }

        fn list_worlds(&self) -> WorldCatalogResult<Vec<LocalWorldSummary>> {
            Ok(vec![self.world.clone()])
        }

        fn handle_request(
            &self,
            request: WorldCatalogRequest,
            _active_world: Option<&LocalWorldId>,
        ) -> WorldCatalogResult<WorldCatalogResponse> {
            match request {
                WorldCatalogRequest::ListWorlds => Ok(WorldCatalogResponse::WorldList {
                    capabilities: self.capabilities(),
                    worlds: self.list_worlds()?,
                }),
                _ => Err(WorldCatalogError::unsupported("list-only test catalog")),
            }
        }
    }

    fn context(seed: i64) -> ClientCatalogActionContext {
        ClientCatalogActionContext {
            new_world_seed: seed,
        }
    }

    fn only_request(effects: ClientCatalogEffects) -> ClientCatalogRequest {
        assert!(effects.session_starts.is_empty());
        assert_eq!(effects.catalog_requests.len(), 1);
        effects.catalog_requests.into_iter().next().unwrap()
    }

    #[test]
    fn immediate_operation_service_folds_completion_through_shared_policy() {
        let world = LocalWorldSummary::new(
            LocalWorldId::new("operation-world").unwrap(),
            "Operation World",
            42,
            1_000,
        )
        .unwrap();
        let mut controller = persistent_controller();
        let request = only_request(controller.request_world_list());
        let mut service = WorldCatalogOperationService::immediate(Box::new(ListOnlyCatalog {
            world: world.clone(),
        }));

        service.submit(request, None);
        let effects = service.poll(&mut controller);

        assert_eq!(effects, ClientCatalogEffects::default());
        assert_eq!(service.pending_len(), 0);
        let state = controller.ui_state();
        assert_eq!(state.entry_count(), 1);
        assert_eq!(state.entries[0].as_ref().unwrap().seed, world.seed);
    }

    /// Behavior-equivalence tripwire (tactical 160, slices 1–2): drive the fixed
    /// sequence `ListWorlds empty → CreateWorld → ListWorlds → OpenWorld →
    /// DeleteWorld → error (open missing id)` through the single shared executor
    /// and assert both the exact `WorldCatalogResponse` and the resulting
    /// `ClientCatalogEffects` at each step. All three native consumers route
    /// through this function, so this is the shared proof that list/create/open/
    /// delete/error semantics and effect ordering are unchanged.
    #[test]
    fn golden_catalog_trace_through_shared_executor() {
        let catalog = FakeWorldCatalog::new();
        let mut controller = persistent_controller();

        // 1. ListWorlds against an empty catalog: WorldList with no worlds, and
        // no follow-up effects.
        let request = only_request(controller.request_world_list());
        let effects = execute_world_catalog_request(Some(&catalog), &mut controller, None, request);
        match catalog.take_last().unwrap() {
            WorldCatalogResponse::WorldList {
                capabilities,
                worlds,
            } => {
                assert_eq!(capabilities, WorldCatalogCapabilities::persistent_local());
                assert!(worlds.is_empty());
            }
            response => panic!("unexpected response: {response:?}"),
        }
        assert_eq!(effects, ClientCatalogEffects::default());

        // 2. CreateWorld: WorldCreated, and exactly one session start.
        let request = only_request(
            controller.apply_ui_action(GameUiAction::CreateCatalogWorld, context(1234)),
        );
        let effects = execute_world_catalog_request(Some(&catalog), &mut controller, None, request);
        let created = match catalog.take_last().unwrap() {
            WorldCatalogResponse::WorldCreated { summary } => summary,
            response => panic!("unexpected response: {response:?}"),
        };
        assert_eq!(created.id.as_str(), "new-world");
        assert_eq!(created.seed, 1234);
        assert!(effects.catalog_requests.is_empty());
        assert_eq!(effects.session_starts.len(), 1);
        assert_eq!(effects.session_starts[0].summary, created);

        // 3. ListWorlds now returns the created world; still no follow-up effects.
        let request = only_request(controller.request_world_list());
        let effects = execute_world_catalog_request(Some(&catalog), &mut controller, None, request);
        match catalog.take_last().unwrap() {
            WorldCatalogResponse::WorldList { worlds, .. } => {
                assert_eq!(worlds.len(), 1);
                assert_eq!(worlds[0].id, created.id);
            }
            response => panic!("unexpected response: {response:?}"),
        }
        assert_eq!(effects, ClientCatalogEffects::default());

        // 4. OpenWorld: WorldOpened, and exactly one session start.
        let ui_id = controller
            .ui_state()
            .selected
            .expect("created world selected");
        let request =
            only_request(controller.apply_ui_action(GameUiAction::OpenWorld(ui_id), context(0)));
        let effects = execute_world_catalog_request(Some(&catalog), &mut controller, None, request);
        let opened = match catalog.take_last().unwrap() {
            WorldCatalogResponse::WorldOpened { summary } => summary,
            response => panic!("unexpected response: {response:?}"),
        };
        assert_eq!(opened.id, created.id);
        assert_eq!(opened.last_played_unix_millis, Some(2_000));
        assert!(effects.catalog_requests.is_empty());
        assert_eq!(effects.session_starts.len(), 1);
        assert_eq!(effects.session_starts[0].summary, opened);

        // 5. DeleteWorld: WorldDeleted, no follow-up effects, success status.
        let ui_id = controller
            .ui_state()
            .selected
            .expect("created world selected");
        let request =
            only_request(controller.apply_ui_action(GameUiAction::DeleteWorld(ui_id), context(0)));
        let effects = execute_world_catalog_request(Some(&catalog), &mut controller, None, request);
        match catalog.take_last().unwrap() {
            WorldCatalogResponse::WorldDeleted { id } => assert_eq!(id, created.id),
            response => panic!("unexpected response: {response:?}"),
        }
        assert_eq!(effects, ClientCatalogEffects::default());
        let state = controller.ui_state();
        assert_eq!(state.entry_count(), 0);
        assert!(state.status.ok);
        assert_eq!(state.status.message.as_str(), "Deleted New World");

        // 6. Error case: open a missing id. The executor surfaces the error
        // through `apply_catalog_error` (WorldNotFound), with no follow-up
        // effects.
        let missing = LocalWorldId::new("ghost-world").unwrap();
        let request = ClientCatalogRequest {
            id: WorldCatalogRequestId(999),
            request: WorldCatalogRequest::OpenWorld {
                id: missing.clone(),
            },
        };
        let effects = execute_world_catalog_request(Some(&catalog), &mut controller, None, request);
        assert_eq!(
            catalog.take_last().unwrap_err().kind,
            WorldCatalogErrorKind::WorldNotFound
        );
        assert_eq!(effects, ClientCatalogEffects::default());
        let state = controller.ui_state();
        assert!(state.status.visible);
        assert!(!state.status.ok);
        assert_eq!(
            state.status.message.as_str(),
            "local world `ghost-world` was not found"
        );
    }

    #[test]
    fn missing_catalog_guard_surfaces_unavailable_error() {
        let mut controller = persistent_controller();
        let request = only_request(controller.request_world_list());

        let effects = execute_world_catalog_request(None, &mut controller, None, request);

        assert_eq!(effects, ClientCatalogEffects::default());
        let state = controller.ui_state();
        assert!(state.status.visible);
        assert!(!state.status.ok);
        assert_eq!(
            state.status.message.as_str(),
            "Persistent worlds unavailable"
        );
    }

    #[test]
    fn refresh_world_catalog_controller_loads_worlds_from_catalog() {
        let catalog = FakeWorldCatalog::new();
        let mut controller = persistent_controller();
        let create =
            only_request(controller.apply_ui_action(GameUiAction::CreateCatalogWorld, context(9)));
        execute_world_catalog_request(Some(&catalog), &mut controller, None, create);
        let created = match catalog.take_last().unwrap() {
            WorldCatalogResponse::WorldCreated { summary } => summary,
            response => panic!("unexpected response: {response:?}"),
        };

        refresh_world_catalog_controller(
            Some(&catalog),
            &mut controller,
            WorldCatalogUiStatus::hidden(),
            "test",
        );

        let state = controller.ui_state();
        assert_eq!(state.entry_count(), 1);
        assert_eq!(state.entries[0].as_ref().unwrap().seed, created.seed);
        assert!(!state.status.visible);
    }

    #[test]
    fn ok_path_folds_response_into_session_start() {
        let catalog = FakeWorldCatalog::new();
        let mut controller = persistent_controller();
        let request =
            only_request(controller.apply_ui_action(GameUiAction::CreateCatalogWorld, context(7)));

        let effects = execute_world_catalog_request(Some(&catalog), &mut controller, None, request);

        assert!(matches!(
            catalog.take_last().unwrap(),
            WorldCatalogResponse::WorldCreated { .. }
        ));
        assert!(effects.catalog_requests.is_empty());
        assert_eq!(effects.session_starts.len(), 1);
    }

    #[test]
    fn err_path_surfaces_status_without_follow_up_effects() {
        let catalog = FakeWorldCatalog::new();
        let mut controller = persistent_controller();
        // Craft an open request for a world the catalog does not have; the
        // controller has no pending entry, so this exercises the executor's Err
        // branch directly.
        let request = ClientCatalogRequest {
            id: WorldCatalogRequestId(42),
            request: WorldCatalogRequest::OpenWorld {
                id: LocalWorldId::new("ghost-world").unwrap(),
            },
        };

        let effects = execute_world_catalog_request(Some(&catalog), &mut controller, None, request);

        let error = catalog.take_last().unwrap_err();
        assert_eq!(error.kind, WorldCatalogErrorKind::WorldNotFound);
        assert_eq!(effects, ClientCatalogEffects::default());
        let state = controller.ui_state();
        assert!(state.status.visible);
        assert!(!state.status.ok);
        assert_eq!(state.status.message.as_str(), error.message);
    }

    #[test]
    fn delete_active_world_error_is_reported_through_executor() {
        let catalog = FakeWorldCatalog::new();
        let mut controller = persistent_controller();
        let create =
            only_request(controller.apply_ui_action(GameUiAction::CreateCatalogWorld, context(1)));
        execute_world_catalog_request(Some(&catalog), &mut controller, None, create);
        let _ = catalog.take_last();
        let id = LocalWorldId::new("new-world").unwrap();

        let request = ClientCatalogRequest {
            id: WorldCatalogRequestId(77),
            request: WorldCatalogRequest::DeleteWorld { id: id.clone() },
        };
        let effects =
            execute_world_catalog_request(Some(&catalog), &mut controller, Some(&id), request);

        assert_eq!(
            catalog.take_last().unwrap_err().kind,
            WorldCatalogErrorKind::ActiveWorld
        );
        assert_eq!(effects, ClientCatalogEffects::default());
        // The catalog still holds the world since the active-world guard blocked
        // the delete.
        assert_eq!(catalog.worlds.borrow().len(), 1);
    }
}
