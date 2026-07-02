use mclone_protocol::EntityKind;
use mclone_worldgen::biome::BiomeDefinition;

use super::mob_category::MobCategory;

pub(crate) const DEFAULT_CREATURE_GENERATION_PROBABILITY: f32 = 0.1;

pub(crate) const FARM_ANIMAL_SPAWNS: [MobSpawnEntry; 4] = [
    MobSpawnEntry::new(VanillaSpawnEntity::Sheep, 12, 4, 4),
    MobSpawnEntry::new(VanillaSpawnEntity::Pig, 10, 4, 4),
    MobSpawnEntry::new(VanillaSpawnEntity::Chicken, 10, 4, 4),
    MobSpawnEntry::new(VanillaSpawnEntity::Cow, 8, 4, 4),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VanillaSpawnEntity {
    Sheep,
    Pig,
    Chicken,
    Cow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MobSpawnEntry {
    pub(crate) entity: VanillaSpawnEntity,
    pub(crate) category: MobCategory,
    pub(crate) weight: u32,
    pub(crate) min_count: u32,
    pub(crate) max_count: u32,
}

impl VanillaSpawnEntity {
    pub(crate) const fn implemented_kind(self) -> Option<EntityKind> {
        match self {
            Self::Chicken => Some(EntityKind::Chicken),
            Self::Cow => Some(EntityKind::Cow),
            Self::Sheep | Self::Pig => None,
        }
    }
}

impl MobSpawnEntry {
    const fn new(entity: VanillaSpawnEntity, weight: u32, min_count: u32, max_count: u32) -> Self {
        Self {
            entity,
            category: MobCategory::Creature,
            weight,
            min_count,
            max_count,
        }
    }

    pub(crate) const fn is_implemented(self) -> bool {
        self.entity.implemented_kind().is_some()
    }
}

pub(crate) fn farm_animal_spawn_for_kind(kind: EntityKind) -> Option<MobSpawnEntry> {
    FARM_ANIMAL_SPAWNS
        .iter()
        .copied()
        .find(|entry| entry.entity.implemented_kind() == Some(kind))
}

pub(crate) fn farm_animal_spawns_for_biome(biome: BiomeDefinition) -> &'static [MobSpawnEntry] {
    if biome_has_farm_animal_spawns(biome) {
        &FARM_ANIMAL_SPAWNS
    } else {
        &[]
    }
}

pub(crate) fn biome_has_farm_animal_spawns(biome: BiomeDefinition) -> bool {
    matches!(
        biome.key(),
        // plainsSpawns(...)
        "minecraft:plains" | "minecraft:sunflower_plains"
            // defaultSpawns(...) forest path
            | "minecraft:forest"
            | "minecraft:flower_forest"
            // birchForestBiome(...)
            | "minecraft:birch_forest"
            | "minecraft:birch_forest_hills"
            | "minecraft:tall_birch_forest"
            | "minecraft:tall_birch_hills"
            // darkForestBiome(...)
            | "minecraft:dark_forest"
            | "minecraft:dark_forest_hills"
            // taigaBiome(...)
            | "minecraft:taiga"
            | "minecraft:taiga_hills"
            | "minecraft:taiga_mountains"
            | "minecraft:snowy_taiga"
            | "minecraft:snowy_taiga_hills"
            | "minecraft:snowy_taiga_mountains"
            // giantTreeTaiga(...)
            | "minecraft:giant_tree_taiga"
            | "minecraft:giant_tree_taiga_hills"
            | "minecraft:giant_spruce_taiga"
            | "minecraft:giant_spruce_taiga_hills"
            // mountainBiome(...)
            | "minecraft:mountains"
            | "minecraft:mountain_edge"
            | "minecraft:wooded_mountains"
            | "minecraft:gravelly_mountains"
            | "minecraft:modified_gravelly_mountains"
            // savannaMobs(...)
            | "minecraft:savanna"
            | "minecraft:savanna_plateau"
            | "minecraft:shattered_savanna"
            | "minecraft:shattered_savanna_plateau"
            // swampBiome(...)
            | "minecraft:swamp"
            | "minecraft:swamp_hills"
    )
}

#[cfg(test)]
mod tests {
    use mclone_worldgen::biome::get_layered_biome_by_id;

    use super::*;

    #[test]
    fn farm_animal_table_keeps_java_1_17_1_order_and_weights() {
        assert_eq!(
            FARM_ANIMAL_SPAWNS,
            [
                MobSpawnEntry::new(VanillaSpawnEntity::Sheep, 12, 4, 4),
                MobSpawnEntry::new(VanillaSpawnEntity::Pig, 10, 4, 4),
                MobSpawnEntry::new(VanillaSpawnEntity::Chicken, 10, 4, 4),
                MobSpawnEntry::new(VanillaSpawnEntity::Cow, 8, 4, 4),
            ]
        );
    }

    #[test]
    fn supported_farm_animals_have_protocol_entity_kinds() {
        let chicken = farm_animal_spawn_for_kind(EntityKind::Chicken).expect("chicken spawn");
        let cow = farm_animal_spawn_for_kind(EntityKind::Cow).expect("cow spawn");

        assert_eq!(chicken.entity, VanillaSpawnEntity::Chicken);
        assert_eq!(chicken.category, MobCategory::Creature);
        assert_eq!(chicken.weight, 10);
        assert_eq!((chicken.min_count, chicken.max_count), (4, 4));
        assert!(chicken.is_implemented());

        assert_eq!(cow.entity, VanillaSpawnEntity::Cow);
        assert_eq!(cow.category, MobCategory::Creature);
        assert_eq!(cow.weight, 8);
        assert_eq!((cow.min_count, cow.max_count), (4, 4));
        assert!(cow.is_implemented());
    }

    #[test]
    fn unsupported_farm_animals_remain_documented_without_protocol_kinds() {
        assert_eq!(VanillaSpawnEntity::Sheep.implemented_kind(), None);
        assert_eq!(VanillaSpawnEntity::Pig.implemented_kind(), None);
        assert_eq!(farm_animal_spawn_for_kind(EntityKind::Item), None);
    }

    #[test]
    fn farm_animal_biome_table_keeps_reference_membership_boundaries() {
        let plains = get_layered_biome_by_id(1);
        let forest = get_layered_biome_by_id(4);
        let savanna = get_layered_biome_by_id(35);
        let snowy_taiga = get_layered_biome_by_id(30);
        let desert = get_layered_biome_by_id(2);
        let jungle = get_layered_biome_by_id(21);
        let ocean = get_layered_biome_by_id(0);
        let badlands = get_layered_biome_by_id(37);

        for biome in [plains, forest, savanna, snowy_taiga] {
            assert!(biome_has_farm_animal_spawns(biome), "{}", biome.key());
            assert_eq!(farm_animal_spawns_for_biome(biome), &FARM_ANIMAL_SPAWNS);
        }

        for biome in [desert, jungle, ocean, badlands] {
            assert!(!biome_has_farm_animal_spawns(biome), "{}", biome.key());
            assert!(farm_animal_spawns_for_biome(biome).is_empty());
        }
    }
}
