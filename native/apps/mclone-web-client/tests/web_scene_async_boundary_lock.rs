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
fn active_session_dispatch_is_rust_owned() {
    assert!(!WEB_APP.contains("operationKind === \"remote\""));
    assert!(!WEB_APP.contains("operationKind === \"localWorld\""));
    assert!(!WEB_APP.contains("session.startIndexedDbLocalWorld("));
    assert!(!WEB_APP.contains("session.joinRemoteWebSocket("));
    assert!(WEB_APP.contains("session.startPendingSession("));
    assert!(WEB_SCENE_HOST.contains("pub async fn start_pending_session"));
    assert!(!WEB_SCENE_HOST.contains("fn write_external_session_start"));
}

#[test]
fn lobby_runtime_start_is_an_opaque_rust_ticket() {
    assert!(!WEB_APP.contains("operation.kind === \"start\""));
    assert!(!WEB_APP.contains("session.prepareLobbyWorldStart("));
    assert!(!WEB_APP.contains("operation.requestId"));
    assert!(WEB_APP.contains("session.takeLobbyRuntimeStart("));

    assert!(WEB_SCENE_HOST.contains("pub fn take_lobby_runtime_start"));
    assert!(!WEB_SCENE_HOST.contains("pub fn take_lobby_operation"));
    assert!(!WEB_SCENE_HOST.contains("pub fn prepare_lobby_world_start"));
    assert!(!WEB_SCENE_HOST.contains("lobby_world_starts:"));
}

#[test]
fn browser_session_lifecycle_is_not_mirrored() {
    assert!(!WEB_SCENE_PROTOCOL.contains("pub struct WebSceneSessionLifecycle"));
    assert!(!WEB_SCENE_PROTOCOL.contains("ReconnectRemote"));
}

#[test]
fn readiness_and_ready_envelopes_are_rust_authored() {
    assert!(WEB_APP.contains("const streamingSettled = Boolean(frame.streamingIdle)"));
    assert!(WEB_APP.contains("return Boolean(frame.initialPresentationReady)"));
    assert!(!WEB_APP.contains("const runnerSettled ="));
    assert!(!WEB_APP.contains("let stableFrames ="));
    assert!(WEB_SCENE_HOST.contains("fn observe_initial_presentation_frame"));
    assert!(WEB_SCENE_HOST.contains("self.render_worker.pending_request_count() == 0"));

    assert!(!INTEGRATED_SERVER_WORKER.contains("ready.result.kind = \"ready\""));
    assert!(!INTEGRATED_SERVER_WORKER.contains("ready.result.requestId ="));
    assert!(!INTEGRATED_SERVER_WORKER.contains("ready.result.updates ="));
    assert!(
        INTEGRATED_SERVER_WORKER
            .contains("workerSelf.postMessage(server.readyReport(Number(message.requestId) || 0))")
    );
    assert!(INTEGRATED_SERVER_WORKER.contains("MAX_BROWSER_PERSISTENCE_CONTINUATIONS"));
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
