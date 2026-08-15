use std::collections::BTreeSet;
use std::env;
use std::error::Error;

use mclone_worldgen::levelgen::{
    MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS, McloneOverworldSamplingTopology,
    McloneOverworldWildlifePlanner, McloneWildlifeCellPlan, McloneWildlifePopulationCell,
    McloneWildlifeSpecies,
};
use serde::Serialize;

type AnyResult<T> = Result<T, Box<dyn Error>>;

#[derive(Clone, Copy, Debug)]
struct Args {
    seed: i64,
    center_cell_x: i32,
    center_cell_z: i32,
    search_radius_cells: i32,
    window_radius_chunks: u32,
    limit: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteCandidate {
    seed: i64,
    center_chunk_x: i32,
    center_chunk_z: i32,
    window_radius_chunks: u32,
    rabbit_groups: u32,
    rabbits: u32,
    deer_groups: u32,
    deer: u32,
    mallard_groups: u32,
    bees: u32,
    habitat_labels: Vec<String>,
    mean_desired_density: u32,
    encounters: Vec<SiteEncounter>,
    simulation_command: String,
    terrain_lab_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteEncounter {
    cell_x: i32,
    cell_z: i32,
    species: &'static str,
    group_size: u8,
    owner_chunk_x: i32,
    owner_chunk_z: i32,
    habitat: String,
    landform: String,
    productivity: u16,
    openness: u16,
    forest_cover: u16,
    wetland: u16,
    water: u16,
    desired_density: u16,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("wildlife site selection failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> AnyResult<()> {
    let args = parse_args()?;
    let planner =
        McloneOverworldWildlifePlanner::new(args.seed, McloneOverworldSamplingTopology::Unbounded);
    let margin = i32::try_from(args.window_radius_chunks)?
        .div_euclid(MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS)
        + 2;
    let mut plans = Vec::new();
    for cell_x in args.center_cell_x - args.search_radius_cells - margin
        ..=args.center_cell_x + args.search_radius_cells + margin
    {
        for cell_z in args.center_cell_z - args.search_radius_cells - margin
            ..=args.center_cell_z + args.search_radius_cells + margin
        {
            plans.push(planner.plan_cell(McloneWildlifePopulationCell {
                x: cell_x,
                z: cell_z,
            })?);
        }
    }

    let mut candidates = Vec::new();
    for cell_x in args.center_cell_x - args.search_radius_cells
        ..=args.center_cell_x + args.search_radius_cells
    {
        for cell_z in args.center_cell_z - args.search_radius_cells
            ..=args.center_cell_z + args.search_radius_cells
        {
            let center_chunk_x = cell_x * MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS
                + MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS / 2;
            let center_chunk_z = cell_z * MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS
                + MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS / 2;
            let candidate = candidate_for_window(
                args.seed,
                center_chunk_x,
                center_chunk_z,
                args.window_radius_chunks,
                &plans,
            );
            if candidate.rabbit_groups > 0 && candidate.deer_groups > 0 {
                candidates.push(candidate);
            }
        }
    }
    candidates.sort_unstable_by_key(|candidate| {
        (
            std::cmp::Reverse(candidate.rabbit_groups + candidate.deer_groups),
            std::cmp::Reverse(candidate.rabbits + candidate.deer),
            std::cmp::Reverse(candidate.habitat_labels.len()),
            candidate.center_chunk_x,
            candidate.center_chunk_z,
        )
    });
    candidates.truncate(args.limit);
    println!("{}", serde_json::to_string_pretty(&candidates)?);
    Ok(())
}

fn candidate_for_window(
    seed: i64,
    center_chunk_x: i32,
    center_chunk_z: i32,
    radius: u32,
    plans: &[McloneWildlifeCellPlan],
) -> SiteCandidate {
    let radius_i32 = radius as i32;
    let min_x = center_chunk_x - radius_i32;
    let max_x = center_chunk_x + radius_i32;
    let min_z = center_chunk_z - radius_i32;
    let max_z = center_chunk_z + radius_i32;
    let mut encounters = Vec::new();
    let mut rabbit_groups = 0;
    let mut rabbits = 0;
    let mut deer_groups = 0;
    let mut deer = 0;
    let mut mallard_groups = 0;
    let mut bees = 0;
    let mut density_sum = 0_u32;
    let mut habitat_labels = BTreeSet::new();
    for plan in plans {
        let Some(encounter) = plan.encounter.filter(|encounter| {
            (min_x..=max_x).contains(&encounter.owner_chunk.x)
                && (min_z..=max_z).contains(&encounter.owner_chunk.z)
        }) else {
            continue;
        };
        match encounter.species {
            McloneWildlifeSpecies::Rabbit => {
                rabbit_groups += 1;
                rabbits += u32::from(encounter.group_size);
            }
            McloneWildlifeSpecies::Deer => {
                deer_groups += 1;
                deer += u32::from(encounter.group_size);
            }
            McloneWildlifeSpecies::Mallard => mallard_groups += 1,
            McloneWildlifeSpecies::Bee => bees += u32::from(encounter.group_size),
        }
        let habitat = format!("{:?}", plan.selected_habitat.biome);
        habitat_labels.insert(habitat.clone());
        density_sum += u32::from(plan.desired_density);
        encounters.push(SiteEncounter {
            cell_x: plan.cell.x,
            cell_z: plan.cell.z,
            species: encounter.species.label(),
            group_size: encounter.group_size,
            owner_chunk_x: encounter.owner_chunk.x,
            owner_chunk_z: encounter.owner_chunk.z,
            habitat,
            landform: format!("{:?}", plan.selected_habitat.landform),
            productivity: plan.selected_habitat.productivity,
            openness: plan.selected_habitat.openness,
            forest_cover: plan.selected_habitat.forest_cover,
            wetland: plan.selected_habitat.wetland,
            water: plan.selected_habitat.water,
            desired_density: plan.desired_density,
        });
    }
    encounters.sort_unstable_by_key(|entry| (entry.cell_x, entry.cell_z));
    let mean_desired_density = if encounters.is_empty() {
        0
    } else {
        density_sum / encounters.len() as u32
    };
    let blocks = ((radius * 2 + 1) * 16).max(128);
    SiteCandidate {
        seed,
        center_chunk_x,
        center_chunk_z,
        window_radius_chunks: radius,
        rabbit_groups,
        rabbits,
        deer_groups,
        deer,
        mallard_groups,
        bees,
        habitat_labels: habitat_labels.into_iter().collect(),
        mean_desired_density,
        encounters,
        simulation_command: format!(
            "pnpm native:wildlife:simulate --seed {seed} --center {center_chunk_x},{center_chunk_z} --radius {radius} --days 120 --output /tmp/mclone-wildlife-{seed}-{center_chunk_x}-{center_chunk_z}"
        ),
        terrain_lab_url: format!(
            "https://mclone.kzahel.com/terrain/?profile=mclone-overworld-v1&seed={seed}&x={}&z={}&blocks={blocks}&panes=wildlife&view=map",
            center_chunk_x * 16,
            center_chunk_z * 16,
        ),
    }
}

fn parse_args() -> AnyResult<Args> {
    let command = env::args().collect::<Vec<_>>();
    if command.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "wildlife_population_sites --seed <i64> [--center-cell <x,z>] \
             [--search-radius-cells <n>] [--window-radius-chunks <n>] [--limit <n>]"
        );
        std::process::exit(0);
    }
    let mut args = Args {
        seed: 0,
        center_cell_x: 0,
        center_cell_z: 0,
        search_radius_cells: 12,
        window_radius_chunks: 4,
        limit: 12,
    };
    let mut saw_seed = false;
    let mut index = 1;
    while index < command.len() {
        let flag = &command[index];
        let value = command
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--seed" => {
                args.seed = value.parse()?;
                saw_seed = true;
            }
            "--center-cell" => {
                let (x, z) = value.split_once(',').ok_or("center cell must be x,z")?;
                args.center_cell_x = x.parse()?;
                args.center_cell_z = z.parse()?;
            }
            "--search-radius-cells" => args.search_radius_cells = value.parse()?,
            "--window-radius-chunks" => args.window_radius_chunks = value.parse()?,
            "--limit" => args.limit = value.parse()?,
            _ => return Err(format!("unknown argument {flag}").into()),
        }
        index += 2;
    }
    if !saw_seed
        || !(0..=128).contains(&args.search_radius_cells)
        || args.window_radius_chunks > 16
        || args.limit > 1_000
    {
        return Err("invalid or unbounded site selection arguments".into());
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_real_seed_search_finds_rabbit_and_deer_window() {
        let planner = McloneOverworldWildlifePlanner::new(
            -98_765,
            McloneOverworldSamplingTopology::Unbounded,
        );
        let plans = (-34..=-30)
            .flat_map(|z| {
                let planner = &planner;
                (-2..=2).map(move |x| {
                    planner
                        .plan_cell(McloneWildlifePopulationCell { x, z })
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let site = candidate_for_window(-98_765, 0, -128, 8, &plans);
        assert!(site.rabbits > 0);
        assert!(site.deer > 0);
    }
}
