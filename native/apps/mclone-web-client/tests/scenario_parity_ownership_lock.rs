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
fn pre_refactor_browser_scenario_debt_is_explicit_and_bounded() {
    assert!(APP_RUNTIME_LIB.contains("pub mod scenario_content;"));
    assert!(!APP_RUNTIME_LIB.contains(
        "#[cfg(not(target_arch = \"wasm32\"))]\n\
         pub mod scenario_content;"
    ));
    assert!(
        CLIENT_EXPERIENCE
            .contains("ClientExperienceCapabilityStatus::Unsupported(WEB_LOBBY_SCENARIO_REASON)")
    );
    assert!(SCENE_SESSION.contains("#[cfg(target_arch = \"wasm32\")]"));
    assert!(SCENE_SESSION.contains("pub(crate) fn apply_managed_scenario_effect("));
    assert!(SCENE_SESSION.contains("Ok(false)"));
    assert!(WARM_WORLD.contains("pub world_dir: Option<PathBuf>"));
    assert!(WARM_WORLD.contains("NativeManagedScenarioContentOperationService"));
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
    assert!(SCENE_SESSION.contains("let mut primary_operations ="));
    assert!(SCENE_SESSION.contains("let mut destination_operations ="));
    assert!(SCENE_SESSION.contains("let primary_token = primary_operations.submit(intent);"));
    assert!(
        SCENE_SESSION.contains("let destination_token = destination_operations.submit(intent);")
    );
    assert!(!SCENE_SESSION.contains("CombinedScenarioContentCompletion"));
}

#[test]
fn current_browser_compiler_and_second_renderer_shell_debt_is_named() {
    assert!(WEB_APP.contains("compiler: RenderCompiler | null;"));
    assert!(WEB_APP.contains("pendingTimings: Map<number, PendingCompile>;"));
    assert!(!WEB_APP.contains("Map<WorldInstanceId"));

    let constructor = TERRAIN_RENDERER
        .split("impl TexturedSectionDrawResources {")
        .nth(1)
        .expect("textured terrain draw implementation exists")
        .split("pub fn create_placed_renderer")
        .next()
        .expect("constructor precedes the placed renderer");
    assert!(constructor.contains("TexturedChunkRenderer::new(device, color_format)"));
    assert!(constructor.contains("GpuChunkTextureAtlas::new"));
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
    }
}
