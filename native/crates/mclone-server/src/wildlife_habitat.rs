use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockPos, CHUNK_WIDTH, ChunkPos, ChunkSnapshot, HorizontalTopology,
    SECTION_HEIGHT, chunk_section_index, local_block_coord,
};
use mclone_worldgen::{
    block::{
        ACACIA_LOG, BIRCH_LOG, BIRCH_LOG_X, BIRCH_LOG_Z, COARSE_DIRT, DANDELION, DARK_OAK_LOG,
        DIRT, FERN, GRASS, GRASS_BLOCK, JUNGLE_LOG, OAK_LOG, OAK_LOG_X, OAK_LOG_Z, POPPY,
        RawBlockId, SAND, SNOW_BLOCK, SPRUCE_LOG, SPRUCE_LOG_X, SPRUCE_LOG_Z, TALL_GRASS_LOWER,
        TALL_GRASS_UPPER, is_leaves, is_water, material_blocks_motion,
    },
    levelgen::{
        OCEAN_BIOME_ID, WildlifeCellPlan, WildlifeEvidenceSet, WildlifeHabitatSample,
        WildlifeHabitatSource, WildlifePlanError, WildlifePopulationCell,
        WildlifePopulationPlanner,
    },
};

const EVIDENCE_RADIUS_BLOCKS: i32 = 4;

pub(crate) fn required_snapshot_chunks(
    planner: &WildlifePopulationPlanner,
    cell: WildlifePopulationCell,
    topology: HorizontalTopology,
) -> Result<BTreeSet<ChunkPos>, WildlifePlanError> {
    let mut required = BTreeSet::new();
    for site in planner.candidate_sites(cell)? {
        if !site.available {
            continue;
        }
        let min_chunk_x = (site.world_x - EVIDENCE_RADIUS_BLOCKS).div_euclid(CHUNK_WIDTH);
        let max_chunk_x = (site.world_x + EVIDENCE_RADIUS_BLOCKS).div_euclid(CHUNK_WIDTH);
        let min_chunk_z = (site.world_z - EVIDENCE_RADIUS_BLOCKS).div_euclid(CHUNK_WIDTH);
        let max_chunk_z = (site.world_z + EVIDENCE_RADIUS_BLOCKS).div_euclid(CHUNK_WIDTH);
        for chunk_x in min_chunk_x..=max_chunk_x {
            for chunk_z in min_chunk_z..=max_chunk_z {
                if let Some(pos) = topology.canonicalize_chunk(ChunkPos::new(chunk_x, chunk_z)) {
                    required.insert(pos);
                }
            }
        }
    }
    Ok(required)
}

pub(crate) fn plan_from_published_snapshots(
    planner: &WildlifePopulationPlanner,
    cell: WildlifePopulationCell,
    topology: HorizontalTopology,
    snapshots: &BTreeMap<ChunkPos, ChunkSnapshot>,
) -> Result<Option<WildlifeCellPlan>, WildlifePlanError> {
    if required_snapshot_chunks(planner, cell, topology)?
        .iter()
        .any(|pos| !snapshots.contains_key(pos))
    {
        return Ok(None);
    }
    let samples = planner
        .candidate_sites(cell)?
        .into_iter()
        .map(|site| {
            if site.available {
                sample_site(site.world_x, site.world_z, topology, snapshots)
            } else {
                WildlifeHabitatSample::unavailable(
                    site.world_x,
                    site.world_z,
                    WildlifeHabitatSource::PublishedBlocks,
                )
            }
        })
        .collect::<Vec<_>>();
    planner.plan_cell_with_samples(cell, &samples).map(Some)
}

