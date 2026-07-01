use super::super::metadata::EntityMetadata;

pub(crate) const JAVA_DEFAULT_FOLLOW_RANGE_BLOCKS: f32 = 16.0;
pub(crate) const JAVA_DEFAULT_MAX_UP_STEP: f64 = 0.6;
pub(crate) const JAVA_DEFAULT_JUMP_POWER: f64 = 0.42;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MobAttributes {
    pub(crate) movement_speed: f64,
    pub(crate) follow_range: f32,
    pub(crate) max_up_step: f64,
    pub(crate) jump_power: f64,
}

impl MobAttributes {
    pub(crate) fn from_metadata(metadata: EntityMetadata) -> Self {
        Self {
            movement_speed: metadata.movement_speed,
            follow_range: JAVA_DEFAULT_FOLLOW_RANGE_BLOCKS,
            max_up_step: JAVA_DEFAULT_MAX_UP_STEP,
            jump_power: JAVA_DEFAULT_JUMP_POWER,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_protocol::EntityKind;

    #[test]
    fn cow_attributes_match_java_defaults_and_type_speed() {
        let metadata = EntityMetadata::for_kind(EntityKind::Cow).unwrap();
        let attributes = MobAttributes::from_metadata(metadata);

        assert_eq!(attributes.movement_speed, 0.2);
        assert_eq!(attributes.follow_range, 16.0);
        assert_eq!(attributes.max_up_step, 0.6);
        assert_eq!(attributes.jump_power, 0.42);
    }

    #[test]
    fn chicken_attributes_match_java_defaults_and_type_speed() {
        let metadata = EntityMetadata::for_kind(EntityKind::Chicken).unwrap();
        let attributes = MobAttributes::from_metadata(metadata);

        assert_eq!(attributes.movement_speed, 0.25);
        assert_eq!(attributes.follow_range, 16.0);
        assert_eq!(attributes.max_up_step, 0.6);
        assert_eq!(attributes.jump_power, 0.42);
    }
}
