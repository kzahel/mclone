//! Tactical 202 baseline locks for the residual browser scene-operation seam.
//!
//! These assertions intentionally pin the pre-cutover shape. Later slices
//! invert them as the decisions move behind existing Rust owners.

const WEB_APP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-web-app.ts"
));
const INTEGRATED_SERVER_WORKER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-integrated-server-worker.ts"
));
const WEB_SCENE_HOST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/web_scene_host.rs"
));
const WEB_SCENE_PROTOCOL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/web_scene_protocol.rs"
));
const WORLD_CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-web-world-catalog.ts"
));

#[test]
fn baseline_pins_clear_typescript_session_and_lobby_dispatch() {
    assert!(WEB_APP.contains("operationKind === \"remote\""));
    assert!(WEB_APP.contains("operationKind === \"localWorld\""));
    assert!(WEB_APP.contains("session.startIndexedDbLocalWorld("));
    assert!(WEB_APP.contains("session.joinRemoteWebSocket("));
    assert!(WEB_APP.contains("operation.kind === \"start\""));
    assert!(WEB_APP.contains("session.prepareLobbyWorldStart("));

    assert!(WEB_SCENE_HOST.contains("pub fn take_lobby_operation"));
    assert!(WEB_SCENE_HOST.contains("pub fn prepare_lobby_world_start"));
}

#[test]
fn baseline_pins_duplicate_lifecycle_readiness_and_ready_envelope() {
    assert!(WEB_SCENE_PROTOCOL.contains("pub struct WebSceneSessionLifecycle"));
    assert!(WEB_SCENE_PROTOCOL.contains("ReconnectRemote"));
    assert!(WEB_APP.contains("const streamingSettled = Boolean(frame.streamingIdle)"));
    assert!(INTEGRATED_SERVER_WORKER.contains("ready.result.kind = \"ready\""));
    assert!(INTEGRATED_SERVER_WORKER.contains("ready.result.requestId ="));
}

#[test]
fn accepted_legacy_migration_remains_one_explicit_exception() {
    assert_eq!(
        WORLD_CATALOG
            .matches("migrateLegacyWorldRecordsToOverworld")
            .count(),
        2,
    );
    assert_eq!(WORLD_CATALOG.matches("minecraft:overworld").count(), 1);
}
