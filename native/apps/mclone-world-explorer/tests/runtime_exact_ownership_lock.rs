//! Tactical 266 shared runtime exact ownership lock.

const EXPLORER_LIB: &str = include_str!("../src/lib.rs");
const EXPLORER_WEB_EXACT: &str = include_str!("../src/web_exact.rs");
const SHARED_EXACT: &str = include_str!("../../../crates/mclone-terrain-view/src/runtime_exact.rs");
const SHARED_BROWSER_EXACT: &str =
    include_str!("../../../crates/mclone-terrain-view/src/browser_exact.rs");
const SHARED_SESSION: &str =
    include_str!("../../../crates/mclone-terrain-view/src/runtime_session.rs");
const SHARED_LIB: &str = include_str!("../../../crates/mclone-terrain-view/src/lib.rs");

#[test]
fn runtime_exact_renderer_is_shared_and_explorer_only_adapts_transport() {
    assert!(!EXPLORER_LIB.contains("mod exact;"));
    assert!(EXPLORER_LIB.contains("TerrainRuntimeExactRenderer as ExplorerExactTerrain"));
    assert!(SHARED_LIB.contains("mod runtime_exact;"));
    assert!(SHARED_EXACT.contains("pub struct TerrainRuntimeExactRenderer"));
    assert!(SHARED_EXACT.contains("pub trait CanonicalExactExecutor"));
    assert!(SHARED_BROWSER_EXACT.contains("pub struct BrowserCanonicalExactExecutor"));
    assert!(SHARED_SESSION.contains("pub struct TerrainRuntimeSession"));
    assert!(!EXPLORER_LIB.contains("mod session;"));

    for forbidden in [
        "ChunkDepthTarget",
        "TexturedSectionDrawResources",
        "mclone_tree_ownership_snapshot",
        "ExactPaintedCoverageSnapshot::new",
    ] {
        assert!(
            !EXPLORER_WEB_EXACT.contains(forbidden),
            "Explorer browser adapter regained shared exact policy through {forbidden:?}"
        );
    }
}
