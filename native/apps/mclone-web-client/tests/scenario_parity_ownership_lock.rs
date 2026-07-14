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
const WEB_WORLD_CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-web-world-catalog.ts"
));
const WEB_MANAGED_PROVISION_WORKER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-managed-scenario-provision-worker.ts"
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

use mclone_app_runtime::scenario_content::{
    ManagedScenarioManifest, ManagedScenarioWorldRole, managed_scenario_payload_fingerprint,
    managed_scenario_world_payload,
};

#[test]
fn web_adapter_consumes_the_shared_manifest_and_fixture_receipts() {
    let manifest = ManagedScenarioManifest::lobby_preview_v1();
    manifest.validate().unwrap();
    let primary =
        managed_scenario_world_payload(&manifest, ManagedScenarioWorldRole::Primary).unwrap();
    let destination =
        managed_scenario_world_payload(&manifest, ManagedScenarioWorldRole::Destination).unwrap();
    assert_eq!(
        managed_scenario_payload_fingerprint(&primary),
        8_001_097_006_086_081_343
    );
    assert_eq!(
        managed_scenario_payload_fingerprint(&destination),
        8_764_019_107_988_679_539
    );
}

#[test]
fn shared_scenario_start_boundary_has_no_native_policy_stub_or_path() {
    assert!(APP_RUNTIME_LIB.contains("pub mod scenario_content;"));
    assert!(!APP_RUNTIME_LIB.contains(
        "#[cfg(not(target_arch = \"wasm32\"))]\n\
         pub mod scenario_content;"
    ));
    assert!(CLIENT_EXPERIENCE.contains("web_client_experience_profile_with_managed_scenarios"));
    assert!(CLIENT_EXPERIENCE.contains("web_client_experience_profile_without_managed_scenarios"));
    assert!(CLIENT_EXPERIENCE.contains("ClientExperienceCapabilityStatus::Supported"));
    assert!(WEB_SCENE_HOST.contains("installManagedScenarioServices"));
    assert!(WEB_APP.contains("this.session.installManagedScenarioServices()"));
    assert!(SCENE_SESSION.contains("pub(crate) fn apply_managed_scenario_effect("));
    let effect = SCENE_SESSION
        .split("pub(crate) fn apply_managed_scenario_effect(")
        .nth(1)
        .expect("shared scenario effect exists")
        .split("fn begin_managed_scenario_launch(")
        .next()
        .unwrap();
    assert!(!effect.contains("Ok(false)"));
    assert!(!effect.contains("WEB_LOBBY_SCENARIO_INITIALIZING_REASON"));
    let request = WARM_WORLD
        .split("pub struct WarmWorldStandbyRequest {")
        .nth(1)
        .unwrap()
        .split("impl WarmWorldStandbyRequest")
        .next()
        .unwrap();
    assert!(!request.contains("PathBuf"));
    assert!(request.contains("pub managed_world_key: Option<ManagedWorldKey>"));
    assert!(WARM_WORLD.contains("PlatformOperationLedger<ProvisionManagedScenarioWorld"));
    assert!(!WARM_WORLD.contains("NativeManagedScenarioContentOperationService"));
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
fn primary_and_destination_provisioning_are_independent() {
    assert!(WARM_WORLD.contains("ManagedScenarioWorldRole::Primary,"));
    assert!(WARM_WORLD.contains("ManagedScenarioWorldRole::Destination,"));
    assert!(WARM_WORLD.contains("provision_operations.issue("));
    assert!(SCENE_SESSION.contains("take_managed_scenario_provision_request"));
    assert!(SCENE_SESSION.contains("complete_managed_scenario_provision"));
    assert!(SCENE_SESSION.contains("take_managed_scenario_world_start"));
    assert!(SCENE_SESSION.contains("complete_external_session_start"));
    assert!(!SCENE_SESSION.contains("CombinedScenarioContentCompletion"));
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
        .find("if !self.external_scene_start_is_current(&pending)")
        .expect("shared operation-epoch guard exists");
    let active_install = completion
        .find("self.active_world.install(DrawableWorldSlotInstall {")
        .expect("active-slot installation remains explicit");
    let standby_install = completion
        .find("self.install_prepared_warm_world_slot(")
        .expect("standby-slot installation remains explicit");
    assert!(guard < active_install);
    assert!(guard < standby_install);

    assert!(WEB_SCENE_HOST.contains("external_scene_start_is_current(&pending)"));
    assert!(WEB_SCENE_HOST.contains("stale_managed_start_completion_count"));
    assert!(WEB_SCENE_HOST.contains("discardManagedScenarioOperations"));
    assert!(WEB_APP.contains("this.session?.discardManagedScenarioOperations()"));
    assert!(WEB_APP.contains("pendingManagedRuntimeStarts"));
    assert!(WEB_APP.contains("else if (this.managedScenarioLaunchObservedActive)"));
}

#[test]
fn browser_compiler_broker_and_shared_renderer_boundary_are_singular() {
    assert!(WEB_APP.contains("compiler: RenderCompiler | null;"));
    assert!(WEB_APP.contains("pendingTimings: Map<string, PendingCompile>;"));
    assert!(!WEB_APP.contains("Map<WorldInstanceId"));
    assert!(WEB_APP.contains("this.compiler?.releaseWorld("));
    assert!(WEB_RENDER_COMPILER_SHARED.contains("nextBrokerRequestId"));
    assert!(WEB_RENDER_COMPILER_SHARED.contains("worldPriority === \"active\""));
    assert!(WEB_RENDER_COMPILER_SHARED.contains("release-render-compiler-world"));
    assert!(WEB_RENDER_COMPILER_WORKER.contains("compilerSessions = new Map"));
    assert!(WEB_RENDER_COMPILER_WORKER.contains("compilerTemplate.forkWorldSession()"));
    assert!(WEB_CANVAS.contains("worldInstanceId"));
    assert!(WEB_CANVAS.contains("worldPriority"));

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
        assert!(
            !WEB_MANAGED_PROVISION_WORKER.contains(forbidden),
            "provision Worker must not own shared scenario term {forbidden}"
        );
    }
    assert!(WEB_WORLD_CATALOG.contains("MANAGED_WORLD_METADATA_STORE"));
    assert!(WEB_WORLD_CATALOG.contains("mclone_web_managed_scenario_prepare_world("));
    assert!(WEB_WORLD_CATALOG.contains("mclone_web_managed_scenario_validate_world("));
    assert!(!WEB_WORLD_CATALOG.contains("ChunkRecord::"));
    assert!(!WEB_WORLD_CATALOG.contains("ProtectedLobby"));
    assert!(WEB_WORLD_CATALOG.contains("MANAGED_WORLD_METADATA_STORE"));
    assert!(WEB_WORLD_CATALOG.contains("provisionIndexedDbManagedScenarioWorldInWorker"));
    assert!(WEB_MANAGED_PROVISION_WORKER.contains("provisionIndexedDbManagedScenarioWorld("));
    assert!(!WEB_MANAGED_PROVISION_WORKER.contains("authored"));
}
