use mclone_protocol::EntityKind;

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

#[cfg(test)]
mod tests {
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
}
