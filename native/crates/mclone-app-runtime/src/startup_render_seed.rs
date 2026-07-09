//! Transient startup render-section seed (docs/tactical/167).
//!
//! During native world startup the section-compile pump produces a stream of
//! [`RenderSectionCacheUpdate`]s while polling to a playable gate. After
//! docs/tactical/163 the resident render-section cache keeps only metadata, so
//! the transient CPU mesh payload is gone by the time a platform adapter wants
//! to create draw resources. The seed captures those payloads for the startup
//! window only and hands the final batch to draw-resource creation, replacing
//! the old "mark every resident section dirty and recompile" workaround.

use std::collections::BTreeMap;

use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh};
use mclone_render_session::RenderSectionCacheUpdate;

/// Short-lived keyed accumulator of the latest compiled mesh per render section.
///
/// Unlike [`RenderSectionCacheUpdate::merge`], which appends rebuilds and unions
/// removals (correct for counters, wrong for reconstructing the final resident
/// upload batch), the seed keeps only the latest mesh per key. It must never
/// become a resident cache: it is drained once at startup completion and the
/// retained CPU payload drops to zero.
#[derive(Clone, Debug, Default)]
pub struct StartupRenderSectionSeed {
    sections: BTreeMap<RenderSectionKey, TexturedRenderSectionMesh>,
}

impl StartupRenderSectionSeed {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold one transient cache update into the seed. Removals are applied before
    /// rebuilds so a key carried in both sets of a single update keeps its
    /// freshly accepted rebuild and stays drawable (docs/tactical/167).
    pub fn observe(&mut self, update: &RenderSectionCacheUpdate) {
        for key in &update.removed_section_keys {
            self.sections.remove(key);
        }
        for section in &update.rebuilt_sections {
            self.sections.insert(section.key, section.clone());
        }
    }

    /// Total sections currently held, including empty (all-air) sections.
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Sections that carry drawable geometry (non-empty mesh).
    pub fn drawable_section_count(&self) -> usize {
        self.sections
            .values()
            .filter(|section| !section.is_empty())
            .count()
    }

    /// Transient CPU payload retained for the startup window. Diagnostics may read
    /// this to observe startup pressure; it must drop to zero after draining.
    pub fn estimated_owned_bytes(&self) -> usize {
        self.sections
            .values()
            .map(TexturedRenderSectionMesh::estimated_owned_bytes)
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    /// Consume the accumulated batch for a one-shot draw-resource upload. The seed
    /// is emptied; it must not be reused as a resident cache after draining.
    pub fn drain_sections(&mut self) -> Vec<TexturedRenderSectionMesh> {
        std::mem::take(&mut self.sections).into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use mclone_mesh::{
        RenderSectionKey, TexturedChunkVertex, TexturedRenderSectionMesh, TexturedVisibleChunkMesh,
        VisibilitySet,
    };
    use mclone_render_session::RenderSectionCacheUpdate;

    use super::StartupRenderSectionSeed;

    fn key(x: i32) -> RenderSectionKey {
        RenderSectionKey::new(x, 0, 0)
    }

    fn drawable_mesh(k: RenderSectionKey) -> TexturedRenderSectionMesh {
        TexturedRenderSectionMesh {
            key: k,
            mesh: TexturedVisibleChunkMesh {
                vertices: vec![TexturedChunkVertex {
                    position: [0.0, 0.0, 0.0],
                    uv: [0.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                    packed_light: 0,
                }],
                indices: vec![0, 1, 2, 2, 3, 0],
                solid_index_count: 6,
                opaque_index_count: 6,
            },
            visibility: VisibilitySet::all_visible(),
        }
    }

    fn empty_mesh(k: RenderSectionKey) -> TexturedRenderSectionMesh {
        TexturedRenderSectionMesh {
            key: k,
            mesh: TexturedVisibleChunkMesh::default(),
            visibility: VisibilitySet::all_visible(),
        }
    }

    fn rebuilt(sections: Vec<TexturedRenderSectionMesh>) -> RenderSectionCacheUpdate {
        RenderSectionCacheUpdate {
            rebuilt_sections: sections,
            ..RenderSectionCacheUpdate::default()
        }
    }

    fn removed(keys: impl IntoIterator<Item = RenderSectionKey>) -> RenderSectionCacheUpdate {
        RenderSectionCacheUpdate {
            removed_section_keys: keys.into_iter().collect::<BTreeSet<_>>(),
            ..RenderSectionCacheUpdate::default()
        }
    }

    #[test]
    fn latest_rebuild_wins_across_updates() {
        let mut seed = StartupRenderSectionSeed::new();
        seed.observe(&rebuilt(vec![empty_mesh(key(0))]));
        seed.observe(&rebuilt(vec![drawable_mesh(key(0))]));

        let sections = seed.drain_sections();
        assert_eq!(sections.len(), 1, "same key must not duplicate");
        assert!(!sections[0].is_empty(), "later rebuild replaces earlier");
    }

    #[test]
    fn removal_deletes_a_previously_rebuilt_section() {
        let mut seed = StartupRenderSectionSeed::new();
        seed.observe(&rebuilt(vec![drawable_mesh(key(0)), drawable_mesh(key(1))]));
        seed.observe(&removed([key(0)]));

        let sections = seed.drain_sections();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].key, key(1));
    }

    #[test]
    fn removal_and_rebuild_in_same_update_keeps_the_rebuild() {
        let mut seed = StartupRenderSectionSeed::new();
        seed.observe(&rebuilt(vec![empty_mesh(key(0))]));

        // One update that both removes and rebuilds the same key: removals are
        // applied before rebuilds, so the freshly accepted rebuild survives.
        let update = RenderSectionCacheUpdate {
            rebuilt_sections: vec![drawable_mesh(key(0))],
            removed_section_keys: [key(0)].into_iter().collect(),
            ..RenderSectionCacheUpdate::default()
        };
        seed.observe(&update);

        let sections = seed.drain_sections();
        assert_eq!(sections.len(), 1);
        assert!(!sections[0].is_empty(), "rebuild must win over removal");
    }

    #[test]
    fn drawable_count_reflects_non_empty_meshes() {
        let mut seed = StartupRenderSectionSeed::new();
        seed.observe(&rebuilt(vec![
            drawable_mesh(key(0)),
            empty_mesh(key(1)),
            drawable_mesh(key(2)),
        ]));

        assert_eq!(seed.section_count(), 3);
        assert_eq!(seed.drawable_section_count(), 2);
        assert!(seed.estimated_owned_bytes() > 0);
    }

    #[test]
    fn drain_empties_the_seed_and_owned_bytes_drop_to_zero() {
        let mut seed = StartupRenderSectionSeed::new();
        seed.observe(&rebuilt(vec![drawable_mesh(key(0))]));
        assert!(!seed.is_empty());

        let sections = seed.drain_sections();
        assert_eq!(sections.len(), 1);
        assert!(seed.is_empty(), "seed must not remain a resident cache");
        assert_eq!(seed.section_count(), 0);
        assert_eq!(seed.estimated_owned_bytes(), 0);
    }
}
