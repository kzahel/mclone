#![cfg(target_arch = "wasm32")]

use mclone_core::{BlockPos, HorizontalTopology};
use mclone_worldgen::homestead_site::{
    FlatGrassHomesteadSurveySource, HOMESTEAD_FLAT_WASM_WITNESS_SHA256, HomesteadScoutRequest,
    scout_homestead_site,
};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn homestead_selector_matches_the_native_flat_witness() {
    let receipt = scout_homestead_site(
        &mut FlatGrassHomesteadSurveySource,
        HomesteadScoutRequest {
            seed: 8_675_309,
            provisional_spawn: BlockPos::new(8, 4, 8),
            topology: HorizontalTopology::UNBOUNDED,
        },
    )
    .expect("flat homestead scout");

    assert_eq!(receipt.evaluation_count, 4_096);
    assert_eq!(
        receipt.selected.unwrap().checksum_sha256,
        HOMESTEAD_FLAT_WASM_WITNESS_SHA256
    );
}
