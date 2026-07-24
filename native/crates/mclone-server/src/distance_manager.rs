//! Ticket distance management.
//!
//! Move-only home for `ChunkDistanceManager`, mirroring Java's
//! `server/level/DistanceManager`: per-chunk ticket storage, player-view ticket
//! reconciliation, timeout purging, and ticket-level/active-level propagation.
//! Ticket data types (`ChunkTicket`/`ChunkTicketType`/`ChunkTicketKey`) live in
//! `crate::types`; the scheduler drives this through `pub(crate)` methods.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use mclone_core::{ChunkPos, HorizontalTopology};

use crate::{
    CHUNK_LEVEL_FULL, ChunkTicket, ChunkTicketKey, ChunkTicketType, MAX_CHUNK_DISTANCE,
    PLAYER_TICKET_LEVEL, UNLOADED_CHUNK_LEVEL,
};

#[derive(Debug)]
pub(crate) struct ChunkDistanceManager {
    topology: HorizontalTopology,
    tickets: BTreeMap<ChunkPos, BTreeSet<ChunkTicket>>,
    aggregate_resident_positions: BTreeSet<ChunkPos>,
    aggregate_simulation_ticket_positions: BTreeSet<ChunkPos>,
    aggregate_interest_priority_centers: Vec<ChunkPos>,
    ticket_tick: u64,
    ticket_generation: u64,
    active_levels_cache: RefCell<Option<(u64, Arc<BTreeMap<ChunkPos, i32>>)>>,
}

impl ChunkDistanceManager {
    pub(crate) fn new() -> Self {
        Self {
            topology: HorizontalTopology::UNBOUNDED,
            tickets: BTreeMap::new(),
            aggregate_resident_positions: BTreeSet::new(),
            aggregate_simulation_ticket_positions: BTreeSet::new(),
            aggregate_interest_priority_centers: Vec::new(),
            ticket_tick: 0,
            ticket_generation: 0,
            active_levels_cache: RefCell::new(None),
        }
    }

    pub(crate) fn set_topology(&mut self, topology: HorizontalTopology) {
        debug_assert!(self.tickets.is_empty());
        if self.topology != topology {
            self.mark_tickets_changed();
        }
        self.topology = topology;
    }

    pub(crate) fn ticket_tick(&self) -> u64 {
        self.ticket_tick
    }

    pub(crate) fn set_aggregate_interest_positions_with_priority(
        &mut self,
        new_resident_positions: BTreeSet<ChunkPos>,
        new_simulation_positions: BTreeSet<ChunkPos>,
        priority_centers: Vec<ChunkPos>,
    ) {
        debug_assert!(new_simulation_positions.is_subset(&new_resident_positions));
        let old_resident_positions = std::mem::take(&mut self.aggregate_resident_positions);
        let old_simulation_positions =
            std::mem::take(&mut self.aggregate_simulation_ticket_positions);
        let old_residency_only = old_resident_positions
            .difference(&old_simulation_positions)
            .copied()
            .collect::<BTreeSet<_>>();
        let new_residency_only = new_resident_positions
            .difference(&new_simulation_positions)
            .copied()
            .collect::<BTreeSet<_>>();

        for pos in old_simulation_positions
            .difference(&new_simulation_positions)
            .copied()
        {
            self.remove_ticket(
                ChunkTicketType::Player,
                pos,
                PLAYER_TICKET_LEVEL,
                ChunkTicketKey::Chunk(pos),
            );
        }

        for pos in old_residency_only.difference(&new_residency_only).copied() {
            self.remove_ticket(
                ChunkTicketType::Observer,
                pos,
                CHUNK_LEVEL_FULL,
                ChunkTicketKey::Chunk(pos),
            );
        }

        for pos in new_simulation_positions
            .difference(&old_simulation_positions)
            .copied()
        {
            self.add_ticket(
                ChunkTicketType::Player,
                pos,
                PLAYER_TICKET_LEVEL,
                ChunkTicketKey::Chunk(pos),
            );
        }

        for pos in new_residency_only.difference(&old_residency_only).copied() {
            self.add_ticket(
                ChunkTicketType::Observer,
                pos,
                CHUNK_LEVEL_FULL,
                ChunkTicketKey::Chunk(pos),
            );
        }

        self.aggregate_resident_positions = new_resident_positions;
        self.aggregate_simulation_ticket_positions = new_simulation_positions;
        self.aggregate_interest_priority_centers = priority_centers
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }

    pub(crate) fn add_region_ticket(
        &mut self,
        ticket_type: ChunkTicketType,
        pos: ChunkPos,
        distance: i32,
        key: ChunkTicketKey,
    ) {
        self.add_ticket(ticket_type, pos, CHUNK_LEVEL_FULL - distance, key);
    }

    pub(crate) fn remove_region_ticket(
        &mut self,
        ticket_type: ChunkTicketType,
        pos: ChunkPos,
        distance: i32,
        key: ChunkTicketKey,
    ) {
        self.remove_ticket(ticket_type, pos, CHUNK_LEVEL_FULL - distance, key);
    }

    pub(crate) fn add_ticket(
        &mut self,
        ticket_type: ChunkTicketType,
        pos: ChunkPos,
        level: i32,
        key: ChunkTicketKey,
    ) {
        let mut ticket = ChunkTicket::new(ticket_type, level, key);
        ticket.created_tick = self.ticket_tick;
        let tickets = self.tickets.entry(pos).or_default();
        tickets.replace(ticket);
        self.mark_tickets_changed();
    }

