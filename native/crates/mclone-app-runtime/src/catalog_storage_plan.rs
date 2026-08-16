//! Shared storage-plan continuation for world-catalog operations.
//!
//! The continuation owns catalog sequencing and emits storage-level plans.
//! Platform adapters encode those plans, execute them against their storage
//! API, and return typed read results without moving catalog policy into the
//! adapter.

use crate::world_catalog::{
    LocalWorldCreateOptions, LocalWorldId, LocalWorldSummary, WorldCatalogCapabilities,
    WorldCatalogError, WorldCatalogErrorKind, WorldCatalogRequest, WorldCatalogResponse,
    duplicate_world_id, sort_local_world_summaries, validate_delete_inactive_world,
    validate_local_world_compatible, world_not_found,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageStore {
    Catalog,
    DimensionChunks,
    DimensionEntityChunks,
    Dimensions,
    Players,
    SavedData,
    WorldMetadata,
    ManagedWorldMetadata,
}

impl StorageStore {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::DimensionChunks => "dimension-chunks",
            Self::DimensionEntityChunks => "dimension-entity-chunks",
            Self::Dimensions => "dimensions",
            Self::Players => "players",
            Self::SavedData => "saved-data",
            Self::WorldMetadata => "world-metadata",
            Self::ManagedWorldMetadata => "managed-world-metadata",
        }
    }
}

const ORDINARY_WORLD_RECORD_STORES: [StorageStore; 6] = [
    StorageStore::DimensionChunks,
    StorageStore::DimensionEntityChunks,
    StorageStore::Dimensions,
    StorageStore::Players,
    StorageStore::SavedData,
    StorageStore::WorldMetadata,
];

