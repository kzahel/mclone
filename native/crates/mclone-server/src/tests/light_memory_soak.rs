//! Tactical 155 P0 and Tactical 312 acceptance gate: the light worker's
//! `RetainedInitialLightState` is evicted on unload (Fix A), so its live entry count
//! (`retained_light_world_chunks`) tracks the bounded loaded set instead of
//! growing without bound. Restartable Feature-to-Light metadata is likewise
//! bounded by resident holders and released on successful publication or
//! authoritative unload. These tests drive settled and non-quiescing movement
//! and assert both stores remain bounded.
//!
//! It is the before/after guard for the P0 diagnosis in
//! `docs/tactical/155-pipeline-capacity-follow-up.md`: before the fix this
//! asserted the leak (retained grew ~+9 chunks/step, no plateau); the fix makes
//! retained bounded, and this now fails if eviction regresses (retained resumes
//! growing) — that inverse-failure is the acceptance proof. Run with
//! `--nocapture` to print the per-step trajectory.

use std::time::Duration;

use mclone_core::ChunkPos;
use mclone_protocol::ChunkView;

use super::support::{apply_interest_and_poll, wait_for_scheduler_completion};
use crate::{ChunkScheduler, MAX_RECENT_COMPLETED_JOB_SUMMARIES, WorldGenerationProfile};

const RADIUS: u32 = 2;
const STEPS: i32 = 24;
// Retained block copy (~64 KB) + sky/block DataLayers (~64 KB) per chunk.
const APPROX_KB_PER_CHUNK: usize = 128;

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

/// Step the view center in +X and, at each step, record the scheduler's loaded
/// set (`holder_count`) against the live retained-light-world entry count. The
/// scheduler unloads chunks that leave the ticket set and enqueues their light
/// eviction, so the retained count is bounded by the loaded set rather than the
/// distance travelled. Returns `(holders, retained)` per step.
fn run_movement_soak(seed: i64) -> (Vec<usize>, Vec<usize>) {
    let mut scheduler = ChunkScheduler::new(seed);
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
        // Unloads are fire-and-forget to the light worker (no completion to wait
        // on), so flush the worker's request queue before reading the retained
        // gauge, otherwise the last step's evictions would not yet be observed.
        assert!(
            scheduler.wait_for_light_idle(Duration::from_secs(30)),
            "light worker did not acknowledge idle sync"
        );
        let scheduler_metrics = scheduler.metrics();
        let mailbox_metrics = scheduler.light_status_mailbox_metrics();
        assert_eq!(scheduler.full_job_record_count(), 0);
        assert!(
            scheduler.recent_completed_job_summary_count() <= MAX_RECENT_COMPLETED_JOB_SUMMARIES
        );
        assert!(scheduler_metrics.player_promotion_max_active <= 4);
        assert_eq!(scheduler_metrics.light_ticket_count, 0);
        assert_eq!(scheduler_metrics.light_ticket_conservation_failures, 0);
        assert_eq!(scheduler_metrics.light_restartable_contexts, 0);
        assert_eq!(scheduler_metrics.light_restartable_context_bytes, 0);
        assert_eq!(mailbox_metrics.admitted_statuses, 0);
        assert_eq!(mailbox_metrics.admitted_owned_bytes, 0);
        assert!(mailbox_metrics.max_pending_statuses <= 18);
        assert!(mailbox_metrics.max_admitted_owned_bytes <= 64 * 1024 * 1024);
        holders.push(scheduler.holder_count());
        retained.push(scheduler_metrics.retained_light_world_chunks);
    }
    (holders, retained)
}

