//! Tactical 312 deterministic native reproduction for promotion re-entry.
//!
//! The shared scheduler delays initial-Light admission long enough to move
//! interest away and back before the original demand enters either physical
//! executor. The final stationary phase requires complete coverage and drained
//! ownership; the pre-fix scheduler exits nonzero with four inert promotions.

use std::time::{Duration, Instant};

use mclone_core::ChunkPos;
use mclone_protocol::ChunkView;
use mclone_server::{ChunkScheduler, ChunkSchedulerMetrics};

const RENDER_DISTANCE: u32 = 8;
const CHUNK_TRACKING_RADIUS: u32 = RENDER_DISTANCE + 1;
const LIGHT_ADMISSION_DELAY_TICKS: u32 = 80;
const PREPARE_POLL_LIMIT: usize = 120_000;
const SETTLE_TIMEOUT: Duration = Duration::from_secs(60);
const WORKER_NUDGE: Duration = Duration::from_millis(1);
const STALL_STABILITY_POLLS: usize = 16;

fn main() {
    if let Err(error) = run() {
        eprintln!("scheduler promotion churn probe failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut scheduler = ChunkScheduler::new(12_345);
    scheduler.set_debug_light_admission_delay_ticks(LIGHT_ADMISSION_DELAY_TICKS);
    let origin = ChunkPos::new(0, 0);
    let departed = ChunkPos::new(32, 0);
    apply_view(&mut scheduler, origin)?;

    let mut prepared = None;
    for poll in 1..=PREPARE_POLL_LIMIT {
        scheduler.poll().map_err(|error| error.to_string())?;
        let metrics = scheduler.metrics();
        if metrics.debug_light_admission_delayed_demands >= 4 {
            prepared = Some((poll, metrics));
            break;
        }
        scheduler.wait_for_worldgen_completion(WORKER_NUDGE);
        scheduler.wait_for_light_completion(WORKER_NUDGE);
    }
    let (prepare_polls, prepared_metrics) = prepared.ok_or_else(|| {
        format!(
            "did not prepare four delayed Light demands; metrics={:?}",
            scheduler.metrics()
        )
    })?;

    // The far jump makes every prepared request obsolete. Returning before
    // pending unloads run rescues the original holders and exercises the exact
    // cancellation/re-entry boundary observed under ordinary view churn.
    apply_view(&mut scheduler, departed)?;
    apply_view(&mut scheduler, origin)?;
    let reentered_metrics = scheduler.metrics();

    let expected_chunks = ((CHUNK_TRACKING_RADIUS * 2 + 1) as usize).pow(2);
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    let mut settle_polls = 0_usize;
    let mut stable_orphan_polls = 0_usize;
    let final_metrics = loop {
        scheduler.poll().map_err(|error| error.to_string())?;
        settle_polls = settle_polls.saturating_add(1);
        let metrics = scheduler.metrics();
        let converged = metrics.client_visible_chunks == expected_chunks
            && metrics.player_promotion_queued == 0
            && metrics.player_promotion_active == 0
            && metrics.pending_jobs == 0
            && scheduler.pending_publication_count() == 0;
        if converged {
            break metrics;
        }

        let inert_orphan = metrics.player_promotion_active > 0
            && metrics.player_promotion_active
                == metrics.player_promotion_active_light_scheduled_without_token
            && metrics.light_demand_queued == 0
            && scheduler.worldgen_mailbox_pending_count() == 0
            && scheduler.light_status_mailbox_pending_count() == 0
            && scheduler.pending_publication_count() == 0;
        stable_orphan_polls = if inert_orphan {
            stable_orphan_polls.saturating_add(1)
        } else {
            0
        };
        if stable_orphan_polls >= STALL_STABILITY_POLLS || Instant::now() >= deadline {
            break metrics;
        }
        scheduler.wait_for_worldgen_completion(WORKER_NUDGE);
        scheduler.wait_for_light_completion(WORKER_NUDGE);
    };

    print_report(
        prepare_polls,
        settle_polls,
        prepared_metrics,
        reentered_metrics,
        final_metrics,
        scheduler.pending_publication_count(),
        scheduler.worldgen_mailbox_pending_count(),
        scheduler.light_status_mailbox_pending_count(),
        expected_chunks,
    );

    if final_metrics.client_visible_chunks != expected_chunks
        || final_metrics.player_promotion_queued != 0
        || final_metrics.player_promotion_active != 0
        || final_metrics.pending_jobs != 0
        || final_metrics.light_deferred != 0
        || final_metrics.light_demand_queued != 0
        || final_metrics.light_restartable_contexts != 0
        || final_metrics.light_ticket_count != 0
        || final_metrics.light_scheduled_without_token != 0
        || final_metrics.player_promotion_active_light_token_without_owner != 0
        || scheduler.pending_publication_count() != 0
        || scheduler.worldgen_mailbox_pending_count() != 0
        || scheduler.light_status_mailbox_pending_count() != 0
    {
        return Err(format!(
            "stationary RD{RENDER_DISTANCE} view did not converge; active_orphans={} queued={} visible={}/{}",
            final_metrics.player_promotion_active_light_scheduled_without_token,
            final_metrics.player_promotion_queued,
            final_metrics.client_visible_chunks,
            expected_chunks,
        ));
    }
    Ok(())
}

fn apply_view(scheduler: &mut ChunkScheduler, center: ChunkPos) -> Result<(), String> {
    scheduler
        .apply_interest(ChunkView {
            center,
            render_distance: RENDER_DISTANCE,
            chunk_tracking_radius: CHUNK_TRACKING_RADIUS,
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
fn print_report(
    prepare_polls: usize,
    settle_polls: usize,
    prepared: ChunkSchedulerMetrics,
    reentered: ChunkSchedulerMetrics,
    final_metrics: ChunkSchedulerMetrics,
    pending_publications: usize,
    worldgen_mailbox_pending: usize,
    light_mailbox_pending: usize,
    expected_chunks: usize,
) {
    println!("{{");
    println!("  \"probe\": \"scheduler-promotion-churn\",");
    println!("  \"render_distance\": {RENDER_DISTANCE},");
    println!("  \"debug_light_admission_delay_ticks\": {LIGHT_ADMISSION_DELAY_TICKS},");
    println!("  \"prepare_polls\": {prepare_polls},");
    println!("  \"settle_polls\": {settle_polls},");
    println!(
        "  \"prepared_delayed_light_demands\": {},",
        prepared.debug_light_admission_delayed_demands
    );
    println!(
        "  \"reentered_active_promotions\": {},",
        reentered.player_promotion_active
    );
    println!(
        "  \"reentered_active_scheduled_without_token\": {},",
        reentered.player_promotion_active_light_scheduled_without_token
    );
    println!(
        "  \"reentered_active_light_deferred\": {},",
        reentered.player_promotion_active_light_deferred
    );
    println!(
        "  \"reentered_active_light_demand_queued\": {},",
        reentered.player_promotion_active_light_demand_queued
    );
    println!("  \"light_retries\": {},", final_metrics.light_retries);
    println!("  \"light_repairs\": {},", final_metrics.light_repairs);
    println!("  \"final\": {{");
    println!(
        "    \"client_visible_chunks\": {},",
        final_metrics.client_visible_chunks
    );
    println!("    \"expected_chunks\": {expected_chunks},");
    println!(
        "    \"player_promotion_queued\": {},",
        final_metrics.player_promotion_queued
    );
    println!(
        "    \"player_promotion_active\": {},",
        final_metrics.player_promotion_active
    );
    println!(
        "    \"active_scheduled_without_token\": {},",
        final_metrics.player_promotion_active_light_scheduled_without_token
    );
    println!(
        "    \"active_token_without_owner\": {},",
        final_metrics.player_promotion_active_light_token_without_owner
    );
    println!("    \"light_deferred\": {},", final_metrics.light_deferred);
    println!(
        "    \"light_demand_queued\": {},",
        final_metrics.light_demand_queued
    );
    println!(
        "    \"light_restartable_contexts\": {},",
        final_metrics.light_restartable_contexts
    );
    println!(
        "    \"light_restartable_context_bytes\": {},",
        final_metrics.light_restartable_context_bytes
    );
    println!(
        "    \"light_ticket_count\": {},",
        final_metrics.light_ticket_count
    );
    println!(
        "    \"light_scheduled_without_token\": {},",
        final_metrics.light_scheduled_without_token
    );
    println!("    \"pending_jobs\": {},", final_metrics.pending_jobs);
    println!("    \"pending_publications\": {pending_publications},");
    println!("    \"worldgen_mailbox_pending\": {worldgen_mailbox_pending},");
    println!("    \"light_mailbox_pending\": {light_mailbox_pending}");
    println!("  }}");
    println!("}}");
}
