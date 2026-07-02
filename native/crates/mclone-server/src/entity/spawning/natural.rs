use std::collections::BTreeSet;

use mclone_core::{BlockPos, ChunkPos, Vec3d};

use super::mob_category::{MobCategory, NATURAL_SPAWNING_CATEGORIES};
use super::spawn_state::{CategorySpawnState, MobCategoryCounts, SpawnState};

pub(crate) const NATURAL_SPAWN_DISTANCE_CHUNKS: i32 = 8;
pub(crate) const NATURAL_SPAWN_DISTANCE_BLOCKS: f64 = 128.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NaturalSpawnConfig {
    pub(crate) enabled: bool,
    pub(crate) allow_friendly_categories: bool,
    pub(crate) allow_hostile_categories: bool,
    pub(crate) allow_persistent_categories: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NaturalSpawnContext {
    pub(crate) game_time: u64,
    pub(crate) spawnable_chunk_count: Option<u32>,
    pub(crate) mob_category_counts: Option<MobCategoryCounts>,
    pub(crate) player_distance_spawnable_chunks_ready: bool,
    pub(crate) biome_spawn_tables_ready: bool,
    pub(crate) placement_predicates_ready: bool,
    pub(crate) brightness_checks_ready: bool,
    pub(crate) collision_checks_ready: bool,
    pub(crate) gamerules_ready: bool,
    pub(crate) despawn_rules_ready: bool,
    pub(crate) persistence_ready: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct NaturalSpawnChunkInputs {
    pub(crate) player_distance_chunks: BTreeSet<ChunkPos>,
    pub(crate) eligible_entity_ticking_chunks: BTreeSet<ChunkPos>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NaturalSpawnBlocker {
    Disabled,
    PlayerDistanceSpawnableChunks,
    LiveCategoryCounts,
    BiomeSpawnTables,
    PlacementPredicates,
    BrightnessChecks,
    CollisionChecks,
    GameRules,
    DespawnRules,
    Persistence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NaturalSpawnPlan {
    pub(crate) blocked_by: Vec<NaturalSpawnBlocker>,
    pub(crate) categories: Vec<CategoryNaturalSpawnPlan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CategoryNaturalSpawnPlan {
    pub(crate) category: MobCategory,
    pub(crate) current_count: u32,
    pub(crate) cap: u32,
    pub(crate) cadence_ready: bool,
    pub(crate) cap_has_room: bool,
    pub(crate) should_attempt: bool,
}

impl Default for NaturalSpawnConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_friendly_categories: true,
            allow_hostile_categories: true,
            allow_persistent_categories: true,
        }
    }
}

impl Default for NaturalSpawnContext {
    fn default() -> Self {
        Self {
            game_time: 0,
            spawnable_chunk_count: None,
            mob_category_counts: None,
            player_distance_spawnable_chunks_ready: false,
            biome_spawn_tables_ready: false,
            placement_predicates_ready: false,
            brightness_checks_ready: false,
            collision_checks_ready: false,
            gamerules_ready: false,
            despawn_rules_ready: false,
            persistence_ready: false,
        }
    }
}

impl NaturalSpawnConfig {
    pub(crate) const fn enabled_all_categories() -> Self {
        Self {
            enabled: true,
            allow_friendly_categories: true,
            allow_hostile_categories: true,
            allow_persistent_categories: true,
        }
    }

    pub(crate) const fn allows_category(self, category: MobCategory) -> bool {
        let friendly_allowed = if category.is_friendly() {
            self.allow_friendly_categories
        } else {
            self.allow_hostile_categories
        };
        let persistent_allowed = !category.is_persistent() || self.allow_persistent_categories;
        friendly_allowed && persistent_allowed
    }
}

impl NaturalSpawnContext {
    pub(crate) const fn with_live_chunk_and_count_inputs(
        game_time: u64,
        spawnable_chunk_count: u32,
        mob_category_counts: MobCategoryCounts,
    ) -> Self {
        Self {
            game_time,
            spawnable_chunk_count: Some(spawnable_chunk_count),
            mob_category_counts: Some(mob_category_counts),
            player_distance_spawnable_chunks_ready: true,
            biome_spawn_tables_ready: false,
            placement_predicates_ready: false,
            brightness_checks_ready: false,
            collision_checks_ready: false,
            gamerules_ready: false,
            despawn_rules_ready: false,
            persistence_ready: false,
        }
    }

    pub(crate) const fn with_ready_inputs(
        game_time: u64,
        spawnable_chunk_count: u32,
        mob_category_counts: MobCategoryCounts,
    ) -> Self {
        Self {
            game_time,
            spawnable_chunk_count: Some(spawnable_chunk_count),
            mob_category_counts: Some(mob_category_counts),
            player_distance_spawnable_chunks_ready: true,
            biome_spawn_tables_ready: true,
            placement_predicates_ready: true,
            brightness_checks_ready: true,
            collision_checks_ready: true,
            gamerules_ready: true,
            despawn_rules_ready: true,
            persistence_ready: true,
        }
    }

    pub(crate) fn missing_blockers(self) -> Vec<NaturalSpawnBlocker> {
        let mut blockers = Vec::new();
        if self.spawnable_chunk_count.is_none() || !self.player_distance_spawnable_chunks_ready {
            blockers.push(NaturalSpawnBlocker::PlayerDistanceSpawnableChunks);
        }
        if self.mob_category_counts.is_none() {
            blockers.push(NaturalSpawnBlocker::LiveCategoryCounts);
        }
        if !self.biome_spawn_tables_ready {
            blockers.push(NaturalSpawnBlocker::BiomeSpawnTables);
        }
        if !self.placement_predicates_ready {
            blockers.push(NaturalSpawnBlocker::PlacementPredicates);
        }
        if !self.brightness_checks_ready {
            blockers.push(NaturalSpawnBlocker::BrightnessChecks);
        }
        if !self.collision_checks_ready {
            blockers.push(NaturalSpawnBlocker::CollisionChecks);
        }
        if !self.gamerules_ready {
            blockers.push(NaturalSpawnBlocker::GameRules);
        }
        if !self.despawn_rules_ready {
            blockers.push(NaturalSpawnBlocker::DespawnRules);
        }
        if !self.persistence_ready {
            blockers.push(NaturalSpawnBlocker::Persistence);
        }
        blockers
    }
}

impl NaturalSpawnChunkInputs {
    pub(crate) fn from_players_and_entity_ticking_chunks(
        player_positions: &[Vec3d],
        entity_ticking_chunks: &[ChunkPos],
    ) -> Self {
        let player_distance_chunks = natural_spawn_player_distance_chunks(player_positions);
        let eligible_entity_ticking_chunks = entity_ticking_chunks
            .iter()
            .copied()
            .filter(|chunk| {
                player_distance_chunks.contains(chunk)
                    && has_player_close_for_spawning(*chunk, player_positions)
            })
            .collect();
        Self {
            player_distance_chunks,
            eligible_entity_ticking_chunks,
        }
    }

    pub(crate) fn player_distance_chunk_count(&self) -> usize {
        self.player_distance_chunks.len()
    }

    pub(crate) fn eligible_entity_ticking_chunk_count(&self) -> usize {
        self.eligible_entity_ticking_chunks.len()
    }
}

impl NaturalSpawnPlan {
    pub(crate) fn is_blocked(&self) -> bool {
        !self.blocked_by.is_empty()
    }
}

impl CategoryNaturalSpawnPlan {
    fn new(category: MobCategory, game_time: u64, category_state: CategorySpawnState) -> Self {
        let cadence_ready = category.should_run_natural_spawn_at_tick(game_time);
        let cap_has_room = category_state.has_room();
        Self {
            category,
            current_count: category_state.current_count,
            cap: category_state.cap,
            cadence_ready,
            cap_has_room,
            should_attempt: cadence_ready && cap_has_room,
        }
    }
}

pub(crate) fn plan_natural_spawns(
    config: NaturalSpawnConfig,
    context: NaturalSpawnContext,
) -> NaturalSpawnPlan {
    if !config.enabled {
        return NaturalSpawnPlan {
            blocked_by: vec![NaturalSpawnBlocker::Disabled],
            categories: Vec::new(),
        };
    }

    let blocked_by = context.missing_blockers();
    if !blocked_by.is_empty() {
        return NaturalSpawnPlan {
            blocked_by,
            categories: Vec::new(),
        };
    }

    let spawn_state = SpawnState::new(
        context
            .spawnable_chunk_count
            .expect("spawnable chunk count checked by missing_blockers"),
        context
            .mob_category_counts
            .expect("category counts checked by missing_blockers"),
    );
    let categories = NATURAL_SPAWNING_CATEGORIES
        .into_iter()
        .filter(|category| config.allows_category(*category))
        .map(|category| {
            CategoryNaturalSpawnPlan::new(
                category,
                context.game_time,
                spawn_state.category_state(category),
            )
        })
        .collect();

    NaturalSpawnPlan {
        blocked_by: Vec::new(),
        categories,
    }
}

fn natural_spawn_player_distance_chunks(player_positions: &[Vec3d]) -> BTreeSet<ChunkPos> {
    player_positions
        .iter()
        .flat_map(|position| {
            let center = BlockPos::containing(*position).chunk_pos();
            let min_x = center.x - NATURAL_SPAWN_DISTANCE_CHUNKS;
            let max_x = center.x + NATURAL_SPAWN_DISTANCE_CHUNKS;
            let min_z = center.z - NATURAL_SPAWN_DISTANCE_CHUNKS;
            let max_z = center.z + NATURAL_SPAWN_DISTANCE_CHUNKS;
            (min_x..=max_x).flat_map(move |x| (min_z..=max_z).map(move |z| ChunkPos::new(x, z)))
        })
        .collect()
}

fn has_player_close_for_spawning(chunk: ChunkPos, player_positions: &[Vec3d]) -> bool {
    player_positions
        .iter()
        .any(|position| player_spawn_distance_squared(chunk, *position) < spawn_distance_sqr())
}

fn player_spawn_distance_squared(chunk: ChunkPos, player_position: Vec3d) -> f64 {
    let center_x = f64::from(chunk.x) * 16.0 + 8.0;
    let center_z = f64::from(chunk.z) * 16.0 + 8.0;
    let dx = center_x - player_position.x;
    let dz = center_z - player_position.z;
    dx * dx + dz * dz
}

const fn spawn_distance_sqr() -> f64 {
    NATURAL_SPAWN_DISTANCE_BLOCKS * NATURAL_SPAWN_DISTANCE_BLOCKS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creature_plan(plan: &NaturalSpawnPlan) -> CategoryNaturalSpawnPlan {
        plan.categories
            .iter()
            .copied()
            .find(|category| category.category == MobCategory::Creature)
            .expect("creature plan")
    }

    #[test]
    fn natural_spawning_defaults_to_disabled_even_before_inputs_exist() {
        let plan = plan_natural_spawns(
            NaturalSpawnConfig::default(),
            NaturalSpawnContext::default(),
        );

        assert!(plan.is_blocked());
        assert_eq!(plan.blocked_by, vec![NaturalSpawnBlocker::Disabled]);
        assert!(plan.categories.is_empty());
    }

    #[test]
    fn enabled_planner_stays_blocked_until_required_inputs_exist() {
        let plan = plan_natural_spawns(
            NaturalSpawnConfig::enabled_all_categories(),
            NaturalSpawnContext::default(),
        );

        assert!(plan.is_blocked());
        assert_eq!(
            plan.blocked_by,
            vec![
                NaturalSpawnBlocker::PlayerDistanceSpawnableChunks,
                NaturalSpawnBlocker::LiveCategoryCounts,
                NaturalSpawnBlocker::BiomeSpawnTables,
                NaturalSpawnBlocker::PlacementPredicates,
                NaturalSpawnBlocker::BrightnessChecks,
                NaturalSpawnBlocker::CollisionChecks,
                NaturalSpawnBlocker::GameRules,
                NaturalSpawnBlocker::DespawnRules,
                NaturalSpawnBlocker::Persistence,
            ]
        );
        assert!(plan.categories.is_empty());
    }

    #[test]
    fn ready_plan_applies_creature_cadence_before_attempting_category() {
        let counts = MobCategoryCounts::new().with_count(MobCategory::Creature, 0);
        let not_creature_tick = plan_natural_spawns(
            NaturalSpawnConfig::enabled_all_categories(),
            NaturalSpawnContext::with_ready_inputs(399, 289, counts),
        );
        let creature_tick = plan_natural_spawns(
            NaturalSpawnConfig::enabled_all_categories(),
            NaturalSpawnContext::with_ready_inputs(400, 289, counts),
        );

        assert!(!not_creature_tick.is_blocked());
        assert!(!creature_plan(&not_creature_tick).cadence_ready);
        assert!(!creature_plan(&not_creature_tick).should_attempt);
        assert!(creature_plan(&creature_tick).cadence_ready);
        assert!(creature_plan(&creature_tick).should_attempt);
    }

    #[test]
    fn ready_plan_applies_mob_cap_before_attempting_category() {
        let full_counts = MobCategoryCounts::new().with_count(MobCategory::Creature, 10);
        let plan = plan_natural_spawns(
            NaturalSpawnConfig::enabled_all_categories(),
            NaturalSpawnContext::with_ready_inputs(400, 289, full_counts),
        );
        let creature = creature_plan(&plan);

        assert!(!plan.is_blocked());
        assert_eq!(creature.current_count, 10);
        assert_eq!(creature.cap, 10);
        assert!(!creature.cap_has_room);
        assert!(!creature.should_attempt);
    }

    #[test]
    fn category_flags_mirror_spawn_for_chunk_filter_shape() {
        let counts = MobCategoryCounts::new();
        let mut config = NaturalSpawnConfig::enabled_all_categories();
        config.allow_persistent_categories = false;

        let plan = plan_natural_spawns(
            config,
            NaturalSpawnContext::with_ready_inputs(400, 289, counts),
        );

        assert!(!plan.is_blocked());
        assert!(
            plan.categories
                .iter()
                .all(|category| category.category != MobCategory::Creature)
        );
    }

    #[test]
    fn natural_spawn_player_distance_chunks_use_java_fixed_distance_tracker_radius() {
        let inputs = NaturalSpawnChunkInputs::from_players_and_entity_ticking_chunks(
            &[Vec3d::new(8.0, 64.0, 8.0)],
            &[],
        );

        assert_eq!(inputs.player_distance_chunk_count(), 17 * 17);
        assert!(
            inputs
                .player_distance_chunks
                .contains(&ChunkPos::new(-8, -8))
        );
        assert!(inputs.player_distance_chunks.contains(&ChunkPos::new(8, 8)));
        assert!(!inputs.player_distance_chunks.contains(&ChunkPos::new(9, 0)));
    }

    #[test]
    fn natural_spawn_eligible_chunks_require_entity_ticking_and_close_player_distance() {
        let inputs = NaturalSpawnChunkInputs::from_players_and_entity_ticking_chunks(
            &[Vec3d::new(8.0, 64.0, 8.0)],
            &[
                ChunkPos::new(0, 0),
                ChunkPos::new(7, 0),
                ChunkPos::new(8, 0),
                ChunkPos::new(7, 7),
                ChunkPos::new(9, 0),
            ],
        );

        assert_eq!(
            inputs.eligible_entity_ticking_chunks,
            BTreeSet::from([ChunkPos::new(0, 0), ChunkPos::new(7, 0)])
        );
    }

    #[test]
    fn partial_live_inputs_clear_chunk_and_count_blockers_only() {
        let context = NaturalSpawnContext::with_live_chunk_and_count_inputs(
            400,
            289,
            MobCategoryCounts::new(),
        );

        assert_eq!(
            context.missing_blockers(),
            vec![
                NaturalSpawnBlocker::BiomeSpawnTables,
                NaturalSpawnBlocker::PlacementPredicates,
                NaturalSpawnBlocker::BrightnessChecks,
                NaturalSpawnBlocker::CollisionChecks,
                NaturalSpawnBlocker::GameRules,
                NaturalSpawnBlocker::DespawnRules,
                NaturalSpawnBlocker::Persistence,
            ]
        );
    }
}
