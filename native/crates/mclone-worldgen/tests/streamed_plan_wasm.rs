#![cfg(target_arch = "wasm32")]

use mclone_worldgen::streamed_plan_harness::{
    STREAMED_PLAN_PHASE_ONE_COMPARISON_SHA256, STREAMED_PLAN_PHASE_ONE_FALLBACK_SHA256,
    coordinate_pure_corpus_sha256, run_streamed_plan_phase_one_suite,
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
