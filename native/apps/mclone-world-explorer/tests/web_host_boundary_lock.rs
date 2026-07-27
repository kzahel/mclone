const WEB_APP: &str = include_str!("../www/world-explorer-app.js");
const WEB_SMOKE_OBSERVER: &str = include_str!("../www/world-explorer-smoke-observer.js");
const WEB_RUST_HOST: &str = include_str!("../src/web.rs");
const WEB_HTML: &str = include_str!("../www/index.html");
const WEB_CSS: &str = include_str!("../www/world-explorer.css");
const EXPLORER_MANIFEST: &str = include_str!("../Cargo.toml");

#[test]
fn ordinary_browser_host_contains_no_explorer_diagnostic_projection() {
    for forbidden in [
        "JSON.parse",
        ".report",
        "dataset.ready",
        "allocationSlots",
        "readySlots",
        "pendingRefills",
        "totalRefills",
        "totalRebases",
        "centerX",
        "centerZ",
        "focusX",
        "focusZ",
        "drawnLevels",
        "drawnTiles",
        "treeInstanceCount",
        "fixedResidentBytes",
        "pendingVegetationTiles",
        "__MCLONE_WORLD_EXPLORER_SMOKE__",
    ] {
        assert!(
            !WEB_APP.contains(forbidden),
            "ordinary World Explorer browser host regained {forbidden}"
        );
    }

    assert!(WEB_APP.contains("runtime.session.renderFrame(frameMillis);"));
    assert!(WEB_APP.contains("status.hidden = true;"));
    assert!(WEB_APP.contains("parameters.get(\"smokeObserver\")"));
    assert!(WEB_APP.contains("import(\"./world-explorer-smoke-observer.js\")"));
}

#[test]
fn smoke_observer_is_query_gated_and_reads_rust_authored_snapshots() {
    assert!(WEB_SMOKE_OBSERVER.contains("session.diagnosticSnapshot()"));
    assert!(WEB_SMOKE_OBSERVER.contains("JSON.parse"));
    assert!(WEB_SMOKE_OBSERVER.contains("__MCLONE_WORLD_EXPLORER_SMOKE__"));
    assert!(WEB_SMOKE_OBSERVER.contains("session.recenterForSmoke("));
    assert!(WEB_RUST_HOST.contains("js_name = diagnosticSnapshot"));
    assert!(WEB_RUST_HOST.contains("js_name = recenterForSmoke"));
    assert!(
        WEB_RUST_HOST
            .contains("pub fn render_frame(&mut self, frame_millis: f64) -> Result<(), JsValue>")
    );
}

#[test]
fn successful_browser_surface_hides_the_generic_startup_fallback() {
    assert!(WEB_HTML.contains("Starting World Explorer…"));
    assert!(WEB_HTML.contains("id=\"world-explorer-status\""));
    assert!(WEB_CSS.contains("#world-explorer-status[hidden]"));
    assert!(!WEB_CSS.contains("data-ready"));
    assert!(WEB_APP.contains("void boot().catch(showFatalError);"));
    assert!(WEB_APP.contains("status.hidden = false;"));
    assert!(WEB_APP.contains("shell.dataset.failed = \"true\";"));
}

#[test]
fn explorer_shared_render_contract_stays_behind_the_lightweight_firewall() {
    assert!(EXPLORER_MANIFEST.contains("mclone-render-color.workspace = true"));
    assert!(EXPLORER_MANIFEST.contains("\nmclone-render.workspace = true"));
    assert!(!EXPLORER_MANIFEST.contains("\nmclone-scene.workspace = true"));
    assert!(!EXPLORER_MANIFEST.contains("\nmclone-ui.workspace = true"));
}