#[test]
fn delayed_admission_churn_bounds_restart_context_memory() {
    const CHURN_STEPS: i32 = 24;
    const CHURN_RENDER_DISTANCE: u32 = 1;
    const CHURN_TRACKING_RADIUS: u32 = 2;
    const CHUNK_JUMP: i32 = 16;
    const LIGHT_ADMISSION_DELAY_TICKS: u32 = 80;
    const DELAYED_DEMAND_TARGET: usize = 4;
    const MAX_CONTEXT_BYTES_PER_HOLDER: usize = 64;

    let mut scheduler = ChunkScheduler::new(12_345);
    scheduler
        .set_world_generation_profile(WorldGenerationProfile::FlatGrassV1)
        .unwrap();
    scheduler.set_lighting_enabled(true);
    scheduler.set_debug_light_admission_delay_ticks(LIGHT_ADMISSION_DELAY_TICKS);

    let mut post_unload_holder_counts = Vec::with_capacity(CHURN_STEPS as usize);
    let mut context_peak = 0_usize;
    let mut context_byte_peak = 0_usize;
    for step in 0..CHURN_STEPS {
        let center = ChunkPos::new(step * CHUNK_JUMP, 0);
        scheduler
            .apply_interest(ChunkView {
                center,
                render_distance: CHURN_RENDER_DISTANCE,
                chunk_tracking_radius: CHURN_TRACKING_RADIUS,
            })
            .unwrap();

        // The jump cancels delayed demands from the preceding view. Process its
        // now-authoritative holder unload before preparing the next cancellation
        // so restart metadata cannot hide in an ever-growing obsolete tail.
        scheduler.process_pending_unloads(usize::MAX).unwrap();
        post_unload_holder_counts.push(scheduler.holder_count());

        let mut prepared = false;
        for _ in 0..60_000 {
            scheduler.poll().unwrap();
            let metrics = scheduler.metrics();
            context_peak = context_peak.max(metrics.light_restartable_contexts);
            context_byte_peak = context_byte_peak.max(metrics.light_restartable_context_bytes);
            assert!(
                metrics.player_promotion_max_active <= DELAYED_DEMAND_TARGET,
                "step {step}: Player promotion admission exceeded four: {}",
                metrics.player_promotion_max_active,
            );
            assert!(
                metrics.light_restartable_contexts <= scheduler.holder_count(),
                "step {step}: restart context exists without a resident holder: {} contexts / \
                 {} holders",
                metrics.light_restartable_contexts,
                scheduler.holder_count(),
            );
            assert!(
                metrics.light_restartable_context_bytes
                    <= scheduler
                        .holder_count()
                        .saturating_mul(MAX_CONTEXT_BYTES_PER_HOLDER),
                "step {step}: tick-free FlatGrass restart metadata exceeded its resident-holder \
                 envelope: {} bytes / {} holders",
                metrics.light_restartable_context_bytes,
                scheduler.holder_count(),
            );
            if metrics.debug_light_admission_delayed_demands >= DELAYED_DEMAND_TARGET {
                prepared = true;
                break;
            }
            wait_for_scheduler_completion(&mut scheduler);
        }
        assert!(
            prepared,
            "step {step}: did not prepare four delayed Light demands"
        );
    }

    let holder_min = *post_unload_holder_counts.iter().min().unwrap();
    let holder_max = *post_unload_holder_counts.iter().max().unwrap();
    assert_eq!(
        holder_min, holder_max,
        "resident holder set grew under repeated far-view churn"
    );
    assert!(context_peak >= DELAYED_DEMAND_TARGET);
    assert!(context_peak <= holder_max);
    assert!(context_byte_peak <= holder_max * MAX_CONTEXT_BYTES_PER_HOLDER);

    let final_center = ChunkPos::new(CHURN_STEPS * CHUNK_JUMP, 0);
    scheduler
        .apply_interest(ChunkView {
            center: final_center,
            render_distance: CHURN_RENDER_DISTANCE,
            chunk_tracking_radius: CHURN_TRACKING_RADIUS,
        })
        .unwrap();
    scheduler.process_pending_unloads(usize::MAX).unwrap();
    let final_metrics = scheduler.metrics();
    assert_eq!(final_metrics.light_restartable_contexts, 0);
    assert_eq!(final_metrics.light_restartable_context_bytes, 0);
}

