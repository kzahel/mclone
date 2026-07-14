//! Native filesystem/SQLite execution for shared managed scenario plans.
//!
//! Scenario policy and identity are shared. This native executor resolves that
//! path-free intent into versioned, app-private filesystem content without
//! admitting those worlds to the user world catalog.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result, anyhow, bail};
use mclone_server::{
    AUTHORED_WORLD_FIXTURE_MARKER_FILE, AuthoredWorldFixtureManifest, SqliteWorldStore,
    write_authored_world_fixture_dir,
};

use crate::platform_operation::{
    CancelledPlatformOperation, DeferredPlatformOperationHandle, PlatformOperation,
    PlatformOperationCompletion, PlatformOperationExecutor, PlatformOperationResolution,
    PlatformOperationService, PlatformOperationToken, deferred_platform_operation_executor,
};
use crate::scenario::{BuiltInScenarioId, ScenarioLaunchIntent};

use super::{
    LOBBY_PREVIEW_DIRECTORY, MANAGED_SCENARIO_MANIFEST_FILE, ManagedScenarioManifest,
    ManagedScenarioWorldManifest,
};

const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq)]
pub struct NativeManagedScenarioWorld {
    pub root: PathBuf,
    pub manifest: ManagedScenarioWorldManifest,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeManagedScenarioContent {
    pub root: PathBuf,
    pub manifest: ManagedScenarioManifest,
    pub primary: NativeManagedScenarioWorld,
    pub destination: NativeManagedScenarioWorld,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeManagedScenarioContentService {
    root: PathBuf,
}

impl NativeManagedScenarioContentService {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve(&self, intent: ScenarioLaunchIntent) -> Result<NativeManagedScenarioContent> {
        match intent.id {
            BuiltInScenarioId::LobbyPreview => self.ensure_lobby_preview_v1(),
        }
    }

    fn ensure_lobby_preview_v1(&self) -> Result<NativeManagedScenarioContent> {
        let expected = ManagedScenarioManifest::lobby_preview_v1();
        let published = self.root.join(LOBBY_PREVIEW_DIRECTORY);
        if published.exists() {
            return validate_published_scenario(&published, &expected);
        }

        fs::create_dir_all(&self.root)
            .with_context(|| format!("create managed scenario root `{}`", self.root.display()))?;
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let staging = self.root.join(format!(
            ".{LOBBY_PREVIEW_DIRECTORY}.staging-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&staging)
            .with_context(|| format!("create managed scenario staging `{}`", staging.display()))?;

        let build_result = build_staged_scenario(&staging, &expected);
        if let Err(error) = build_result {
            remove_owned_staging(&staging);
            return Err(error);
        }

        match fs::rename(&staging, &published) {
            Ok(()) => validate_published_scenario(&published, &expected),
            Err(error) if published.exists() => {
                remove_owned_staging(&staging);
                validate_published_scenario(&published, &expected).with_context(|| {
                    format!("another managed scenario publisher won after rename failed: {error}")
                })
            }
            Err(error) => {
                remove_owned_staging(&staging);
                Err(error).with_context(|| {
                    format!(
                        "publish managed scenario `{}` from `{}`",
                        published.display(),
                        staging.display()
                    )
                })
            }
        }
    }
}

struct BackgroundManagedScenarioExecutor {
    content: NativeManagedScenarioContentService,
    sender: mpsc::Sender<PlatformOperationCompletion<NativeManagedScenarioContent, String>>,
    receiver: mpsc::Receiver<PlatformOperationCompletion<NativeManagedScenarioContent, String>>,
    workers: Vec<JoinHandle<()>>,
    immediate: Vec<PlatformOperationCompletion<NativeManagedScenarioContent, String>>,
}

impl std::fmt::Debug for BackgroundManagedScenarioExecutor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BackgroundManagedScenarioExecutor")
            .field("root", &self.content.root())
            .field("workers", &self.workers.len())
            .field("immediate", &self.immediate.len())
            .finish()
    }
}

impl BackgroundManagedScenarioExecutor {
    fn new(content: NativeManagedScenarioContentService) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            content,
            sender,
            receiver,
            workers: Vec::new(),
            immediate: Vec::new(),
        }
    }

    fn reap_workers(&mut self) {
        let mut index = 0;
        while index < self.workers.len() {
            if self.workers[index].is_finished() {
                let worker = self.workers.swap_remove(index);
                if worker.join().is_err() {
                    log::warn!("managed scenario content worker panicked");
                }
            } else {
                index += 1;
            }
        }
    }
}

