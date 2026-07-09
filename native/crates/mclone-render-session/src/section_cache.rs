use super::*;

#[derive(Clone, Debug)]
struct CachedTexturedRenderSectionSlot {
    // docs/tactical/163: the resident cache retains only compact metadata after a
    // section is compiled and uploaded, not the owned CPU vertex/index payload.
    metadata: TexturedRenderSectionMetadata,
    dirty: bool,
}

impl CachedTexturedRenderSectionSlot {
    fn new(metadata: TexturedRenderSectionMetadata) -> Self {
        Self {
            metadata,
            dirty: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderSectionResidentMeshStats {
    pub resident_section_count: usize,
    pub resident_vertex_count: u32,
    pub resident_index_count: u32,
    pub resident_mesh_owned_bytes: usize,
}

impl RenderSectionResidentMeshStats {
    pub fn resident_face_count(&self) -> u32 {
        quad_face_count_from_indices(self.resident_index_count)
    }
}

#[derive(Clone, Debug, Default)]
pub struct CachedTexturedRenderSections {
    sections: BTreeMap<RenderSectionKey, CachedTexturedRenderSectionSlot>,
    section_keys_by_chunk: BTreeMap<ChunkPos, BTreeSet<RenderSectionKey>>,
    generation: u64,
}

#[derive(Clone, Debug, Default)]
pub struct RenderSectionCacheUpdate {
    pub rebuilt_sections: Vec<TexturedRenderSectionMesh>,
    pub removed_section_keys: BTreeSet<RenderSectionKey>,
    pub rebuilt_vertex_count: u32,
    pub rebuilt_index_count: u32,
    pub neighbor_ready_section_count: usize,
    pub near_exception_section_count: usize,
    pub deferred_section_count: usize,
    pub submitted_compile_section_count: usize,
    pub deadline_skipped_compile_request_count: usize,
    pub accepted_compile_result_count: usize,
    pub queued_completed_compile_result_count: usize,
    pub completed_compile_section_count: usize,
    pub stale_compile_section_count: usize,
    pub pending_compile_jobs: usize,
    pub visibility_graph_stats: VisibilityGraphBuildStats,
    pub resident_mesh_stats: Option<RenderSectionResidentMeshStats>,
}

impl RenderSectionCacheUpdate {
    pub fn rebuilt_section_count(&self) -> usize {
        self.rebuilt_sections.len()
    }

    pub fn removed_section_count(&self) -> usize {
        self.removed_section_keys.len()
    }

    pub fn rebuilt_face_count(&self) -> u32 {
        quad_face_count_from_indices(self.rebuilt_index_count)
    }

    pub fn merge(&mut self, other: Self) {
        self.rebuilt_sections.extend(other.rebuilt_sections);
        self.removed_section_keys.extend(other.removed_section_keys);
        self.rebuilt_vertex_count += other.rebuilt_vertex_count;
        self.rebuilt_index_count += other.rebuilt_index_count;
        self.neighbor_ready_section_count += other.neighbor_ready_section_count;
        self.near_exception_section_count += other.near_exception_section_count;
        self.deferred_section_count += other.deferred_section_count;
        self.submitted_compile_section_count += other.submitted_compile_section_count;
        self.deadline_skipped_compile_request_count += other.deadline_skipped_compile_request_count;
        self.accepted_compile_result_count += other.accepted_compile_result_count;
        self.queued_completed_compile_result_count = other.queued_completed_compile_result_count;
        self.completed_compile_section_count += other.completed_compile_section_count;
        self.stale_compile_section_count += other.stale_compile_section_count;
        self.pending_compile_jobs = other.pending_compile_jobs;
        if other.resident_mesh_stats.is_some() {
            self.resident_mesh_stats = other.resident_mesh_stats;
        }
        self.visibility_graph_stats.build_count += other.visibility_graph_stats.build_count;
        self.visibility_graph_stats.total_ms += other.visibility_graph_stats.total_ms;
        self.visibility_graph_stats.worst_ms = self
            .visibility_graph_stats
            .worst_ms
            .max(other.visibility_graph_stats.worst_ms);
    }
}

impl CachedTexturedRenderSections {
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    pub fn contains_chunk(&self, pos: ChunkPos) -> bool {
        self.section_keys_by_chunk.contains_key(&pos)
    }

    pub fn contains_section(&self, key: RenderSectionKey) -> bool {
        self.sections.contains_key(&key)
    }

    /// Resident section metadata for every cached section. This replaces the old
    /// `sections()` accessor that cloned full CPU meshes (docs/tactical/163).
    pub fn section_metadata(&self) -> Vec<TexturedRenderSectionMetadata> {
        self.sections
            .values()
            .map(|slot| slot.metadata.clone())
            .collect()
    }

    pub fn cached_section_count(&self) -> usize {
        self.sections.len()
    }

    pub fn section_keys(&self) -> impl Iterator<Item = RenderSectionKey> + '_ {
        self.sections.keys().copied()
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn resident_mesh_stats(&self) -> RenderSectionResidentMeshStats {
        let mut stats = RenderSectionResidentMeshStats {
            resident_section_count: self.sections.len(),
            ..RenderSectionResidentMeshStats::default()
        };
        for slot in self.sections.values() {
            let section_stats = &slot.metadata.stats;
            stats.resident_vertex_count = stats
                .resident_vertex_count
                .saturating_add(section_stats.vertex_count);
            stats.resident_index_count = stats
                .resident_index_count
                .saturating_add(section_stats.index_count);
            // docs/tactical/163: the resident cache no longer owns vertex/index
            // vectors, so its retained CPU mesh byte pressure is zero. The
            // transient compile/upload payloads still carry those bytes and are
            // accounted separately by the upload coordinator.
        }
        stats
    }

    pub fn section_keys_for_chunk(
        &self,
        pos: ChunkPos,
    ) -> impl Iterator<Item = RenderSectionKey> + '_ {
        self.section_keys_by_chunk
            .get(&pos)
            .into_iter()
            .flat_map(|keys| keys.iter().copied())
    }

    pub(crate) fn has_dirty_sections(&self) -> bool {
        self.sections.values().any(|slot| slot.dirty)
    }

    pub(crate) fn dirty_section_count(&self) -> usize {
        self.sections.values().filter(|slot| slot.dirty).count()
    }

    pub(crate) fn dirty_section_keys(&self) -> impl Iterator<Item = RenderSectionKey> + '_ {
        self.sections
            .iter()
            .filter_map(|(key, slot)| slot.dirty.then_some(*key))
    }

    pub(crate) fn mark_section_dirty(&mut self, key: RenderSectionKey) -> bool {
        let Some(slot) = self.sections.get_mut(&key) else {
            return false;
        };
        slot.dirty = true;
        true
    }

    /// Mark every resident section dirty so the next sync recompiles and
    /// re-emits their meshes. Used by explicit render-resource rebuilds that
    /// must reconstruct GPU buffers without a retained CPU mesh copy
    /// (docs/tactical/163). Returns the number of sections marked.
    pub(crate) fn mark_all_sections_dirty(&mut self) -> usize {
        let mut marked = 0;
        for slot in self.sections.values_mut() {
            if !slot.dirty {
                marked += 1;
            }
            slot.dirty = true;
        }
        marked
    }

    pub(crate) fn clear_section_dirty(&mut self, key: RenderSectionKey) -> bool {
        let Some(slot) = self.sections.get_mut(&key) else {
            return false;
        };
        slot.dirty = false;
        true
    }

    pub fn apply_build_report(
        &mut self,
        ready_section_keys: &BTreeSet<RenderSectionKey>,
        rebuilt_report: TexturedRenderSectionBuildReport,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_section_keys: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        let rebuilt_keys = rebuilt_report
            .sections
            .iter()
            .map(|section| section.key)
            .collect::<BTreeSet<_>>();
        let rebuilt_sections = rebuilt_report
            .sections
            .into_iter()
            .filter(|section| ready_section_keys.contains(&section.key))
            .collect::<Vec<_>>();
        let old_ready_keys = self
            .sections
            .keys()
            .copied()
            .filter(|key| ready_section_keys.contains(key))
            .collect::<BTreeSet<_>>();
        let removal_keys = self
            .sections
            .keys()
            .copied()
            .filter(|key| removal_chunks.contains(&ChunkPos::new(key.chunk_x, key.chunk_z)))
            .collect::<BTreeSet<_>>();
        let mut removed_section_keys = old_ready_keys
            .difference(&rebuilt_keys)
            .copied()
            .collect::<BTreeSet<_>>();
        removed_section_keys.extend(removal_keys);
        removed_section_keys.extend(removal_section_keys.iter().copied());

        for key in &removed_section_keys {
            self.remove_section(*key);
        }

        let mut report = RenderSectionCacheUpdate {
            rebuilt_sections,
            removed_section_keys,
            rebuilt_vertex_count: 0,
            rebuilt_index_count: 0,
            visibility_graph_stats: rebuilt_report.visibility_graph,
            neighbor_ready_section_count: ready_section_keys.len(),
            completed_compile_section_count: ready_section_keys.len(),
            ..RenderSectionCacheUpdate::default()
        };
        // docs/tactical/163: borrow each rebuilt section only long enough to
        // record accounting and insert its compact metadata. The full mesh
        // payload stays in `report.rebuilt_sections` and is moved out to the
        // upload path below — it is never cloned back into the resident cache.
        for section in &report.rebuilt_sections {
            let stats = section.stats();
            report.rebuilt_vertex_count += stats.vertex_count;
            report.rebuilt_index_count += stats.index_count;
            self.insert_metadata(section.metadata());
        }
        report.resident_mesh_stats = Some(self.resident_mesh_stats());
        report
    }

    pub fn remove_sections(
        &mut self,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_section_keys: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        self.apply_build_report(
            &BTreeSet::new(),
            TexturedRenderSectionBuildReport::default(),
            removal_chunks,
            removal_section_keys,
        )
    }

    fn insert_metadata(&mut self, metadata: TexturedRenderSectionMetadata) {
        let key = metadata.key;
        if !self.sections.contains_key(&key) {
            self.generation = self.generation.wrapping_add(1);
        }
        self.sections
            .insert(key, CachedTexturedRenderSectionSlot::new(metadata));
        self.section_keys_by_chunk
            .entry(render_section_chunk_pos(key))
            .or_default()
            .insert(key);
    }

    fn remove_section(&mut self, key: RenderSectionKey) -> Option<TexturedRenderSectionMetadata> {
        let metadata = self.sections.remove(&key)?.metadata;
        self.generation = self.generation.wrapping_add(1);
        let pos = render_section_chunk_pos(key);
        if let Some(keys) = self.section_keys_by_chunk.get_mut(&pos) {
            keys.remove(&key);
            if keys.is_empty() {
                self.section_keys_by_chunk.remove(&pos);
            }
        }
        Some(metadata)
    }
}
