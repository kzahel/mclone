//! Exact native/Wasm corpus for the continental broad-surface candidate.

use std::collections::BTreeSet;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    continental_ecoregion::{ContinentalEcoregionDescriptor, ContinentalEcoregionTopology},
    continental_surface::{
        CONTINENTAL_SURFACE_FAMILY_COUNT, ContinentalSurfaceConstructionCounts,
        ContinentalSurfacePlan, ContinentalSurfaceSample, ContinentalSurfaceWindowRequest,
    },
};

pub const CONTINENTAL_SURFACE_HARNESS_SCHEMA_REVISION: &str =
    "mclone-continental-surface-harness-v1";
pub const CONTINENTAL_SURFACE_WITNESS_SHA256: &str =
    "0760b0f2bda0c798d21bee7be873f1255c30b28dbd863967046cd56f15f68c84";

const CORPUS_SEEDS: [i64; 2] = [12_345, -98_765];
const CORPUS_PERIOD_BLOCKS: i32 = 196_608;
const CORPUS_MIN_X: i32 = -65_536;
const CORPUS_MIN_Z: i32 = -49_152;
const CORPUS_WIDTH: u32 = 33;
const CORPUS_DEPTH: u32 = 25;
const CORPUS_STEP: u32 = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContinentalSurfaceCorpusTopology {
    Plane,
    CylinderX196608,
}

impl ContinentalSurfaceCorpusTopology {
    const ALL: [Self; 2] = [Self::Plane, Self::CylinderX196608];

