#![allow(dead_code)]

use mclone_protocol::EntityKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntityCategory {
    Creature,
    Misc,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EntityDimensions {
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl EntityDimensions {
    pub(crate) const fn scalable(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum StandingEyeHeight {
    Fixed(f32),
    HeightScale(f32),
}

impl StandingEyeHeight {
    pub(crate) fn resolve(self, dimensions: EntityDimensions) -> f32 {
        match self {
            Self::Fixed(height) => height,
            Self::HeightScale(scale) => dimensions.height * scale,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EntityMetadata {
    pub(crate) kind: EntityKind,
    pub(crate) category: EntityCategory,
    pub(crate) dimensions: EntityDimensions,
    pub(crate) standing_eye_height: StandingEyeHeight,
    pub(crate) movement_speed: f64,
    pub(crate) client_tracking_range: u8,
}

/// Authored debug-showcase species, not the complete passive-mob catalogue.
pub(crate) const PASSIVE_MOB_KINDS: &[EntityKind] = &[EntityKind::Cow, EntityKind::Chicken];

impl EntityMetadata {
    pub(crate) const COW: Self = Self {
        kind: EntityKind::Cow,
        category: EntityCategory::Creature,
        dimensions: EntityDimensions::scalable(0.9, 1.4),
        standing_eye_height: StandingEyeHeight::Fixed(1.3),
        movement_speed: 0.2,
        client_tracking_range: 10,
    };

    pub(crate) const CHICKEN: Self = Self {
        kind: EntityKind::Chicken,
        category: EntityCategory::Creature,
        dimensions: EntityDimensions::scalable(0.4, 0.7),
        standing_eye_height: StandingEyeHeight::HeightScale(0.92),
        movement_speed: 0.25,
        client_tracking_range: 10,
    };

    pub(crate) const MALLARD: Self = Self {
        kind: EntityKind::Mallard,
        category: EntityCategory::Creature,
        dimensions: EntityDimensions::scalable(0.7, 0.75),
        standing_eye_height: StandingEyeHeight::HeightScale(0.82),
        movement_speed: 0.23,
        client_tracking_range: 10,
    };

    pub(crate) const MALLARD_NEST: Self = Self {
        kind: EntityKind::MallardNest,
        category: EntityCategory::Misc,
        dimensions: EntityDimensions::scalable(0.8, 0.32),
        standing_eye_height: StandingEyeHeight::HeightScale(0.5),
        movement_speed: 0.0,
        client_tracking_range: 10,
    };

    pub(crate) const DEER: Self = Self {
        kind: EntityKind::Deer,
        category: EntityCategory::Creature,
        dimensions: EntityDimensions::scalable(0.9, 1.85),
        standing_eye_height: StandingEyeHeight::Fixed(1.62),
        movement_speed: 0.28,
        client_tracking_range: 12,
    };

    pub(crate) const DEER_BED: Self = Self {
        kind: EntityKind::DeerBed,
        category: EntityCategory::Misc,
        dimensions: EntityDimensions::scalable(0.9, 0.08),
        standing_eye_height: StandingEyeHeight::HeightScale(0.5),
        movement_speed: 0.0,
        client_tracking_range: 10,
    };

    pub(crate) const MANNEQUIN: Self = Self {
        kind: EntityKind::Mannequin,
        category: EntityCategory::Creature,
        dimensions: EntityDimensions::scalable(0.6, 1.8),
        standing_eye_height: StandingEyeHeight::Fixed(1.62),
        movement_speed: 0.2,
        client_tracking_range: 10,
    };

    pub(crate) const ITEM: Self = Self {
        kind: EntityKind::Item,
        category: EntityCategory::Misc,
        dimensions: EntityDimensions::scalable(0.25, 0.25),
        standing_eye_height: StandingEyeHeight::HeightScale(0.5),
        movement_speed: 0.0,
        client_tracking_range: 6,
    };

    pub(crate) const fn for_kind(kind: EntityKind) -> Option<Self> {
        match kind {
            EntityKind::Cow => Some(Self::COW),
            EntityKind::Chicken => Some(Self::CHICKEN),
            EntityKind::Mallard => Some(Self::MALLARD),
            EntityKind::MallardNest => Some(Self::MALLARD_NEST),
            EntityKind::Deer => Some(Self::DEER),
            EntityKind::DeerBed => Some(Self::DEER_BED),
            EntityKind::Mannequin => Some(Self::MANNEQUIN),
            EntityKind::Item => Some(Self::ITEM),
            EntityKind::DebugCube => None,
        }
    }

    pub(crate) fn standing_eye_height(self) -> f32 {
        self.standing_eye_height.resolve(self.dimensions)
    }

    pub(crate) const fn is_passive_mob(self) -> bool {
        matches!(
            self.kind,
            EntityKind::Cow
                | EntityKind::Chicken
                | EntityKind::Mallard
                | EntityKind::Deer
                | EntityKind::Mannequin
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cow_metadata_matches_java_1_17_1_type_and_attributes() {
        let metadata = EntityMetadata::for_kind(EntityKind::Cow).expect("cow metadata");

        assert_eq!(metadata.kind, EntityKind::Cow);
        assert_eq!(metadata.category, EntityCategory::Creature);
        assert_eq!(metadata.dimensions, EntityDimensions::scalable(0.9, 1.4));
        assert_eq!(metadata.standing_eye_height(), 1.3);
        assert_eq!(metadata.movement_speed, 0.2);
        assert_eq!(metadata.client_tracking_range, 10);
    }

    #[test]
    fn chicken_metadata_matches_java_1_17_1_type_and_attributes() {
        let metadata = EntityMetadata::for_kind(EntityKind::Chicken).expect("chicken metadata");

        assert_eq!(metadata.kind, EntityKind::Chicken);
        assert_eq!(metadata.category, EntityCategory::Creature);
        assert_eq!(metadata.dimensions, EntityDimensions::scalable(0.4, 0.7));
        assert_eq!(metadata.standing_eye_height(), 0.7 * 0.92);
        assert_eq!(metadata.movement_speed, 0.25);
        assert_eq!(metadata.client_tracking_range, 10);
    }

    #[test]
    fn mallard_metadata_is_a_persistent_creature_without_joining_debug_showcase() {
        let metadata = EntityMetadata::for_kind(EntityKind::Mallard).expect("mallard metadata");

        assert_eq!(metadata.category, EntityCategory::Creature);
        assert!(metadata.is_passive_mob());
        assert_eq!(metadata.dimensions, EntityDimensions::scalable(0.7, 0.75));
        assert_eq!(metadata.standing_eye_height(), 0.75 * 0.82);
        assert_eq!(metadata.movement_speed, 0.23);
        assert!(!PASSIVE_MOB_KINDS.contains(&EntityKind::Mallard));
    }

    #[test]
    fn non_vanilla_debug_cube_has_no_passive_mob_metadata() {
        assert_eq!(EntityMetadata::for_kind(EntityKind::DebugCube), None);
    }

    #[test]
    fn mallard_metadata_is_a_small_original_passive_creature() {
        let metadata = EntityMetadata::for_kind(EntityKind::Mallard).expect("mallard metadata");

        assert_eq!(metadata.category, EntityCategory::Creature);
        assert_eq!(metadata.dimensions, EntityDimensions::scalable(0.7, 0.75));
        assert_eq!(metadata.standing_eye_height(), 0.75 * 0.82);
        assert_eq!(metadata.movement_speed, 0.23);
        assert!(metadata.is_passive_mob());
        assert!(!PASSIVE_MOB_KINDS.contains(&EntityKind::Mallard));
    }

    #[test]
    fn deer_metadata_is_an_ordinary_original_creature() {
        let metadata = EntityMetadata::for_kind(EntityKind::Deer).expect("deer metadata");

        assert_eq!(metadata.category, EntityCategory::Creature);
        assert_eq!(metadata.dimensions, EntityDimensions::scalable(0.9, 1.85));
        assert_eq!(metadata.standing_eye_height(), 1.62);
        assert_eq!(metadata.movement_speed, 0.28);
        assert!(metadata.is_passive_mob());
        assert!(!PASSIVE_MOB_KINDS.contains(&EntityKind::Deer));
    }

    #[test]
    fn mannequin_uses_player_dimensions_and_cow_movement_speed() {
        let metadata = EntityMetadata::for_kind(EntityKind::Mannequin).expect("mannequin");

        assert_eq!(metadata.dimensions, EntityDimensions::scalable(0.6, 1.8));
        assert_eq!(metadata.standing_eye_height(), 1.62);
        assert_eq!(metadata.movement_speed, 0.2);
        assert!(metadata.is_passive_mob());
        assert!(!PASSIVE_MOB_KINDS.contains(&EntityKind::Mannequin));
    }

    #[test]
    fn item_metadata_matches_java_1_17_1_type() {
        let metadata = EntityMetadata::for_kind(EntityKind::Item).expect("item metadata");

        assert_eq!(metadata.kind, EntityKind::Item);
        assert_eq!(metadata.category, EntityCategory::Misc);
        assert_eq!(metadata.dimensions, EntityDimensions::scalable(0.25, 0.25));
        assert_eq!(metadata.client_tracking_range, 6);
        assert!(!metadata.is_passive_mob());
    }
}