fn sample_site(
    world_x: i32,
    world_z: i32,
    topology: HorizontalTopology,
    snapshots: &BTreeMap<ChunkPos, ChunkSnapshot>,
) -> WildlifeHabitatSample {
    let mut columns = Vec::with_capacity(((EVIDENCE_RADIUS_BLOCKS * 2 + 1).pow(2)) as usize);
    for dz in -EVIDENCE_RADIUS_BLOCKS..=EVIDENCE_RADIUS_BLOCKS {
        for dx in -EVIDENCE_RADIUS_BLOCKS..=EVIDENCE_RADIUS_BLOCKS {
            columns.push(observe_column(
                world_x + dx,
                world_z + dz,
                topology,
                snapshots,
            ));
        }
    }
    let center_index = columns.len() / 2;
    let center = columns[center_index];
    let count = columns.len() as f64;
    let land_columns = columns.iter().filter(|column| column.land).count() as f64;
    let water_columns = columns.iter().filter(|column| column.water).count() as f64;
    let inland_water_columns = columns
        .iter()
        .filter(|column| column.water && column.biome != Some(OCEAN_BIOME_ID))
        .count() as f64;
    let grass_soil_columns = columns
        .iter()
        .filter(|column| column.productive_support)
        .count() as f64;
    let low_cover_columns = columns.iter().filter(|column| column.low_cover).count() as f64;
    let flower_columns = columns.iter().filter(|column| column.flower).count() as f64;
    let canopy_columns = columns.iter().filter(|column| column.canopy).count() as f64;
    let mature_tree_columns = columns.iter().filter(|column| column.mature_tree).count() as f64;
    let shallow_water_columns = columns
        .iter()
        .filter(|column| column.water && column.water_depth <= 2)
        .count() as f64;
    let bank_columns = (0..columns.len())
        .filter(|index| columns[*index].land && touches_water(*index, &columns))
        .count() as f64;
    let minimum_surface = columns
        .iter()
        .map(|column| column.surface_y)
        .min()
        .unwrap_or(center.surface_y);
    let maximum_surface = columns
        .iter()
        .map(|column| column.surface_y)
        .max()
        .unwrap_or(center.surface_y);
    let slope_comfort = (1.0 - f64::from(maximum_surface - minimum_surface) / 12.0).clamp(0.0, 1.0);
    let land = land_columns / count;
    let productive_support = grass_soil_columns / count;
    let low_cover = low_cover_columns / count;
    let flowers = flower_columns / count;
    let forest_cover = canopy_columns / count;
    let mature_trees = mature_tree_columns / count;
    let openness = (1.0 - forest_cover * 0.86).clamp(0.0, 1.0) * slope_comfort;
    let forest_edge = (forest_cover * (1.0 - forest_cover) * 4.0).clamp(0.0, 1.0);
    let productivity = (productive_support * 0.68 + low_cover * 0.20 + flowers * 0.12)
        * (0.72 + slope_comfort * 0.28);
    let bank = bank_columns / count;
    let wetland = (bank * 0.62 + shallow_water_columns / count * 0.38).clamp(0.0, 1.0);
    WildlifeHabitatSample {
        world_x,
        world_z,
        surface_y: center.surface_y,
        source: WildlifeHabitatSource::PublishedBlocks,
        supported: true,
        available_evidence: WildlifeEvidenceSet::ALL,
        land: to_permille(land),
        productivity: to_permille(productivity),
        openness: to_permille(openness),
        low_cover: to_permille(low_cover),
        forest_cover: to_permille(forest_cover),
        forest_edge: to_permille(forest_edge),
        wetland: to_permille(wetland),
        water: to_permille(water_columns / count),
        inland_water: to_permille(inland_water_columns / count),
        shore: to_permille(bank),
        bank: to_permille(bank),
        flowering: to_permille(flowers),
        seeds_and_soft_mast: to_permille(
            (low_cover * 0.56 + flowers * 0.22 + mature_trees * 0.22).clamp(0.0, 1.0),
        ),
        mature_trees: to_permille(mature_trees.max(forest_cover * 0.72)),
    }
}

#[derive(Clone, Copy, Debug)]
struct ColumnObservation {
    surface_y: i32,
    biome: Option<i32>,
    land: bool,
    productive_support: bool,
    water: bool,
    water_depth: i32,
    low_cover: bool,
    flower: bool,
    canopy: bool,
    mature_tree: bool,
}

fn observe_column(
    world_x: i32,
    world_z: i32,
    topology: HorizontalTopology,
    snapshots: &BTreeMap<ChunkPos, ChunkSnapshot>,
) -> ColumnObservation {
    let Some(canonical) = topology.canonicalize_block(BlockPos::new(world_x, 0, world_z)) else {
        return ColumnObservation::empty();
    };
    let Some(snapshot) = snapshots.get(&canonical.chunk_pos()) else {
        return ColumnObservation::empty();
    };
    let local_x = local_block_coord(canonical.x);
    let local_z = local_block_coord(canonical.z);
    let mut surface_y = snapshot.min_y;
    let mut ground = None;
    let mut water_depth = 0;
    let mut low_cover = false;
    let mut flower = false;
    let mut canopy = false;
    let mut log_count = 0;
    for y in (snapshot.min_y..snapshot.min_y + snapshot.height).rev() {
        let block = snapshot_raw_block(snapshot, local_x, y, local_z);
        canopy |= is_leaves(block);
        log_count += usize::from(is_log(block));
        low_cover |= is_low_cover(block);
        flower |= matches!(block, DANDELION | POPPY);
        if ground.is_none() && material_blocks_motion(block) && !is_leaves(block) && !is_log(block)
        {
            surface_y = y;
            ground = Some(block);
        }
    }
    for y in surface_y + 1..=(surface_y + 8).min(snapshot.min_y + snapshot.height - 1) {
        if is_water(snapshot_raw_block(snapshot, local_x, y, local_z)) {
            water_depth += 1;
        }
    }
    let biome = snapshot.biome_id_at_local_block(local_x, surface_y, local_z);
    let ground = ground.unwrap_or(AIR_BLOCK_STATE_ID.0 as RawBlockId);
    ColumnObservation {
        surface_y,
        biome,
        land: water_depth == 0 && is_supported_ground(ground),
        productive_support: water_depth == 0 && is_productive_ground(ground),
        water: water_depth > 0,
        water_depth,
        low_cover,
        flower,
        canopy,
        mature_tree: canopy && log_count > 0,
    }
}

