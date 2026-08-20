//! Exact cross-query corpus for the continental/ecoregion candidate.

use std::collections::BTreeSet;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::continental_ecoregion::{
    ContinentalEcoregionDescriptor, ContinentalEcoregionPlan, ContinentalEcoregionTopology,
    LandscapePlanDetail, LandscapePlanSample, LandscapeWindowRequest, PlanConstructionCounts,
};

pub const CONTINENTAL_ECOREGION_HARNESS_SCHEMA_REVISION: &str =
    "mclone-continental-ecoregion-harness-v1";
pub const CONTINENTAL_ECOREGION_WITNESS_SHA256: &str =
    "cd1a8638630df83d4fc5b9f642da9e9dbbbdd06fd1169558a98e9621cf2ef925";
pub const CONTINENTAL_ECOREGION_CORPUS_SEEDS: [i64; 3] = [12_345, 8_675_309, -98_765];
pub const CONTINENTAL_ECOREGION_CORPUS_PERIOD_BLOCKS: i32 = 196_608;

const CORPUS_WIDTH: u32 = 33;
const CORPUS_DEPTH: u32 = 25;
const CORPUS_STEP_BLOCKS: u32 = 2_048;
const CORPUS_MIN_X: i32 = -32_768;
const CORPUS_MIN_Z: i32 = -24_576;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CorpusTopology {
    Plane,
    CylinderX196608,
}

