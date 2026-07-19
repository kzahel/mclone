//! Rust-owned continuation for ordinary browser world-catalog operations.
//!
//! The continuation selects catalog policy and emits only storage-level
//! transaction plans. TypeScript maps stable store identifiers to IndexedDB,
//! executes browser requests, and synchronously returns read results. No Rust
//! borrow or JavaScript view survives an asynchronous gap.

use mclone_app_runtime::world_catalog::{
    LocalWorldCreateOptions, LocalWorldId, LocalWorldSummary, WorldCatalogCapabilities,
    WorldCatalogError, WorldCatalogErrorKind, WorldCatalogRequest, WorldCatalogResponse,
    duplicate_world_id, sort_local_world_summaries, validate_delete_inactive_world,
    validate_local_world_compatible, world_not_found,
};

pub(crate) const WEB_WORLD_BACKEND_LABEL: &str = "web-indexeddb";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StorageStore {
    Catalog,
    DimensionChunks,
    DimensionEntityChunks,
    LegacyChunks,
    LegacyEntityChunks,
    Dimensions,
    Players,
    WorldMetadata,
    ManagedWorldMetadata,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl StorageStore {
    const fn label(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::DimensionChunks => "dimension-chunks",
            Self::DimensionEntityChunks => "dimension-entity-chunks",
            Self::LegacyChunks => "legacy-chunks",
            Self::LegacyEntityChunks => "legacy-entity-chunks",
            Self::Dimensions => "dimensions",
            Self::Players => "players",
            Self::WorldMetadata => "world-metadata",
            Self::ManagedWorldMetadata => "managed-world-metadata",
        }
    }
}

const ORDINARY_WORLD_RECORD_STORES: [StorageStore; 7] = [
    StorageStore::DimensionChunks,
    StorageStore::DimensionEntityChunks,
    StorageStore::LegacyChunks,
    StorageStore::LegacyEntityChunks,
    StorageStore::Dimensions,
    StorageStore::Players,
    StorageStore::WorldMetadata,
];

