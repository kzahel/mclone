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
const BROWSER_SMOKE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/scripts/browser-smoke.mjs"
));
const WEB_SCENE_HOST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/web_scene_host.rs"
));
const WEB_CATALOG_EXECUTION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/web_catalog_execution.rs"
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

fn braced_item<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source.find(marker).expect("wasm item marker present");
    let body = &source[start..];
    let mut depth = 0usize;
    let mut opened = false;
    for (index, byte) in body.bytes().enumerate() {
        match byte {
            b'{' => {
                opened = true;
                depth += 1;
            }
            b'}' if opened => {
                depth -= 1;
                if depth == 0 {
                    return &body[..=index];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated braced item for {marker}")
}

fn wasm_export_count(source: &str, marker: &str) -> usize {
    let item = braced_item(source, marker);
    item.matches("#[wasm_bindgen(js_name").count()
        + item.matches("#[wasm_bindgen(constructor)]").count()
}

#[test]
fn typescript_scene_operation_coordination_debt_only_decreases() {
    for (label, needle, ceiling) in [
        ("global scene borrow guard", "sessionBusy", 0),
        ("scene borrow spin helper", "waitForSessionIdle", 0),
        ("lobby promise registry", "pendingLobbyRuntimeStarts", 0),
        ("lobby drain guard", "lobbyOperationDrainActive", 0),
        ("catalog promise tail", "worldCatalogOperationTail", 0),
        (
            "session dispatch branch",
            "dispatchSceneSessionOperation",
            0,
        ),
        (
            "catalog dispatch branch",
            "dispatchWorldCatalogOperation",
            0,
        ),
        ("asset dispatch branch", "dispatchAssetPackOperation", 0),
        ("catalog string identity", "catalogRequestId", 0),
        ("session report wakeup", "sessionStartPending", 0),
        ("catalog report wakeup", "catalogRequest", 0),
        ("asset report wakeup", "assetPackRequest", 0),
        (
            "render-queue readiness reconstruction",
            "renderWorkerPendingRequestCount",
            0,
        ),
        ("opaque operation drain", "drainSceneOperations", 5),
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
            "browser-local asset operation identity guard",
            WEB_SCENE_HOST,
            "asset_pack_preparation_in_flight",
            0,
        ),
        (
            "browser-local catalog operation guard",
            WEB_SCENE_HOST,
            "catalog_operation_in_flight",
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

#[test]
fn browser_smoke_uses_current_operation_outcomes() {
    assert_at_most(
        "deleted stale-completion diagnostic",
        BROWSER_SMOKE,
        "staleLobbyStartCompletionCount",
        0,
    );
}

#[test]
fn test_only_coarse_operation_is_a_boundary_fixpoint() {
    assert!(WEB_SCENE_HOST.contains("TestOnlyRemote(String)"));
    assert!(!WEB_APP.contains("TestOnlyRemote"));

    // These exact pins make sibling ABI growth a deliberate review event.
    // Smoke-only hooks live on explicit smoke types, leaving the product host
    // with 37 mechanical exports.
    assert_eq!(
        wasm_export_count(WEB_SCENE_HOST, "#[wasm_bindgen]\nimpl WebSceneHost {"),
        37
    );
    assert_eq!(
        wasm_export_count(WEB_SCENE_HOST, "#[wasm_bindgen]\nimpl WebSceneOperation {"),
        3
    );
    assert_eq!(
        wasm_export_count(
            WEB_CATALOG_EXECUTION,
            "#[wasm_bindgen]\n    impl WebCatalogExecution {"
        ),
        6
    );
    assert_eq!(
        wasm_export_count(
            WEB_SCENE_HOST,
            "#[wasm_bindgen]\nimpl WebSceneSmokeHarness {"
        ),
        6
    );
    assert_eq!(
        wasm_export_count(
            WEB_CATALOG_EXECUTION,
            "#[wasm_bindgen]\n    impl WebCatalogSmokeExecution {"
        ),
        8
    );
}
