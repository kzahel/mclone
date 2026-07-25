use std::collections::BTreeMap;

use mclone_core::{CHUNK_WIDTH, ChunkPos, ChunkStatus, chunk_min_block_coord};

use crate::feature::{FEATURES_BLOCK_DEPENDENCY_RADIUS, FeatureRegion};
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
use super::streams::{McloneOverworldStreamPlanCache, McloneOverworldStreamPlanCacheReport};
use super::terrain::{
    generate_mclone_overworld_surface_buffer_with_stream_cache,
    mclone_overworld_chunk_biomes_with_stream_cache,
};
use super::vegetation::{
    McloneOverworldVegetationPlanCache, McloneTreeId, McloneTreeOccurrence, McloneVegetationBounds,
    McloneVegetationError, McloneVegetationPlanCacheReport, McloneVegetationSource,
    realize_mclone_tree_occurrences,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McloneOverworldFeatureDependencyCacheReport {
    pub requested_dependency_chunks: usize,
    pub cache_hits: usize,
    pub generated_dependency_chunks: usize,
    pub retained_dependency_chunks: usize,
    pub stream_plan_requests: u64,
    pub stream_plan_cache_hits: u64,
    pub stream_plan_cache_misses: u64,
    pub stream_intersection_requests: u64,
    pub stream_intersection_cache_hits: u64,
    pub retained_stream_intersection_queries: usize,
    pub accepted_stream_plans: usize,
    pub rejected_stream_candidates: usize,
    pub vegetation_cell_requests: u64,
    pub vegetation_cell_cache_hits: u64,
    pub vegetation_cell_cache_misses: u64,
    pub retained_vegetation_cells: usize,
    pub retained_preliminary_tree_candidates: usize,
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
    stream_plans: Option<McloneOverworldStreamPlanCache>,
    vegetation_plans: Option<McloneOverworldVegetationPlanCache>,
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
        self.stream_plans = None;
        self.vegetation_plans = None;
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
        let topology = McloneOverworldSamplingTopology::Unbounded;
        let plan = ChunkGenerationPlan::mclone_overworld_features(targets);
        ensure_stream_cache(&mut self.stream_plans, seed, topology);
        ensure_vegetation_cache(&mut self.vegetation_plans, seed, topology);
        let stream_plans = self
            .stream_plans
            .as_mut()
            .expect("Mclone stream cache was initialized");
        let PreparedSurfaceDependencies {
            region_chunks,
            retained_dependencies,
            report,
        } = self.cache.prepare(seed, &plan, dependencies, |pos| {
            generate_mclone_overworld_surface_buffer_with_stream_cache(
                seed,
                topology,
                pos.x,
                pos.z,
                stream_plans,
            )
        });
        if plan.output_chunks().is_empty() {
            let vegetation_report = self
                .vegetation_plans
                .as_ref()
                .expect("Mclone vegetation cache was initialized")
                .report();
            return McloneOverworldFeatureBatchResult {
                chunks: BTreeMap::new(),
                retained_dependencies,
                cache_report: mclone_overworld_cache_report(
                    report,
                    stream_plans.report(),
                    vegetation_report,
                ),
            };
        }

        let work_centers =
            sorted_chunk_positions_z_major(plan.backend_work_chunks().iter().copied());
        let tree_occurrences = planned_tree_occurrences_for_centers(
            self.vegetation_plans
                .as_mut()
                .expect("Mclone vegetation cache was initialized"),
            work_centers.iter().copied(),
        )
        .expect("Mclone vegetation work bounds are representable");
        let first_target = *plan
            .output_chunks()
            .iter()
            .next()
            .expect("non-empty Mclone Overworld targets");
        let mut region = FeatureRegion::new(first_target.x, first_target.z, region_chunks);
        realize_mclone_tree_occurrences(&mut region, &tree_occurrences);
        for center in work_centers {
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
                    mclone_overworld_chunk_biomes_with_stream_cache(
                        seed,
                        topology,
                        min_x,
                        min_z,
                        stream_plans,
                    ),
                ),
            );
        }

        let vegetation_report = self
            .vegetation_plans
            .as_ref()
            .expect("Mclone vegetation cache was initialized")
            .report();
        McloneOverworldFeatureBatchResult {
            chunks,
            retained_dependencies,
            cache_report: mclone_overworld_cache_report(
                report,
                stream_plans.report(),
                vegetation_report,
            ),
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
        ensure_stream_cache(&mut self.stream_plans, seed, topology);
        ensure_vegetation_cache(&mut self.vegetation_plans, seed, topology);
        let stream_plans = self
            .stream_plans
            .as_mut()
            .expect("Mclone stream cache was initialized");
        let PreparedSurfaceDependencies {
            retained_dependencies,
            report,
            ..
        } = self
            .cache
            .prepare_scoped(seed, topology.cache_scope(), &plan, dependencies, |pos| {
                generate_mclone_overworld_surface_buffer_with_stream_cache(
                    seed,
                    topology,
                    pos.x,
                    pos.z,
                    stream_plans,
                )
            });
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
            let work_centers =
                sorted_chunk_positions_z_major(work_plan.backend_work_chunks().iter().copied());
            let tree_occurrences = planned_tree_occurrences_for_centers(
                self.vegetation_plans
                    .as_mut()
                    .expect("Mclone vegetation cache was initialized"),
                work_centers.iter().copied(),
            )
            .expect("periodic Mclone vegetation work bounds are representable");
            realize_mclone_tree_occurrences(&mut region, &tree_occurrences);
            for center in work_centers {
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
                        mclone_overworld_chunk_biomes_with_stream_cache(
                            seed,
                            topology,
                            chunk_min_block_coord(canonical.x),
                            chunk_min_block_coord(canonical.z),
                            stream_plans,
                        ),
                    ),
                );
            }

            let vegetation_report = self
                .vegetation_plans
                .as_ref()
                .expect("Mclone vegetation cache was initialized")
                .report();
            return McloneOverworldFeatureBatchResult {
                chunks,
                retained_dependencies,
                cache_report: mclone_overworld_cache_report(
                    report,
                    stream_plans.report(),
                    vegetation_report,
                ),
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
            let work_centers =
                sorted_chunk_positions_z_major(work_plan.backend_work_chunks().iter().copied());
            let tree_occurrences = planned_tree_occurrences_for_centers(
                self.vegetation_plans
                    .as_mut()
                    .expect("Mclone vegetation cache was initialized"),
                work_centers.iter().copied(),
            )
            .expect("periodic Mclone vegetation work bounds are representable");
            realize_mclone_tree_occurrences(&mut region, &tree_occurrences);
            for center in work_centers {
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
                    mclone_overworld_chunk_biomes_with_stream_cache(
                        seed,
                        topology,
                        chunk_min_block_coord(target.x),
                        chunk_min_block_coord(target.z),
                        stream_plans,
                    ),
                ),
            );
        }

        let vegetation_report = self
            .vegetation_plans
            .as_ref()
            .expect("Mclone vegetation cache was initialized")
            .report();
        McloneOverworldFeatureBatchResult {
            chunks,
            retained_dependencies,
            cache_report: mclone_overworld_cache_report(
                report,
                stream_plans.report(),
                vegetation_report,
            ),
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
    stream_report: McloneOverworldStreamPlanCacheReport,
    vegetation_report: McloneVegetationPlanCacheReport,
) -> McloneOverworldFeatureDependencyCacheReport {
    McloneOverworldFeatureDependencyCacheReport {
        requested_dependency_chunks: report.requested_dependency_chunks,
        cache_hits: report.cache_hits,
        generated_dependency_chunks: report.generated_dependency_chunks,
        retained_dependency_chunks: report.retained_dependency_chunks,
        stream_plan_requests: stream_report.requests,
        stream_plan_cache_hits: stream_report.hits,
        stream_plan_cache_misses: stream_report.misses,
        stream_intersection_requests: stream_report.intersection_requests,
        stream_intersection_cache_hits: stream_report.intersection_hits,
        retained_stream_intersection_queries: stream_report.retained_intersection_queries,
        accepted_stream_plans: stream_report.accepted_plans,
        rejected_stream_candidates: stream_report.rejected_candidates,
        vegetation_cell_requests: vegetation_report.cell_requests,
        vegetation_cell_cache_hits: vegetation_report.cell_hits,
        vegetation_cell_cache_misses: vegetation_report.cell_misses,
        retained_vegetation_cells: vegetation_report.retained_cells,
        retained_preliminary_tree_candidates: vegetation_report.retained_preliminary_candidates,
    }
}

