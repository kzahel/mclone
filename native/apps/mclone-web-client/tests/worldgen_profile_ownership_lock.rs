//! Tactical 187 lock: platform adapters transport worldgen jobs while shared
//! Rust owns profile dispatch and generation algorithms.

const JOB_WORKER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/www/mclone-server-job-worker.ts"
));
const WEB_SERVER_WORKER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/web_server_worker.rs"
));

#[test]
fn browser_job_worker_delegates_worldgen_frames_to_shared_rust() {
    assert!(JOB_WORKER.contains("new module.WebWorldgenJobSession()"));
    assert!(JOB_WORKER.contains("worldgenSession.computeWorldgenJobFrame(frame)"));
    assert!(WEB_SERVER_WORKER.contains("session: WorldgenJobSession"));
    assert!(WEB_SERVER_WORKER.contains(".compute_delta_job_frame(&frame.to_vec())"));

    for forbidden in [
        "NoiseBasedChunkGenerator",
        "generate_overworld",
        "generate_flat_grass",
        "generate_small_island",
        "block_state_ids",
        "setBlock",
    ] {
        assert!(
            !JOB_WORKER.contains(forbidden),
            "TypeScript job adapter must not implement generator behavior: {forbidden}"
        );
    }
}