impl PlatformOperationExecutor<ScenarioLaunchIntent, NativeManagedScenarioContent, String>
    for BackgroundManagedScenarioExecutor
{
    fn submit(&mut self, operation: PlatformOperation<ScenarioLaunchIntent>) {
        self.reap_workers();
        let service = self.content.clone();
        let sender = self.sender.clone();
        let token = operation.token;
        let intent = operation.kind;
        match thread::Builder::new()
            .name(format!(
                "mclone-scenario-content-{}",
                token.request_id.get()
            ))
            .spawn(move || {
                let completion = PlatformOperationCompletion {
                    token,
                    result: service
                        .resolve(intent)
                        .map_err(|error| format!("{error:#}")),
                };
                let _ = sender.send(completion);
            }) {
            Ok(worker) => self.workers.push(worker),
            Err(error) => self.immediate.push(PlatformOperationCompletion {
                token,
                result: Err(format!("spawn managed scenario content worker: {error}")),
            }),
        }
    }

    fn try_recv_completion(
        &mut self,
    ) -> Option<PlatformOperationCompletion<NativeManagedScenarioContent, String>> {
        self.reap_workers();
        self.immediate
            .pop()
            .or_else(|| self.receiver.try_recv().ok())
    }
}

pub type ManagedScenarioContentResolution = PlatformOperationResolution<
    ScenarioLaunchIntent,
    ScenarioLaunchIntent,
    NativeManagedScenarioContent,
    String,
>;

pub type DeferredManagedScenarioContentHandle =
    DeferredPlatformOperationHandle<ScenarioLaunchIntent, NativeManagedScenarioContent, String>;

#[derive(Debug)]
pub struct NativeManagedScenarioContentOperationService {
    operations: PlatformOperationService<
        ScenarioLaunchIntent,
        ScenarioLaunchIntent,
        NativeManagedScenarioContent,
        String,
    >,
}

impl NativeManagedScenarioContentOperationService {
    pub fn background(root: impl Into<PathBuf>) -> Self {
        let content = NativeManagedScenarioContentService::new(root);
        Self::new(Box::new(BackgroundManagedScenarioExecutor::new(content)))
    }

    fn new(
        executor: Box<
            dyn PlatformOperationExecutor<
                    ScenarioLaunchIntent,
                    NativeManagedScenarioContent,
                    String,
                >,
        >,
    ) -> Self {
        Self {
            operations: PlatformOperationService::new(executor),
        }
    }

    pub fn deferred() -> (Self, DeferredManagedScenarioContentHandle) {
        let (executor, handle) = deferred_platform_operation_executor();
        (Self::new(Box::new(executor)), handle)
    }

    pub fn submit(&mut self, intent: ScenarioLaunchIntent) -> PlatformOperationToken {
        self.operations.issue(intent, intent)
    }

    pub fn poll(&mut self) -> Vec<ManagedScenarioContentResolution> {
        self.operations.poll()
    }

    pub fn pending_len(&self) -> usize {
        self.operations.pending_len()
    }

    pub fn begin_epoch(
        &mut self,
    ) -> Vec<CancelledPlatformOperation<ScenarioLaunchIntent, ScenarioLaunchIntent>> {
        self.operations.begin_epoch()
    }
}

/// Derive an app-private managed-scenario sibling without placing application
/// content inside the user-deletable world catalog.
pub fn native_managed_scenario_root_from_world_root(world_root: &Path) -> PathBuf {
    world_root.parent().unwrap_or(world_root).join("scenarios")
}

fn build_staged_scenario(root: &Path, expected: &ManagedScenarioManifest) -> Result<()> {
    let primary_root = root.join(&expected.primary.directory);
    let destination_root = root.join(&expected.destination.directory);
    write_authored_world_fixture_dir(&primary_root, expected.primary.fixture.kind)
        .context("build managed lobby content")?;
    write_authored_world_fixture_dir(&destination_root, expected.destination.fixture.kind)
        .context("build managed demo-island content")?;

    let bytes = serde_json::to_vec_pretty(expected).context("encode managed scenario manifest")?;
    fs::write(root.join(MANAGED_SCENARIO_MANIFEST_FILE), bytes)
        .context("write staged managed scenario manifest")?;
    validate_published_scenario(root, expected).map(|_| ())
}

