//! Small, server-internal primitives shared by materialized wildlife.
//!
//! These types deliberately stop short of a generic animal framework. They
//! encode facts that every ecology policy must handle consistently: bounded
//! place knowledge, explicit world availability, stale-safe attempts, and
//! deterministic work admission.

use mclone_core::BlockPos;
use mclone_protocol::EntityPersistentId;

pub(crate) const MAX_KNOWN_PLACES: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorldFactLocator {
    pub(crate) persistent_id: EntityPersistentId,
    pub(crate) last_known_position: Option<BlockPos>,
    pub(crate) revision: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct KnownPlace {
    pub(crate) locator: WorldFactLocator,
    pub(crate) last_confirmed_tick: u64,
    pub(crate) familiarity: u8,
}

impl KnownPlace {
    pub(crate) const fn unresolved(persistent_id: EntityPersistentId) -> Self {
        Self {
            locator: WorldFactLocator {
                persistent_id,
                last_known_position: None,
                revision: None,
            },
            last_confirmed_tick: 0,
            familiarity: 1,
        }
    }

    pub(crate) const fn observed(
        persistent_id: EntityPersistentId,
        position: BlockPos,
        observed_tick: u64,
    ) -> Self {
        Self {
            locator: WorldFactLocator {
                persistent_id,
                last_known_position: Some(position),
                revision: None,
            },
            last_confirmed_tick: observed_tick,
            familiarity: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Availability<T> {
    Available(T),
    CurrentlyUnavailable,
    ConfirmedUnsuitable,
    #[allow(dead_code)]
    ConfirmedGone,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DecisionSchedule {
    pub(crate) next_due_tick: u64,
    pub(crate) attempt_generation: u32,
}

impl DecisionSchedule {
    pub(crate) const fn new(next_due_tick: u64, attempt_generation: u32) -> Self {
        Self {
            next_due_tick,
            attempt_generation,
        }
    }

    pub(crate) const fn is_due(self, now: u64) -> bool {
        now >= self.next_due_tick
    }

    pub(crate) fn complete(&mut self, now: u64, interval: u64) -> u32 {
        self.attempt_generation = self.attempt_generation.wrapping_add(1);
        self.next_due_tick = now.saturating_add(interval.max(1));
        self.attempt_generation
    }

    pub(crate) fn wake(&mut self, now: u64) {
        self.next_due_tick = self.next_due_tick.min(now);
        self.attempt_generation = self.attempt_generation.wrapping_add(1);
    }

    pub(crate) const fn accepts(self, generation: u32) -> bool {
        self.attempt_generation == generation
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum EcologyWorkClass {
    Decision,
    HabitatQuery,
    PathRequest,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct EcologyWorkDiagnostics {
    pub(crate) admitted: [u32; 3],
    pub(crate) deferred: [u32; 3],
    pub(crate) oldest_debt_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EcologyWorkBudget {
    remaining: [u32; 3],
    pub(crate) diagnostics: EcologyWorkDiagnostics,
}

impl EcologyWorkBudget {
    pub(crate) const fn new(decisions: u32, habitat_queries: u32, paths: u32) -> Self {
        Self {
            remaining: [decisions, habitat_queries, paths],
            diagnostics: EcologyWorkDiagnostics {
                admitted: [0; 3],
                deferred: [0; 3],
                oldest_debt_ticks: 0,
            },
        }
    }

    pub(crate) fn admit(&mut self, class: EcologyWorkClass, debt_ticks: u64) -> bool {
        let index = class as usize;
        if self.remaining[index] == 0 {
            self.diagnostics.deferred[index] = self.diagnostics.deferred[index].saturating_add(1);
            self.diagnostics.oldest_debt_ticks = self.diagnostics.oldest_debt_ticks.max(debt_ticks);
            return false;
        }
        self.remaining[index] -= 1;
        self.diagnostics.admitted[index] = self.diagnostics.admitted[index].saturating_add(1);
        true
    }
}

pub(crate) fn remember_known_place(
    places: &mut [Option<KnownPlace>; MAX_KNOWN_PLACES],
    observation: KnownPlace,
) {
    if let Some(existing) = places
        .iter_mut()
        .flatten()
        .find(|known| known.locator.persistent_id == observation.locator.persistent_id)
    {
        existing.locator = observation.locator;
        existing.last_confirmed_tick = existing
            .last_confirmed_tick
            .max(observation.last_confirmed_tick);
        existing.familiarity = existing.familiarity.saturating_add(1);
        return;
    }
    if let Some(empty) = places.iter_mut().find(|place| place.is_none()) {
        *empty = Some(observation);
        return;
    }

    let replace = places
        .iter()
        .enumerate()
        .min_by_key(|(_, place)| {
            let place = place.expect("full known-place list");
            (
                place.familiarity,
                place.last_confirmed_tick,
                place.locator.persistent_id,
            )
        })
        .map(|(index, _)| index)
        .expect("bounded known-place list is nonempty");
    places[replace] = Some(observation);
}

pub(crate) fn invalidate_known_place(
    places: &mut [Option<KnownPlace>; MAX_KNOWN_PLACES],
    persistent_id: EntityPersistentId,
) -> bool {
    let Some(index) = places
        .iter()
        .position(|place| place.is_some_and(|known| known.locator.persistent_id == persistent_id))
    else {
        return false;
    };
    places[index] = None;
    places.sort_by_key(Option::is_none);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u64) -> EntityPersistentId {
        EntityPersistentId::new(7, value)
    }

    #[test]
    fn bounded_memory_confirms_and_replaces_deterministically() {
        let mut places = [None; MAX_KNOWN_PLACES];
        remember_known_place(
            &mut places,
            KnownPlace::observed(id(3), BlockPos::new(3, 4, 5), 8),
        );
        remember_known_place(&mut places, KnownPlace::unresolved(id(1)));
        remember_known_place(&mut places, KnownPlace::unresolved(id(2)));
        remember_known_place(
            &mut places,
            KnownPlace::observed(id(3), BlockPos::new(6, 7, 8), 12),
        );
        assert_eq!(places[0].unwrap().familiarity, 2);
        assert_eq!(
            places[0].unwrap().locator.last_known_position,
            Some(BlockPos::new(6, 7, 8))
        );

        remember_known_place(&mut places, KnownPlace::unresolved(id(4)));
        assert!(
            places
                .iter()
                .flatten()
                .any(|place| place.locator.persistent_id == id(3))
        );
        assert!(
            !places
                .iter()
                .flatten()
                .any(|place| place.locator.persistent_id == id(1))
        );
        assert!(invalidate_known_place(&mut places, id(3)));
        assert!(!invalidate_known_place(&mut places, id(3)));
    }

    #[test]
    fn availability_outcomes_never_alias() {
        let values = [
            Availability::Available(1_u8),
            Availability::CurrentlyUnavailable,
            Availability::ConfirmedUnsuitable,
            Availability::ConfirmedGone,
        ];
        for (left_index, left) in values.iter().enumerate() {
            for (right_index, right) in values.iter().enumerate() {
                assert_eq!(left == right, left_index == right_index);
            }
        }
    }

    #[test]
    fn work_budget_defers_without_consuming_or_catching_up() {
        let mut budget = EcologyWorkBudget::new(1, 0, 1);
        assert!(budget.admit(EcologyWorkClass::Decision, 0));
        assert!(!budget.admit(EcologyWorkClass::Decision, 9));
        assert!(!budget.admit(EcologyWorkClass::HabitatQuery, 3));
        assert!(budget.admit(EcologyWorkClass::PathRequest, 0));
        assert_eq!(budget.diagnostics.admitted, [1, 0, 1]);
        assert_eq!(budget.diagnostics.deferred, [1, 1, 0]);
        assert_eq!(budget.diagnostics.oldest_debt_ticks, 9);
    }

    #[test]
    fn decision_generation_rejects_stale_results() {
        let mut schedule = DecisionSchedule::new(10, 4);
        assert!(!schedule.is_due(9));
        let current = schedule.complete(10, 5);
        assert_eq!(current, 5);
        assert!(schedule.accepts(5));
        schedule.wake(11);
        assert!(!schedule.accepts(5));
        assert!(schedule.is_due(11));
    }
}
