//! Source ownership lock for Tactical 178's pre-refactor browser lobby debt.
//!
//! These assertions are intentionally transitional. Each slice updates the
//! corresponding assertion when it removes the named platform exception; an
//! accidental second implementation should never make the test green.

const APP_RUNTIME_LIB: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/mclone-app-runtime/src/lib.rs"
));
const CLIENT_EXPERIENCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/mclone-app-runtime/src/client_experience.rs"
));
const SCENE_SESSION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/mclone-scene/src/session.rs"
));
const WARM_WORLD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/mclone-scene/src/warm_world.rs"
));
const TERRAIN_RENDERER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/mclone-render/src/chunk.rs"
));
const WEB_APP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-web-app.ts"
));
const WEB_SMOKE_OBSERVER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-web-smoke-observer.ts"
));
const BROWSER_SMOKE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/scripts/browser-smoke.mjs"
));
const WEB_WORLD_CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-web-world-catalog.ts"
));
const WEB_RENDER_COMPILER_SHARED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-render-compiler-shared.ts"
));
const WEB_RENDER_COMPILER_WORKER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-render-compiler-worker.ts"
));
const WEB_CANVAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/web_canvas.rs"));
const WEB_SCENE_HOST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/web_scene_host.rs"
));
const WEB_RENDER_WORKER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/web_render_worker.rs"
));
const WEB_RENDER_WORKER_ACTOR: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/web_render_worker_actor.rs"
));
const WEB_SERVER_WORKER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/web_server_worker.rs"
));
const WEB_INTEGRATED_SERVER_STARTUP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/web_integrated_server_startup.rs"
));
const INTEGRATED_SERVER_WORKER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-integrated-server-worker.ts"
));

use mclone_app_runtime::scenario_content::{
    AppPrivateWorldKey, LobbyScenarioContent, LobbyWorldSource,
};

#[test]
fn shared_lobby_recipe_selects_storage_without_materializing_payloads() {
    let content = LobbyScenarioContent::current();
    assert_eq!(
        LobbyWorldSource::TransientAuthored(content.primary.kind).kind_label(),
        "transient-authored"
    );
    assert_eq!(
        LobbyWorldSource::AppPrivate(AppPrivateWorldKey::LobbyFallback).kind_label(),
        "app-private"
    );
}

#[test]
fn browser_runtime_honors_the_shared_lobby_actor_policy() {
    assert!(
        SCENE_SESSION
            .contains("scene.debug_passive_showcase = self.debug_lobby_auxiliary_player_script;")
    );
    assert!(
        WEB_SCENE_HOST
            .contains(".with_debug_passive_showcase(pending.scene.debug_passive_showcase)")
    );
    assert!(WEB_SERVER_WORKER.contains("self.config.debug_passive_showcase"));
    assert!(WEB_INTEGRATED_SERVER_STARTUP.contains("debug_passive_showcase: bool"));
    assert!(!INTEGRATED_SERVER_WORKER.contains("debugPassiveShowcase"));
}

#[test]
fn browser_auxiliary_player_control_only_enables_the_shared_rust_script() {
    assert!(SCENE_SESSION.contains("scene.debug_auxiliary_player_script ="));
    assert!(WEB_SCENE_HOST.contains(
        ".with_debug_auxiliary_player_script(pending.scene.debug_auxiliary_player_script)"
    ));
    assert!(WEB_SERVER_WORKER.contains("self.config.debug_auxiliary_player_script"));
    assert!(WEB_INTEGRATED_SERVER_STARTUP.contains("debug_auxiliary_player_script: bool"));
    assert!(!INTEGRATED_SERVER_WORKER.contains("debugAuxiliaryPlayerScript"));
    assert!(WEB_SMOKE_OBSERVER.contains("Object.assign(runtime.state, report)"));
    for forwarded_fact in [
        "embeddedPreviewRemotePlayerObservationCount",
        "embeddedPreviewFirstRemotePlayerId",
        "embeddedPreviewFirstRemotePlayerSourceX",
        "embeddedPreviewRemotePlayerMotionSequence",
        "embeddedPreviewRemotePlayerMotionFromSourceX",
        "embeddedPreviewRemotePlayerMotionToCompositionX",
    ] {
        assert!(
            BROWSER_SMOKE.contains(forwarded_fact),
            "browser smoke must consume shared Rust receipt {forwarded_fact}"
        );
    }
    for forbidden in [
        "MovePlayer",
        "RemotePlayerAdd",
        "RemotePlayerUpdate",
        "ServerUpdate",
    ] {
        assert!(
            !INTEGRATED_SERVER_WORKER.contains(forbidden),
            "TypeScript integrated-server Worker must not author {forbidden}"
        );
    }
}

