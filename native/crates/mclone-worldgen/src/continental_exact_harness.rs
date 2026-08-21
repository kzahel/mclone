//! Deterministic direct-versus-exact evidence for continental review sites.

use serde::Serialize;
use sha2::{Digest, Sha256};

use mclone_core::{CHUNK_WIDTH, chunk_min_block_coord};

use crate::{
    block::{
        ACACIA_LEAVES, ACACIA_LOG, AIR, OAK_LEAVES, OAK_LOG, SPRUCE_LEAVES, SPRUCE_LOG, WATER,
    },
    continental_ecoregion::ContinentalEcoregionDescriptor,
    continental_surface::{ContinentalSurfaceWaterKind, continental_surface_biome_id},
    continental_surface_journey::{
        ContinentalSurfaceJourneyKind, compile_continental_surface_journeys,
    },
    levelgen::{
        CONTINENTAL_CANDIDATE_EXACT_REVISION, ContinentalCandidateExactGenerator,
        ContinentalCandidateFeatureDependencyCache, McloneTreeArchetype, McloneVegetationBounds,
        continental_candidate_stratum, quantized_continental_candidate_surface_y,
    },
    terrain_preview::continental_candidate_tree_records_intersecting,
};

pub const CONTINENTAL_EXACT_REVIEW_SCHEMA_REVISION: &str = "mclone-continental-exact-review-v1";
pub const CONTINENTAL_EXACT_REVIEW_RADIUS_CHUNKS: i32 = 2;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalExactSiteReceipt {
    pub label: &'static str,
    pub journey: &'static str,
    pub center_x: i32,
    pub center_z: i32,
    pub min_chunk_x: i32,
    pub min_chunk_z: i32,
    pub max_chunk_x: i32,
    pub max_chunk_z: i32,
    pub exact_chunks: u32,
    pub compared_columns: u32,
    pub compared_biomes: u32,
    pub height_mismatches: u32,
    pub material_mismatches: u32,
    pub water_mismatches: u32,
    pub biome_mismatches: u32,
    pub intersecting_tree_records: u32,
    pub owned_tree_bases: u32,
    pub tree_base_mismatches: u32,
    pub exact_tree_voxels: u32,
    pub feature_dependency_requests: u32,
    pub feature_dependency_cache_hits: u32,
    pub feature_dependency_chunks_generated: u32,
    pub retained_feature_dependency_chunks: u32,
    pub exact_sha256: String,
}

