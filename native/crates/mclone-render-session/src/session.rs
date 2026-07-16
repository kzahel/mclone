use super::*;
use crate::server_updates::EngineServerUpdateDirtyBatch;

#[derive(Clone, Debug, Default)]
pub struct RenderSectionSession {
    cache: CachedTexturedRenderSections,
    dirty: RenderSectionDirtyState,
}

#[derive(Debug)]
pub struct EngineRenderSession {
    client: ClientRuntime,
    render_session: RenderSectionSession,
    pending_completed_compile_results: VecDeque<RenderSectionCompileResult>,
}

impl EngineRenderSession {
    pub fn new(client: ClientRuntime) -> Self {
        Self {
            client,
            render_session: RenderSectionSession::default(),
            pending_completed_compile_results: VecDeque::new(),
        }
    }

    pub const fn client(&self) -> &ClientRuntime {
        &self.client
    }

    pub const fn client_mut(&mut self) -> &mut ClientRuntime {
        &mut self.client
    }

    pub const fn render_session(&self) -> &RenderSectionSession {
        &self.render_session
    }

    pub const fn render_session_mut(&mut self) -> &mut RenderSectionSession {
        &mut self.render_session
    }

    pub fn pending_completed_compile_result_count(&self) -> usize {
        self.pending_completed_compile_results.len()
    }

    /// Install metadata for a fully prepared asset epoch and discard every
    /// pending result/in-flight marker owned by the retired compiler instance.
    pub fn replace_asset_epoch_sections(
        &mut self,
        report: TexturedRenderSectionBuildReport,
    ) -> RenderSectionCacheUpdate {
        self.pending_completed_compile_results.clear();
        self.render_session.replace_asset_epoch_sections(report)
    }

    pub fn mark_chunk_neighborhood_dirty(&mut self, pos: ChunkPos) -> usize {
        let client = &self.client;
        self.render_session
            .mark_chunk_neighborhood_dirty_with_loaded_sections(pos, |dirty_pos| {
                client
                    .chunk_snapshot(dirty_pos)
                    .map(render_section_keys_for_snapshot)
                    .unwrap_or_default()
            })
    }

    pub fn mark_section_dirty(&mut self, key: RenderSectionKey) {
        self.render_session.mark_section_dirty(key);
    }

    pub fn mark_server_update_render_dirty(
        &mut self,
        updates: &[ServerUpdate],
    ) -> EngineServerUpdateReport {
        self.mark_server_update_render_dirty_with_policy(
            updates,
            EngineServerUpdateDirtyPolicy::ALL,
        )
    }

    pub fn mark_server_update_render_dirty_with_policy(
        &mut self,
        updates: &[ServerUpdate],
        policy: EngineServerUpdateDirtyPolicy,
    ) -> EngineServerUpdateReport {
        let report = EngineServerUpdateReport::classify(updates);
        let dirty_batch = EngineServerUpdateDirtyBatch::collect(updates, policy);
        let client = &self.client;
        self.render_session
            .mark_chunks_dirty_with_loaded_sections_and_forced(
                dirty_batch.dirty_chunks,
                dirty_batch.force_dirty_chunks,
                |pos| {
                    client
                        .chunk_snapshot(pos)
                        .map(render_section_keys_for_snapshot)
                        .unwrap_or_default()
                },
            );
        for key in dirty_batch.dirty_sections {
            self.mark_section_dirty(key);
        }
        report
    }

    pub fn apply_server_updates(&mut self, updates: Vec<ServerUpdate>) -> EngineServerUpdateReport {
        let stale_chunks = self.dimension_change_stale_chunks(&updates);
        let report = self.mark_server_update_render_dirty(&updates);
        for pos in stale_chunks {
            self.mark_chunk_neighborhood_dirty(pos);
        }
        self.client.apply_updates(updates);
        report
    }

    pub fn apply_server_updates_with_dirty_policy(
        &mut self,
        updates: Vec<ServerUpdate>,
        policy: EngineServerUpdateDirtyPolicy,
    ) -> EngineServerUpdateReport {
        let stale_chunks = self.dimension_change_stale_chunks(&updates);
        let report = self.mark_server_update_render_dirty_with_policy(&updates, policy);
        for pos in stale_chunks {
            self.mark_chunk_neighborhood_dirty(pos);
        }
        self.client.apply_updates(updates);
        report
    }

    fn dimension_change_stale_chunks(&self, updates: &[ServerUpdate]) -> Vec<ChunkPos> {
        updates
            .iter()
            .any(|update| matches!(update, ServerUpdate::DimensionChange { .. }))
            .then(|| self.client.loaded_chunk_positions().collect())
            .unwrap_or_default()
    }

