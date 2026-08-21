use mclone_worldgen::continental_hydrography_harness::{
    CONTINENTAL_HYDROGRAPHY_WITNESS_SHA256, run_continental_hydrography_suite,
};

#[test]
fn continental_hydrography_corpus_matches_native() {
    let receipt = run_continental_hydrography_suite().expect("continental hydrography suite");
    assert_eq!(
        receipt.witness_sha256,
        CONTINENTAL_HYDROGRAPHY_WITNESS_SHA256
    );
}
