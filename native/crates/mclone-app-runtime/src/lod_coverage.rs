//! CPU-side LOD coverage coordinator (tactical 162 Slice 2).
//!
//! This module moves far-LOD visibility decisions behind an explicit,
//! observable coordinator instead of the previous implicit path where the
//! synthetic cache silently omitted geometry wherever a normal chunk happened
//! to be drawable. The coordinator tracks two coverage maps separately:
//!
//! - [`NormalChunkCoverage`]: per-chunk *drawable* state, kept distinct from the
//!   client's loaded-chunk map. A chunk that is loaded but whose render mesh is
//!   not yet drawable does **not** suppress overlapping LOD; only a drawable
//!   normal chunk does. This is what keeps old LOD visible until a better
//!   replacement is actually drawable (no blank flicker on replacement), the
//!   same "old output stays until replacement is complete" invariant tracked in
//!   tactical 128.
//! - [`LodTileCoverage`]: per-tile source kind, quality, and resident lifecycle
//!   state for the LOD representation of that cell.
//!
//! Precedence is applied *per tile/cell*, never by rebuilding one centered world
//! mesh:
//!
//! ```text
//! normal drawable > reduced real chunk > synthetic surface > nothing
//! ```
//!
//! Slice 2 only exercises the synthetic-surface source; the reduced-real slots
//! are modeled here so tactical 162 Slice 4 drops straight in. The coordinator
//! is a pure decision component: it is fed the drawable/loaded normal sets and
//! the LOD tiles a source currently has available, and it returns per-frame
//! replacement/pop diagnostics plus the set of tiles drawn as LOD.

use std::collections::{BTreeMap, BTreeSet};

use mclone_core::ChunkPos;

/// Backstop on how many LOD-relevant tiles the coordinator tracks across frames.
/// Coverage is already bounded by the synthetic cache's retention radius and the
/// normal render area, so this only guards against unbounded growth of stale
/// `Normal`-visible entries. Mirrors the synthetic patch cap.
pub const MAX_TRACKED_LOD_COVERAGE_TILES: usize = 4096;

/// Drawable state of a normal (real) chunk mesh, tracked separately from the
/// client's loaded-chunk map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NormalChunkDrawState {
    /// Chunk data is present but its render mesh is not yet drawable. Does not
    /// suppress overlapping LOD.
    Loaded,
    /// The normal chunk mesh is drawable; it suppresses overlapping LOD.
    Drawable,
}

/// Drawable coverage map for normal chunks, kept separate from the loaded-chunk
/// map. Rebuilt each resolve from the drawable and loaded sets the runtime
/// observes so it never carries stale drawable state.
#[derive(Clone, Debug, Default)]
pub struct NormalChunkCoverage {
    chunks: BTreeMap<ChunkPos, NormalChunkDrawState>,
}

impl NormalChunkCoverage {
    fn rebuild(&mut self, drawable: &BTreeSet<ChunkPos>, loaded: &BTreeSet<ChunkPos>) {
        self.chunks.clear();
        for pos in loaded {
            self.chunks.insert(*pos, NormalChunkDrawState::Loaded);
        }
        // Drawable overrides loaded: a chunk can be both loaded and drawable, and
        // drawable is the state that decides suppression.
        for pos in drawable {
            self.chunks.insert(*pos, NormalChunkDrawState::Drawable);
        }
    }

    pub fn state(&self, pos: ChunkPos) -> Option<NormalChunkDrawState> {
        self.chunks.get(&pos).copied()
    }

    pub fn is_drawable(&self, pos: ChunkPos) -> bool {
        matches!(self.state(pos), Some(NormalChunkDrawState::Drawable))
    }

    pub fn drawable_count(&self) -> usize {
        self.chunks
            .values()
            .filter(|state| matches!(state, NormalChunkDrawState::Drawable))
            .count()
    }

    pub fn tracked_count(&self) -> usize {
        self.chunks.len()
    }
}

/// Where the LOD facts backing a tile came from. Precedence rank increases with
/// fidelity; `normal drawable` outranks all of these and is handled separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LodSourceKind {
    /// Cheap seed/worldgen surface shell. The only real source in Slice 2.
    SyntheticSurface,
    /// Derived from loaded/published real chunk facts (tactical 162 Slice 4).
    ReducedRealChunk,
    /// Reduced-real facts restored from the discardable cache (Slice 6).
    PersistedReducedRealChunk,
}

