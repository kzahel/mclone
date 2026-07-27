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
const RUNTIME_CANVAS: &str =
    include_str!("../../../../tools/terrain-lab/src/web/RuntimeCompositionCanvas.tsx");
const RUNTIME_EXACT_WORKER: &str =
    include_str!("../../../../tools/terrain-lab/src/web/runtime-exact-worker.ts");
const RUNTIME_VEGETATION_WORKER: &str =
    include_str!("../../../../tools/terrain-lab/src/web/runtime-vegetation-worker.ts");
const RUNTIME_WEB: &str = include_str!("../src/runtime_web.rs");
const LANDFORM_PLAN_CANVAS: &str =
    include_str!("../../../../tools/terrain-lab/src/web/LandformPlanCanvas.tsx");
const LANDFORM_PLAN_WORKER: &str =
    include_str!("../../../../tools/terrain-lab/src/web/landform-plan-worker.ts");
const LANDFORM_PLAN_RUST: &str =
    include_str!("../../../crates/mclone-worldgen/src/landform_plan.rs");
const STREAMED_PLAN_ATLAS_CANVAS: &str =
    include_str!("../../../../tools/terrain-lab/src/web/StreamedPlanAtlasCanvas.tsx");
const STREAMED_PLAN_ATLAS_WORKER: &str =
    include_str!("../../../../tools/terrain-lab/src/web/streamed-plan-atlas-worker.ts");
const STREAMED_PLAN_ATLAS_RUST: &str =
    include_str!("../../../crates/mclone-worldgen/src/streamed_plan_atlas.rs");

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

#[test]
fn runtime_composition_consumes_shared_rust_owners() {
    for required in [
        "TerrainRuntimeSession",
        "TerrainRuntimeExactRenderer",
        "BrowserCanonicalExactExecutor",
        "BrowserTerrainVegetationExecutor",
        "TerrainHorizonRenderTarget",
        "encode_prepared_to_target",
        "TerrainExactCoverageMode::DiscardPainted",
        "TerrainRuntimeExactAnchor::Focus",
    ] {
        assert!(
            RUNTIME_WEB.contains(required),
            "runtime Terrain Lab host lost shared owner {required:?}"
        );
    }
    for forbidden in [
        "CanonicalMeshSession",
        "TerrainVegetationCompilerSession",
        "mclone_tree_ownership_snapshot",
        "ExactPaintedCoverageSnapshot::new",
    ] {
        assert!(
            !RUNTIME_WEB.contains(forbidden),
            "runtime Terrain Lab host regained engine policy through {forbidden:?}"
        );
    }
    assert_eq!(RUNTIME_CANVAS.matches("new Worker(").count(), 2);
    assert!(RUNTIME_EXACT_WORKER.contains("actor.handleMessage(frame)"));
    assert!(RUNTIME_VEGETATION_WORKER.contains("actor.handleMessage(frame)"));
    for worker in [RUNTIME_EXACT_WORKER, RUNTIME_VEGETATION_WORKER] {
        for forbidden in ["chunkX", "ownership", "sampleSpacing", "physicalSlot"] {
            assert!(
                !worker.contains(forbidden),
                "runtime Worker shell gained domain policy through {forbidden:?}"
            );
        }
    }
}

#[test]
fn landform_plan_semantics_stay_in_rust() {
    for required in [
        "CHANNEL_ACCUMULATION_THRESHOLD",
        "select_sinks",
        "build_drainage_forest",
        "classify_stream_order",
        "receiver_owners_diverge",
        "McloneLandformPlanSummary",
    ] {
        assert!(
            LANDFORM_PLAN_RUST.contains(required),
            "shared landform planner lost required semantic owner {required:?}"
        );
    }
    for forbidden in [
        "CHANNEL_ACCUMULATION_THRESHOLD",
        "MIN_SINK_SEPARATION",
        "GradientNoise2d",
        "sink_permission",
        "receiver_is_ancestor",
    ] {
        assert!(
            !LANDFORM_PLAN_CANVAS.contains(forbidden) && !LANDFORM_PLAN_WORKER.contains(forbidden),
            "Terrain Lab browser code gained planner policy through {forbidden:?}"
        );
    }
    assert!(LANDFORM_PLAN_WORKER.contains("LandformPlanCompiler"));
    assert!(LANDFORM_PLAN_CANVAS.contains("useWorldViewNavigation"));
    assert_eq!(LANDFORM_PLAN_CANVAS.matches("new Worker(").count(), 1);
}

#[test]
fn streamed_plan_atlas_semantics_and_cache_stay_in_rust() {
    for required in [
        "StreamedPlanAtlasCompiler",
        "CandidateCache",
        "lift_block",
        "atlas_snapshot_sha256",
        "CoordinatePureControl",
        "HierarchicalSharedFactsControl",
        "FeatureOwnedGraphControl",
        "MultiscaleSemanticRefinementControl",
    ] {
        assert!(
            STREAMED_PLAN_ATLAS_RUST.contains(required),
            "shared streamed-plan atlas lost required Rust owner {required:?}"
        );
    }
    for forbidden in [
        "FEATURE_GRAPH_MAXIMUM_REACH_BLOCKS",
        "possible_owner_regions",
        "shortest_block_displacement",
        "canonicalize_block",
        "typed_hash",
        "StreamedPlanControl",
    ] {
        assert!(
            !STREAMED_PLAN_ATLAS_CANVAS.contains(forbidden)
                && !STREAMED_PLAN_ATLAS_WORKER.contains(forbidden),
            "Terrain Lab browser code gained streamed-plan policy through {forbidden:?}"
        );
    }
    assert!(STREAMED_PLAN_ATLAS_WORKER.contains("StreamedPlanAtlasCompiler"));
    assert!(STREAMED_PLAN_ATLAS_CANVAS.contains("useWorldViewNavigation"));
    assert_eq!(STREAMED_PLAN_ATLAS_CANVAS.matches("new Worker(").count(), 1);
}
