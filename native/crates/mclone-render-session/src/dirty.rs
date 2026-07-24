use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderSectionDirtyState {
    pub dirty_chunks: BTreeSet<ChunkPos>,
    pub dirty_sections: BTreeSet<RenderSectionKey>,
    pub inflight_sections: BTreeSet<RenderSectionKey>,
    pub(crate) section_revisions: BTreeMap<RenderSectionKey, u64>,
    pub(crate) work_generation: u64,
}

impl RenderSectionDirtyState {
    pub fn is_empty(&self) -> bool {
        self.dirty_chunks.is_empty()
            && self.dirty_sections.is_empty()
            && self.inflight_sections.is_empty()
    }

    pub fn dirty_work_is_empty(&self) -> bool {
        self.dirty_chunks.is_empty() && self.dirty_sections.is_empty()
    }

    pub fn mark_chunk_dirty(
        &mut self,
        pos: ChunkPos,
        known_section_keys: impl IntoIterator<Item = RenderSectionKey>,
    ) {
        for key in known_section_keys {
            self.bump_section_revision(key);
        }
        self.dirty_chunks.insert(pos);
        self.mark_work_changed();
    }

    pub fn mark_section_dirty(&mut self, key: RenderSectionKey) {
        self.bump_section_revision(key);
        self.dirty_sections.insert(key);
        self.mark_work_changed();
    }

    pub fn bump_section_revision(&mut self, key: RenderSectionKey) {
        let revision = self.section_revisions.entry(key).or_default();
        *revision = revision.wrapping_add(1);
    }

    pub fn bump_section_revisions(&mut self, keys: impl IntoIterator<Item = RenderSectionKey>) {
        for key in keys {
            self.bump_section_revision(key);
        }
    }

    pub fn section_revision(&self, key: RenderSectionKey) -> u64 {
        self.section_revisions
            .get(&key)
            .copied()
            .unwrap_or_default()
    }

    pub fn build_compile_request(
        &self,
        target_sections: BTreeSet<RenderSectionKey>,
        snapshots: Vec<ChunkSnapshot>,
    ) -> RenderSectionCompileRequest {
        let section_revisions = target_sections
            .iter()
            .map(|key| (*key, self.section_revision(*key)))
            .collect();
        RenderSectionCompileRequest {
            target_sections,
            section_revisions,
            snapshots,
            biome_zoom_seed: None,
            topology: HorizontalTopology::UNBOUNDED,
            grass_patches: false,
        }
    }

    pub fn mark_compile_submitted(&mut self, target_sections: &BTreeSet<RenderSectionKey>) {
        self.inflight_sections
            .extend(target_sections.iter().copied());
        if !target_sections.is_empty() {
            self.mark_work_changed();
        }
    }

    pub fn accept_completed_compile_result(
        &mut self,
        completed: &RenderSectionCompileResult,
    ) -> RenderSectionCompileAcceptance {
        for key in &completed.target_sections {
            self.inflight_sections.remove(key);
        }
        if !completed.target_sections.is_empty() {
            self.mark_work_changed();
        }
        completed.partition_by_revision(|key| self.section_revision(key))
    }

    pub fn discard_stale_dirty_work(
        &mut self,
        stale_chunks: &BTreeSet<ChunkPos>,
        stale_sections: &BTreeSet<RenderSectionKey>,
    ) {
        if !stale_chunks.is_empty() || !stale_sections.is_empty() {
            self.mark_work_changed();
        }
        for pos in stale_chunks {
            self.dirty_chunks.remove(pos);
        }
        for key in stale_sections {
            self.dirty_sections.remove(key);
        }
    }

    pub fn discard_removed_dirty_work(
        &mut self,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_sections: &BTreeSet<RenderSectionKey>,
    ) {
        if !removal_chunks.is_empty() || !removal_sections.is_empty() {
            self.mark_work_changed();
        }
        for pos in removal_chunks {
            self.dirty_chunks.remove(pos);
            self.dirty_sections
                .retain(|key| render_section_chunk_pos(*key) != *pos);
            self.inflight_sections
                .retain(|key| render_section_chunk_pos(*key) != *pos);
        }
        for key in removal_sections {
            self.dirty_sections.remove(key);
            self.inflight_sections.remove(key);
        }
    }

    pub fn apply_ready_plan(&mut self, plan: &RenderSectionReadyPlan) {
        if !plan.budgeted_loaded_chunks.is_empty()
            || !plan.ready_section_keys.is_empty()
            || !plan.deferred_section_keys.is_empty()
        {
            self.mark_work_changed();
        }
        for pos in &plan.budgeted_loaded_chunks {
            self.dirty_chunks.remove(pos);
        }
        for key in &plan.ready_section_keys {
            self.dirty_sections.remove(key);
        }
        self.dirty_sections
            .extend(plan.deferred_section_keys.iter().copied());
    }

    pub fn build_ready_plan_compile_request(
        &self,
        plan: &RenderSectionReadyPlan,
        snapshots: Vec<ChunkSnapshot>,
    ) -> Option<RenderSectionCompileRequest> {
        if plan.ready_section_keys.is_empty() {
            return None;
        }
        Some(self.build_compile_request(plan.ready_section_keys.clone(), snapshots))
    }

    pub fn accept_ready_plan_compile_submission(&mut self, plan: &RenderSectionReadyPlan) {
        self.mark_compile_submitted(&plan.ready_section_keys);
        self.apply_ready_plan(plan);
    }

    pub const fn work_generation(&self) -> u64 {
        self.work_generation
    }