impl LodSourceKind {
    /// Higher rank wins when two LOD sources cover the same cell.
    const fn precedence_rank(self) -> u8 {
        match self {
            Self::SyntheticSurface => 1,
            Self::PersistedReducedRealChunk => 2,
            Self::ReducedRealChunk => 3,
        }
    }

    const fn is_reduced_real(self) -> bool {
        matches!(
            self,
            Self::ReducedRealChunk | Self::PersistedReducedRealChunk
        )
    }
}

/// Fidelity of a LOD tile's facts. Lit reduced-real tiles should replace unlit or
/// stale ones; synthetic tiles are effectively `Unlit` daytime shells.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LodTileQuality {
    Unlit,
    Lit,
    Stale,
    Dirty,
    Cached,
}

/// Resident lifecycle of a LOD tile, aligned with tactical 128's
/// mark-dirty → try-ready → inflight → accept → upload → publish-drawable model.
/// Slice 2 only moves synthetic tiles between `Pending` and `Drawable`; the
/// intermediate phases are modeled for Slice 3/4 budget/reduction work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LodTileLifecycle {
    Pending,
    Reducing,
    Meshing,
    Uploading,
    Drawable,
}

impl LodTileLifecycle {
    pub const fn is_drawable(self) -> bool {
        matches!(self, Self::Drawable)
    }
}

/// Which representation is currently drawn for a cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisibleSource {
    Nothing,
    Lod(LodSourceKind),
    Normal,
}

/// The LOD representation state of one tracked tile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LodTileState {
    pub source: LodSourceKind,
    pub quality: LodTileQuality,
    pub lifecycle: LodTileLifecycle,
}

/// Per-tile coverage record: tile identity, its last-known LOD representation (if
/// any), and which source is currently visible after precedence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LodTileCoverage {
    pub tile: ChunkPos,
    pub lod: Option<LodTileState>,
    pub visible: VisibleSource,
}

/// What a LOD source is offering for a cell this frame. Multiple availabilities
/// for the same tile are resolved by precedence rank.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LodTileAvailability {
    pub tile: ChunkPos,
    pub source: LodSourceKind,
    pub quality: LodTileQuality,
    /// Whether this representation is ready to draw. A tile whose reduction/mesh
    /// is still pending is available but not drawable, so it cannot yet win
    /// precedence over an already-drawable representation.
    pub drawable: bool,
}

impl LodTileAvailability {
    /// A drawable synthetic-surface tile: the only source the runtime feeds in
    /// Slice 2. Synthetic shells carry no real light, so they are `Unlit`.
    pub const fn synthetic(tile: ChunkPos) -> Self {
        Self {
            tile,
            source: LodSourceKind::SyntheticSurface,
            quality: LodTileQuality::Unlit,
            drawable: true,
        }
    }

    const fn lifecycle(self) -> LodTileLifecycle {
        if self.drawable {
            LodTileLifecycle::Drawable
        } else {
            LodTileLifecycle::Pending
        }
    }

    const fn state(self) -> LodTileState {
        LodTileState {
            source: self.source,
            quality: self.quality,
            lifecycle: self.lifecycle(),
        }
    }
}

/// Replacement and pop counters. Frame deltas accumulate into a cumulative total
/// so movement and startup replacement/pop behavior is visible over time.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LodReplacementCounters {
    /// A drawable normal chunk began suppressing a previously drawable LOD tile.
    pub normal_over_lod: u64,
    /// A reduced-real (or persisted reduced-real) tile replaced a synthetic tile.
    pub reduced_real_over_synthetic: u64,
    /// A normal chunk stopped being drawable and a retained LOD tile became
    /// visible again (unload re-reveals LOD).
    pub lod_revealed_on_unload: u64,
    /// A LOD tile was drawn for the first time (startup/coverage fill).
    pub lod_first_drawn: u64,
    /// A tile left the coverage area entirely (benign eviction, not a pop).
    pub lod_evicted: u64,
    /// A tile still expected on screen (loaded or drawable normal present) lost
    /// its drawable representation with nothing to replace it — a real blank/pop.
    /// This should stay at zero when replacement-before-suppress holds.
    pub suppressed_without_replacement: u64,
}