fn validate_published_scenario(
    root: &Path,
    expected: &ManagedScenarioManifest,
) -> Result<NativeManagedScenarioContent> {
    if !root.is_dir() {
        bail!(
            "managed scenario path `{}` is not a directory",
            root.display()
        );
    }
    let manifest_path = root.join(MANAGED_SCENARIO_MANIFEST_FILE);
    let bytes = fs::read(&manifest_path).with_context(|| {
        format!(
            "read managed scenario manifest `{}`",
            manifest_path.display()
        )
    })?;
    let manifest: ManagedScenarioManifest = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "decode managed scenario manifest `{}`",
            manifest_path.display()
        )
    })?;
    if &manifest != expected {
        bail!(
            "managed scenario manifest `{}` does not match lobby-preview v1",
            manifest_path.display()
        );
    }

    let primary = validate_managed_world(root, &manifest.primary)?;
    let destination = validate_managed_world(root, &manifest.destination)?;
    Ok(NativeManagedScenarioContent {
        root: root.to_owned(),
        manifest,
        primary,
        destination,
    })
}

fn validate_managed_world(
    scenario_root: &Path,
    expected: &ManagedScenarioWorldManifest,
) -> Result<NativeManagedScenarioWorld> {
    let root = scenario_root.join(&expected.directory);
    if !root.is_dir() {
        bail!(
            "managed scenario content `{}` is not a directory",
            root.display()
        );
    }
    let marker_path = root.join(AUTHORED_WORLD_FIXTURE_MARKER_FILE);
    let marker_bytes = fs::read(&marker_path)
        .with_context(|| format!("read managed content marker `{}`", marker_path.display()))?;
    let marker: AuthoredWorldFixtureManifest = serde_json::from_slice(&marker_bytes)
        .with_context(|| format!("decode managed content marker `{}`", marker_path.display()))?;
    if marker != expected.fixture {
        bail!(
            "managed content marker `{}` does not match `{}`",
            marker_path.display(),
            expected.content_id
        );
    }

    let database = SqliteWorldStore::database_path_for_world_dir(&root);
    let mut file = File::open(&database)
        .with_context(|| format!("open managed world database `{}`", database.display()))?;
    let mut header = [0_u8; SQLITE_HEADER.len()];
    file.read_exact(&mut header)
        .with_context(|| format!("read managed world database `{}`", database.display()))?;
    if &header != SQLITE_HEADER {
        return Err(anyhow!(
            "managed world database `{}` has an invalid SQLite header",
            database.display()
        ));
    }

    Ok(NativeManagedScenarioWorld {
        root,
        manifest: expected.clone(),
    })
}

