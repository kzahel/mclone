use std::collections::BTreeMap;

use mclone_core::{ChunkPos, ChunkStatus, chunk_min_block_coord};

use crate::feature::FeatureRegion;
use crate::levelgen::feature_batch::sorted_chunk_positions_z_major;
use crate::levelgen::surface_dependency_cache::{
    PreparedSurfaceDependencies, SurfaceDependencyCache, SurfaceDependencyCacheReport,
};
use crate::levelgen::{
    ChunkGenerationPlan, ChunkStatusRequirement, GeneratedChunk, MutableChunkBlockBuffer,
};

use super::decoration::{
    decorate_mclone_overworld_center, decorate_mclone_overworld_center_with_topology,
};
use super::fields::{MCLONE_OVERWORLD_PERIOD_CHUNKS, McloneOverworldSamplingTopology};
use super::terrain::{
    generate_mclone_overworld_surface_buffer,
    generate_mclone_overworld_surface_buffer_with_topology, mclone_overworld_chunk_biomes,
    mclone_overworld_chunk_biomes_with_topology,
};

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
        self.generate_features_chunks_with_topology_and_dependencies(
            seed,
            McloneOverworldSamplingTopology::Unbounded,
            targets,
            dependencies,
        )
    }

    pub fn generate_features_chunks_with_topology_and_dependencies(
        &mut self,
        seed: i64,
        topology: McloneOverworldSamplingTopology,
        targets: impl IntoIterator<Item = ChunkPos>,
        dependencies: impl IntoIterator<Item = MutableChunkBlockBuffer>,
    ) -> McloneOverworldFeatureBatchResult {
        if topology == McloneOverworldSamplingTopology::Unbounded {
            return self.generate_unbounded_features_chunks_with_dependencies(
                seed,
                targets,
                dependencies,
            );
        }

        self.generate_periodic_features_chunks_with_dependencies(
            seed,
            topology,
            targets,
            dependencies,
        )
    }

    fn generate_unbounded_features_chunks_with_dependencies(
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

    fn generate_periodic_features_chunks_with_dependencies(
        &mut self,
        seed: i64,
        topology: McloneOverworldSamplingTopology,
        targets: impl IntoIterator<Item = ChunkPos>,
        dependencies: impl IntoIterator<Item = MutableChunkBlockBuffer>,
    ) -> McloneOverworldFeatureBatchResult {
        let targets = targets
            .into_iter()
            .map(|target| ChunkPos::new(topology.canonical_chunk_x(target.x), target.z))
            .collect::<std::collections::BTreeSet<_>>();
        let plan = canonical_periodic_plan(topology, targets.iter().copied());
        let PreparedSurfaceDependencies {
            retained_dependencies,
            report,
            ..
        } = self
            .cache
            .prepare_scoped(seed, topology.cache_scope(), &plan, dependencies, |pos| {
                generate_mclone_overworld_surface_buffer_with_topology(seed, topology, pos.x, pos.z)
            });
        let cache_report = mclone_overworld_cache_report(report);
        let mut chunks = BTreeMap::new();

        if let Some(work_targets) = coherent_periodic_work_targets(&targets) {
            let work_plan =
                ChunkGenerationPlan::mclone_overworld_features(work_targets.values().copied());
            let mut region_chunks = work_plan
                .prerequisites()
                .iter()
                .map(|requirement| {
                    let canonical = ChunkPos::new(
                        topology.canonical_chunk_x(requirement.pos.x),
                        requirement.pos.z,
                    );
                    let mut chunk = retained_dependencies
                        .get(&canonical)
                        .unwrap_or_else(|| {
                            panic!(
                                "periodic Mclone feature region omitted canonical dependency ({}, {})",
                                canonical.x, canonical.z
                            )
                        })
                        .clone();
                    chunk.chunk_x = requirement.pos.x;
                    chunk.chunk_z = requirement.pos.z;
                    chunk
                })
                .collect::<Vec<_>>();
            region_chunks.sort_by_key(|chunk| (chunk.chunk_z, chunk.chunk_x));
            let first_work_target = *work_targets
                .values()
                .next()
                .expect("non-empty coherent periodic target set");
            let mut region =
                FeatureRegion::new(first_work_target.x, first_work_target.z, region_chunks);
            for center in
                sorted_chunk_positions_z_major(work_plan.backend_work_chunks().iter().copied())
            {
                region.set_center_with_decoration_identity(
                    center.x,
                    center.z,
                    topology.canonical_chunk_x(center.x),
                    center.z,
                );
                decorate_mclone_overworld_center_with_topology(seed, topology, &mut region);
            }

            for (canonical, work) in work_targets {
                let mut chunk = region.remove_chunk(work.x, work.z).unwrap_or_else(|| {
                    panic!(
                        "periodic Mclone feature region omitted lifted target ({}, {})",
                        work.x, work.z
                    )
                });
                chunk.chunk_x = canonical.x;
                chunk.chunk_z = canonical.z;
                chunks.insert(
                    canonical,
                    GeneratedChunk::from_mutable_buffer_with_biomes(
                        chunk,
                        mclone_overworld_chunk_biomes_with_topology(
                            seed,
                            topology,
                            chunk_min_block_coord(canonical.x),
                            chunk_min_block_coord(canonical.z),
                        ),
                    ),
                );
            }

            return McloneOverworldFeatureBatchResult {
                chunks,
                retained_dependencies,
                cache_report,
            };
        }

        for target in targets {
            let work_plan = ChunkGenerationPlan::mclone_overworld_features([target]);
            let mut region_chunks = work_plan
                .prerequisites()
                .iter()
                .map(|requirement| {
                    let canonical = ChunkPos::new(
                        topology.canonical_chunk_x(requirement.pos.x),
                        requirement.pos.z,
                    );
                    let mut chunk = retained_dependencies
                        .get(&canonical)
                        .unwrap_or_else(|| {
                            panic!(
                                "periodic Mclone feature region omitted canonical dependency ({}, {})",
                                canonical.x, canonical.z
                            )
                        })
                        .clone();
                    chunk.chunk_x = requirement.pos.x;
                    chunk.chunk_z = requirement.pos.z;
                    chunk
                })
                .collect::<Vec<_>>();
            region_chunks.sort_by_key(|chunk| (chunk.chunk_z, chunk.chunk_x));
            let mut region = FeatureRegion::new(target.x, target.z, region_chunks);
            for center in
                sorted_chunk_positions_z_major(work_plan.backend_work_chunks().iter().copied())
            {
                region.set_center_with_decoration_identity(
                    center.x,
                    center.z,
                    topology.canonical_chunk_x(center.x),
                    center.z,
                );
                decorate_mclone_overworld_center_with_topology(seed, topology, &mut region);
            }

            let mut chunk = region.remove_chunk(target.x, target.z).unwrap_or_else(|| {
                panic!(
                    "periodic Mclone feature region omitted target ({}, {})",
                    target.x, target.z
                )
            });
            chunk.chunk_x = target.x;
            chunk.chunk_z = target.z;
            chunks.insert(
                target,
                GeneratedChunk::from_mutable_buffer_with_biomes(
                    chunk,
                    mclone_overworld_chunk_biomes_with_topology(
                        seed,
                        topology,
                        chunk_min_block_coord(target.x),
                        chunk_min_block_coord(target.z),
                    ),
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

const MAX_COHERENT_PERIODIC_TARGET_SPAN_CHUNKS: i32 = 64;

fn coherent_periodic_work_targets(
    targets: &std::collections::BTreeSet<ChunkPos>,
) -> Option<BTreeMap<ChunkPos, ChunkPos>> {
    let anchor = *targets.iter().next()?;
    let period = MCLONE_OVERWORLD_PERIOD_CHUNKS as i32;
    let mut lifted = BTreeMap::new();
    for target in targets {
        let forward = (target.x - anchor.x).rem_euclid(period);
        let offset_x = if forward > period / 2 {
            forward - period
        } else {
            forward
        };
        lifted.insert(*target, ChunkPos::new(anchor.x + offset_x, target.z));
    }
    let min_x = lifted.values().map(|pos| pos.x).min()?;
    let max_x = lifted.values().map(|pos| pos.x).max()?;
    let min_z = lifted.values().map(|pos| pos.z).min()?;
    let max_z = lifted.values().map(|pos| pos.z).max()?;
    ((max_x - min_x) <= MAX_COHERENT_PERIODIC_TARGET_SPAN_CHUNKS
        && (max_z - min_z) <= MAX_COHERENT_PERIODIC_TARGET_SPAN_CHUNKS)
        .then_some(lifted)
}

fn canonical_periodic_plan(
    topology: McloneOverworldSamplingTopology,
    targets: impl IntoIterator<Item = ChunkPos>,
) -> ChunkGenerationPlan {
    let raw = ChunkGenerationPlan::mclone_overworld_features(targets);
    let (outputs, backend_work, prerequisites) = raw.into_parts();
    ChunkGenerationPlan::from_parts(
        outputs
            .into_iter()
            .map(|pos| ChunkPos::new(topology.canonical_chunk_x(pos.x), pos.z)),
        backend_work
            .into_iter()
            .map(|pos| ChunkPos::new(topology.canonical_chunk_x(pos.x), pos.z)),
        prerequisites.into_iter().map(|requirement| {
            ChunkStatusRequirement::new(
                ChunkPos::new(
                    topology.canonical_chunk_x(requirement.pos.x),
                    requirement.pos.z,
                ),
                ChunkStatus::Surface,
            )
        }),
    )
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
    generate_mclone_overworld_chunk_with_topology(
        seed,
        McloneOverworldSamplingTopology::Unbounded,
        chunk_x,
        chunk_z,
    )
}

pub fn generate_mclone_overworld_chunk_with_topology(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    chunk_x: i32,
    chunk_z: i32,
) -> GeneratedChunk {
    let pos = ChunkPos::new(topology.canonical_chunk_x(chunk_x), chunk_z);
    McloneOverworldFeatureDependencyCache::new()
        .generate_features_chunks_with_topology_and_dependencies(
            seed,
            topology,
            [pos],
            std::iter::empty(),
        )
        .chunks
        .remove(&pos)
        .unwrap_or_else(|| panic!("Mclone Overworld feature batch omitted ({chunk_x}, {chunk_z})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{DANDELION, GRASS, OAK_LOG, POPPY};
    use crate::levelgen::MCLONE_OVERWORLD_PERIOD_CHUNKS;

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
    fn periodic_seam_batches_are_canonical_partition_and_order_independent() {
        let topology = McloneOverworldSamplingTopology::PeriodicX;
        let last_x = MCLONE_OVERWORLD_PERIOD_CHUNKS as i32 - 1;
        let targets = [ChunkPos::new(last_x, 0), ChunkPos::new(0, 0)];
        let mut combined_cache = McloneOverworldFeatureDependencyCache::new();
        let combined = combined_cache.generate_features_chunks_with_topology_and_dependencies(
            -98_765,
            topology,
            targets,
            std::iter::empty(),
        );
        let reversed = McloneOverworldFeatureDependencyCache::new()
            .generate_features_chunks_with_topology_and_dependencies(
                -98_765,
                topology,
                [targets[1], targets[0]],
                std::iter::empty(),
            )
            .chunks;
        let partitioned = targets
            .into_iter()
            .map(|target| {
                (
                    target,
                    generate_mclone_overworld_chunk_with_topology(
                        -98_765, topology, target.x, target.z,
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(combined.chunks, reversed);
        assert_eq!(combined.chunks, partitioned);
        assert!(
            combined
                .retained_dependencies
                .keys()
                .all(|pos| { (0..MCLONE_OVERWORLD_PERIOD_CHUNKS as i32).contains(&pos.x) })
        );
        assert_eq!(
            generate_mclone_overworld_chunk_with_topology(-98_765, topology, -1, 0),
            generate_mclone_overworld_chunk_with_topology(-98_765, topology, last_x, 0)
        );
        assert_eq!(
            generate_mclone_overworld_chunk_with_topology(
                -98_765,
                topology,
                MCLONE_OVERWORLD_PERIOD_CHUNKS as i32,
                0,
            ),
            generate_mclone_overworld_chunk_with_topology(-98_765, topology, 0, 0)
        );
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
                ([0, 0, 0, 0], 5_654_216_133_643_199_936),
                ([0, 39, 6, 0], 4_248_178_665_201_983_486),
                ([0, 35, 12, 0], 11_256_445_758_675_643_422),
            ]
        );
    }
}
