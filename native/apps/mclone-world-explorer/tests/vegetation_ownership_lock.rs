//! Tactical 256 ownership and pre-cutover debt locks.
//!
//! These assertions track the staged platform cutovers. Native synchronous
//! compilation is gone; the browser-disabled configuration remains the one
//! named exception until Slice 4.

const SESSION: &str = include_str!("../src/session.rs");
const NATIVE_TERRAIN: &str = include_str!("../src/terrain.rs");
const WEB_HOST: &str = include_str!("../src/web.rs");
const WEB_APP: &str = include_str!("../www/world-explorer-app.js");
const WEB_WORKER: &str = include_str!("../www/world-explorer-worker.js");
const WEB_EXACT_WORKER: &str = include_str!("../www/world-explorer-exact-worker.js");
const SHARED_TRANSPORT: &str =
    include_str!("../../mclone-web-client/www/mclone-worker-transport.ts");
const TERRAIN_VIEW_RENDERER: &str =
    include_str!("../../../crates/mclone-terrain-view/src/viewport_renderer.rs");
const TERRAIN_VIEW_MANIFEST: &str = include_str!("../../../crates/mclone-terrain-view/Cargo.toml");
const WORLDGEN_MANIFEST: &str = include_str!("../../../crates/mclone-worldgen/Cargo.toml");

fn braced_item<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source.find(marker).expect("item marker present");
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

#[test]
fn explorer_session_does_not_own_vegetation_coordination_policy() {
    for forbidden in [
        "TerrainHorizonVegetationCoordinator",
        "McloneOverworldVegetationPlanCache",
        "VecDeque",
        "source_epoch",
        "executor_generation",
        "stale_completion",
        "restart_executor",
    ] {
        assert!(
            !SESSION.contains(forbidden),
            "World Explorer session regained vegetation policy through {forbidden:?}"
        );
    }
    for forbidden in [
        "vegetation",
        "tree",
        "TerrainViewportTileId",
        "landmarkRank",
        "sourceEpoch",
        "cacheHits",
        "pendingTiles",
    ] {
        assert!(
            !WEB_WORKER.contains(forbidden),
            "Explorer Worker shell gained domain policy through {forbidden:?}"
        );
    }
    assert!(WEB_APP.contains("new PolledWorkerTransport("));
    assert!(WEB_WORKER.contains("actor.handleMessage(message)"));
    assert!(WEB_WORKER.contains("self.postMessage(dispatch.message)"));
    assert!(WEB_EXACT_WORKER.contains("actor.handleMessage(message)"));
    for forbidden in [
        "chunk_x",
        "natural_trees",
        "working_bounds",
        "packed_sections",
        "ownership",
    ] {
        assert!(
            !WEB_EXACT_WORKER.contains(forbidden),
            "exact Worker shell gained Rust domain policy through {forbidden:?}"
        );
    }
}

#[test]
fn browser_javascript_and_shared_transport_are_domain_blind() {
    for forbidden in [
        "vegetation",
        "tree",
        "TerrainViewportTileId",
        "landmarkRank",
        "sourceEpoch",
        "cacheHits",
        "pendingTiles",
    ] {
        assert!(
            !WEB_APP.contains(forbidden),
            "ordinary Explorer JavaScript gained vegetation policy through {forbidden:?}"
        );
    }
    for forbidden in [
        "terrain",
        "vegetation",
        "tree",
        "profile",
        "epoch",
        "cache",
        "resident",
    ] {
        assert!(
            !SHARED_TRANSPORT.to_ascii_lowercase().contains(forbidden),
            "shared Worker transport contains domain token {forbidden:?}"
        );
    }
}

#[test]
fn compiler_dependency_direction_stays_worldgen_to_terrain_view_consumer() {
    assert!(TERRAIN_VIEW_MANIFEST.contains("mclone-worldgen.workspace = true"));
    for forbidden in [
        "mclone-terrain-view",
        "\nwgpu",
        "wasm-bindgen",
        "web-sys",
        "winit",
    ] {
        assert!(
            !WORLDGEN_MANIFEST.contains(forbidden),
            "worldgen compiler boundary gained forbidden dependency {forbidden:?}"
        );
    }
}

#[test]
fn native_and_browser_cutovers_remove_horizon_sync_compilation() {
    let horizon = braced_item(TERRAIN_VIEW_RENDERER, "impl TerrainHorizonRenderer {");
    assert_eq!(
        horizon
            .matches("TerrainPreviewVegetationProduct::compile_with_cache(")
            .count(),
        0,
        "the Horizon renderer regained synchronous vegetation compilation"
    );
    assert_eq!(
        NATIVE_TERRAIN.matches("vegetation_enabled: true").count(),
        1,
        "native must retain exactly one named enable site"
    );
    assert_eq!(
        WEB_HOST.matches("vegetation_enabled: true").count(),
        1,
        "browser must retain exactly one named enable site"
    );
    assert!(!NATIVE_TERRAIN.contains("vegetation_enabled: false"));
    assert!(!WEB_HOST.contains("vegetation_enabled: false"));
}
