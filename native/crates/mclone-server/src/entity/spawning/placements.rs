use mclone_protocol::EntityKind;

use super::biome_tables::VanillaSpawnEntity;

pub(crate) const FARM_ANIMAL_PLACEMENTS: [SpawnPlacement; 4] = [
    SpawnPlacement::animal(VanillaSpawnEntity::Sheep),
    SpawnPlacement::animal(VanillaSpawnEntity::Pig),
    SpawnPlacement::animal(VanillaSpawnEntity::Chicken),
    SpawnPlacement::animal(VanillaSpawnEntity::Cow),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpawnPlacementType {
    OnGround,
    InWater,
    NoRestrictions,
    InLava,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpawnHeightmapType {
    MotionBlocking,
    MotionBlockingNoLeaves,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpawnPredicateKind {
    Animal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpawnPlacement {
    pub(crate) entity: VanillaSpawnEntity,
    pub(crate) placement_type: SpawnPlacementType,
    pub(crate) heightmap: SpawnHeightmapType,
    pub(crate) predicate: SpawnPredicateKind,
}

impl SpawnPlacement {
    const fn animal(entity: VanillaSpawnEntity) -> Self {
        Self {
            entity,
            placement_type: SpawnPlacementType::OnGround,
            heightmap: SpawnHeightmapType::MotionBlockingNoLeaves,
            predicate: SpawnPredicateKind::Animal,
        }
    }
}

pub(crate) fn farm_animal_placement_for_kind(kind: EntityKind) -> Option<SpawnPlacement> {
    FARM_ANIMAL_PLACEMENTS
        .iter()
        .copied()
        .find(|placement| placement.entity.implemented_kind() == Some(kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cow_and_chicken_use_java_on_ground_animal_placement() {
        for kind in [EntityKind::Cow, EntityKind::Chicken] {
            let placement = farm_animal_placement_for_kind(kind).expect("placement");

            assert_eq!(placement.placement_type, SpawnPlacementType::OnGround);
            assert_eq!(
                placement.heightmap,
                SpawnHeightmapType::MotionBlockingNoLeaves
            );
            assert_eq!(placement.predicate, SpawnPredicateKind::Animal);
        }
    }

    #[test]
    fn non_farm_animals_have_no_scaffolded_placement_yet() {
        assert_eq!(farm_animal_placement_for_kind(EntityKind::Item), None);
    }
}
