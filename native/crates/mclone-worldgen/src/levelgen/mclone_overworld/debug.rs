use super::biomes::{McloneOverworldBiomeDecision, mclone_overworld_biome_decision};
use super::fields::{
    McloneOverworldLandformFamily, McloneOverworldLandformIntent, McloneOverworldLandformSample,
    McloneOverworldSampler, McloneOverworldSamplingTopology,
};
use super::surface::{McloneOverworldSurfaceRecipe, mclone_overworld_surface_recipe};
use super::terrain::sample_mclone_overworld_landform_with_streams;
use mclone_core::ChunkPos;

pub const MCLONE_OVERWORLD_DEBUG_CELL_SIZE: i32 = 4;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum McloneOverworldLandformKind {
    Ocean,
    Coast,
    River,
    Wetland,
    QuietPlain,
    RollingUpland,
    RidgeValley,
    BroadBasin,
    MountainRange,
}

impl McloneOverworldLandformKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ocean => "ocean",
            Self::Coast => "coast",
            Self::River => "river channel",
            Self::Wetland => "wetland",
            Self::QuietPlain => "quiet plain",
            Self::RollingUpland => "rolling upland",
            Self::RidgeValley => "ridge-and-valley",
            Self::BroadBasin => "broad basin",
            Self::MountainRange => "mountain range",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum McloneOverworldHydrologyKind {
    Dry,
    RiverBank,
    Wetland,
    WetlandPool,
    MajorRiver,
    SubmergedOutlet,
    PlannedStream,
}

impl McloneOverworldHydrologyKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dry => "dry",
            Self::RiverBank => "river bank",
            Self::Wetland => "wetland influence",
            Self::WetlandPool => "wetland pool",
            Self::MajorRiver => "major river",
            Self::SubmergedOutlet => "submerged outlet",
            Self::PlannedStream => "planned stream",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct McloneOverworldDebugSample {
    pub world_x: i32,
    pub world_z: i32,
    pub quart_x: i32,
    pub quart_z: i32,
    pub landform_sample: McloneOverworldLandformSample,
    pub landform_intent: McloneOverworldLandformIntent,
    pub biome: McloneOverworldBiomeDecision,
    pub landform: McloneOverworldLandformKind,
    pub surface: McloneOverworldSurfaceRecipe,
    pub hydrology: McloneOverworldHydrologyKind,
    pub planned_stream_start: Option<ChunkPos>,
}

pub fn mclone_overworld_debug_sample(
    sampler: McloneOverworldSampler,
    world_x: i32,
    world_z: i32,
) -> McloneOverworldDebugSample {
    let landform_sample = sampler.sample_landform(world_x, world_z);
    debug_sample_from_landform(world_x, world_z, landform_sample, None)
}

pub fn mclone_overworld_debug_sample_with_streams(
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    world_x: i32,
    world_z: i32,
) -> Result<McloneOverworldDebugSample, String> {
    let (landform_sample, planned_stream_start) =
        sample_mclone_overworld_landform_with_streams(seed, topology, world_x, world_z)?;
    Ok(debug_sample_from_landform(
        world_x,
        world_z,
        landform_sample,
        planned_stream_start,
    ))
}

fn debug_sample_from_landform(
    world_x: i32,
    world_z: i32,
    landform_sample: McloneOverworldLandformSample,
    planned_stream_start: Option<ChunkPos>,
) -> McloneOverworldDebugSample {
    let landform_intent = landform_sample.terrain.landform_intent();
    McloneOverworldDebugSample {
        world_x,
        world_z,
        quart_x: world_x.div_euclid(MCLONE_OVERWORLD_DEBUG_CELL_SIZE),
        quart_z: world_z.div_euclid(MCLONE_OVERWORLD_DEBUG_CELL_SIZE),
        biome: mclone_overworld_biome_decision(landform_sample),
        landform: mclone_overworld_landform_kind(landform_sample),
        surface: mclone_overworld_surface_recipe(landform_sample),
        hydrology: debug_hydrology_kind(landform_sample),
        planned_stream_start,
        landform_sample,
        landform_intent,
    }
}

