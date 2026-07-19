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
    assert!(JOB_WORKER.contains("new module.WebServerJobActor(message.actorInitFrame)"));
    assert!(JOB_WORKER.contains("serverJobActor.computeFrame(frame)"));
    assert!(WEB_SERVER_WORKER.contains("actor: ServerJobActor"));
    assert!(WEB_SERVER_WORKER.contains(".compute_frame(&frame.to_vec())"));

    for forbidden in [
        "new module.WebWorldgenJobSession()",
        "computeWorldgenJobFrame(frame)",
        "mclone_web_compute_light_status_job_frame(frame)",
        "case \"worldgen\"",
        "case \"light-status\"",
    ] {
        assert!(
            !JOB_WORKER.contains(forbidden),
            "TypeScript job broker must remain domain-blind: {forbidden}"
        );
    }

    for forbidden in [
        "NoiseBasedChunkGenerator",
        "generate_overworld",
        "generate_flat_grass",
        "generate_small_island",
        "generate_mclone_overworld",
        "block_state_ids",
        "setBlock",
    ] {
        assert!(
            !JOB_WORKER.contains(forbidden),
            "TypeScript job adapter must not implement generator behavior: {forbidden}"
        );
    }
}
