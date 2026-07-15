use std::collections::{BTreeSet, HashMap};

use crate::session::{ActiveSessionDescriptor, SessionStartRequest};
use crate::world_catalog::{
    LocalWorldCreateOptions, LocalWorldId, LocalWorldSummary, WorldCatalogCapabilities,
    WorldCatalogError, WorldCatalogRequest, WorldCatalogRequestId, WorldCatalogResponse,
    most_recent_compatible_local_world, sort_local_world_summaries,
};
use mclone_ui::{
    GameUiAction, WORLD_CATALOG_UI_ROW_CAPACITY, WorldCatalogUiEntry, WorldCatalogUiState,
    WorldCatalogUiStatus, WorldCatalogUiText, WorldCatalogUiWorldId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientCatalogController {
    capabilities: WorldCatalogCapabilities,
    worlds: Vec<LocalWorldSummary>,
    entries: Vec<ClientCatalogEntry>,
    ui: WorldCatalogUiState,
    active_world: Option<LocalWorldId>,
    pending: HashMap<WorldCatalogRequestId, PendingCatalogRequest>,
    next_request_id: u64,
}

impl Default for ClientCatalogController {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientCatalogController {
    pub fn new() -> Self {
        Self {
            capabilities: WorldCatalogCapabilities::default(),
            worlds: Vec::new(),
            entries: Vec::new(),
            ui: WorldCatalogUiState::default(),
            active_world: None,
            pending: HashMap::new(),
            next_request_id: 1,
        }
    }

    pub fn ui_state(&self) -> WorldCatalogUiState {
        self.ui_state_with_active_world(self.active_world.as_ref())
    }

    pub fn ui_state_with_active_world(
        &self,
        active_world: Option<&LocalWorldId>,
    ) -> WorldCatalogUiState {
        let mut state = self.ui;
        state.active = self.active_local_world_ui_id_for_world(active_world);
        state.loading = !self.pending.is_empty();
        state
    }

    pub fn set_active_world(&mut self, active_world: Option<LocalWorldId>) {
        self.active_world = active_world;
        self.ui.active = self.active_local_world_ui_id();
    }

    pub fn world_summary(&self, id: &LocalWorldId) -> Option<&LocalWorldSummary> {
        self.worlds.iter().find(|summary| &summary.id == id)
    }

    pub fn world_summaries(&self) -> &[LocalWorldSummary] {
        &self.worlds
    }

    pub fn most_recent_compatible_world(&self) -> Option<&LocalWorldSummary> {
        most_recent_compatible_local_world(&self.worlds)
    }

    pub fn world_list_pending(&self) -> bool {
        self.pending
            .values()
            .any(|pending| matches!(pending, PendingCatalogRequest::List))
    }

    pub fn set_create_display_name(&mut self, display_name: &str) {
        self.ui.create_display_name = if self.capabilities.create_supported {
            WorldCatalogUiText::new(display_name)
        } else {
            WorldCatalogUiText::empty()
        };
    }

    pub fn set_capabilities(&mut self, capabilities: WorldCatalogCapabilities) {
        let worlds = self.worlds.clone();
        let status = self.ui.status;
        self.set_worlds(capabilities, worlds, status);
    }

    pub fn set_worlds(
        &mut self,
        capabilities: WorldCatalogCapabilities,
        worlds: Vec<LocalWorldSummary>,
        status: WorldCatalogUiStatus,
    ) {
        self.capabilities = capabilities;
        self.set_world_catalog_worlds(worlds, status);
    }

    pub fn clear_status(&mut self) {
        self.ui.status = WorldCatalogUiStatus::hidden();
    }

    pub fn request_world_list(&mut self) -> ClientCatalogEffects {
        if !self.capabilities.list_supported {
            self.set_unsupported_status();
            return ClientCatalogEffects::default();
        }
        self.queue_catalog_request(WorldCatalogRequest::ListWorlds, PendingCatalogRequest::List)
    }

    /// Record recency only after a catalog world has actually become active.
    /// This request never starts or reopens a session.
    pub fn request_record_world_played(&mut self, id: LocalWorldId) -> ClientCatalogEffects {
        if !self.capabilities.open_supported {
            self.set_unsupported_status();
            return ClientCatalogEffects::default();
        }
        if self.world_summary(&id).is_none() {
            self.ui.status = WorldCatalogUiStatus::new("Selected world is unavailable", false);
            return ClientCatalogEffects::default();
        }
        self.queue_catalog_request(
            WorldCatalogRequest::RecordWorldPlayed { id: id.clone() },
            PendingCatalogRequest::RecordPlayed { id },
        )
    }

    pub fn apply_ui_action(
        &mut self,
        action: GameUiAction,
        context: ClientCatalogActionContext,
    ) -> ClientCatalogEffects {
        match action {
            GameUiAction::OpenWorldList | GameUiAction::OpenWorldCreate => {
                self.clear_status();
                if self.capabilities.list_supported {
                    self.request_world_list()
                } else {
                    ClientCatalogEffects::default()
                }
            }
            GameUiAction::SelectWorld(id) => {
                if self.ui.entry(id).is_some() {
                    self.ui.selected = Some(id);
                }
                self.clear_status();
                ClientCatalogEffects::default()
            }
            GameUiAction::ConfirmDeleteWorld(_) | GameUiAction::CancelDeleteWorld => {
                self.clear_status();
                ClientCatalogEffects::default()
            }
            GameUiAction::OpenWorld(id) => self.request_catalog_world_open(id),
            GameUiAction::CreateCatalogWorld => {
                self.request_catalog_world_create(context.new_world_seed)
            }
            GameUiAction::DeleteWorld(id) => self.request_catalog_world_delete(id),
            _ => ClientCatalogEffects::default(),
        }
    }

    pub fn apply_catalog_response(
        &mut self,
        request_id: WorldCatalogRequestId,
        response: WorldCatalogResponse,
    ) -> ClientCatalogEffects {
        let Some(pending) = self.pending.remove(&request_id) else {
            self.update_loading_flag();
            return ClientCatalogEffects::default();
        };

        self.update_loading_flag();
        match (pending, response) {
            (
                PendingCatalogRequest::List,
                WorldCatalogResponse::WorldList {
                    capabilities,
                    worlds,
                },
            ) => {
                self.set_worlds(capabilities, worlds, WorldCatalogUiStatus::hidden());
                ClientCatalogEffects::default()
            }
            (
                PendingCatalogRequest::Create { options },
                WorldCatalogResponse::WorldCreated { summary },
            ) => {
                let start = ClientCatalogSessionStart {
                    request: SessionStartRequest::create_local_world(
                        options.with_requested_id(summary.id.clone()),
                    ),
                    descriptor: ActiveSessionDescriptor::from_local_world_summary(&summary),
                    summary: summary.clone(),
                };
                self.upsert_world(summary.clone());
                self.select_local_world(&summary.id);
                ClientCatalogEffects::with_session_start(start)
            }
            (PendingCatalogRequest::Open { .. }, WorldCatalogResponse::WorldOpened { summary }) => {
                let start = ClientCatalogSessionStart {
                    request: SessionStartRequest::open_local_world(summary.id.clone()),
                    descriptor: ActiveSessionDescriptor::from_local_world_summary(&summary),
                    summary: summary.clone(),
                };
                self.upsert_world(summary.clone());
                self.select_local_world(&summary.id);
                ClientCatalogEffects::with_session_start(start)
            }
            (
                PendingCatalogRequest::RecordPlayed { id },
                WorldCatalogResponse::WorldPlayRecorded { summary },
            ) if id == summary.id => {
                self.upsert_world(summary);
                ClientCatalogEffects::default()
            }
            (
                PendingCatalogRequest::Delete { id, display_name },
                WorldCatalogResponse::WorldDeleted { id: deleted_id },
            ) if id == deleted_id => {
                self.remove_world(&deleted_id);
                self.ui.status =
                    WorldCatalogUiStatus::new(&format!("Deleted {display_name}"), true);
                ClientCatalogEffects::default()
            }
            _ => {
                self.ui.status =
                    WorldCatalogUiStatus::new("Unexpected world catalog response", false);
                ClientCatalogEffects::default()
            }
        }
    }

    pub fn apply_catalog_error(
        &mut self,
        request_id: WorldCatalogRequestId,
        error: WorldCatalogError,
    ) -> ClientCatalogEffects {
        let _ = self.pending.remove(&request_id);
        self.update_loading_flag();
        self.set_world_catalog_error(&error);
        ClientCatalogEffects::default()
    }

    fn request_catalog_world_create(&mut self, seed: i64) -> ClientCatalogEffects {
        if !self.capabilities.create_supported {
            self.set_unsupported_status();
            return ClientCatalogEffects::default();
        }

        let display_name = if self.ui.create_display_name.is_empty() {
            "New World"
        } else {
            self.ui.create_display_name.as_str()
        };
        let options = match LocalWorldCreateOptions::new(display_name, seed) {
            Ok(options) => options,
            Err(error) => {
                self.set_world_catalog_error(&error);
                return ClientCatalogEffects::default();
            }
        };

        self.queue_catalog_request(
            WorldCatalogRequest::CreateWorld {
                options: options.clone(),
            },
            PendingCatalogRequest::Create { options },
        )
    }

    fn request_catalog_world_open(&mut self, ui_id: WorldCatalogUiWorldId) -> ClientCatalogEffects {
        if !self.capabilities.open_supported {
            self.set_unsupported_status();
            return ClientCatalogEffects::default();
        }
        if !self.ui.can_open_world(ui_id) {
            self.ui.status = WorldCatalogUiStatus::new("Selected world is unavailable", false);
            return ClientCatalogEffects::default();
        }
        let Some(id) = self.local_world_id_for_ui_id(ui_id).cloned() else {
            self.ui.status = WorldCatalogUiStatus::new("Selected world is unavailable", false);
            return ClientCatalogEffects::default();
        };

        self.queue_catalog_request(
            WorldCatalogRequest::OpenWorld { id: id.clone() },
            PendingCatalogRequest::Open { id },
        )
    }

    fn request_catalog_world_delete(
        &mut self,
        ui_id: WorldCatalogUiWorldId,
    ) -> ClientCatalogEffects {
        let Some(entry) = self.catalog_entry_for_ui_id(ui_id).cloned() else {
            self.ui.status = WorldCatalogUiStatus::new("Selected world is unavailable", false);
            return ClientCatalogEffects::default();
        };
        if let Err(error) = entry
            .summary
            .can_delete(self.active_world.as_ref(), self.capabilities)
        {
            self.set_world_catalog_error(&error);
            return ClientCatalogEffects::default();
        }

        self.queue_catalog_request(
            WorldCatalogRequest::DeleteWorld {
                id: entry.summary.id.clone(),
            },
            PendingCatalogRequest::Delete {
                id: entry.summary.id,
                display_name: entry.summary.display_name,
            },
        )
    }

    fn queue_catalog_request(
        &mut self,
        request: WorldCatalogRequest,
        pending: PendingCatalogRequest,
    ) -> ClientCatalogEffects {
        let request_id = self.next_request_id();
        self.pending.insert(request_id, pending);
        self.update_loading_flag();
        ClientCatalogEffects::with_catalog_request(ClientCatalogRequest {
            id: request_id,
            request,
        })
    }

    fn next_request_id(&mut self) -> WorldCatalogRequestId {
        let id = WorldCatalogRequestId(self.next_request_id);
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        id
    }

    fn set_world_catalog_worlds(
        &mut self,
        mut worlds: Vec<LocalWorldSummary>,
        status: WorldCatalogUiStatus,
    ) {
        sort_local_world_summaries(&mut worlds);
        let previous_selected = self.ui.selected;
        let create_display_name = self.ui.create_display_name;
        let total_world_count = worlds.len();
        let mut used_ui_ids = BTreeSet::new();
        let mut cached_entries = Vec::new();
        let mut ui_entries = Vec::new();

        for summary in worlds.iter().take(WORLD_CATALOG_UI_ROW_CAPACITY).cloned() {
            let ui_id = allocate_world_catalog_ui_id(&summary.id, &mut used_ui_ids);
            ui_entries.push(world_catalog_ui_entry(ui_id, &summary));
            cached_entries.push(ClientCatalogEntry { ui_id, summary });
        }

        let mut state = world_catalog_ui_state_for_capabilities(self.capabilities);
        if self.capabilities.create_supported && !create_display_name.is_empty() {
            state.create_display_name = create_display_name;
        }
        state.selected = previous_selected;
        state.active = self.active_world.as_ref().and_then(|active| {
            cached_entries
                .iter()
                .find(|entry| &entry.summary.id == active)
                .map(|entry| entry.ui_id)
                .or_else(|| Some(world_catalog_ui_id_for_local_world(active)))
        });
        state.status = if status.visible {
            status
        } else if total_world_count > WORLD_CATALOG_UI_ROW_CAPACITY {
            WorldCatalogUiStatus::new("Showing first 8 worlds", true)
        } else {
            WorldCatalogUiStatus::hidden()
        };
        state.set_entries(&ui_entries);
        state.loading = !self.pending.is_empty();

        self.worlds = worlds;
        self.entries = cached_entries;
        self.ui = state;
    }

    fn upsert_world(&mut self, summary: LocalWorldSummary) {
        let mut worlds = self.worlds.clone();
        if let Some(existing) = worlds.iter_mut().find(|world| world.id == summary.id) {
            *existing = summary;
        } else {
            worlds.push(summary);
        }
        sort_local_world_summaries(&mut worlds);
        self.set_world_catalog_worlds(worlds, WorldCatalogUiStatus::hidden());
    }

    fn remove_world(&mut self, id: &LocalWorldId) {
        let worlds = self
            .worlds
            .clone()
            .into_iter()
            .filter(|world| &world.id != id)
            .collect::<Vec<_>>();
        self.set_world_catalog_worlds(worlds, WorldCatalogUiStatus::hidden());
    }

    fn select_local_world(&mut self, id: &LocalWorldId) {
        if let Some(ui_id) = self
            .entries
            .iter()
            .find(|entry| &entry.summary.id == id)
            .map(|entry| entry.ui_id)
        {
            self.ui.selected = Some(ui_id);
        }
    }

    fn catalog_entry_for_ui_id(&self, ui_id: WorldCatalogUiWorldId) -> Option<&ClientCatalogEntry> {
        self.entries.iter().find(|entry| entry.ui_id == ui_id)
    }

    pub fn local_world_id_for_ui_id(&self, ui_id: WorldCatalogUiWorldId) -> Option<&LocalWorldId> {
        self.catalog_entry_for_ui_id(ui_id)
            .map(|entry| &entry.summary.id)
    }

    fn active_local_world_ui_id(&self) -> Option<WorldCatalogUiWorldId> {
        self.active_local_world_ui_id_for_world(self.active_world.as_ref())
    }

    fn active_local_world_ui_id_for_world(
        &self,
        active_world: Option<&LocalWorldId>,
    ) -> Option<WorldCatalogUiWorldId> {
        let active = active_world?;
        self.entries
            .iter()
            .find(|entry| &entry.summary.id == active)
            .map(|entry| entry.ui_id)
            .or_else(|| Some(world_catalog_ui_id_for_local_world(active)))
    }

    fn set_world_catalog_error(&mut self, error: &WorldCatalogError) {
        self.ui.status = WorldCatalogUiStatus::new(&error.message, false);
    }

    fn set_unsupported_status(&mut self) {
        self.ui.status = if self.capabilities.persistent {
            WorldCatalogUiStatus::new("World catalog operation is not supported", false)
        } else {
            WorldCatalogUiStatus::new("Persistent worlds unavailable", false)
        };
    }

    fn update_loading_flag(&mut self) {
        self.ui.loading = !self.pending.is_empty();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientCatalogActionContext {
    pub new_world_seed: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientCatalogEffects {
    pub catalog_requests: Vec<ClientCatalogRequest>,
    pub session_starts: Vec<ClientCatalogSessionStart>,
}

impl ClientCatalogEffects {
    fn with_catalog_request(request: ClientCatalogRequest) -> Self {
        Self {
            catalog_requests: vec![request],
            session_starts: Vec::new(),
        }
    }

    fn with_session_start(start: ClientCatalogSessionStart) -> Self {
        Self {
            catalog_requests: Vec::new(),
            session_starts: vec![start],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientCatalogRequest {
    pub id: WorldCatalogRequestId,
    pub request: WorldCatalogRequest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientCatalogSessionStart {
    pub request: SessionStartRequest,
    pub descriptor: ActiveSessionDescriptor,
    pub summary: LocalWorldSummary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClientCatalogEntry {
    ui_id: WorldCatalogUiWorldId,
    summary: LocalWorldSummary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PendingCatalogRequest {
    List,
    Create {
        options: LocalWorldCreateOptions,
    },
    Open {
        id: LocalWorldId,
    },
    RecordPlayed {
        id: LocalWorldId,
    },
    Delete {
        id: LocalWorldId,
        display_name: String,
    },
}

fn world_catalog_ui_state_for_capabilities(
    capabilities: WorldCatalogCapabilities,
) -> WorldCatalogUiState {
    WorldCatalogUiState {
        persistent: capabilities.persistent,
        list_supported: capabilities.list_supported,
        create_supported: capabilities.create_supported,
        open_supported: capabilities.open_supported,
        delete_supported: capabilities.delete_supported,
        create_display_name: if capabilities.create_supported {
            WorldCatalogUiText::new("New World")
        } else {
            WorldCatalogUiText::empty()
        },
        ..WorldCatalogUiState::empty()
    }
}

fn world_catalog_ui_entry(
    id: WorldCatalogUiWorldId,
    summary: &LocalWorldSummary,
) -> WorldCatalogUiEntry {
    WorldCatalogUiEntry::new(id, &summary.display_name, summary.seed)
        .with_created_unix_millis(summary.created_unix_millis)
        .with_last_played_unix_millis(summary.last_played_unix_millis)
        .locked(summary.locked)
        .compatible(summary.compatible)
}

fn allocate_world_catalog_ui_id(
    id: &LocalWorldId,
    used_ui_ids: &mut BTreeSet<u64>,
) -> WorldCatalogUiWorldId {
    let mut ui_id = world_catalog_ui_id_for_local_world(id).0;
    while !used_ui_ids.insert(ui_id) {
        ui_id = ui_id.wrapping_add(1);
        if ui_id == 0 {
            ui_id = 1;
        }
    }
    WorldCatalogUiWorldId(ui_id)
}

fn world_catalog_ui_id_for_local_world(id: &LocalWorldId) -> WorldCatalogUiWorldId {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for byte in id.as_str().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    WorldCatalogUiWorldId(hash.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_catalog::WorldCatalogErrorKind;

    fn context(seed: i64) -> ClientCatalogActionContext {
        ClientCatalogActionContext {
            new_world_seed: seed,
        }
    }

    fn summary(id: &str, display_name: &str, seed: i64) -> LocalWorldSummary {
        LocalWorldSummary::new(LocalWorldId::new(id).unwrap(), display_name, seed, 100).unwrap()
    }

    fn persistent_controller(worlds: Vec<LocalWorldSummary>) -> ClientCatalogController {
        let mut controller = ClientCatalogController::new();
        controller.set_worlds(
            WorldCatalogCapabilities::persistent_local(),
            worlds,
            WorldCatalogUiStatus::hidden(),
        );
        controller
    }

    fn only_request(effects: ClientCatalogEffects) -> ClientCatalogRequest {
        assert!(effects.session_starts.is_empty());
        assert_eq!(effects.catalog_requests.len(), 1);
        effects.catalog_requests.into_iter().next().unwrap()
    }

    fn only_start(effects: ClientCatalogEffects) -> ClientCatalogSessionStart {
        assert!(effects.catalog_requests.is_empty());
        assert_eq!(effects.session_starts.len(), 1);
        effects.session_starts.into_iter().next().unwrap()
    }

    #[test]
    fn world_list_response_builds_ui_state_and_stable_ids() {
        let alpha = summary("alpha-base", "Alpha Base", 11);
        let beta = summary("beta-mine", "Beta Mine", 22);
        let mut controller = ClientCatalogController::new();
        controller.set_capabilities(WorldCatalogCapabilities::persistent_local());

        let request = only_request(controller.request_world_list());
        assert_eq!(request.request, WorldCatalogRequest::ListWorlds);
        assert!(controller.ui_state().loading);

        controller.apply_catalog_response(
            request.id,
            WorldCatalogResponse::WorldList {
                capabilities: WorldCatalogCapabilities::persistent_local(),
                worlds: vec![alpha.clone(), beta.clone()],
            },
        );

        let state = controller.ui_state();
        assert!(!state.loading);
        assert_eq!(state.entry_count(), 2);
        assert_eq!(
            state
                .selected_entry()
                .map(|entry| entry.display_name.as_str()),
            Some("Alpha Base")
        );
        let first_ids = state
            .entries
            .iter()
            .flatten()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();

        controller.set_worlds(
            WorldCatalogCapabilities::persistent_local(),
            vec![alpha, beta],
            WorldCatalogUiStatus::hidden(),
        );
        let second_ids = controller
            .ui_state()
            .entries
            .iter()
            .flatten()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();

        assert_eq!(first_ids, second_ids);
    }

    #[test]
    fn create_action_emits_request_then_session_start_after_completion() {
        let mut controller = persistent_controller(Vec::new());

        let request = only_request(
            controller.apply_ui_action(GameUiAction::CreateCatalogWorld, context(1234)),
        );
        let WorldCatalogRequest::CreateWorld { options } = &request.request else {
            panic!("expected create request");
        };
        assert_eq!(options.display_name, "New World");
        assert_eq!(options.seed, 1234);
        assert!(controller.ui_state().loading);

        let created = summary("new-world", "New World", 1234);
        let start = only_start(controller.apply_catalog_response(
            request.id,
            WorldCatalogResponse::WorldCreated {
                summary: created.clone(),
            },
        ));

        assert_eq!(
            start.request,
            SessionStartRequest::create_local_world(
                LocalWorldCreateOptions::new("New World", 1234)
                    .unwrap()
                    .with_requested_id(LocalWorldId::new("new-world").unwrap())
            )
        );
        assert_eq!(
            start.descriptor,
            ActiveSessionDescriptor::from_local_world_summary(&created)
        );
        assert_eq!(start.summary, created);
        assert_eq!(controller.ui_state().entry_count(), 1);
        assert!(!controller.ui_state().loading);
    }

    #[test]
    fn open_action_emits_request_then_session_start_after_completion() {
        let alpha = summary("alpha-base", "Alpha Base", 11);
        let mut controller = persistent_controller(vec![alpha.clone()]);
        let ui_id = controller.ui_state().selected.unwrap();

        let request =
            only_request(controller.apply_ui_action(GameUiAction::OpenWorld(ui_id), context(0)));
        assert_eq!(
            request.request,
            WorldCatalogRequest::OpenWorld {
                id: alpha.id.clone()
            }
        );

        let mut opened = alpha.clone();
        opened.last_played_unix_millis = Some(500);
        let start = only_start(controller.apply_catalog_response(
            request.id,
            WorldCatalogResponse::WorldOpened {
                summary: opened.clone(),
            },
        ));

        assert_eq!(
            start.request,
            SessionStartRequest::open_local_world(alpha.id.clone())
        );
        assert_eq!(
            start.descriptor,
            ActiveSessionDescriptor::from_local_world_summary(&opened)
        );
        assert_eq!(
            controller
                .ui_state()
                .selected_entry()
                .and_then(|entry| entry.last_played_unix_millis),
            Some(500)
        );
    }

    #[test]
    fn catalog_retains_all_worlds_and_selects_most_recent_compatible() {
        let mut worlds = (0..10)
            .map(|index| {
                let mut world =
                    summary(&format!("world-{index}"), &format!("World {index}"), index);
                world.last_played_unix_millis = Some(1_000 - index as u64);
                world
            })
            .collect::<Vec<_>>();
        worlds[0].compatible = false;
        let outside_ui_id = worlds[9].id.clone();
        let controller = persistent_controller(worlds);

        assert_eq!(
            controller.ui_state().entry_count(),
            WORLD_CATALOG_UI_ROW_CAPACITY
        );
        assert_eq!(controller.world_summaries().len(), 10);
        assert!(controller.world_summary(&outside_ui_id).is_some());
        assert_eq!(
            controller
                .most_recent_compatible_world()
                .map(|world| world.id.as_str()),
            Some("world-1")
        );
    }

    #[test]
    fn most_recent_compatible_selection_uses_catalog_tie_breaks() {
        let mut alpha = summary("alpha", "Alpha", 1);
        let mut beta = summary("beta", "Beta", 2);
        let mut incompatible = summary("newest", "Newest", 3);
        alpha.last_played_unix_millis = Some(500);
        beta.last_played_unix_millis = Some(500);
        incompatible.last_played_unix_millis = Some(600);
        incompatible.compatible = false;

        let controller = persistent_controller(vec![beta, incompatible, alpha]);

        assert_eq!(
            controller
                .most_recent_compatible_world()
                .map(|world| world.id.as_str()),
            Some("alpha")
        );
    }

    #[test]
    fn most_recent_compatible_selection_rejects_empty_and_incompatible_catalogs() {
        assert!(
            persistent_controller(Vec::new())
                .most_recent_compatible_world()
                .is_none()
        );

        let mut incompatible = summary("newest", "Newest", 3);
        incompatible.last_played_unix_millis = Some(600);
        incompatible.compatible = false;
        assert!(
            persistent_controller(vec![incompatible])
                .most_recent_compatible_world()
                .is_none()
        );
    }

    #[test]
    fn record_play_updates_cached_recency_without_starting_session() {
        let alpha = summary("alpha-base", "Alpha Base", 11);
        let mut controller = persistent_controller(vec![alpha.clone()]);

        let request = only_request(controller.request_record_world_played(alpha.id.clone()));
        assert_eq!(
            request.request,
            WorldCatalogRequest::RecordWorldPlayed {
                id: alpha.id.clone()
            }
        );
        let mut recorded = alpha.clone();
        recorded.last_played_unix_millis = Some(750);
        let effects = controller.apply_catalog_response(
            request.id,
            WorldCatalogResponse::WorldPlayRecorded {
                summary: recorded.clone(),
            },
        );

        assert!(effects.catalog_requests.is_empty());
        assert!(effects.session_starts.is_empty());
        assert_eq!(
            controller
                .world_summary(&alpha.id)
                .and_then(|world| world.last_played_unix_millis),
            Some(750)
        );
    }

    #[test]
    fn list_pending_is_distinct_from_other_catalog_work() {
        let alpha = summary("alpha-base", "Alpha Base", 11);
        let mut controller = persistent_controller(vec![alpha.clone()]);
        let record = only_request(controller.request_record_world_played(alpha.id));
        assert!(!controller.world_list_pending());

        let list = only_request(controller.request_world_list());
        assert!(controller.world_list_pending());
        controller.apply_catalog_error(list.id, WorldCatalogError::unsupported("cancelled list"));
        assert!(!controller.world_list_pending());
        controller.apply_catalog_error(
            record.id,
            WorldCatalogError::unsupported("cancelled record"),
        );
    }

    #[test]
    fn stale_record_completion_cannot_change_cached_recency() {
        let alpha = summary("alpha-base", "Alpha Base", 11);
        let mut controller = persistent_controller(vec![alpha.clone()]);
        let record = only_request(controller.request_record_world_played(alpha.id.clone()));
        controller.apply_catalog_error(
            record.id,
            WorldCatalogError::unsupported("cancelled record"),
        );

        let mut stale = alpha.clone();
        stale.last_played_unix_millis = Some(900);
        let effects = controller.apply_catalog_response(
            record.id,
            WorldCatalogResponse::WorldPlayRecorded { summary: stale },
        );

        assert!(effects.catalog_requests.is_empty());
        assert!(effects.session_starts.is_empty());
        assert_eq!(
            controller
                .world_summary(&alpha.id)
                .and_then(|world| world.last_played_unix_millis),
            alpha.last_played_unix_millis
        );
    }

    #[test]
    fn delete_action_removes_world_after_async_completion() {
        let alpha = summary("alpha-base", "Alpha Base", 11);
        let mut controller = persistent_controller(vec![alpha.clone()]);
        let ui_id = controller.ui_state().selected.unwrap();

        let request =
            only_request(controller.apply_ui_action(GameUiAction::DeleteWorld(ui_id), context(0)));
        assert_eq!(
            request.request,
            WorldCatalogRequest::DeleteWorld {
                id: alpha.id.clone()
            }
        );
        assert!(controller.ui_state().loading);

        controller.apply_catalog_response(
            request.id,
            WorldCatalogResponse::WorldDeleted {
                id: alpha.id.clone(),
            },
        );

        let state = controller.ui_state();
        assert_eq!(state.entry_count(), 0);
        assert!(!state.loading);
        assert!(state.status.visible);
        assert!(state.status.ok);
        assert_eq!(state.status.message.as_str(), "Deleted Alpha Base");
    }

    #[test]
    fn delete_active_world_is_rejected_before_request() {
        let alpha = summary("alpha-base", "Alpha Base", 11);
        let mut controller = persistent_controller(vec![alpha.clone()]);
        let ui_id = controller.ui_state().selected.unwrap();
        controller.set_active_world(Some(alpha.id.clone()));

        let effects = controller.apply_ui_action(GameUiAction::DeleteWorld(ui_id), context(0));

        assert!(effects.catalog_requests.is_empty());
        assert!(effects.session_starts.is_empty());
        let state = controller.ui_state();
        assert_eq!(state.active, Some(ui_id));
        assert!(state.status.visible);
        assert!(!state.status.ok);
        assert!(
            state
                .status
                .message
                .as_str()
                .contains("cannot delete active")
        );
    }

    #[test]
    fn catalog_error_surfaces_status_and_clears_loading() {
        let alpha = summary("alpha-base", "Alpha Base", 11);
        let mut controller = persistent_controller(vec![alpha]);
        let ui_id = controller.ui_state().selected.unwrap();
        let request =
            only_request(controller.apply_ui_action(GameUiAction::OpenWorld(ui_id), context(0)));

        controller.apply_catalog_error(
            request.id,
            WorldCatalogError::new(WorldCatalogErrorKind::StorageFailure, "indexeddb failed"),
        );

        let state = controller.ui_state();
        assert!(!state.loading);
        assert!(state.status.visible);
        assert!(!state.status.ok);
        assert_eq!(state.status.message.as_str(), "indexeddb failed");
    }

    #[test]
    fn unsupported_catalog_action_surfaces_status_without_request() {
        let mut controller = ClientCatalogController::new();

        let effects = controller.apply_ui_action(GameUiAction::CreateCatalogWorld, context(1234));

        assert!(effects.catalog_requests.is_empty());
        assert!(effects.session_starts.is_empty());
        let state = controller.ui_state();
        assert!(state.status.visible);
        assert!(!state.status.ok);
        assert_eq!(
            state.status.message.as_str(),
            "Persistent worlds unavailable"
        );
    }

    #[test]
    fn transient_create_only_allows_create_and_rejects_persistent_actions() {
        let mut controller = ClientCatalogController::new();
        controller.set_capabilities(WorldCatalogCapabilities::transient_create_only());

        let request = only_request(
            controller.apply_ui_action(GameUiAction::CreateCatalogWorld, context(4321)),
        );
        assert_eq!(
            request.request,
            WorldCatalogRequest::CreateWorld {
                options: LocalWorldCreateOptions::new("New World", 4321).unwrap()
            }
        );

        let effects = controller.apply_ui_action(
            GameUiAction::OpenWorld(WorldCatalogUiWorldId(1)),
            context(0),
        );
        assert!(effects.catalog_requests.is_empty());
        assert!(effects.session_starts.is_empty());
        let state = controller.ui_state();
        assert!(state.create_supported);
        assert!(!state.list_supported);
        assert!(!state.open_supported);
        assert!(!state.delete_supported);
        assert!(state.status.visible);
        assert!(!state.status.ok);
        assert_eq!(
            state.status.message.as_str(),
            "Persistent worlds unavailable"
        );
    }

    #[test]
    fn catalog_failure_response_covers_duplicate_id_errors() {
        let mut controller = persistent_controller(Vec::new());
        let request =
            only_request(controller.apply_ui_action(GameUiAction::CreateCatalogWorld, context(7)));

        controller.apply_catalog_error(
            request.id,
            WorldCatalogError::new(
                WorldCatalogErrorKind::DuplicateWorldId,
                "duplicate world id",
            ),
        );

        let state = controller.ui_state();
        assert!(state.status.visible);
        assert!(!state.status.ok);
        assert_eq!(state.status.message.as_str(), "duplicate world id");
    }
}
