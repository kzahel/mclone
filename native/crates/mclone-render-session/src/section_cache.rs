use super::*;

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
    // Tactical 166 Slice 1: real sections are the level-zero producer on the
    // shared resident-tile lifecycle. The facade preserves section-specific
    // names so generic bounds do not leak through render-session call sites.
    tiles: ResidentTileCache<RenderSectionKey, TexturedRenderSectionMetadata>,
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
    /// Convert an already-compiled one-shot startup seed into ordinary upload
    /// coordinator work.
    ///
    /// These meshes did not consume a live compile-dispatch grant, so the
    /// accepted-result count deliberately remains zero. Their vertex/index
    /// totals still describe the payload exactly for diagnostics and
    /// conservation checks.
    pub fn from_startup_seed(rebuilt_sections: Vec<TexturedRenderSectionMesh>) -> Self {
        let (rebuilt_vertex_count, rebuilt_index_count) =
            rebuilt_sections
                .iter()
                .fold((0_u32, 0_u32), |(vertices, indices), section| {
                    let stats = section.stats();
                    (
                        vertices.saturating_add(stats.vertex_count),
                        indices.saturating_add(stats.index_count),
                    )
                });
        Self {
            rebuilt_sections,
            rebuilt_vertex_count,
            rebuilt_index_count,
            ..Self::default()
        }
    }

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
        self.tiles.is_empty()
    }

    pub fn contains_chunk(&self, pos: ChunkPos) -> bool {
        self.tiles.contains_chunk(pos)
    }

    pub fn contains_section(&self, key: RenderSectionKey) -> bool {
        self.tiles.contains_tile(key)
    }

    /// Resident section metadata for every cached section. This replaces the old
    /// `sections()` accessor that cloned full CPU meshes (docs/tactical/163).
    pub fn section_metadata(&self) -> Vec<TexturedRenderSectionMetadata> {
        self.tiles.metadata().cloned().collect()
    }

    pub fn cached_section_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn section_keys(&self) -> impl Iterator<Item = RenderSectionKey> + '_ {
        self.tiles.tile_keys()
    }

    pub fn generation(&self) -> u64 {
        self.tiles.generation()
    }

    pub fn resident_mesh_stats(&self) -> RenderSectionResidentMeshStats {
        let mut stats = RenderSectionResidentMeshStats {
            resident_section_count: self.tiles.len(),
            ..RenderSectionResidentMeshStats::default()
        };
        for metadata in self.tiles.metadata() {
            let section_stats = &metadata.stats;
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
        self.tiles.tile_keys_for_chunk(pos)
    }

    pub(crate) fn has_dirty_sections(&self) -> bool {
        self.tiles.has_dirty_tiles()
    }

    pub(crate) fn dirty_section_count(&self) -> usize {
        self.tiles.dirty_tile_count()
    }

    pub(crate) fn dirty_section_keys(&self) -> impl Iterator<Item = RenderSectionKey> + '_ {
        self.tiles.dirty_tile_keys()
    }

    pub(crate) fn mark_section_dirty(&mut self, key: RenderSectionKey) -> bool {
        self.tiles.mark_tile_dirty(key)
    }

    /// Mark every resident section dirty so the next sync recompiles and
    /// re-emits their meshes. Used by explicit render-resource rebuilds that
    /// must reconstruct GPU buffers without a retained CPU mesh copy
    /// (docs/tactical/163). Returns the number of sections marked.
    pub(crate) fn mark_all_sections_dirty(&mut self) -> usize {
        self.tiles.mark_all_tiles_dirty()
    }

    pub(crate) fn clear_section_dirty(&mut self, key: RenderSectionKey) -> bool {
        self.tiles.clear_tile_dirty(key)
    }

    pub fn apply_build_report(
        &mut self,
        ready_section_keys: &BTreeSet<RenderSectionKey>,
        rebuilt_report: TexturedRenderSectionBuildReport,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_section_keys: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        let resident_update = self.tiles.apply_build_report(
            ready_section_keys,
            rebuilt_report.sections,
            removal_chunks,
            removal_section_keys,
            |section| section.key,
            TexturedRenderSectionMesh::metadata,
        );

        let mut report = RenderSectionCacheUpdate {
            rebuilt_sections: resident_update.rebuilt_tiles,
            removed_section_keys: resident_update.removed_tile_keys,
            rebuilt_vertex_count: 0,
            rebuilt_index_count: 0,
            visibility_graph_stats: rebuilt_report.visibility_graph,
            neighbor_ready_section_count: ready_section_keys.len(),
            completed_compile_section_count: ready_section_keys.len(),
            ..RenderSectionCacheUpdate::default()
        };
        // Tactical 163: the shared substrate inserted compact metadata only.
        // Full payloads remain in this update and move to the upload path.
        for section in &report.rebuilt_sections {
            let stats = section.stats();
            report.rebuilt_vertex_count += stats.vertex_count;
            report.rebuilt_index_count += stats.index_count;
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
}