#[test]
fn integrated_server_startup_domain_is_an_opaque_rust_frame() {
    assert!(WEB_SERVER_WORKER.contains("WebIntegratedServerStartupConfig"));
    assert!(WEB_SERVER_WORKER.contains("\"startupFrame\""));
    assert!(WEB_SERVER_WORKER.contains("pub struct WebIntegratedServerStartup"));
    assert!(WEB_SERVER_WORKER.contains("pub struct WebIntegratedServerActor"));
    assert!(WEB_SERVER_WORKER.contains("enum WebIntegratedServerOperationKind"));
    assert!(WEB_SERVER_WORKER.contains("MAX_INTEGRATED_SERVER_PENDING_JOB_POLLS"));
    assert!(WEB_INTEGRATED_SERVER_STARTUP.contains("const STARTUP_MAGIC"));
    assert!(WEB_INTEGRATED_SERVER_STARTUP.contains("const STARTUP_VERSION"));
    assert!(INTEGRATED_SERVER_WORKER.contains("new module.WebIntegratedServerStartup("));
    assert!(INTEGRATED_SERVER_WORKER.contains("startup.createTransient("));
    assert!(INTEGRATED_SERVER_WORKER.contains("startup.indexedDbBootstrapRequests()"));
    assert!(INTEGRATED_SERVER_WORKER.contains("startup.createIndexedDb("));
    assert!(INTEGRATED_SERVER_WORKER.contains("activeServer.beginMessage("));
    assert!(INTEGRATED_SERVER_WORKER.contains("activeServer.hasPendingJobs()"));
    assert!(INTEGRATED_SERVER_WORKER.contains("activeServer.finishOperation("));
    for forbidden in [
        "generationProfile",
        "worldTopology",
        "behaviorProfile",
        "lightStatusBatchSize",
        "freezeScheduledFluidTicks",
        "debugPassiveShowcase",
        "debugAuxiliaryPlayerScript",
        "observerOnly",
        "setLocalPlayerIdentity",
        "hasPendingServerJobs",
        "handleCommandFrame",
        "promoteObserverToPlayer",
        "demotePlayerToObserver",
        "kind: \"command-result\"",
    ] {
        assert!(
            !INTEGRATED_SERVER_WORKER.contains(forbidden),
            "TypeScript integrated-server startup must not own {forbidden}"
        );
    }
}

#[test]
fn shared_scenario_start_boundary_has_no_native_policy_stub_or_path() {
    assert!(APP_RUNTIME_LIB.contains("pub mod scenario_content;"));
    assert!(!APP_RUNTIME_LIB.contains(
        "#[cfg(not(target_arch = \"wasm32\"))]\n\
         pub mod scenario_content;"
    ));
    assert!(!CLIENT_EXPERIENCE.contains("web_client_experience_profile_without"));
    assert!(CLIENT_EXPERIENCE.contains("ClientExperienceCapabilityStatus::Supported"));
    assert!(!WEB_SCENE_HOST.contains("installManagedScenarioServices"));
    assert!(!WEB_APP.contains("installManagedScenarioServices"));
    assert!(SCENE_SESSION.contains("pub(crate) fn apply_lobby_effect("));
    let effect = SCENE_SESSION
        .split("pub(crate) fn apply_lobby_effect(")
        .nth(1)
        .expect("shared scenario effect exists")
        .split("pub fn begin_lobby_launch(")
        .next()
        .unwrap();
    assert!(!effect.contains("Ok(false)"));
    let request = WARM_WORLD
        .split("pub struct WarmWorldStandbyRequest {")
        .nth(1)
        .unwrap()
        .split("impl WarmWorldStandbyRequest")
        .next()
        .unwrap();
    assert!(!request.contains("PathBuf"));
    assert!(request.contains("pub storage_source: Option<LobbyWorldSource>"));
    assert!(!WARM_WORLD.contains("ProvisionManagedScenarioWorld"));
    assert!(!WARM_WORLD.contains("ManagedScenarioContentOperationService"));
}