impl CorpusTopology {
    pub const ALL: [Self; 2] = [Self::Plane, Self::CylinderX196608];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Plane => "plane",
            Self::CylinderX196608 => "cylinder-x-196608",
        }
    }

    const fn topology(self) -> ContinentalEcoregionTopology {
        match self {
            Self::Plane => ContinentalEcoregionTopology::Plane,
            Self::CylinderX196608 => ContinentalEcoregionTopology::CylinderX {
                period_blocks: CONTINENTAL_ECOREGION_CORPUS_PERIOD_BLOCKS,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalEcoregionCaseReceipt {
    pub seed: i64,
    pub topology: CorpusTopology,
    pub semantic_sha256: String,
    pub sample_count: u32,
    pub land_samples: u32,
    pub continent_count: u32,
    pub province_count: u32,
    pub province_kind_count: u32,
    pub ecoregion_count: u32,
    pub ecoregion_kind_count: u32,
    pub clearing_count: u32,
    pub clearing_cause_count: u32,
    pub exact_comparisons: u32,
    pub work: PlanConstructionCounts,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalEcoregionSuiteReceipt {
    pub schema_revision: &'static str,
    pub suite_passed: bool,
    pub witness_sha256: String,
    pub exact_comparisons: u32,
    pub cases: Vec<ContinentalEcoregionCaseReceipt>,
}

pub fn run_continental_ecoregion_suite() -> Result<ContinentalEcoregionSuiteReceipt, String> {
    let mut cases = Vec::new();
    let mut exact_comparisons = 0_u32;
    for seed in CONTINENTAL_ECOREGION_CORPUS_SEEDS {
        for topology in CorpusTopology::ALL {
            let (case, comparisons) = run_case(seed, topology)?;
            exact_comparisons = exact_comparisons
                .checked_add(comparisons)
                .ok_or_else(|| "comparison count overflow".to_owned())?;
            cases.push(case);
        }
    }
    let witness_sha256 = case_witness_sha256(&cases)?;
    Ok(ContinentalEcoregionSuiteReceipt {
        schema_revision: CONTINENTAL_ECOREGION_HARNESS_SCHEMA_REVISION,
        suite_passed: true,
        witness_sha256,
        exact_comparisons,
        cases,
    })
}

fn run_case(
    seed: i64,
    topology: CorpusTopology,
) -> Result<(ContinentalEcoregionCaseReceipt, u32), String> {
    let plan = ContinentalEcoregionPlan::new(ContinentalEcoregionDescriptor::new(
        seed,
        topology.topology(),
    ))
    .map_err(|error| error.to_string())?;
    let request = LandscapeWindowRequest::new(
        CORPUS_MIN_X,
        CORPUS_MIN_Z,
        CORPUS_WIDTH,
        CORPUS_DEPTH,
        CORPUS_STEP_BLOCKS,
        LandscapePlanDetail::Mosaic,
    );
    let whole = plan
        .query_window(request)
        .map_err(|error| error.to_string())?;
    let mut exact_comparisons = 0_u32;

    let split_width = 14;
    let left = plan
        .query_window(LandscapeWindowRequest {
            width_samples: split_width,
            ..request
        })
        .map_err(|error| error.to_string())?;
    let right = plan
        .query_window(LandscapeWindowRequest {
            min_x: request.min_x + split_width as i32 * request.step_blocks as i32,
            width_samples: request.width_samples - split_width,
            ..request
        })
        .map_err(|error| error.to_string())?;
    for row in 0..request.depth_samples as usize {
        let whole_start = row * request.width_samples as usize;
        let left_start = row * left.request.width_samples as usize;
        let right_start = row * right.request.width_samples as usize;
        compare_slices(
            "left partition",
            &whole.samples[whole_start..whole_start + split_width as usize],
            &left.samples[left_start..left_start + split_width as usize],
        )?;
        compare_slices(
            "right partition",
            &whole.samples
                [whole_start + split_width as usize..whole_start + request.width_samples as usize],
            &right.samples[right_start..right_start + right.request.width_samples as usize],
        )?;
        exact_comparisons += request.width_samples;
    }

    let mut traversal = (0..whole.samples.len()).collect::<Vec<_>>();
    traversal.sort_by_key(|index| permutation_key(*index as u64, seed));
    for index in traversal {
        let expected = &whole.samples[index];
        let actual = plan
            .query_point(request.detail, expected.world_x, expected.world_z)
            .sample;
        if actual != *expected {
            return Err(format!(
                "{} seed {seed} randomized point mismatch at sample {index}",
                topology.label()
            ));
        }
        exact_comparisons += 1;
    }

    for index in (0..whole.samples.len()).step_by(17) {
        let detailed = &whole.samples[index];
        for detail in LandscapePlanDetail::ALL {
            let direct = plan.query_point(detail, detailed.world_x, detailed.world_z);
            let projected = project_sample(detailed.clone(), detail);
            if direct.sample != projected {
                return Err(format!(
                    "{} seed {seed} direct {} projection mismatch at sample {index}",
                    topology.label(),
                    detail.label()
                ));
            }
            validate_direct_work(detail, direct.work)?;
            exact_comparisons += 1;
        }
    }

    if topology == CorpusTopology::CylinderX196608 {
        for index in (0..whole.samples.len()).step_by(11) {
            let expected = &whole.samples[index];
            let lifted = plan.query_point(
                LandscapePlanDetail::Mosaic,
                expected.world_x + CONTINENTAL_ECOREGION_CORPUS_PERIOD_BLOCKS,
                expected.world_z,
            );
            let mut normalized = lifted.sample;
            normalized.world_x -= CONTINENTAL_ECOREGION_CORPUS_PERIOD_BLOCKS;
            if normalized != *expected {
                return Err(format!(
                    "{} seed {seed} periodic lift mismatch at sample {index}",
                    topology.label()
                ));
            }
            exact_comparisons += 1;
        }
    }

    validate_window_work(whole.work, whole.samples.len())?;
    let mut continents = BTreeSet::new();
    let mut provinces = BTreeSet::new();
    let mut province_kinds = BTreeSet::new();
    let mut ecoregions = BTreeSet::new();
    let mut ecoregion_kinds = BTreeSet::new();
    let mut clearings = BTreeSet::new();
    let mut clearing_causes = BTreeSet::new();
    let mut land_samples = 0_u32;
    for sample in &whole.samples {
        if sample.continent.is_some() {
            land_samples += 1;
        }
        if let Some(continent) = sample.continent {
            continents.insert(continent.id.hash);
        }
        if let Some(province) = sample.province {
            provinces.insert(province.id.hash);
            province_kinds.insert(province.kind as u8);
        }
        if let Some(ecoregion) = sample.ecoregion {
            ecoregions.insert(ecoregion.id.hash);
            ecoregion_kinds.insert(ecoregion.kind as u8);
        }
        if let Some(mosaic) = sample.mosaic {
            if let Some(clearing) = mosaic.clearing_id {
                clearings.insert(clearing.hash);
            }
            if let Some(cause) = mosaic.clearing_cause {
                clearing_causes.insert(cause as u8);
            }
        }
    }
    if land_samples == 0
        || land_samples == whole.samples.len() as u32
        || continents.is_empty()
        || provinces.is_empty()
        || ecoregions.is_empty()
    {
        return Err(format!(
            "{} seed {seed} corpus failed to exercise both ocean and land hierarchy",
            topology.label()
        ));
    }

    let receipt = ContinentalEcoregionCaseReceipt {
        seed,
        topology,
        semantic_sha256: whole.semantic_sha256,
        sample_count: whole.samples.len() as u32,
        land_samples,
        continent_count: continents.len() as u32,
        province_count: provinces.len() as u32,
        province_kind_count: province_kinds.len() as u32,
        ecoregion_count: ecoregions.len() as u32,
        ecoregion_kind_count: ecoregion_kinds.len() as u32,
        clearing_count: clearings.len() as u32,
        clearing_cause_count: clearing_causes.len() as u32,
        exact_comparisons,
        work: whole.work,
    };
    Ok((receipt, exact_comparisons))
}

fn project_sample(
    mut sample: LandscapePlanSample,
    detail: LandscapePlanDetail,
) -> LandscapePlanSample {
    sample.detail = detail;
    match detail {
        LandscapePlanDetail::Continental => {
            sample.province = None;
            sample.ecoregion = None;
            sample.mosaic = None;
        }
        LandscapePlanDetail::Province => {
            sample.ecoregion = None;
            sample.mosaic = None;
        }
        LandscapePlanDetail::Ecoregion => sample.mosaic = None,
        LandscapePlanDetail::Mosaic => {}
    }
    sample
}

fn validate_direct_work(
    detail: LandscapePlanDetail,
    work: PlanConstructionCounts,
) -> Result<(), String> {
    if work.requested_samples != 1 || work.exact_chunks != 0 {
        return Err("direct query reported invalid sample or exact-chunk work".to_owned());
    }
    if work.continental_owner_evaluations != 25 {
        return Err(format!(
            "direct query examined {} continental owners rather than 25",
            work.continental_owner_evaluations
        ));
    }
    if detail == LandscapePlanDetail::Continental
        && (work.province_owner_evaluations != 0
            || work.ecoregion_owner_evaluations != 0
            || work.mosaic_owner_evaluations != 0
            || work.local_field_evaluations != 0)
    {
        return Err("continental query constructed hidden children".to_owned());
    }
    if detail <= LandscapePlanDetail::Province
        && (work.ecoregion_owner_evaluations != 0 || work.mosaic_owner_evaluations != 0)
    {
        return Err("province-or-coarser query constructed hidden children".to_owned());
    }
    if detail <= LandscapePlanDetail::Ecoregion && work.mosaic_owner_evaluations != 0 {
        return Err("ecoregion-or-coarser query constructed mosaic children".to_owned());
    }
    if work.province_owner_evaluations > 9
        || work.ecoregion_owner_evaluations > 9
        || work.mosaic_owner_evaluations > 9
        || work.local_field_evaluations > 6
    {
        return Err("direct query exceeded its declared owner or field caps".to_owned());
    }
    Ok(())
}

fn validate_window_work(work: PlanConstructionCounts, sample_count: usize) -> Result<(), String> {
    if work.requested_samples != sample_count as u64 || work.exact_chunks != 0 {
        return Err("window work did not report exact requested-sample bounds".to_owned());
    }
    if work.continental_owner_evaluations != sample_count as u64 * 25
        || work.province_owner_evaluations > sample_count as u64 * 9
        || work.ecoregion_owner_evaluations > sample_count as u64 * 9
        || work.mosaic_owner_evaluations > sample_count as u64 * 9
        || work.local_field_evaluations > sample_count as u64 * 6
    {
        return Err("window work exceeded a declared per-sample cap".to_owned());
    }
    Ok(())
}

fn compare_slices(
    label: &str,
    expected: &[LandscapePlanSample],
    actual: &[LandscapePlanSample],
) -> Result<(), String> {
    if expected == actual {
        Ok(())
    } else {
        Err(format!("{label} sample mismatch"))
    }
}

fn permutation_key(index: u64, seed: i64) -> u64 {
    let mut value = index ^ seed as u64;
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn case_witness_sha256(cases: &[ContinentalEcoregionCaseReceipt]) -> Result<String, String> {
    let canonical = serde_json::to_vec(cases)
        .map_err(|error| format!("serialize continental/ecoregion cases: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(CONTINENTAL_ECOREGION_HARNESS_SCHEMA_REVISION.as_bytes());
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
    fn exact_suite_passes_and_exercises_authored_facts() {
        let receipt = run_continental_ecoregion_suite().unwrap();
        assert!(receipt.suite_passed);
        assert_eq!(receipt.witness_sha256, CONTINENTAL_ECOREGION_WITNESS_SHA256);
        assert_eq!(receipt.cases.len(), 6);
        assert!(receipt.exact_comparisons > 5_000);
        assert!(receipt.cases.iter().all(|case| case.land_samples > 0));
        assert!(receipt.cases.iter().all(|case| case.province_count > 1));
        assert!(receipt.cases.iter().all(|case| case.ecoregion_count > 2));
    }

    #[test]
    fn native_threads_publish_one_exact_suite() {
        let expected = run_continental_ecoregion_suite().unwrap().witness_sha256;
        let handles = (0..4)
            .map(|_| std::thread::spawn(run_continental_ecoregion_suite))
            .collect::<Vec<_>>();
        for handle in handles {
            let receipt = handle.join().expect("suite thread").expect("suite");
            assert_eq!(receipt.witness_sha256, expected);
        }
    }
}