pub fn mclone_overworld_landform_kind(
    sample: McloneOverworldLandformSample,
) -> McloneOverworldLandformKind {
    let terrain = sample.terrain;
    if terrain.watercourse.is_channel() {
        McloneOverworldLandformKind::River
    } else if terrain.watercourse.is_wetland_pool() || terrain.watercourse.wetland_influence > 0.25
    {
        McloneOverworldLandformKind::Wetland
    } else if terrain.continentalness <= 0.0 {
        McloneOverworldLandformKind::Ocean
    } else if terrain.coast.family.is_coast() && terrain.coast.proximity >= 0.12 {
        McloneOverworldLandformKind::Coast
    } else {
        match terrain.landform_intent().family {
            McloneOverworldLandformFamily::QuietPlain => McloneOverworldLandformKind::QuietPlain,
            McloneOverworldLandformFamily::RollingUpland => {
                McloneOverworldLandformKind::RollingUpland
            }
            McloneOverworldLandformFamily::RidgeValley => McloneOverworldLandformKind::RidgeValley,
            McloneOverworldLandformFamily::BroadBasin => McloneOverworldLandformKind::BroadBasin,
            McloneOverworldLandformFamily::MountainRange => {
                McloneOverworldLandformKind::MountainRange
            }
        }
    }
}

fn debug_hydrology_kind(sample: McloneOverworldLandformSample) -> McloneOverworldHydrologyKind {
    let watercourse = sample.terrain.watercourse;
    if watercourse.is_planned_stream() {
        McloneOverworldHydrologyKind::PlannedStream
    } else if watercourse.submerged_outlet_influence > 0.0 {
        McloneOverworldHydrologyKind::SubmergedOutlet
    } else if watercourse.is_major_channel() {
        McloneOverworldHydrologyKind::MajorRiver
    } else if watercourse.is_wetland_pool() {
        McloneOverworldHydrologyKind::WetlandPool
    } else if watercourse.wetland_influence > 0.25 {
        McloneOverworldHydrologyKind::Wetland
    } else if watercourse.is_bank() {
        McloneOverworldHydrologyKind::RiverBank
    } else {
        McloneOverworldHydrologyKind::Dry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levelgen::{
        McloneOverworldBiomeRecipe, McloneOverworldStreamPlanner, mclone_overworld_biome_recipe,
        mclone_overworld_surface_recipe,
    };
    use mclone_core::ChunkPos;

    #[test]
    fn diagnostic_sample_uses_production_classifiers() {
        let sampler = McloneOverworldSampler::new(8_675_309);
        for (x, z) in [(-129, -33), (-1, -1), (0, 0), (127, 255), (512, -384)] {
            let debug = mclone_overworld_debug_sample(sampler, x, z);
            assert_eq!(
                debug.biome.recipe,
                mclone_overworld_biome_recipe(debug.landform_sample)
            );
            assert_eq!(
                debug.surface,
                mclone_overworld_surface_recipe(debug.landform_sample)
            );
        }
    }

    #[test]
    fn diagnostic_quart_cells_use_euclidean_negative_alignment() {
        let sampler = McloneOverworldSampler::new(1);
        let cases = [
            ((-5, -4), (-2, -1)),
            ((-1, -1), (-1, -1)),
            ((0, 3), (0, 0)),
            ((4, 7), (1, 1)),
        ];
        for ((x, z), expected) in cases {
            let sample = mclone_overworld_debug_sample(sampler, x, z);
            assert_eq!((sample.quart_x, sample.quart_z), expected);
        }
    }

    #[test]
    fn diagnostic_scan_exposes_multiple_biome_reasons() {
        let sampler = McloneOverworldSampler::new(8_675_309);
        let mut recipes = std::collections::BTreeSet::new();
        let mut reasons = std::collections::BTreeSet::new();
        for z in (-2_048..=2_048).step_by(64) {
            for x in (-2_048..=2_048).step_by(64) {
                let sample = mclone_overworld_debug_sample(sampler, x, z);
                recipes.insert(sample.biome.recipe);
                reasons.insert(sample.biome.reason);
            }
        }
        assert!(recipes.contains(&McloneOverworldBiomeRecipe::Ocean));
        assert!(recipes.contains(&McloneOverworldBiomeRecipe::TemperateMeadow));
        assert!(recipes.len() >= 5, "recipes={recipes:?}");
        assert!(reasons.len() >= recipes.len(), "reasons={reasons:?}");
    }

    #[test]
    fn diagnostic_stream_sample_names_the_owning_start() {
        let seed = -98_765;
        let topology = McloneOverworldSamplingTopology::Unbounded;
        let plan = McloneOverworldStreamPlanner::new(seed, topology)
            .plans_intersecting_chunks(ChunkPos::new(86, 54), ChunkPos::new(91, 62))
            .unwrap()
            .into_iter()
            .next()
            .expect("review region has a planned stream");
        let node = plan.nodes[plan.nodes.len() / 2];
        let debug =
            mclone_overworld_debug_sample_with_streams(seed, topology, node.x, node.z).unwrap();
        assert_eq!(debug.hydrology, McloneOverworldHydrologyKind::PlannedStream);
        assert_eq!(
            debug.planned_stream_start,
            Some(plan.structure.key.canonical_start)
        );
    }
}
