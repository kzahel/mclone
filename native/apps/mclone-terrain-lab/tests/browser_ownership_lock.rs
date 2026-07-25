use std::path::Path;

const CANONICAL_CANVAS: &str =
    include_str!("../../../../tools/terrain-lab/src/web/CanonicalTerrainCanvas.tsx");
const CANONICAL_WORKER: &str =
    include_str!("../../../../tools/terrain-lab/src/web/canonical-worker.ts");
const SHARED_TRANSPORT: &str =
    include_str!("../../mclone-web-client/www/mclone-worker-transport.ts");
const CANONICAL_WEB: &str = include_str!("../src/canonical_web.rs");
const CANONICAL_WORKER_RUST: &str = include_str!("../src/canonical_worker_web.rs");
const CANONICAL_COORDINATOR: &str = include_str!("../src/canonical_coordinator_web.rs");
const CANONICAL_MAILBOX: &str = include_str!("../src/canonical_mailbox_web.rs");
const TERRAIN_WEB: &str = include_str!("../src/web.rs");
const VITE_CONFIG: &str = include_str!("../../../../tools/terrain-lab/src/web/vite.config.ts");

#[test]
fn browser_typescript_has_no_exact_worker_policy() {
    for forbidden in [
        "worker.onmessage",
        "epochRef",
        "residentRef",
        "workerReadyRef",
        "CANONICAL_PENDING_HIGH_WATER",
        "CANONICAL_WORKER_MAX_BATCH",
        "canonicalTerrainWorkerBeginFrame",
        "canonicalTerrainWorkerCompileFrame",
        "CanonicalTerrainWorkerResponse",
        "packedSections",
    ] {
        assert!(
            !CANONICAL_CANVAS.contains(forbidden),
            "CanonicalTerrainCanvas.tsx regained exact-worker policy token {forbidden:?}"
        );
    }
    assert!(
        CANONICAL_CANVAS
            .contains("../../../../native/apps/mclone-web-client/www/mclone-worker-transport"),
        "Terrain Lab must consume the same opaque Worker transport source as the game"
    );
    assert_eq!(
        CANONICAL_CANVAS.matches("new Worker(").count(),
        1,
        "Terrain Lab should expose exactly one platform Worker-construction seam"
    );

    for forbidden in [
        "coordinatesJson",
        "cacheEnabled",
        "packedSections",
        "chunkX",
        "batchIndex",
        "transferables",
    ] {
        assert!(
            !CANONICAL_WORKER.contains(forbidden),
            "canonical-worker.ts regained terrain protocol token {forbidden:?}"
        );
    }
    assert!(CANONICAL_WORKER.contains("actor.handleMessage(frame)"));
    assert!(CANONICAL_WORKER.contains("workerSelf.postMessage(dispatch.message)"));

    let removed_protocol = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tools/terrain-lab/src/web/canonical-worker-protocol.ts");
    assert!(
        !removed_protocol.exists(),
        "the superseded TypeScript canonical Worker protocol returned"
    );
}

#[test]
fn exact_worker_ownership_and_shared_result_stay_in_rust() {
    for required in [
        "CanonicalTerrainWorkerCoordinator",
        "CANONICAL_WORKER_MAX_BATCH",
        "worker_in_flight",
        "next_missing_index",
        "stale_chunks",
        "admit_one",
    ] {
        assert!(
            CANONICAL_COORDINATOR.contains(required),
            "Rust exact coordinator lost required ownership token {required:?}"
        );
    }
    for required in [
        "SharedArrayBuffer",
        "CANONICAL_SHARED_RESULT_PENDING",
        "CANONICAL_SHARED_RESULT_COMPLETE",
        "CANONICAL_SHARED_RESULT_OVERFLOW",
        "CANONICAL_SHARED_RESULT_FAILED",
        "CANONICAL_SHARED_RESULT_MAX_CAPACITY",
        "crossOriginIsolated",
    ] {
        assert!(
            CANONICAL_MAILBOX.contains(required),
            "Rust shared-result mailbox lost required contract token {required:?}"
        );
    }
    assert!(CANONICAL_WORKER_RUST.contains("publish_canonical_shared_result"));
    assert!(!CANONICAL_WORKER_RUST.contains("admissionPackedSections"));
    assert!(!CANONICAL_WORKER_RUST.contains("CanonicalTerrainWorkerResponse)]"));
}

#[test]
fn legacy_raw_admission_and_domain_aware_transport_stay_deleted() {
    for forbidden in [
        "js_name = acceptChunk",
        "js_name = retainChunks",
        "js_name = setPresentation",
        "ResidentCanonicalChunk",
        "resident_raw_bytes(",
        "rebuild_chunks(",
        "canonical_terrain_presentation_blocks",
        "CanonicalTerrainCompiler",
        "CanonicalTerrainChunkPayload",
        "canonicalTerrainChunkOrder",
    ] {
        assert!(
            !CANONICAL_WEB.contains(forbidden) && !TERRAIN_WEB.contains(forbidden),
            "legacy main-thread exact path token {forbidden:?} returned"
        );
    }

    for forbidden in [
        "terrain", "chunk", "mesh", "profile", "epoch", "cache", "resident",
    ] {
        assert!(
            !SHARED_TRANSPORT.to_ascii_lowercase().contains(forbidden),
            "shared Worker transport contains domain token {forbidden:?}"
        );
    }
}

#[test]
fn local_terrain_lab_is_cross_origin_isolated() {
    for header in [
        "Cross-Origin-Opener-Policy",
        "Cross-Origin-Embedder-Policy",
        "Cross-Origin-Resource-Policy",
    ] {
        assert!(
            VITE_CONFIG.contains(header),
            "Terrain Lab Vite config is missing {header}"
        );
    }
    assert!(VITE_CONFIG.contains("server:"));
    assert!(VITE_CONFIG.contains("preview:"));
}
