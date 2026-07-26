use mclone_worldgen::levelgen::{McloneTreeId, McloneTreeOccurrence};

use super::{
    BoundedRepresentationBounds, BoundedRepresentationOwner,
    BoundedRepresentationOwnershipSnapshot, BoundedRepresentationReadiness,
    BoundedRepresentationUnit, ExactPaintedCoverageSnapshot, TerrainCompositionSourceIdentity,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct McloneTreeOccurrenceId {
    pub tree: McloneTreeId,
    pub x_lift: i64,
}

impl McloneTreeOccurrenceId {
    pub const fn new(tree: McloneTreeId, x_lift: i64) -> Self {
        Self { tree, x_lift }
    }
}

impl From<McloneTreeOccurrence> for McloneTreeOccurrenceId {
    fn from(occurrence: McloneTreeOccurrence) -> Self {
        Self::new(occurrence.record.id, occurrence.x_lift)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McloneTreeOwnershipCandidate {
    pub occurrence: McloneTreeOccurrence,
    pub exact_drawable: bool,
    pub proxy_drawable: bool,
}

impl McloneTreeOwnershipCandidate {
    pub const fn new(
        occurrence: McloneTreeOccurrence,
        exact_drawable: bool,
        proxy_drawable: bool,
    ) -> Self {
        Self {
            occurrence,
            exact_drawable,
            proxy_drawable,
        }
    }
}

pub fn mclone_tree_working_bounds(occurrence: McloneTreeOccurrence) -> BoundedRepresentationBounds {
    let bounds = occurrence.working_bounds;
    BoundedRepresentationBounds {
        min: [bounds.min_x, bounds.min_y, bounds.min_z],
        max: [bounds.max_x, bounds.max_y, bounds.max_z],
    }
}

pub fn mclone_tree_ownership_snapshot(
    source: TerrainCompositionSourceIdentity,
    generation: u64,
    terrain_coverage: &ExactPaintedCoverageSnapshot,
    collar_blocks: f32,
    candidates: impl IntoIterator<Item = McloneTreeOwnershipCandidate>,
) -> Result<BoundedRepresentationOwnershipSnapshot<McloneTreeOccurrenceId>, String> {
    if terrain_coverage.source() != source {
        return Err("tree ownership source does not match exact terrain coverage".to_owned());
    }
    let mut units = Vec::new();
    for candidate in candidates {
        let bounds = mclone_tree_working_bounds(candidate.occurrence);
        let exact_safe =
            terrain_coverage.contains_exact_safe_horizontal_bounds(bounds, collar_blocks)?;
        let owner = if exact_safe && candidate.exact_drawable {
            BoundedRepresentationOwner::Exact
        } else if candidate.proxy_drawable {
            BoundedRepresentationOwner::Approximate
        } else {
            return Err(
                "tree ownership has neither an exact-safe draw nor a drawable proxy".to_owned(),
            );
        };
        units.push(BoundedRepresentationUnit {
            id: candidate.occurrence.into(),
            bounds,
            readiness: BoundedRepresentationReadiness {
                exact_drawable: candidate.exact_drawable,
                approximate_drawable: candidate.proxy_drawable,
            },
            owner,
        });
    }
    BoundedRepresentationOwnershipSnapshot::new(source, generation, units)
}

#[cfg(test)]
mod tests {
    use mclone_worldgen::levelgen::{
        McloneTreeArchetype, McloneTreeBounds, McloneTreeFamily, McloneTreeRecord,
    };
    use mclone_worldgen::placement::BlockPos;
    use mclone_worldgen::terrain_preview::TerrainPreviewProfile;

    use super::*;

    fn source() -> TerrainCompositionSourceIdentity {
        TerrainCompositionSourceIdentity::new(TerrainPreviewProfile::McloneOverworldV1, 7)
    }

    fn occurrence(
        slot: u8,
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
    ) -> McloneTreeOccurrence {
        let bounds = McloneTreeBounds::new(min_x, 63, min_z, max_x, 75, max_z).unwrap();
        McloneTreeOccurrence {
            record: McloneTreeRecord {
                id: McloneTreeId {
                    planning_cell_x: 0,
                    planning_cell_z: 0,
                    candidate_slot: slot,
                    vegetation_revision: 2,
                },
                canonical_base: BlockPos::new(min_x, 64, min_z),
                family: McloneTreeFamily::TemperateBroadleaf,
                archetype: McloneTreeArchetype::RoundedBroadleaf,
                trunk_height: 7,
                crown_radius: 3,
                crown_depth: 5,
                orientation: 0,
                landmark_rank: 0,
                variant_seed: 11,
                bounds,
            },
            x_lift: 0,
            working_bounds: bounds,
        }
    }

    #[test]
    fn complete_bounds_choose_exact_or_proxy_without_fragment_ownership() {
        let coverage = ExactPaintedCoverageSnapshot::new(
            source(),
            9,
            [
                mclone_core::ChunkPos::new(0, 0),
                mclone_core::ChunkPos::new(1, 0),
                mclone_core::ChunkPos::new(0, 1),
                mclone_core::ChunkPos::new(1, 1),
            ],
        )
        .unwrap();
        let interior = occurrence(1, 3, 3, 10, 10);
        let crossing = occurrence(2, 1, 3, 7, 10);
        let snapshot = mclone_tree_ownership_snapshot(
            source(),
            4,
            &coverage,
            1.5,
            [
                McloneTreeOwnershipCandidate::new(interior, true, true),
                McloneTreeOwnershipCandidate::new(crossing, true, true),
            ],
        )
        .unwrap();
        assert_eq!(
            snapshot.owner(&interior.into()),
            Some(BoundedRepresentationOwner::Exact)
        );
        assert_eq!(
            snapshot.owner(&crossing.into()),
            Some(BoundedRepresentationOwner::Approximate)
        );
    }

    #[test]
    fn unsafe_tree_requires_a_complete_proxy() {
        let coverage =
            ExactPaintedCoverageSnapshot::new(source(), 1, [mclone_core::ChunkPos::new(0, 0)])
                .unwrap();
        let crossing = occurrence(3, 1, 3, 7, 10);
        assert_eq!(
            mclone_tree_ownership_snapshot(
                source(),
                1,
                &coverage,
                1.5,
                [McloneTreeOwnershipCandidate::new(crossing, true, false)],
            )
            .unwrap_err(),
            "tree ownership has neither an exact-safe draw nor a drawable proxy"
        );
    }
}
