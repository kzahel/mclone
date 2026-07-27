//! Freely pannable structural atlas for the Tactical 270 streamed-plan trials.
//!
//! This is a research and diagnostic adapter. It gives Terrain Lab typed
//! drawing facts for the Phase 2 fallback and surviving candidates without
//! making any of them production terrain inputs.

use std::collections::{BTreeMap, VecDeque};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::streamed_plan_feature_graph_trial::{
    FEATURE_GRAPH_CANDIDATE_REVISION, FeatureOwnedGraphControl, GRAPH_EDGE_KIND, GRAPH_HEADER_KIND,
    GRAPH_NODE_KIND, GRAPH_QUERY_KIND, GRAPH_SINK_KIND,
};
use crate::streamed_plan_harness::{
    CoordinatePureControl, FALLBACK_CONTROL_REVISION, FALLBACK_FEATURE_KIND, FALLBACK_REGION_KIND,
    FALLBACK_SAMPLE_KIND, PlanRegion, STREAMED_PLAN_BASE_REGION_BLOCKS,
    STREAMED_PLAN_PERIOD_BLOCKS, StreamedPlanControl, StreamedPlanDescriptor, StreamedPlanKey,
    StreamedPlanRequest, StreamedPlanSnapshot, StreamedPlanTopology, StreamedSemanticFact,
};
use crate::streamed_plan_trials::{
    HIERARCHICAL_CANDIDATE_REVISION, HIERARCHY_HORIZONTAL_FACET_KIND, HIERARCHY_MID_BLOCKS,
    HIERARCHY_MID_KIND, HIERARCHY_REGION_KIND, HIERARCHY_ROOT_BLOCKS, HIERARCHY_ROOT_KIND,
    HIERARCHY_VERTICAL_FACET_KIND, HierarchicalSharedFactsControl,
    STREAMED_PLAN_PHASE_TWO_WITNESS_SHA256,
};

pub const STREAMED_PLAN_ATLAS_SCHEMA_REVISION: &str = "mclone-streamed-plan-atlas-v1";
pub const STREAMED_PLAN_ATLAS_MAX_REGIONS_PER_AXIS: i32 = 16;
pub const STREAMED_PLAN_ATLAS_CACHE_CAPACITY_PER_CANDIDATE: usize = 384;
pub const STREAMED_PLAN_ATLAS_FALLBACK_WITNESS_SHA256: &str =
    "7c098d833328fbd87b480f9f092536c815fb60ae66701f4278269185b1b7de7f";
pub const STREAMED_PLAN_ATLAS_HIERARCHY_WITNESS_SHA256: &str =
    "3041cac9d531349e5f1fda1dbfe3849be8752693a8b3b397e81351b9f184d91f";