fn assert_retained_light_is_bounded(seed: i64, holders: &[usize], retained: &[usize]) {
    for index in 0..STEPS as usize {
        eprintln!(
            "seed {seed} step {:2} center_x={:2} holder_chunks(loaded)={:4} \
             retained_light_chunks={:5} (~{} MB)",
            index,
            index,
            holders[index],
            retained[index],
            retained[index] * APPROX_KB_PER_CHUNK / 1024,
        );
    }

    let holder_peak = *holders.iter().max().expect("at least one step");
    let holder_min = *holders.iter().min().expect("at least one step");
    let retained_initial = retained[0];
    let retained_final = *retained.last().expect("at least one step");
    let retained_peak = *retained.iter().max().expect("at least one step");
    let midpoint = STEPS as usize / 2;
    let second_half_growth = retained_final as i64 - retained[midpoint] as i64;
    eprintln!(
        "SUMMARY seed {seed} holders(loaded)=[{holder_min}..{holder_peak}] flat_bounded={} \
         retained_light: initial={retained_initial} mid={} final={retained_final} \
         peak={retained_peak} second_half_growth={second_half_growth} ratio_vs_one_view={:.2}x \
         approx_retained_mb={}",
        holder_min == holder_peak,
        retained[midpoint],
        retained_peak as f64 / retained_initial.max(1) as f64,
        retained_peak * APPROX_KB_PER_CHUNK / 1024,
    );

    // The loaded set is bounded by the ticket system and stays flat as the center
    // moves.
    assert!(
        holder_min == holder_peak,
        "seed {seed}: loaded set should stay flat under movement: \
         holders=[{holder_min}..{holder_peak}]"
    );
    assert!(
        retained_initial > 0,
        "seed {seed}: expected a non-empty retained light set after the first view"
    );

    // Bounded acceptance. Before Fix A the retained light world grew by ~one
    // movement strip *every* step (+9/step, 3.56x over 24 steps, no plateau)
    // because unloaded chunks were never evicted. With unload eviction wired,
    // retained fills for the first `loaded_radius - lit_radius` steps (while the
    // band's trailing edge is still inside the ticket set) and then plateaus: from
    // that point each leading strip that is lit is matched by a trailing strip
    // that is unloaded and evicted. The plateau value is one view widened by that
    // unload lag — a constant bounded by the loaded set, NOT a function of the
    // distance travelled. Running further steps holds it flat.
    //
    // The leak signature is monotonic growth across the whole run; the fix
    // signature is no net growth once the band has filled. Assert the second half
    // does not grow — this fails loudly if eviction regresses to the leak.
    assert!(
        second_half_growth <= 0,
        "seed {seed}: retained light world is still growing in its second half (leak \
         signature): mid[{midpoint}]={} final={retained_final} (+{second_half_growth} over {} \
         steps). Unload eviction (tactical 155 Fix A) regressed",
        retained[midpoint],
        STEPS as usize - midpoint,
    );

    // Hard steady-state plateau over the final quarter: bounded by the loaded set,
    // not the distance travelled (an evicting store reaches a fixed band width).
    let tail_start = STEPS as usize - STEPS as usize / 4;
    let tail_min = retained[tail_start..].iter().copied().min().unwrap();
    let tail_max = retained[tail_start..].iter().copied().max().unwrap();
    assert_eq!(
        tail_min, tail_max,
        "seed {seed}: retained light world has not reached a steady plateau over the final \
         quarter ([{tail_min}..{tail_max}]): eviction is not keeping pace with unloads"
    );
}

#[test]
fn movement_soak_keeps_retained_light_memory_bounded() {
    // A contrasting seed (rule E) exercises eviction over different terrain —
    // different per-section emptiness patterns mean different stored-section
    // shapes to free. The retained-set *membership* is geometry-driven (which
    // chunks are lit/loaded), independent of terrain, so both seeds must trace the
    // identical bounded trajectory even though their light *data* differs.
    let (holders_a, retained_a) = run_movement_soak(12_345);
    assert_retained_light_is_bounded(12_345, &holders_a, &retained_a);

    let (holders_b, retained_b) = run_movement_soak(987_654_321);
    assert_retained_light_is_bounded(987_654_321, &holders_b, &retained_b);

    assert_eq!(
        (holders_a, retained_a),
        (holders_b, retained_b),
        "loaded/retained trajectories must be geometry-driven and seed-independent; a \
         divergence means eviction is freeing a terrain-dependent set"
    );
}

#[test]
fn long_movement_prunes_full_jobs_and_bounds_recent_summaries() {
    let mut scheduler = ChunkScheduler::new(12_345);
    scheduler
        .set_world_generation_profile(WorldGenerationProfile::FlatGrassV1)
        .unwrap();
    scheduler.set_lighting_enabled(false);

    let steps = MAX_RECENT_COMPLETED_JOB_SUMMARIES + 16;
    for index in 0..steps {
        apply_interest_and_poll(
            &mut scheduler,
            ChunkView {
                center: ChunkPos::new((index * 4) as i32, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            },
        );
        scheduler.process_pending_unloads(usize::MAX).unwrap();
        assert_eq!(
            scheduler.full_job_record_count(),
            0,
            "completed full job survived step {index}"
        );
        assert!(
            scheduler.recent_completed_job_summary_count() <= MAX_RECENT_COMPLETED_JOB_SUMMARIES
        );
    }

    let metrics = scheduler.metrics();
    assert!(metrics.completed_jobs >= steps);
    assert_eq!(metrics.completed_job_records_retained, 0);
    assert_eq!(
        metrics.recent_job_summaries_retained,
        MAX_RECENT_COMPLETED_JOB_SUMMARIES
    );
    assert_eq!(scheduler.job_count(), MAX_RECENT_COMPLETED_JOB_SUMMARIES);
}