    pub fn clear_client_replica_and_mark_render_dirty(&mut self) -> Vec<ChunkPos> {
        let stale_chunks = self.client.loaded_chunk_positions().collect::<Vec<_>>();
        self.client.clear_server_replica();
        for pos in stale_chunks.iter().copied() {
            self.mark_chunk_neighborhood_dirty(pos);
        }
        stale_chunks
    }

    pub fn mark_loaded_chunks_dirty_when_cache_empty(&mut self) -> bool {
        if !self.render_session.cache_is_empty()
            || self.client.loaded_chunk_count() == 0
            || !self.render_session.dirty_is_empty()
        {
            return false;
        }
        let loaded = self
            .client
            .chunk_snapshots()
            .map(|snapshot| snapshot.pos)
            .collect::<Vec<_>>();
        for pos in loaded {
            self.mark_chunk_neighborhood_dirty(pos);
        }
        true
    }

    pub fn apply_loaded_view_sync<I>(
        &mut self,
        previous_chunks: &BTreeSet<ChunkPos>,
        current_loaded_chunks: BTreeSet<ChunkPos>,
        loaded_section_keys_for_chunk: impl FnMut(ChunkPos) -> I,
    ) -> RenderSectionViewSync
    where
        I: IntoIterator<Item = RenderSectionKey>,
    {
        self.render_session.apply_loaded_view_sync(
            previous_chunks,
            current_loaded_chunks,
            loaded_section_keys_for_chunk,
        )
    }

    pub fn apply_current_loaded_view_sync(
        &mut self,
        previous_chunks: &BTreeSet<ChunkPos>,
    ) -> RenderSectionViewSync {
        let current_loaded_chunks = self
            .client
            .loaded_chunk_positions()
            .collect::<BTreeSet<_>>();
        let client = &self.client;
        self.render_session
            .apply_loaded_view_sync(previous_chunks, current_loaded_chunks, |pos| {
                client
                    .chunk_snapshot(pos)
                    .map(render_section_keys_for_snapshot)
                    .unwrap_or_default()
            })
    }

    pub fn drain_completed_compile_updates(
        &mut self,
        completed_results: impl IntoIterator<Item = RenderSectionCompileResult>,
        request_id_for_result: impl FnMut(&RenderSectionCompileResult) -> u32,
        pending_compile_jobs: usize,
    ) -> Result<RenderSectionCacheUpdate> {
        self.drain_completed_compile_updates_with_acceptance_budget(
            completed_results,
            request_id_for_result,
            pending_compile_jobs,
            None,
        )
    }

    pub fn drain_completed_compile_updates_with_acceptance_budget(
        &mut self,
        completed_results: impl IntoIterator<Item = RenderSectionCompileResult>,
        mut request_id_for_result: impl FnMut(&RenderSectionCompileResult) -> u32,
        pending_compile_jobs: usize,
        completed_result_accept_budget: Option<usize>,
    ) -> Result<RenderSectionCacheUpdate> {
        self.pending_completed_compile_results
            .extend(completed_results);
        let accept_count = completed_result_accept_budget
            .unwrap_or(usize::MAX)
            .min(self.pending_completed_compile_results.len());
        let client = &self.client;
        let mut update = RenderSectionCacheUpdate::default();
        for _ in 0..accept_count {
            let Some(completed) = self.pending_completed_compile_results.pop_front() else {
                break;
            };
            let request_id = request_id_for_result(&completed);
            let finished = self.render_session.finish_compile_update(
                completed,
                request_id,
                &BTreeSet::new(),
                &BTreeSet::new(),
                |key| {
                    let pos = render_section_chunk_pos(key);
                    client
                        .chunk_snapshot(pos)
                        .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
                },
            )?;
            update.accepted_compile_result_count += 1;
            update.merge(finished.cache_update);
        }
        update.pending_compile_jobs = pending_compile_jobs;
        update.queued_completed_compile_result_count = self.pending_completed_compile_results.len();
        Ok(update)
    }

    pub fn prepare_sync_update(
        &mut self,
        order_loaded_dirty_chunks: impl FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        order_dirty_section_chunks: impl FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        chunk_budget: usize,
        section_readiness: impl FnMut(
            &ClientRuntime,
            RenderSectionKey,
        ) -> RenderSectionNeighborReadiness,
        removal_mode: RenderSectionRemovalMode,
    ) -> RenderSectionSyncUpdate {
        let client = &self.client;
        self.render_session.prepare_sync_update(
            |pos| client.chunk_snapshot(pos).is_some(),
            |key| {
                let pos = render_section_chunk_pos(key);
                client
                    .chunk_snapshot(pos)
                    .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
            },
            order_loaded_dirty_chunks,
            order_dirty_section_chunks,
            chunk_budget,
            |pos| {
                client
                    .chunk_snapshot(pos)
                    .map(render_section_keys_for_snapshot)
                    .unwrap_or_default()
            },
            {
                let mut section_readiness = section_readiness;
                move |key| section_readiness(client, key)
            },
            removal_mode,
        )
    }

