use super::*;

#[derive(Clone, Copy, Debug)]
pub struct XrTerrainFrameSummary {
    pub rendered_frames: u32,
    pub section_count: usize,
    pub drawn_section_count: usize,
    pub index_count: u32,
    pub drawn_index_count: u32,
    pub gui_command_count: usize,
    pub ui_panel: WorldGuiPanelRenderStats,
    pub ui_draw_cache: UiDrawCacheStats,
    pub ui_active: bool,
    pub local_startup_active: bool,
    pub actor_count: usize,
    pub drawn_actor_count: usize,
    pub head_comfort: XrHeadComfortState,
    pub timing: XrTerrainFrameTiming,
    pub upload: XrTerrainUploadSummary,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct XrTerrainFrameTiming {
    pub render_views_ms: f64,
    pub menu_pointer_ms: f64,
    pub runtime_upload_ms: f64,
    pub runtime_poll_ms: f64,
    pub runtime_pending_render_count_ms: f64,
    pub runtime_sync_ms: f64,
    pub runtime_sync_unattributed_ms: f64,
    pub runtime_result_accept_ms: f64,
    pub runtime_dirty_seed_ms: f64,
    pub runtime_prepare_ms: f64,
    pub runtime_submit_ms: f64,
    pub runtime_submit_snapshot_ms: f64,
    pub runtime_submit_handoff_ms: f64,
    pub runtime_submit_handoff_worst_ms: f64,
    pub runtime_submit_request_count: usize,
    pub runtime_submit_request_build_ms: f64,
    pub runtime_submit_compiler_ms: f64,
    pub runtime_submit_compiler_worst_ms: f64,
    pub runtime_submit_compiler_capacity_check_ms: f64,
    pub runtime_submit_compiler_capacity_check_worst_ms: f64,
    pub runtime_submit_compiler_command_send_ms: f64,
    pub runtime_submit_compiler_command_send_worst_ms: f64,
    pub runtime_submit_compiler_command_lock_wait_ms: f64,
    pub runtime_submit_compiler_command_lock_wait_worst_ms: f64,
    pub runtime_submit_compiler_command_slot_select_ms: f64,
    pub runtime_submit_compiler_command_slot_select_worst_ms: f64,
    pub runtime_submit_compiler_command_slot_write_ms: f64,
    pub runtime_submit_compiler_command_slot_write_worst_ms: f64,
    pub runtime_submit_compiler_command_queue_push_ms: f64,
    pub runtime_submit_compiler_command_queue_push_worst_ms: f64,
    pub runtime_submit_compiler_command_notify_ms: f64,
    pub runtime_submit_compiler_command_notify_worst_ms: f64,
    pub runtime_submit_compiler_command_post_enqueue_ms: f64,
    pub runtime_submit_compiler_command_post_enqueue_worst_ms: f64,
    pub runtime_submit_compiler_pending_mark_ms: f64,
    pub runtime_submit_compiler_pending_mark_worst_ms: f64,
    pub runtime_submit_mark_inflight_ms: f64,
    pub runtime_submit_apply_ready_plan_ms: f64,
    pub runtime_submit_ready_update_ms: f64,
    pub runtime_submit_ready_section_count: usize,
    pub runtime_submit_deferred_section_count: usize,
    pub runtime_submit_dirty_chunk_count_before: usize,
    pub runtime_submit_dirty_chunk_count_after: usize,
    pub runtime_submit_dirty_section_count_before: usize,
    pub runtime_submit_dirty_section_count_after: usize,
    pub runtime_submit_inflight_section_count_before: usize,
    pub runtime_submit_inflight_section_count_after: usize,
    pub runtime_submit_request_target_section_count: usize,
    pub runtime_submit_request_target_section_count_worst: usize,
    pub runtime_submit_request_snapshot_count: usize,
    pub runtime_submit_request_snapshot_section_count: usize,
    pub runtime_submit_request_snapshot_section_count_worst: usize,
    pub runtime_submit_request_light_section_count: usize,
    pub runtime_submit_request_light_section_count_worst: usize,
    pub runtime_submit_request_revision_count: usize,
    pub runtime_submit_request_estimated_payload_bytes: usize,
    pub runtime_submit_request_estimated_payload_bytes_worst: usize,
    pub runtime_dispatcher_pending_jobs: usize,
    pub runtime_dispatcher_max_pending_jobs: usize,
    pub runtime_dispatcher_available_job_slots: usize,
    pub runtime_dispatcher_queued_compile_tasks: usize,
    pub runtime_dispatcher_compile_worker_count: usize,
    pub runtime_dispatcher_completed_compile_tasks: usize,
    pub runtime_dispatcher_total_compile_worker_busy_ms: f64,
    pub runtime_dispatcher_max_compile_worker_task_ms: f64,
    pub runtime_gpu_upload_ms: f64,
    pub runtime_gpu_upload_pre_sync_ms: f64,
    pub runtime_gpu_upload_post_sync_ms: f64,
    pub runtime_upload_enqueue_ms: f64,
    pub runtime_upload_select_ms: f64,
    pub runtime_upload_apply_ms: f64,
    pub runtime_upload_apply_dirty_mark_ms: f64,
    pub runtime_upload_apply_remove_ms: f64,
    pub runtime_upload_apply_section_state_ms: f64,
    pub runtime_upload_apply_vertex_bytes_ms: f64,
    pub runtime_upload_apply_vertex_buffer_ms: f64,
    pub runtime_upload_apply_index_bytes_ms: f64,
    pub runtime_upload_apply_index_buffer_ms: f64,
    pub runtime_upload_apply_mesh_insert_ms: f64,
    pub runtime_upload_apply_mesh_upload_worst_ms: f64,
    pub runtime_ready_sections_ms: f64,
    pub runtime_ready_publish_ms: f64,
    pub shared_records_ms: f64,
    pub record_cache_prepare: TexturedSectionRecordPrepareStats,
    pub left_eye_ms: f64,
    pub right_eye_ms: f64,
    pub left_eye_render: XrTerrainEyeRenderTiming,
    pub right_eye_render: XrTerrainEyeRenderTiming,
    pub stereo_finish_ms: f64,
    pub stereo_submit_ms: f64,
    pub stereo_poll_wait_ms: f64,
    pub overlap_runtime_prefetch_ms: f64,
    pub overlap_runtime_prefetch_poll_ms: f64,
    pub overlap_runtime_prefetch_sync_ms: f64,
    pub overlap_runtime_prefetch_gpu_upload_ms: f64,
    pub overlap_runtime_prefetch_ready_sections_ms: f64,
    pub multiview_sky_ms: f64,
    pub multiview_terrain_ms: f64,
    pub multiview_actor_ms: f64,
    pub multiview_screen_effect_ms: f64,
    pub multiview_world_overlays_ms: f64,
    pub multiview_submit_ms: f64,
    pub multiview_poll_wait_ms: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct XrLocomotionTiming {
    pub input_ms: f64,
    pub camera_apply_ms: f64,
    pub commit_ms: f64,
    pub commit_server_command_ms: f64,
    pub commit_server_command_send_ms: f64,
    pub commit_server_command_drain_updates_ms: f64,
    pub commit_server_command_apply_updates_ms: f64,
    pub commit_server_command_apply_dirty_mark_ms: f64,
    pub commit_server_command_apply_client_updates_ms: f64,
    pub commit_server_command_updates: usize,
    pub commit_server_command_snapshot_updates: usize,
    pub commit_server_command_section_block_updates: usize,
    pub commit_server_command_unload_updates: usize,
    pub commit_position_updates_ms: f64,
    pub commit_interest_ms: f64,
    pub commit_interest_command_send_ms: f64,
    pub commit_interest_command_drain_updates_ms: f64,
    pub commit_interest_command_apply_updates_ms: f64,
    pub commit_interest_command_apply_dirty_mark_ms: f64,
    pub commit_interest_command_apply_client_updates_ms: f64,
    pub commit_interest_command_updates: usize,
    pub commit_interest_command_snapshot_updates: usize,
    pub commit_interest_command_section_block_updates: usize,
    pub commit_interest_command_unload_updates: usize,
    pub gameplay_interaction_ms: f64,
}

impl XrLocomotionTiming {
    pub(crate) fn record_commit_timing(&mut self, timing: EngineCameraCommitTiming) {
        self.commit_server_command_ms = timing.server_command_ms;
        self.commit_server_command_send_ms = timing.server_command.send_ms;
        self.commit_server_command_drain_updates_ms = timing.server_command.drain_updates_ms;
        self.commit_server_command_apply_updates_ms = timing.server_command.apply_updates_ms;
        self.commit_server_command_apply_dirty_mark_ms = timing.server_command.apply_dirty_mark_ms;
        self.commit_server_command_apply_client_updates_ms =
            timing.server_command.apply_client_updates_ms;
        self.commit_server_command_updates = timing.server_command.updates;
        self.commit_server_command_snapshot_updates = timing.server_command.snapshot_updates;
        self.commit_server_command_section_block_updates =
            timing.server_command.section_block_updates;
        self.commit_server_command_unload_updates = timing.server_command.unload_updates;
        self.commit_position_updates_ms = timing.position_updates_ms;
        self.commit_interest_ms = timing.interest_ms;
        self.commit_interest_command_send_ms = timing.interest_command.send_ms;
        self.commit_interest_command_drain_updates_ms = timing.interest_command.drain_updates_ms;
        self.commit_interest_command_apply_updates_ms = timing.interest_command.apply_updates_ms;
        self.commit_interest_command_apply_dirty_mark_ms =
            timing.interest_command.apply_dirty_mark_ms;
        self.commit_interest_command_apply_client_updates_ms =
            timing.interest_command.apply_client_updates_ms;
        self.commit_interest_command_updates = timing.interest_command.updates;
        self.commit_interest_command_snapshot_updates = timing.interest_command.snapshot_updates;
        self.commit_interest_command_section_block_updates =
            timing.interest_command.section_block_updates;
        self.commit_interest_command_unload_updates = timing.interest_command.unload_updates;
    }

    pub(crate) fn record_interest_command_timing(&mut self, timing: GameplayCommandTiming) {
        self.commit_interest_command_send_ms = timing.send_ms;
        self.commit_interest_command_drain_updates_ms = timing.drain_updates_ms;
        self.commit_interest_command_apply_updates_ms = timing.apply_updates_ms;
        self.commit_interest_command_apply_dirty_mark_ms = timing.apply_dirty_mark_ms;
        self.commit_interest_command_apply_client_updates_ms = timing.apply_client_updates_ms;
        self.commit_interest_command_updates = timing.updates;
        self.commit_interest_command_snapshot_updates = timing.snapshot_updates;
        self.commit_interest_command_section_block_updates = timing.section_block_updates;
        self.commit_interest_command_unload_updates = timing.unload_updates;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct XrTerrainEyeRenderTiming {
    pub full_frame_ms: f64,
    pub sky_ms: f64,
    pub terrain_opaque_ms: f64,
    pub terrain_translucent_ms: f64,
    pub prepare_ms: f64,
    pub cull_ms: f64,
    pub uniform_write_ms: f64,
    pub translucent_collect_ms: f64,
    pub translucent_sort_ms: f64,
    pub encode_ms: f64,
    pub section_encode_ms: f64,
    pub placed_cull_ms: f64,
    pub placed_draw_ms: f64,
    pub actor_ms: f64,
    pub placed_actor_ms: f64,
    pub screen_effect_ms: f64,
    pub gui_ms: f64,
    pub xr_fade_ms: f64,
    pub xr_selection_ms: f64,
    pub xr_world_lines_ms: f64,
    pub xr_world_panel_ms: f64,
    pub encoder_finish_ms: f64,
    pub submit_ms: f64,
    pub poll_wait_ms: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct XrTerrainUploadSummary {
    pub host_mode: XrTerrainHostMode,
    pub poll_changed: bool,
    pub poll_total_ms: f64,
    pub poll_drain_updates_ms: f64,
    pub poll_producer_read_ms: f64,
    pub poll_producer_decode_ms: f64,
    pub poll_producer_inbound_frame_sequence: Option<u64>,
    pub poll_client_deferred_chunk_drop_ms: f64,
    pub poll_client_deferred_chunk_drop_items: usize,
    pub poll_client_deferred_chunk_drop_backlog_items: usize,
    pub poll_apply_updates_ms: f64,
    pub poll_dirty_mark_ms: f64,
    pub poll_client_apply_updates_ms: f64,
    pub poll_snapshot_update_apply_ms: f64,
    pub poll_snapshot_update_dirty_mark_ms: f64,
    pub poll_snapshot_update_client_apply_ms: f64,
    pub poll_section_block_update_apply_ms: f64,
    pub poll_section_block_update_dirty_mark_ms: f64,
    pub poll_section_block_update_client_apply_ms: f64,
    pub poll_unload_update_apply_ms: f64,
    pub poll_unload_update_dirty_mark_ms: f64,
    pub poll_unload_update_client_apply_ms: f64,
    pub poll_other_update_apply_ms: f64,
    pub poll_other_update_dirty_mark_ms: f64,
    pub poll_other_update_client_apply_ms: f64,
    pub poll_mixed_update_apply_ms: f64,
    pub poll_mixed_update_dirty_mark_ms: f64,
    pub poll_mixed_update_client_apply_ms: f64,
    pub update_pump_stalled: bool,
    pub update_pump_stall_count: usize,
    pub server_update_applied_bytes: usize,
    pub server_update_oldest_applied_age_ms: f64,
    pub poll_diagnostics_ms: f64,
    pub poll_diagnostics_refreshed: bool,
    pub poll_diagnostics_cache_age_ms: f64,
    pub server_diagnostics_detail_refreshes: u64,
    pub server_diagnostics_detail_age_ms: f64,
    pub poll_server_tick_ms: f64,
    pub poll_server_reported_total_ms: f64,
    pub poll_scheduler_tick_ms: f64,
    pub poll_scheduler_reconcile_holders_ms: f64,
    pub poll_scheduler_active_levels_ms: f64,
    pub poll_scheduler_holder_updates_ms: f64,
    pub poll_scheduler_runtime_enqueue_ms: f64,
    pub poll_scheduler_active_levels_calls: usize,
    pub poll_scheduler_active_levels_cache_hits: usize,
    pub poll_scheduler_holder_update_count: usize,
    pub poll_scheduler_runtime_target_count: usize,
    pub poll_scheduler_adaptive_publication_budget_enabled: bool,
    pub poll_scheduler_feature_publish_budget_max_units: usize,
    pub poll_scheduler_feature_publish_budget_ms: f64,
    pub poll_scheduler_feature_publish_spent_units: usize,
    pub poll_scheduler_feature_publish_spent_ms: f64,
    pub poll_scheduler_light_publish_budget_max_units: usize,
    pub poll_scheduler_light_publish_budget_ms: f64,
    pub poll_scheduler_light_publish_spent_units: usize,
    pub poll_scheduler_light_publish_spent_ms: f64,
    pub poll_scheduler_pending_worldgen_publication_chunk_limit: usize,
    pub poll_scheduler_completed_feature_jobs_drained: usize,
    pub poll_scheduler_feature_chunks_published: usize,
    pub poll_scheduler_feature_chunks_skipped: usize,
    pub poll_scheduler_feature_jobs_completed: usize,
    pub poll_scheduler_feature_snapshot_ready_events: usize,
    pub poll_scheduler_light_status_batches_enqueued: usize,
    pub poll_scheduler_completed_light_statuses_drained: usize,
    pub poll_scheduler_light_statuses_published: usize,
    pub poll_scheduler_light_statuses_skipped: usize,
    pub poll_scheduler_light_snapshot_ready_events: usize,
    pub poll_scheduler_cumulative_feature_chunks_published: u64,
    pub poll_scheduler_cumulative_light_statuses_published: u64,
    pub poll_scheduler_pending_worldgen_publication_jobs: usize,
    pub poll_scheduler_pending_worldgen_publication_chunks: usize,
    pub poll_scheduler_pending_light_publications: usize,
    pub poll_scheduler_worldgen_mailbox_pending_jobs: usize,
    pub poll_scheduler_light_mailbox_pending_statuses: usize,
    pub poll_updates: usize,
    pub poll_snapshot_updates: usize,
    pub poll_section_block_updates: usize,
    pub poll_unload_updates: usize,
    pub poll_other_updates: usize,
    pub poll_mixed_updates: usize,
    pub poll_fluid_due_ticks: usize,
    pub poll_fluid_executed_ticks: usize,
    pub poll_fluid_deferred_ticks: usize,
    pub poll_fluid_mutated_blocks: usize,
    pub poll_scheduled_fluid_ticks: usize,
    pub server_command_queue_depth: usize,
    pub server_update_queue_depth: usize,
    pub server_update_queue_bytes: usize,
    pub server_pending_jobs: usize,
    pub server_pending_publications: usize,
    pub runner_frame_metrics: WorkerFrameMetrics,
    pub worldgen_job_frame_metrics: WorkerFrameMetrics,
    pub light_status_job_frame_metrics: WorkerFrameMetrics,
    pub scheduler_pending_jobs: usize,
    pub scheduler_completed_jobs: usize,
    pub scheduler_dirty_chunks: usize,
    pub scheduler_loaded_snapshot_chunks: usize,
    pub scheduler_client_visible_chunks: usize,
    pub scheduler_active_ticket_chunks: usize,
    pub player_visible_chunks: usize,
    pub player_outbound_queue_depth: usize,
    pub pending_render_chunks_before: usize,
    pub pending_render_chunks_after: usize,
    pub pending_compile_jobs_before: usize,
    pub pending_compile_jobs_after: usize,
    pub max_pending_compile_jobs: usize,
    pub available_compile_slots_before: usize,
    pub available_compile_slots_after: usize,
    pub dispatcher_compile_worker_count: usize,
    pub dispatcher_completed_compile_tasks: usize,
    pub dispatcher_total_compile_worker_busy_ms: f64,
    pub dispatcher_max_compile_worker_task_ms: f64,
    pub rebuilt_section_count: usize,
    pub removed_section_count: usize,
    pub rebuilt_vertex_count: u32,
    pub rebuilt_index_count: u32,
    pub target_rebuilt_section_count: usize,
    pub non_target_rebuilt_section_count: usize,
    pub target_removed_section_count: usize,
    pub non_target_removed_section_count: usize,
    pub section_sync_timing: mclone_app_runtime::RenderSectionSyncTiming,
    pub neighbor_ready_section_count: usize,
    pub near_exception_section_count: usize,
    pub deferred_section_count: usize,
    pub submitted_compile_section_count: usize,
    pub deadline_skipped_compile_request_count: usize,
    pub accepted_compile_result_count: usize,
    pub queued_completed_compile_result_count: usize,
    pub completed_compile_section_count: usize,
    pub stale_compile_section_count: usize,
    pub uploaded_section_count: usize,
    pub upload_removed_section_count: usize,
    pub uploaded_vertex_count: u32,
    pub uploaded_index_count: u32,
    pub queued_upload_section_count: usize,
    pub queued_upload_removed_section_count: usize,
    pub queued_upload_lifecycle_item_count: usize,
    pub queued_upload_mesh_owned_bytes: usize,
    pub upload_phase_event_count: usize,
    pub upload_enqueued_lifecycle_item_count: usize,
    pub upload_superseded_lifecycle_item_count: usize,
    pub upload_drained_lifecycle_item_count: usize,
    pub upload_released_compile_job_count: usize,
    pub upload_released_compile_jobs_on_enqueue: usize,
    pub upload_released_compile_jobs_on_apply: usize,
    pub upload_held_lifecycle_item_count: usize,
    pub upload_held_compile_job_count: usize,
    pub upload_limited: bool,
    pub upload_accept_limited: bool,
    pub upload_backpressured: bool,
    pub traversal_ready_section_count: usize,
    pub record_cache: TexturedSectionRecordCacheStats,
    pub visibility_graph_build_count: usize,
    pub visibility_graph_total_ms: f64,
    pub visibility_graph_worst_ms: f64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum XrTerrainHostMode {
    #[default]
    LocalIntegrated,
    RemoteDedicated,
}

impl XrTerrainHostMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::LocalIntegrated => "local-integrated",
            Self::RemoteDedicated => "remote-dedicated",
        }
    }

    pub const fn server_owned_lanes_are_remote(self) -> bool {
        matches!(self, Self::RemoteDedicated)
    }
}

impl From<SingleViewHostMode> for XrTerrainHostMode {
    fn from(value: SingleViewHostMode) -> Self {
        match value {
            SingleViewHostMode::LocalIntegrated => Self::LocalIntegrated,
            SingleViewHostMode::RemoteDedicated => Self::RemoteDedicated,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct XrTerrainUploadApplyReport {
    pub(super) upload: TexturedSectionUploadReport,
    pub(super) phase: RenderSectionUploadPhaseReport,
    pub(super) release_compile_jobs: usize,
}

impl XrTerrainFrameTiming {
    pub(crate) fn absorb_upload_apply_timing(&mut self, timing: TexturedSectionUploadTiming) {
        self.runtime_upload_apply_dirty_mark_ms += timing.dirty_mark_ms;
        self.runtime_upload_apply_remove_ms += timing.remove_ms;
        self.runtime_upload_apply_section_state_ms += timing.section_state_ms;
        self.runtime_upload_apply_vertex_bytes_ms += timing.vertex_bytes_ms;
        self.runtime_upload_apply_vertex_buffer_ms += timing.vertex_buffer_ms;
        self.runtime_upload_apply_index_bytes_ms += timing.index_bytes_ms;
        self.runtime_upload_apply_index_buffer_ms += timing.index_buffer_ms;
        self.runtime_upload_apply_mesh_insert_ms += timing.mesh_insert_ms;
        self.runtime_upload_apply_mesh_upload_worst_ms = self
            .runtime_upload_apply_mesh_upload_worst_ms
            .max(timing.mesh_upload_worst_ms);
    }
}

impl McloneSceneHost {
    pub fn frame_summary(&self) -> XrTerrainFrameSummary {
        self.frame_summary_with_timing(
            XrTerrainFrameTiming::default(),
            XrTerrainUploadSummary::default(),
        )
    }

    pub(crate) fn frame_summary_with_timing(
        &self,
        timing: XrTerrainFrameTiming,
        upload: XrTerrainUploadSummary,
    ) -> XrTerrainFrameSummary {
        if let Some(summary) = self.first_eye_summary {
            return XrTerrainFrameSummary {
                rendered_frames: self.rendered_frames,
                section_count: summary.section_count,
                drawn_section_count: summary.drawn_section_count,
                index_count: summary.index_count,
                drawn_index_count: summary.drawn_index_count,
                gui_command_count: summary.gui_command_count,
                ui_panel: self.last_ui_panel_stats,
                ui_draw_cache: self.last_ui_draw_cache_stats,
                ui_active: self.ui.is_active(),
                local_startup_active: self.active_world.local_startup.is_some(),
                actor_count: summary.actor_count,
                drawn_actor_count: summary.drawn_actor_count,
                head_comfort: self.head_comfort,
                timing,
                upload,
            };
        }
        XrTerrainFrameSummary {
            rendered_frames: self.rendered_frames,
            section_count: self.active_world.render_stats.section_count,
            drawn_section_count: self.active_world.render_stats.drawn_section_count,
            index_count: self.active_world.render_stats.index_count,
            drawn_index_count: self.active_world.render_stats.drawn_index_count,
            gui_command_count: 0,
            ui_panel: self.last_ui_panel_stats,
            ui_draw_cache: self.last_ui_draw_cache_stats,
            ui_active: self.ui.is_active(),
            local_startup_active: self.active_world.local_startup.is_some(),
            actor_count: self.active_world.render_stats.actor_count,
            drawn_actor_count: self.active_world.render_stats.drawn_actor_count,
            head_comfort: self.head_comfort,
            timing,
            upload,
        }
    }

    pub(crate) fn frame_summary_from_multiview(
        &self,
        summary: XrTerrainMultiviewFrameSummary,
        timing: XrTerrainFrameTiming,
    ) -> XrTerrainFrameSummary {
        XrTerrainFrameSummary {
            rendered_frames: summary.rendered_frames,
            section_count: summary.section_count,
            drawn_section_count: summary.left.drawn_section_count,
            index_count: self.active_world.render_stats.index_count,
            drawn_index_count: summary.left.drawn_index_count,
            gui_command_count: 0,
            ui_panel: summary.ui_panel,
            ui_draw_cache: summary.ui_draw_cache,
            ui_active: self.ui.is_active(),
            local_startup_active: self.active_world.local_startup.is_some(),
            actor_count: summary.actor_count,
            drawn_actor_count: summary.drawn_actor_count,
            head_comfort: self.head_comfort,
            timing,
            upload: summary.upload,
        }
    }

    pub(crate) fn record_eye0_summary(&mut self, summary: FullFrameRenderSummary) {
        self.first_eye_summary = Some(summary);
        self.rendered_frames += 1;
    }
}

pub(crate) fn xr_poll_diagnostics_upload_summary(
    host_mode: XrTerrainHostMode,
    diagnostics: RuntimePollDiagnostics,
) -> XrTerrainUploadSummary {
    XrTerrainUploadSummary {
        host_mode,
        poll_total_ms: diagnostics.poll_total_ms,
        poll_drain_updates_ms: diagnostics.drain_updates_ms,
        poll_producer_read_ms: diagnostics.producer_read_ms,
        poll_producer_decode_ms: diagnostics.producer_decode_ms,
        poll_producer_inbound_frame_sequence: diagnostics.producer_inbound_frame_sequence,
        poll_client_deferred_chunk_drop_ms: diagnostics.client_deferred_chunk_drop_ms,
        poll_client_deferred_chunk_drop_items: diagnostics.client_deferred_chunk_drop_items,
        poll_client_deferred_chunk_drop_backlog_items: diagnostics
            .client_deferred_chunk_drop_backlog_items,
        poll_apply_updates_ms: diagnostics.apply_updates_ms,
        poll_dirty_mark_ms: diagnostics.dirty_mark_ms,
        poll_client_apply_updates_ms: diagnostics.client_apply_updates_ms,
        poll_snapshot_update_apply_ms: diagnostics.snapshot_update_apply_ms,
        poll_snapshot_update_dirty_mark_ms: diagnostics.snapshot_update_dirty_mark_ms,
        poll_snapshot_update_client_apply_ms: diagnostics.snapshot_update_client_apply_ms,
        poll_section_block_update_apply_ms: diagnostics.section_block_update_apply_ms,
        poll_section_block_update_dirty_mark_ms: diagnostics.section_block_update_dirty_mark_ms,
        poll_section_block_update_client_apply_ms: diagnostics.section_block_update_client_apply_ms,
        poll_unload_update_apply_ms: diagnostics.unload_update_apply_ms,
        poll_unload_update_dirty_mark_ms: diagnostics.unload_update_dirty_mark_ms,
        poll_unload_update_client_apply_ms: diagnostics.unload_update_client_apply_ms,
        poll_other_update_apply_ms: diagnostics.other_update_apply_ms,
        poll_other_update_dirty_mark_ms: diagnostics.other_update_dirty_mark_ms,
        poll_other_update_client_apply_ms: diagnostics.other_update_client_apply_ms,
        poll_mixed_update_apply_ms: diagnostics.mixed_update_apply_ms,
        poll_mixed_update_dirty_mark_ms: diagnostics.mixed_update_dirty_mark_ms,
        poll_mixed_update_client_apply_ms: diagnostics.mixed_update_client_apply_ms,
        update_pump_stalled: diagnostics.update_pump_stalled,
        update_pump_stall_count: diagnostics.update_pump_stall_count,
        server_update_applied_bytes: diagnostics.server_update_applied_bytes,
        server_update_oldest_applied_age_ms: diagnostics.server_update_oldest_applied_age_ms,
        poll_diagnostics_ms: diagnostics.poll_diagnostics_ms,
        poll_diagnostics_refreshed: diagnostics.diagnostics_refreshed,
        poll_diagnostics_cache_age_ms: diagnostics.diagnostics_cache_age_ms,
        server_diagnostics_detail_refreshes: diagnostics.server_diagnostics_detail_refreshes,
        server_diagnostics_detail_age_ms: diagnostics.server_diagnostics_detail_age_ms,
        poll_server_tick_ms: diagnostics.server_tick_ms,
        poll_server_reported_total_ms: diagnostics.server_reported_total_ms,
        poll_scheduler_tick_ms: diagnostics.scheduler_tick_ms,
        poll_scheduler_reconcile_holders_ms: diagnostics.scheduler_reconcile_holders_ms,
        poll_scheduler_active_levels_ms: diagnostics.scheduler_active_levels_ms,
        poll_scheduler_holder_updates_ms: diagnostics.scheduler_holder_updates_ms,
        poll_scheduler_runtime_enqueue_ms: diagnostics.scheduler_runtime_enqueue_ms,
        poll_scheduler_active_levels_calls: diagnostics.scheduler_active_levels_calls,
        poll_scheduler_active_levels_cache_hits: diagnostics.scheduler_active_levels_cache_hits,
        poll_scheduler_holder_update_count: diagnostics.scheduler_holder_update_count,
        poll_scheduler_runtime_target_count: diagnostics.scheduler_runtime_target_count,
        poll_scheduler_adaptive_publication_budget_enabled: diagnostics
            .scheduler_adaptive_publication_budget_enabled,
        poll_scheduler_feature_publish_budget_max_units: diagnostics
            .scheduler_feature_publish_budget_max_units,
        poll_scheduler_feature_publish_budget_ms: diagnostics.scheduler_feature_publish_budget_ms,
        poll_scheduler_feature_publish_spent_units: diagnostics
            .scheduler_feature_publish_spent_units,
        poll_scheduler_feature_publish_spent_ms: diagnostics.scheduler_feature_publish_spent_ms,
        poll_scheduler_light_publish_budget_max_units: diagnostics
            .scheduler_light_publish_budget_max_units,
        poll_scheduler_light_publish_budget_ms: diagnostics.scheduler_light_publish_budget_ms,
        poll_scheduler_light_publish_spent_units: diagnostics.scheduler_light_publish_spent_units,
        poll_scheduler_light_publish_spent_ms: diagnostics.scheduler_light_publish_spent_ms,
        poll_scheduler_pending_worldgen_publication_chunk_limit: diagnostics
            .scheduler_pending_worldgen_publication_chunk_limit,
        poll_scheduler_completed_feature_jobs_drained: diagnostics
            .scheduler_completed_feature_jobs_drained,
        poll_scheduler_feature_chunks_published: diagnostics.scheduler_feature_chunks_published,
        poll_scheduler_feature_chunks_skipped: diagnostics.scheduler_feature_chunks_skipped,
        poll_scheduler_feature_jobs_completed: diagnostics.scheduler_feature_jobs_completed,
        poll_scheduler_feature_snapshot_ready_events: diagnostics
            .scheduler_feature_snapshot_ready_events,
        poll_scheduler_light_status_batches_enqueued: diagnostics
            .scheduler_light_status_batches_enqueued,
        poll_scheduler_completed_light_statuses_drained: diagnostics
            .scheduler_completed_light_statuses_drained,
        poll_scheduler_light_statuses_published: diagnostics.scheduler_light_statuses_published,
        poll_scheduler_light_statuses_skipped: diagnostics.scheduler_light_statuses_skipped,
        poll_scheduler_light_snapshot_ready_events: diagnostics
            .scheduler_light_snapshot_ready_events,
        poll_scheduler_cumulative_feature_chunks_published: diagnostics
            .scheduler_cumulative_feature_chunks_published,
        poll_scheduler_cumulative_light_statuses_published: diagnostics
            .scheduler_cumulative_light_statuses_published,
        poll_scheduler_pending_worldgen_publication_jobs: diagnostics
            .scheduler_pending_worldgen_publication_jobs,
        poll_scheduler_pending_worldgen_publication_chunks: diagnostics
            .scheduler_pending_worldgen_publication_chunks,
        poll_scheduler_pending_light_publications: diagnostics.scheduler_pending_light_publications,
        poll_scheduler_worldgen_mailbox_pending_jobs: diagnostics
            .scheduler_worldgen_mailbox_pending_jobs,
        poll_scheduler_light_mailbox_pending_statuses: diagnostics
            .scheduler_light_mailbox_pending_statuses,
        poll_updates: diagnostics.updates,
        poll_snapshot_updates: diagnostics.snapshot_updates,
        poll_section_block_updates: diagnostics.section_block_updates,
        poll_unload_updates: diagnostics.unload_updates,
        poll_other_updates: diagnostics.other_updates,
        poll_mixed_updates: diagnostics.mixed_updates,
        poll_fluid_due_ticks: diagnostics.fluid_due_ticks,
        poll_fluid_executed_ticks: diagnostics.fluid_executed_ticks,
        poll_fluid_deferred_ticks: diagnostics.fluid_deferred_ticks,
        poll_fluid_mutated_blocks: diagnostics.fluid_mutated_blocks,
        poll_scheduled_fluid_ticks: diagnostics.scheduled_fluid_ticks,
        server_command_queue_depth: diagnostics.server_command_queue_depth,
        server_update_queue_depth: diagnostics.server_update_queue_depth,
        server_update_queue_bytes: diagnostics.server_update_queue_bytes,
        server_pending_jobs: diagnostics.server_pending_jobs,
        server_pending_publications: diagnostics.server_pending_publications,
        runner_frame_metrics: diagnostics.runner_frame_metrics,
        worldgen_job_frame_metrics: diagnostics.worldgen_job_frame_metrics,
        light_status_job_frame_metrics: diagnostics.light_status_job_frame_metrics,
        scheduler_pending_jobs: diagnostics.scheduler_pending_jobs,
        scheduler_completed_jobs: diagnostics.scheduler_completed_jobs,
        scheduler_dirty_chunks: diagnostics.scheduler_dirty_chunks,
        scheduler_loaded_snapshot_chunks: diagnostics.scheduler_loaded_snapshot_chunks,
        scheduler_client_visible_chunks: diagnostics.scheduler_client_visible_chunks,
        scheduler_active_ticket_chunks: diagnostics.scheduler_active_ticket_chunks,
        player_visible_chunks: diagnostics.player_visible_chunks,
        player_outbound_queue_depth: diagnostics.player_outbound_queue_depth,
        ..XrTerrainUploadSummary::default()
    }
}

pub(crate) fn accumulate_upload_report(
    total: &mut TexturedSectionUploadReport,
    report: TexturedSectionUploadReport,
) {
    total.uploaded_section_count += report.uploaded_section_count;
    total.removed_section_count += report.removed_section_count;
    total.uploaded_vertex_count += report.uploaded_vertex_count;
    total.uploaded_index_count += report.uploaded_index_count;
}
