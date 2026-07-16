//! Ticket distance management.
//!
//! Move-only home for `ChunkDistanceManager`, mirroring Java's
//! `server/level/DistanceManager`: per-chunk ticket storage, player-view ticket
//! reconciliation, timeout purging, and ticket-level/active-level propagation.
//! Ticket data types (`ChunkTicket`/`ChunkTicketType`/`ChunkTicketKey`) live in
//! `crate::types`; the scheduler drives this through `pub(crate)` methods.

use std::collections::{BTreeMap, BTreeSet};

use mclone_core::ChunkPos;

use crate::{
    CHUNK_LEVEL_FULL, ChunkTicket, ChunkTicketKey, ChunkTicketType, MAX_CHUNK_DISTANCE,
    PLAYER_TICKET_LEVEL, UNLOADED_CHUNK_LEVEL,
};

#[derive(Debug)]
pub(crate) struct ChunkDistanceManager {
    tickets: BTreeMap<ChunkPos, BTreeSet<ChunkTicket>>,
    aggregate_resident_positions: BTreeSet<ChunkPos>,
    aggregate_simulation_ticket_positions: BTreeSet<ChunkPos>,
    aggregate_interest_priority_centers: Vec<ChunkPos>,
    ticket_tick: u64,
}

impl ChunkDistanceManager {
    pub(crate) fn new() -> Self {
        Self {
            tickets: BTreeMap::new(),
            aggregate_resident_positions: BTreeSet::new(),
            aggregate_simulation_ticket_positions: BTreeSet::new(),
            aggregate_interest_priority_centers: Vec::new(),
            ticket_tick: 0,
        }
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
    }

    pub(crate) fn remove_ticket(
        &mut self,
        ticket_type: ChunkTicketType,
        pos: ChunkPos,
        level: i32,
        key: ChunkTicketKey,
    ) {
        let ticket = ChunkTicket::new(ticket_type, level, key);
        if let Some(tickets) = self.tickets.get_mut(&pos) {
            tickets.remove(&ticket);
            if tickets.is_empty() {
                self.tickets.remove(&pos);
            }
        }
    }

    pub(crate) fn purge_stale_tickets(&mut self) {
        self.ticket_tick += 1;
        let ticket_tick = self.ticket_tick;
        let mut empty_chunks = Vec::new();

        for (pos, tickets) in &mut self.tickets {
            tickets.retain(|ticket| !ticket.timed_out(ticket_tick));
            if tickets.is_empty() {
                empty_chunks.push(*pos);
            }
        }

        for pos in empty_chunks {
            self.tickets.remove(&pos);
        }
    }

    pub(crate) fn active_levels(&self) -> BTreeMap<ChunkPos, i32> {
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

                    let pos = ChunkPos::new(source_pos.x + dx, source_pos.z + dz);
                    levels
                        .entry(pos)
                        .and_modify(|current| *current = (*current).min(level))
                        .or_insert(level);
                }
            }
        }

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
}