impl ContinentalExactSiteReceipt {
    pub const fn passed(&self) -> bool {
        self.height_mismatches == 0
            && self.material_mismatches == 0
            && self.water_mismatches == 0
            && self.biome_mismatches == 0
            && self.tree_base_mismatches == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalExactReviewReceipt {
    pub schema_revision: &'static str,
    pub candidate_exact_revision: u16,
    pub seed: i64,
    pub radius_chunks: i32,
    pub suite_passed: bool,
    pub semantic_sha256: String,
    pub sites: Vec<ContinentalExactSiteReceipt>,
}

pub fn run_continental_exact_review(seed: i64) -> Result<ContinentalExactReviewReceipt, String> {
    let catalog = compile_continental_surface_journeys(ContinentalEcoregionDescriptor::plane(seed))
        .map_err(|error| error.to_string())?;
    let clearing = catalog
        .journey(ContinentalSurfaceJourneyKind::ClearingBetweenForestCores)
        .ok_or_else(|| "missing clearing journey".to_owned())?;
    let water = catalog
        .journey(ContinentalSurfaceJourneyKind::ConnectedWaterCountry)
        .ok_or_else(|| "missing connected-water journey".to_owned())?;
    let water_checkpoint = water
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.water_kind != ContinentalSurfaceWaterKind::None)
        .ok_or_else(|| "connected-water journey has no water-owning checkpoint".to_owned())?;
    let arid = catalog
        .journey(ContinentalSurfaceJourneyKind::UplandToAridBasin)
        .ok_or_else(|| "missing upland-to-arid journey".to_owned())?;

    let generator = ContinentalCandidateExactGenerator::new(seed);
    let mut feature_cache = ContinentalCandidateFeatureDependencyCache::new(seed);
    let site_specs = [
        (
            "clearing",
            clearing.kind.label(),
            clearing.center_x,
            clearing.center_z,
        ),
        (
            "water",
            water.kind.label(),
            water_checkpoint.world_x,
            water_checkpoint.world_z,
        ),
        ("arid", arid.kind.label(), arid.center_x, arid.center_z),
    ];
    let mut sites = Vec::with_capacity(site_specs.len());
    for (label, journey, center_x, center_z) in site_specs {
        sites.push(review_site(
            label,
            journey,
            center_x,
            center_z,
            &generator,
            &mut feature_cache,
        )?);
    }
    let suite_passed = sites.iter().all(ContinentalExactSiteReceipt::passed);
    let canonical = serde_json::to_vec(&sites)
        .map_err(|error| format!("serialize continental exact sites: {error}"))?;
    let semantic_sha256 = digest_bytes([
        CONTINENTAL_EXACT_REVIEW_SCHEMA_REVISION.as_bytes(),
        &canonical,
    ]);
    Ok(ContinentalExactReviewReceipt {
        schema_revision: CONTINENTAL_EXACT_REVIEW_SCHEMA_REVISION,
        candidate_exact_revision: CONTINENTAL_CANDIDATE_EXACT_REVISION,
        seed,
        radius_chunks: CONTINENTAL_EXACT_REVIEW_RADIUS_CHUNKS,
        suite_passed,
        semantic_sha256,
        sites,
    })
}

fn review_site(
    label: &'static str,
    journey: &'static str,
    center_x: i32,
    center_z: i32,
    generator: &ContinentalCandidateExactGenerator,
    feature_cache: &mut ContinentalCandidateFeatureDependencyCache,
) -> Result<ContinentalExactSiteReceipt, String> {
    let center_chunk_x = center_x.div_euclid(CHUNK_WIDTH);
    let center_chunk_z = center_z.div_euclid(CHUNK_WIDTH);
    let min_chunk_x = center_chunk_x - CONTINENTAL_EXACT_REVIEW_RADIUS_CHUNKS;
    let min_chunk_z = center_chunk_z - CONTINENTAL_EXACT_REVIEW_RADIUS_CHUNKS;
    let max_chunk_x = center_chunk_x + CONTINENTAL_EXACT_REVIEW_RADIUS_CHUNKS;
    let max_chunk_z = center_chunk_z + CONTINENTAL_EXACT_REVIEW_RADIUS_CHUNKS;
    let bounds = McloneVegetationBounds::new(
        chunk_min_block_coord(min_chunk_x),
        chunk_min_block_coord(min_chunk_z),
        chunk_min_block_coord(max_chunk_x) + CHUNK_WIDTH - 1,
        chunk_min_block_coord(max_chunk_z) + CHUNK_WIDTH - 1,
    )
    .map_err(|error| error.to_string())?;
    let tree_records = continental_candidate_tree_records_intersecting(generator.seed(), bounds)
        .map_err(|error| error.to_string())?;

    let mut height_mismatches = 0_u32;
    let mut material_mismatches = 0_u32;
    let mut water_mismatches = 0_u32;
    let mut biome_mismatches = 0_u32;
    let mut compared_columns = 0_u32;
    let mut compared_biomes = 0_u32;
    let mut owned_tree_bases = 0_u32;
    let mut tree_base_mismatches = 0_u32;
    let mut exact_tree_voxels = 0_u32;
    let mut feature_dependency_requests = 0_u32;
    let mut feature_dependency_cache_hits = 0_u32;
    let mut feature_dependency_chunks_generated = 0_u32;
    let mut retained_feature_dependency_chunks = 0_u32;
    let mut exact_digest = Sha256::new();

    for chunk_z in min_chunk_z..=max_chunk_z {
        for chunk_x in min_chunk_x..=max_chunk_x {
            let min_x = chunk_min_block_coord(chunk_x);
            let min_z = chunk_min_block_coord(chunk_z);
            let surface = generator.generate_surface_chunk(chunk_x, chunk_z);
            for local_z in 0..CHUNK_WIDTH {
                for local_x in 0..CHUNK_WIDTH {
                    compared_columns += 1;
                    let sample = generator
                        .surface()
                        .query_point(min_x + local_x, min_z + local_z)
                        .sample;
                    let expected_y = quantized_continental_candidate_surface_y(sample);
                    let actual_y = highest_solid_y(&surface, local_x, local_z);
                    height_mismatches += u32::from(actual_y != Some(expected_y));
                    material_mismatches += u32::from(
                        surface.block_at_y(local_x, expected_y, local_z).raw()
                            != continental_candidate_stratum(sample.substrate, 0),
                    );
                    let expected_water = sample
                        .water_level_y
                        .filter(|level| *level > sample.solid_surface_y)
                        .map(|level| level.floor() as i32);
                    water_mismatches +=
                        u32::from(highest_water_y(&surface, local_x, local_z) != expected_water);
                }
            }
            let quart_width = CHUNK_WIDTH / 4;
            let quart_height = surface.height / 4;
            for quart_y in 0..quart_height {
                for quart_z in 0..quart_width {
                    for quart_x in 0..quart_width {
                        compared_biomes += 1;
                        let index = (quart_y * quart_width * quart_width
                            + quart_z * quart_width
                            + quart_x) as usize;
                        let sample = generator
                            .surface()
                            .query_point(min_x + quart_x * 4 + 2, min_z + quart_z * 4 + 2)
                            .sample;
                        biome_mismatches += u32::from(
                            surface.biomes()[index] != continental_surface_biome_id(sample),
                        );
                    }
                }
            }

            let (final_chunk, report) = feature_cache.generate_features_chunk(chunk_x, chunk_z);
            feature_dependency_requests += report.requested_dependency_chunks as u32;
            feature_dependency_cache_hits += report.cache_hits as u32;
            feature_dependency_chunks_generated += report.generated_dependency_chunks as u32;
            retained_feature_dependency_chunks = report.retained_dependency_chunks as u32;
            exact_tree_voxels += tree_voxel_count(&final_chunk) as u32;
            for occurrence in &tree_records {
                let base = occurrence
                    .working_base()
                    .map_err(|error| error.to_string())?;
                if base.x.div_euclid(CHUNK_WIDTH) != chunk_x
                    || base.z.div_euclid(CHUNK_WIDTH) != chunk_z
                {
                    continue;
                }
                owned_tree_bases += 1;
                let expected = match occurrence.record.archetype {
                    McloneTreeArchetype::RoundedBroadleaf => OAK_LOG,
                    McloneTreeArchetype::LayeredConifer => SPRUCE_LOG,
                    McloneTreeArchetype::ForkedAcacia => ACACIA_LOG,
                };
                tree_base_mismatches += u32::from(
                    final_chunk
                        .block_at_y(
                            base.x.rem_euclid(CHUNK_WIDTH),
                            base.y,
                            base.z.rem_euclid(CHUNK_WIDTH),
                        )
                        .raw()
                        != expected,
                );
            }

            exact_digest.update(chunk_x.to_le_bytes());
            exact_digest.update(chunk_z.to_le_bytes());
            for block in final_chunk.blocks() {
                exact_digest.update(block.to_le_bytes());
            }
            for biome in final_chunk.biomes() {
                exact_digest.update(biome.to_le_bytes());
            }
        }
    }

    Ok(ContinentalExactSiteReceipt {
        label,
        journey,
        center_x,
        center_z,
        min_chunk_x,
        min_chunk_z,
        max_chunk_x,
        max_chunk_z,
        exact_chunks: ((max_chunk_x - min_chunk_x + 1) * (max_chunk_z - min_chunk_z + 1)) as u32,
        compared_columns,
        compared_biomes,
        height_mismatches,
        material_mismatches,
        water_mismatches,
        biome_mismatches,
        intersecting_tree_records: tree_records.len() as u32,
        owned_tree_bases,
        tree_base_mismatches,
        exact_tree_voxels,
        feature_dependency_requests,
        feature_dependency_cache_hits,
        feature_dependency_chunks_generated,
        retained_feature_dependency_chunks,
        exact_sha256: digest_hex(exact_digest.finalize()),
    })
}

fn highest_solid_y(
    chunk: &crate::levelgen::GeneratedChunk,
    local_x: i32,
    local_z: i32,
) -> Option<i32> {
    (chunk.min_y..chunk.min_y + chunk.height)
        .rev()
        .find(|y| !matches!(chunk.block_at_y(local_x, *y, local_z).raw(), AIR | WATER))
}

fn highest_water_y(
    chunk: &crate::levelgen::GeneratedChunk,
    local_x: i32,
    local_z: i32,
) -> Option<i32> {
    (chunk.min_y..chunk.min_y + chunk.height)
        .rev()
        .find(|y| chunk.block_at_y(local_x, *y, local_z).raw() == WATER)
}

fn tree_voxel_count(chunk: &crate::levelgen::GeneratedChunk) -> usize {
    [
        OAK_LOG,
        OAK_LEAVES,
        SPRUCE_LOG,
        SPRUCE_LEAVES,
        ACACIA_LOG,
        ACACIA_LEAVES,
    ]
    .into_iter()
    .map(|block| chunk.block_count(block))
    .sum()
}

fn digest_bytes<const N: usize>(parts: [&[u8]; N]) -> String {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update(part);
    }
    digest_hex(digest.finalize())
}

fn digest_hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_review_sites_match_direct_surface_and_cover_facts() {
        let receipt = run_continental_exact_review(12_345).unwrap();
        assert!(receipt.suite_passed);
        assert_eq!(receipt.sites.len(), 3);
        assert!(receipt.sites.iter().all(|site| site.exact_chunks == 25));
        assert!(
            receipt
                .sites
                .iter()
                .all(|site| site.compared_columns == 6_400)
        );
        assert!(
            receipt
                .sites
                .iter()
                .all(|site| site.compared_biomes == 25_600)
        );
        assert!(
            receipt
                .sites
                .iter()
                .any(|site| site.owned_tree_bases > 0 && site.exact_tree_voxels > 0)
        );
    }
}
