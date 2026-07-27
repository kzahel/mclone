#![cfg(target_arch = "wasm32")]

use mclone_worldgen::multiscale_terrain_witness::{
    MULTISCALE_WITNESS_SHA256, run_multiscale_witness_suite,
};
use mclone_worldgen::streamed_plan_atlas::{
    STREAMED_PLAN_ATLAS_FALLBACK_WITNESS_SHA256, STREAMED_PLAN_ATLAS_GRAPH_WITNESS_SHA256,
    STREAMED_PLAN_ATLAS_HIERARCHY_WITNESS_SHA256, STREAMED_PLAN_ATLAS_MULTISCALE_WITNESS_SHA256,
    StreamedPlanAtlasCompiler,
};
use mclone_worldgen::streamed_plan_harness::{
    STREAMED_PLAN_PHASE_ONE_COMPARISON_SHA256, STREAMED_PLAN_PHASE_ONE_FALLBACK_SHA256,
    StreamedPlanTopology, coordinate_pure_corpus_sha256, run_streamed_plan_phase_one_suite,
};
use mclone_worldgen::streamed_plan_trials::{
    STREAMED_PLAN_PHASE_TWO_WITNESS_SHA256, run_streamed_plan_phase_two_suite,
};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn coordinate_pure_phase_one_corpus_matches_native() {
    assert_eq!(
        coordinate_pure_corpus_sha256().expect("coordinate-pure corpus"),
        STREAMED_PLAN_PHASE_ONE_FALLBACK_SHA256
    );
}

#[wasm_bindgen_test]
fn full_phase_one_comparison_corpus_matches_native() {
    let receipt = run_streamed_plan_phase_one_suite().expect("Phase 1 suite");
    assert!(receipt.suite_passed);
    assert_eq!(
        receipt.comparison_corpus_sha256,
        STREAMED_PLAN_PHASE_ONE_COMPARISON_SHA256
    );
}

#[wasm_bindgen_test]
fn full_phase_two_candidate_corpus_matches_native() {
    let receipt = run_streamed_plan_phase_two_suite().expect("Phase 2 suite");
    assert!(receipt.suite_passed);
    assert_eq!(
        receipt.phase_two_witness_sha256,
        STREAMED_PLAN_PHASE_TWO_WITNESS_SHA256
    );
}

#[wasm_bindgen_test]
fn streamed_plan_atlas_matches_native_witness() {
    let summary = StreamedPlanAtlasCompiler::new(12_345, StreamedPlanTopology::Plane)
        .query(0, 0, 1_024, 1.0)
        .expect("streamed-plan atlas");
    assert_eq!(
        summary.fallback.receipt.semantic_sha256,
        STREAMED_PLAN_ATLAS_FALLBACK_WITNESS_SHA256
    );
    assert_eq!(
        summary.hierarchy.receipt.semantic_sha256,
        STREAMED_PLAN_ATLAS_HIERARCHY_WITNESS_SHA256
    );
    assert_eq!(
        summary.feature_graph.receipt.semantic_sha256,
        STREAMED_PLAN_ATLAS_GRAPH_WITNESS_SHA256
    );
    assert_eq!(
        summary.multiscale_witness.receipt.semantic_sha256,
        STREAMED_PLAN_ATLAS_MULTISCALE_WITNESS_SHA256
    );
}

#[wasm_bindgen_test]
fn multiscale_semantic_refinement_matches_its_exact_corpus() {
    let receipt = run_multiscale_witness_suite().expect("multiscale witness suite");
    assert!(receipt.suite_passed);
    assert_eq!(receipt.witness_sha256, MULTISCALE_WITNESS_SHA256);
    assert_eq!(receipt.corpora.len(), 9);
    assert!(
        receipt
            .corpora
            .iter()
            .all(|corpus| corpus.direct_parent_projection_mismatch_count == 0)
    );
}