fn remove_owned_staging(staging: &Path) {
    if let Err(error) = fs::remove_dir_all(staging)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!(
            "failed to remove managed scenario staging `{}`: {error}",
            staging.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use mclone_core::{
        AIR_BLOCK_STATE_ID, ChunkPos, block_to_section_coord, chunk_section_index,
        local_block_coord, local_section_block_coord,
    };
    use mclone_server::WorldBehaviorProfile;
    use mclone_server::WorldStore;

    use super::*;
    use crate::world_catalog::NativeWorldCatalog;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn managed_lobby_is_versioned_reused_and_preserves_destination_edits() {
        let root = unique_test_root("reuse");
        let managed_root = root.join("scenarios");
        let service = NativeManagedScenarioContentService::new(&managed_root);
        let first = service
            .resolve(ScenarioLaunchIntent::lobby_preview())
            .unwrap();
        assert_eq!(first.root, managed_root.join(LOBBY_PREVIEW_DIRECTORY));
        assert_eq!(first.primary.root, first.root.join("lobby"));
        assert_eq!(first.destination.root, first.root.join("demo-island"));
        assert_eq!(
            first.primary.manifest.behavior_profile,
            WorldBehaviorProfile::ProtectedLobby
        );
        assert_eq!(
            first.destination.manifest.behavior_profile,
            WorldBehaviorProfile::Mutable
        );

        let mutation = first.destination.manifest.fixture.mutation_block;
        let chunk = ChunkPos::from_block_coords(mutation[0], mutation[2]);
        let mut store = SqliteWorldStore::open_world_dir(&first.destination.root).unwrap();
        let mut record = store.load_chunk(chunk).unwrap().unwrap();
        assert!(record.snapshot.patch_section_block(
            block_to_section_coord(mutation[1]),
            local_block_coord(mutation[0]),
            local_section_block_coord(mutation[1]),
            local_block_coord(mutation[2]),
            AIR_BLOCK_STATE_ID,
        ));
        store.save_chunk(&record).unwrap();
        store.flush().unwrap();
        store.close().unwrap();
        let database = SqliteWorldStore::database_path_for_world_dir(&first.destination.root);
        let edited_database = fs::read(&database).unwrap();

        let second = service
            .resolve(ScenarioLaunchIntent::lobby_preview())
            .unwrap();
        assert_eq!(second, first);
        assert_eq!(fs::read(&database).unwrap(), edited_database);
        let mut store = SqliteWorldStore::open_world_dir(&second.destination.root).unwrap();
        let record = store.load_chunk(chunk).unwrap().unwrap();
        let state = record
            .snapshot
            .sections
            .iter()
            .find(|section| section.section_y == block_to_section_coord(mutation[1]))
            .map(|section| {
                section.unpack_block_state_ids()[chunk_section_index(
                    local_block_coord(mutation[0]),
                    local_section_block_coord(mutation[1]),
                    local_block_coord(mutation[2]),
                )]
            })
            .unwrap_or(AIR_BLOCK_STATE_ID);
        assert_eq!(state, AIR_BLOCK_STATE_ID);
        store.close().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn managed_lobby_refuses_corrupt_or_unrelated_published_content() {
        let root = unique_test_root("reject");
        let service = NativeManagedScenarioContentService::new(root.join("scenarios"));
        let content = service
            .resolve(ScenarioLaunchIntent::lobby_preview())
            .unwrap();
        let manifest_path = content.root.join(MANAGED_SCENARIO_MANIFEST_FILE);
        fs::write(&manifest_path, b"not json").unwrap();
        let error = service
            .resolve(ScenarioLaunchIntent::lobby_preview())
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("decode managed scenario manifest")
        );
        assert_eq!(fs::read(&manifest_path).unwrap(), b"not json");

        let mismatched = ManagedScenarioManifest {
            content_version: 99,
            ..ManagedScenarioManifest::lobby_preview_v1()
        };
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&mismatched).unwrap(),
        )
        .unwrap();
        assert!(
            service
                .resolve(ScenarioLaunchIntent::lobby_preview())
                .unwrap_err()
                .to_string()
                .contains("does not match lobby-preview v1")
        );

        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&ManagedScenarioManifest::lobby_preview_v1()).unwrap(),
        )
        .unwrap();
        let database = SqliteWorldStore::database_path_for_world_dir(&content.destination.root);
        fs::write(&database, [b'x'; SQLITE_HEADER.len()]).unwrap();
        assert!(
            service
                .resolve(ScenarioLaunchIntent::lobby_preview())
                .unwrap_err()
                .to_string()
                .contains("invalid SQLite header")
        );
        fs::remove_dir_all(&root).unwrap();

        let unrelated = unique_test_root("unrelated");
        let published = unrelated.join("scenarios").join(LOBBY_PREVIEW_DIRECTORY);
        fs::create_dir_all(&published).unwrap();
        fs::write(published.join("keep.txt"), b"keep").unwrap();
        let service = NativeManagedScenarioContentService::new(unrelated.join("scenarios"));
        assert!(
            service
                .resolve(ScenarioLaunchIntent::lobby_preview())
                .unwrap_err()
                .to_string()
                .contains("read managed scenario manifest")
        );
        assert_eq!(fs::read(published.join("keep.txt")).unwrap(), b"keep");
        fs::remove_dir_all(unrelated).unwrap();
    }

    #[test]
    fn stale_staging_is_never_published_or_removed_as_owned_work() {
        let root = unique_test_root("stale");
        let managed_root = root.join("scenarios");
        let stale = managed_root.join(format!(".{LOBBY_PREVIEW_DIRECTORY}.staging-stale"));
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join("keep.txt"), b"stale").unwrap();

        let content = NativeManagedScenarioContentService::new(&managed_root)
            .resolve(ScenarioLaunchIntent::lobby_preview())
            .unwrap();
        assert_eq!(content.root, managed_root.join(LOBBY_PREVIEW_DIRECTORY));
        assert_eq!(fs::read(stale.join("keep.txt")).unwrap(), b"stale");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn managed_scenario_root_is_a_catalog_sibling() {
        assert_eq!(
            native_managed_scenario_root_from_world_root(Path::new("/app/mclone/worlds")),
            PathBuf::from("/app/mclone/scenarios")
        );
    }

    #[test]
    fn managed_worlds_never_enter_the_user_world_catalog() {
        let root = unique_test_root("catalog-separation");
        let world_root = root.join("worlds");
        let scenario_root = native_managed_scenario_root_from_world_root(&world_root);
        NativeManagedScenarioContentService::new(&scenario_root)
            .resolve(ScenarioLaunchIntent::lobby_preview())
            .unwrap();
        assert!(
            NativeWorldCatalog::new(&world_root)
                .list_worlds()
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_managed_content_resolution_can_retry_after_the_blocker_is_removed() {
        let root = unique_test_root("failure-retry");
        let managed_root = root.join("scenarios");
        let published = managed_root.join(LOBBY_PREVIEW_DIRECTORY);
        fs::create_dir_all(&published).unwrap();
        fs::write(published.join("unrelated.txt"), b"do not overwrite").unwrap();
        let service = NativeManagedScenarioContentService::new(&managed_root);

        let failure = service
            .resolve(ScenarioLaunchIntent::lobby_preview())
            .unwrap_err();
        assert!(
            failure
                .to_string()
                .contains("read managed scenario manifest")
        );
        assert_eq!(
            fs::read(published.join("unrelated.txt")).unwrap(),
            b"do not overwrite"
        );

        fs::remove_dir_all(&published).unwrap();
        let resolved = service
            .resolve(ScenarioLaunchIntent::lobby_preview())
            .unwrap();
        let reopened = service
            .resolve(ScenarioLaunchIntent::lobby_preview())
            .unwrap();
        assert_eq!(resolved, reopened);
        assert!(resolved.primary.root.exists());
        assert!(resolved.destination.root.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tokened_content_operations_reject_cancelled_and_duplicate_completions() {
        let (mut operations, handle) = NativeManagedScenarioContentOperationService::deferred();
        let intent = ScenarioLaunchIntent::lobby_preview();
        let token = operations.submit(intent);
        let submitted = handle.take_submitted().unwrap();
        assert_eq!(submitted.token, token);
        assert_eq!(submitted.kind, intent);

        let cancelled = operations.begin_epoch();
        assert_eq!(cancelled.len(), 1);
        let stale_value = NativeManagedScenarioContent {
            root: PathBuf::from("stale"),
            manifest: ManagedScenarioManifest::lobby_preview_v1(),
            primary: NativeManagedScenarioWorld {
                root: PathBuf::from("stale/lobby"),
                manifest: ManagedScenarioManifest::lobby_preview_v1().primary,
            },
            destination: NativeManagedScenarioWorld {
                root: PathBuf::from("stale/demo-island"),
                manifest: ManagedScenarioManifest::lobby_preview_v1().destination,
            },
        };
        let completion = PlatformOperationCompletion {
            token,
            result: Ok(stale_value.clone()),
        };
        handle.submit_completion(completion.clone());
        assert!(matches!(
            operations.poll().as_slice(),
            [PlatformOperationResolution::Stale(value)] if value == &completion
        ));

        let current = operations.submit(intent);
        let _ = handle.take_submitted().unwrap();
        let completion = PlatformOperationCompletion {
            token: current,
            result: Ok(stale_value),
        };
        handle.submit_completion(completion.clone());
        handle.submit_completion(completion);
        let resolutions = operations.poll();
        assert!(matches!(
            &resolutions[0],
            PlatformOperationResolution::Applied { token, .. } if *token == current
        ));
        assert!(matches!(
            &resolutions[1],
            PlatformOperationResolution::Duplicate(value) if value.token == current
        ));
    }

    #[test]
    fn background_content_operation_publishes_outside_the_caller_thread() {
        let root = unique_test_root("background");
        let mut operations =
            NativeManagedScenarioContentOperationService::background(root.join("scenarios"));
        operations.submit(ScenarioLaunchIntent::lobby_preview());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let resolved = loop {
            if let Some(resolution) = operations.poll().into_iter().next() {
                break resolution;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert!(matches!(
            resolved,
            PlatformOperationResolution::Applied { value, .. }
                if value.root.ends_with(LOBBY_PREVIEW_DIRECTORY)
        ));
        assert_eq!(operations.pending_len(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_background_resolves_converge_on_one_published_root() {
        let root = unique_test_root("concurrent");
        let mut operations =
            NativeManagedScenarioContentOperationService::background(root.join("scenarios"));
        operations.submit(ScenarioLaunchIntent::lobby_preview());
        operations.submit(ScenarioLaunchIntent::lobby_preview());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut values = Vec::new();
        while values.len() < 2 {
            for resolution in operations.poll() {
                match resolution {
                    PlatformOperationResolution::Applied { value, .. } => values.push(value),
                    other => panic!("unexpected concurrent resolution: {other:?}"),
                }
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(values[0], values[1]);
        assert_eq!(
            fs::read_dir(root.join("scenarios"))
                .unwrap()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_name() == LOBBY_PREVIEW_DIRECTORY)
                .count(),
            1
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn unique_test_root(label: &str) -> PathBuf {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "mclone-managed-scenario-{label}-{}-{sequence}",
            std::process::id()
        ))
    }
}
