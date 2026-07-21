//! Non-increasing source inventory for Tactical 207.
//!
//! These ceilings describe the pre-cutover boundary. Implementation slices
//! lower them as owners disappear; they must never be raised to accommodate a
//! replacement path. The zero target is enforced by the tactical closeout,
//! while semantic tests and traces remain the authority for behavior.

const WEB_APP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-web-app.ts"
));
const WEB_SCENE_HOST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/web_scene_host.rs"
));
const SCENE_SESSION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/mclone-scene/src/session.rs"
));
const SCENE_ASSET_REPLACEMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/mclone-scene/src/asset_replacement.rs"
));

fn assert_at_most(label: &str, source: &str, needle: &str, ceiling: usize) {
    let count = source.matches(needle).count();
    assert!(
        count <= ceiling,
        "{label} debt grew from ceiling {ceiling} to {count}; delete or reuse the existing owner instead of raising the ceiling"
    );
}

#[test]
fn typescript_scene_operation_coordination_debt_only_decreases() {
    for (label, needle, ceiling) in [
        ("global scene borrow guard", "sessionBusy", 0),
        ("scene borrow spin helper", "waitForSessionIdle", 0),
        ("lobby promise registry", "pendingLobbyRuntimeStarts", 0),
        ("lobby drain guard", "lobbyOperationDrainActive", 5),
        ("catalog promise tail", "worldCatalogOperationTail", 5),
        (
            "session dispatch branch",
            "dispatchSceneSessionOperation",
            3,
        ),
        (
            "catalog dispatch branch",
            "dispatchWorldCatalogOperation",
            2,
        ),
        ("asset dispatch branch", "dispatchAssetPackOperation", 3),
        ("catalog string identity", "catalogRequestId", 0),
        ("session report wakeup", "sessionStartPending", 1),
        ("asset report wakeup", "assetPackRequest", 1),
        (
            "render-queue readiness reconstruction",
            "renderWorkerPendingRequestCount",
            1,
        ),
    ] {
        assert_at_most(label, WEB_APP, needle, ceiling);
    }
}

#[test]
fn rust_boundary_identity_and_async_export_debt_only_decreases() {
    for (label, source, needle, ceiling) in [
        (
            "active-session async mutable borrow",
            WEB_SCENE_HOST,
            "pub async fn start_pending_session(",
            0,
        ),
        (
            "shutdown async mutable borrow",
            WEB_SCENE_HOST,
            "pub async fn shutdown_async(",
            0,
        ),
        (
            "asset async mutable borrow",
            WEB_SCENE_HOST,
            "pub async fn complete_asset_pack_selection(",
            0,
        ),
        (
            "lobby-ticket async mutable borrow",
            WEB_SCENE_HOST,
            "pub async fn start(&mut self)",
            0,
        ),
        (
            "browser-local stale completion counter",
            WEB_SCENE_HOST,
            "stale_lobby_start_completion_count",
            0,
        ),
        (
            "parallel session currentness check",
            SCENE_SESSION,
            "external_scene_start_is_current",
            0,
        ),
        (
            "catalog string completion parameter",
            WEB_SCENE_HOST,
            "request_id: String",
            0,
        ),
        (
            "asset generation used as browser completion identity",
            WEB_SCENE_HOST,
            "asset_pack_preparation_in_flight: Option<u64>",
            0,
        ),
        (
            "asset epoch used as external operation identity",
            SCENE_ASSET_REPLACEMENT,
            "ExternalAssetPackSelection {\n    pub epoch",
            0,
        ),
    ] {
        assert_at_most(label, source, needle, ceiling);
    }
}
