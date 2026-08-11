use std::collections::BTreeSet;

use mclone_core::{BlockPos, ChunkPos, Vec3d};
use mclone_protocol::EntityKind;
use mclone_worldgen::biome::BiomeDefinition;
use mclone_worldgen::block::{RawBlockId, generated_block_state_id};
use mclone_worldgen::prng::SimpleRandomSource;

use super::biome_tables::{MobSpawnEntry, farm_animal_spawns_for_biome};
use super::dry_run::{SurfaceProbeFailure, top_motion_blocking_no_leaves_feet_y};
use super::habitat::{WetlandHabitatFailure, sample_wetland_habitat};
use super::placements::{check_farm_animal_natural_spawn, check_land_creature_natural_spawn};

pub(crate) const CREATURE_SPAWN_MAX_CHUNKS_PER_TICK: usize = 8;
pub(crate) const CREATURE_SPAWN_MAX_ATTEMPTS_PER_TICK: usize = 16;
pub(crate) const CREATURE_SPAWN_MAX_SPAWNS_PER_TICK: usize = 4;

const CREATURE_SPAWN_ATTEMPTS_PER_CHUNK: usize = 2;
const MIN_SPAWN_DISTANCE_BLOCKS: f64 = 24.0;
const MAX_SPAWN_DISTANCE_BLOCKS: f64 = 128.0;
const MALLARD_FLOCK_MIN_SIZE: usize = 2;
const MALLARD_FLOCK_MAX_SIZE: usize = 4;
const MALLARD_FLOCK_MEMBER_RADIUS: i32 = 5;
const MALLARD_FLOCK_MEMBER_ATTEMPTS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CreatureSpawnProfile {
    ReferenceFarmAnimals,
    McloneOverworld,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CreatureSpawnDiagnostics {
    pub(crate) chunks_checked: usize,
    pub(crate) chunk_budget_exhausted: bool,
    pub(crate) attempts: usize,
    pub(crate) spawned: usize,
    pub(crate) spawn_budget_exhausted: bool,
    pub(crate) blocked_by_biome: usize,
    pub(crate) blocked_missing_biome_data: usize,
    pub(crate) blocked_missing_block_data: usize,
    pub(crate) blocked_player_distance: usize,
    pub(crate) blocked_world_predicate: usize,
    pub(crate) blocked_unsupported: usize,
    pub(crate) wetland_habitats_detected: usize,
    pub(crate) blocked_missing_wetland_data: usize,
    pub(crate) mallard_flocks_spawned: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CreatureSpawnRequest {
    pub(crate) kind: EntityKind,
    pub(crate) position: Vec3d,
    pub(crate) y_rot_degrees: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CreatureSpawnResult {
    pub(crate) diagnostics: CreatureSpawnDiagnostics,
    pub(crate) requests: Vec<CreatureSpawnRequest>,
}

pub(crate) fn plan_creature_spawns<F, B, L>(
    eligible_chunks: &BTreeSet<ChunkPos>,
    player_positions: &[Vec3d],
    max_spawns: usize,
    profile: CreatureSpawnProfile,
    random: &mut SimpleRandomSource,
    mut block_at: F,
    mut biome_at: B,
    mut raw_brightness_at: L,
) -> CreatureSpawnResult
where
    F: FnMut(BlockPos) -> Option<RawBlockId>,
    B: FnMut(BlockPos) -> Option<BiomeDefinition>,
    L: FnMut(BlockPos) -> Option<u8>,
{
    let effective_max_spawns = max_spawns.min(CREATURE_SPAWN_MAX_SPAWNS_PER_TICK);
    let mut result = CreatureSpawnResult {
        diagnostics: CreatureSpawnDiagnostics {
            chunk_budget_exhausted: eligible_chunks.len() > CREATURE_SPAWN_MAX_CHUNKS_PER_TICK,
            ..CreatureSpawnDiagnostics::default()
        },
        requests: Vec::new(),
    };

    if effective_max_spawns == 0 || player_positions.is_empty() {
        return result;
    }

    for chunk in eligible_chunks
        .iter()
        .take(CREATURE_SPAWN_MAX_CHUNKS_PER_TICK)
    {
        result.diagnostics.chunks_checked += 1;

        for _ in 0..CREATURE_SPAWN_ATTEMPTS_PER_CHUNK {
            if result.diagnostics.attempts >= CREATURE_SPAWN_MAX_ATTEMPTS_PER_TICK
                || result.requests.len() >= effective_max_spawns
            {
                break;
            }
            result.diagnostics.attempts += 1;

            let x = chunk.min_block_x() + random.next_int_bound(16);
            let z = chunk.min_block_z() + random.next_int_bound(16);
            let feet_y = match top_motion_blocking_no_leaves_feet_y(x, z, &mut block_at) {
                Ok(feet_y) => feet_y,
                Err(SurfaceProbeFailure::MissingBlockData) => {
                    result.diagnostics.blocked_missing_block_data += 1;
                    continue;
                }
                Err(SurfaceProbeFailure::NoSurface) => {
                    result.diagnostics.blocked_world_predicate += 1;
                    continue;
                }
            };

            let pos = BlockPos::new(x, feet_y, z);
            if !is_right_distance_to_player(pos, player_positions) {
                result.diagnostics.blocked_player_distance += 1;
                continue;
            }

            if profile == CreatureSpawnProfile::McloneOverworld {
                match wetland_sample_from_raw(pos, &mut block_at) {
                    Ok(sample) if sample.suitable() => {
                        result.diagnostics.wetland_habitats_detected += 1;
                        let remaining = effective_max_spawns - result.requests.len();
                        let requested_size = MALLARD_FLOCK_MIN_SIZE
                            + random.next_int_bound(
                                (MALLARD_FLOCK_MAX_SIZE - MALLARD_FLOCK_MIN_SIZE + 1) as i32,
                            ) as usize;
                        let target_size = requested_size.min(remaining);
                        let flock = plan_mallard_flock(
                            pos,
                            target_size,
                            player_positions,
                            random,
                            &mut block_at,
                            &mut raw_brightness_at,
                            &mut result.diagnostics,
                        );
                        if flock.len() >= target_size.min(MALLARD_FLOCK_MIN_SIZE) {
                            result.requests.extend(flock);
                            result.diagnostics.mallard_flocks_spawned += 1;
                        } else {
                            result.diagnostics.blocked_world_predicate += 1;
                        }
                        continue;
                    }
                    Ok(_) => {}
                    Err(WetlandHabitatFailure::MissingBlockData) => {
                        result.diagnostics.blocked_missing_wetland_data += 1;
                        continue;
                    }
                }
            }

            let Some(biome) = biome_at(pos) else {
                result.diagnostics.blocked_missing_biome_data += 1;
                continue;
            };
            let spawn_entries = farm_animal_spawns_for_biome(biome);
            if spawn_entries.is_empty() {
                result.diagnostics.blocked_by_biome += 1;
                continue;
            }

            let Some(entry) = choose_weighted_implemented_entry(spawn_entries, random) else {
                result.diagnostics.blocked_unsupported += 1;
                continue;
            };
            let Some(kind) = entry.entity.implemented_kind() else {
                result.diagnostics.blocked_unsupported += 1;
                continue;
            };

            if check_farm_animal_natural_spawn(
                kind,
                pos,
                |block_pos| block_at(block_pos),
                |brightness_pos| raw_brightness_at(brightness_pos),
            )
            .is_err()
            {
                result.diagnostics.blocked_world_predicate += 1;
                continue;
            }

            result.requests.push(CreatureSpawnRequest {
                kind,
                position: Vec3d::new(
                    f64::from(pos.x) + 0.5,
                    f64::from(pos.y),
                    f64::from(pos.z) + 0.5,
                ),
                y_rot_degrees: random.next_float() * 360.0,
            });
        }

        if result.diagnostics.attempts >= CREATURE_SPAWN_MAX_ATTEMPTS_PER_TICK
            || result.requests.len() >= effective_max_spawns
        {
            break;
        }
    }

    result.diagnostics.spawned = result.requests.len();
    result.diagnostics.spawn_budget_exhausted = result.requests.len() >= effective_max_spawns;
    result
}

fn wetland_sample_from_raw(
    pos: BlockPos,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
) -> Result<super::habitat::WetlandHabitatSample, WetlandHabitatFailure> {
    sample_wetland_habitat(pos, &mut |sample_pos| {
        block_at(sample_pos).map(generated_block_state_id)
    })
}

fn plan_mallard_flock(
    anchor: BlockPos,
    target_size: usize,
    player_positions: &[Vec3d],
    random: &mut SimpleRandomSource,
    block_at: &mut impl FnMut(BlockPos) -> Option<RawBlockId>,
    raw_brightness_at: &mut impl FnMut(BlockPos) -> Option<u8>,
    diagnostics: &mut CreatureSpawnDiagnostics,
) -> Vec<CreatureSpawnRequest> {
    let mut requests = Vec::with_capacity(target_size);
    let mut accepted = BTreeSet::new();
    let maximum_attempts = target_size.saturating_mul(MALLARD_FLOCK_MEMBER_ATTEMPTS);

    for member_attempt in 0..maximum_attempts {
        if requests.len() >= target_size {
            break;
        }
        let candidate = if member_attempt == 0 {
            anchor
        } else {
            let x = anchor.x + random.next_int_bound(MALLARD_FLOCK_MEMBER_RADIUS * 2 + 1)
                - MALLARD_FLOCK_MEMBER_RADIUS;
            let z = anchor.z + random.next_int_bound(MALLARD_FLOCK_MEMBER_RADIUS * 2 + 1)
                - MALLARD_FLOCK_MEMBER_RADIUS;
            match top_motion_blocking_no_leaves_feet_y(x, z, block_at) {
                Ok(feet_y) => BlockPos::new(x, feet_y, z),
                Err(SurfaceProbeFailure::MissingBlockData) => {
                    diagnostics.blocked_missing_block_data += 1;
                    continue;
                }
                Err(SurfaceProbeFailure::NoSurface) => {
                    diagnostics.blocked_world_predicate += 1;
                    continue;
                }
            }
        };

        if !accepted.insert(candidate) {
            continue;
        }
        if !is_right_distance_to_player(candidate, player_positions) {
            diagnostics.blocked_player_distance += 1;
            continue;
        }
        match wetland_sample_from_raw(candidate, block_at) {
            Ok(sample) if sample.suitable() => {}
            Ok(_) => continue,
            Err(WetlandHabitatFailure::MissingBlockData) => {
                diagnostics.blocked_missing_wetland_data += 1;
                continue;
            }
        }
        if check_land_creature_natural_spawn(
            EntityKind::Mallard,
            candidate,
            |block_pos| block_at(block_pos),
            |brightness_pos| raw_brightness_at(brightness_pos),
        )
        .is_err()
        {
            diagnostics.blocked_world_predicate += 1;
            continue;
        }
        requests.push(CreatureSpawnRequest {
            kind: EntityKind::Mallard,
            position: Vec3d::new(
                f64::from(candidate.x) + 0.5,
                f64::from(candidate.y),
                f64::from(candidate.z) + 0.5,
            ),
            y_rot_degrees: random.next_float() * 360.0,
        });
    }
    requests
}

fn choose_weighted_implemented_entry(
    entries: &[MobSpawnEntry],
    random: &mut SimpleRandomSource,
) -> Option<MobSpawnEntry> {
    let total_weight = entries
        .iter()
        .copied()
        .filter(|entry| entry.is_implemented())
        .map(|entry| entry.weight)
        .sum::<u32>();
    if total_weight == 0 {
        return None;
    }

    let mut pick = random.next_int_bound(total_weight as i32) as u32;
    for entry in entries
        .iter()
        .copied()
        .filter(|entry| entry.is_implemented())
    {
        if pick < entry.weight {
            return Some(entry);
        }
        pick -= entry.weight;
    }
    None
}

fn is_right_distance_to_player(pos: BlockPos, player_positions: &[Vec3d]) -> bool {
    let x = f64::from(pos.x) + 0.5;
    let y = f64::from(pos.y);
    let z = f64::from(pos.z) + 0.5;
    player_positions.iter().any(|player| {
        let distance_sqr = squared_distance(x, y, z, *player);
        distance_sqr > MIN_SPAWN_DISTANCE_BLOCKS * MIN_SPAWN_DISTANCE_BLOCKS
            && distance_sqr <= MAX_SPAWN_DISTANCE_BLOCKS * MAX_SPAWN_DISTANCE_BLOCKS
    })
}

fn squared_distance(x: f64, y: f64, z: f64, player: Vec3d) -> f64 {
    let dx = x - player.x;
    let dy = y - player.y;
    let dz = z - player.z;
    dx * dx + dy * dy + dz * dz
}

#[cfg(test)]
mod tests {
    use mclone_worldgen::biome::get_layered_biome_by_id;
    use mclone_worldgen::block::{AIR, DIRT, GRASS_BLOCK, WATER};

    use super::*;

    fn grass_surface_block_at(pos: BlockPos) -> Option<RawBlockId> {
        if pos.y == 63 {
            Some(GRASS_BLOCK)
        } else {
            Some(AIR)
        }
    }

    fn striped_wetland_block_at(pos: BlockPos) -> Option<RawBlockId> {
        Some(if pos.y <= 62 {
            DIRT
        } else if pos.y == 63 {
            GRASS_BLOCK
        } else if pos.y == 64 && pos.x.rem_euclid(4) == 0 {
            WATER
        } else {
            AIR
        })
    }

    #[test]
    fn creature_spawns_from_supported_biome_and_valid_surface() {
        let chunks = BTreeSet::from([ChunkPos::new(0, 0)]);
        let mut random = SimpleRandomSource::new(12_345);
        let plains = get_layered_biome_by_id(1);

        let result = plan_creature_spawns(
            &chunks,
            &[Vec3d::new(80.0, 64.0, 8.0)],
            2,
            CreatureSpawnProfile::ReferenceFarmAnimals,
            &mut random,
            grass_surface_block_at,
            |_| Some(plains),
            |_| Some(15),
        );

        assert_eq!(result.diagnostics.chunks_checked, 1);
        assert!(result.diagnostics.attempts >= 2);
        assert_eq!(result.requests.len(), 2);
        assert_eq!(result.diagnostics.spawned, 2);
        assert!(result.diagnostics.spawn_budget_exhausted);
        for request in result.requests {
            assert!(matches!(
                request.kind,
                EntityKind::Cow | EntityKind::Chicken
            ));
            assert_eq!(request.position.y, 64.0);
            assert!((0.0..360.0).contains(&request.y_rot_degrees));
        }
    }

    #[test]
    fn creature_spawns_respect_player_minimum_distance() {
        let chunks = BTreeSet::from([ChunkPos::new(0, 0)]);
        let mut random = SimpleRandomSource::new(12_345);
        let plains = get_layered_biome_by_id(1);

        let result = plan_creature_spawns(
            &chunks,
            &[Vec3d::new(8.0, 64.0, 8.0)],
            2,
            CreatureSpawnProfile::ReferenceFarmAnimals,
            &mut random,
            grass_surface_block_at,
            |_| Some(plains),
            |_| Some(15),
        );

        assert_eq!(result.requests, Vec::new());
        assert_eq!(result.diagnostics.spawned, 0);
        assert_eq!(
            result.diagnostics.blocked_player_distance,
            CREATURE_SPAWN_ATTEMPTS_PER_CHUNK
        );
    }

    #[test]
    fn creature_spawns_skip_unsupported_biomes() {
        let chunks = BTreeSet::from([ChunkPos::new(0, 0)]);
        let mut random = SimpleRandomSource::new(12_345);
        let desert = get_layered_biome_by_id(2);

        let result = plan_creature_spawns(
            &chunks,
            &[Vec3d::new(80.0, 64.0, 8.0)],
            2,
            CreatureSpawnProfile::ReferenceFarmAnimals,
            &mut random,
            grass_surface_block_at,
            |_| Some(desert),
            |_| Some(15),
        );

        assert_eq!(result.requests, Vec::new());
        assert_eq!(
            result.diagnostics.blocked_by_biome,
            result.diagnostics.attempts
        );
    }

    #[test]
    fn creature_spawns_report_missing_generated_biome_data() {
        let chunks = BTreeSet::from([ChunkPos::new(0, 0)]);
        let mut random = SimpleRandomSource::new(12_345);

        let result = plan_creature_spawns(
            &chunks,
            &[Vec3d::new(80.0, 64.0, 8.0)],
            2,
            CreatureSpawnProfile::ReferenceFarmAnimals,
            &mut random,
            grass_surface_block_at,
            |_| None,
            |_| Some(15),
        );

        assert_eq!(result.requests, Vec::new());
        assert_eq!(
            result.diagnostics.blocked_missing_biome_data,
            result.diagnostics.attempts
        );
        assert_eq!(result.diagnostics.blocked_by_biome, 0);
    }

    #[test]
    fn mclone_wetlands_admit_bounded_mallard_flocks_only_in_that_profile() {
        let chunks = BTreeSet::from([ChunkPos::new(0, 0)]);
        let plains = get_layered_biome_by_id(1);
        let mut mclone_random = SimpleRandomSource::new(12_345);

        let mclone = plan_creature_spawns(
            &chunks,
            &[Vec3d::new(80.0, 64.0, 8.0)],
            4,
            CreatureSpawnProfile::McloneOverworld,
            &mut mclone_random,
            striped_wetland_block_at,
            |_| Some(plains),
            |_| Some(15),
        );

        assert!((2..=4).contains(&mclone.requests.len()));
        assert!(
            mclone
                .requests
                .iter()
                .all(|request| request.kind == EntityKind::Mallard)
        );
        assert!(mclone.diagnostics.wetland_habitats_detected >= 1);
        assert!(mclone.diagnostics.mallard_flocks_spawned >= 1);
        for request in &mclone.requests {
            let feet = BlockPos::containing(request.position);
            let sample = wetland_sample_from_raw(feet, &mut striped_wetland_block_at).unwrap();
            assert!(sample.suitable(), "every flock member validates habitat");
            assert_eq!(
                check_land_creature_natural_spawn(
                    EntityKind::Mallard,
                    feet,
                    striped_wetland_block_at,
                    |_| Some(15),
                ),
                Ok(())
            );
        }

        let mut reference_random = SimpleRandomSource::new(12_345);
        let reference = plan_creature_spawns(
            &chunks,
            &[Vec3d::new(80.0, 64.0, 8.0)],
            4,
            CreatureSpawnProfile::ReferenceFarmAnimals,
            &mut reference_random,
            striped_wetland_block_at,
            |_| Some(plains),
            |_| Some(15),
        );
        assert!(
            reference
                .requests
                .iter()
                .all(|request| request.kind != EntityKind::Mallard)
        );
        assert_eq!(reference.diagnostics.wetland_habitats_detected, 0);
    }

    #[test]
    fn mallard_flock_respects_a_one_member_remaining_budget() {
        let chunks = BTreeSet::from([ChunkPos::new(0, 0)]);
        let plains = get_layered_biome_by_id(1);
        let mut random = SimpleRandomSource::new(12_345);

        let result = plan_creature_spawns(
            &chunks,
            &[Vec3d::new(80.0, 64.0, 8.0)],
            1,
            CreatureSpawnProfile::McloneOverworld,
            &mut random,
            striped_wetland_block_at,
            |_| Some(plains),
            |_| Some(15),
        );

        assert_eq!(result.requests.len(), 1);
        assert_eq!(result.requests[0].kind, EntityKind::Mallard);
        assert!(result.diagnostics.spawn_budget_exhausted);
    }

    #[test]
    fn mclone_wetland_planning_fails_closed_on_missing_habitat_blocks() {
        let chunks = BTreeSet::from([ChunkPos::new(0, 0)]);
        let plains = get_layered_biome_by_id(1);
        let mut random = SimpleRandomSource::new(12_345);

        let result = plan_creature_spawns(
            &chunks,
            &[Vec3d::new(80.0, 64.0, 8.0)],
            4,
            CreatureSpawnProfile::McloneOverworld,
            &mut random,
            |pos| {
                if pos.y == 62 {
                    None
                } else {
                    Some(if pos.y == 63 { GRASS_BLOCK } else { AIR })
                }
            },
            |_| Some(plains),
            |_| Some(15),
        );

        assert!(result.requests.is_empty());
        assert_eq!(
            result.diagnostics.blocked_missing_wetland_data,
            result.diagnostics.attempts
        );
    }
}
