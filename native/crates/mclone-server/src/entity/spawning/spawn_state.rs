use super::mob_category::{MobCategory, NATURAL_SPAWN_MAGIC_NUMBER};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MobCategoryCounts {
    counts: [u32; MobCategory::COUNT],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CategorySpawnState {
    pub(crate) category: MobCategory,
    pub(crate) current_count: u32,
    pub(crate) cap: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpawnState {
    spawnable_chunk_count: u32,
    counts: MobCategoryCounts,
}

impl MobCategoryCounts {
    pub(crate) const fn new() -> Self {
        Self {
            counts: [0; MobCategory::COUNT],
        }
    }

    pub(crate) fn with_count(mut self, category: MobCategory, count: u32) -> Self {
        self.set(category, count);
        self
    }

    pub(crate) fn get(self, category: MobCategory) -> u32 {
        self.counts[category.index()]
    }

    pub(crate) fn set(&mut self, category: MobCategory, count: u32) {
        self.counts[category.index()] = count;
    }

    pub(crate) fn increment(&mut self, category: MobCategory) {
        self.counts[category.index()] = self.counts[category.index()].saturating_add(1);
    }
}

impl CategorySpawnState {
    pub(crate) fn new(
        category: MobCategory,
        spawnable_chunk_count: u32,
        current_count: u32,
    ) -> Self {
        Self {
            category,
            current_count,
            cap: natural_spawn_cap(category, spawnable_chunk_count),
        }
    }

    pub(crate) const fn has_room(self) -> bool {
        self.current_count < self.cap
    }
}

impl SpawnState {
    pub(crate) const fn new(spawnable_chunk_count: u32, counts: MobCategoryCounts) -> Self {
        Self {
            spawnable_chunk_count,
            counts,
        }
    }

    pub(crate) const fn spawnable_chunk_count(self) -> u32 {
        self.spawnable_chunk_count
    }

    pub(crate) const fn counts(self) -> MobCategoryCounts {
        self.counts
    }

    pub(crate) fn category_state(self, category: MobCategory) -> CategorySpawnState {
        CategorySpawnState::new(
            category,
            self.spawnable_chunk_count,
            self.counts.get(category),
        )
    }

    pub(crate) fn can_spawn_for_category(self, category: MobCategory) -> bool {
        self.category_state(category).has_room()
    }
}

pub(crate) fn natural_spawn_cap(category: MobCategory, spawnable_chunk_count: u32) -> u32 {
    u32::try_from(category.max_instances_per_chunk())
        .ok()
        .map_or(0, |max| {
            max.saturating_mul(spawnable_chunk_count) / NATURAL_SPAWN_MAGIC_NUMBER
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mob_cap_formula_matches_java_spawn_state() {
        assert_eq!(NATURAL_SPAWN_MAGIC_NUMBER, 289);
        assert_eq!(natural_spawn_cap(MobCategory::Creature, 0), 0);
        assert_eq!(natural_spawn_cap(MobCategory::Creature, 1), 0);
        assert_eq!(natural_spawn_cap(MobCategory::Creature, 144), 4);
        assert_eq!(natural_spawn_cap(MobCategory::Creature, 289), 10);
        assert_eq!(natural_spawn_cap(MobCategory::Creature, 578), 20);
    }

    #[test]
    fn misc_category_has_no_natural_spawn_cap() {
        assert_eq!(natural_spawn_cap(MobCategory::Misc, 289), 0);
        assert!(!MobCategory::Misc.is_natural_spawning());
    }

    #[test]
    fn spawn_state_allows_category_only_below_scaled_cap() {
        let below_cap = SpawnState::new(
            289,
            MobCategoryCounts::new().with_count(MobCategory::Creature, 9),
        );
        let at_cap = SpawnState::new(
            289,
            MobCategoryCounts::new().with_count(MobCategory::Creature, 10),
        );

        assert!(below_cap.can_spawn_for_category(MobCategory::Creature));
        assert!(!at_cap.can_spawn_for_category(MobCategory::Creature));
    }

    #[test]
    fn category_counts_track_java_mob_category_buckets() {
        let mut counts = MobCategoryCounts::new();

        counts.increment(MobCategory::Creature);
        counts.increment(MobCategory::Creature);
        counts.set(MobCategory::Monster, 70);

        assert_eq!(counts.get(MobCategory::Creature), 2);
        assert_eq!(counts.get(MobCategory::Monster), 70);
        assert_eq!(counts.get(MobCategory::Ambient), 0);
    }
}
