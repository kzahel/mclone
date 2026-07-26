use std::collections::{BTreeMap, BTreeSet};

use mclone_core::ChunkPos;
use mclone_worldgen::terrain_preview::TerrainPreviewProfile;

pub const TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS: u32 = 64;
pub const TERRAIN_EXACT_COVERAGE_WORD_COUNT: usize = (TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS
    as usize
    * TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS as usize)
    / u32::BITS as usize;
pub const TERRAIN_EXACT_COVERAGE_MASK_BYTES: u64 =
    (TERRAIN_EXACT_COVERAGE_WORD_COUNT * size_of::<u32>()) as u64;
pub const TERRAIN_EXACT_FRONTIER_COLLAR_BLOCKS: f32 = 1.5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundedRepresentationBounds {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

impl BoundedRepresentationBounds {
    pub fn new(min: [i32; 3], max: [i32; 3]) -> Result<Self, String> {
        if min
            .into_iter()
            .zip(max)
            .any(|(minimum, maximum)| minimum > maximum)
        {
            return Err("bounded representation has inverted bounds".to_owned());
        }
        Ok(Self { min, max })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BoundedRepresentationReadiness {
    pub exact_drawable: bool,
    pub approximate_drawable: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BoundedRepresentationOwner {
    #[default]
    Approximate,
    Exact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedRepresentationUnit<I> {
    pub id: I,
    pub bounds: BoundedRepresentationBounds,
    pub readiness: BoundedRepresentationReadiness,
    pub owner: BoundedRepresentationOwner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedRepresentationOwnershipSnapshot<I> {
    source: TerrainCompositionSourceIdentity,
    generation: u64,
    units: BTreeMap<I, BoundedRepresentationUnit<I>>,
}

impl<I> BoundedRepresentationOwnershipSnapshot<I>
where
    I: Clone + Ord,
{
    pub fn new(
        source: TerrainCompositionSourceIdentity,
        generation: u64,
        units: impl IntoIterator<Item = BoundedRepresentationUnit<I>>,
    ) -> Result<Self, String> {
        if generation == 0 {
            return Err("bounded representation generation must be non-zero".to_owned());
        }
        let mut indexed = BTreeMap::new();
        for unit in units {
            let selected_drawable = match unit.owner {
                BoundedRepresentationOwner::Approximate => unit.readiness.approximate_drawable,
                BoundedRepresentationOwner::Exact => unit.readiness.exact_drawable,
            };
            if !selected_drawable {
                return Err("bounded representation selected a non-drawable owner".to_owned());
            }
            if indexed.insert(unit.id.clone(), unit).is_some() {
                return Err("bounded representation snapshot contains a duplicate unit".to_owned());
            }
        }
        Ok(Self {
            source,
            generation,
            units: indexed,
        })
    }

    pub const fn source(&self) -> TerrainCompositionSourceIdentity {
        self.source
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn units(&self) -> &BTreeMap<I, BoundedRepresentationUnit<I>> {
        &self.units
    }

    pub fn owner(&self, id: &I) -> Option<BoundedRepresentationOwner> {
        self.units.get(id).map(|unit| unit.owner)
    }

    pub fn exact_owned_ids(&self) -> impl Iterator<Item = &I> {
        self.units
            .values()
            .filter(|unit| unit.owner == BoundedRepresentationOwner::Exact)
            .map(|unit| &unit.id)
    }

    pub fn approximate_owned_ids(&self) -> impl Iterator<Item = &I> {
        self.units
            .values()
            .filter(|unit| unit.owner == BoundedRepresentationOwner::Approximate)
            .map(|unit| &unit.id)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TerrainCompositionSourceIdentity {
    pub profile: TerrainPreviewProfile,
    pub seed: i64,
}

impl TerrainCompositionSourceIdentity {
    pub const fn new(profile: TerrainPreviewProfile, seed: i64) -> Self {
        Self { profile, seed }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u32)]
pub enum TerrainExactCoverageMode {
    #[default]
    Disabled = 0,
    DiscardPainted = 1,
    VisualizePainted = 2,
}

impl TerrainExactCoverageMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::DiscardPainted => "discard-painted",
            Self::VisualizePainted => "visualize-painted",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactPaintedCoverageSnapshot {
    source: TerrainCompositionSourceIdentity,
    generation: u64,
    chunks: BTreeSet<ChunkPos>,
}

impl ExactPaintedCoverageSnapshot {
    pub fn new(
        source: TerrainCompositionSourceIdentity,
        generation: u64,
        chunks: impl IntoIterator<Item = ChunkPos>,
    ) -> Result<Self, String> {
        if generation == 0 {
            return Err("exact-painted coverage generation must be non-zero".to_owned());
        }
        let snapshot = Self {
            source,
            generation,
            chunks: chunks.into_iter().collect(),
        };
        snapshot.packed_mask()?;
        Ok(snapshot)
    }

    pub const fn source(&self) -> TerrainCompositionSourceIdentity {
        self.source
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn chunks(&self) -> &BTreeSet<ChunkPos> {
        &self.chunks
    }

    pub fn contains(&self, chunk: ChunkPos) -> bool {
        self.chunks.contains(&chunk)
    }

    pub fn contains_exact_safe_horizontal_bounds(
        &self,
        bounds: BoundedRepresentationBounds,
        collar_blocks: f32,
    ) -> Result<bool, String> {
        if !collar_blocks.is_finite() || !(0.0..8.0).contains(&collar_blocks) {
            return Err(
                "exact-safe representation collar must be finite and in [0, 8) blocks".to_owned(),
            );
        }
        let min_chunk_x = bounds.min[0].div_euclid(16);
        let max_chunk_x = bounds.max[0].div_euclid(16);
        let min_chunk_z = bounds.min[2].div_euclid(16);
        let max_chunk_z = bounds.max[2].div_euclid(16);
        for chunk_z in min_chunk_z..=max_chunk_z {
            for chunk_x in min_chunk_x..=max_chunk_x {
                let chunk = ChunkPos::new(chunk_x, chunk_z);
                if !self.contains(chunk) {
                    return Ok(false);
                }
                let chunk_min_x = chunk_x.saturating_mul(16);
                let chunk_min_z = chunk_z.saturating_mul(16);
                let local_min_x = bounds.min[0].max(chunk_min_x) - chunk_min_x;
                let local_max_x = bounds.max[0].min(chunk_min_x.saturating_add(15)) - chunk_min_x;
                let local_min_z = bounds.min[2].max(chunk_min_z) - chunk_min_z;
                let local_max_z = bounds.max[2].min(chunk_min_z.saturating_add(15)) - chunk_min_z;
                if (local_min_x as f32) < collar_blocks
                    && !self.contains(ChunkPos::new(chunk_x.saturating_sub(1), chunk_z))
                {
                    return Ok(false);
                }
                if (local_max_x + 1) as f32 > 16.0 - collar_blocks
                    && !self.contains(ChunkPos::new(chunk_x.saturating_add(1), chunk_z))
                {
                    return Ok(false);
                }
                if (local_min_z as f32) < collar_blocks
                    && !self.contains(ChunkPos::new(chunk_x, chunk_z.saturating_sub(1)))
                {
                    return Ok(false);
                }
                if (local_max_z + 1) as f32 > 16.0 - collar_blocks
                    && !self.contains(ChunkPos::new(chunk_x, chunk_z.saturating_add(1)))
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    pub fn packed_mask(&self) -> Result<TerrainExactCoverageMask, String> {
        TerrainExactCoverageMask::pack(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainExactCoverageMask {
    pub source: TerrainCompositionSourceIdentity,
    pub generation: u64,
    pub origin_chunk_x: i32,
    pub origin_chunk_z: i32,
    pub width: u32,
    pub height: u32,
    pub painted_chunks: u32,
    words: [u32; TERRAIN_EXACT_COVERAGE_WORD_COUNT],
}

impl TerrainExactCoverageMask {
    fn pack(snapshot: &ExactPaintedCoverageSnapshot) -> Result<Self, String> {
        let Some(min_chunk_x) = snapshot.chunks.iter().map(|chunk| chunk.x).min() else {
            return Ok(Self {
                source: snapshot.source,
                generation: snapshot.generation,
                origin_chunk_x: 0,
                origin_chunk_z: 0,
                width: 0,
                height: 0,
                painted_chunks: 0,
                words: [0; TERRAIN_EXACT_COVERAGE_WORD_COUNT],
            });
        };
        let max_chunk_x = snapshot
            .chunks
            .iter()
            .map(|chunk| chunk.x)
            .max()
            .expect("non-empty exact-painted chunks have a maximum X");
        let min_chunk_z = snapshot
            .chunks
            .iter()
            .map(|chunk| chunk.z)
            .min()
            .expect("non-empty exact-painted chunks have a minimum Z");
        let max_chunk_z = snapshot
            .chunks
            .iter()
            .map(|chunk| chunk.z)
            .max()
            .expect("non-empty exact-painted chunks have a maximum Z");
        let width = u32::try_from(i64::from(max_chunk_x) - i64::from(min_chunk_x) + 1)
            .map_err(|_| "exact-painted coverage width exceeds u32".to_owned())?;
        let height = u32::try_from(i64::from(max_chunk_z) - i64::from(min_chunk_z) + 1)
            .map_err(|_| "exact-painted coverage height exceeds u32".to_owned())?;
        if width > TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS
            || height > TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS
        {
            return Err(format!(
                "exact-painted coverage spans {width}x{height} chunks, exceeding the fixed \
                 {}x{} mask",
                TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS,
                TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS
            ));
        }

        let mut mask =
            Self {
                source: snapshot.source,
                generation: snapshot.generation,
                origin_chunk_x: min_chunk_x,
                origin_chunk_z: min_chunk_z,
                width,
                height,
                painted_chunks: snapshot.chunks.len().try_into().map_err(|_| {
                    "exact-painted coverage contains more than u32 chunks".to_owned()
                })?,
                words: [0; TERRAIN_EXACT_COVERAGE_WORD_COUNT],
            };
        for chunk in &snapshot.chunks {
            let local_x =
                u32::try_from(chunk.x - min_chunk_x).expect("packed exact chunk X is non-negative");
            let local_z =
                u32::try_from(chunk.z - min_chunk_z).expect("packed exact chunk Z is non-negative");
            let bit = local_z * TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS + local_x;
            mask.words[(bit / u32::BITS) as usize] |= 1 << (bit % u32::BITS);
        }
        Ok(mask)
    }

    pub fn contains(&self, chunk: ChunkPos) -> bool {
        let local_x = i64::from(chunk.x) - i64::from(self.origin_chunk_x);
        let local_z = i64::from(chunk.z) - i64::from(self.origin_chunk_z);
        if local_x < 0
            || local_z < 0
            || local_x >= i64::from(self.width)
            || local_z >= i64::from(self.height)
        {
            return false;
        }
        let bit = local_z as u32 * TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS + local_x as u32;
        self.words[(bit / u32::BITS) as usize] & (1 << (bit % u32::BITS)) != 0
    }

    pub const fn words(&self) -> &[u32; TERRAIN_EXACT_COVERAGE_WORD_COUNT] {
        &self.words
    }

    pub fn word_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(TERRAIN_EXACT_COVERAGE_MASK_BYTES as usize);
        for word in self.words {
            bytes.extend_from_slice(&word.to_ne_bytes());
        }
        bytes
    }

    pub fn uniform_bytes(&self, mode: TerrainExactCoverageMode) -> [u8; 32] {
        let mut bytes = [0; 32];
        for (index, value) in [
            self.origin_chunk_x,
            self.origin_chunk_z,
            self.width as i32,
            self.height as i32,
            mode as i32,
            self.painted_chunks as i32,
            self.generation as u32 as i32,
            (self.generation >> 32) as u32 as i32,
        ]
        .into_iter()
        .enumerate()
        {
            let start = index * size_of::<i32>();
            bytes[start..start + size_of::<i32>()].copy_from_slice(&value.to_ne_bytes());
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> TerrainCompositionSourceIdentity {
        TerrainCompositionSourceIdentity::new(TerrainPreviewProfile::McloneOverworldV1, 12_345)
    }

    fn bounds(min_x: i32, min_z: i32, max_x: i32, max_z: i32) -> BoundedRepresentationBounds {
        BoundedRepresentationBounds::new([min_x, 60, min_z], [max_x, 80, max_z]).unwrap()
    }

    #[test]
    fn bounded_ownership_requires_unique_units_and_a_drawable_owner() {
        let exact = BoundedRepresentationUnit {
            id: 7_u32,
            bounds: bounds(0, 0, 1, 1),
            readiness: BoundedRepresentationReadiness {
                exact_drawable: true,
                approximate_drawable: true,
            },
            owner: BoundedRepresentationOwner::Exact,
        };
        let snapshot =
            BoundedRepresentationOwnershipSnapshot::new(source(), 3, [exact.clone()]).unwrap();
        assert_eq!(snapshot.owner(&7), Some(BoundedRepresentationOwner::Exact));
        assert_eq!(snapshot.exact_owned_ids().copied().collect::<Vec<_>>(), [7]);
        assert!(snapshot.approximate_owned_ids().next().is_none());

        assert_eq!(
            BoundedRepresentationOwnershipSnapshot::new(source(), 0, [exact.clone()]).unwrap_err(),
            "bounded representation generation must be non-zero"
        );
        assert_eq!(
            BoundedRepresentationOwnershipSnapshot::new(source(), 1, [exact.clone(), exact])
                .unwrap_err(),
            "bounded representation snapshot contains a duplicate unit"
        );
        assert_eq!(
            BoundedRepresentationOwnershipSnapshot::new(
                source(),
                1,
                [BoundedRepresentationUnit {
                    id: 8_u32,
                    bounds: bounds(0, 0, 1, 1),
                    readiness: BoundedRepresentationReadiness {
                        exact_drawable: false,
                        approximate_drawable: true,
                    },
                    owner: BoundedRepresentationOwner::Exact,
                }],
            )
            .unwrap_err(),
            "bounded representation selected a non-drawable owner"
        );
    }

    #[test]
    fn exact_safe_bounds_exclude_the_procedural_collar() {
        let snapshot = ExactPaintedCoverageSnapshot::new(
            source(),
            1,
            [
                ChunkPos::new(0, 0),
                ChunkPos::new(1, 0),
                ChunkPos::new(0, 1),
                ChunkPos::new(1, 1),
            ],
        )
        .unwrap();
        assert!(
            snapshot
                .contains_exact_safe_horizontal_bounds(bounds(2, 2, 29, 29), 1.5)
                .unwrap()
        );
        assert!(
            !snapshot
                .contains_exact_safe_horizontal_bounds(bounds(1, 2, 29, 29), 1.5)
                .unwrap()
        );
        assert!(
            !snapshot
                .contains_exact_safe_horizontal_bounds(bounds(2, 2, 30, 29), 1.5)
                .unwrap()
        );
        assert!(
            !snapshot
                .contains_exact_safe_horizontal_bounds(bounds(2, 2, 32, 29), 1.5)
                .unwrap()
        );
    }

    #[test]
    fn exact_safe_bounds_use_floor_chunks_at_negative_coordinates() {
        let snapshot = ExactPaintedCoverageSnapshot::new(
            source(),
            1,
            [
                ChunkPos::new(-2, -2),
                ChunkPos::new(-1, -2),
                ChunkPos::new(-2, -1),
                ChunkPos::new(-1, -1),
            ],
        )
        .unwrap();
        assert!(
            snapshot
                .contains_exact_safe_horizontal_bounds(bounds(-30, -30, -3, -3), 1.5)
                .unwrap()
        );
        assert!(
            !snapshot
                .contains_exact_safe_horizontal_bounds(bounds(-31, -30, -3, -3), 1.5)
                .unwrap()
        );
        assert!(
            !snapshot
                .contains_exact_safe_horizontal_bounds(bounds(-30, -30, -2, -3), 1.5)
                .unwrap()
        );
    }

    #[test]
    fn exact_painted_mask_preserves_sparse_negative_chunks() {
        let chunks = [
            ChunkPos::new(-3, -2),
            ChunkPos::new(-1, -2),
            ChunkPos::new(-2, 0),
        ];
        let snapshot = ExactPaintedCoverageSnapshot::new(source(), 7, chunks).unwrap();
        let mask = snapshot.packed_mask().unwrap();
        assert_eq!(mask.origin_chunk_x, -3);
        assert_eq!(mask.origin_chunk_z, -2);
        assert_eq!((mask.width, mask.height), (3, 3));
        assert_eq!(mask.painted_chunks, 3);
        for chunk in chunks {
            assert!(snapshot.contains(chunk));
            assert!(mask.contains(chunk));
        }
        assert!(!mask.contains(ChunkPos::new(-2, -2)));
        assert!(!mask.contains(ChunkPos::new(0, 0)));
    }

    #[test]
    fn empty_exact_painted_snapshot_produces_disabled_extent() {
        let snapshot = ExactPaintedCoverageSnapshot::new(source(), 1, []).unwrap();
        let mask = snapshot.packed_mask().unwrap();
        assert_eq!((mask.width, mask.height, mask.painted_chunks), (0, 0, 0));
        assert!(mask.words().iter().all(|word| *word == 0));
    }

    #[test]
    fn exact_painted_mask_rejects_unbounded_span() {
        let error = ExactPaintedCoverageSnapshot::new(
            source(),
            1,
            [
                ChunkPos::new(0, 0),
                ChunkPos::new(TERRAIN_EXACT_COVERAGE_MAX_CHUNKS_PER_AXIS as i32, 0),
            ],
        )
        .unwrap_err();
        assert!(error.contains("exceeding the fixed"));
    }

    #[test]
    fn exact_painted_generation_is_never_zero() {
        assert_eq!(
            ExactPaintedCoverageSnapshot::new(source(), 0, []).unwrap_err(),
            "exact-painted coverage generation must be non-zero"
        );
    }

    #[test]
    fn packed_uniform_retains_mode_and_generation() {
        let snapshot = ExactPaintedCoverageSnapshot::new(
            source(),
            0x0123_4567_89ab_cdef,
            [ChunkPos::new(-1, 4)],
        )
        .unwrap();
        let uniform = snapshot
            .packed_mask()
            .unwrap()
            .uniform_bytes(TerrainExactCoverageMode::VisualizePainted);
        let words = uniform
            .chunks_exact(4)
            .map(|bytes| i32::from_ne_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(words[0..6], [-1, 4, 1, 1, 2, 1]);
        assert_eq!(words[6] as u32, 0x89ab_cdef);
        assert_eq!(words[7] as u32, 0x0123_4567);
    }
}
