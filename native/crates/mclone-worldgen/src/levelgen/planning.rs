use std::collections::BTreeSet;

use mclone_core::{ChunkPos, ChunkStatus};

use crate::feature::{FEATURES_BLOCK_DEPENDENCY_RADIUS, FEATURES_WRITE_RADIUS_CUTOFF};

/// A chunk/status input that must be ready before a generation plan may run.
///
/// Requirements are inputs only. They do not become requested outputs merely
/// because a scheduler materializes them.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChunkStatusRequirement {
    pub pos: ChunkPos,
    pub status: ChunkStatus,
}

impl ChunkStatusRequirement {
    pub const fn new(pos: ChunkPos, status: ChunkStatus) -> Self {
        Self { pos, status }
    }
}

/// Pure, deterministic generation work requested for an admitted chunk batch.
///
/// The plan declares what the caller requested, where the generator performs
/// backend work, and which chunk/status inputs it needs. It deliberately owns
/// no scheduler priority, admission, readiness, publication, or persistence
/// policy.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChunkGenerationPlan {
    output_chunks: BTreeSet<ChunkPos>,
    backend_work_chunks: BTreeSet<ChunkPos>,
    prerequisites: BTreeSet<ChunkStatusRequirement>,
}

impl ChunkGenerationPlan {
    pub fn from_parts(
        output_chunks: impl IntoIterator<Item = ChunkPos>,
        backend_work_chunks: impl IntoIterator<Item = ChunkPos>,
        prerequisites: impl IntoIterator<Item = ChunkStatusRequirement>,
    ) -> Self {
        Self {
            output_chunks: output_chunks.into_iter().collect(),
            backend_work_chunks: backend_work_chunks.into_iter().collect(),
            prerequisites: prerequisites.into_iter().collect(),
        }
    }

    /// Current Overworld FEATURES footprint: requested outputs, the 3x3
    /// feature-center expansion, and the 5x5 Surface dependency expansion for
    /// a single target.
    pub fn overworld_features(targets: impl IntoIterator<Item = ChunkPos>) -> Self {
        let output_chunks = targets.into_iter().collect::<BTreeSet<_>>();
        let backend_work_chunks = expand_chunks(&output_chunks, FEATURES_WRITE_RADIUS_CUTOFF);
        let prerequisites = expand_chunks(&backend_work_chunks, FEATURES_BLOCK_DEPENDENCY_RADIUS)
            .into_iter()
            .map(|pos| ChunkStatusRequirement::new(pos, ChunkStatus::Surface))
            .collect::<BTreeSet<_>>();

        Self {
            output_chunks,
            backend_work_chunks,
            prerequisites,
        }
    }

    /// Target-only generators perform one independent unit of backend work per
    /// requested output and declare no neighboring chunk prerequisite.
    pub fn target_only(targets: impl IntoIterator<Item = ChunkPos>) -> Self {
        let output_chunks = targets.into_iter().collect::<BTreeSet<_>>();
        Self {
            backend_work_chunks: output_chunks.clone(),
            output_chunks,
            prerequisites: BTreeSet::new(),
        }
    }

    pub fn output_chunks(&self) -> &BTreeSet<ChunkPos> {
        &self.output_chunks
    }

    pub fn backend_work_chunks(&self) -> &BTreeSet<ChunkPos> {
        &self.backend_work_chunks
    }

    pub fn prerequisites(&self) -> &BTreeSet<ChunkStatusRequirement> {
        &self.prerequisites
    }

    pub fn into_parts(
        self,
    ) -> (
        BTreeSet<ChunkPos>,
        BTreeSet<ChunkPos>,
        BTreeSet<ChunkStatusRequirement>,
    ) {
        (
            self.output_chunks,
            self.backend_work_chunks,
            self.prerequisites,
        )
    }
}

