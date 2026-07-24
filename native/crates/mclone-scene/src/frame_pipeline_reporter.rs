use mclone_app_runtime::frame_pipeline_accounting::{
    FramePipelineAccountant, FramePipelineAccountingUpdate, FramePipelinePeerThreadInput,
    FramePipelineQueueDepths, FramePipelineReportExtras, FramePipelineStageTiming,
    frame_pipeline_stage_spans,
};
use mclone_diagnostics::{
    BudgetDecisionPanelReport, FrameAccountingConfig, FrameObservation, PercentileMethod, StageSpan,
};

use crate::{
    MonoSceneFrameSummary, XrTerrainFrameSummary, XrTerrainFrameTiming, XrTerrainUploadSummary,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct XrFramePipelineHostTiming {
    pub frame_wall_ms: f64,
    pub wait_frame_ms: f64,
    pub controller_poll_ms: f64,
    pub rendered: bool,
    pub thread_cpu_ms: Option<f64>,
}

pub fn xr_frame_pipeline_accounting_config(target_hz: Option<f64>) -> FrameAccountingConfig {
    target_hz
        .map_or_else(
            FrameAccountingConfig::without_budget,
            FrameAccountingConfig::from_target_hz,
        )
        .with_percentile_method(PercentileMethod::NearestRank)
}

pub fn record_xr_frame_pipeline(
    accountant: &mut FramePipelineAccountant,
    host: XrFramePipelineHostTiming,
    summary: Option<XrTerrainFrameSummary>,
    budget_decision_panel: BudgetDecisionPanelReport,
) -> FramePipelineAccountingUpdate {
    record_xr_frame_pipeline_with_peer_threads(
        accountant,
        host,
        summary,
        budget_decision_panel,
        None,
    )
}

pub fn record_mono_frame_pipeline(
    accountant: &mut FramePipelineAccountant,
    frame_wall_ms: f64,
    rendered: bool,
    summary: Option<MonoSceneFrameSummary>,
    budget_decision_panel: BudgetDecisionPanelReport,
) -> FramePipelineAccountingUpdate {
    let frame_index = accountant.next_frame_index();
    let frame_wall_ms = sanitize_ms(frame_wall_ms);
    let mut observation = FrameObservation::new(frame_index, frame_wall_ms)
        .with_wait_ms(0.0)
        .with_app_work_ms(frame_wall_ms)
        .with_rendered(rendered);
    if let Some(summary) = summary.as_ref() {
        for span in xr_frame_pipeline_stage_spans(summary.timing) {
            observation = observation.with_stage_span(span);
        }
    }
    let upload = summary.as_ref().map(|summary| summary.upload);
    let queues = xr_frame_pipeline_queue_depths(upload);
    let mut extras =
        FramePipelineReportExtras::default().with_budget_decision_panel(budget_decision_panel);
    if let Some(upload) = upload {
        extras = extras.with_peer_threads(xr_frame_pipeline_peer_threads(upload));
    }
    accountant.record_prebuilt(observation, queues, extras)
}

pub fn record_xr_frame_pipeline_with_peer_threads(
    accountant: &mut FramePipelineAccountant,
    host: XrFramePipelineHostTiming,
    summary: Option<XrTerrainFrameSummary>,
    budget_decision_panel: BudgetDecisionPanelReport,
    peer_threads: Option<FramePipelinePeerThreadInput>,
) -> FramePipelineAccountingUpdate {
    let observation = xr_frame_pipeline_observation(accountant.next_frame_index(), host, summary);
    let queues = xr_frame_pipeline_queue_depths(summary.map(|summary| summary.upload));
    let mut extras =
        FramePipelineReportExtras::default().with_budget_decision_panel(budget_decision_panel);
    if let Some(peer_threads) = peer_threads
        .or_else(|| summary.map(|summary| xr_frame_pipeline_peer_threads(summary.upload)))
    {
        extras = extras.with_peer_threads(peer_threads);
    }
    accountant.record_prebuilt(observation, queues, extras)
}

pub fn xr_frame_pipeline_observation(
    frame_index: u64,
    host: XrFramePipelineHostTiming,
    summary: Option<XrTerrainFrameSummary>,
) -> FrameObservation {
    let frame_wall_ms = sanitize_ms(host.frame_wall_ms);
    let wait_frame_ms = sanitize_ms(host.wait_frame_ms);
    let app_work_ms = (frame_wall_ms - wait_frame_ms).max(0.0);
    let mut observation = FrameObservation::new(frame_index, frame_wall_ms)
        .with_wait_ms(wait_frame_ms)
        .with_app_work_ms(app_work_ms)
        .with_thread_cpu_ms(host.thread_cpu_ms)
        .with_rendered(host.rendered)
        .with_stage_span(mclone_diagnostics::StageSpan::new(
            mclone_diagnostics::StageId::GpuExecutionPresentationWait,
            wait_frame_ms,
        ))
        .with_stage_span(mclone_diagnostics::StageSpan::new(
            mclone_diagnostics::StageId::InputPoseEvents,
            host.controller_poll_ms,
        ));
    if let Some(summary) = summary {
        for span in xr_frame_pipeline_stage_spans(summary.timing) {
            observation = observation.with_stage_span(span);
        }
    }
    observation
}

pub fn xr_frame_pipeline_stage_spans(timing: XrTerrainFrameTiming) -> Vec<StageSpan> {
    let dirty_ready_scan_ms = timing.runtime_dirty_seed_ms + timing.runtime_prepare_ms;
    let request_build_ms =
        timing.runtime_submit_snapshot_ms + timing.runtime_submit_request_build_ms;
    let worker_submit_ms = timing.runtime_submit_compiler_ms;
    let prepared_record_ms = timing.runtime_submit_mark_inflight_ms
        + timing.runtime_submit_apply_ready_plan_ms
        + timing.runtime_submit_ready_update_ms;
    let attributed_sync_ms = timing.runtime_result_accept_ms
        + dirty_ready_scan_ms
        + request_build_ms
        + worker_submit_ms
        + prepared_record_ms;
    let sync_unattributed_ms = timing
        .runtime_sync_unattributed_ms
        .max((timing.runtime_sync_ms - attributed_sync_ms).max(0.0));
    let multiview_draw_ms = timing.multiview_sky_ms
        + timing.multiview_terrain_ms
        + timing.multiview_actor_ms
        + timing.multiview_screen_effect_ms
        + timing.multiview_world_overlays_ms
        + timing.multiview_submit_ms;
    frame_pipeline_stage_spans(FramePipelineStageTiming {
        host_session_commands_ms: timing.runtime_poll_ms,
        completed_result_acceptance_ms: timing.runtime_result_accept_ms,
        render_admission_dirty_ready_scan_ms: dirty_ready_scan_ms,
        render_admission_request_build_ms: request_build_ms,
        render_admission_worker_submit_ms: worker_submit_ms,
        render_admission_prepared_record_maintenance_ms: prepared_record_ms,
        render_section_admission_ms: sync_unattributed_ms,
        upload_apply_ms: timing.runtime_gpu_upload_ms,
        prepared_draw_records_ms: timing.shared_records_ms,
        draw_encode_ms: timing.render_views_ms.max(multiview_draw_ms),
    })
}

pub fn xr_frame_pipeline_queue_depths(
    upload: Option<XrTerrainUploadSummary>,
) -> FramePipelineQueueDepths {
    let Some(upload) = upload else {
        return FramePipelineQueueDepths::default();
    };
    FramePipelineQueueDepths {
        observed: true,
        server_owned_lanes_remote: upload.host_mode.server_owned_lanes_are_remote(),
        inbound_updates: upload.server_update_queue_depth,
        host_publication_runner: upload.server_pending_publications,
        host_publication_worldgen: upload.poll_scheduler_pending_worldgen_publication_chunks,
        host_publication_light: upload.poll_scheduler_pending_light_publications,
        render_compile_jobs: upload.pending_compile_jobs_after,
        completed_results: upload.queued_completed_compile_result_count,
        completed_results_processed: upload.completed_compile_section_count,
        upload_work: upload.queued_upload_lifecycle_item_count,
        upload_work_processed: 0,
    }
}

pub fn xr_frame_pipeline_peer_threads(
    upload: XrTerrainUploadSummary,
) -> FramePipelinePeerThreadInput {
    FramePipelinePeerThreadInput {
        server_owned_lanes_remote: upload.host_mode.server_owned_lanes_are_remote(),
        server_pending_jobs: upload.server_pending_jobs,
        server_busy_ms: upload.poll_server_reported_total_ms,
        worldgen_metrics: upload.worldgen_job_frame_metrics,
        worldgen_pending_jobs: upload.poll_scheduler_worldgen_mailbox_pending_jobs,
        light_metrics: upload.light_status_job_frame_metrics,
        light_pending_jobs: upload.poll_scheduler_light_mailbox_pending_statuses,
        render_compile_pending_jobs: upload.pending_compile_jobs_after,
        render_compile_submitted: upload.submitted_compile_section_count,
        render_compile_completed: upload.completed_compile_section_count,
        render_compile_busy_ms: upload.dispatcher_total_compile_worker_busy_ms,
        render_compile_max_task_ms: upload.dispatcher_max_compile_worker_task_ms,
        render_compile_idle_ms: None,
    }
}

fn sanitize_ms(ms: f64) -> f64 {
    if ms.is_finite() { ms.max(0.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_diagnostics::{DiagnosticLaneAvailability, PeerThreadId, QueueId, StageId};

    #[test]
    fn reporter_records_visible_frame_report() {
        let mut accountant =
            FramePipelineAccountant::new(xr_frame_pipeline_accounting_config(Some(90.0)));
        let update = record_xr_frame_pipeline(
            &mut accountant,
            XrFramePipelineHostTiming {
                frame_wall_ms: 11.0,
                wait_frame_ms: 2.0,
                controller_poll_ms: 0.25,
                rendered: true,
                thread_cpu_ms: Some(8.0),
            },
            None,
            BudgetDecisionPanelReport::empty(),
        );
        let (report, revision) = update.published_report.expect("exact report");
        assert_eq!(revision, 1);
        assert_eq!(report.frame_summary.frames, 1);
        assert_eq!(report.queue_panel.queues.len(), 8);
        assert!(report.stage_spans.iter().any(|span| {
            span.stage == StageId::GpuExecutionPresentationWait && span.elapsed_ms == 2.0
        }));
    }

    #[test]
    fn mono_reporter_records_shared_timing_and_queue_depths() {
        let mut accountant =
            FramePipelineAccountant::new(xr_frame_pipeline_accounting_config(Some(60.0)));
        let update = record_mono_frame_pipeline(
            &mut accountant,
            16.0,
            true,
            Some(MonoSceneFrameSummary {
                render: Default::default(),
                render_timing: Default::default(),
                timing: XrTerrainFrameTiming {
                    runtime_poll_ms: 0.75,
                    render_views_ms: 2.5,
                    ..Default::default()
                },
                upload: XrTerrainUploadSummary {
                    server_update_queue_depth: 4,
                    pending_compile_jobs_after: 2,
                    ..Default::default()
                },
            }),
            BudgetDecisionPanelReport::empty(),
        );
        let (report, revision) = update.published_report.expect("exact report");

        assert_eq!(revision, 1);
        assert_eq!(report.frame_summary.frames, 1);
        assert!(
            report.stage_spans.iter().any(|span| {
                span.stage == StageId::HostSessionCommands && span.elapsed_ms == 0.75
            })
        );
        let inbound = report
            .queue_panel
            .queues
            .iter()
            .find(|queue| queue.queue == QueueId::InboundUpdates)
            .expect("inbound queue");
        assert_eq!(inbound.depth, 4);
    }

    #[test]
    fn reporter_marks_remote_host_lanes_as_unavailable_locally() {
        let mut accountant =
            FramePipelineAccountant::new(xr_frame_pipeline_accounting_config(Some(72.0)));
        let update = record_xr_frame_pipeline(
            &mut accountant,
            XrFramePipelineHostTiming {
                frame_wall_ms: 13.0,
                wait_frame_ms: 4.0,
                controller_poll_ms: 0.25,
                rendered: true,
                thread_cpu_ms: Some(7.0),
            },
            Some(XrTerrainFrameSummary {
                rendered_frames: 1,
                section_count: 1,
                drawn_section_count: 1,
                index_count: 6,
                drawn_index_count: 6,
                gui_command_count: 0,
                ui_panel: Default::default(),
                ui_draw_cache: Default::default(),
                ui_active: false,
                local_startup_active: false,
                actor_count: 0,
                drawn_actor_count: 0,
                head_comfort: Default::default(),
                timing: XrTerrainFrameTiming::default(),
                upload: XrTerrainUploadSummary {
                    host_mode: crate::XrTerrainHostMode::RemoteDedicated,
                    server_update_queue_depth: 3,
                    pending_compile_jobs_after: 2,
                    ..XrTerrainUploadSummary::default()
                },
            }),
            BudgetDecisionPanelReport::empty(),
        );
        let (report, _revision) = update.published_report.expect("exact report");

        for queue_id in [
            QueueId::HostPublication,
            QueueId::HostPublicationRunner,
            QueueId::HostPublicationWorldgen,
            QueueId::HostPublicationLight,
        ] {
            let queue = report
                .queue_panel
                .queues
                .iter()
                .find(|queue| queue.queue == queue_id)
                .expect("host publication queue");
            assert_eq!(queue.availability, DiagnosticLaneAvailability::RemoteHost);
            assert_eq!(queue.depth, 0);
        }
        let inbound_updates = report
            .queue_panel
            .queues
            .iter()
            .find(|queue| queue.queue == QueueId::InboundUpdates)
            .expect("inbound update queue");
        assert_eq!(inbound_updates.depth, 3);

        for lane in [
            PeerThreadId::ServerRunner,
            PeerThreadId::Worldgen,
            PeerThreadId::LightStatus,
        ] {
            let peer = report
                .peer_thread_panel
                .peers
                .iter()
                .find(|peer| peer.lane == lane)
                .expect("server-owned peer lane");
            assert_eq!(peer.availability, DiagnosticLaneAvailability::RemoteHost);
            assert!(!peer.active);
        }
        let render_workers = report
            .peer_thread_panel
            .peers
            .iter()
            .find(|peer| peer.lane == PeerThreadId::RenderCompileWorkers)
            .expect("render workers lane");
        assert_eq!(
            render_workers.availability,
            DiagnosticLaneAvailability::Local
        );
        assert!(render_workers.active);
    }

    #[test]
    fn stage_spans_include_xr_scene_timing() {
        let timing = XrTerrainFrameTiming {
            runtime_poll_ms: 0.5,
            runtime_sync_ms: 4.0,
            runtime_result_accept_ms: 0.25,
            runtime_dirty_seed_ms: 0.5,
            runtime_prepare_ms: 0.5,
            runtime_submit_snapshot_ms: 0.25,
            runtime_submit_request_build_ms: 0.25,
            runtime_submit_compiler_ms: 0.5,
            runtime_submit_mark_inflight_ms: 0.25,
            runtime_submit_apply_ready_plan_ms: 0.25,
            runtime_submit_ready_update_ms: 0.25,
            runtime_gpu_upload_ms: 1.0,
            shared_records_ms: 0.75,
            render_views_ms: 3.0,
            ..Default::default()
        };
        let spans = xr_frame_pipeline_stage_spans(timing);
        assert!(spans.iter().any(|span| {
            span.stage == StageId::RenderSectionAdmission && span.elapsed_ms == 1.0
        }));
        assert!(
            spans
                .iter()
                .any(|span| span.stage == StageId::DrawEncode && span.elapsed_ms == 3.0)
        );
    }

    #[test]
    fn completed_result_events_preserve_queue_age_semantics() {
        let mut accountant =
            FramePipelineAccountant::new(xr_frame_pipeline_accounting_config(Some(90.0)));
        let summary = XrTerrainFrameSummary {
            rendered_frames: 1,
            section_count: 0,
            drawn_section_count: 0,
            index_count: 0,
            drawn_index_count: 0,
            gui_command_count: 0,
            ui_panel: Default::default(),
            ui_draw_cache: Default::default(),
            ui_active: false,
            local_startup_active: false,
            actor_count: 0,
            drawn_actor_count: 0,
            head_comfort: Default::default(),
            timing: XrTerrainFrameTiming::default(),
            upload: XrTerrainUploadSummary {
                completed_compile_section_count: 2,
                queued_completed_compile_result_count: 1,
                ..XrTerrainUploadSummary::default()
            },
        };
        let update = record_xr_frame_pipeline(
            &mut accountant,
            XrFramePipelineHostTiming {
                frame_wall_ms: 10.0,
                rendered: true,
                ..Default::default()
            },
            Some(summary),
            BudgetDecisionPanelReport::empty(),
        );
        let (report, _) = update.published_report.expect("exact report");
        let completed = report
            .queue_panel
            .queues
            .iter()
            .find(|queue| queue.queue == QueueId::CompletedRenderResults)
            .expect("completed result queue");
        assert_eq!(completed.enqueued_total, 3);
        assert_eq!(completed.dequeued_total, 2);
        assert_eq!(completed.depth, 1);
    }

    #[test]
    fn skipped_render_advances_age_without_clearing_last_queue_depths() {
        let mut accountant =
            FramePipelineAccountant::new(xr_frame_pipeline_accounting_config(Some(90.0)));
        let summary = XrTerrainFrameSummary {
            rendered_frames: 1,
            section_count: 0,
            drawn_section_count: 0,
            index_count: 0,
            drawn_index_count: 0,
            gui_command_count: 0,
            ui_panel: Default::default(),
            ui_draw_cache: Default::default(),
            ui_active: false,
            local_startup_active: false,
            actor_count: 0,
            drawn_actor_count: 0,
            head_comfort: Default::default(),
            timing: XrTerrainFrameTiming::default(),
            upload: XrTerrainUploadSummary {
                queued_upload_lifecycle_item_count: 1,
                ..XrTerrainUploadSummary::default()
            },
        };
        record_xr_frame_pipeline(
            &mut accountant,
            XrFramePipelineHostTiming {
                frame_wall_ms: 10.0,
                rendered: true,
                ..Default::default()
            },
            Some(summary),
            BudgetDecisionPanelReport::empty(),
        );
        let update = record_xr_frame_pipeline(
            &mut accountant,
            XrFramePipelineHostTiming {
                frame_wall_ms: 10.0,
                rendered: false,
                ..Default::default()
            },
            None,
            BudgetDecisionPanelReport::empty(),
        );
        let (report, _) = update.published_report.expect("exact report");
        let upload = report
            .queue_panel
            .queues
            .iter()
            .find(|queue| queue.queue == QueueId::UploadWork)
            .expect("upload queue");
        assert_eq!(upload.depth, 1);
        assert_eq!(upload.oldest_age_ms, Some(10.0));
    }
}