const FACTORY_RESET_STORES: [StorageStore; 8] = [
    StorageStore::ManagedWorldMetadata,
    StorageStore::DimensionChunks,
    StorageStore::DimensionEntityChunks,
    StorageStore::LegacyChunks,
    StorageStore::LegacyEntityChunks,
    StorageStore::Dimensions,
    StorageStore::Players,
    StorageStore::WorldMetadata,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StorageMode {
    ReadOnly,
    ReadWrite,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl StorageMode {
    const fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "readonly",
            Self::ReadWrite => "readwrite",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StorageActionKind {
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

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl StorageActionKind {
    const fn store(&self) -> StorageStore {
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
struct StorageAction {
    id: u32,
    kind: StorageActionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StorageTransaction {
    stores: Vec<StorageStore>,
    mode: StorageMode,
    optional_stores: bool,
    actions: Vec<StorageAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StorageStep {
    id: u32,
    transactions: Vec<StorageTransaction>,
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
        include_managed_content: bool,
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
enum CatalogReadResult {
    All(Vec<LocalWorldSummary>),
    One(Option<LocalWorldSummary>),
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CatalogReadShape {
    All,
    One,
}

#[derive(Debug)]
struct CatalogExecutionCore {
    flow: CatalogFlow,
    active_world: Option<LocalWorldId>,
    next_step_id: u32,
    next_action_id: u32,
    outstanding: Option<OutstandingStep>,
    response: Option<WorldCatalogResponse>,
}

impl CatalogExecutionCore {
    fn new(
        request: WorldCatalogRequest,
        active_world: Option<LocalWorldId>,
    ) -> Result<Self, String> {
        let flow = match request {
            WorldCatalogRequest::ListWorlds => CatalogFlow::List,
            WorldCatalogRequest::CreateWorld { mut options } => {
                // Preserve the pre-cutover browser creation contract. The UI's
                // typed i64 was projected through a JavaScript Number before
                // catalog policy consumed it, including IEEE-754 rounding for
                // values outside the exact integer range. Descriptor reads and
                // existing stored seeds remain full width.
                options.seed = options.seed as f64 as i64;
                CatalogFlow::Create {
                    options,
                    summary: None,
                }
            }
            WorldCatalogRequest::OpenWorld { id } => CatalogFlow::Open { id, summary: None },
            WorldCatalogRequest::RecordWorldPlayed { id } => CatalogFlow::RecordPlayed { id },
            WorldCatalogRequest::DeleteWorld { id } => CatalogFlow::Delete {
                id,
                summary: None,
                cleared: false,
            },
            WorldCatalogRequest::DeleteAllLocalWorlds {
                include_managed_content,
            } => {
                if active_world.is_some() {
                    return Err("Quit to title before deleting all local worlds".to_owned());
                }
                CatalogFlow::DeleteMany {
                    include_managed_content,
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
            next_step_id: 1,
            next_action_id: 1,
            outstanding: None,
            response: None,
        })
    }

    fn is_complete(&self) -> bool {
        self.response.is_some()
    }

    fn response(&self) -> Result<&WorldCatalogResponse, String> {
        self.response
            .as_ref()
            .ok_or_else(|| "world catalog execution is not complete".to_owned())
    }

    fn next_step(&mut self) -> Result<Option<StorageStep>, String> {
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
                include_managed_content,
                world_ids,
                deleted_count,
                index,
                stage,
            } => {
                let Some(world_ids) = world_ids else {
                    return self.read_all_step(StorageMode::ReadOnly, ReadPurpose::DeleteManyList);
                };
                let Some(id) = world_ids.get(index).cloned() else {
                    if include_managed_content {
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

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn pending_read_shape(&self, step_id: u32, action_id: u32) -> Result<CatalogReadShape, String> {
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

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn pending_read_needs_timestamp(&self, step_id: u32, action_id: u32) -> Result<bool, String> {
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

    fn accept_read_result(
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
                summary.last_played_unix_millis = Some(now);
                summary.backend_label = Some(WEB_WORLD_BACKEND_LABEL.to_owned());
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

    fn complete_step(&mut self, step_id: u32) -> Result<(), String> {
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

#[cfg(target_arch = "wasm32")]
mod wasm {
    use js_sys::{Array, Object, Reflect};
    use wasm_bindgen::prelude::*;

    use super::*;
    use crate::web_canvas::{
        decode_web_local_world_create_options, decode_web_local_world_summaries,
        decode_web_local_world_summary, encode_web_catalog_record, encode_web_local_world_summary,
        parse_web_unix_millis,
    };

    #[wasm_bindgen]
    pub struct WebCatalogExecution {
        core: CatalogExecutionCore,
    }

    impl WebCatalogExecution {
        pub(crate) fn new(
            request: WorldCatalogRequest,
            active_world: Option<LocalWorldId>,
        ) -> Result<Self, String> {
            Ok(Self {
                core: CatalogExecutionCore::new(request, active_world)?,
            })
        }

        pub(crate) fn response(&self) -> Result<WorldCatalogResponse, String> {
            self.core.response().cloned()
        }
    }

    #[wasm_bindgen]
    impl WebCatalogExecution {
        #[wasm_bindgen(js_name = nextStorageStep)]
        pub fn next_storage_step(&mut self) -> Result<JsValue, JsValue> {
            self.core
                .next_step()
                .and_then(|step| step.map(encode_storage_step).transpose())
                .map(|step| step.unwrap_or(JsValue::UNDEFINED))
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = acceptStorageRead)]
        pub fn accept_storage_read(
            &mut self,
            step_id: f64,
            action_id: f64,
            value: JsValue,
            now_unix_millis: f64,
        ) -> Result<Array, JsValue> {
            let step_id = exact_u32(step_id, "catalog storage step id")?;
            let action_id = exact_u32(action_id, "catalog storage action id")?;
            let shape = self
                .core
                .pending_read_shape(step_id, action_id)
                .map_err(|error| JsValue::from_str(&error))?;
            let result = match shape {
                CatalogReadShape::All => {
                    CatalogReadResult::All(decode_web_local_world_summaries(&value).map_err(
                        |error| JsValue::from_str(&format!("decode catalog storage rows: {error}")),
                    )?)
                }
                CatalogReadShape::One => {
                    let summary = if value.is_null() || value.is_undefined() {
                        None
                    } else {
                        Some(decode_web_local_world_summary(&value).map_err(|error| {
                            JsValue::from_str(&format!("decode catalog storage row: {error}"))
                        })?)
                    };
                    CatalogReadResult::One(summary)
                }
            };
            let timestamp = self
                .core
                .pending_read_needs_timestamp(step_id, action_id)
                .map_err(|error| JsValue::from_str(&error))?
                .then(|| parse_web_unix_millis(now_unix_millis, "nowUnixMillis"))
                .transpose()
                .map_err(|error| JsValue::from_str(&error))?;
            let followups = self
                .core
                .accept_read_result(step_id, action_id, result, timestamp)
                .map_err(|error| JsValue::from_str(&error))?;
            let array = Array::new();
            for action in followups {
                array.push(&encode_storage_action(&action).map_err(|error| {
                    JsValue::from_str(&format!("encode catalog follow-up action: {error}"))
                })?);
            }
            Ok(array)
        }

        #[wasm_bindgen(js_name = completeStorageStep)]
        pub fn complete_storage_step(&mut self, step_id: f64) -> Result<(), JsValue> {
            self.core
                .complete_step(exact_u32(step_id, "catalog storage step id")?)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = isComplete)]
        pub fn is_complete(&self) -> bool {
            self.core.is_complete()
        }

        #[wasm_bindgen(js_name = responseForSmoke)]
        pub fn response_for_smoke(&self) -> Result<JsValue, JsValue> {
            encode_smoke_response(
                self.core
                    .response()
                    .map_err(|error| JsValue::from_str(&error))?,
            )
            .map_err(|error| JsValue::from_str(&error))
        }
    }

    #[wasm_bindgen(js_name = mclone_web_catalog_smoke_execution)]
    pub fn catalog_smoke_execution(
        operation: String,
        options: JsValue,
        active_world_id: String,
    ) -> Result<WebCatalogExecution, JsValue> {
        let id = || -> Result<LocalWorldId, JsValue> {
            LocalWorldId::new(required_string(&options, "id")?)
                .map_err(|error| JsValue::from_str(&error.message))
        };
        let request = match operation.as_str() {
            "listWorlds" => WorldCatalogRequest::ListWorlds,
            "createWorld" => WorldCatalogRequest::CreateWorld {
                options: decode_web_local_world_create_options(&options)
                    .map_err(|error| JsValue::from_str(&error))?,
            },
            "openWorld" => WorldCatalogRequest::OpenWorld { id: id()? },
            "recordWorldPlayed" => WorldCatalogRequest::RecordWorldPlayed { id: id()? },
            "deleteWorld" => WorldCatalogRequest::DeleteWorld { id: id()? },
            "deleteAllLocalWorlds" => WorldCatalogRequest::DeleteAllLocalWorlds {
                include_managed_content: false,
            },
            "factoryResetLocalData" => WorldCatalogRequest::DeleteAllLocalWorlds {
                include_managed_content: true,
            },
            _ => {
                return Err(JsValue::from_str(&format!(
                    "unsupported catalog smoke operation {operation:?}"
                )));
            }
        };
        let active_world = if active_world_id.trim().is_empty() {
            None
        } else {
            Some(
                LocalWorldId::new(active_world_id)
                    .map_err(|error| JsValue::from_str(&error.message))?,
            )
        };
        WebCatalogExecution::new(request, active_world).map_err(|error| JsValue::from_str(&error))
    }

    fn encode_storage_step(step: StorageStep) -> Result<JsValue, String> {
        let object = Object::new();
        set_number(&object, "stepId", f64::from(step.id))?;
        let transactions = Array::new();
        for transaction in step.transactions {
            let encoded = Object::new();
            let stores = Array::new();
            for store in transaction.stores {
                stores.push(&JsValue::from_str(store.label()));
            }
            set_value(&encoded, "stores", &stores)?;
            set_string(&encoded, "mode", transaction.mode.label())?;
            set_bool(&encoded, "optionalStores", transaction.optional_stores)?;
            let actions = Array::new();
            for action in transaction.actions {
                actions.push(&encode_storage_action(&action)?);
            }
            set_value(&encoded, "actions", &actions)?;
            transactions.push(&encoded);
        }
        set_value(&object, "transactions", &transactions)?;
        Ok(object.into())
    }

    fn encode_storage_action(action: &StorageAction) -> Result<JsValue, String> {
        let object = Object::new();
        set_number(&object, "actionId", f64::from(action.id))?;
        set_string(&object, "store", action.kind.store().label())?;
        match &action.kind {
            StorageActionKind::GetAll {
                needs_timestamp, ..
            } => {
                set_string(&object, "kind", "get-all")?;
                set_bool(&object, "needsTimestamp", *needs_timestamp)?;
            }
            StorageActionKind::Get {
                key,
                needs_timestamp,
                ..
            } => {
                set_string(&object, "kind", "get")?;
                set_string(&object, "key", key)?;
                set_bool(&object, "needsTimestamp", *needs_timestamp)?;
            }
            StorageActionKind::AddCatalogRecord(summary) => {
                set_string(&object, "kind", "add")?;
                set_value(&object, "value", &encode_web_catalog_record(summary)?)?;
            }
            StorageActionKind::PutCatalogRecord(summary) => {
                set_string(&object, "kind", "put")?;
                set_value(&object, "value", &encode_web_catalog_record(summary)?)?;
            }
            StorageActionKind::DeleteKey { key, .. } => {
                set_string(&object, "kind", "delete-key")?;
                set_string(&object, "key", key)?;
            }
            StorageActionKind::DeleteIndexRange { index, key, .. } => {
                set_string(&object, "kind", "delete-index-range")?;
                set_string(&object, "index", index)?;
                set_string(&object, "key", key)?;
            }
            StorageActionKind::Clear { .. } => set_string(&object, "kind", "clear")?,
        }
        Ok(object.into())
    }

    fn encode_smoke_response(response: &WorldCatalogResponse) -> Result<JsValue, String> {
        match response {
            WorldCatalogResponse::WorldList { worlds, .. } => {
                let array = Array::new();
                for world in worlds {
                    array.push(&encode_web_local_world_summary(world)?);
                }
                Ok(array.into())
            }
            WorldCatalogResponse::WorldCreated { summary }
            | WorldCatalogResponse::WorldOpened { summary }
            | WorldCatalogResponse::WorldPlayRecorded { summary } => {
                encode_web_local_world_summary(summary)
            }
            WorldCatalogResponse::WorldDeleted { id } => {
                let object = Object::new();
                set_string(&object, "id", id.as_str())?;
                Ok(object.into())
            }
            WorldCatalogResponse::AllLocalWorldsDeleted { deleted_count } => {
                let object = Object::new();
                set_number(&object, "deletedCount", *deleted_count as f64)?;
                Ok(object.into())
            }
        }
    }

    fn required_string(value: &JsValue, name: &str) -> Result<String, JsValue> {
        Reflect::get(value, &JsValue::from_str(name))
            .map_err(|error| JsValue::from_str(&format!("read catalog smoke field: {error:?}")))?
            .as_string()
            .ok_or_else(|| JsValue::from_str(&format!("catalog smoke field {name} is missing")))
    }

    fn exact_u32(value: f64, label: &str) -> Result<u32, JsValue> {
        if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > f64::from(u32::MAX)
        {
            return Err(JsValue::from_str(&format!("{label} must be a u32")));
        }
        Ok(value as u32)
    }

    fn set_value(object: &Object, name: &str, value: &JsValue) -> Result<(), String> {
        Reflect::set(object, &JsValue::from_str(name), value)
            .map(|_| ())
            .map_err(|error| format!("set catalog storage field {name}: {error:?}"))
    }

    fn set_string(object: &Object, name: &str, value: &str) -> Result<(), String> {
        set_value(object, name, &JsValue::from_str(value))
    }

    fn set_number(object: &Object, name: &str, value: f64) -> Result<(), String> {
        set_value(object, name, &JsValue::from_f64(value))
    }

    fn set_bool(object: &Object, name: &str, value: bool) -> Result<(), String> {
        set_value(object, name, &JsValue::from_bool(value))
    }

    pub use WebCatalogExecution as ExportedWebCatalogExecution;
}

#[cfg(target_arch = "wasm32")]
pub use wasm::ExportedWebCatalogExecution as WebCatalogExecution;

#[cfg(test)]
mod tests {
    use mclone_server::WorldGenerationProfile;

    use super::*;

    fn id(value: &str) -> LocalWorldId {
        LocalWorldId::new(value).unwrap()
    }

    fn summary(value: &str, created: u64) -> LocalWorldSummary {
        let mut summary = LocalWorldSummary::new(id(value), value, 17, created).unwrap();
        summary.last_played_unix_millis = Some(created);
        summary.backend_label = Some(WEB_WORLD_BACKEND_LABEL.to_owned());
        summary
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
        let mut execution =
            CatalogExecutionCore::new(WorldCatalogRequest::ListWorlds, None).unwrap();
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
        let mut execution =
            CatalogExecutionCore::new(WorldCatalogRequest::CreateWorld { options }, None).unwrap();
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
        assert_eq!(created.seed, (i64::MIN + 9) as f64 as i64);
        assert_eq!(created.created_unix_millis, 9_007_199_254_740_993);
        execution.complete_step(add.id).unwrap();
        assert!(execution.is_complete());
    }

    #[test]
    fn stored_descriptor_values_remain_full_width() {
        let mut stored = summary("full-width", 9_007_199_254_740_993);
        stored.seed = i64::MIN + 9;
        stored.last_played_unix_millis = Some(u64::MAX - 9);
        let mut execution =
            CatalogExecutionCore::new(WorldCatalogRequest::ListWorlds, None).unwrap();
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
    fn duplicate_create_is_rejected_before_add() {
        let options = LocalWorldCreateOptions::new("Duplicate", 1)
            .unwrap()
            .with_requested_id(id("duplicate"));
        let mut execution =
            CatalogExecutionCore::new(WorldCatalogRequest::CreateWorld { options }, None).unwrap();
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
        let mut open =
            CatalogExecutionCore::new(WorldCatalogRequest::OpenWorld { id: id("world") }, None)
                .unwrap();
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

        let mut played = CatalogExecutionCore::new(
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
            CatalogExecutionCore::new(WorldCatalogRequest::DeleteWorld { id: id("world") }, None)
                .unwrap();
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
        let mut execution = CatalogExecutionCore::new(
            WorldCatalogRequest::DeleteAllLocalWorlds {
                include_managed_content: true,
            },
            None,
        )
        .unwrap();
        let list = execution.next_step().unwrap().unwrap();
        accept(
            &mut execution,
            &list,
            CatalogReadResult::All(vec![summary("world", 1)]),
            None,
        );
        execution.complete_step(list.id).unwrap();
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
            CatalogExecutionCore::new(
                WorldCatalogRequest::DeleteAllLocalWorlds {
                    include_managed_content: false,
                },
                Some(id("active")),
            )
            .unwrap_err()
            .contains("Quit to title")
        );

        let mut execution = CatalogExecutionCore::new(
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
        let mut execution =
            CatalogExecutionCore::new(WorldCatalogRequest::ListWorlds, None).unwrap();
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