    pub fn submit_prepared_sync_plan<T, E>(
        &mut self,
        sync_plan: &RenderSectionSyncPlan,
        snapshots: Vec<ChunkSnapshot>,
        submit: impl FnOnce(
            &RenderSectionSyncPlan,
            RenderSectionCompileRequest,
        ) -> std::result::Result<T, E>,
    ) -> std::result::Result<RenderSectionReadyWorkSubmission<T>, E> {
        self.render_session
            .submit_prepared_sync_plan(sync_plan, snapshots, submit)
    }

    pub fn finish_compile_update(
        &mut self,
        completed: RenderSectionCompileResult,
        request_id: u32,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_sections: &BTreeSet<RenderSectionKey>,
    ) -> Result<RenderSectionFinishedCompileUpdate> {
        let client = &self.client;
        self.render_session.finish_compile_update(
            completed,
            request_id,
            removal_chunks,
            removal_sections,
            |key| {
                let pos = render_section_chunk_pos(key);
                client
                    .chunk_snapshot(pos)
                    .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
            },
        )
    }

    /// Drive one budgeted increment of the shared render-section streaming loop over
    /// `compiler` (067 Stage 3). This is the single compile loop both platforms run:
    ///
    /// 1. drain the compiler's completed jobs and merge them into the resident cache,
    /// 2. seed dirty work when the cache is empty (first frame after a view loads),
    /// 3. plan a `chunk_budget`-bounded ready increment using the caller's ordering,
    ///    readiness, and removal-mode policy, then
    /// 4. submit that increment through `compiler` — but only when the compiler reports
    ///    free capacity, matching Java's buffer-pack backpressure shape.
    ///
    /// Desktop currently exposes one compile slot over an OS-thread `mpsc` worker; web
    /// exposes one slot over the resident `SharedArrayBuffer` ring. The capacity check is
    /// deliberately part of the shared [`RenderSectionCompiler`] trait so native workers can
    /// grow toward a Java-style bounded buffer-pack pool without making callers guess at
    /// pending-job policy. With budget 1 each submitted job is tiny, so the current cache
    /// stays visible and movement fills progressively instead of stalling on a whole-view
    /// mega-job. The transport (move vs. arena) lives behind the compiler trait; this loop is
    /// identical on both platforms.
    pub fn sync_render_sections_with_budget<C, OrderChunks, OrderSections, Ready, Snapshots>(
        &mut self,
        compiler: &mut C,
        chunk_budget: usize,
        order_loaded_dirty_chunks: OrderChunks,
        order_dirty_section_chunks: OrderSections,
        section_readiness: Ready,
        removal_mode: RenderSectionRemovalMode,
        snapshots_for_submit: Snapshots,
    ) -> Result<RenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
        OrderChunks: FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        OrderSections: FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        Ready: FnMut(&ClientRuntime, RenderSectionKey) -> RenderSectionNeighborReadiness,
        // The snapshot step receives the compiler as well as the client so a platform that
        // keeps a worker-side snapshot mirror (web) can diff the live snapshots against its
        // own shadow and return only the changed columns, instead of deep-cloning every
        // loaded column each frame. Desktop ignores the compiler and clones all columns (the
        // owned `Vec` is moved over `mpsc` to the worker thread, so the clone is load-bearing).
        Snapshots: FnOnce(&ClientRuntime, &mut C) -> Vec<ChunkSnapshot>,
    {
        self.sync_render_sections_with_budget_and_completed_result_acceptance(
            compiler,
            chunk_budget,
            order_loaded_dirty_chunks,
            order_dirty_section_chunks,
            section_readiness,
            removal_mode,
            None,
            snapshots_for_submit,
        )
    }

