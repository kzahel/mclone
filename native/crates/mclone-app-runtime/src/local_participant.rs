use std::{collections::BTreeMap, num::NonZeroU64};

use mclone_input::{LocalParticipantSlot, MAX_LOCAL_PARTICIPANTS};

/// Opaque identity for one participant admitted during a local host session.
///
/// It is intentionally not serializable and cannot be constructed from an
/// input-source, profile, server-player, or presentation-view identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalParticipantId(NonZeroU64);

#[derive(Clone, Debug)]
pub struct LocalParticipantIdAllocator {
    next: Option<NonZeroU64>,
}

impl Default for LocalParticipantIdAllocator {
    fn default() -> Self {
        Self {
            next: NonZeroU64::new(1),
        }
    }
}

impl LocalParticipantIdAllocator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `None` only after exhausting every nonzero `u64` in one session.
    pub fn allocate(&mut self) -> Option<LocalParticipantId> {
        let id = self.next?;
        self.next = id.get().checked_add(1).and_then(NonZeroU64::new);
        Some(LocalParticipantId(id))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalParticipantAdmission {
    pub id: LocalParticipantId,
    pub slot: LocalParticipantSlot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalParticipantGroupError {
    Full,
    SlotOccupied(LocalParticipantSlot),
    IdSpaceExhausted,
}

#[derive(Clone, Debug)]
struct LocalParticipantEntry<T> {
    slot: LocalParticipantSlot,
    value: T,
}

/// Bounded session-local participant collection with stable identity and slot
/// assignment. Removal never renumbers the remaining participants.
#[derive(Clone, Debug)]
pub struct LocalParticipantGroup<T> {
    ids: LocalParticipantIdAllocator,
    entries: BTreeMap<LocalParticipantId, LocalParticipantEntry<T>>,
    slots: [Option<LocalParticipantId>; MAX_LOCAL_PARTICIPANTS],
}

impl<T> Default for LocalParticipantGroup<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LocalParticipantGroup<T> {
    pub fn new() -> Self {
        Self {
            ids: LocalParticipantIdAllocator::new(),
            entries: BTreeMap::new(),
            slots: [None; MAX_LOCAL_PARTICIPANTS],
        }
    }

    pub fn admit_next(
        &mut self,
        value: T,
    ) -> Result<LocalParticipantAdmission, LocalParticipantGroupError> {
        let slot = self
            .next_available_slot()
            .ok_or(LocalParticipantGroupError::Full)?;
        self.admit_at(slot, value)
    }

    pub fn admit_at(
        &mut self,
        slot: LocalParticipantSlot,
        value: T,
    ) -> Result<LocalParticipantAdmission, LocalParticipantGroupError> {
        if self.slots[slot.index()].is_some() {
            return Err(LocalParticipantGroupError::SlotOccupied(slot));
        }
        let id = self
            .ids
            .allocate()
            .ok_or(LocalParticipantGroupError::IdSpaceExhausted)?;
        self.entries
            .insert(id, LocalParticipantEntry { slot, value });
        self.slots[slot.index()] = Some(id);
        Ok(LocalParticipantAdmission { id, slot })
    }

    pub fn remove(&mut self, id: LocalParticipantId) -> Option<T> {
        let entry = self.entries.remove(&id)?;
        self.slots[entry.slot.index()] = None;
        Some(entry.value)
    }

    pub fn get(&self, id: LocalParticipantId) -> Option<&T> {
        self.entries.get(&id).map(|entry| &entry.value)
    }

    pub fn get_mut(&mut self, id: LocalParticipantId) -> Option<&mut T> {
        self.entries.get_mut(&id).map(|entry| &mut entry.value)
    }

    pub fn get_at(&self, slot: LocalParticipantSlot) -> Option<&T> {
        self.id_at(slot).and_then(|id| self.get(id))
    }

    pub fn get_at_mut(&mut self, slot: LocalParticipantSlot) -> Option<&mut T> {
        self.id_at(slot).and_then(|id| self.get_mut(id))
    }

    pub fn id_at(&self, slot: LocalParticipantSlot) -> Option<LocalParticipantId> {
        self.slots[slot.index()]
    }

    pub fn slot_of(&self, id: LocalParticipantId) -> Option<LocalParticipantSlot> {
        self.entries.get(&id).map(|entry| entry.slot)
    }

    pub fn next_available_slot(&self) -> Option<LocalParticipantSlot> {
        self.slots
            .iter()
            .position(Option::is_none)
            .and_then(LocalParticipantSlot::from_index)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.len() == MAX_LOCAL_PARTICIPANTS
    }

    pub fn iter(&self) -> impl Iterator<Item = (LocalParticipantAdmission, &T)> {
        self.entries.iter().map(|(id, entry)| {
            (
                LocalParticipantAdmission {
                    id: *id,
                    slot: entry.slot,
                },
                &entry.value,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn participant_ids_and_slots_remain_stable_across_removal() {
        let mut group = LocalParticipantGroup::new();
        let first = group.admit_next("first").unwrap();
        let second = group.admit_next("second").unwrap();
        let third = group.admit_next("third").unwrap();

        assert_eq!(first.slot.index(), 0);
        assert_eq!(second.slot.index(), 1);
        assert_eq!(third.slot.index(), 2);
        assert_eq!(group.remove(second.id), Some("second"));
        assert_eq!(group.slot_of(first.id), Some(first.slot));
        assert_eq!(group.slot_of(third.id), Some(third.slot));
        assert_eq!(group.id_at(second.slot), None);

        let replacement = group.admit_next("replacement").unwrap();
        assert_eq!(replacement.slot, second.slot);
        assert_ne!(replacement.id, second.id);
        assert_eq!(group.slot_of(third.id), Some(third.slot));
    }

    #[test]
    fn participant_group_admits_exactly_four_without_pair_model() {
        let mut group = LocalParticipantGroup::new();
        let admissions = (0..MAX_LOCAL_PARTICIPANTS)
            .map(|index| group.admit_next(index).unwrap())
            .collect::<Vec<_>>();

        assert!(group.is_full());
        assert_eq!(group.len(), MAX_LOCAL_PARTICIPANTS);
        assert_eq!(
            group.admit_next(MAX_LOCAL_PARTICIPANTS),
            Err(LocalParticipantGroupError::Full)
        );
        for (index, admission) in admissions.into_iter().enumerate() {
            assert_eq!(admission.slot.index(), index);
            assert_eq!(group.get(admission.id), Some(&index));
        }
    }

    #[test]
    fn explicit_slot_admission_rejects_duplicates() {
        let mut group = LocalParticipantGroup::new();
        let slot = LocalParticipantSlot::from_index(3).unwrap();
        let admission = group.admit_at(slot, 17).unwrap();
        assert_eq!(admission.slot, slot);
        assert_eq!(group.get_at(slot), Some(&17));
        assert_eq!(
            group.admit_at(slot, 18),
            Err(LocalParticipantGroupError::SlotOccupied(slot))
        );
    }
}