const FACTORY_RESET_STORES: [StorageStore; 7] = [
    StorageStore::ManagedWorldMetadata,
    StorageStore::DimensionChunks,
    StorageStore::DimensionEntityChunks,
    StorageStore::Dimensions,
    StorageStore::Players,
    StorageStore::SavedData,
    StorageStore::WorldMetadata,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageMode {
    ReadOnly,
    ReadWrite,
}

impl StorageMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "readonly",
            Self::ReadWrite => "readwrite",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageActionKind {
    GetAll {
        store: StorageStore,
        needs_timestamp: bool,
    },
    Get {
        store: StorageStore,
        key: String,
        needs_timestamp: bool,
    },
    AddCatalogRecord(LocalWorldSummary),
    PutCatalogRecord(LocalWorldSummary),
    DeleteKey {
        store: StorageStore,
        key: String,
    },
    DeleteIndexRange {
        store: StorageStore,
        index: &'static str,
        key: String,
    },
    Clear {
        store: StorageStore,
    },
}

impl StorageActionKind {
    pub const fn store(&self) -> StorageStore {
        match self {
            Self::GetAll { store, .. }
            | Self::Get { store, .. }
            | Self::DeleteKey { store, .. }
            | Self::DeleteIndexRange { store, .. }
            | Self::Clear { store } => *store,
            Self::AddCatalogRecord(_) | Self::PutCatalogRecord(_) => StorageStore::Catalog,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageAction {
    pub id: u32,
    pub kind: StorageActionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageTransaction {
    pub stores: Vec<StorageStore>,
    pub mode: StorageMode,
    pub optional_stores: bool,
    pub actions: Vec<StorageAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageStep {
    pub id: u32,
    pub transactions: Vec<StorageTransaction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CatalogFlow {
    List,
    Create {
        options: LocalWorldCreateOptions,
        summary: Option<LocalWorldSummary>,
    },
    Open {
        id: LocalWorldId,
        summary: Option<LocalWorldSummary>,
    },
    RecordPlayed {
        id: LocalWorldId,
    },
    Delete {
        id: LocalWorldId,
        summary: Option<LocalWorldSummary>,
        cleared: bool,
    },
    DeleteMany {
        include_app_private_content: bool,
        world_ids: Option<Vec<LocalWorldId>>,
        deleted_count: usize,
        index: usize,
        stage: DeleteManyStage,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeleteManyStage {
    Read,
    Clear,
    Delete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ReadPurpose {
    List,
    CreateExisting(LocalWorldCreateOptions),
    Open(LocalWorldId),
    RecordPlayed(LocalWorldId),
    Delete(LocalWorldId),
    DeleteManyList,
    DeleteManyWorld(LocalWorldId),
}

impl ReadPurpose {
    const fn needs_timestamp(&self) -> bool {
        matches!(
            self,
            Self::CreateExisting(_) | Self::Open(_) | Self::RecordPlayed(_)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StepCompletion {
    Complete(WorldCatalogResponse),
    CreateRead(LocalWorldSummary),
    OpenRead(LocalWorldSummary),
    DeleteRead(LocalWorldSummary),
    DeleteCleared,
    DeleteManyListed(Vec<LocalWorldId>),
    DeleteManyWorldRead,
    DeleteManyWorldCleared,
    DeleteManyWorldDeleted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingRead {
    action_id: u32,
    purpose: ReadPurpose,
    completed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OutstandingStep {
    id: u32,
    read: Option<PendingRead>,
    completion: Option<StepCompletion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CatalogReadResult {
    All(Vec<LocalWorldSummary>),
    One(Option<LocalWorldSummary>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogReadShape {
    All,
    One,
}

#[derive(Debug)]
pub struct CatalogExecutionCore {
    flow: CatalogFlow,
    active_world: Option<LocalWorldId>,
    created_world_backend_label: String,
    next_step_id: u32,
    next_action_id: u32,
    outstanding: Option<OutstandingStep>,
    response: Option<WorldCatalogResponse>,
}

impl CatalogExecutionCore {
    pub fn new(
        request: WorldCatalogRequest,
        active_world: Option<LocalWorldId>,
        created_world_backend_label: impl Into<String>,
    ) -> Result<Self, String> {
        let flow = match request {
            WorldCatalogRequest::ListWorlds => CatalogFlow::List,
            WorldCatalogRequest::CreateWorld { options } => CatalogFlow::Create {
                options,
                summary: None,
            },
            WorldCatalogRequest::OpenWorld { id } => CatalogFlow::Open { id, summary: None },
            WorldCatalogRequest::RecordWorldPlayed { id } => CatalogFlow::RecordPlayed { id },
            WorldCatalogRequest::DeleteWorld { id } => CatalogFlow::Delete {
                id,
                summary: None,
                cleared: false,
            },
            WorldCatalogRequest::DeleteAllLocalWorlds {
                include_app_private_content,
            } => {
                if active_world.is_some() {
                    return Err("Quit to title before deleting all local worlds".to_owned());
                }
                CatalogFlow::DeleteMany {
                    include_app_private_content,
                    world_ids: None,
                    deleted_count: 0,
                    index: 0,
                    stage: DeleteManyStage::Read,
                }
            }
        };
        Ok(Self {
            flow,
            active_world,
            created_world_backend_label: created_world_backend_label.into(),
            next_step_id: 1,
            next_action_id: 1,
            outstanding: None,
            response: None,
        })
    }

    pub fn required_writer_world_ids(&self) -> Vec<LocalWorldId> {
        match &self.flow {
            CatalogFlow::Delete { id, .. } => vec![id.clone()],
            CatalogFlow::DeleteMany {
                world_ids: Some(world_ids),
                ..
            } => world_ids.clone(),
            _ => Vec::new(),
        }
    }

    pub fn is_complete(&self) -> bool {
        self.response.is_some()
    }

    pub fn response(&self) -> Result<&WorldCatalogResponse, String> {
        self.response
            .as_ref()
            .ok_or_else(|| "world catalog execution is not complete".to_owned())
    }

    pub fn next_step(&mut self) -> Result<Option<StorageStep>, String> {
        if self.outstanding.is_some() {
            return Err("world catalog storage step is still outstanding".to_owned());
        }
        if self.response.is_some() {
            return Ok(None);
        }

        let flow = self.flow.clone();
        match flow {
            CatalogFlow::List => self.read_all_step(StorageMode::ReadOnly, ReadPurpose::List),
            CatalogFlow::Create { options, summary } => match summary {
                None => {
                    self.read_all_step(StorageMode::ReadOnly, ReadPurpose::CreateExisting(options))
                }
                Some(summary) => {
                    let response = WorldCatalogResponse::WorldCreated {
                        summary: summary.clone(),
                    };
                    let action = self.action(StorageActionKind::AddCatalogRecord(summary));
                    self.fixed_step(
                        vec![required_transaction(
                            vec![StorageStore::Catalog],
                            StorageMode::ReadWrite,
                            vec![action],
                        )],
                        StepCompletion::Complete(response),
                    )
                }
            },
            CatalogFlow::Open { id, summary } => match summary {
                None => self.read_one_step(StorageMode::ReadOnly, ReadPurpose::Open(id)),
                Some(summary) => {
                    let response = WorldCatalogResponse::WorldOpened {
                        summary: summary.clone(),
                    };
                    let action = self.action(StorageActionKind::PutCatalogRecord(summary));
                    self.fixed_step(
                        vec![required_transaction(
                            vec![StorageStore::Catalog],
                            StorageMode::ReadWrite,
                            vec![action],
                        )],
                        StepCompletion::Complete(response),
                    )
                }
            },
            CatalogFlow::RecordPlayed { id } => {
                self.read_one_step(StorageMode::ReadWrite, ReadPurpose::RecordPlayed(id))
            }
            CatalogFlow::Delete {
                id,
                summary,
                cleared,
            } => match (summary, cleared) {
                (None, _) => self.read_one_step(StorageMode::ReadOnly, ReadPurpose::Delete(id)),
                (Some(_), false) => {
                    let transactions =
                        clear_world_transactions(&mut self.next_action_id, id.as_str());
                    self.fixed_step(transactions, StepCompletion::DeleteCleared)
                }
                (Some(_), true) => {
                    let response = WorldCatalogResponse::WorldDeleted { id: id.clone() };
                    let action = self.action(StorageActionKind::DeleteKey {
                        store: StorageStore::Catalog,
                        key: id.into_string(),
                    });
                    self.fixed_step(
                        vec![required_transaction(
                            vec![StorageStore::Catalog],
                            StorageMode::ReadWrite,
                            vec![action],
                        )],
                        StepCompletion::Complete(response),
                    )
                }
            },
            CatalogFlow::DeleteMany {
                include_app_private_content,
                world_ids,
                deleted_count,
                index,
                stage,
            } => {
                let Some(world_ids) = world_ids else {
                    return self.read_all_step(StorageMode::ReadOnly, ReadPurpose::DeleteManyList);
                };
                let Some(id) = world_ids.get(index).cloned() else {
                    if include_app_private_content {
                        let actions = FACTORY_RESET_STORES
                            .iter()
                            .copied()
                            .map(|store| self.action(StorageActionKind::Clear { store }))
                            .collect();
                        return self.fixed_step(
                            vec![optional_transaction(
                                FACTORY_RESET_STORES.to_vec(),
                                StorageMode::ReadWrite,
                                actions,
                            )],
                            StepCompletion::Complete(WorldCatalogResponse::AllLocalWorldsDeleted {
                                deleted_count,
                            }),
                        );
                    }
                    self.response =
                        Some(WorldCatalogResponse::AllLocalWorldsDeleted { deleted_count });
                    return Ok(None);
                };
                match stage {
                    DeleteManyStage::Read => {
                        self.read_one_step(StorageMode::ReadOnly, ReadPurpose::DeleteManyWorld(id))
                    }
                    DeleteManyStage::Clear => {
                        let transactions =
                            clear_world_transactions(&mut self.next_action_id, id.as_str());
                        self.fixed_step(transactions, StepCompletion::DeleteManyWorldCleared)
                    }
                    DeleteManyStage::Delete => {
                        let action = self.action(StorageActionKind::DeleteKey {
                            store: StorageStore::Catalog,
                            key: id.into_string(),
                        });
                        self.fixed_step(
                            vec![required_transaction(
                                vec![StorageStore::Catalog],
                                StorageMode::ReadWrite,
                                vec![action],
                            )],
                            StepCompletion::DeleteManyWorldDeleted,
                        )
                    }
                }
            }
        }
    }

    fn read_all_step(
        &mut self,
        mode: StorageMode,
        purpose: ReadPurpose,
    ) -> Result<Option<StorageStep>, String> {
        self.read_step(mode, purpose, None)
    }

    fn read_one_step(
        &mut self,
        mode: StorageMode,
        purpose: ReadPurpose,
    ) -> Result<Option<StorageStep>, String> {
        let key = match &purpose {
            ReadPurpose::Open(id)
            | ReadPurpose::RecordPlayed(id)
            | ReadPurpose::Delete(id)
            | ReadPurpose::DeleteManyWorld(id) => Some(id.as_str().to_owned()),
            _ => return Err("catalog read-one purpose has no world id".to_owned()),
        };
        self.read_step(mode, purpose, key)
    }

    fn read_step(
        &mut self,
        mode: StorageMode,
        purpose: ReadPurpose,
        key: Option<String>,
    ) -> Result<Option<StorageStep>, String> {
        let step_id = self.take_step_id();
        let action_id = self.take_action_id();
        let needs_timestamp = purpose.needs_timestamp();
        let kind = match key {
            Some(key) => StorageActionKind::Get {
                store: StorageStore::Catalog,
                key,
                needs_timestamp,
            },
            None => StorageActionKind::GetAll {
                store: StorageStore::Catalog,
                needs_timestamp,
            },
        };
        let action = StorageAction {
            id: action_id,
            kind,
        };
        let step = StorageStep {
            id: step_id,
            transactions: vec![required_transaction(
                vec![StorageStore::Catalog],
                mode,
                vec![action],
            )],
        };
        self.outstanding = Some(OutstandingStep {
            id: step_id,
            read: Some(PendingRead {
                action_id,
                purpose,
                completed: false,
            }),
            completion: None,
        });
        Ok(Some(step))
    }

    fn fixed_step(
        &mut self,
        transactions: Vec<StorageTransaction>,
        completion: StepCompletion,
    ) -> Result<Option<StorageStep>, String> {
        let step_id = self.take_step_id();
        self.outstanding = Some(OutstandingStep {
            id: step_id,
            read: None,
            completion: Some(completion),
        });
        Ok(Some(StorageStep {
            id: step_id,
            transactions,
        }))
    }

    pub fn pending_read_shape(
        &self,
        step_id: u32,
        action_id: u32,
    ) -> Result<CatalogReadShape, String> {
        let read = self.pending_read(step_id, action_id)?;
        Ok(match read.purpose {
            ReadPurpose::List | ReadPurpose::CreateExisting(_) | ReadPurpose::DeleteManyList => {
                CatalogReadShape::All
            }
            ReadPurpose::Open(_)
            | ReadPurpose::RecordPlayed(_)
            | ReadPurpose::Delete(_)
            | ReadPurpose::DeleteManyWorld(_) => CatalogReadShape::One,
        })
    }

    pub fn pending_read_needs_timestamp(
        &self,
        step_id: u32,
        action_id: u32,
    ) -> Result<bool, String> {
        Ok(self
            .pending_read(step_id, action_id)?
            .purpose
            .needs_timestamp())
    }

    fn pending_read(&self, step_id: u32, action_id: u32) -> Result<&PendingRead, String> {
        let outstanding = self
            .outstanding
            .as_ref()
            .ok_or_else(|| "world catalog execution has no outstanding step".to_owned())?;
        if outstanding.id != step_id {
            return Err(format!(
                "world catalog read belongs to step {step_id}, expected {}",
                outstanding.id
            ));
        }
        let read = outstanding
            .read
            .as_ref()
            .ok_or_else(|| "world catalog storage step does not expect a read".to_owned())?;
        if read.action_id != action_id {
            return Err(format!(
                "world catalog read action {action_id} does not match {}",
                read.action_id
            ));
        }
        if read.completed {
            return Err("world catalog storage read was already completed".to_owned());
        }
        Ok(read)
    }

    pub fn accept_read_result(
        &mut self,
        step_id: u32,
        action_id: u32,
        result: CatalogReadResult,
        now_unix_millis: Option<u64>,
    ) -> Result<Vec<StorageAction>, String> {
        let purpose = self.pending_read(step_id, action_id)?.purpose.clone();
        let timestamp = || {
            now_unix_millis
                .ok_or_else(|| "world catalog storage read requires a browser timestamp".to_owned())
        };
        let (completion, followups) = match (purpose, result) {
            (ReadPurpose::List, CatalogReadResult::All(mut worlds)) => {
                sort_local_world_summaries(&mut worlds);
                (
                    StepCompletion::Complete(WorldCatalogResponse::WorldList {
                        capabilities: WorldCatalogCapabilities::persistent_local(),
                        worlds,
                    }),
                    Vec::new(),
                )
            }
            (ReadPurpose::CreateExisting(options), CatalogReadResult::All(existing)) => {
                let id = options
                    .resolve_id(existing.iter().map(|world| &world.id))
                    .map_err(|error| error.message)?;
                if existing.iter().any(|world| world.id == id) {
                    return Err(duplicate_world_id(&id).message);
                }
                let now = timestamp()?;
                let mut summary =
                    LocalWorldSummary::new(id, options.display_name, options.seed, now)
                        .map_err(|error| error.message)?;
                summary.world_generation_profile = options.world_generation_profile;
                summary.starter_content = options.starter_content;
                summary.last_played_unix_millis = Some(now);
                summary.backend_label = Some(self.created_world_backend_label.clone());
                (StepCompletion::CreateRead(summary), Vec::new())
            }
            (ReadPurpose::Open(id), CatalogReadResult::One(record)) => {
                let mut summary = required_summary(record, &id)?;
                validate_local_world_compatible(&summary).map_err(|error| error.message)?;
                summary.last_played_unix_millis = Some(timestamp()?);
                (StepCompletion::OpenRead(summary), Vec::new())
            }
            (ReadPurpose::RecordPlayed(id), CatalogReadResult::One(record)) => {
                let mut summary = required_summary(record, &id)?;
                validate_local_world_compatible(&summary).map_err(|error| error.message)?;
                summary.last_played_unix_millis = Some(timestamp()?);
                let followup = StorageAction {
                    id: self.take_action_id(),
                    kind: StorageActionKind::PutCatalogRecord(summary.clone()),
                };
                (
                    StepCompletion::Complete(WorldCatalogResponse::WorldPlayRecorded { summary }),
                    vec![followup],
                )
            }
            (ReadPurpose::Delete(id), CatalogReadResult::One(record)) => {
                validate_delete_inactive_world(&id, self.active_world.as_ref())
                    .map_err(|error| error.message)?;
                (
                    StepCompletion::DeleteRead(required_summary(record, &id)?),
                    Vec::new(),
                )
            }
            (ReadPurpose::DeleteManyList, CatalogReadResult::All(mut worlds)) => {
                sort_local_world_summaries(&mut worlds);
                (
                    StepCompletion::DeleteManyListed(
                        worlds.into_iter().map(|world| world.id).collect(),
                    ),
                    Vec::new(),
                )
            }
            (ReadPurpose::DeleteManyWorld(id), CatalogReadResult::One(record)) => {
                validate_delete_inactive_world(&id, self.active_world.as_ref())
                    .map_err(|error| error.message)?;
                let _ = required_summary(record, &id)?;
                (StepCompletion::DeleteManyWorldRead, Vec::new())
            }
            (purpose, result) => {
                return Err(format!(
                    "world catalog read result {result:?} does not match {purpose:?}"
                ));
            }
        };

        let outstanding = self
            .outstanding
            .as_mut()
            .expect("pending read proved an outstanding step");
        let read = outstanding
            .read
            .as_mut()
            .expect("pending read proved a read action");
        read.completed = true;
        outstanding.completion = Some(completion);
        Ok(followups)
    }

    pub fn complete_step(&mut self, step_id: u32) -> Result<(), String> {
        let outstanding = self
            .outstanding
            .take()
            .ok_or_else(|| "world catalog execution has no outstanding step".to_owned())?;
        if outstanding.id != step_id {
            self.outstanding = Some(outstanding);
            return Err(format!(
                "world catalog completion belongs to step {step_id}, expected {}",
                self.outstanding.as_ref().expect("step was restored").id
            ));
        }
        if outstanding
            .read
            .as_ref()
            .is_some_and(|read| !read.completed)
        {
            self.outstanding = Some(outstanding);
            return Err("world catalog storage step completed before its read".to_owned());
        }
        let completion = outstanding
            .completion
            .ok_or_else(|| "world catalog storage step has no completion transition".to_owned())?;
        match completion {
            StepCompletion::Complete(response) => self.response = Some(response),
            StepCompletion::CreateRead(summary) => {
                let CatalogFlow::Create {
                    summary: current, ..
                } = &mut self.flow
                else {
                    return Err("create read completed in another catalog flow".to_owned());
                };
                *current = Some(summary);
            }
            StepCompletion::OpenRead(summary) => {
                let CatalogFlow::Open {
                    summary: current, ..
                } = &mut self.flow
                else {
                    return Err("open read completed in another catalog flow".to_owned());
                };
                *current = Some(summary);
            }
            StepCompletion::DeleteRead(summary) => {
                let CatalogFlow::Delete {
                    summary: current, ..
                } = &mut self.flow
                else {
                    return Err("delete read completed in another catalog flow".to_owned());
                };
                *current = Some(summary);
            }
            StepCompletion::DeleteCleared => {
                let CatalogFlow::Delete { cleared, .. } = &mut self.flow else {
                    return Err("delete clear completed in another catalog flow".to_owned());
                };
                *cleared = true;
            }
            StepCompletion::DeleteManyListed(ids) => {
                let CatalogFlow::DeleteMany { world_ids, .. } = &mut self.flow else {
                    return Err("delete-all list completed in another catalog flow".to_owned());
                };
                *world_ids = Some(ids);
            }
            StepCompletion::DeleteManyWorldRead => {
                let CatalogFlow::DeleteMany { stage, .. } = &mut self.flow else {
                    return Err("delete-all read completed in another catalog flow".to_owned());
                };
                *stage = DeleteManyStage::Clear;
            }
            StepCompletion::DeleteManyWorldCleared => {
                let CatalogFlow::DeleteMany { stage, .. } = &mut self.flow else {
                    return Err("delete-all clear completed in another catalog flow".to_owned());
                };
                *stage = DeleteManyStage::Delete;
            }
            StepCompletion::DeleteManyWorldDeleted => {
                let CatalogFlow::DeleteMany {
                    deleted_count,
                    index,
                    stage,
                    ..
                } = &mut self.flow
                else {
                    return Err("delete-all write completed in another catalog flow".to_owned());
                };
                *deleted_count = deleted_count.saturating_add(1);
                *index = index.saturating_add(1);
                *stage = DeleteManyStage::Read;
            }
        }
        Ok(())
    }

    fn take_step_id(&mut self) -> u32 {
        let id = self.next_step_id;
        self.next_step_id = self.next_step_id.saturating_add(1);
        id
    }

    fn take_action_id(&mut self) -> u32 {
        let id = self.next_action_id;
        self.next_action_id = self.next_action_id.saturating_add(1);
        id
    }

    fn action(&mut self, kind: StorageActionKind) -> StorageAction {
        StorageAction {
            id: self.take_action_id(),
            kind,
        }
    }
}

fn required_summary(
    summary: Option<LocalWorldSummary>,
    id: &LocalWorldId,
) -> Result<LocalWorldSummary, String> {
    let summary = summary.ok_or_else(|| world_not_found(id).message)?;
    if &summary.id != id {
        return Err(WorldCatalogError::new(
            WorldCatalogErrorKind::StorageFailure,
            format!("local world record `{id}` identified `{}`", summary.id),
        )
        .message);
    }
    Ok(summary)
}

fn required_transaction(
    stores: Vec<StorageStore>,
    mode: StorageMode,
    actions: Vec<StorageAction>,
) -> StorageTransaction {
    StorageTransaction {
        stores,
        mode,
        optional_stores: false,
        actions,
    }
}

fn optional_transaction(
    stores: Vec<StorageStore>,
    mode: StorageMode,
    actions: Vec<StorageAction>,
) -> StorageTransaction {
    StorageTransaction {
        stores,
        mode,
        optional_stores: true,
        actions,
    }
}

fn clear_world_transactions(next_action_id: &mut u32, world_id: &str) -> Vec<StorageTransaction> {
    ORDINARY_WORLD_RECORD_STORES
        .iter()
        .copied()
        .map(|store| {
            let action = StorageAction {
                id: *next_action_id,
                kind: StorageActionKind::DeleteIndexRange {
                    store,
                    index: "world-id",
                    key: world_id.to_owned(),
                },
            };
            *next_action_id = next_action_id.saturating_add(1);
            optional_transaction(vec![store], StorageMode::ReadWrite, vec![action])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use mclone_server::WorldGenerationProfile;

    use super::*;

    const BACKEND_LABEL: &str = "web-indexeddb";

    fn id(value: &str) -> LocalWorldId {
        LocalWorldId::new(value).unwrap()
    }

    fn summary(value: &str, created: u64) -> LocalWorldSummary {
        let mut summary = LocalWorldSummary::new(id(value), value, 17, created).unwrap();
        summary.last_played_unix_millis = Some(created);
        summary.backend_label = Some(BACKEND_LABEL.to_owned());
        summary
    }

    fn execution(
        request: WorldCatalogRequest,
        active_world: Option<LocalWorldId>,
    ) -> Result<CatalogExecutionCore, String> {
        CatalogExecutionCore::new(request, active_world, BACKEND_LABEL)
    }

    fn only_action(step: &StorageStep) -> &StorageActionKind {
        assert_eq!(step.transactions.len(), 1);
        assert_eq!(step.transactions[0].actions.len(), 1);
        &step.transactions[0].actions[0].kind
    }

    fn accept(
        execution: &mut CatalogExecutionCore,
        step: &StorageStep,
        result: CatalogReadResult,
        timestamp: Option<u64>,
    ) -> Vec<StorageAction> {
        let action_id = step.transactions[0].actions[0].id;
        execution
            .accept_read_result(step.id, action_id, result, timestamp)
            .unwrap()
    }

    #[test]
    fn list_is_one_readonly_get_all_and_sorts_in_rust() {
        let mut execution = execution(WorldCatalogRequest::ListWorlds, None).unwrap();
        assert!(execution.required_writer_world_ids().is_empty());
        let step = execution.next_step().unwrap().unwrap();
        assert_eq!(step.transactions[0].mode, StorageMode::ReadOnly);
        assert!(matches!(
            only_action(&step),
            StorageActionKind::GetAll { .. }
        ));
        accept(
            &mut execution,
            &step,
            CatalogReadResult::All(vec![summary("older", 1), summary("newer", 2)]),
            None,
        );
        execution.complete_step(step.id).unwrap();
        let WorldCatalogResponse::WorldList { worlds, .. } = execution.response().unwrap() else {
            panic!("list response expected");
        };
        assert_eq!(worlds[0].id, id("newer"));
    }

    #[test]
    fn create_reads_then_adds_in_a_separate_transaction() {
        let options = LocalWorldCreateOptions::new("Created", i64::MIN + 9)
            .unwrap()
            .with_requested_id(id("created"))
            .with_world_generation_profile(WorldGenerationProfile::alpha_v1(false));
        let mut execution = execution(WorldCatalogRequest::CreateWorld { options }, None).unwrap();
        let read = execution.next_step().unwrap().unwrap();
        accept(
            &mut execution,
            &read,
            CatalogReadResult::All(Vec::new()),
            Some(9_007_199_254_740_993),
        );
        execution.complete_step(read.id).unwrap();
        let add = execution.next_step().unwrap().unwrap();
        assert_eq!(add.transactions[0].mode, StorageMode::ReadWrite);
        let StorageActionKind::AddCatalogRecord(created) = only_action(&add) else {
            panic!("create must use add");
        };
        assert_eq!(created.seed, i64::MIN + 9);
        assert_eq!(created.created_unix_millis, 9_007_199_254_740_993);
        assert_eq!(created.backend_label.as_deref(), Some(BACKEND_LABEL));
        execution.complete_step(add.id).unwrap();
        assert!(execution.is_complete());
    }

    #[test]
    fn stored_descriptor_values_remain_full_width() {
        let mut stored = summary("full-width", 9_007_199_254_740_993);
        stored.seed = i64::MIN + 9;
        stored.last_played_unix_millis = Some(u64::MAX - 9);
        let mut execution = execution(WorldCatalogRequest::ListWorlds, None).unwrap();
        let step = execution.next_step().unwrap().unwrap();
        accept(
            &mut execution,
            &step,
            CatalogReadResult::All(vec![stored.clone()]),
            None,
        );
        execution.complete_step(step.id).unwrap();
        let WorldCatalogResponse::WorldList { worlds, .. } = execution.response().unwrap() else {
            panic!("list response expected");
        };
        assert_eq!(worlds, &[stored]);
    }

    #[test]
    fn newly_created_world_seed_remains_full_width() {
        let seed = 553_534_047_293_117_028;
        let options = LocalWorldCreateOptions::new("Mclone Wild", seed)
            .unwrap()
            .with_requested_id(id("mclone-wild"));
        let mut execution = execution(WorldCatalogRequest::CreateWorld { options }, None).unwrap();
        let read = execution.next_step().unwrap().unwrap();
        accept(
            &mut execution,
            &read,
            CatalogReadResult::All(Vec::new()),
            Some(1),
        );
        execution.complete_step(read.id).unwrap();
        let add = execution.next_step().unwrap().unwrap();
        let StorageActionKind::AddCatalogRecord(created) = only_action(&add) else {
            panic!("create must add a catalog record");
        };
        assert_eq!(created.seed, seed);
    }

    #[test]
    fn duplicate_create_is_rejected_before_add() {
        let options = LocalWorldCreateOptions::new("Duplicate", 1)
            .unwrap()
            .with_requested_id(id("duplicate"));
        let mut execution = execution(WorldCatalogRequest::CreateWorld { options }, None).unwrap();
        let read = execution.next_step().unwrap().unwrap();
        let error = execution
            .accept_read_result(
                read.id,
                read.transactions[0].actions[0].id,
                CatalogReadResult::All(vec![summary("duplicate", 1)]),
                Some(2),
            )
            .unwrap_err();
        assert!(error.contains("already exists"));
    }

    #[test]
    fn open_reads_then_puts_but_record_played_puts_in_same_transaction() {
        let mut open = execution(WorldCatalogRequest::OpenWorld { id: id("world") }, None).unwrap();
        let read = open.next_step().unwrap().unwrap();
        accept(
            &mut open,
            &read,
            CatalogReadResult::One(Some(summary("world", 1))),
            Some(10),
        );
        open.complete_step(read.id).unwrap();
        let write = open.next_step().unwrap().unwrap();
        assert!(matches!(
            only_action(&write),
            StorageActionKind::PutCatalogRecord(_)
        ));

        let mut played = execution(
            WorldCatalogRequest::RecordWorldPlayed { id: id("world") },
            None,
        )
        .unwrap();
        let transaction = played.next_step().unwrap().unwrap();
        assert_eq!(transaction.transactions[0].mode, StorageMode::ReadWrite);
        let followups = accept(
            &mut played,
            &transaction,
            CatalogReadResult::One(Some(summary("world", 1))),
            Some(11),
        );
        assert!(matches!(
            followups.as_slice(),
            [StorageAction {
                kind: StorageActionKind::PutCatalogRecord(_),
                ..
            }]
        ));
        played.complete_step(transaction.id).unwrap();
        assert!(played.is_complete());
    }

    #[test]
    fn delete_preserves_read_parallel_clear_and_catalog_delete_order() {
        let mut execution =
            execution(WorldCatalogRequest::DeleteWorld { id: id("world") }, None).unwrap();
        assert_eq!(execution.required_writer_world_ids(), vec![id("world")]);
        let read = execution.next_step().unwrap().unwrap();
        accept(
            &mut execution,
            &read,
            CatalogReadResult::One(Some(summary("world", 1))),
            None,
        );
        execution.complete_step(read.id).unwrap();
        let clears = execution.next_step().unwrap().unwrap();
        assert_eq!(
            clears.transactions.len(),
            ORDINARY_WORLD_RECORD_STORES.len()
        );
        assert!(clears.transactions.iter().all(|transaction| {
            transaction.optional_stores
                && matches!(
                    transaction.actions[0].kind,
                    StorageActionKind::DeleteIndexRange { .. }
                )
        }));
        execution.complete_step(clears.id).unwrap();
        let delete = execution.next_step().unwrap().unwrap();
        assert!(matches!(
            only_action(&delete),
            StorageActionKind::DeleteKey { .. }
        ));
        execution.complete_step(delete.id).unwrap();
        assert_eq!(
            execution.response().unwrap(),
            &WorldCatalogResponse::WorldDeleted { id: id("world") }
        );
    }

    #[test]
    fn delete_all_repeats_per_world_and_factory_reset_is_one_final_clear() {
        let mut execution = execution(
            WorldCatalogRequest::DeleteAllLocalWorlds {
                include_app_private_content: true,
            },
            None,
        )
        .unwrap();
        assert!(execution.required_writer_world_ids().is_empty());
        let list = execution.next_step().unwrap().unwrap();
        accept(
            &mut execution,
            &list,
            CatalogReadResult::All(vec![summary("world", 1)]),
            None,
        );
        execution.complete_step(list.id).unwrap();
        assert_eq!(execution.required_writer_world_ids(), vec![id("world")]);
        let read = execution.next_step().unwrap().unwrap();
        accept(
            &mut execution,
            &read,
            CatalogReadResult::One(Some(summary("world", 1))),
            None,
        );
        execution.complete_step(read.id).unwrap();
        let clears = execution.next_step().unwrap().unwrap();
        execution.complete_step(clears.id).unwrap();
        let delete = execution.next_step().unwrap().unwrap();
        execution.complete_step(delete.id).unwrap();
        let reset = execution.next_step().unwrap().unwrap();
        assert_eq!(reset.transactions.len(), 1);
        assert!(reset.transactions[0].optional_stores);
        assert_eq!(
            reset.transactions[0].stores.len(),
            FACTORY_RESET_STORES.len()
        );
        execution.complete_step(reset.id).unwrap();
        assert_eq!(
            execution.response().unwrap(),
            &WorldCatalogResponse::AllLocalWorldsDeleted { deleted_count: 1 }
        );
    }

    #[test]
    fn active_delete_and_reset_are_refused() {
        assert!(
            execution(
                WorldCatalogRequest::DeleteAllLocalWorlds {
                    include_app_private_content: false,
                },
                Some(id("active")),
            )
            .unwrap_err()
            .contains("Quit to title")
        );

        let mut execution = execution(
            WorldCatalogRequest::DeleteWorld { id: id("active") },
            Some(id("active")),
        )
        .unwrap();
        let read = execution.next_step().unwrap().unwrap();
        let error = execution
            .accept_read_result(
                read.id,
                read.transactions[0].actions[0].id,
                CatalogReadResult::One(Some(summary("active", 1))),
                None,
            )
            .unwrap_err();
        assert!(error.contains("cannot delete active local world"));
    }

    #[test]
    fn stale_and_duplicate_storage_completions_are_rejected() {
        let mut execution = execution(WorldCatalogRequest::ListWorlds, None).unwrap();
        let step = execution.next_step().unwrap().unwrap();
        assert!(execution.complete_step(step.id + 1).is_err());
        accept(
            &mut execution,
            &step,
            CatalogReadResult::All(Vec::new()),
            None,
        );
        assert!(
            execution
                .accept_read_result(
                    step.id,
                    step.transactions[0].actions[0].id,
                    CatalogReadResult::All(Vec::new()),
                    None,
                )
                .is_err()
        );
        execution.complete_step(step.id).unwrap();
        assert!(execution.complete_step(step.id).is_err());
    }
}