    pub fn sync_render_sections_with_budget_and_completed_result_acceptance<
        C,
        OrderChunks,
        OrderSections,
        Ready,
        Snapshots,
    >(
        &mut self,
        compiler: &mut C,
        chunk_budget: usize,
        order_loaded_dirty_chunks: OrderChunks,
        order_dirty_section_chunks: OrderSections,
        section_readiness: Ready,
        removal_mode: RenderSectionRemovalMode,
        completed_result_accept_budget: Option<usize>,
        snapshots_for_submit: Snapshots,
    ) -> Result<RenderSectionCacheUpdate>
    where
        C: RenderSectionCompiler,
        OrderChunks: FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        OrderSections: FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        Ready: FnMut(&ClientRuntime, RenderSectionKey) -> RenderSectionNeighborReadiness,
        Snapshots: FnOnce(&ClientRuntime, &mut C) -> Vec<ChunkSnapshot>,
    {
        let completed_results = compiler.try_recv_completed()?;
        let pending_compile_jobs = compiler.pending_job_count();
        let mut report = self.drain_completed_compile_updates_with_acceptance_budget(
            completed_results,
            |_| 0,
            pending_compile_jobs,
            completed_result_accept_budget,
        )?;

        if report.queued_completed_compile_result_count > 0 {
            report.pending_compile_jobs = compiler.pending_job_count();
            return Ok(report);
        }

        self.mark_loaded_chunks_dirty_when_cache_empty();

        if self.render_session().dirty_work_is_empty() {
            report.pending_compile_jobs = compiler.pending_job_count();
            return Ok(report);
        }

        let sync_update = self.prepare_sync_update(
            order_loaded_dirty_chunks,
            order_dirty_section_chunks,
            chunk_budget,
            section_readiness,
            removal_mode,
        );
        report.merge(sync_update.cache_update);

        if chunk_budget == 0 || !compiler.has_pending_job_capacity() {
            report.pending_compile_jobs = compiler.pending_job_count();
            return Ok(report);
        }

        let sync_plan = sync_update.sync_plan;
        let snapshots = snapshots_for_submit(self.client(), compiler);
        let submission_update =
            self.submit_prepared_sync_plan(&sync_plan, snapshots, |_sync_plan, request| {
                compiler.submit(request)
            })?;
        report.merge(submission_update.cache_update);
        report.pending_compile_jobs = compiler.pending_job_count();
        Ok(report)
    }

    /// True when there is render work the streaming loop could still make progress on:
    /// a job in flight, or dirty work whose sections are ready to compile. Drives the
    /// "pump to idle" loops (desktop `sync_all_render_sections`, the web deterministic
    /// smoke) without re-deriving readiness in the caller.
    pub fn has_pending_render_work(
        &self,
        compiler_pending_job_count: usize,
        mut section_readiness: impl FnMut(
            &ClientRuntime,
            RenderSectionKey,
        ) -> RenderSectionNeighborReadiness,
    ) -> bool {
        if compiler_pending_job_count > 0 {
            return true;
        }
        if !self.pending_completed_compile_results.is_empty() {
            return true;
        }
        let client = &self.client;
        self.render_session
            .has_ready_pending_dirty_work(client, |key| section_readiness(client, key))
    }

    pub fn section_metadata(&self) -> Vec<TexturedRenderSectionMetadata> {
        self.render_session.section_metadata()
    }

    pub fn cached_section_count(&self) -> usize {
        self.render_session.cached_section_count()
    }

    pub fn mark_all_sections_dirty_for_resource_rebuild(&mut self) -> usize {
        self.render_session
            .mark_all_sections_dirty_for_resource_rebuild()
    }
}

impl RenderSectionSession {
    pub fn replace_asset_epoch_sections(
        &mut self,
        report: TexturedRenderSectionBuildReport,
    ) -> RenderSectionCacheUpdate {
        let ready = report
            .sections
            .iter()
            .map(|section| section.key)
            .collect::<BTreeSet<_>>();
        self.cache = CachedTexturedRenderSections::default();
        self.dirty = RenderSectionDirtyState::default();
        self.cache
            .apply_build_report(&ready, report, &BTreeSet::new(), &BTreeSet::new())
    }

    pub fn cache_is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn dirty_is_empty(&self) -> bool {
        self.dirty.is_empty() && !self.cache.has_dirty_sections()
    }

    pub fn dirty_work_is_empty(&self) -> bool {
        self.dirty.dirty_work_is_empty() && !self.cache.has_dirty_sections()
    }

    pub fn dirty(&self) -> &RenderSectionDirtyState {
        &self.dirty
    }

    pub fn dirty_mut(&mut self) -> &mut RenderSectionDirtyState {
        &mut self.dirty
    }

    pub fn resident_dirty_section_count(&self) -> usize {
        self.cache.dirty_section_count()
    }