pub const STREAMED_PLAN_ATLAS_GRAPH_WITNESS_SHA256: &str =
    "c619dcf4bfd2108ff53d0a2712974f55073b601f1fc0ab9b770e7a9e59161558";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamedPlanAtlasSummary {
    pub schema: &'static str,
    pub research_only: bool,
    pub production_terrain_unchanged: bool,
    pub phase_two_witness_sha256: &'static str,
    pub seed: String,
    pub topology: &'static str,
    pub period_blocks: Option<i32>,
    pub center_x: i32,
    pub center_z: i32,
    pub blocks_across: i32,
    pub view_height_blocks: i32,
    pub base_region_blocks: i32,
    pub coverage: AtlasCoverage,
    pub fallback: FallbackAtlas,
    pub hierarchy: HierarchyAtlas,
    pub feature_graph: FeatureGraphAtlas,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtlasCoverage {
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
    pub region_columns: u32,
    pub region_rows: u32,
    pub clipped: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtlasCandidateReceipt {
    pub candidate: &'static str,
    pub candidate_revision: &'static str,
    pub semantic_sha256: String,
    pub viewed_plan_count: u32,
    pub canonical_plan_count: u32,
    pub cache_hits: u32,
    pub cache_misses: u32,
    pub cache_evictions: u32,
    pub retained_plans: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtlasRegionIdentity {
    pub requested_region_x: i32,
    pub requested_region_z: i32,
    pub canonical_region_x: i32,
    pub canonical_region_z: i32,
    pub world_min_x: i32,
    pub world_min_z: i32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackAtlas {
    #[serde(flatten)]
    pub receipt: AtlasCandidateReceipt,
    pub cells: Vec<FallbackAtlasCell>,
    pub samples: Vec<FallbackAtlasSample>,
    pub features: Vec<FallbackAtlasFeature>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackAtlasCell {
    #[serde(flatten)]
    pub region: AtlasRegionIdentity,
    pub region_class: u8,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackAtlasSample {
    pub world_x: i32,
    pub world_z: i32,
    pub surface_y: i32,
    pub water_surface_y: Option<i32>,
    pub ridge_lift: i32,
    pub channel: bool,
    pub island: bool,
    pub pond: bool,
    pub top_block: u16,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackAtlasFeature {
    pub canonical_id: String,
    pub canonical_owner_x: i32,
    pub canonical_owner_z: i32,
    pub world_x: i32,
    pub world_z: i32,
    pub radius_blocks: i32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HierarchyAtlas {
    #[serde(flatten)]
    pub receipt: AtlasCandidateReceipt,
    pub cells: Vec<HierarchyAtlasCell>,
    pub providers: Vec<HierarchyAtlasProvider>,
    pub facets: Vec<HierarchyAtlasFacet>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HierarchyAtlasCell {
    #[serde(flatten)]
    pub region: AtlasRegionIdentity,
    pub root_tendency: i32,
    pub mid_tendency: i32,
    pub port_mask: u8,
    pub character: u8,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HierarchyAtlasProvider {
    pub level: u8,
    pub world_min_x: i32,
    pub world_min_z: i32,
    pub extent_blocks: i32,
    pub tendency: i32,
    pub class: u8,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HierarchyAtlasFacet {
    pub vertical: bool,
    pub world_a_x: i32,
    pub world_a_z: i32,
    pub world_b_x: i32,
    pub world_b_z: i32,
    pub open: bool,
    pub tendency: i32,
    pub direction: i8,
    pub strength: u8,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureGraphAtlas {
    #[serde(flatten)]
    pub receipt: AtlasCandidateReceipt,
    pub cells: Vec<FeatureGraphAtlasCell>,
    pub graphs: Vec<FeatureGraphAtlasGraph>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureGraphAtlasCell {
    #[serde(flatten)]
    pub region: AtlasRegionIdentity,
    pub owner_cells_examined: u8,
    pub accepted_graphs: u8,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureGraphAtlasGraph {
    pub canonical_owner_x: i32,
    pub canonical_owner_z: i32,
    pub anchor_x: i32,
    pub anchor_z: i32,
    pub bounds_min_x: i32,
    pub bounds_min_z: i32,
    pub bounds_max_x: i32,
    pub bounds_max_z: i32,
    pub maximum_reach_blocks: u32,
    pub nodes: Vec<FeatureGraphAtlasNode>,
    pub edges: Vec<FeatureGraphAtlasEdge>,
    pub sink_node: u8,
    pub sink_kind: u8,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureGraphAtlasNode {
    pub index: u8,
    pub world_x: i32,
    pub world_z: i32,
    pub potential: i32,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureGraphAtlasEdge {
    pub from: u8,
    pub to: u8,
    pub width: u8,
}

#[derive(Clone, Debug)]
struct CandidateCache {
    entries: BTreeMap<StreamedPlanKey, StreamedPlanSnapshot>,
    insertion_order: VecDeque<StreamedPlanKey>,
    capacity: usize,
}

impl CandidateCache {
    fn new(capacity: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            insertion_order: VecDeque::new(),
            capacity,
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.insertion_order.clear();
    }

    fn resolve<C: StreamedPlanControl + Default>(
        &mut self,
        descriptor: &StreamedPlanDescriptor,
        requested_region: PlanRegion,
        window_center_region: PlanRegion,
    ) -> Result<(StreamedPlanSnapshot, CacheDelta), String> {
        let control = C::default();
        let key = descriptor.plan_key(control.candidate_revision(), requested_region)?;
        if let Some(snapshot) = self.entries.get(&key) {
            return Ok((
                snapshot.clone(),
                CacheDelta {
                    hits: 1,
                    ..CacheDelta::default()
                },
            ));
        }
        let snapshot = C::default().construct(
            descriptor,
            StreamedPlanRequest::new(requested_region, window_center_region),
        )?;
        if snapshot.key != key {
            return Err(format!(
                "atlas candidate {} returned the wrong key",
                control.candidate_revision()
            ));
        }
        let mut evictions = 0;
        if self.capacity > 0 && self.entries.len() >= self.capacity {
            if let Some(evicted) = self.insertion_order.pop_front() {
                self.entries.remove(&evicted);
                evictions = 1;
            }
        }
        if self.capacity > 0 {
            self.insertion_order.push_back(key.clone());
            self.entries.insert(key, snapshot.clone());
        }
        Ok((
            snapshot,
            CacheDelta {
                misses: 1,
                evictions,
                ..CacheDelta::default()
            },
        ))
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct CacheDelta {
    hits: u32,
    misses: u32,
    evictions: u32,
}

impl CacheDelta {
    fn add(&mut self, other: Self) {
        self.hits = self.hits.saturating_add(other.hits);
        self.misses = self.misses.saturating_add(other.misses);
        self.evictions = self.evictions.saturating_add(other.evictions);
    }
}

#[derive(Clone, Debug)]
pub struct StreamedPlanAtlasCompiler {
    descriptor: StreamedPlanDescriptor,
    fallback_cache: CandidateCache,
    hierarchy_cache: CandidateCache,
    graph_cache: CandidateCache,
}

impl StreamedPlanAtlasCompiler {
    pub fn new(seed: i64, topology: StreamedPlanTopology) -> Self {
        Self::with_cache_capacity(
            seed,
            topology,
            STREAMED_PLAN_ATLAS_CACHE_CAPACITY_PER_CANDIDATE,
        )
    }

    fn with_cache_capacity(seed: i64, topology: StreamedPlanTopology, capacity: usize) -> Self {
        Self {
            descriptor: StreamedPlanDescriptor::new(seed, topology),
            fallback_cache: CandidateCache::new(capacity),
            hierarchy_cache: CandidateCache::new(capacity),
            graph_cache: CandidateCache::new(capacity),
        }
    }

    pub fn clear_cache(&mut self) {
        self.fallback_cache.clear();
        self.hierarchy_cache.clear();
        self.graph_cache.clear();
    }

    pub fn query(
        &mut self,
        center_x: i32,
        center_z: i32,
        blocks_across: i32,
        aspect_ratio: f64,
    ) -> Result<StreamedPlanAtlasSummary, String> {
        if blocks_across <= 0 {
            return Err("atlas blocks across must be positive".to_owned());
        }
        if !aspect_ratio.is_finite() || !(0.1..=10.0).contains(&aspect_ratio) {
            return Err("atlas aspect ratio must be finite and between 0.1 and 10".to_owned());
        }
        let view_height_blocks = (f64::from(blocks_across) / aspect_ratio).round().max(1.0) as i64;
        let requested = atlas_regions(center_x, center_z, blocks_across, view_height_blocks)?;
        let center_region = PlanRegion::new(
            center_x.div_euclid(STREAMED_PLAN_BASE_REGION_BLOCKS),
            center_z.div_euclid(STREAMED_PLAN_BASE_REGION_BLOCKS),
        );
        let fallback = build_fallback_atlas(
            &self.descriptor,
            &requested.regions,
            center_region,
            &mut self.fallback_cache,
        )?;
        let hierarchy = build_hierarchy_atlas(
            &self.descriptor,
            &requested.regions,
            center_region,
            &mut self.hierarchy_cache,
        )?;
        let feature_graph = build_feature_graph_atlas(
            &self.descriptor,
            &requested.regions,
            center_region,
            &mut self.graph_cache,
        )?;
        Ok(StreamedPlanAtlasSummary {
            schema: STREAMED_PLAN_ATLAS_SCHEMA_REVISION,
            research_only: true,
            production_terrain_unchanged: true,
            phase_two_witness_sha256: STREAMED_PLAN_PHASE_TWO_WITNESS_SHA256,
            seed: self.descriptor.seed.to_string(),
            topology: self.descriptor.topology.label(),
            period_blocks: (self.descriptor.topology != StreamedPlanTopology::Plane)
                .then_some(STREAMED_PLAN_PERIOD_BLOCKS),
            center_x,
            center_z,
            blocks_across,
            view_height_blocks: i32::try_from(view_height_blocks)
                .map_err(|_| "atlas view height exceeds signed 32-bit coordinates")?,
            base_region_blocks: STREAMED_PLAN_BASE_REGION_BLOCKS,
            coverage: requested.coverage,
            fallback,
            hierarchy,
            feature_graph,
        })
    }
}

struct RequestedAtlasRegions {
    regions: Vec<PlanRegion>,
    coverage: AtlasCoverage,
}

fn atlas_regions(
    center_x: i32,
    center_z: i32,
    blocks_across: i32,
    view_height_blocks: i64,
) -> Result<RequestedAtlasRegions, String> {
    let half_width = i64::from(blocks_across).div_euclid(2);
    let half_height = view_height_blocks.div_euclid(2);
    let view_min_x = i64::from(center_x) - half_width;
    let view_max_x = i64::from(center_x) + i64::from(blocks_across) - half_width - 1;
    let view_min_z = i64::from(center_z) - half_height;
    let view_max_z = i64::from(center_z) + view_height_blocks - half_height - 1;
    let base = i64::from(STREAMED_PLAN_BASE_REGION_BLOCKS);
    let full_min_region_x = view_min_x.div_euclid(base);
    let full_max_region_x = view_max_x.div_euclid(base);
    let full_min_region_z = view_min_z.div_euclid(base);
    let full_max_region_z = view_max_z.div_euclid(base);
    let center_region_x = i64::from(center_x).div_euclid(base);
    let center_region_z = i64::from(center_z).div_euclid(base);
    let (min_region_x, max_region_x, clipped_x) =
        clipped_region_axis(full_min_region_x, full_max_region_x, center_region_x);
    let (min_region_z, max_region_z, clipped_z) =
        clipped_region_axis(full_min_region_z, full_max_region_z, center_region_z);
    let mut regions = Vec::new();
    for z in min_region_z..=max_region_z {
        for x in min_region_x..=max_region_x {
            regions.push(PlanRegion::new(
                i32::try_from(x).map_err(|_| "atlas X region exceeds signed 32-bit range")?,
                i32::try_from(z).map_err(|_| "atlas Z region exceeds signed 32-bit range")?,
            ));
        }
    }
    let min_x = checked_region_min(min_region_x)?;
    let min_z = checked_region_min(min_region_z)?;
    let max_x = checked_region_min(max_region_x + 1)?;
    let max_z = checked_region_min(max_region_z + 1)?;
    Ok(RequestedAtlasRegions {
        regions,
        coverage: AtlasCoverage {
            min_x,
            min_z,
            max_x,
            max_z,
            region_columns: u32::try_from(max_region_x - min_region_x + 1)
                .map_err(|_| "atlas column count overflow")?,
            region_rows: u32::try_from(max_region_z - min_region_z + 1)
                .map_err(|_| "atlas row count overflow")?,
            clipped: clipped_x || clipped_z,
        },
    })
}

fn clipped_region_axis(minimum: i64, maximum: i64, center: i64) -> (i64, i64, bool) {
    let count = maximum - minimum + 1;
    let limit = i64::from(STREAMED_PLAN_ATLAS_MAX_REGIONS_PER_AXIS);
    if count <= limit {
        return (minimum, maximum, false);
    }
    let preferred = center - limit.div_euclid(2);
    let start = preferred.clamp(minimum, maximum - limit + 1);
    (start, start + limit - 1, true)
}

fn checked_region_min(region: i64) -> Result<i32, String> {
    let value = region
        .checked_mul(i64::from(STREAMED_PLAN_BASE_REGION_BLOCKS))
        .ok_or_else(|| "atlas region block coordinate overflow".to_owned())?;
    i32::try_from(value).map_err(|_| "atlas coverage exceeds signed 32-bit coordinates".to_owned())
}

fn build_fallback_atlas(
    descriptor: &StreamedPlanDescriptor,
    requested_regions: &[PlanRegion],
    center_region: PlanRegion,
    cache: &mut CandidateCache,
) -> Result<FallbackAtlas, String> {
    let mut delta = CacheDelta::default();
    let mut snapshots = BTreeMap::new();
    let mut cells = Vec::new();
    let mut samples = Vec::new();
    let mut features = BTreeMap::new();
    for requested in requested_regions {
        let (snapshot, next_delta) =
            cache.resolve::<CoordinatePureControl>(descriptor, *requested, center_region)?;
        delta.add(next_delta);
        snapshots.insert(snapshot.key.clone(), snapshot.clone());
        let identity = region_identity(*requested, &snapshot)?;
        let observer = region_center(*requested)?;
        let mut region_class = 0;
        for fact in &snapshot.facts {
            match fact.id.kind {
                FALLBACK_REGION_KIND => {
                    region_class = (fact.payload[0] as u64 % 6) as u8;
                }
                FALLBACK_SAMPLE_KIND => {
                    let (world_x, world_z) =
                        lift_block(descriptor, observer, fact.world_x, fact.world_z)?;
                    samples.push(FallbackAtlasSample {
                        world_x,
                        world_z,
                        surface_y: fact.payload[0] as i32,
                        water_surface_y: (fact.payload[1] != i64::from(i32::MIN))
                            .then_some(fact.payload[1] as i32),
                        ridge_lift: fact.payload[2] as i32,
                        channel: fact.payload[3] != 0,
                        island: fact.payload[4] != 0,
                        pond: fact.payload[5] != 0,
                        top_block: fact.payload[6] as u16,
                    });
                }
                FALLBACK_FEATURE_KIND => {
                    let (world_x, world_z) =
                        lift_block(descriptor, observer, fact.world_x, fact.world_z)?;
                    let feature = FallbackAtlasFeature {
                        canonical_id: (fact.payload[0] as u64).to_string(),
                        canonical_owner_x: fact.payload[1] as i32,
                        canonical_owner_z: fact.payload[2] as i32,
                        world_x,
                        world_z,
                        radius_blocks: fact.payload[3] as i32,
                    };
                    features
                        .entry((fact.payload[0], world_x, world_z))
                        .or_insert(feature);
                }
                _ => {}
            }
        }
        cells.push(FallbackAtlasCell {
            region: identity,
            region_class,
        });
    }
    Ok(FallbackAtlas {
        receipt: candidate_receipt(
            "coordinate-pure fallback",
            FALLBACK_CONTROL_REVISION,
            requested_regions.len(),
            &snapshots,
            delta,
            cache.entries.len(),
        ),
        cells,
        samples,
        features: features.into_values().collect(),
    })
}

fn build_hierarchy_atlas(
    descriptor: &StreamedPlanDescriptor,
    requested_regions: &[PlanRegion],
    center_region: PlanRegion,
    cache: &mut CandidateCache,
) -> Result<HierarchyAtlas, String> {
    let mut delta = CacheDelta::default();
    let mut snapshots = BTreeMap::new();
    let mut cells = Vec::new();
    let mut providers = BTreeMap::new();
    let mut facets = BTreeMap::new();
    for requested in requested_regions {
        let (snapshot, next_delta) = cache.resolve::<HierarchicalSharedFactsControl>(
            descriptor,
            *requested,
            center_region,
        )?;
        delta.add(next_delta);
        snapshots.insert(snapshot.key.clone(), snapshot.clone());
        let identity = region_identity(*requested, &snapshot)?;
        let observer = region_center(*requested)?;
        for fact in &snapshot.facts {
            match fact.id.kind {
                HIERARCHY_REGION_KIND => cells.push(HierarchyAtlasCell {
                    region: identity,
                    root_tendency: fact.payload[0] as i32,
                    mid_tendency: fact.payload[1] as i32,
                    port_mask: fact.payload[2] as u8,
                    character: fact.payload[3] as u8,
                }),
                HIERARCHY_ROOT_KIND | HIERARCHY_MID_KIND => {
                    let (center_x, center_z) =
                        lift_block(descriptor, observer, fact.world_x, fact.world_z)?;
                    let (level, extent) = if fact.id.kind == HIERARCHY_ROOT_KIND {
                        (2, HIERARCHY_ROOT_BLOCKS)
                    } else {
                        (1, HIERARCHY_MID_BLOCKS)
                    };
                    let provider = HierarchyAtlasProvider {
                        level,
                        world_min_x: center_x - extent / 2,
                        world_min_z: center_z - extent / 2,
                        extent_blocks: extent,
                        tendency: fact.payload[0] as i32,
                        class: fact.payload[1] as u8,
                    };
                    providers
                        .entry((level, provider.world_min_x, provider.world_min_z))
                        .or_insert(provider);
                }
                HIERARCHY_VERTICAL_FACET_KIND | HIERARCHY_HORIZONTAL_FACET_KIND => {
                    let (center_x, center_z) =
                        lift_block(descriptor, observer, fact.world_x, fact.world_z)?;
                    let vertical = fact.id.kind == HIERARCHY_VERTICAL_FACET_KIND;
                    let half = STREAMED_PLAN_BASE_REGION_BLOCKS / 2;
                    let facet = HierarchyAtlasFacet {
                        vertical,
                        world_a_x: if vertical { center_x } else { center_x - half },
                        world_a_z: if vertical { center_z - half } else { center_z },
                        world_b_x: if vertical { center_x } else { center_x + half },
                        world_b_z: if vertical { center_z + half } else { center_z },
                        open: fact.payload[0] != 0,
                        tendency: fact.payload[1] as i32,
                        direction: fact.payload[2] as i8,
                        strength: fact.payload[3] as u8,
                    };
                    facets
                        .entry((
                            vertical,
                            facet.world_a_x,
                            facet.world_a_z,
                            facet.world_b_x,
                            facet.world_b_z,
                        ))
                        .or_insert(facet);
                }
                _ => {}
            }
        }
    }
    Ok(HierarchyAtlas {
        receipt: candidate_receipt(
            "hierarchical shared facts",
            HIERARCHICAL_CANDIDATE_REVISION,
            requested_regions.len(),
            &snapshots,
            delta,
            cache.entries.len(),
        ),
        cells,
        providers: providers.into_values().collect(),
        facets: facets.into_values().collect(),
    })
}

fn build_feature_graph_atlas(
    descriptor: &StreamedPlanDescriptor,
    requested_regions: &[PlanRegion],
    center_region: PlanRegion,
    cache: &mut CandidateCache,
) -> Result<FeatureGraphAtlas, String> {
    let mut delta = CacheDelta::default();
    let mut snapshots = BTreeMap::new();
    let mut cells = Vec::new();
    let mut graphs = BTreeMap::new();
    for requested in requested_regions {
        let (snapshot, next_delta) =
            cache.resolve::<FeatureOwnedGraphControl>(descriptor, *requested, center_region)?;
        delta.add(next_delta);
        snapshots.insert(snapshot.key.clone(), snapshot.clone());
        let identity = region_identity(*requested, &snapshot)?;
        let observer = region_center(*requested)?;
        if let Some(query) = snapshot
            .facts
            .iter()
            .find(|fact| fact.id.kind == GRAPH_QUERY_KIND)
        {
            cells.push(FeatureGraphAtlasCell {
                region: identity,
                owner_cells_examined: query.payload[0] as u8,
                accepted_graphs: query.payload[1] as u8,
            });
        }
        let mut by_owner = BTreeMap::<StreamedPlanKey, Vec<&StreamedSemanticFact>>::new();
        for fact in &snapshot.facts {
            if fact.id.kind != GRAPH_QUERY_KIND {
                by_owner
                    .entry(fact.id.owner.clone())
                    .or_default()
                    .push(fact);
            }
        }
        for (owner, facts) in by_owner {
            let graph = atlas_graph(descriptor, observer, &owner, &facts)?;
            graphs
                .entry((
                    owner.region.x,
                    owner.region.z,
                    graph.anchor_x,
                    graph.anchor_z,
                ))
                .or_insert(graph);
        }
    }
    Ok(FeatureGraphAtlas {
        receipt: candidate_receipt(
            "feature-owned bounded graph",
            FEATURE_GRAPH_CANDIDATE_REVISION,
            requested_regions.len(),
            &snapshots,
            delta,
            cache.entries.len(),
        ),
        cells,
        graphs: graphs.into_values().collect(),
    })
}

fn atlas_graph(
    descriptor: &StreamedPlanDescriptor,
    observer: (i32, i32),
    owner: &StreamedPlanKey,
    facts: &[&StreamedSemanticFact],
) -> Result<FeatureGraphAtlasGraph, String> {
    let header = facts
        .iter()
        .copied()
        .find(|fact| fact.id.kind == GRAPH_HEADER_KIND)
        .ok_or_else(|| format!("graph owner {:?} has no header", owner.region))?;
    let (anchor_x, anchor_z) = lift_block(descriptor, observer, header.world_x, header.world_z)?;
    let mut nodes = BTreeMap::new();
    let mut edges = Vec::new();
    let mut sink_node = 0;
    let mut sink_kind = 0;
    for fact in facts {
        match fact.id.kind {
            GRAPH_NODE_KIND => {
                nodes.insert(
                    fact.id.local_index,
                    FeatureGraphAtlasNode {
                        index: fact.id.local_index as u8,
                        world_x: anchor_x + fact.payload[0] as i32,
                        world_z: anchor_z + fact.payload[1] as i32,
                        potential: fact.payload[2] as i32,
                    },
                );
            }
            GRAPH_EDGE_KIND => edges.push(FeatureGraphAtlasEdge {
                from: fact.payload[0] as u8,
                to: fact.payload[1] as u8,
                width: fact.payload[2] as u8,
            }),
            GRAPH_SINK_KIND => {
                sink_node = fact.payload[0] as u8;
                sink_kind = fact.payload[1] as u8;
            }
            _ => {}
        }
    }
    edges.sort_by_key(|edge| (edge.from, edge.to));
    Ok(FeatureGraphAtlasGraph {
        canonical_owner_x: owner.region.x,
        canonical_owner_z: owner.region.z,
        anchor_x,
        anchor_z,
        bounds_min_x: anchor_x + header.payload[0] as i32,
        bounds_min_z: anchor_z + header.payload[2] as i32,
        bounds_max_x: anchor_x + header.payload[1] as i32,
        bounds_max_z: anchor_z + header.payload[3] as i32,
        maximum_reach_blocks: header.payload[4] as u32,
        nodes: nodes.into_values().collect(),
        edges,
        sink_node,
        sink_kind,
    })
}

fn region_identity(
    requested: PlanRegion,
    snapshot: &StreamedPlanSnapshot,
) -> Result<AtlasRegionIdentity, String> {
    Ok(AtlasRegionIdentity {
        requested_region_x: requested.x,
        requested_region_z: requested.z,
        canonical_region_x: snapshot.key.region.x,
        canonical_region_z: snapshot.key.region.z,
        world_min_x: checked_region_min(i64::from(requested.x))?,
        world_min_z: checked_region_min(i64::from(requested.z))?,
    })
}

fn region_center(region: PlanRegion) -> Result<(i32, i32), String> {
    let min_x = checked_region_min(i64::from(region.x))?;
    let min_z = checked_region_min(i64::from(region.z))?;
    Ok((
        min_x + STREAMED_PLAN_BASE_REGION_BLOCKS / 2,
        min_z + STREAMED_PLAN_BASE_REGION_BLOCKS / 2,
    ))
}

fn lift_block(
    descriptor: &StreamedPlanDescriptor,
    observer: (i32, i32),
    canonical_x: i32,
    canonical_z: i32,
) -> Result<(i32, i32), String> {
    let topology = descriptor.topology.horizontal();
    let x = i64::from(observer.0)
        + topology
            .x
            .shortest_block_displacement(observer.0, canonical_x);
    let z = i64::from(observer.1)
        + topology
            .z
            .shortest_block_displacement(observer.1, canonical_z);
    Ok((
        i32::try_from(x).map_err(|_| "lifted atlas X coordinate overflow")?,
        i32::try_from(z).map_err(|_| "lifted atlas Z coordinate overflow")?,
    ))
}

fn candidate_receipt(
    candidate: &'static str,
    candidate_revision: &'static str,
    viewed_plan_count: usize,
    snapshots: &BTreeMap<StreamedPlanKey, StreamedPlanSnapshot>,
    delta: CacheDelta,
    retained_plans: usize,
) -> AtlasCandidateReceipt {
    AtlasCandidateReceipt {
        candidate,
        candidate_revision,
        semantic_sha256: atlas_snapshot_sha256(candidate_revision, snapshots),
        viewed_plan_count: viewed_plan_count as u32,
        canonical_plan_count: snapshots.len() as u32,
        cache_hits: delta.hits,
        cache_misses: delta.misses,
        cache_evictions: delta.evictions,
        retained_plans: retained_plans as u32,
    }
}

fn atlas_snapshot_sha256(
    candidate_revision: &str,
    snapshots: &BTreeMap<StreamedPlanKey, StreamedPlanSnapshot>,
) -> String {
    let mut digest = Sha256::new();
    digest.update(STREAMED_PLAN_ATLAS_SCHEMA_REVISION.as_bytes());
    digest.update(candidate_revision.as_bytes());
    for snapshot in snapshots.values() {
        digest.update(snapshot.semantic_sha256().as_bytes());
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_atlas_moves_with_the_view_and_reuses_overlap() {
        let mut compiler = StreamedPlanAtlasCompiler::new(12_345, StreamedPlanTopology::Plane);
        let first = compiler.query(0, 0, 6_144, 1.0).unwrap();
        let second = compiler.query(1_024, 0, 6_144, 1.0).unwrap();
        assert_eq!(first.coverage.region_columns, 6);
        assert_eq!(first.coverage.region_rows, 6);
        assert_eq!(second.coverage.min_x - first.coverage.min_x, 1_024);
        assert!(second.fallback.receipt.cache_hits > 0);
        assert!(second.hierarchy.receipt.cache_hits > 0);
        assert!(second.feature_graph.receipt.cache_hits > 0);
        assert!(!second.coverage.clipped);
    }

    #[test]
    fn periodic_lift_repeats_canonical_semantics_without_fixing_the_view() {
        for topology in [StreamedPlanTopology::CylinderX, StreamedPlanTopology::Torus] {
            let mut compiler = StreamedPlanAtlasCompiler::new(-98_765, topology);
            let origin = compiler.query(0, 0, 4_096, 1.0).unwrap();
            compiler.clear_cache();
            let shifted_z = if topology == StreamedPlanTopology::Torus {
                STREAMED_PLAN_PERIOD_BLOCKS
            } else {
                0
            };
            let repeated = compiler
                .query(STREAMED_PLAN_PERIOD_BLOCKS, shifted_z, 4_096, 1.0)
                .unwrap();
            assert_eq!(
                origin.fallback.receipt.semantic_sha256,
                repeated.fallback.receipt.semantic_sha256
            );
            assert_eq!(
                origin.hierarchy.receipt.semantic_sha256,
                repeated.hierarchy.receipt.semantic_sha256
            );
            assert_eq!(
                origin.feature_graph.receipt.semantic_sha256,
                repeated.feature_graph.receipt.semantic_sha256
            );
            assert_eq!(
                repeated.fallback.cells[0].region.world_min_x
                    - origin.fallback.cells[0].region.world_min_x,
                STREAMED_PLAN_PERIOD_BLOCKS
            );
        }
    }

    #[test]
    fn cold_rebuild_preserves_the_exact_atlas_checksums() {
        let mut compiler = StreamedPlanAtlasCompiler::new(8_675_309, StreamedPlanTopology::Torus);
        let warm = compiler.query(5_900, 5_900, 6_144, 1.5).unwrap();
        compiler.clear_cache();
        let cold = compiler.query(5_900, 5_900, 6_144, 1.5).unwrap();
        assert_eq!(
            warm.fallback.receipt.semantic_sha256,
            cold.fallback.receipt.semantic_sha256
        );
        assert_eq!(
            warm.hierarchy.receipt.semantic_sha256,
            cold.hierarchy.receipt.semantic_sha256
        );
        assert_eq!(
            warm.feature_graph.receipt.semantic_sha256,
            cold.feature_graph.receipt.semantic_sha256
        );
        assert!(cold.fallback.receipt.cache_misses > 0);
        assert_eq!(
            cold.fallback.receipt.cache_misses,
            cold.fallback.receipt.canonical_plan_count
        );
    }

    #[test]
    fn atlas_clips_cost_without_becoming_origin_fixed() {
        let mut compiler =
            StreamedPlanAtlasCompiler::with_cache_capacity(12_345, StreamedPlanTopology::Plane, 8);
        let broad = compiler.query(100_000, -200_000, 131_072, 1.0).unwrap();
        assert!(broad.coverage.clipped);
        assert_eq!(
            broad.coverage.region_columns,
            STREAMED_PLAN_ATLAS_MAX_REGIONS_PER_AXIS as u32
        );
        assert_eq!(
            broad.coverage.region_rows,
            STREAMED_PLAN_ATLAS_MAX_REGIONS_PER_AXIS as u32
        );
        assert!(broad.fallback.receipt.cache_evictions > 0);
        assert!(broad.fallback.receipt.retained_plans <= 8);
        assert!(broad.coverage.min_x > 80_000);
        assert!(broad.coverage.max_z < -180_000);
    }

    #[test]
    fn atlas_keeps_the_phase_two_witness_as_its_evidence_anchor() {
        let summary = StreamedPlanAtlasCompiler::new(12_345, StreamedPlanTopology::Plane)
            .query(0, 0, 1_024, 1.0)
            .unwrap();
        assert_eq!(
            summary.fallback.receipt.semantic_sha256,
            STREAMED_PLAN_ATLAS_FALLBACK_WITNESS_SHA256
        );
        assert_eq!(
            summary.hierarchy.receipt.semantic_sha256,
            STREAMED_PLAN_ATLAS_HIERARCHY_WITNESS_SHA256
        );
        assert_eq!(
            summary.feature_graph.receipt.semantic_sha256,
            STREAMED_PLAN_ATLAS_GRAPH_WITNESS_SHA256
        );
        assert_eq!(
            summary.phase_two_witness_sha256,
            STREAMED_PLAN_PHASE_TWO_WITNESS_SHA256
        );
        assert!(summary.research_only);
        assert!(summary.production_terrain_unchanged);
    }
}