    const fn topology(self) -> ContinentalEcoregionTopology {
        match self {
            Self::Plane => ContinentalEcoregionTopology::Plane,
            Self::CylinderX196608 => ContinentalEcoregionTopology::CylinderX {
                period_blocks: CORPUS_PERIOD_BLOCKS,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalSurfaceCaseReceipt {
    pub seed: i64,
    pub topology: ContinentalSurfaceCorpusTopology,
    pub semantic_sha256: String,
    pub sample_count: u32,
    pub family_counts: [u32; CONTINENTAL_SURFACE_FAMILY_COUNT],
    pub water_kind_count: u32,
    pub substrate_count: u32,
    pub water_samples: u32,
    pub exact_comparisons: u32,
    pub work: ContinentalSurfaceConstructionCounts,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalSurfaceSuiteReceipt {
    pub schema_revision: &'static str,
    pub suite_passed: bool,
    pub witness_sha256: String,
    pub exact_comparisons: u32,
    pub cases: Vec<ContinentalSurfaceCaseReceipt>,
}

pub fn run_continental_surface_suite() -> Result<ContinentalSurfaceSuiteReceipt, String> {
    let mut cases = Vec::new();
    let mut exact_comparisons = 0_u32;
    for seed in CORPUS_SEEDS {
        for topology in ContinentalSurfaceCorpusTopology::ALL {
            let (case, comparisons) = run_case(seed, topology)?;
            exact_comparisons = exact_comparisons
                .checked_add(comparisons)
                .ok_or_else(|| "surface comparison count overflow".to_owned())?;
            cases.push(case);
        }
    }
    let witness_sha256 = case_witness_sha256(&cases)?;
    Ok(ContinentalSurfaceSuiteReceipt {
        schema_revision: CONTINENTAL_SURFACE_HARNESS_SCHEMA_REVISION,
        suite_passed: true,
        witness_sha256,
        exact_comparisons,
        cases,
    })
}

fn run_case(
    seed: i64,
    topology: ContinentalSurfaceCorpusTopology,
) -> Result<(ContinentalSurfaceCaseReceipt, u32), String> {
    let surface = ContinentalSurfacePlan::new(ContinentalEcoregionDescriptor::new(
        seed,
        topology.topology(),
    ))
    .map_err(|error| error.to_string())?;
    let request = ContinentalSurfaceWindowRequest::new(
        CORPUS_MIN_X,
        CORPUS_MIN_Z,
        CORPUS_WIDTH,
        CORPUS_DEPTH,
        CORPUS_STEP,
    );
    let whole = surface
        .query_window(request)
        .map_err(|error| error.to_string())?;
    validate_work(whole.work, whole.samples.len())?;
    let mut exact_comparisons = 0_u32;

    let split = 11_u32;
    let left = surface
        .query_window(ContinentalSurfaceWindowRequest {
            width_samples: split,
            ..request
        })
        .map_err(|error| error.to_string())?;
    let right = surface
        .query_window(ContinentalSurfaceWindowRequest {
            min_x: request.min_x + split as i32 * request.step_blocks as i32,
            width_samples: request.width_samples - split,
            ..request
        })
        .map_err(|error| error.to_string())?;
    for row in 0..request.depth_samples as usize {
        let whole_start = row * request.width_samples as usize;
        let left_start = row * left.request.width_samples as usize;
        let right_start = row * right.request.width_samples as usize;
        compare(
            &whole.samples[whole_start..whole_start + split as usize],
            &left.samples[left_start..left_start + split as usize],
            "left partition",
        )?;
        compare(
            &whole.samples
                [whole_start + split as usize..whole_start + request.width_samples as usize],
            &right.samples[right_start..right_start + right.request.width_samples as usize],
            "right partition",
        )?;
        exact_comparisons += request.width_samples;
    }

    let mut traversal = (0..whole.samples.len()).collect::<Vec<_>>();
    traversal.sort_by_key(|index| permutation_key(*index as u64, seed));
    for index in traversal {
        let expected = whole.samples[index];
        let actual = surface.query_point(expected.world_x, expected.world_z);
        if actual.sample != expected {
            return Err(format!(
                "surface seed {seed} point mismatch at corpus sample {index}"
            ));
        }
        validate_work(actual.work, 1)?;
        exact_comparisons += 1;
    }

    if topology == ContinentalSurfaceCorpusTopology::CylinderX196608 {
        for expected in whole.samples.iter().step_by(9) {
            let lifted =
                surface.query_point(expected.world_x + CORPUS_PERIOD_BLOCKS, expected.world_z);
            let mut normalized = lifted.sample;
            normalized.world_x -= CORPUS_PERIOD_BLOCKS;
            if normalized != *expected {
                return Err(format!(
                    "surface seed {seed} periodic lift mismatch at ({}, {})",
                    expected.world_x, expected.world_z
                ));
            }
            exact_comparisons += 1;
        }
    }

    let mut family_counts = [0_u32; CONTINENTAL_SURFACE_FAMILY_COUNT];
    let mut water_kinds = BTreeSet::new();
    let mut substrates = BTreeSet::new();
    let mut water_samples = 0_u32;
    for sample in &whole.samples {
        family_counts[sample.dominant_family as usize] += 1;
        water_kinds.insert(sample.water_kind as u8);
        substrates.insert(sample.substrate as u8);
        water_samples += u32::from(sample.is_water());
    }
    if family_counts.iter().any(|count| *count == 0) || water_samples == 0 || substrates.len() < 3 {
        return Err(format!(
            "surface seed {seed} {:?} corpus lacks contrast: families={family_counts:?}, \
             water={water_samples}, substrates={}",
            topology,
            substrates.len(),
        ));
    }

    let receipt = ContinentalSurfaceCaseReceipt {
        seed,
        topology,
        semantic_sha256: whole.semantic_sha256,
        sample_count: whole.samples.len() as u32,
        family_counts,
        water_kind_count: water_kinds.len() as u32,
        substrate_count: substrates.len() as u32,
        water_samples,
        exact_comparisons,
        work: whole.work,
    };
    Ok((receipt, exact_comparisons))
}

fn validate_work(
    work: ContinentalSurfaceConstructionCounts,
    sample_count: usize,
) -> Result<(), String> {
    let samples = sample_count as u64;
    if work.requested_samples != samples
        || work.continental_owner_evaluations != samples * 25
        || work.province_owner_evaluations > samples * 9
        || work.ecoregion_owner_evaluations > samples * 9
        || work.mosaic_owner_evaluations > samples * 9
        || work.plan_field_evaluations > samples * 10
        || work.surface_field_evaluations != samples * 7
        || work.exact_chunks != 0
        || work.density_volumes != 0
        || work.feature_batches != 0
    {
        return Err("surface work exceeded its declared direct-query bounds".to_owned());
    }
    Ok(())
}

fn compare(
    expected: &[ContinentalSurfaceSample],
    actual: &[ContinentalSurfaceSample],
    label: &str,
) -> Result<(), String> {
    if expected == actual {
        Ok(())
    } else {
        Err(format!("surface {label} mismatch"))
    }
}

fn permutation_key(index: u64, seed: i64) -> u64 {
    let mut value = index ^ seed as u64;
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn case_witness_sha256(cases: &[ContinentalSurfaceCaseReceipt]) -> Result<String, String> {
    let canonical = serde_json::to_vec(cases)
        .map_err(|error| format!("serialize continental surface cases: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(CONTINENTAL_SURFACE_HARNESS_SCHEMA_REVISION.as_bytes());
    digest.update(canonical);
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_suite_passes_and_exercises_surface_families() {
        let receipt = run_continental_surface_suite().unwrap();
        assert!(receipt.suite_passed);
        assert_eq!(receipt.witness_sha256, CONTINENTAL_SURFACE_WITNESS_SHA256);
        assert_eq!(receipt.cases.len(), 4);
        assert!(receipt.exact_comparisons > 3_000);
    }

    #[test]
    fn native_threads_publish_one_surface_witness() {
        let expected = run_continental_surface_suite().unwrap().witness_sha256;
        let handles = (0..4)
            .map(|_| std::thread::spawn(run_continental_surface_suite))
            .collect::<Vec<_>>();
        for handle in handles {
            let receipt = handle
                .join()
                .expect("surface suite thread")
                .expect("surface suite");
            assert_eq!(receipt.witness_sha256, expected);
        }
    }
}
