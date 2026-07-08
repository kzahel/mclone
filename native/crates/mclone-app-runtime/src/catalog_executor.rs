//! Shared native world-catalog request executor.
//!
//! This owns the duplicated middle of every native catalog consumer (desktop
//! flat, desktop XR, Android XR, flat Android): the missing-catalog guard →
//! `handle_request` → `apply_catalog_response`/`apply_catalog_error` → return
//! effects. It does **not** apply the effects; each app keeps its own
//! `apply_*_catalog_effects` tail, which drives session/GPU lifecycle and
//! recurses for nested catalog requests.
//!
//! Native-only (`#[cfg(not(target_arch = "wasm32"))]`) on purpose: web's
//! executor is asynchronous and lives in JavaScript/IndexedDB, so it stays a
//! separate `+1` over the already-shared request/response protocol.

use crate::client_catalog_policy::{
    ClientCatalogController, ClientCatalogEffects, ClientCatalogRequest,
};
use crate::world_catalog::{
    LocalWorldId, WorldCatalog, WorldCatalogCapabilities, WorldCatalogError,
};
use mclone_ui::WorldCatalogUiStatus;

/// Run one `ClientCatalogRequest` against a native catalog backend and fold the
/// result back through the shared `ClientCatalogController`.
///
/// When `catalog` is `None` (genuinely transient targets: headless, initial
/// local session), the request fails with the shared "Persistent worlds
/// unavailable" error, exactly as the per-app copies did.
pub fn execute_world_catalog_request(
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
pub fn refresh_world_catalog_controller(
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
    use std::path::PathBuf;

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
        root: PathBuf,
        worlds: RefCell<Vec<LocalWorldSummary>>,
        last: RefCell<Option<WorldCatalogResult<WorldCatalogResponse>>>,
    }

    impl FakeWorldCatalog {
        fn new() -> Self {
            Self {
                root: PathBuf::from("/fake/worlds"),
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

        fn world_dir(&self, id: &LocalWorldId) -> PathBuf {
            self.root.join(id.as_str())
        }
    }

    fn persistent_controller() -> ClientCatalogController {
        let mut controller = ClientCatalogController::new();
        controller.set_capabilities(WorldCatalogCapabilities::persistent_local());
        controller
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
