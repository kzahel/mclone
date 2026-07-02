use super::super::metadata::EntityCategory;

pub(crate) const NATURAL_SPAWN_MAGIC_NUMBER: u32 = 17 * 17;
pub(crate) const CREATURE_NATURAL_SPAWN_INTERVAL_TICKS: u64 = 400;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum MobCategory {
    Monster,
    Creature,
    Ambient,
    UndergroundWaterCreature,
    WaterCreature,
    WaterAmbient,
    Misc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MobCategoryFacts {
    pub(crate) name: &'static str,
    pub(crate) max_instances_per_chunk: i32,
    pub(crate) friendly: bool,
    pub(crate) persistent: bool,
    pub(crate) no_despawn_distance: u32,
    pub(crate) despawn_distance: u32,
}

pub(crate) const ALL_MOB_CATEGORIES: [MobCategory; MobCategory::COUNT] = [
    MobCategory::Monster,
    MobCategory::Creature,
    MobCategory::Ambient,
    MobCategory::UndergroundWaterCreature,
    MobCategory::WaterCreature,
    MobCategory::WaterAmbient,
    MobCategory::Misc,
];

pub(crate) const NATURAL_SPAWNING_CATEGORIES: [MobCategory; 6] = [
    MobCategory::Monster,
    MobCategory::Creature,
    MobCategory::Ambient,
    MobCategory::UndergroundWaterCreature,
    MobCategory::WaterCreature,
    MobCategory::WaterAmbient,
];

impl MobCategory {
    pub(crate) const COUNT: usize = 7;

    pub(crate) const fn facts(self) -> MobCategoryFacts {
        match self {
            Self::Monster => MobCategoryFacts::new("monster", 70, false, false, 128),
            Self::Creature => MobCategoryFacts::new("creature", 10, true, true, 128),
            Self::Ambient => MobCategoryFacts::new("ambient", 15, true, false, 128),
            Self::UndergroundWaterCreature => {
                MobCategoryFacts::new("underground_water_creature", 5, true, false, 128)
            }
            Self::WaterCreature => MobCategoryFacts::new("water_creature", 5, true, false, 128),
            Self::WaterAmbient => MobCategoryFacts::new("water_ambient", 20, true, false, 64),
            Self::Misc => MobCategoryFacts::new("misc", -1, true, true, 128),
        }
    }

    pub(crate) const fn from_entity_category(category: EntityCategory) -> Self {
        match category {
            EntityCategory::Creature => Self::Creature,
            EntityCategory::Misc => Self::Misc,
        }
    }

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Monster => 0,
            Self::Creature => 1,
            Self::Ambient => 2,
            Self::UndergroundWaterCreature => 3,
            Self::WaterCreature => 4,
            Self::WaterAmbient => 5,
            Self::Misc => 6,
        }
    }

    pub(crate) const fn max_instances_per_chunk(self) -> i32 {
        self.facts().max_instances_per_chunk
    }

    pub(crate) const fn is_friendly(self) -> bool {
        self.facts().friendly
    }

    pub(crate) const fn is_persistent(self) -> bool {
        self.facts().persistent
    }

    pub(crate) const fn is_natural_spawning(self) -> bool {
        !matches!(self, Self::Misc)
    }

    pub(crate) const fn natural_spawn_interval_ticks(self) -> u64 {
        match self {
            Self::Creature => CREATURE_NATURAL_SPAWN_INTERVAL_TICKS,
            _ => 1,
        }
    }

    pub(crate) const fn should_run_natural_spawn_at_tick(self, game_time: u64) -> bool {
        match self {
            Self::Creature => game_time % CREATURE_NATURAL_SPAWN_INTERVAL_TICKS == 0,
            _ => true,
        }
    }
}

impl MobCategoryFacts {
    const fn new(
        name: &'static str,
        max_instances_per_chunk: i32,
        friendly: bool,
        persistent: bool,
        despawn_distance: u32,
    ) -> Self {
        Self {
            name,
            max_instances_per_chunk,
            friendly,
            persistent,
            no_despawn_distance: 32,
            despawn_distance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creature_category_matches_java_1_17_1_facts() {
        let facts = MobCategory::Creature.facts();

        assert_eq!(facts.name, "creature");
        assert_eq!(facts.max_instances_per_chunk, 10);
        assert!(facts.friendly);
        assert!(facts.persistent);
        assert_eq!(facts.no_despawn_distance, 32);
        assert_eq!(facts.despawn_distance, 128);
    }

    #[test]
    fn natural_spawning_categories_exclude_misc() {
        assert_eq!(
            NATURAL_SPAWNING_CATEGORIES.len(),
            ALL_MOB_CATEGORIES.len() - 1
        );
        assert!(
            NATURAL_SPAWNING_CATEGORIES
                .iter()
                .all(|category| category.is_natural_spawning())
        );
        assert!(!NATURAL_SPAWNING_CATEGORIES.contains(&MobCategory::Misc));
    }

    #[test]
    fn creature_spawn_cadence_matches_java_400_tick_gate() {
        assert!(MobCategory::Creature.should_run_natural_spawn_at_tick(0));
        assert!(!MobCategory::Creature.should_run_natural_spawn_at_tick(399));
        assert!(MobCategory::Creature.should_run_natural_spawn_at_tick(400));
        assert!(!MobCategory::Creature.should_run_natural_spawn_at_tick(401));
        assert!(MobCategory::Monster.should_run_natural_spawn_at_tick(401));
    }

    #[test]
    fn entity_category_mapping_keeps_passive_mobs_in_creature_bucket() {
        assert_eq!(
            MobCategory::from_entity_category(EntityCategory::Creature),
            MobCategory::Creature
        );
        assert_eq!(
            MobCategory::from_entity_category(EntityCategory::Misc),
            MobCategory::Misc
        );
    }
}