fn expand_chunks(chunks: &BTreeSet<ChunkPos>, radius: i32) -> BTreeSet<ChunkPos> {
    let mut expanded = BTreeSet::new();
    for chunk in chunks {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                expanded.insert(ChunkPos::new(chunk.x + dx, chunk.z + dz));
            }
        }
    }
    expanded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(center: ChunkPos, radius: i32) -> BTreeSet<ChunkPos> {
        (-radius..=radius)
            .flat_map(|dz| {
                (-radius..=radius).map(move |dx| ChunkPos::new(center.x + dx, center.z + dz))
            })
            .collect()
    }

    fn surface_requirements(chunks: BTreeSet<ChunkPos>) -> BTreeSet<ChunkStatusRequirement> {
        chunks
            .into_iter()
            .map(|pos| ChunkStatusRequirement::new(pos, ChunkStatus::Surface))
            .collect()
    }

    #[test]
    fn overworld_single_target_has_exact_3x3_and_5x5_footprints() {
        let target = ChunkPos::new(0, 0);
        let plan = ChunkGenerationPlan::overworld_features([target]);

        assert_eq!(plan.output_chunks(), &BTreeSet::from([target]));
        assert_eq!(plan.backend_work_chunks(), &square(target, 1));
        assert_eq!(
            plan.prerequisites(),
            &surface_requirements(square(target, 2))
        );
    }

    #[test]
    fn overworld_contiguous_targets_reuse_overlapping_footprints() {
        let targets = square(ChunkPos::new(0, 0), 1);
        let plan = ChunkGenerationPlan::overworld_features(targets.clone());

        assert_eq!(plan.output_chunks(), &targets);
        assert_eq!(plan.backend_work_chunks(), &square(ChunkPos::new(0, 0), 2));
        assert_eq!(
            plan.prerequisites(),
            &surface_requirements(square(ChunkPos::new(0, 0), 3))
        );
    }

    #[test]
    fn overworld_negative_coordinates_expand_without_origin_assumptions() {
        let target = ChunkPos::new(-7, -11);
        let plan = ChunkGenerationPlan::overworld_features([target]);

        assert_eq!(plan.backend_work_chunks(), &square(target, 1));
        assert_eq!(
            plan.prerequisites(),
            &surface_requirements(square(target, 2))
        );
        assert!(plan.prerequisites().contains(&ChunkStatusRequirement::new(
            ChunkPos::new(-9, -13),
            ChunkStatus::Surface,
        )));
    }

    #[test]
    fn duplicate_and_reversed_targets_produce_the_same_plan() {
        let ordered = [
            ChunkPos::new(-2, 4),
            ChunkPos::new(0, 0),
            ChunkPos::new(3, -5),
        ];
        let duplicated = [ordered[2], ordered[1], ordered[0], ordered[1], ordered[2]];

        assert_eq!(
            ChunkGenerationPlan::overworld_features(ordered),
            ChunkGenerationPlan::overworld_features(duplicated)
        );
    }

    #[test]
    fn partitioned_overworld_plans_union_to_the_combined_plan() {
        let first = [ChunkPos::new(-4, 2), ChunkPos::new(-3, 2)];
        let second = [ChunkPos::new(7, -6), ChunkPos::new(8, -6)];
        let combined = ChunkGenerationPlan::overworld_features(first.into_iter().chain(second));
        let partitions = [
            ChunkGenerationPlan::overworld_features(first),
            ChunkGenerationPlan::overworld_features(second),
        ];

        let outputs = partitions
            .iter()
            .flat_map(|plan| plan.output_chunks().iter().copied())
            .collect::<BTreeSet<_>>();
        let work = partitions
            .iter()
            .flat_map(|plan| plan.backend_work_chunks().iter().copied())
            .collect::<BTreeSet<_>>();
        let prerequisites = partitions
            .iter()
            .flat_map(|plan| plan.prerequisites().iter().copied())
            .collect::<BTreeSet<_>>();

        assert_eq!(outputs, *combined.output_chunks());
        assert_eq!(work, *combined.backend_work_chunks());
        assert_eq!(prerequisites, *combined.prerequisites());
    }

    #[test]
    fn target_only_plan_has_no_implicit_neighbor_outputs_or_requirements() {
        let targets = [
            ChunkPos::new(5, -3),
            ChunkPos::new(5, -3),
            ChunkPos::new(-8, 13),
        ];
        let expected = BTreeSet::from([ChunkPos::new(5, -3), ChunkPos::new(-8, 13)]);
        let plan = ChunkGenerationPlan::target_only(targets);

        assert_eq!(plan.output_chunks(), &expected);
        assert_eq!(plan.backend_work_chunks(), &expected);
        assert!(plan.prerequisites().is_empty());
    }
}
