use std::collections::BTreeMap;

use mclone_protocol::DimensionKey;

use crate::WorldGenerationProfile;
use crate::persistence::{DimensionDefinition, DimensionRecord};

/// Realm-owned registry of durable dimension definitions.
///
/// Slice 2 deliberately keeps the live scheduler fields on `RealmServer`.
/// The registry supplies identity and definition ownership without adding a
/// dimension key to hot in-dimension `ChunkPos` APIs.
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionRegistry {
    definitions: BTreeMap<DimensionKey, DimensionDefinition>,
}

impl DimensionRegistry {
    pub fn single_overworld(seed: i64, generation_profile: WorldGenerationProfile) -> Self {
        let record = DimensionRecord::overworld(seed, generation_profile);
        Self {
            definitions: BTreeMap::from([(record.key, record.definition)]),
        }
    }

    pub fn get(&self, key: &DimensionKey) -> Option<&DimensionDefinition> {
        self.definitions.get(key)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&DimensionKey, &DimensionDefinition)> {
        self.definitions.iter()
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub(crate) fn insert(
        &mut self,
        key: DimensionKey,
        definition: DimensionDefinition,
    ) -> Result<bool, String> {
        if let Some(existing) = self.definitions.get(&key) {
            return if existing == &definition {
                Ok(false)
            } else {
                Err(format!(
                    "dimension {key} is already registered with a different definition"
                ))
            };
        }
        self.definitions.insert(key, definition);
        Ok(true)
    }

    pub(crate) fn set_overworld_generation_profile(
        &mut self,
        generation_profile: WorldGenerationProfile,
    ) {
        self.definitions
            .get_mut(&DimensionKey::overworld())
            .expect("single-dimension realm must retain its Overworld definition")
            .generation_profile = generation_profile;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_registry_contains_exactly_one_overworld_definition() {
        let registry =
            DimensionRegistry::single_overworld(12_345, WorldGenerationProfile::authored_only());

        assert_eq!(registry.len(), 1);
        let definition = registry.get(&DimensionKey::overworld()).unwrap();
        assert_eq!(definition.seed, 12_345);
        assert_eq!(
            definition.generation_profile,
            WorldGenerationProfile::authored_only()
        );
        assert_eq!(definition.min_y, 0);
        assert_eq!(definition.height, 256);
        assert_eq!(definition.coordinate_scale, 1.0);
        assert!(definition.has_sky_light);
        assert!(!definition.has_ceiling);
        assert!(!definition.ultrawarm);
    }
}