fn ensure_stream_cache(
    cache: &mut Option<McloneOverworldStreamPlanCache>,
    seed: i64,
    topology: McloneOverworldSamplingTopology,
) {
    if cache
        .as_ref()
        .is_none_or(|cache| !cache.matches(seed, topology))
    {
        *cache = Some(McloneOverworldStreamPlanCache::new(seed, topology));
    }
}

fn ensure_vegetation_cache(
    cache: &mut Option<McloneOverworldVegetationPlanCache>,
    seed: i64,
    topology: McloneOverworldSamplingTopology,
) {
    let source = McloneVegetationSource::new(seed, topology);
    if cache.as_ref().is_none_or(|cache| !cache.matches(source)) {
        *cache = Some(McloneOverworldVegetationPlanCache::new(source));
    }
}

fn planned_tree_occurrences_for_centers(
    cache: &mut McloneOverworldVegetationPlanCache,
    centers: impl IntoIterator<Item = ChunkPos>,
) -> Result<Vec<McloneTreeOccurrence>, McloneVegetationError> {
    let mut occurrences = BTreeMap::<(McloneTreeId, i64), McloneTreeOccurrence>::new();
    for center in centers {
        let min_chunk_x = center
            .x
            .checked_sub(FEATURES_BLOCK_DEPENDENCY_RADIUS)
            .ok_or(McloneVegetationError::CoordinateOverflow)?;
        let min_chunk_z = center
            .z
            .checked_sub(FEATURES_BLOCK_DEPENDENCY_RADIUS)
            .ok_or(McloneVegetationError::CoordinateOverflow)?;
        let max_chunk_x = center
            .x
            .checked_add(FEATURES_BLOCK_DEPENDENCY_RADIUS)
            .ok_or(McloneVegetationError::CoordinateOverflow)?;
        let max_chunk_z = center
            .z
            .checked_add(FEATURES_BLOCK_DEPENDENCY_RADIUS)
            .ok_or(McloneVegetationError::CoordinateOverflow)?;
        let bounds = McloneVegetationBounds::new(
            checked_chunk_min_block_coord(min_chunk_x)?,
            checked_chunk_min_block_coord(min_chunk_z)?,
            checked_chunk_min_block_coord(max_chunk_x)?
                .checked_add(CHUNK_WIDTH - 1)
                .ok_or(McloneVegetationError::CoordinateOverflow)?,
            checked_chunk_min_block_coord(max_chunk_z)?
                .checked_add(CHUNK_WIDTH - 1)
                .ok_or(McloneVegetationError::CoordinateOverflow)?,
        )?;
        for occurrence in cache.tree_records_intersecting(bounds)? {
            occurrences.insert((occurrence.record.id, occurrence.x_lift), occurrence);
        }
    }
    Ok(occurrences.into_values().collect())
}

