//! Native filesystem/SQLite execution for shared managed scenario plans.
//!
//! Scenario policy and identity are shared. This native executor resolves that
//! path-free intent into versioned, app-private filesystem content without
//! admitting those worlds to the user world catalog.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result, anyhow, bail};
use mclone_server::{
    AUTHORED_WORLD_FIXTURE_MARKER_FILE, AuthoredWorldFixtureManifest, SqliteWorldStore, WorldStore,
    write_authored_world_fixture_dir,
};

use crate::platform_operation::{
    CancelledPlatformOperation, DeferredPlatformOperationHandle, PlatformOperation,
    PlatformOperationCompletion, PlatformOperationExecutor, PlatformOperationResolution,
    PlatformOperationService, PlatformOperationToken, deferred_platform_operation_executor,
};
use crate::scenario::{BuiltInScenarioId, ScenarioLaunchIntent};

use super::{
    LOBBY_PREVIEW_DIRECTORY, MANAGED_SCENARIO_MANIFEST_FILE, MANAGED_SCENARIO_WORLD_MANIFEST_FILE,
    ManagedScenarioManifest, ManagedScenarioWorldContent, ManagedScenarioWorldManifest,
    ManagedScenarioWorldRole, ManagedWorldKey, ProvisionManagedScenarioWorld,
    ProvisionedManagedScenarioWorld,
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
            BuiltInScenarioId::LobbyPreview => self.ensure_current_lobby_preview(),
        }
    }

    fn ensure_current_lobby_preview(&self) -> Result<NativeManagedScenarioContent> {
        let expected = ManagedScenarioManifest::current_lobby_preview();
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

#[derive(Debug)]
struct NativeProvisionCompletion {
    token: PlatformOperationToken,
    result: Result<(ProvisionedManagedScenarioWorld, PathBuf), String>,
}

/// Native execution adapter for the shared, externally tokened provisioning
/// operation. Paths remain here and are looked up only when native assembly
/// constructs an integrated-server runner.
#[derive(Debug)]
pub struct NativeManagedScenarioProvisionAdapter {
    content: NativeManagedScenarioContentService,
    sender: mpsc::Sender<NativeProvisionCompletion>,
    receiver: mpsc::Receiver<NativeProvisionCompletion>,
    workers: Vec<JoinHandle<()>>,
    world_dirs: HashMap<ManagedWorldKey, PathBuf>,
    immediate: Vec<NativeProvisionCompletion>,
}

impl NativeManagedScenarioProvisionAdapter {
    pub fn background(root: impl Into<PathBuf>) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            content: NativeManagedScenarioContentService::new(root),
            sender,
            receiver,
            workers: Vec::new(),
            world_dirs: HashMap::new(),
            immediate: Vec::new(),
        }
    }

    pub fn submit(&mut self, operation: PlatformOperation<ProvisionManagedScenarioWorld>) {
        self.reap_workers();
        let service = self.content.clone();
        let sender = self.sender.clone();
        let token = operation.token;
        let request = operation.kind;
        match thread::Builder::new()
            .name(format!("mclone-scenario-world-{}", token.request_id.get()))
            .spawn(move || {
                let result = service.resolve(request.intent).and_then(|content| {
                    let (world, key) = match request.role {
                        ManagedScenarioWorldRole::Primary => (
                            content.primary,
                            content
                                .manifest
                                .world_key(ManagedScenarioWorldRole::Primary)?,
                        ),
                        ManagedScenarioWorldRole::Destination => (
                            content.destination,
                            content
                                .manifest
                                .world_key(ManagedScenarioWorldRole::Destination)?,
                        ),
                    };
                    Ok((
                        ProvisionedManagedScenarioWorld {
                            role: request.role,
                            key,
                        },
                        world.root,
                    ))
                });
                let _ = sender.send(NativeProvisionCompletion {
                    token,
                    result: result.map_err(|error| format!("{error:#}")),
                });
            }) {
            Ok(worker) => self.workers.push(worker),
            Err(error) => self.immediate.push(NativeProvisionCompletion {
                token,
                result: Err(format!("spawn managed scenario content worker: {error}")),
            }),
        }
    }

    pub fn poll(
        &mut self,
    ) -> Vec<PlatformOperationCompletion<ProvisionedManagedScenarioWorld, String>> {
        self.reap_workers();
        let mut ready = self.immediate.drain(..).collect::<Vec<_>>();
        ready.extend(self.receiver.try_iter());
        ready
            .into_iter()
            .map(|completion| PlatformOperationCompletion {
                token: completion.token,
                result: completion.result.map(|(provisioned, world_dir)| {
                    self.world_dirs.insert(provisioned.key.clone(), world_dir);
                    provisioned
                }),
            })
            .collect()
    }

    pub fn world_dir(&self, key: &ManagedWorldKey) -> Option<&Path> {
        self.world_dirs.get(key).map(PathBuf::as_path)
    }

    fn reap_workers(&mut self) {
        let mut index = 0;
        while index < self.workers.len() {
            if self.workers[index].is_finished() {
                let worker = self.workers.swap_remove(index);
                if worker.join().is_err() {
                    log::warn!("managed scenario provision worker panicked");
                }
            } else {
                index += 1;
            }
        }
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
    build_managed_world(&primary_root, &expected.primary).context("build managed lobby content")?;
    build_managed_world(&destination_root, &expected.destination)
        .context("build managed destination content")?;

    let bytes = serde_json::to_vec_pretty(expected).context("encode managed scenario manifest")?;
    fs::write(root.join(MANAGED_SCENARIO_MANIFEST_FILE), bytes)
        .context("write staged managed scenario manifest")?;
    validate_published_scenario(root, expected).map(|_| ())
}

