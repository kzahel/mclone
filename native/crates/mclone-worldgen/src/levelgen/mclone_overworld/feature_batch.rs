use std::collections::BTreeMap;

use mclone_core::{ChunkPos, chunk_min_block_coord};

use crate::feature::FeatureRegion;
use crate::levelgen::feature_batch::sorted_chunk_positions_z_major;
use crate::levelgen::surface_dependency_cache::{
    PreparedSurfaceDependencies, SurfaceDependencyCache, SurfaceDependencyCacheReport,
};
use crate::levelgen::{ChunkGenerationPlan, GeneratedChunk, MutableChunkBlockBuffer};

use super::decoration::decorate_mclone_overworld_center;
use super::terrain::{generate_mclone_overworld_surface_buffer, mclone_overworld_chunk_biomes};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McloneOverworldFeatureDependencyCacheReport {
    pub requested_dependency_chunks: usize,
    pub cache_hits: usize,
    pub generated_dependency_chunks: usize,
    pub retained_dependency_chunks: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McloneOverworldFeatureBatchResult {
    pub chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    pub retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    pub cache_report: McloneOverworldFeatureDependencyCacheReport,
}

#[derive(Debug, Default)]
pub struct McloneOverworldFeatureDependencyCache {
    cache: SurfaceDependencyCache,
}

impl McloneOverworldFeatureDependencyCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn retained_chunk_count(&self) -> usize {
        self.cache.retained_chunk_count()
    }

    pub fn resident_positions(&self) -> std::collections::BTreeSet<ChunkPos> {
        self.cache.resident_positions()
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn generate_features_chunks(
        &mut self,
        seed: i64,
        targets: impl IntoIterator<Item = ChunkPos>,
    ) -> McloneOverworldFeatureBatchResult {
        self.generate_features_chunks_with_dependencies(seed, targets, std::iter::empty())
    }

    pub fn generate_features_chunks_with_dependencies(
        &mut self,
        seed: i64,
        targets: impl IntoIterator<Item = ChunkPos>,
        dependencies: impl IntoIterator<Item = MutableChunkBlockBuffer>,
    ) -> McloneOverworldFeatureBatchResult {
        let plan = ChunkGenerationPlan::mclone_overworld_features(targets);
        let PreparedSurfaceDependencies {
            region_chunks,
            retained_dependencies,
            report,
        } = self.cache.prepare(seed, &plan, dependencies, |pos| {
            generate_mclone_overworld_surface_buffer(seed, pos.x, pos.z)
        });
        let cache_report = mclone_overworld_cache_report(report);

        if plan.output_chunks().is_empty() {
            return McloneOverworldFeatureBatchResult {
                chunks: BTreeMap::new(),
                retained_dependencies,
                cache_report,
            };
        }

        let first_target = *plan
            .output_chunks()
            .iter()
            .next()
            .expect("non-empty Mclone Overworld targets");
        let mut region = FeatureRegion::new(first_target.x, first_target.z, region_chunks);
        for center in sorted_chunk_positions_z_major(plan.backend_work_chunks().iter().copied()) {
            region.set_center(center.x, center.z);
            decorate_mclone_overworld_center(seed, &mut region);
        }

        let (targets, _, _) = plan.into_parts();
        let mut chunks = BTreeMap::new();
        for target in targets {
            let chunk = region.remove_chunk(target.x, target.z).unwrap_or_else(|| {
                panic!(
                    "Mclone Overworld feature region omitted target ({}, {})",
                    target.x, target.z
                )
            });
            let min_x = chunk_min_block_coord(target.x);
            let min_z = chunk_min_block_coord(target.z);
            chunks.insert(
                target,
                GeneratedChunk::from_mutable_buffer_with_biomes(
                    chunk,
                    mclone_overworld_chunk_biomes(seed, min_x, min_z),
                ),
            );
        }

        McloneOverworldFeatureBatchResult {
            chunks,
            retained_dependencies,
            cache_report,
        }
    }
}

fn mclone_overworld_cache_report(
    report: SurfaceDependencyCacheReport,
) -> McloneOverworldFeatureDependencyCacheReport {
    McloneOverworldFeatureDependencyCacheReport {
        requested_dependency_chunks: report.requested_dependency_chunks,
        cache_hits: report.cache_hits,
        generated_dependency_chunks: report.generated_dependency_chunks,
        retained_dependency_chunks: report.retained_dependency_chunks,
    }
}

pub fn generate_mclone_overworld_chunk(seed: i64, chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
    let pos = ChunkPos::new(chunk_x, chunk_z);
    McloneOverworldFeatureDependencyCache::new()
        .generate_features_chunks(seed, [pos])
        .chunks
        .remove(&pos)
        .unwrap_or_else(|| panic!("Mclone Overworld feature batch omitted ({chunk_x}, {chunk_z})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{DANDELION, GRASS, OAK_LOG, POPPY};

    #[test]
    fn cache_reuses_overlapping_surface_inputs() {
        let mut cache = McloneOverworldFeatureDependencyCache::new();
        let first = cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);
        assert_eq!(first.cache_report.requested_dependency_chunks, 25);
        assert_eq!(first.cache_report.cache_hits, 0);
        assert_eq!(first.cache_report.generated_dependency_chunks, 25);

        let second = cache.generate_features_chunks(12_345, [ChunkPos::new(1, 0)]);
        assert_eq!(second.cache_report.requested_dependency_chunks, 25);
        assert_eq!(second.cache_report.cache_hits, 20);
        assert_eq!(second.cache_report.generated_dependency_chunks, 5);
        assert_eq!(second.cache_report.retained_dependency_chunks, 25);
    }

    #[test]
    fn neighboring_targets_are_partition_independent() {
        let targets = [ChunkPos::new(0, 0), ChunkPos::new(1, 0)];
        let combined = McloneOverworldFeatureDependencyCache::new()
            .generate_features_chunks(12_345, targets)
            .chunks;
        let partitioned = targets
            .into_iter()
            .map(|target| {
                (
                    target,
                    generate_mclone_overworld_chunk(12_345, target.x, target.z),
                )
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(combined, partitioned);
    }

    #[test]
    fn feature_stage_places_the_profile_owned_vegetation_family() {
        let targets = (-4..=4)
            .flat_map(|z| (-4..=4).map(move |x| ChunkPos::new(x, z)))
            .collect::<Vec<_>>();
        let chunks = McloneOverworldFeatureDependencyCache::new()
            .generate_features_chunks(12_345, targets)
            .chunks;
        let decoration_counts = [OAK_LOG, GRASS, DANDELION, POPPY].map(|block| {
            chunks
                .values()
                .map(|chunk| chunk.block_count(block))
                .sum::<usize>()
        });
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for chunk in chunks.values() {
            for byte in chunk
                .blocks()
                .iter()
                .copied()
                .chain(chunk.biomes().iter().flat_map(|id| id.to_le_bytes()))
            {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }

        assert_eq!(decoration_counts, [57, 3_326, 383, 223]);
        assert_eq!(hash, 6_043_725_934_403_648_447);
    }

    #[test]
    fn reviewed_landforms_pin_final_decorated_payloads() {
        let receipts = [
            (-98_765, ChunkPos::new(-186, 25)),
            (-98_765, ChunkPos::new(-204, 22)),
            (12_345, ChunkPos::new(-142, -51)),
        ]
        .map(|(seed, target)| {
            let chunk = generate_mclone_overworld_chunk(seed, target.x, target.z);
            let decoration_counts =
                [OAK_LOG, GRASS, DANDELION, POPPY].map(|block| chunk.block_count(block));
            let mut hash = 0xcbf2_9ce4_8422_2325_u64;
            for byte in chunk
                .blocks()
                .iter()
                .copied()
                .chain(chunk.biomes().iter().flat_map(|id| id.to_le_bytes()))
            {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            (decoration_counts, hash)
        });

        assert_eq!(
            receipts,
            [
                ([0, 0, 0, 0], 7_356_793_827_551_712_870),
                ([0, 31, 8, 0], 17_780_938_515_556_800_113),
                ([0, 46, 12, 0], 7_126_608_364_848_634_164),
            ]
        );
    }
}