fn checked_chunk_min_block_coord(chunk_coord: i32) -> Result<i32, McloneVegetationError> {
    i32::try_from(i64::from(chunk_coord) * i64::from(CHUNK_WIDTH))
        .map_err(|_| McloneVegetationError::CoordinateOverflow)
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
    use crate::block::{
        ACACIA_LOG, ANDESITE, COBBLESTONE, DANDELION, FERN, GRASS, LARGE_FERN_LOWER,
        MOSSY_COBBLESTONE, OAK_LOG, POPPY, SPRUCE_LOG, SWEET_BERRY_BUSH, TALL_GRASS_LOWER, WATER,
    };
    use crate::levelgen::MCLONE_OVERWORLD_PERIOD_CHUNKS;
    use crate::levelgen::mclone_overworld::{
        McloneTreeFamily, McloneVegetationSource, tree_records_intersecting,
    };
    use crate::placement::BlockPos;

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
    fn planned_stream_chunks_are_partition_order_and_cache_independent() {
        let seed = -98_765;
        let targets = [ChunkPos::new(148, -124), ChunkPos::new(149, -124)];
        let mut combined_cache = McloneOverworldFeatureDependencyCache::new();
        let combined = combined_cache.generate_features_chunks(seed, targets);
        let reversed = McloneOverworldFeatureDependencyCache::new()
            .generate_features_chunks(seed, [targets[1], targets[0]])
            .chunks;
        let partitioned = targets
            .into_iter()
            .map(|target| {
                (
                    target,
                    generate_mclone_overworld_chunk(seed, target.x, target.z),
                )
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(combined.chunks, reversed);
        assert_eq!(combined.chunks, partitioned);
        assert!(combined.cache_report.accepted_stream_plans > 0);
        assert!(combined.cache_report.stream_plan_cache_hits > 0);
        assert_eq!(
            combined.cache_report.stream_plan_requests,
            combined.cache_report.stream_plan_cache_hits
                + combined.cache_report.stream_plan_cache_misses
        );
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
    fn periodic_stream_realization_repeats_across_seam_lifts() {
        let seed = 12_345;
        let topology = McloneOverworldSamplingTopology::PeriodicX;
        let planner = super::super::streams::McloneOverworldStreamPlanner::new(seed, topology);
        let candidate = planner
            .potential_start(ChunkPos::new(3, -935))
            .expect("periodic stream placement");
        let plan = planner
            .plan_start(candidate)
            .expect("periodic stream plan")
            .expect("reviewed seam-crossing stream");
        let seam_node = plan
            .nodes
            .iter()
            .min_by_key(|node| node.x.abs())
            .expect("stream plan has nodes");
        let left = ChunkPos::new(seam_node.x.div_euclid(16), seam_node.z.div_euclid(16));
        assert!(plan.structure.bounds.intersects_chunk(left));
        assert!(plan.terrain_intent(seam_node.x, seam_node.z, 80).is_some());

        let lifted = ChunkPos::new(left.x + MCLONE_OVERWORLD_PERIOD_CHUNKS as i32, left.z);
        assert_eq!(
            generate_mclone_overworld_chunk_with_topology(seed, topology, left.x, left.z),
            generate_mclone_overworld_chunk_with_topology(seed, topology, lifted.x, lifted.z)
        );
    }

    #[test]
    fn feature_stage_places_the_profile_owned_vegetation_family() {
        let targets = [(ChunkPos::new(-43, 6), 3), (ChunkPos::new(-5, 0), 3)]
            .into_iter()
            .flat_map(|(center, radius)| {
                (center.z - radius..=center.z + radius).flat_map(move |z| {
                    (center.x - radius..=center.x + radius).map(move |x| ChunkPos::new(x, z))
                })
            })
            .collect::<Vec<_>>();
        let chunks = McloneOverworldFeatureDependencyCache::new()
            .generate_features_chunks(12_345, targets.iter().copied())
            .chunks;
        let decoration_counts = [
            OAK_LOG,
            SPRUCE_LOG,
            ACACIA_LOG,
            GRASS,
            FERN,
            LARGE_FERN_LOWER,
            SWEET_BERRY_BUSH,
            TALL_GRASS_LOWER,
            DANDELION,
            POPPY,
        ]
        .map(|block| {
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

        assert_eq!(
            decoration_counts,
            [199, 768, 294, 1_934, 272, 0, 10, 45, 82, 49]
        );
        assert_eq!(hash, 1_780_731_906_598_478_474);

        let source =
            McloneVegetationSource::new(12_345, McloneOverworldSamplingTopology::Unbounded);
        let planned_records = targets
            .iter()
            .flat_map(|target| {
                let min_x = chunk_min_block_coord(target.x);
                let min_z = chunk_min_block_coord(target.z);
                tree_records_intersecting(
                    source,
                    McloneVegetationBounds::new(
                        min_x,
                        min_z,
                        min_x + CHUNK_WIDTH - 1,
                        min_z + CHUNK_WIDTH - 1,
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .filter(|occurrence| {
                let base_chunk = ChunkPos::new(
                    mclone_core::block_to_chunk_coord(occurrence.record.canonical_base.x),
                    mclone_core::block_to_chunk_coord(occurrence.record.canonical_base.z),
                );
                (-1..=1).all(|dz| {
                    (-1..=1).all(|dx| {
                        chunks.contains_key(&ChunkPos::new(base_chunk.x + dx, base_chunk.z + dz))
                    })
                })
            })
            .collect::<std::collections::BTreeSet<_>>();
        let mut seen_families = [false; 3];
        for occurrence in planned_records {
            let family_index = match occurrence.record.family {
                McloneTreeFamily::TemperateBroadleaf => 0,
                McloneTreeFamily::CoolWetConifer => 1,
                McloneTreeFamily::WarmDryAcacia => 2,
            };
            seen_families[family_index] = true;
            let base = occurrence.record.canonical_base;
            let log = match occurrence.record.family {
                McloneTreeFamily::TemperateBroadleaf => OAK_LOG,
                McloneTreeFamily::CoolWetConifer => SPRUCE_LOG,
                McloneTreeFamily::WarmDryAcacia => ACACIA_LOG,
            };
            let vertical_height = match occurrence.record.family {
                McloneTreeFamily::WarmDryAcacia => i32::from(occurrence.record.trunk_height) - 2,
                McloneTreeFamily::TemperateBroadleaf | McloneTreeFamily::CoolWetConifer => {
                    i32::from(occurrence.record.trunk_height)
                }
            };
            for dy in 0..vertical_height {
                assert_eq!(
                    block_at_world(&chunks, BlockPos::new(base.x, base.y + dy, base.z)),
                    Some(log),
                    "record {:?} trunk differs at y {}",
                    occurrence.record.id,
                    base.y + dy
                );
            }
            if occurrence.record.family == McloneTreeFamily::WarmDryAcacia {
                let trunk_height = i32::from(occurrence.record.trunk_height);
                let (dx, dz) = test_tree_direction(occurrence.record.orientation);
                for step in 1..=3 {
                    let pos = BlockPos::new(
                        base.x + dx * step,
                        base.y + trunk_height - 3 + step,
                        base.z + dz * step,
                    );
                    assert_eq!(
                        block_at_world(&chunks, pos),
                        Some(ACACIA_LOG),
                        "record {:?} main fork differs at {pos:?}",
                        occurrence.record.id
                    );
                }
            }
        }
        assert_eq!(seen_families, [true; 3]);
    }

    fn block_at_world(
        chunks: &BTreeMap<ChunkPos, GeneratedChunk>,
        pos: BlockPos,
    ) -> Option<crate::block::RawBlockId> {
        chunks
            .get(&ChunkPos::new(
                mclone_core::block_to_chunk_coord(pos.x),
                mclone_core::block_to_chunk_coord(pos.z),
            ))
            .map(|chunk| {
                chunk
                    .block_at_y(
                        mclone_core::local_block_coord(pos.x),
                        pos.y,
                        mclone_core::local_block_coord(pos.z),
                    )
                    .raw()
            })
    }

    fn test_tree_direction(orientation: u8) -> (i32, i32) {
        match orientation & 3 {
            0 => (0, -1),
            1 => (1, 0),
            2 => (0, 1),
            3 => (-1, 0),
            _ => unreachable!(),
        }
    }

    #[test]
    fn river_heavy_region_places_sparse_watercourse_rocks() {
        let center = ChunkPos::new(47, 102);
        let targets = (center.z - 3..=center.z + 3)
            .flat_map(|z| (center.x - 3..=center.x + 3).map(move |x| ChunkPos::new(x, z)))
            .collect::<Vec<_>>();
        let chunks = McloneOverworldFeatureDependencyCache::new()
            .generate_features_chunks(-98_765, targets)
            .chunks;
        let rock_blocks = chunks
            .values()
            .map(|chunk| {
                chunk.block_count(COBBLESTONE)
                    + chunk.block_count(MOSSY_COBBLESTONE)
                    + chunk.block_count(ANDESITE)
            })
            .sum::<usize>();
        let water_blocks = chunks
            .values()
            .map(|chunk| chunk.block_count(WATER))
            .sum::<usize>();

        assert!(
            (1..=256).contains(&rock_blocks),
            "{rock_blocks} rock blocks beside {water_blocks} water blocks"
        );
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
                ([0, 0, 0, 0], 2_659_555_282_091_584_435),
                ([0, 2, 0, 0], 18_311_530_829_734_120_101),
                ([0, 1, 0, 0], 8_259_124_316_197_497_460),
            ]
        );
    }
}
