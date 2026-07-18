use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{ChunkPos, ChunkStatus};

use super::feature_batch::sorted_chunk_positions_z_major;
use super::{ChunkGenerationPlan, MutableChunkBlockBuffer};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct SurfaceDependencyCacheReport {
    pub requested_dependency_chunks: usize,
    pub cache_hits: usize,
    pub generated_dependency_chunks: usize,
    pub retained_dependency_chunks: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PreparedSurfaceDependencies {
    pub region_chunks: Vec<MutableChunkBlockBuffer>,
    pub retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    pub report: SurfaceDependencyCacheReport,
}

#[derive(Debug, Default)]
pub(super) struct SurfaceDependencyCache {
    seed: Option<i64>,
    chunks: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
}

impl SurfaceDependencyCache {
    pub fn retained_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn resident_positions(&self) -> BTreeSet<ChunkPos> {
        self.chunks.keys().copied().collect()
    }

    pub fn clear(&mut self) {
        self.seed = None;
        self.chunks.clear();
    }

    pub fn prepare(
        &mut self,
        seed: i64,
        plan: &ChunkGenerationPlan,
        dependencies: impl IntoIterator<Item = MutableChunkBlockBuffer>,
        mut generate: impl FnMut(ChunkPos) -> MutableChunkBlockBuffer,
    ) -> PreparedSurfaceDependencies {
        if self.seed != Some(seed) {
            self.seed = Some(seed);
            self.chunks.clear();
        }
        for dependency in dependencies {
            self.chunks.insert(
                ChunkPos::new(dependency.chunk_x, dependency.chunk_z),
                dependency,
            );
        }

        let mut report = SurfaceDependencyCacheReport {
            requested_dependency_chunks: plan.prerequisites().len(),
            ..SurfaceDependencyCacheReport::default()
        };
        let required_positions = plan
            .prerequisites()
            .iter()
            .map(|requirement| {
                debug_assert_eq!(requirement.status, ChunkStatus::Surface);
                requirement.pos
            })
            .collect::<BTreeSet<_>>();
        let mut region_chunks = Vec::with_capacity(required_positions.len());
        for pos in sorted_chunk_positions_z_major(required_positions.iter().copied()) {
            if let Some(chunk) = self.chunks.get(&pos) {
                report.cache_hits += 1;
                region_chunks.push(chunk.clone());
            } else {
                let chunk = generate(pos);
                self.chunks.insert(pos, chunk.clone());
                region_chunks.push(chunk);
                report.generated_dependency_chunks += 1;
            }
        }

        self.chunks
            .retain(|pos, _| required_positions.contains(pos));
        report.retained_dependency_chunks = self.chunks.len();
        PreparedSurfaceDependencies {
            region_chunks,
            retained_dependencies: self.chunks.clone(),
            report,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(pos: ChunkPos) -> MutableChunkBlockBuffer {
        MutableChunkBlockBuffer::new(pos.x, pos.z, 0, 256)
    }

    #[test]
    fn preparation_reuses_overlap_and_drops_inputs_outside_the_plan() {
        let mut cache = SurfaceDependencyCache::default();
        let first_plan = ChunkGenerationPlan::mclone_overworld_features([ChunkPos::new(0, 0)]);
        let first = cache.prepare(12_345, &first_plan, [buffer(ChunkPos::new(99, 99))], buffer);
        assert_eq!(first.report.requested_dependency_chunks, 25);
        assert_eq!(first.report.cache_hits, 0);
        assert_eq!(first.report.generated_dependency_chunks, 25);
        assert_eq!(first.report.retained_dependency_chunks, 25);
        assert!(
            !first
                .retained_dependencies
                .contains_key(&ChunkPos::new(99, 99))
        );

        let second_plan = ChunkGenerationPlan::small_island_features([ChunkPos::new(1, 0)]);
        let second = cache.prepare(12_345, &second_plan, std::iter::empty(), buffer);
        assert_eq!(second.report.requested_dependency_chunks, 25);
        assert_eq!(second.report.cache_hits, 20);
        assert_eq!(second.report.generated_dependency_chunks, 5);
        assert_eq!(second.report.retained_dependency_chunks, 25);
    }

    #[test]
    fn seed_change_and_empty_plan_reset_residency() {
        let mut cache = SurfaceDependencyCache::default();
        let plan = ChunkGenerationPlan::mclone_overworld_features([ChunkPos::new(0, 0)]);
        let _ = cache.prepare(1, &plan, std::iter::empty(), buffer);
        assert_eq!(cache.retained_chunk_count(), 25);

        let changed = cache.prepare(2, &plan, std::iter::empty(), buffer);
        assert_eq!(changed.report.cache_hits, 0);
        assert_eq!(changed.report.generated_dependency_chunks, 25);

        let empty = ChunkGenerationPlan::mclone_overworld_features(std::iter::empty());
        let prepared = cache.prepare(2, &empty, std::iter::empty(), buffer);
        assert!(prepared.region_chunks.is_empty());
        assert!(prepared.retained_dependencies.is_empty());
        assert_eq!(cache.retained_chunk_count(), 0);
    }
}