impl LodReplacementCounters {
    pub fn accumulate(&mut self, other: Self) {
        self.normal_over_lod += other.normal_over_lod;
        self.reduced_real_over_synthetic += other.reduced_real_over_synthetic;
        self.lod_revealed_on_unload += other.lod_revealed_on_unload;
        self.lod_first_drawn += other.lod_first_drawn;
        self.lod_evicted += other.lod_evicted;
        self.suppressed_without_replacement += other.suppressed_without_replacement;
    }

    pub const fn is_zero(&self) -> bool {
        self.normal_over_lod == 0
            && self.reduced_real_over_synthetic == 0
            && self.lod_revealed_on_unload == 0
            && self.lod_first_drawn == 0
            && self.lod_evicted == 0
            && self.suppressed_without_replacement == 0
    }
}

/// Outcome of one [`LodCoverageCoordinator::resolve`] pass.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LodCoverageResolution {
    /// Tiles drawn as LOD this frame (i.e. not suppressed by a drawable normal
    /// chunk). This is what the synthetic mesh should contain; the runtime keeps
    /// the mesh and this set consistent per tile rather than rebuilding a whole
    /// centered world.
    pub visible_lod_tiles: BTreeSet<ChunkPos>,
    /// Replacement/pop counters observed this frame.
    pub frame: LodReplacementCounters,
    /// Cumulative counters after this frame.
    pub total: LodReplacementCounters,
    /// How many LOD-relevant tiles the coordinator is tracking.
    pub tracked_tiles: usize,
}

/// The shared owner of per-tile LOD visibility decisions.
///
/// Owner boundary (tactical 162 Slice 2): the coordinator decides *which source
/// is visible per tile* and records replacement/pop diagnostics. The synthetic
/// [`crate::far_lod::FarTerrainLodCache`] remains the source that builds the LOD
/// mesh handed to the render session, and the renderer's draw buffers stay a
/// derived product of that mesh. The coordinator never builds GPU buffers and
/// never satisfies chunk interest, collision, raycast, edits, or gameplay
/// authority.
#[derive(Clone, Debug, Default)]
pub struct LodCoverageCoordinator {
    normal: NormalChunkCoverage,
    tiles: BTreeMap<ChunkPos, LodTileCoverage>,
    total: LodReplacementCounters,
}

impl LodCoverageCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.normal = NormalChunkCoverage::default();
        self.tiles.clear();
        self.total = LodReplacementCounters::default();
    }

    pub const fn counters(&self) -> LodReplacementCounters {
        self.total
    }

    pub fn normal_coverage(&self) -> &NormalChunkCoverage {
        &self.normal
    }

    pub fn tile(&self, pos: ChunkPos) -> Option<&LodTileCoverage> {
        self.tiles.get(&pos)
    }

    pub fn tracked_tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Resolve per-tile precedence for one frame.
    ///
    /// `normal_drawable` is the set of chunks whose real mesh is drawable (these
    /// suppress overlapping LOD). `normal_loaded` is the set of chunks whose data
    /// is loaded; a loaded-but-not-drawable chunk does not suppress LOD, and it
    /// marks a cell as still expected on screen so that losing its LOD there is
    /// counted as a pop rather than a benign eviction. `lod_tiles` is what the LOD
    /// sources currently offer for the covered cells.
    pub fn resolve<I>(
        &mut self,
        normal_drawable: &BTreeSet<ChunkPos>,
        normal_loaded: &BTreeSet<ChunkPos>,
        lod_tiles: I,
    ) -> LodCoverageResolution
    where
        I: IntoIterator<Item = LodTileAvailability>,
    {
        self.normal.rebuild(normal_drawable, normal_loaded);

        // Best (highest-rank drawable-preferring) availability per tile.
        let mut available: BTreeMap<ChunkPos, LodTileAvailability> = BTreeMap::new();
        for tile in lod_tiles {
            available
                .entry(tile.tile)
                .and_modify(|existing| {
                    if better_availability(&tile, existing) {
                        *existing = tile;
                    }
                })
                .or_insert(tile);
        }

        let considered: BTreeSet<ChunkPos> = self
            .tiles
            .keys()
            .copied()
            .chain(available.keys().copied())
            .collect();

        let mut frame = LodReplacementCounters::default();
        let mut visible_lod_tiles = BTreeSet::new();

        for pos in considered {
            let normal_state = self.normal.state(pos);
            let availability = available.get(&pos).copied();
            let desired = desired_source(normal_state, availability.as_ref());
            let prev = self
                .tiles
                .get(&pos)
                .map_or(VisibleSource::Nothing, |tile| tile.visible);
            let relevant = normal_drawable.contains(&pos) || normal_loaded.contains(&pos);

            record_transition(&mut frame, prev, desired, relevant);

            if let VisibleSource::Lod(_) = desired {
                visible_lod_tiles.insert(pos);
            }

            // Prune tiles that carry no representation and are no longer expected
            // on screen; keep everything else so recovery is observable.
            if matches!(desired, VisibleSource::Nothing) && availability.is_none() && !relevant {
                self.tiles.remove(&pos);
                continue;
            }

            let lod = availability
                .map(|availability| availability.state())
                .or_else(|| self.tiles.get(&pos).and_then(|tile| tile.lod));
            self.tiles.insert(
                pos,
                LodTileCoverage {
                    tile: pos,
                    lod,
                    visible: desired,
                },
            );
        }

        self.enforce_tracked_cap();
        self.total.accumulate(frame);

        LodCoverageResolution {
            visible_lod_tiles,
            frame,
            total: self.total,
            tracked_tiles: self.tiles.len(),
        }
    }

    /// Bound tracked-tile growth. Prefer dropping `Normal`-visible and
    /// `Nothing`-visible entries (no LOD geometry to lose) before entries that
    /// are actively drawn as LOD.
    fn enforce_tracked_cap(&mut self) {
        if self.tiles.len() <= MAX_TRACKED_LOD_COVERAGE_TILES {
            return;
        }
        let mut evictable: Vec<ChunkPos> = self
            .tiles
            .iter()
            .filter(|(_, tile)| !matches!(tile.visible, VisibleSource::Lod(_)))
            .map(|(pos, _)| *pos)
            .collect();
        let overflow = self.tiles.len() - MAX_TRACKED_LOD_COVERAGE_TILES;
        for pos in evictable.drain(..).take(overflow) {
            self.tiles.remove(&pos);
        }
    }
}