#[test]
fn live_identity_moves_with_the_complete_world_slot() {
    assert!(WARM_WORLD.contains("pub struct WorldInstanceId(u64);"));
    let swap = SCENE_SESSION
        .split("std::mem::swap(\n            &mut self.active_world,")
        .nth(1)
        .expect("complete active-slot exchange remains explicit");
    assert!(swap.contains("self.standby_world"));
    assert!(!WARM_WORLD.contains("enum WorldInstanceId"));
}

#[test]
fn primary_and_destination_start_operations_are_independent() {
    assert!(WARM_WORLD.contains("LobbyWorldRole::Primary,"));
    assert!(SCENE_SESSION.contains("LobbyWorldRole::Destination =>"));
    assert!(WARM_WORLD.contains("start_operations.issue("));
    assert!(!SCENE_SESSION.contains("provision_request"));
    assert!(SCENE_SESSION.contains("take_lobby_world_start"));
    assert!(SCENE_SESSION.contains("complete_external_session_start"));
}

#[test]
fn stale_browser_starts_are_rejected_before_slot_installation() {
    let completion = SCENE_SESSION
        .split("pub fn complete_external_session_start(")
        .nth(1)
        .expect("shared external completion exists")
        .split("pub fn fail_external_session_start(")
        .next()
        .expect("completion precedes failure handling");
    let guard = completion
        .find("if !self.accept_external_scene_start(&pending)")
        .expect("shared operation-ledger acceptance exists");
    let active_install = completion
        .find("self.active_world.install(DrawableWorldSlotInstall {")
        .expect("active-slot installation remains explicit");
    let standby_install = completion
        .find("self.install_prepared_warm_world_slot(")
        .expect("standby-slot installation remains explicit");
    assert!(guard < active_install);
    assert!(guard < standby_install);

    assert!(SCENE_SESSION.contains("active_session_start_operations.complete("));
    assert!(SCENE_SESSION.contains("launch.complete_start("));
    assert!(!SCENE_SESSION.contains("external_scene_start_is_current"));
    assert!(!WEB_SCENE_HOST.contains("external_scene_start_is_current"));
    assert!(!WEB_SCENE_HOST.contains("stale_lobby_start_completion_count"));
    assert!(WEB_SCENE_HOST.contains("pub fn take_lobby_runtime_start"));
    assert!(!WEB_SCENE_HOST.contains("discardLobbyOperations"));
    assert!(!WEB_APP.contains("discardLobbyOperations"));
    assert!(WEB_APP.contains("pendingSceneOperations"));
    assert!(!WEB_APP.contains("pendingLobbyRuntimeStarts"));
    assert!(!WEB_APP.contains("lobbyLaunchObservedActive"));
}