    pub(crate) fn remove_ticket(
        &mut self,
        ticket_type: ChunkTicketType,
        pos: ChunkPos,
        level: i32,
        key: ChunkTicketKey,
    ) {
        let ticket = ChunkTicket::new(ticket_type, level, key);
        let mut changed = false;
        if let Some(tickets) = self.tickets.get_mut(&pos) {
            changed = tickets.remove(&ticket);
            if tickets.is_empty() {
                self.tickets.remove(&pos);
            }
        }
        if changed {
            self.mark_tickets_changed();
        }
    }

    pub(crate) fn purge_stale_tickets(&mut self) {
        self.ticket_tick += 1;
        let ticket_tick = self.ticket_tick;
        let mut empty_chunks = Vec::new();
        let mut changed = false;

        for (pos, tickets) in &mut self.tickets {
            let before = tickets.len();
            tickets.retain(|ticket| !ticket.timed_out(ticket_tick));
            changed |= tickets.len() != before;
            if tickets.is_empty() {
                empty_chunks.push(*pos);
            }
        }

        for pos in empty_chunks {
            self.tickets.remove(&pos);
        }
        if changed {
            self.mark_tickets_changed();
        }
    }

    pub(crate) fn active_levels(&self) -> Arc<BTreeMap<ChunkPos, i32>> {
        if let Some((generation, levels)) = self.active_levels_cache.borrow().as_ref()
            && *generation == self.ticket_generation
        {
            return Arc::clone(levels);
        }

        let mut levels: BTreeMap<ChunkPos, i32> = BTreeMap::new();

        for (source_pos, tickets) in &self.tickets {
            let Some(ticket) = tickets.first() else {
                continue;
            };
            if ticket.level > MAX_CHUNK_DISTANCE {
                continue;
            }

            let radius = MAX_CHUNK_DISTANCE - ticket.level;
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let distance = dx.abs().max(dz.abs());
                    let level = ticket.level + distance;
                    if level > MAX_CHUNK_DISTANCE {
                        continue;
                    }

                    let Some(pos) = self.topology.neighbor_chunk(*source_pos, dx, dz) else {
                        continue;
                    };
                    levels
                        .entry(pos)
                        .and_modify(|current| *current = (*current).min(level))
                        .or_insert(level);
                }
            }
        }

        let levels = Arc::new(levels);
        *self.active_levels_cache.borrow_mut() =
            Some((self.ticket_generation, Arc::clone(&levels)));
        levels
    }

    pub(crate) fn player_interest_positions(&self) -> BTreeSet<ChunkPos> {
        self.aggregate_resident_positions.clone()
    }

    pub(crate) fn player_interest_priority_centers(&self) -> &[ChunkPos] {
        &self.aggregate_interest_priority_centers
    }

    pub(crate) fn ticketed_chunk_count(&self) -> usize {
        self.tickets
            .values()
            .filter(|tickets| {
                tickets
                    .first()
                    .is_some_and(|ticket| ticket.level <= MAX_CHUNK_DISTANCE)
            })
            .count()
    }

    pub(crate) fn ticket_count_at(&self, pos: ChunkPos) -> usize {
        self.tickets.get(&pos).map_or(0, BTreeSet::len)
    }

    pub(crate) fn ticket_level_at(&self, pos: ChunkPos) -> i32 {
        self.tickets
            .get(&pos)
            .and_then(|tickets| tickets.first())
            .map_or(UNLOADED_CHUNK_LEVEL, |ticket| ticket.level)
    }

    pub(crate) fn active_level_at(&self, pos: ChunkPos) -> i32 {
        self.active_levels()
            .get(&pos)
            .copied()
            .unwrap_or(UNLOADED_CHUNK_LEVEL)
    }

    fn mark_tickets_changed(&mut self) {
        self.ticket_generation = self.ticket_generation.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_levels_reuse_one_map_until_ticket_state_changes() {
        let mut manager = ChunkDistanceManager::new();
        let center = ChunkPos::new(0, 0);
        manager.add_ticket(
            ChunkTicketType::Forced,
            center,
            MAX_CHUNK_DISTANCE,
            ChunkTicketKey::Chunk(center),
        );

        let first = manager.active_levels();
        let second = manager.active_levels();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first.get(&center), Some(&MAX_CHUNK_DISTANCE));

        let east = ChunkPos::new(1, 0);
        manager.add_ticket(
            ChunkTicketType::Forced,
            east,
            MAX_CHUNK_DISTANCE,
            ChunkTicketKey::Chunk(east),
        );
        let after_add = manager.active_levels();
        assert!(!Arc::ptr_eq(&first, &after_add));
        assert_eq!(after_add.get(&east), Some(&MAX_CHUNK_DISTANCE));

        manager.remove_ticket(
            ChunkTicketType::Forced,
            east,
            MAX_CHUNK_DISTANCE,
            ChunkTicketKey::Chunk(east),
        );
        let after_remove = manager.active_levels();
        assert!(!Arc::ptr_eq(&after_add, &after_remove));
        assert!(!after_remove.contains_key(&east));
    }

    #[test]
    fn timed_out_ticket_invalidates_cached_active_levels() {
        let mut manager = ChunkDistanceManager::new();
        let center = ChunkPos::new(0, 0);
        manager.add_ticket(
            ChunkTicketType::Unknown,
            center,
            MAX_CHUNK_DISTANCE,
            ChunkTicketKey::Chunk(center),
        );
        let before = manager.active_levels();

        manager.purge_stale_tickets();
        assert!(Arc::ptr_eq(&before, &manager.active_levels()));
        manager.purge_stale_tickets();

        let after = manager.active_levels();
        assert!(!Arc::ptr_eq(&before, &after));
        assert!(after.is_empty());
    }
}
