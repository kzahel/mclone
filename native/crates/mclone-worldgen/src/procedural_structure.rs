use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use mclone_core::{CHUNK_WIDTH, ChunkPos, chunk_min_block_coord};

use crate::prng::WorldgenRandom;

pub const MAX_STRUCTURE_REFERENCE_RADIUS: u8 = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StructurePlacementTopology {
    Unbounded,
    PeriodicX { period_chunks: u32 },
}

impl StructurePlacementTopology {
    fn period_chunks_i32(self) -> Option<i32> {
        match self {
            Self::Unbounded => None,
            Self::PeriodicX { period_chunks } => Some(
                i32::try_from(period_chunks)
                    .expect("structure placement period must fit signed chunk coordinates"),
            ),
        }
    }

    pub fn canonical_chunk_x(self, chunk_x: i32) -> i32 {
        self.period_chunks_i32()
            .map_or(chunk_x, |period| chunk_x.rem_euclid(period))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructurePlacement {
    spacing: u32,
    separation: u32,
    salt: i32,
    reference_radius: u8,
}

impl StructurePlacement {
    pub fn new(
        spacing: u32,
        separation: u32,
        salt: i32,
        reference_radius: u8,
    ) -> Result<Self, ProceduralStructureError> {
        if spacing == 0 {
            return Err(ProceduralStructureError::ZeroSpacing);
        }
        if separation >= spacing {
            return Err(ProceduralStructureError::InvalidSeparation {
                spacing,
                separation,
            });
        }
        if reference_radius > MAX_STRUCTURE_REFERENCE_RADIUS {
            return Err(ProceduralStructureError::ReferenceRadiusTooLarge {
                radius: reference_radius,
            });
        }
        Ok(Self {
            spacing,
            separation,
            salt,
            reference_radius,
        })
    }

    pub const fn spacing(self) -> u32 {
        self.spacing
    }

    pub const fn separation(self) -> u32 {
        self.separation
    }

    pub const fn salt(self) -> i32 {
        self.salt
    }

    pub const fn reference_radius(self) -> u8 {
        self.reference_radius
    }

    pub fn validate_topology(
        self,
        topology: StructurePlacementTopology,
    ) -> Result<(), ProceduralStructureError> {
        let StructurePlacementTopology::PeriodicX { period_chunks } = topology else {
            return Ok(());
        };
        if period_chunks == 0 || period_chunks % self.spacing != 0 {
            return Err(ProceduralStructureError::IncompatiblePeriodicSpacing {
                spacing: self.spacing,
                period_chunks,
            });
        }
        Ok(())
    }

    pub fn potential_start_chunk(
        self,
        seed: i64,
        query: ChunkPos,
        topology: StructurePlacementTopology,
    ) -> Result<StructureStartCandidate, ProceduralStructureError> {
        self.validate_topology(topology)?;
        let spacing =
            i32::try_from(self.spacing).expect("validated structure spacing must fit i32");
        let separation =
            i32::try_from(self.separation).expect("validated structure separation must fit i32");
        let bound = spacing - separation;
        let canonical_query_x = topology.canonical_chunk_x(query.x);
        let region_x = canonical_query_x.div_euclid(spacing);
        let region_z = query.z.div_euclid(spacing);
        let mut random = WorldgenRandom::default();
        random.set_large_feature_with_salt(seed, region_x, region_z, self.salt);
        let canonical_start_x = region_x
            .checked_mul(spacing)
            .and_then(|value| value.checked_add(random.next_int_bound(bound)))
            .ok_or(ProceduralStructureError::CoordinateOverflow)?;
        let start_z = region_z
            .checked_mul(spacing)
            .and_then(|value| value.checked_add(random.next_int_bound(bound)))
            .ok_or(ProceduralStructureError::CoordinateOverflow)?;
        let work_start_x = match topology.period_chunks_i32() {
            None => canonical_start_x,
            Some(period) => lift_periodic_chunk_x(canonical_start_x, query.x, period),
        };
        Ok(StructureStartCandidate {
            canonical_start: ChunkPos::new(canonical_start_x, start_z),
            work_start: ChunkPos::new(work_start_x, start_z),
        })
    }

    pub fn candidate_starts_near(
        self,
        seed: i64,
        target: ChunkPos,
        topology: StructurePlacementTopology,
    ) -> Result<Vec<StructureStartCandidate>, ProceduralStructureError> {
        self.validate_topology(topology)?;
        let radius = i32::from(self.reference_radius);
        let mut candidates = BTreeMap::new();
        for chunk_z in target.z - radius..=target.z + radius {
            for chunk_x in target.x - radius..=target.x + radius {
                let query = ChunkPos::new(chunk_x, chunk_z);
                let candidate = self.potential_start_chunk(seed, query, topology)?;
                if candidate.work_start == query {
                    candidates.insert(candidate.canonical_start, candidate);
                }
            }
        }
        Ok(candidates.into_values().collect())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructureStartCandidate {
    pub canonical_start: ChunkPos,
    pub work_start: ChunkPos,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructureStartKey {
    pub structure_type: String,
    pub canonical_start: ChunkPos,
}

impl StructureStartKey {
    pub fn new(structure_type: impl Into<String>, canonical_start: ChunkPos) -> Self {
        Self {
            structure_type: structure_type.into(),
            canonical_start,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructureBoundingBox {
    pub min_x: i32,
    pub min_y: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_y: i32,
    pub max_z: i32,
}

impl StructureBoundingBox {
    pub fn new(
        min_x: i32,
        min_y: i32,
        min_z: i32,
        max_x: i32,
        max_y: i32,
        max_z: i32,
    ) -> Result<Self, ProceduralStructureError> {
        if min_x > max_x || min_y > max_y || min_z > max_z {
            return Err(ProceduralStructureError::InvalidBoundingBox);
        }
        Ok(Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        })
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            min_z: self.min_z.min(other.min_z),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
            max_z: self.max_z.max(other.max_z),
        }
    }

    pub fn intersects(self, other: Self) -> bool {
        self.max_x >= other.min_x
            && self.min_x <= other.max_x
            && self.max_y >= other.min_y
            && self.min_y <= other.max_y
            && self.max_z >= other.min_z
            && self.min_z <= other.max_z
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        self.intersects(other).then_some(Self {
            min_x: self.min_x.max(other.min_x),
            min_y: self.min_y.max(other.min_y),
            min_z: self.min_z.max(other.min_z),
            max_x: self.max_x.min(other.max_x),
            max_y: self.max_y.min(other.max_y),
            max_z: self.max_z.min(other.max_z),
        })
    }

    pub fn for_chunk(chunk: ChunkPos, min_y: i32, max_y: i32) -> Self {
        let min_x = chunk_min_block_coord(chunk.x);
        let min_z = chunk_min_block_coord(chunk.z);
        Self {
            min_x,
            min_y,
            min_z,
            max_x: min_x + CHUNK_WIDTH - 1,
            max_y,
            max_z: min_z + CHUNK_WIDTH - 1,
        }
    }

    pub fn intersects_chunk(self, chunk: ChunkPos) -> bool {
        let min_x = chunk_min_block_coord(chunk.x);
        let min_z = chunk_min_block_coord(chunk.z);
        self.max_x >= min_x
            && self.min_x < min_x + CHUNK_WIDTH
            && self.max_z >= min_z
            && self.min_z < min_z + CHUNK_WIDTH
    }

    pub fn translated_x(self, delta_x: i32) -> Result<Self, ProceduralStructureError> {
        Ok(Self {
            min_x: self
                .min_x
                .checked_add(delta_x)
                .ok_or(ProceduralStructureError::CoordinateOverflow)?,
            max_x: self
                .max_x
                .checked_add(delta_x)
                .ok_or(ProceduralStructureError::CoordinateOverflow)?,
            ..self
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProceduralStructurePiece<P> {
    pub ordinal: u32,
    pub kind: String,
    pub bounds: StructureBoundingBox,
    pub payload: P,
}

impl<P> ProceduralStructurePiece<P> {
    pub fn new(
        ordinal: u32,
        kind: impl Into<String>,
        bounds: StructureBoundingBox,
        payload: P,
    ) -> Self {
        Self {
            ordinal,
            kind: kind.into(),
            bounds,
            payload,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProceduralStructureStart<P> {
    pub key: StructureStartKey,
    pub work_start: ChunkPos,
    pub bounds: StructureBoundingBox,
    pieces: Vec<ProceduralStructurePiece<P>>,
}

impl<P> ProceduralStructureStart<P> {
    pub fn new(
        key: StructureStartKey,
        work_start: ChunkPos,
        mut pieces: Vec<ProceduralStructurePiece<P>>,
    ) -> Result<Self, ProceduralStructureError> {
        if pieces.is_empty() {
            return Err(ProceduralStructureError::EmptyStart);
        }
        pieces.sort_by_key(|piece| piece.ordinal);
        if pieces
            .windows(2)
            .any(|pair| pair[0].ordinal == pair[1].ordinal)
        {
            return Err(ProceduralStructureError::DuplicatePieceOrdinal);
        }
        let bounds = pieces
            .iter()
            .map(|piece| piece.bounds)
            .reduce(StructureBoundingBox::union)
            .expect("non-empty structure pieces must have aggregate bounds");
        Ok(Self {
            key,
            work_start,
            bounds,
            pieces,
        })
    }

    pub fn pieces(&self) -> &[ProceduralStructurePiece<P>] {
        &self.pieces
    }

    pub fn reference_for_target(
        &self,
        target: ChunkPos,
        reference_radius: u8,
    ) -> Option<StructureReference> {
        let dx = i64::from(target.x) - i64::from(self.work_start.x);
        let dz = i64::from(target.z) - i64::from(self.work_start.z);
        let inside_radius = dx.abs().max(dz.abs()) <= i64::from(reference_radius);
        (inside_radius && self.bounds.intersects_chunk(target)).then(|| StructureReference {
            target,
            start: self.key.clone(),
        })
    }

    pub fn clipped_pieces_for_target(
        &self,
        target: ChunkPos,
        min_y: i32,
        max_y: i32,
    ) -> Vec<ClippedStructurePiece<'_, P>> {
        let chunk_box = StructureBoundingBox::for_chunk(target, min_y, max_y);
        self.pieces
            .iter()
            .filter_map(|piece| {
                piece
                    .bounds
                    .intersection(chunk_box)
                    .map(|clip| ClippedStructurePiece { piece, clip })
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClippedStructurePiece<'a, P> {
    pub piece: &'a ProceduralStructurePiece<P>,
    pub clip: StructureBoundingBox,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructureReference {
    pub target: ChunkPos,
    pub start: StructureStartKey,
}

pub fn references_for_targets<'a, P: 'a>(
    targets: impl IntoIterator<Item = ChunkPos>,
    starts: impl IntoIterator<Item = &'a ProceduralStructureStart<P>>,
    reference_radius: u8,
) -> BTreeMap<ChunkPos, Vec<StructureReference>> {
    let mut starts = starts.into_iter().collect::<Vec<_>>();
    starts.sort_by(|left, right| left.key.cmp(&right.key));
    starts.dedup_by(|left, right| left.key == right.key);
    let targets = targets.into_iter().collect::<BTreeSet<_>>();
    targets
        .into_iter()
        .map(|target| {
            let references = starts
                .iter()
                .filter_map(|start| start.reference_for_target(target, reference_radius))
                .collect();
            (target, references)
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProceduralStructureError {
    ZeroSpacing,
    InvalidSeparation { spacing: u32, separation: u32 },
    ReferenceRadiusTooLarge { radius: u8 },
    IncompatiblePeriodicSpacing { spacing: u32, period_chunks: u32 },
    InvalidBoundingBox,
    EmptyStart,
    DuplicatePieceOrdinal,
    CoordinateOverflow,
}

impl fmt::Display for ProceduralStructureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroSpacing => write!(formatter, "structure spacing must be positive"),
            Self::InvalidSeparation {
                spacing,
                separation,
            } => write!(
                formatter,
                "structure separation {separation} must be smaller than spacing {spacing}"
            ),
            Self::ReferenceRadiusTooLarge { radius } => write!(
                formatter,
                "structure reference radius {radius} exceeds vanilla maximum {MAX_STRUCTURE_REFERENCE_RADIUS}"
            ),
            Self::IncompatiblePeriodicSpacing {
                spacing,
                period_chunks,
            } => write!(
                formatter,
                "structure spacing {spacing} must divide periodic circumference {period_chunks}"
            ),
            Self::InvalidBoundingBox => {
                write!(
                    formatter,
                    "structure bounding-box minima must not exceed maxima"
                )
            }
            Self::EmptyStart => {
                write!(formatter, "structure start must contain at least one piece")
            }
            Self::DuplicatePieceOrdinal => {
                write!(formatter, "structure piece ordinals must be unique")
            }
            Self::CoordinateOverflow => write!(formatter, "structure coordinate overflow"),
        }
    }
}

impl Error for ProceduralStructureError {}

fn lift_periodic_chunk_x(canonical_x: i32, reference_x: i32, period: i32) -> i32 {
    let delta = i64::from(reference_x) - i64::from(canonical_x);
    let period = i64::from(period);
    let lower_lift = delta.div_euclid(period);
    let lower = i64::from(canonical_x) + lower_lift * period;
    let upper = lower + period;
    let chosen = if (i64::from(reference_x) - lower).abs() <= (upper - i64::from(reference_x)).abs()
    {
        lower
    } else {
        upper
    };
    i32::try_from(chosen).expect("periodic structure lift must fit i32")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(
        min_x: i32,
        min_y: i32,
        min_z: i32,
        max_x: i32,
        max_y: i32,
        max_z: i32,
    ) -> StructureBoundingBox {
        StructureBoundingBox::new(min_x, min_y, min_z, max_x, max_y, max_z).unwrap()
    }

    #[test]
    fn placement_rejects_invalid_spacing_separation_radius_and_period() {
        assert_eq!(
            StructurePlacement::new(0, 0, 1, 8),
            Err(ProceduralStructureError::ZeroSpacing)
        );
        assert_eq!(
            StructurePlacement::new(8, 8, 1, 8),
            Err(ProceduralStructureError::InvalidSeparation {
                spacing: 8,
                separation: 8,
            })
        );
        assert_eq!(
            StructurePlacement::new(8, 2, 1, 9),
            Err(ProceduralStructureError::ReferenceRadiusTooLarge { radius: 9 })
        );
        assert_eq!(
            StructurePlacement::new(10, 2, 1, 8)
                .unwrap()
                .validate_topology(StructurePlacementTopology::PeriodicX { period_chunks: 384 }),
            Err(ProceduralStructureError::IncompatiblePeriodicSpacing {
                spacing: 10,
                period_chunks: 384,
            })
        );
    }

    #[test]
    fn random_spread_placement_is_pinned_across_signed_regions() {
        let placement = StructurePlacement::new(12, 4, 0x5354_524d, 8).unwrap();
        let starts = [
            ChunkPos::new(0, 0),
            ChunkPos::new(12, 12),
            ChunkPos::new(-1, -1),
            ChunkPos::new(-13, 25),
        ]
        .map(|query| {
            placement
                .potential_start_chunk(12_345, query, StructurePlacementTopology::Unbounded)
                .unwrap()
                .canonical_start
        });
        assert_eq!(
            starts,
            [
                ChunkPos::new(4, 2),
                ChunkPos::new(14, 12),
                ChunkPos::new(-5, -6),
                ChunkPos::new(-22, 29),
            ]
        );
    }

    #[test]
    fn periodic_candidates_share_canonical_identity_across_the_seam() {
        let placement = StructurePlacement::new(12, 4, 0x5354_524d, 8).unwrap();
        let topology = StructurePlacementTopology::PeriodicX { period_chunks: 384 };
        let left = placement
            .candidate_starts_near(-98_765, ChunkPos::new(-1, 7), topology)
            .unwrap();
        let right = placement
            .candidate_starts_near(-98_765, ChunkPos::new(383, 7), topology)
            .unwrap();
        assert_eq!(
            left.iter()
                .map(|candidate| candidate.canonical_start)
                .collect::<BTreeSet<_>>(),
            right
                .iter()
                .map(|candidate| candidate.canonical_start)
                .collect::<BTreeSet<_>>()
        );
        for candidate in left {
            assert!((candidate.work_start.x + 1).abs() <= 8);
        }
    }

    #[test]
    fn boxes_union_intersect_and_clip_inclusive_chunk_columns() {
        let first = bounds(-4, 60, 7, 20, 72, 18);
        let second = bounds(16, 62, 12, 48, 80, 30);
        assert_eq!(first.union(second), bounds(-4, 60, 7, 48, 80, 30));
        assert_eq!(
            first.intersection(StructureBoundingBox::for_chunk(ChunkPos::new(0, 0), 0, 255)),
            Some(bounds(0, 60, 7, 15, 72, 15))
        );
        assert!(first.intersects_chunk(ChunkPos::new(-1, 0)));
        assert!(first.intersects_chunk(ChunkPos::new(1, 1)));
        assert!(!first.intersects_chunk(ChunkPos::new(2, 0)));
    }

    #[test]
    fn starts_aggregate_ordered_pieces_and_reject_duplicate_ordinals() {
        let key = StructureStartKey::new("mclone:test", ChunkPos::new(0, 0));
        let pieces = vec![
            ProceduralStructurePiece::new(2, "tail", bounds(16, 60, 0, 31, 70, 15), 2_u8),
            ProceduralStructurePiece::new(1, "head", bounds(0, 60, 0, 15, 70, 15), 1_u8),
        ];
        let start =
            ProceduralStructureStart::new(key.clone(), ChunkPos::new(0, 0), pieces).unwrap();
        assert_eq!(start.bounds, bounds(0, 60, 0, 31, 70, 15));
        assert_eq!(
            start
                .pieces()
                .iter()
                .map(|piece| piece.ordinal)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            ProceduralStructureStart::new(
                key,
                ChunkPos::new(0, 0),
                vec![
                    ProceduralStructurePiece::new(1, "first", bounds(0, 0, 0, 0, 0, 0), (),),
                    ProceduralStructurePiece::new(1, "duplicate", bounds(1, 0, 0, 1, 0, 0), (),),
                ],
            ),
            Err(ProceduralStructureError::DuplicatePieceOrdinal)
        );
    }

    #[test]
    fn references_and_clipped_pieces_are_target_partition_independent() {
        let start = ProceduralStructureStart::new(
            StructureStartKey::new("mclone:test_stream", ChunkPos::new(0, 0)),
            ChunkPos::new(0, 0),
            vec![
                ProceduralStructurePiece::new(0, "headwater", bounds(-6, 64, 2, 10, 74, 18), ()),
                ProceduralStructurePiece::new(1, "reach", bounds(8, 60, 8, 39, 72, 22), ()),
                ProceduralStructurePiece::new(2, "sink", bounds(36, 58, 12, 51, 68, 27), ()),
            ],
        )
        .unwrap();
        let targets = [
            ChunkPos::new(-1, 0),
            ChunkPos::new(0, 0),
            ChunkPos::new(1, 0),
            ChunkPos::new(2, 1),
            ChunkPos::new(3, 1),
        ];
        let ordered = references_for_targets(targets, [&start], 8);
        let reversed = references_for_targets(targets.into_iter().rev(), [&start], 8);
        assert_eq!(ordered, reversed);
        let split = references_for_targets(targets[..2].iter().copied(), [&start], 8)
            .into_iter()
            .chain(references_for_targets(
                targets[2..].iter().copied(),
                [&start],
                8,
            ))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(ordered, split);
        assert!(ordered.values().all(|references| references.len() == 1));
        assert_eq!(
            start
                .clipped_pieces_for_target(ChunkPos::new(1, 0), 0, 255)
                .iter()
                .map(|clipped| (clipped.piece.ordinal, clipped.clip))
                .collect::<Vec<_>>(),
            vec![(1, bounds(16, 60, 8, 31, 72, 15))]
        );
    }

    #[test]
    fn references_respect_the_vanilla_radius_cap() {
        let start = ProceduralStructureStart::new(
            StructureStartKey::new("mclone:test_long", ChunkPos::new(0, 0)),
            ChunkPos::new(0, 0),
            vec![ProceduralStructurePiece::new(
                0,
                "long",
                bounds(0, 60, 0, 159, 70, 15),
                (),
            )],
        )
        .unwrap();
        assert!(start.reference_for_target(ChunkPos::new(8, 0), 8).is_some());
        assert!(start.reference_for_target(ChunkPos::new(9, 0), 8).is_none());
    }
}
