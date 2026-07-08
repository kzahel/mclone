//! Tactical 155 P0 measurement: the light worker's `RetainedInitialLightState`
//! has no eviction path, so `total_light_status_inserted_chunks` — which counts
//! each chunk's first insertion into the retained light world and is never
//! decremented — is a live proxy for the retained-world entry count. As the
//! view center moves, the scheduler unloads chunks (`holder_count` stays bounded
//! by the ticket set), but the retained light world keeps every chunk ever lit.
//!
//! This test drives sustained one-axis movement and records the divergence. It
//! is the empirical companion to the P0 diagnosis in
//! `docs/tactical/155-pipeline-capacity-follow-up.md`. Run with
//! `--nocapture` to print the per-step trajectory.
//!
//! When the vanilla-shaped unload-eviction fix (Fix A) lands, the retained
//! proxy should track the bounded loaded set instead of growing, and the
//! `still_unbounded` assertion below will fail — that failure is the signal to
//! flip this into the fix's acceptance test (retained bounded ~= holders).

use mclone_core::ChunkPos;
use mclone_protocol::ChunkView;

use super::support::{apply_interest_and_poll, wait_for_scheduler_completion};
use crate::ChunkScheduler;

fn drain_to_quiescence(scheduler: &mut ChunkScheduler) {
    for _ in 0..120_000 {
        scheduler.poll().unwrap();
        wait_for_scheduler_completion(scheduler);
        if scheduler.pending_job_count() == 0 && scheduler.pending_publication_count() == 0 {
            return;
        }
    }
    panic!("timed out draining scheduler + light to quiescence");
}

#[test]
fn movement_soak_shows_unbounded_retained_light_memory() {
    const RADIUS: u32 = 2;
    const STEPS: i32 = 24;
    // Retained block copy (~64 KB) + sky/block DataLayers (~64 KB) per chunk.
    const APPROX_KB_PER_CHUNK: usize = 128;

    let mut scheduler = ChunkScheduler::new(12_345);
    scheduler.set_lighting_enabled(true);

    let mut holders = Vec::with_capacity(STEPS as usize);
    let mut retained = Vec::with_capacity(STEPS as usize);
    for index in 0..STEPS {
        let center = ChunkPos::new(index, 0);
        apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center,
                render_distance: RADIUS,
                chunk_tracking_radius: RADIUS,
            },
        );
        drain_to_quiescence(&mut scheduler);
        scheduler.process_pending_unloads(usize::MAX).unwrap();
        holders.push(scheduler.holder_count());
        retained.push(scheduler.metrics().total_light_status_inserted_chunks);
    }

    for index in 0..STEPS as usize {
        eprintln!(
            "step {:2} center_x={:2} holder_chunks(loaded)={:4} retained_light_chunks={:5} (~{} MB)",
            index,
            index,
            holders[index],
            retained[index],
            retained[index] * APPROX_KB_PER_CHUNK / 1024,
        );
    }

    let holder_peak = *holders.iter().max().expect("at least one step");
    let holder_min = *holders.iter().min().expect("at least one step");
    // Step 0 is one view's worth of lit chunks — what a store that evicted on
    // unload would hold roughly constant as the center moves.
    let retained_initial = retained[0];
    let retained_final = *retained.last().expect("at least one step");
    let per_step_growth = (retained_final - retained_initial) as f64 / (STEPS - 1).max(1) as f64;
    eprintln!(
        "SUMMARY holders(loaded)=[{holder_min}..{holder_peak}] flat_bounded={} \
         retained_light: initial={retained_initial} final={retained_final} \
         growth_per_step={per_step_growth:.1} ratio_vs_one_view={:.2}x approx_retained_mb={}",
        holder_min == holder_peak,
        retained_final as f64 / retained_initial.max(1) as f64,
        retained_final * APPROX_KB_PER_CHUNK / 1024,
    );

    // The loaded set is bounded by the ticket system and stays flat as the
    // center moves (holders constant above). The retained light world is not
    // bounded: it grows by ~one movement strip every step and never plateaus,
    // because unloaded chunks are never evicted from it. A correctly-evicting
    // store would hold ~`retained_initial` (one view's light set) as you move.
    //
    // When the vanilla-shaped unload-eviction fix (tactical 155 Fix A) lands,
    // retained should plateau near one view and this assertion will fail — flip
    // it then into the bounded acceptance form (e.g. retained_final close to
    // retained_initial).
    let still_unbounded = retained_final > retained_initial * 2;
    assert!(
        still_unbounded,
        "retained light world grew initial={retained_initial} -> final={retained_final} \
         (+{per_step_growth:.1}/step). If it now plateaus near one view, unload eviction \
         (tactical 155 Fix A) landed — flip this into the bounded acceptance assertion"
    );
}