    pub(crate) fn mark_work_changed(&mut self) {
        self.work_generation = self.work_generation.wrapping_add(1);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionDirtyWork {
    pub stale_dirty_chunks: BTreeSet<ChunkPos>,
    pub loaded_dirty_chunks: BTreeSet<ChunkPos>,
    pub removal_dirty_chunks: BTreeSet<ChunkPos>,
    pub stale_dirty_sections: BTreeSet<RenderSectionKey>,
    pub loaded_dirty_sections_by_chunk: BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    pub removal_dirty_sections: BTreeSet<RenderSectionKey>,
}

impl RenderSectionDirtyWork {
    pub fn has_removals(&self) -> bool {
        !self.removal_dirty_chunks.is_empty() || !self.removal_dirty_sections.is_empty()
    }
}

pub fn classify_render_section_dirty_work(
    dirty: &RenderSectionDirtyState,
    mut chunk_is_loaded: impl FnMut(ChunkPos) -> bool,
    mut chunk_is_cached: impl FnMut(ChunkPos) -> bool,
    mut section_is_loaded: impl FnMut(RenderSectionKey) -> bool,
    mut section_is_cached: impl FnMut(RenderSectionKey) -> bool,
) -> RenderSectionDirtyWork {
    let mut work = RenderSectionDirtyWork::default();

    for pos in dirty.dirty_chunks.iter().copied() {
        if chunk_is_loaded(pos) {
            work.loaded_dirty_chunks.insert(pos);
        } else if chunk_is_cached(pos) {
            work.removal_dirty_chunks.insert(pos);
        } else {
            work.stale_dirty_chunks.insert(pos);
        }
    }

    for key in dirty.dirty_sections.iter().copied() {
        if section_is_loaded(key) {
            work.loaded_dirty_sections_by_chunk
                .entry(render_section_chunk_pos(key))
                .or_default()
                .insert(key);
        } else if section_is_cached(key) {
            work.removal_dirty_sections.insert(key);
        } else {
            work.stale_dirty_sections.insert(key);
        }
    }

    work
}

pub fn render_section_chunk_pos(key: RenderSectionKey) -> ChunkPos {
    ChunkPos::new(key.chunk_x, key.chunk_z)
}

pub fn render_section_keys_for_snapshot(snapshot: &ChunkSnapshot) -> Vec<RenderSectionKey> {
    let min_section_y = snapshot.min_y.div_euclid(SECTION_HEIGHT);
    let section_count = snapshot.height / SECTION_HEIGHT;
    (0..section_count)
        .map(|offset| RenderSectionKey::new(snapshot.pos.x, min_section_y + offset, snapshot.pos.z))
        .collect()
}

pub fn snapshot_contains_render_section(snapshot: &ChunkSnapshot, key: RenderSectionKey) -> bool {
    snapshot.pos == render_section_chunk_pos(key)
        && key.section_y * SECTION_HEIGHT >= snapshot.min_y
        && key.section_y * SECTION_HEIGHT < snapshot.min_y + snapshot.height
}

pub fn ready_section_keys_for_report(
    report: &TexturedRenderSectionBuildReport,
    dirty_chunks: &BTreeSet<ChunkPos>,
) -> BTreeSet<RenderSectionKey> {
    report
        .sections
        .iter()
        .map(|section| section.key)
        .filter(|key| dirty_chunks.contains(&render_section_chunk_pos(*key)))
        .collect()
}

pub fn target_section_keys_for_dirty_chunks<'a>(
    snapshots: impl IntoIterator<Item = &'a ChunkSnapshot>,
    dirty_chunks: &BTreeSet<ChunkPos>,
) -> BTreeSet<RenderSectionKey> {
    snapshots
        .into_iter()
        .filter(|snapshot| dirty_chunks.contains(&snapshot.pos))
        .flat_map(render_section_keys_for_snapshot)
        .collect()
}

pub fn dirty_chunk_positions(
    previous_chunks: &BTreeSet<ChunkPos>,
    current_chunks: &BTreeSet<ChunkPos>,
) -> BTreeSet<ChunkPos> {
    if previous_chunks.is_empty() {
        return current_chunks.clone();
    }
    let added_chunks = current_chunks
        .difference(previous_chunks)
        .copied()
        .collect::<BTreeSet<_>>();
    let removed_chunks = previous_chunks
        .difference(current_chunks)
        .copied()
        .collect::<BTreeSet<_>>();
    current_chunks
        .iter()
        .copied()
        .filter(|pos| added_chunks.contains(pos) || has_removed_neighbor(*pos, &removed_chunks))
        .collect()
}

pub fn render_dirty_chunk_neighborhood(pos: ChunkPos) -> [ChunkPos; 5] {
    [
        pos,
        ChunkPos::new(pos.x - 1, pos.z),
        ChunkPos::new(pos.x + 1, pos.z),
        ChunkPos::new(pos.x, pos.z - 1),
        ChunkPos::new(pos.x, pos.z + 1),
    ]
}

fn has_removed_neighbor(pos: ChunkPos, removed_chunks: &BTreeSet<ChunkPos>) -> bool {
    [
        ChunkPos::new(pos.x - 1, pos.z),
        ChunkPos::new(pos.x + 1, pos.z),
        ChunkPos::new(pos.x, pos.z - 1),
        ChunkPos::new(pos.x, pos.z + 1),
    ]
    .into_iter()
    .any(|neighbor| removed_chunks.contains(&neighbor))
}