fn build_managed_world(root: &Path, expected: &ManagedScenarioWorldManifest) -> Result<()> {
    match &expected.content {
        ManagedScenarioWorldContent::AuthoredFixture { fixture } => {
            write_authored_world_fixture_dir(root, fixture.kind)?;
        }
        ManagedScenarioWorldContent::GeneratedOverworld { .. } => {
            fs::create_dir_all(root)
                .with_context(|| format!("create generated managed world `{}`", root.display()))?;
            let mut store = SqliteWorldStore::open_world_dir(root)?;
            store.flush()?;
            store.close()?;
            let bytes = serde_json::to_vec_pretty(expected)
                .context("encode generated managed-world manifest")?;
            fs::write(root.join(MANAGED_SCENARIO_WORLD_MANIFEST_FILE), bytes)
                .context("write generated managed-world manifest")?;
        }
    }
    Ok(())
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
            "managed scenario manifest `{}` does not match the requested content recipe",
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
    match &expected.content {
        ManagedScenarioWorldContent::AuthoredFixture { fixture } => {
            let marker_path = root.join(AUTHORED_WORLD_FIXTURE_MARKER_FILE);
            let marker_bytes = fs::read(&marker_path).with_context(|| {
                format!("read managed content marker `{}`", marker_path.display())
            })?;
            let marker: AuthoredWorldFixtureManifest = serde_json::from_slice(&marker_bytes)
                .with_context(|| {
                    format!("decode managed content marker `{}`", marker_path.display())
                })?;
            if &marker != fixture {
                bail!(
                    "managed content marker `{}` does not match `{}`",
                    marker_path.display(),
                    expected.content_id
                );
            }
        }
        ManagedScenarioWorldContent::GeneratedOverworld { .. } => {
            let marker_path = root.join(MANAGED_SCENARIO_WORLD_MANIFEST_FILE);
            let marker_bytes = fs::read(&marker_path).with_context(|| {
                format!("read generated world marker `{}`", marker_path.display())
            })?;
            let marker: ManagedScenarioWorldManifest = serde_json::from_slice(&marker_bytes)
                .with_context(|| {
                    format!("decode generated world marker `{}`", marker_path.display())
                })?;
            if &marker != expected {
                bail!(
                    "generated world marker `{}` does not match `{}`",
                    marker_path.display(),
                    expected.content_id
                );
            }
        }
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

    use mclone_server::WorldBehaviorProfile;
    use mclone_server::WorldStore;

    use super::*;
    use crate::scenario_content::{LOBBY_PREVIEW_V1_DIRECTORY, LOBBY_PREVIEW_V2_DIRECTORY};
    use crate::world_catalog::NativeWorldCatalog;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn managed_lobby_is_versioned_reused_and_preserves_generated_destination_store() {
        let root = unique_test_root("reuse");
        let managed_root = root.join("scenarios");
        let service = NativeManagedScenarioContentService::new(&managed_root);
        let first = service
            .resolve(ScenarioLaunchIntent::lobby_preview())
            .unwrap();
        assert_eq!(first.root, managed_root.join(LOBBY_PREVIEW_DIRECTORY));
        assert_eq!(first.primary.root, first.root.join("lobby"));
        assert_eq!(
            first.destination.root,
            first.root.join("fallback-overworld")
        );
        assert_eq!(
            first.primary.manifest.behavior_profile,
            WorldBehaviorProfile::ProtectedLobby
        );
        assert_eq!(
            first.destination.manifest.behavior_profile,
            WorldBehaviorProfile::Mutable
        );

        assert!(matches!(
            first.destination.manifest.content,
            ManagedScenarioWorldContent::GeneratedOverworld { .. }
        ));
        let mut store = SqliteWorldStore::open_world_dir(&first.destination.root).unwrap();
        store.flush().unwrap();
        store.close().unwrap();
        let database = SqliteWorldStore::database_path_for_world_dir(&first.destination.root);
        let original_database = fs::read(&database).unwrap();
        let retained = first.destination.root.join("retained-runtime-edit.txt");
        fs::write(&retained, b"runtime-owned").unwrap();

        let second = service
            .resolve(ScenarioLaunchIntent::lobby_preview())
            .unwrap();
        assert_eq!(second, first);
        assert_eq!(fs::read(&database).unwrap(), original_database);
        assert_eq!(fs::read(retained).unwrap(), b"runtime-owned");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn current_lobby_publication_leaves_v1_and_v2_content_untouched() {
        let root = unique_test_root("old-version-preservation");
        let managed_root = root.join("scenarios");
        let v1 = managed_root.join(LOBBY_PREVIEW_V1_DIRECTORY);
        let v2 = managed_root.join(LOBBY_PREVIEW_V2_DIRECTORY);
        fs::create_dir_all(&v1).unwrap();
        fs::create_dir_all(&v2).unwrap();
        fs::write(v1.join("keep.txt"), b"v1 remains owned by its old recipe").unwrap();
        fs::write(v2.join("keep.txt"), b"v2 remains owned by its old recipe").unwrap();

        let content = NativeManagedScenarioContentService::new(&managed_root)
            .resolve(ScenarioLaunchIntent::lobby_preview())
            .unwrap();

        assert_eq!(content.root, managed_root.join(LOBBY_PREVIEW_DIRECTORY));
        assert_eq!(
            content.manifest,
            ManagedScenarioManifest::lobby_preview_v3()
        );
        assert_eq!(
            fs::read(v1.join("keep.txt")).unwrap(),
            b"v1 remains owned by its old recipe"
        );
        assert_eq!(
            fs::read(v2.join("keep.txt")).unwrap(),
            b"v2 remains owned by its old recipe"
        );
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
                .contains("does not match the requested content recipe")
        );

        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&ManagedScenarioManifest::current_lobby_preview()).unwrap(),
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

    #[test]
    fn externally_tokened_adapter_keeps_paths_out_of_shared_completions() {
        let root = unique_test_root("external-tokens");
        let mut adapter = NativeManagedScenarioProvisionAdapter::background(root.join("scenarios"));
        let intent = ScenarioLaunchIntent::lobby_preview();
        let mut ledger = crate::platform_operation::PlatformOperationLedger::new();
        let primary = ledger.issue(
            ProvisionManagedScenarioWorld {
                intent,
                role: ManagedScenarioWorldRole::Primary,
            },
            intent,
        );
        let destination = ledger.issue(
            ProvisionManagedScenarioWorld {
                intent,
                role: ManagedScenarioWorldRole::Destination,
            },
            intent,
        );
        let tokens = [primary.token, destination.token];
        adapter.submit(destination);
        adapter.submit(primary);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut completions = Vec::new();
        while completions.len() < 2 {
            completions.extend(adapter.poll());
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        completions.sort_by_key(|completion| completion.token.request_id);
        assert_eq!(
            completions
                .iter()
                .map(|completion| completion.token)
                .collect::<Vec<_>>(),
            tokens
        );
        for completion in completions {
            let provisioned = completion.result.unwrap();
            assert!(adapter.world_dir(&provisioned.key).unwrap().is_dir());
            assert!(provisioned.key.as_str().starts_with("managed."));
        }
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
