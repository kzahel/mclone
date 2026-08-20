#![cfg(target_arch = "wasm32")]

use mclone_worldgen::continental_ecoregion_harness::{
    CONTINENTAL_ECOREGION_WITNESS_SHA256, run_continental_ecoregion_suite,
};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn continental_ecoregion_corpus_matches_native() {
    let receipt = run_continental_ecoregion_suite().expect("continental/ecoregion suite");
    assert!(receipt.suite_passed);
    assert_eq!(receipt.witness_sha256, CONTINENTAL_ECOREGION_WITNESS_SHA256);
}