impl ColumnObservation {
    const fn empty() -> Self {
        Self {
            surface_y: 0,
            biome: None,
            land: false,
            productive_support: false,
            water: false,
            water_depth: 0,
            low_cover: false,
            flower: false,
            canopy: false,
            mature_tree: false,
        }
    }
}

fn touches_water(index: usize, columns: &[ColumnObservation]) -> bool {
    let width = usize::try_from(EVIDENCE_RADIUS_BLOCKS * 2 + 1).expect("positive width");
    let x = index % width;
    let z = index / width;
    [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .any(|(dx, dz)| {
            let x = i32::try_from(x).expect("small evidence X") + dx;
            let z = i32::try_from(z).expect("small evidence Z") + dz;
            x >= 0
                && z >= 0
                && x < width as i32
                && z < width as i32
                && columns[z as usize * width + x as usize].water
        })
}

fn snapshot_raw_block(
    snapshot: &ChunkSnapshot,
    local_x: i32,
    world_y: i32,
    local_z: i32,
) -> RawBlockId {
    if world_y < snapshot.min_y || world_y >= snapshot.min_y + snapshot.height {
        return AIR_BLOCK_STATE_ID.0 as RawBlockId;
    }
    let section_y = world_y.div_euclid(SECTION_HEIGHT);
    let local_y = world_y.rem_euclid(SECTION_HEIGHT);
    snapshot
        .sections
        .iter()
        .find(|section| section.section_y == section_y)
        .and_then(|section| {
            RawBlockId::try_from(
                section
                    .block_state_id_at(chunk_section_index(local_x, local_y, local_z))
                    .0,
            )
            .ok()
        })
        .unwrap_or(AIR_BLOCK_STATE_ID.0 as RawBlockId)
}

const fn is_supported_ground(block: RawBlockId) -> bool {
    matches!(block, GRASS_BLOCK | DIRT | COARSE_DIRT | SAND | SNOW_BLOCK)
}

const fn is_productive_ground(block: RawBlockId) -> bool {
    matches!(block, GRASS_BLOCK | DIRT | COARSE_DIRT)
}

const fn is_low_cover(block: RawBlockId) -> bool {
    matches!(block, GRASS | FERN | TALL_GRASS_LOWER | TALL_GRASS_UPPER)
}

const fn is_log(block: RawBlockId) -> bool {
    matches!(
        block,
        OAK_LOG
            | OAK_LOG_X
            | OAK_LOG_Z
            | BIRCH_LOG
            | BIRCH_LOG_X
            | BIRCH_LOG_Z
            | SPRUCE_LOG
            | SPRUCE_LOG_X
            | SPRUCE_LOG_Z
            | DARK_OAK_LOG
            | ACACIA_LOG
            | JUNGLE_LOG
    )
}

fn to_permille(value: f64) -> u16 {
    (value.clamp(0.0, 1.0) * 1_000.0).round() as u16
}

#[cfg(test)]
mod tests {
    use mclone_core::{ChunkRevision, ChunkStatus};
    use mclone_worldgen::block::{OAK_LEAVES, WATER, generated_block_state_id};

    use super::*;

    fn snapshot_with_column(blocks: &[(i32, RawBlockId)]) -> ChunkSnapshot {
        let mut states = vec![AIR_BLOCK_STATE_ID; (CHUNK_WIDTH * CHUNK_WIDTH * 16) as usize];
        for (y, block) in blocks {
            states[chunk_section_index(8, *y, 8)] = generated_block_state_id(*block);
        }
        ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Features,
            ChunkRevision(1),
            0,
            16,
            &states,
        )
    }

    #[test]
    fn immutable_snapshot_observation_distinguishes_water_and_tree_evidence() {
        let water = snapshot_with_column(&[(3, DIRT), (4, WATER), (5, WATER)]);
        let observed = observe_column(
            8,
            8,
            HorizontalTopology::UNBOUNDED,
            &BTreeMap::from([(ChunkPos::new(0, 0), water)]),
        );
        assert!(observed.water);
        assert_eq!(observed.water_depth, 2);

        let tree = snapshot_with_column(&[(3, DIRT), (4, OAK_LOG), (5, OAK_LEAVES)]);
        let observed = observe_column(
            8,
            8,
            HorizontalTopology::UNBOUNDED,
            &BTreeMap::from([(ChunkPos::new(0, 0), tree)]),
        );
        assert!(observed.canopy);
        assert!(observed.mature_tree);
        assert_eq!(
            snapshot_raw_block(&snapshot_with_column(&[(3, DIRT)]), 8, 3, 8,),
            DIRT
        );
    }

    #[test]
    fn incomplete_snapshot_set_defers_instead_of_observing_empty_habitat() {
        let planner = WildlifePopulationPlanner::for_habitat(
            12_345,
            HorizontalTopology::UNBOUNDED,
            WildlifeHabitatSource::PublishedBlocks,
        )
        .unwrap();
        let plan = plan_from_published_snapshots(
            &planner,
            WildlifePopulationCell { x: 0, z: 0 },
            HorizontalTopology::UNBOUNDED,
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(plan, None);
    }
}
