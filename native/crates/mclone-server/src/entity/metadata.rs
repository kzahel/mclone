#![allow(dead_code)]

use mclone_protocol::EntityKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntityCategory {
    Creature,
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

    pub(crate) const fn for_kind(kind: EntityKind) -> Option<Self> {
        match kind {
            EntityKind::Cow => Some(Self::COW),
            EntityKind::Chicken => Some(Self::CHICKEN),
            EntityKind::DebugCube => None,
        }
    }

    pub(crate) fn standing_eye_height(self) -> f32 {
        self.standing_eye_height.resolve(self.dimensions)
    }

    pub(crate) const fn is_passive_mob(self) -> bool {
        matches!(self.category, EntityCategory::Creature)
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
    fn non_vanilla_debug_cube_has_no_passive_mob_metadata() {
        assert_eq!(EntityMetadata::for_kind(EntityKind::DebugCube), None);
    }
}
