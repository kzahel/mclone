//! Deterministic native/Wasm witness for bounded continental hydrography.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    continental_ecoregion::{ContinentalEcoregionDescriptor, ContinentalEcoregionTopology},
    continental_hydrography::{
        CONTINENTAL_HYDROGRAPHY_SCHEMA_REVISION, CatchmentLocalPoint, ContinentalCatchment,
        ContinentalHydrographyPlan, ContinentalHydrographySample, ContinentalHydrographyWork,
    },
};

pub const CONTINENTAL_HYDROGRAPHY_HARNESS_REVISION: &str =
    "mclone-continental-hydrography-harness-v1";
pub const CONTINENTAL_HYDROGRAPHY_WITNESS_SHA256: &str =
    "8010fe5dbb1b6bb8ea344d1a003d4ff822ef7773c34121ca4f903c25ffce2ca6";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalHydrographyCaseReceipt {
    pub label: &'static str,
    pub topology: &'static str,
    pub owner_x: i32,
    pub owner_z: i32,
    pub catchment_hash: u64,
    pub center_x: i64,
    pub center_z: i64,
    pub downstream_axis_x: f64,
    pub downstream_axis_z: f64,
    pub reach_hashes: Vec<u64>,
    pub samples: Vec<ContinentalHydrographySample>,
    pub work: ContinentalHydrographyWork,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinentalHydrographySuiteReceipt {
    pub schema_revision: &'static str,
    pub harness_revision: &'static str,
    pub cases: Vec<ContinentalHydrographyCaseReceipt>,
    pub witness_sha256: String,
}

pub fn run_continental_hydrography_suite() -> Result<ContinentalHydrographySuiteReceipt, String> {
    let cases = vec![
        compile_case(
            "plane-origin",
            ContinentalEcoregionDescriptor::plane(12_345),
            0,
            0,
        )?,
        compile_case(
            "plane-negative",
            ContinentalEcoregionDescriptor::plane(-98_765),
            -3,
            2,
        )?,
        compile_case(
            "large-cylinder-seam-owner",
            ContinentalEcoregionDescriptor::new(
                12_345,
                ContinentalEcoregionTopology::cylinder_x(196_608),
            ),
            5,
            -1,
        )?,
    ];
    let bytes = serde_json::to_vec(&cases)
        .map_err(|error| format!("serialize continental hydrography cases: {error}"))?;
    let witness_sha256 = format!("{:x}", Sha256::digest(bytes));
    Ok(ContinentalHydrographySuiteReceipt {
        schema_revision: CONTINENTAL_HYDROGRAPHY_SCHEMA_REVISION,
        harness_revision: CONTINENTAL_HYDROGRAPHY_HARNESS_REVISION,
        cases,
        witness_sha256,
    })
}

fn compile_case(
    label: &'static str,
    descriptor: ContinentalEcoregionDescriptor,
    owner_x: i32,
    owner_z: i32,
) -> Result<ContinentalHydrographyCaseReceipt, String> {
    let plan = ContinentalHydrographyPlan::new(descriptor);
    let catchment = plan.catchment_for_owner(owner_x, owner_z);
    let mut samples = Vec::with_capacity(catchment.reaches.len() + 3);
    let mut work = ContinentalHydrographyWork::default();
    for reach in &catchment.reaches {
        let midpoint = quadratic_midpoint(
            catchment.nodes[usize::from(reach.start_node)],
            reach.control,
            catchment.nodes[usize::from(reach.end_node)],
        );
        sample_local(&plan, &catchment, midpoint, &mut samples, &mut work)?;
    }
    for point in [
        catchment.lake_center,
        catchment.nodes[usize::from(catchment.spill_node)],
        CatchmentLocalPoint {
            across: 0.0,
            downstream: -10_300.0,
            bed_y: 0.0,
        },
    ] {
        sample_local(&plan, &catchment, point, &mut samples, &mut work)?;
    }

    let topology = match descriptor.topology {
        ContinentalEcoregionTopology::Plane => "plane",
        ContinentalEcoregionTopology::CylinderX { .. } => "cylinder-x-196608",
    };
    Ok(ContinentalHydrographyCaseReceipt {
        label,
        topology,
        owner_x: catchment.owner_x,
        owner_z: catchment.owner_z,
        catchment_hash: catchment.id.hash,
        center_x: catchment.center_x,
        center_z: catchment.center_z,
        downstream_axis_x: catchment.downstream_axis_x,
        downstream_axis_z: catchment.downstream_axis_z,
        reach_hashes: catchment
            .reaches
            .iter()
            .map(|reach| reach.id.hash)
            .collect(),
        samples,
        work,
    })
}

fn sample_local(
    plan: &ContinentalHydrographyPlan,
    catchment: &ContinentalCatchment,
    point: CatchmentLocalPoint,
    samples: &mut Vec<ContinentalHydrographySample>,
    work: &mut ContinentalHydrographyWork,
) -> Result<(), String> {
    let (world_x, world_z) = catchment.local_to_world(point);
    let query = plan.query_point(world_x.round() as i32, world_z.round() as i32);
    let sample = query.sample.ok_or_else(|| {
        format!(
            "{},{}, owner {},{} did not resolve to a catchment",
            world_x, world_z, catchment.owner_x, catchment.owner_z
        )
    })?;
    samples.push(sample);
    add_work(work, query.work);
    Ok(())
}

fn quadratic_midpoint(
    start: CatchmentLocalPoint,
    control: CatchmentLocalPoint,
    end: CatchmentLocalPoint,
) -> CatchmentLocalPoint {
    CatchmentLocalPoint {
        across: start.across * 0.25 + control.across * 0.5 + end.across * 0.25,
        downstream: start.downstream * 0.25 + control.downstream * 0.5 + end.downstream * 0.25,
        bed_y: 0.0,
    }
}

fn add_work(total: &mut ContinentalHydrographyWork, work: ContinentalHydrographyWork) {
    total.owner_evaluations += work.owner_evaluations;
    total.graph_constructions += work.graph_constructions;
    total.reach_evaluations += work.reach_evaluations;
    total.exact_chunks += work.exact_chunks;
    total.raster_cells += work.raster_cells;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hydrography_suite_matches_committed_witness() {
        let receipt = run_continental_hydrography_suite().unwrap();
        assert_eq!(
            receipt.witness_sha256,
            CONTINENTAL_HYDROGRAPHY_WITNESS_SHA256
        );
        assert_eq!(receipt.cases.len(), 3);
        assert!(receipt.cases.iter().all(|case| case.work.exact_chunks == 0));
        assert!(receipt.cases.iter().all(|case| case.work.raster_cells == 0));
    }
}
