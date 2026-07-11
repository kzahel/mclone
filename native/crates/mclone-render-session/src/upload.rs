use super::*;

#[derive(Clone, Debug, Default)]
pub struct RenderSectionUploadDrain {
    pub rebuilt_sections: Vec<TexturedRenderSectionMesh>,
    pub removed_section_keys: BTreeSet<RenderSectionKey>,
    pub lifecycle_item_count: usize,
    pub phase_report: RenderSectionUploadPhaseReport,
}

impl RenderSectionUploadDrain {
    pub fn is_empty(&self) -> bool {
        self.rebuilt_sections.is_empty() && self.removed_section_keys.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionUploadQueueStats {
    pub queued_upload_sections: usize,
    pub queued_removed_sections: usize,
    pub queued_lifecycle_items: usize,
    pub queued_upload_mesh_owned_bytes: usize,
    pub held_release_batches: usize,
    pub held_release_lifecycle_items: usize,
    pub held_compile_jobs: usize,
}

pub type RenderSectionUploadFramePolicy = ResidentTileUploadFramePolicy;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderSectionUploadFrameLimit {
    #[default]
    None,
    UploadBacklog,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionUploadFrameDecision {
    pub policy: RenderSectionUploadFramePolicy,
    pub runtime_work_requested: bool,
    pub drained_before_sync: bool,
    pub queue_after_drain: RenderSectionUploadQueueStats,
    pub upload_backpressured: bool,
    pub should_sync_render_sections: bool,
    pub should_apply_section_update_after_sync: bool,
    pub limit: RenderSectionUploadFrameLimit,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionUploadPhaseReport {
    pub phase_event_count: usize,
    pub accepted_compile_jobs: usize,
    pub queued_lifecycle_items: usize,
    pub superseded_lifecycle_items: usize,
    pub drained_upload_sections: usize,
    pub drained_removed_sections: usize,
    pub drained_lifecycle_items: usize,
    pub released_compile_jobs: usize,
    pub released_compile_jobs_on_enqueue: usize,
    pub released_compile_jobs_on_apply: usize,
    pub queued_upload_sections_before: usize,
    pub queued_removed_sections_before: usize,
    pub queued_lifecycle_items_before: usize,
    pub queued_upload_sections_after: usize,
    pub queued_removed_sections_after: usize,
    pub queued_lifecycle_items_after: usize,
    pub held_release_lifecycle_items: usize,
    pub held_compile_jobs: usize,
    pub upload_limited: bool,
    pub accept_limited: bool,
}

impl RenderSectionUploadPhaseReport {
    pub fn direct(
        accepted_compile_jobs: usize,
        uploaded_sections: usize,
        removed_sections: usize,
        released_compile_jobs: usize,
    ) -> Self {
        Self {
            phase_event_count: 1,
            accepted_compile_jobs,
            drained_upload_sections: uploaded_sections,
            drained_removed_sections: removed_sections,
            drained_lifecycle_items: uploaded_sections + removed_sections,
            released_compile_jobs,
            released_compile_jobs_on_apply: released_compile_jobs,
            ..Self::default()
        }
    }

    pub fn absorb(&mut self, other: Self) {
        if other.phase_event_count == 0 {
            return;
        }
        if self.phase_event_count == 0 {
            *self = other;
            return;
        }
        self.phase_event_count += other.phase_event_count;
        self.accepted_compile_jobs += other.accepted_compile_jobs;
        self.queued_lifecycle_items += other.queued_lifecycle_items;
        self.superseded_lifecycle_items += other.superseded_lifecycle_items;
        self.drained_upload_sections += other.drained_upload_sections;
        self.drained_removed_sections += other.drained_removed_sections;
        self.drained_lifecycle_items += other.drained_lifecycle_items;
        self.released_compile_jobs += other.released_compile_jobs;
        self.released_compile_jobs_on_enqueue += other.released_compile_jobs_on_enqueue;
        self.released_compile_jobs_on_apply += other.released_compile_jobs_on_apply;
        self.queued_upload_sections_after = other.queued_upload_sections_after;
        self.queued_removed_sections_after = other.queued_removed_sections_after;
        self.queued_lifecycle_items_after = other.queued_lifecycle_items_after;
        self.held_release_lifecycle_items = other.held_release_lifecycle_items;
        self.held_compile_jobs = other.held_compile_jobs;
        self.upload_limited |= other.upload_limited;
        self.accept_limited |= other.accept_limited;
    }

    pub fn record_applied_release(
        &mut self,
        released_compile_jobs: usize,
        queue_stats: RenderSectionUploadQueueStats,
    ) {
        self.released_compile_jobs += released_compile_jobs;
        self.released_compile_jobs_on_apply += released_compile_jobs;
        self.apply_queue_after(queue_stats);
    }

    fn from_queue_before(queue_stats: RenderSectionUploadQueueStats) -> Self {
        Self {
            phase_event_count: 1,
            queued_upload_sections_before: queue_stats.queued_upload_sections,
            queued_removed_sections_before: queue_stats.queued_removed_sections,
            queued_lifecycle_items_before: queue_stats.queued_lifecycle_items,
            queued_upload_sections_after: queue_stats.queued_upload_sections,
            queued_removed_sections_after: queue_stats.queued_removed_sections,
            queued_lifecycle_items_after: queue_stats.queued_lifecycle_items,
            held_release_lifecycle_items: queue_stats.held_release_lifecycle_items,
            held_compile_jobs: queue_stats.held_compile_jobs,
            ..Self::default()
        }
    }

    fn apply_queue_after(&mut self, queue_stats: RenderSectionUploadQueueStats) {
        self.queued_upload_sections_after = queue_stats.queued_upload_sections;
        self.queued_removed_sections_after = queue_stats.queued_removed_sections;
        self.queued_lifecycle_items_after = queue_stats.queued_lifecycle_items;
        self.held_release_lifecycle_items = queue_stats.held_release_lifecycle_items;
        self.held_compile_jobs = queue_stats.held_compile_jobs;
    }
}

#[derive(Clone, Debug, Default)]
pub struct RenderSectionUploadCoordinator {
    tiles: ResidentTileUploadCoordinator<RenderSectionKey, TexturedRenderSectionMesh>,
}

impl RenderSectionUploadCoordinator {
    pub fn clear(&mut self) {
        self.tiles.clear();
    }

    pub fn has_pending_work(&self) -> bool {
        self.tiles.has_pending_work()
    }

    pub fn queued_upload_section_count(&self) -> usize {
        self.tiles.queued_upload_count()
    }

    pub fn queued_removed_section_count(&self) -> usize {
        self.tiles.queued_removal_count()
    }

    pub fn should_drain_before_runtime_sync(&self, policy: RenderSectionUploadFramePolicy) -> bool {
        policy.is_budgeted() && self.has_pending_work()
    }

    pub fn frame_decision_after_pre_sync_drain(
        &self,
        policy: RenderSectionUploadFramePolicy,
        runtime_work_requested: bool,
        drained_before_sync: bool,
    ) -> RenderSectionUploadFrameDecision {
        let queue_after_drain = self.stats();
        let upload_backpressured =
            policy.is_budgeted() && queue_after_drain.queued_lifecycle_items > 0;
        RenderSectionUploadFrameDecision {
            policy,
            runtime_work_requested,
            drained_before_sync,
            queue_after_drain,
            upload_backpressured,
            should_sync_render_sections: runtime_work_requested && !upload_backpressured,
            should_apply_section_update_after_sync: (runtime_work_requested
                && !upload_backpressured)
                || !drained_before_sync,
            limit: if upload_backpressured {
                RenderSectionUploadFrameLimit::UploadBacklog
            } else {
                RenderSectionUploadFrameLimit::None
            },
        }
    }

    pub fn stats(&self) -> RenderSectionUploadQueueStats {
        let stats = self.tiles.stats();
        RenderSectionUploadQueueStats {
            queued_upload_sections: stats.queued_uploads,
            queued_removed_sections: stats.queued_removals,
            queued_lifecycle_items: stats.queued_lifecycle_items,
            queued_upload_mesh_owned_bytes: stats.queued_upload_owned_bytes,
            held_release_batches: stats.held_release_batches,
            held_release_lifecycle_items: stats.held_release_lifecycle_items,
            held_compile_jobs: stats.held_producer_jobs,
        }
    }

    pub fn direct_release_count(section_update: &RenderSectionCacheUpdate) -> usize {
        section_update.accepted_compile_result_count
    }

    pub fn enqueue_cache_update(
        &mut self,
        section_update: RenderSectionCacheUpdate,
    ) -> RenderSectionUploadPhaseReport {
        let queue_before = self.stats();
        let accepted_compile_result_count = section_update.accepted_compile_result_count;
        let enqueue_report = self.tiles.enqueue(
            section_update.rebuilt_sections,
            section_update.removed_section_keys,
            accepted_compile_result_count,
        );
        let release_compile_jobs = enqueue_report.released_producer_jobs;
        if accepted_compile_result_count == 0
            && enqueue_report.queued_lifecycle_items == 0
            && enqueue_report.superseded_lifecycle_items == 0
            && release_compile_jobs == 0
        {
            return RenderSectionUploadPhaseReport::default();
        }
        let mut report = RenderSectionUploadPhaseReport::from_queue_before(queue_before);
        report.accepted_compile_jobs = accepted_compile_result_count;
        report.queued_lifecycle_items = enqueue_report.queued_lifecycle_items;
        report.superseded_lifecycle_items = enqueue_report.superseded_lifecycle_items;
        report.released_compile_jobs = release_compile_jobs;
        report.released_compile_jobs_on_enqueue = release_compile_jobs;
        report.apply_queue_after(self.stats());
        report
    }

    pub fn drain_budgeted(
        &mut self,
        upload_budget: Option<usize>,
        accept_budget: Option<usize>,
    ) -> RenderSectionUploadDrain {
        let queue_before = self.stats();
        let drain = self.tiles.drain_budgeted(upload_budget, accept_budget);
        let rebuilt_sections = drain.uploads;
        let removed_section_keys = drain.removed_tile_keys;
        let lifecycle_item_count = drain.lifecycle_item_count;
        let queue_after = self.stats();
        let upload_limited = drain.upload_limited;
        let accept_limited = drain.accept_limited;
        let mut phase_report = RenderSectionUploadPhaseReport::from_queue_before(queue_before);
        phase_report.drained_upload_sections = rebuilt_sections.len();
        phase_report.drained_removed_sections = removed_section_keys.len();
        phase_report.drained_lifecycle_items = lifecycle_item_count;
        phase_report.upload_limited = upload_limited;
        phase_report.accept_limited = accept_limited;
        phase_report.apply_queue_after(queue_after);
        if lifecycle_item_count == 0
            && queue_before.queued_lifecycle_items == 0
            && !upload_limited
            && !accept_limited
        {
            phase_report = RenderSectionUploadPhaseReport::default();
        }
        RenderSectionUploadDrain {
            rebuilt_sections,
            removed_section_keys,
            lifecycle_item_count,
            phase_report,
        }
    }

    pub fn complete_applied_lifecycle_items(&mut self, lifecycle_items: usize) -> usize {
        self.tiles.complete_applied_lifecycle_items(lifecycle_items)
    }
}
