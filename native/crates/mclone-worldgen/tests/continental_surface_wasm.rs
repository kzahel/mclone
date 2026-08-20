#![cfg(target_arch = "wasm32")]

use mclone_worldgen::continental_surface_harness::{
    CONTINENTAL_SURFACE_WITNESS_SHA256, run_continental_surface_suite,
};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn continental_surface_corpus_matches_native() {
    let receipt = run_continental_surface_suite().expect("continental surface suite");
    assert!(receipt.suite_passed);
    assert_eq!(receipt.witness_sha256, CONTINENTAL_SURFACE_WITNESS_SHA256);
}