    pub fn resident_dirty_section_keys(&self) -> impl Iterator<Item = RenderSectionKey> + '_ {
        self.cache.dirty_section_keys()
    }

    pub fn section_cache_generation(&self) -> u64 {
        self.cache.generation()
    }

    /// Whether any dirty chunk/section is ready to make compile progress this frame —
    /// either a cached chunk/section that is no longer loaded (removal work) or a loaded,
    /// not-in-flight section whose neighbors are ready. Mirrors the planning gate so the
    /// "pump to idle" loops can stop exactly when the streaming loop would idle.
    pub fn has_ready_pending_dirty_work(
        &self,
        client: &ClientRuntime,
        mut section_readiness: impl FnMut(RenderSectionKey) -> RenderSectionNeighborReadiness,
    ) -> bool {
        let ready_chunk = self.dirty.dirty_chunks.iter().copied().any(|pos| {
            if self.contains_chunk(pos) && client.chunk_snapshot(pos).is_none() {
                return true;
            }
            let Some(snapshot) = client.chunk_snapshot(pos) else {
                return false;
            };
            render_section_keys_for_snapshot(snapshot)
                .into_iter()
                .any(|key| {
                    !self.dirty.inflight_sections.contains(&key)
                        && section_readiness(key).is_ready()
                })
        });
        if ready_chunk {
            return true;
        }
        self.dirty.dirty_sections.iter().copied().any(|key| {
            if self.dirty.inflight_sections.contains(&key) {
                return false;
            }
            let pos = render_section_chunk_pos(key);
            if self.contains_section(key) && client.chunk_snapshot(pos).is_none() {
                return true;
            }
            client
                .chunk_snapshot(pos)
                .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
                && section_readiness(key).is_ready()
        }) || self.cache.dirty_section_keys().any(|key| {
            if self.dirty.inflight_sections.contains(&key) {
                return false;
            }
            let pos = render_section_chunk_pos(key);
            if self.contains_section(key) && client.chunk_snapshot(pos).is_none() {
                return true;
            }
            client
                .chunk_snapshot(pos)
                .is_some_and(|snapshot| snapshot_contains_render_section(snapshot, key))
                && section_readiness(key).is_ready()
        })
    }

    pub fn mark_chunk_dirty(
        &mut self,
        pos: ChunkPos,
        known_section_keys: impl IntoIterator<Item = RenderSectionKey>,
    ) {
        self.dirty.mark_chunk_dirty(pos, known_section_keys);
    }

    pub fn mark_chunk_dirty_with_loaded_sections(
        &mut self,
        pos: ChunkPos,
        loaded_section_keys: impl IntoIterator<Item = RenderSectionKey>,
    ) {
        self.mark_chunk_dirty_with_loaded_sections_and_unknown_policy(
            pos,
            loaded_section_keys,
            false,
        );
    }

    fn mark_chunk_dirty_with_loaded_sections_and_unknown_policy(
        &mut self,
        pos: ChunkPos,
        loaded_section_keys: impl IntoIterator<Item = RenderSectionKey>,
        force_dirty_if_unknown: bool,
    ) {
        let loaded_section_keys = loaded_section_keys.into_iter().collect::<BTreeSet<_>>();
        let resident_section_keys = self
            .cache
            .section_keys_for_chunk(pos)
            .collect::<BTreeSet<_>>();
        let known_section_keys =
            self.known_section_keys_for_chunk(pos, loaded_section_keys.iter().copied());
        self.dirty
            .bump_section_revisions(known_section_keys.iter().copied());

        if !resident_section_keys.is_empty()
            && !loaded_section_keys.is_empty()
            && resident_section_keys == loaded_section_keys
        {
            for key in resident_section_keys {
                self.cache.mark_section_dirty(key);
            }
            return;
        }

        if loaded_section_keys.is_empty() && resident_section_keys.is_empty() {
            if known_section_keys.is_empty() && !force_dirty_if_unknown {
                return;
            }
        }

        for key in resident_section_keys {
            self.cache.mark_section_dirty(key);
        }
        self.dirty.dirty_chunks.insert(pos);
    }

    pub fn mark_chunks_dirty_with_loaded_sections<I>(
        &mut self,
        chunks: impl IntoIterator<Item = ChunkPos>,
        mut loaded_section_keys_for_chunk: impl FnMut(ChunkPos) -> I,
    ) -> usize
    where
        I: IntoIterator<Item = RenderSectionKey>,
    {
        let chunks = chunks.into_iter().collect::<BTreeSet<_>>();
        let marked_chunk_count = chunks.len();
        for pos in chunks {
            self.mark_chunk_dirty_with_loaded_sections(pos, loaded_section_keys_for_chunk(pos));
        }
        marked_chunk_count
    }

    pub fn mark_chunks_dirty_with_loaded_sections_and_forced<I>(
        &mut self,
        chunks: impl IntoIterator<Item = ChunkPos>,
        force_dirty_chunks: impl IntoIterator<Item = ChunkPos>,
        mut loaded_section_keys_for_chunk: impl FnMut(ChunkPos) -> I,
    ) -> usize
    where
        I: IntoIterator<Item = RenderSectionKey>,
    {
        let force_dirty_chunks = force_dirty_chunks.into_iter().collect::<BTreeSet<_>>();
        let chunks = chunks.into_iter().collect::<BTreeSet<_>>();
        let marked_chunk_count = chunks.len();
        for pos in chunks {
            self.mark_chunk_dirty_with_loaded_sections_and_unknown_policy(
                pos,
                loaded_section_keys_for_chunk(pos),
                force_dirty_chunks.contains(&pos),
            );
        }
        marked_chunk_count
    }

    pub fn mark_chunk_neighborhood_dirty_with_loaded_sections<I>(
        &mut self,
        pos: ChunkPos,
        loaded_section_keys_for_chunk: impl FnMut(ChunkPos) -> I,
    ) -> usize
    where
        I: IntoIterator<Item = RenderSectionKey>,
    {
        self.mark_chunks_dirty_with_loaded_sections(
            render_dirty_chunk_neighborhood(pos),
            loaded_section_keys_for_chunk,
        )
    }

    pub fn mark_view_sync_dirty_with_loaded_sections<I>(
        &mut self,
        sync: &RenderSectionViewSync,
        loaded_section_keys_for_chunk: impl FnMut(ChunkPos) -> I,
    ) -> usize
    where
        I: IntoIterator<Item = RenderSectionKey>,
    {
        self.mark_chunks_dirty_with_loaded_sections(
            sync.dirty_chunks
                .iter()
                .chain(sync.removal_chunks.iter())
                .copied(),
            loaded_section_keys_for_chunk,
        )
    }

    pub fn apply_loaded_view_sync<I>(
        &mut self,
        previous_chunks: &BTreeSet<ChunkPos>,
        current_loaded_chunks: BTreeSet<ChunkPos>,
        loaded_section_keys_for_chunk: impl FnMut(ChunkPos) -> I,
    ) -> RenderSectionViewSync
    where
        I: IntoIterator<Item = RenderSectionKey>,
    {
        let sync =
            RenderSectionViewSync::from_loaded_chunks(previous_chunks, current_loaded_chunks);
        self.mark_view_sync_dirty_with_loaded_sections(&sync, loaded_section_keys_for_chunk);
        sync
    }

    pub fn mark_section_dirty(&mut self, key: RenderSectionKey) {
        self.dirty.bump_section_revision(key);
        if !self.cache.mark_section_dirty(key) {
            self.dirty.dirty_sections.insert(key);
        }
    }

    pub fn section_metadata(&self) -> Vec<TexturedRenderSectionMetadata> {
        self.cache.section_metadata()
    }

    pub fn cached_section_count(&self) -> usize {
        self.cache.cached_section_count()
    }

    /// Mark every resident render section dirty for an explicit render-resource
    /// rebuild (docs/tactical/163). Returns the number of sections marked so the
    /// caller can log/skip when there is nothing to rebuild.
    pub fn mark_all_sections_dirty_for_resource_rebuild(&mut self) -> usize {
        self.cache.mark_all_sections_dirty()
    }

    pub fn section_keys(&self) -> impl Iterator<Item = RenderSectionKey> + '_ {
        self.cache.section_keys()
    }

    pub fn contains_chunk(&self, pos: ChunkPos) -> bool {
        self.cache.contains_chunk(pos)
    }

    pub fn contains_section(&self, key: RenderSectionKey) -> bool {
        self.cache.contains_section(key)
    }

    pub fn known_section_keys_for_chunk(
        &self,
        pos: ChunkPos,
        loaded_section_keys: impl IntoIterator<Item = RenderSectionKey>,
    ) -> BTreeSet<RenderSectionKey> {
        let mut keys = BTreeSet::new();
        keys.extend(loaded_section_keys);
        keys.extend(self.cache.section_keys_for_chunk(pos));
        keys.extend(
            self.dirty
                .dirty_sections
                .iter()
                .copied()
                .filter(|key| render_section_chunk_pos(*key) == pos),
        );
        keys.extend(
            self.dirty
                .inflight_sections
                .iter()
                .copied()
                .filter(|key| render_section_chunk_pos(*key) == pos),
        );
        keys
    }

    pub fn classify_dirty_work(
        &self,
        chunk_is_loaded: impl FnMut(ChunkPos) -> bool,
        chunk_is_cached: impl FnMut(ChunkPos) -> bool,
        mut section_is_loaded: impl FnMut(RenderSectionKey) -> bool,
        section_is_cached: impl FnMut(RenderSectionKey) -> bool,
    ) -> RenderSectionDirtyWork {
        let mut work = classify_render_section_dirty_work(
            &self.dirty,
            chunk_is_loaded,
            chunk_is_cached,
            |key| section_is_loaded(key),
            section_is_cached,
        );
        for key in self.cache.dirty_section_keys() {
            if section_is_loaded(key) {
                work.loaded_dirty_sections_by_chunk
                    .entry(render_section_chunk_pos(key))
                    .or_default()
                    .insert(key);
            } else {
                work.removal_dirty_sections.insert(key);
            }
        }
        work
    }

    pub fn prepare_sync_update(
        &mut self,
        chunk_is_loaded: impl FnMut(ChunkPos) -> bool,
        section_is_loaded: impl FnMut(RenderSectionKey) -> bool,
        order_loaded_dirty_chunks: impl FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        order_dirty_section_chunks: impl FnOnce(&RenderSectionDirtyWork) -> Vec<ChunkPos>,
        chunk_budget: usize,
        section_keys_for_chunk: impl FnMut(ChunkPos) -> Vec<RenderSectionKey>,
        section_readiness: impl FnMut(RenderSectionKey) -> RenderSectionNeighborReadiness,
        removal_mode: RenderSectionRemovalMode,
    ) -> RenderSectionSyncUpdate {
        let dirty_work = self.classify_dirty_work(
            chunk_is_loaded,
            |pos| self.cache.contains_chunk(pos),
            section_is_loaded,
            |key| self.cache.contains_section(key),
        );
        let sorted_loaded_dirty_chunks = order_loaded_dirty_chunks(&dirty_work);
        let sorted_dirty_section_chunks = order_dirty_section_chunks(&dirty_work);
        let sync_plan = self.prepare_sync_plan(
            dirty_work,
            sorted_loaded_dirty_chunks,
            sorted_dirty_section_chunks,
            chunk_budget,
            section_keys_for_chunk,
            section_readiness,
        );
        let mut cache_update = RenderSectionCacheUpdate::default();
        if removal_mode == RenderSectionRemovalMode::ApplyImmediately && sync_plan.has_removals() {
            cache_update.merge(self.apply_removals(
                &sync_plan.dirty_work.removal_dirty_chunks,
                &sync_plan.dirty_work.removal_dirty_sections,
            ));
        }
        RenderSectionSyncUpdate {
            sync_plan,
            cache_update,
        }
    }

    pub fn prepare_sync_plan(
        &mut self,
        dirty_work: RenderSectionDirtyWork,
        sorted_loaded_dirty_chunks: impl IntoIterator<Item = ChunkPos>,
        sorted_dirty_section_chunks: impl IntoIterator<Item = ChunkPos>,
        chunk_budget: usize,
        section_keys_for_chunk: impl FnMut(ChunkPos) -> Vec<RenderSectionKey>,
        section_readiness: impl FnMut(RenderSectionKey) -> RenderSectionNeighborReadiness,
    ) -> RenderSectionSyncPlan {
        prepare_render_section_sync_plan(
            &mut self.dirty,
            dirty_work,
            sorted_loaded_dirty_chunks,
            sorted_dirty_section_chunks,
            chunk_budget,
            section_keys_for_chunk,
            section_readiness,
        )
    }

    pub fn apply_ready_plan(&mut self, plan: &RenderSectionReadyPlan) {
        for pos in &plan.budgeted_loaded_chunks {
            self.dirty.dirty_chunks.remove(pos);
        }
        for key in &plan.ready_section_keys {
            self.dirty.dirty_sections.remove(key);
            self.cache.clear_section_dirty(*key);
        }
        for key in &plan.deferred_section_keys {
            if self.cache.mark_section_dirty(*key) {
                self.dirty.dirty_sections.remove(key);
            } else {
                self.dirty.dirty_sections.insert(*key);
            }
        }
    }

    pub fn build_ready_plan_compile_request(
        &self,
        plan: &RenderSectionReadyPlan,
        snapshots: Vec<ChunkSnapshot>,
    ) -> Option<RenderSectionCompileRequest> {
        self.dirty.build_ready_plan_compile_request(plan, snapshots)
    }

    pub fn accept_ready_plan_compile_submission(&mut self, plan: &RenderSectionReadyPlan) {
        self.dirty.mark_compile_submitted(&plan.ready_section_keys);
        self.apply_ready_plan(plan);
    }

    pub fn submit_ready_plan_compile_request<T, E>(
        &mut self,
        plan: &RenderSectionReadyPlan,
        snapshots: Vec<ChunkSnapshot>,
        submit: impl FnOnce(RenderSectionCompileRequest) -> std::result::Result<T, E>,
    ) -> std::result::Result<Option<RenderSectionCompileSubmission<T>>, E> {
        let Some(request) = self.build_ready_plan_compile_request(plan, snapshots) else {
            return Ok(None);
        };
        let submitted_section_count = request.target_sections.len();
        let output = submit(request)?;
        self.accept_ready_plan_compile_submission(plan);
        Ok(Some(RenderSectionCompileSubmission {
            submitted_section_count,
            output,
        }))
    }

    pub fn submit_prepared_sync_plan<T, E>(
        &mut self,
        sync_plan: &RenderSectionSyncPlan,
        snapshots: Vec<ChunkSnapshot>,
        submit: impl FnOnce(
            &RenderSectionSyncPlan,
            RenderSectionCompileRequest,
        ) -> std::result::Result<T, E>,
    ) -> std::result::Result<RenderSectionReadyWorkSubmission<T>, E> {
        let mut cache_update = RenderSectionCacheUpdate::default();
        let Some(request) = self.build_ready_plan_compile_request(&sync_plan.ready_plan, snapshots)
        else {
            self.apply_ready_plan(&sync_plan.ready_plan);
            cache_update.merge(sync_plan.ready_update(0));
            return Ok(RenderSectionReadyWorkSubmission {
                cache_update,
                submission: None,
            });
        };
        let submitted_section_count = request.target_sections.len();
        let output = submit(sync_plan, request)?;
        self.accept_ready_plan_compile_submission(&sync_plan.ready_plan);
        cache_update.merge(sync_plan.ready_update(submitted_section_count));
        Ok(RenderSectionReadyWorkSubmission {
            cache_update,
            submission: Some(RenderSectionCompileSubmission {
                submitted_section_count,
                output,
            }),
        })
    }

    pub fn finish_compile_result(
        &mut self,
        completed: RenderSectionCompileResult,
        request_id: u32,
    ) -> Result<RenderSectionCompileFinish> {
        finish_render_section_compile_result(&mut self.dirty, completed, request_id)
    }

    pub fn finish_compile_update(
        &mut self,
        completed: RenderSectionCompileResult,
        request_id: u32,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_sections: &BTreeSet<RenderSectionKey>,
        section_is_loaded: impl FnMut(RenderSectionKey) -> bool,
    ) -> Result<RenderSectionFinishedCompileUpdate> {
        let RenderSectionCompileFinish {
            acceptance_report,
            accepted_sections,
            stale_sections,
            build_report,
        } = self.finish_compile_result(completed, request_id)?;
        let mut cache_update = RenderSectionCacheUpdate::default();
        if !stale_sections.is_empty() {
            cache_update.stale_compile_section_count += stale_sections.len();
            self.requeue_stale_sections(stale_sections.clone(), section_is_loaded);
        }
        if let Some(build_report) = build_report {
            cache_update.merge(self.apply_finished_compile_report(
                &accepted_sections,
                build_report,
                removal_chunks,
                removal_sections,
            ));
        } else if !removal_chunks.is_empty() || !removal_sections.is_empty() {
            cache_update.merge(self.apply_removals(removal_chunks, removal_sections));
        }
        Ok(RenderSectionFinishedCompileUpdate {
            acceptance_report,
            accepted_sections,
            stale_sections,
            cache_update,
        })
    }

    pub fn drain_completed_compile_updates(
        &mut self,
        completed_results: impl IntoIterator<Item = RenderSectionCompileResult>,
        mut request_id_for_result: impl FnMut(&RenderSectionCompileResult) -> u32,
        pending_compile_jobs: usize,
        mut section_is_loaded: impl FnMut(RenderSectionKey) -> bool,
    ) -> Result<RenderSectionCacheUpdate> {
        let mut update = RenderSectionCacheUpdate::default();
        for completed in completed_results {
            let request_id = request_id_for_result(&completed);
            let finished = self.finish_compile_update(
                completed,
                request_id,
                &BTreeSet::new(),
                &BTreeSet::new(),
                |key| section_is_loaded(key),
            )?;
            update.merge(finished.cache_update);
        }
        update.pending_compile_jobs = pending_compile_jobs;
        Ok(update)
    }

    pub fn apply_finished_compile_report(
        &mut self,
        accepted_sections: &BTreeSet<RenderSectionKey>,
        build_report: TexturedRenderSectionBuildReport,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_sections: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        let update = self.cache.apply_build_report(
            accepted_sections,
            build_report,
            removal_chunks,
            removal_sections,
        );
        self.dirty
            .discard_removed_dirty_work(removal_chunks, removal_sections);
        update
    }

    pub fn apply_removals(
        &mut self,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_sections: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        let update = self.cache.remove_sections(removal_chunks, removal_sections);
        self.dirty
            .discard_removed_dirty_work(removal_chunks, removal_sections);
        update
    }

    pub fn requeue_stale_sections(
        &mut self,
        target_sections: BTreeSet<RenderSectionKey>,
        mut section_is_loaded: impl FnMut(RenderSectionKey) -> bool,
    ) -> usize {
        let mut requeued = 0;
        for key in target_sections {
            if section_is_loaded(key) || self.cache.contains_section(key) {
                if !self.cache.mark_section_dirty(key) {
                    self.dirty.dirty_sections.insert(key);
                }
                requeued += 1;
            }
        }
        requeued
    }
}