fn better_availability(candidate: &LodTileAvailability, existing: &LodTileAvailability) -> bool {
    // A drawable representation always beats a non-drawable one; otherwise the
    // higher-fidelity source wins.
    match (candidate.drawable, existing.drawable) {
        (true, false) => true,
        (false, true) => false,
        _ => candidate.source.precedence_rank() > existing.source.precedence_rank(),
    }
}

fn desired_source(
    normal: Option<NormalChunkDrawState>,
    lod: Option<&LodTileAvailability>,
) -> VisibleSource {
    // Only a drawable normal chunk suppresses LOD. Loaded-but-not-drawable keeps
    // old LOD visible until the real mesh is actually drawable.
    if matches!(normal, Some(NormalChunkDrawState::Drawable)) {
        return VisibleSource::Normal;
    }
    if let Some(lod) = lod {
        if lod.drawable {
            return VisibleSource::Lod(lod.source);
        }
    }
    VisibleSource::Nothing
}

fn record_transition(
    frame: &mut LodReplacementCounters,
    prev: VisibleSource,
    desired: VisibleSource,
    relevant: bool,
) {
    match (prev, desired) {
        (VisibleSource::Lod(_), VisibleSource::Normal) => frame.normal_over_lod += 1,
        (VisibleSource::Normal, VisibleSource::Lod(_)) => frame.lod_revealed_on_unload += 1,
        (VisibleSource::Lod(old), VisibleSource::Lod(new))
            if new.is_reduced_real() && !old.is_reduced_real() =>
        {
            frame.reduced_real_over_synthetic += 1;
        }
        (VisibleSource::Nothing, VisibleSource::Lod(_)) => frame.lod_first_drawn += 1,
        (VisibleSource::Lod(_) | VisibleSource::Normal, VisibleSource::Nothing) => {
            if relevant {
                frame.suppressed_without_replacement += 1;
            } else {
                frame.lod_evicted += 1;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunks(positions: &[(i32, i32)]) -> BTreeSet<ChunkPos> {
        positions
            .iter()
            .map(|(x, z)| ChunkPos::new(*x, *z))
            .collect()
    }

    #[test]
    fn synthetic_tile_first_draw_is_not_a_replacement() {
        let mut coordinator = LodCoverageCoordinator::new();
        let resolution = coordinator.resolve(
            &BTreeSet::new(),
            &BTreeSet::new(),
            [LodTileAvailability::synthetic(ChunkPos::new(0, 0))],
        );

        assert!(resolution.visible_lod_tiles.contains(&ChunkPos::new(0, 0)));
        assert_eq!(resolution.frame.lod_first_drawn, 1);
        assert_eq!(resolution.frame.normal_over_lod, 0);
        assert_eq!(
            coordinator
                .tile(ChunkPos::new(0, 0))
                .map(|tile| tile.visible),
            Some(VisibleSource::Lod(LodSourceKind::SyntheticSurface))
        );
    }

    #[test]
    fn drawable_normal_chunk_suppresses_overlapping_lod() {
        let mut coordinator = LodCoverageCoordinator::new();
        // Frame 1: synthetic LOD visible at the tile.
        coordinator.resolve(
            &BTreeSet::new(),
            &BTreeSet::new(),
            [LodTileAvailability::synthetic(ChunkPos::new(0, 0))],
        );

        // Frame 2: the real chunk becomes drawable and the synthetic cache no
        // longer offers a tile there.
        let resolution = coordinator.resolve(&chunks(&[(0, 0)]), &chunks(&[(0, 0)]), []);

        assert!(!resolution.visible_lod_tiles.contains(&ChunkPos::new(0, 0)));
        assert_eq!(resolution.frame.normal_over_lod, 1);
        assert_eq!(resolution.frame.suppressed_without_replacement, 0);
        assert_eq!(
            coordinator
                .tile(ChunkPos::new(0, 0))
                .map(|tile| tile.visible),
            Some(VisibleSource::Normal)
        );
    }

    #[test]
    fn loaded_but_not_drawable_normal_keeps_old_lod_visible() {
        let mut coordinator = LodCoverageCoordinator::new();
        coordinator.resolve(
            &BTreeSet::new(),
            &BTreeSet::new(),
            [LodTileAvailability::synthetic(ChunkPos::new(0, 0))],
        );

        // The real chunk is loaded but its mesh is not yet drawable, and the
        // synthetic tile is still available: old LOD must stay visible.
        let resolution = coordinator.resolve(
            &BTreeSet::new(),
            &chunks(&[(0, 0)]),
            [LodTileAvailability::synthetic(ChunkPos::new(0, 0))],
        );

        assert!(resolution.visible_lod_tiles.contains(&ChunkPos::new(0, 0)));
        assert_eq!(resolution.frame.normal_over_lod, 0);
        assert_eq!(resolution.frame.suppressed_without_replacement, 0);
    }

    #[test]
    fn chunk_unload_re_reveals_lod() {
        let mut coordinator = LodCoverageCoordinator::new();
        coordinator.resolve(
            &BTreeSet::new(),
            &BTreeSet::new(),
            [LodTileAvailability::synthetic(ChunkPos::new(0, 0))],
        );
        coordinator.resolve(&chunks(&[(0, 0)]), &chunks(&[(0, 0)]), []);

        // The chunk unloads; the retained synthetic tile becomes visible again.
        let resolution = coordinator.resolve(
            &BTreeSet::new(),
            &BTreeSet::new(),
            [LodTileAvailability::synthetic(ChunkPos::new(0, 0))],
        );

        assert!(resolution.visible_lod_tiles.contains(&ChunkPos::new(0, 0)));
        assert_eq!(resolution.frame.lod_revealed_on_unload, 1);
    }

    #[test]
    fn reduced_real_tile_replaces_synthetic() {
        let mut coordinator = LodCoverageCoordinator::new();
        coordinator.resolve(
            &BTreeSet::new(),
            &BTreeSet::new(),
            [LodTileAvailability::synthetic(ChunkPos::new(0, 0))],
        );

        let reduced = LodTileAvailability {
            tile: ChunkPos::new(0, 0),
            source: LodSourceKind::ReducedRealChunk,
            quality: LodTileQuality::Lit,
            drawable: true,
        };
        let resolution = coordinator.resolve(&BTreeSet::new(), &BTreeSet::new(), [reduced]);

        assert!(resolution.visible_lod_tiles.contains(&ChunkPos::new(0, 0)));
        assert_eq!(resolution.frame.reduced_real_over_synthetic, 1);
        assert_eq!(
            coordinator
                .tile(ChunkPos::new(0, 0))
                .map(|tile| tile.visible),
            Some(VisibleSource::Lod(LodSourceKind::ReducedRealChunk))
        );
    }

    #[test]
    fn reduced_real_wins_precedence_when_both_available() {
        let mut coordinator = LodCoverageCoordinator::new();
        let synthetic = LodTileAvailability::synthetic(ChunkPos::new(2, 3));
        let reduced = LodTileAvailability {
            tile: ChunkPos::new(2, 3),
            source: LodSourceKind::ReducedRealChunk,
            quality: LodTileQuality::Lit,
            drawable: true,
        };

        let resolution =
            coordinator.resolve(&BTreeSet::new(), &BTreeSet::new(), [synthetic, reduced]);

        assert_eq!(
            coordinator
                .tile(ChunkPos::new(2, 3))
                .map(|tile| tile.visible),
            Some(VisibleSource::Lod(LodSourceKind::ReducedRealChunk))
        );
        assert!(resolution.visible_lod_tiles.contains(&ChunkPos::new(2, 3)));
    }

    #[test]
    fn losing_lod_while_loaded_counts_as_pop() {
        let mut coordinator = LodCoverageCoordinator::new();
        coordinator.resolve(
            &BTreeSet::new(),
            &BTreeSet::new(),
            [LodTileAvailability::synthetic(ChunkPos::new(0, 0))],
        );

        // The chunk is loaded (still expected on screen) but not drawable, and the
        // synthetic tile vanished: a genuine blank with no replacement.
        let resolution = coordinator.resolve(&BTreeSet::new(), &chunks(&[(0, 0)]), []);

        assert!(!resolution.visible_lod_tiles.contains(&ChunkPos::new(0, 0)));
        assert_eq!(resolution.frame.suppressed_without_replacement, 1);
        assert_eq!(resolution.frame.lod_evicted, 0);
    }

    #[test]
    fn losing_lod_outside_coverage_is_benign_eviction() {
        let mut coordinator = LodCoverageCoordinator::new();
        coordinator.resolve(
            &BTreeSet::new(),
            &BTreeSet::new(),
            [LodTileAvailability::synthetic(ChunkPos::new(0, 0))],
        );

        // The tile is no longer offered and not loaded/drawable: it left coverage.
        let resolution = coordinator.resolve(&BTreeSet::new(), &BTreeSet::new(), []);

        assert_eq!(resolution.frame.lod_evicted, 1);
        assert_eq!(resolution.frame.suppressed_without_replacement, 0);
        assert!(coordinator.tile(ChunkPos::new(0, 0)).is_none());
    }

    #[test]
    fn cumulative_counters_accumulate_across_frames() {
        let mut coordinator = LodCoverageCoordinator::new();
        coordinator.resolve(
            &BTreeSet::new(),
            &BTreeSet::new(),
            [LodTileAvailability::synthetic(ChunkPos::new(0, 0))],
        );
        coordinator.resolve(&chunks(&[(0, 0)]), &chunks(&[(0, 0)]), []);
        let resolution = coordinator.resolve(
            &BTreeSet::new(),
            &BTreeSet::new(),
            [LodTileAvailability::synthetic(ChunkPos::new(0, 0))],
        );

        assert_eq!(resolution.total.lod_first_drawn, 1);
        assert_eq!(resolution.total.normal_over_lod, 1);
        assert_eq!(resolution.total.lod_revealed_on_unload, 1);
        assert_eq!(coordinator.counters(), resolution.total);
    }

    #[test]
    fn normal_coverage_tracks_drawable_separately_from_loaded() {
        let mut coordinator = LodCoverageCoordinator::new();
        coordinator.resolve(&chunks(&[(1, 1)]), &chunks(&[(1, 1), (2, 2)]), []);

        let coverage = coordinator.normal_coverage();
        assert_eq!(
            coverage.state(ChunkPos::new(1, 1)),
            Some(NormalChunkDrawState::Drawable)
        );
        assert_eq!(
            coverage.state(ChunkPos::new(2, 2)),
            Some(NormalChunkDrawState::Loaded)
        );
        assert!(coverage.is_drawable(ChunkPos::new(1, 1)));
        assert!(!coverage.is_drawable(ChunkPos::new(2, 2)));
        assert_eq!(coverage.drawable_count(), 1);
    }
}