#[test]
fn browser_compiler_broker_and_shared_renderer_boundary_are_singular() {
    assert!(!WEB_APP.contains("compiler: RenderCompiler | null;"));
    assert!(!WEB_APP.contains("pendingTimings: Map<string, PendingCompile>;"));
    assert!(!WEB_APP.contains("Map<WorldInstanceId"));
    assert!(!WEB_APP.contains("this.compiler?.releaseWorld("));
    assert!(WEB_APP.contains("new PolledWorkerTransport("));
    assert!(!WEB_RENDER_COMPILER_SHARED.contains("nextBrokerRequestId"));
    assert!(!WEB_RENDER_COMPILER_SHARED.contains("worldPriority === \"active\""));
    assert!(!WEB_RENDER_COMPILER_SHARED.contains("release-render-compiler-world"));
    assert!(WEB_RENDER_WORKER.contains("struct WebRenderWorkerCoordinator"));
    assert!(WEB_RENDER_WORKER.contains("RenderWorkerAssetSwapState"));
    assert!(WEB_RENDER_WORKER.contains("RenderWorkerPriority::Active"));
    assert!(WEB_RENDER_WORKER.contains("release-render-compiler-world"));
    assert!(!WEB_RENDER_COMPILER_WORKER.contains("compilerSessions = new Map"));
    assert!(!WEB_RENDER_COMPILER_WORKER.contains("forkWorldSession"));
    assert!(!WEB_RENDER_COMPILER_WORKER.contains("workKind"));
    assert!(WEB_RENDER_COMPILER_WORKER.contains("new module.WebRenderWorkerActor"));
    assert!(WEB_RENDER_WORKER_ACTOR.contains("compiler_sessions: BTreeMap"));
    assert!(WEB_RENDER_WORKER_ACTOR.contains("enum WorkKind"));
    assert!(WEB_RENDER_WORKER_ACTOR.contains("view.copy_from(packed)"));
    assert!(WEB_CANVAS.contains("WebRenderWorkerWorldHandle"));

    let shared_constructor = TERRAIN_RENDERER
        .split("impl TexturedSectionSharedResources {")
        .nth(1)
        .expect("shared terrain resource implementation exists")
        .split("pub struct TexturedSectionDrawResources")
        .next()
        .expect("shared constructor precedes mutable draw resources");
    assert!(shared_constructor.contains("TexturedChunkRenderer::new(device, color_format)"));
    assert!(shared_constructor.contains("GpuChunkTextureAtlas::new"));

    let draw_constructor = TERRAIN_RENDERER
        .split("impl TexturedSectionDrawResources {")
        .nth(1)
        .expect("textured terrain draw implementation exists")
        .split("pub fn create_placed_renderer")
        .next()
        .expect("constructor precedes the placed renderer");
    assert!(draw_constructor.contains("TexturedSectionSharedResources::new("));
    assert!(draw_constructor.contains("Self::new_with_shared_resources("));
    assert!(!draw_constructor.contains("GpuChunkTextureAtlas::new"));

    let prepare = SCENE_SESSION
        .split("pub fn prepare_warm_world_standby_shell(")
        .nth(1)
        .expect("standby shell preparation exists")
        .split("pub fn begin_warm_world_standby(")
        .next()
        .expect("shell preparation precedes launch helper");
    assert!(prepare.contains("TexturedSectionDrawResources::new_with_shared_resources("));
    assert!(prepare.contains("self.active_world.draw.shared_resources()"));
    assert!(!prepare.contains("TexturedSectionDrawResources::new("));
}

#[test]
fn typescript_does_not_own_lobby_policy_or_authored_content() {
    for forbidden in [
        "LobbyPreview",
        "lobby_preview",
        "authored_world_fixture_records",
        "WorldBehaviorProfile::Protected",
        "EmbeddedChunkRegion",
        "WorldPlacement",
    ] {
        assert!(
            !WEB_APP.contains(forbidden),
            "browser adapter must not own shared scenario term {forbidden}"
        );
        assert!(
            !WEB_WORLD_CATALOG.contains(forbidden),
            "IndexedDB adapter must not own shared scenario term {forbidden}"
        );
    }
    assert!(WEB_WORLD_CATALOG.contains("MANAGED_WORLD_METADATA_STORE"));
    assert!(!WEB_WORLD_CATALOG.contains("prepareManagedWorldPayload"));
    assert!(!WEB_WORLD_CATALOG.contains("validateManagedWorldPayload"));
    assert!(!WEB_WORLD_CATALOG.contains("ChunkRecord::"));
    assert!(!WEB_WORLD_CATALOG.contains("ProtectedLobby"));
    assert!(!WEB_WORLD_CATALOG.contains("provisionIndexedDbManagedScenarioWorld"));
    assert!(!WEB_APP.contains("scenarioId"));
    assert!(!WEB_APP.contains("contentVersion"));
    assert!(!WEB_APP.contains("fingerprint"));
}
