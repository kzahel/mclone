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

pub(crate) const DEFAULT_MAX_ACTIVE_PLAYER_PROMOTIONS: usize = 4;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlayerPromotionDiagnostics {
    pub desired: usize,
    pub queued: usize,
    pub active: usize,
    pub max_active: usize,
    pub admitted_total: u64,
    pub cancelled_before_admission: u64,
    pub oldest_queued_age_ticks: u64,
}

#[derive(Debug)]
pub(crate) struct ChunkDistanceManager {
    topology: HorizontalTopology,
    tickets: BTreeMap<ChunkPos, BTreeSet<ChunkTicket>>,
    aggregate_resident_positions: BTreeSet<ChunkPos>,
    aggregate_simulation_positions: BTreeSet<ChunkPos>,
    player_ticket_positions: BTreeSet<ChunkPos>,
    active_player_promotions: BTreeSet<ChunkPos>,
    player_promotion_first_desired_tick: BTreeMap<ChunkPos, u64>,
    max_active_player_promotions: usize,
    player_promotions_admitted_total: u64,
    player_promotions_cancelled_before_admission: u64,
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
            aggregate_simulation_positions: BTreeSet::new(),
            player_ticket_positions: BTreeSet::new(),
            active_player_promotions: BTreeSet::new(),
            player_promotion_first_desired_tick: BTreeMap::new(),
            max_active_player_promotions: DEFAULT_MAX_ACTIVE_PLAYER_PROMOTIONS,
            player_promotions_admitted_total: 0,
            player_promotions_cancelled_before_admission: 0,
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

    pub(crate) fn ticket_generation(&self) -> u64 {
        self.ticket_generation
    }

    pub(crate) fn set_aggregate_interest_positions_with_priority(
        &mut self,
        new_resident_positions: BTreeSet<ChunkPos>,
        new_simulation_positions: BTreeSet<ChunkPos>,
        priority_centers: Vec<ChunkPos>,
    ) {
        debug_assert!(new_simulation_positions.is_subset(&new_resident_positions));
        let priority_centers = priority_centers
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let priority_changed = self.aggregate_interest_priority_centers != priority_centers;
        let old_resident_positions = std::mem::take(&mut self.aggregate_resident_positions);
        let old_simulation_positions = std::mem::take(&mut self.aggregate_simulation_positions);
        let old_residency_only = old_resident_positions
            .difference(&old_simulation_positions)
            .copied()
            .collect::<BTreeSet<_>>();
        let new_residency_only = new_resident_positions
            .difference(&new_simulation_positions)
            .copied()
            .collect::<BTreeSet<_>>();

        let departed_simulation_positions = old_simulation_positions
            .difference(&new_simulation_positions)
            .copied()
            .collect::<Vec<_>>();
        for pos in &departed_simulation_positions {
            if !self.player_ticket_positions.contains(pos) {
                self.player_promotions_cancelled_before_admission = self
                    .player_promotions_cancelled_before_admission
                    .saturating_add(1);
            }
            self.active_player_promotions.remove(pos);
            self.player_promotion_first_desired_tick.remove(pos);
            if !self.player_ticket_positions.remove(pos) {
                continue;
            }
            self.remove_ticket(
                ChunkTicketType::Player,
                *pos,
                PLAYER_TICKET_LEVEL,
                ChunkTicketKey::Chunk(*pos),
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
            self.player_promotion_first_desired_tick
                .insert(pos, self.ticket_tick);
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
        self.aggregate_simulation_positions = new_simulation_positions;
        self.aggregate_interest_priority_centers = priority_centers;
        if priority_changed {
            self.mark_tickets_changed();
        }
        self.refill_player_promotions();
    }

    pub(crate) fn complete_player_promotion(&mut self, pos: ChunkPos) -> bool {
        if !self.active_player_promotions.remove(&pos) {
            return false;
        }
        self.refill_player_promotions();
        true
    }

    fn refill_player_promotions(&mut self) {
        let available = self
            .max_active_player_promotions
            .saturating_sub(self.active_player_promotions.len());
        if available == 0 {
            return;
        }

        let mut candidates = self
            .aggregate_simulation_positions
            .difference(&self.player_ticket_positions)
            .copied()
            .collect::<Vec<_>>();
        candidates.sort_by_key(|pos| self.player_promotion_priority_key(*pos));

        for pos in candidates.into_iter().take(available) {
            self.player_ticket_positions.insert(pos);
            self.active_player_promotions.insert(pos);
            self.player_promotions_admitted_total =
                self.player_promotions_admitted_total.saturating_add(1);
            self.add_ticket(
                ChunkTicketType::Player,
                pos,
                PLAYER_TICKET_LEVEL,
                ChunkTicketKey::Chunk(pos),
            );
        }
    }

    fn player_promotion_priority_key(&self, pos: ChunkPos) -> (i64, i64, u64, i32, i32) {
        let (chebyshev_distance, manhattan_distance) = self
            .aggregate_interest_priority_centers
            .iter()
            .map(|center| {
                let [dx, dz] = self.topology.shortest_chunk_displacement(*center, pos);
                let dx = dx.abs();
                let dz = dz.abs();
                (dx.max(dz), dx + dz)
            })
            .min()
            .unwrap_or((0, 0));
        (
            chebyshev_distance,
            manhattan_distance,
            self.player_promotion_first_desired_tick
                .get(&pos)
                .copied()
                .unwrap_or(self.ticket_tick),
            pos.z,
            pos.x,
        )
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
        self.active_levels_with_cache_status().0
    }

    pub(crate) fn active_levels_with_cache_status(&self) -> (Arc<BTreeMap<ChunkPos, i32>>, bool) {
        if let Some((generation, levels)) = self.active_levels_cache.borrow().as_ref()
            && *generation == self.ticket_generation
        {
            return (Arc::clone(levels), true);
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
        (levels, false)
    }

    pub(crate) fn player_interest_positions(&self) -> BTreeSet<ChunkPos> {
        self.aggregate_resident_positions.clone()
    }

    pub(crate) fn player_simulation_positions(&self) -> &BTreeSet<ChunkPos> {
        &self.aggregate_simulation_positions
    }

    pub(crate) fn active_player_promotion_positions(&self) -> &BTreeSet<ChunkPos> {
        &self.active_player_promotions
    }

    pub(crate) fn player_promotion_diagnostics(&self) -> PlayerPromotionDiagnostics {
        let queued = self
            .aggregate_simulation_positions
            .difference(&self.player_ticket_positions)
            .count();
        let oldest_queued_age_ticks = self
            .aggregate_simulation_positions
            .difference(&self.player_ticket_positions)
            .filter_map(|pos| self.player_promotion_first_desired_tick.get(pos))
            .map(|first_tick| self.ticket_tick.saturating_sub(*first_tick))
            .max()
            .unwrap_or(0);
        PlayerPromotionDiagnostics {
            desired: self.aggregate_simulation_positions.len(),
            queued,
            active: self.active_player_promotions.len(),
            max_active: self.max_active_player_promotions,
            admitted_total: self.player_promotions_admitted_total,
            cancelled_before_admission: self.player_promotions_cancelled_before_admission,
            oldest_queued_age_ticks,
        }
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

        let (first, first_hit) = manager.active_levels_with_cache_status();
        let (second, second_hit) = manager.active_levels_with_cache_status();
        assert!(!first_hit);
        assert!(second_hit);
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first.get(&center), Some(&MAX_CHUNK_DISTANCE));

        let east = ChunkPos::new(1, 0);
        manager.add_ticket(
            ChunkTicketType::Forced,
            east,
            MAX_CHUNK_DISTANCE,
            ChunkTicketKey::Chunk(east),
        );
        let (after_add, after_add_hit) = manager.active_levels_with_cache_status();
        assert!(!after_add_hit);
        assert!(!Arc::ptr_eq(&first, &after_add));
        assert_eq!(after_add.get(&east), Some(&MAX_CHUNK_DISTANCE));

        manager.remove_ticket(
            ChunkTicketType::Forced,
            east,
            MAX_CHUNK_DISTANCE,
            ChunkTicketKey::Chunk(east),
        );
        let (after_remove, after_remove_hit) = manager.active_levels_with_cache_status();
        assert!(!after_remove_hit);
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

    #[test]
    fn changed_priority_center_invalidates_reconciliation_generation() {
        let mut manager = ChunkDistanceManager::new();
        let positions = [ChunkPos::new(0, 0), ChunkPos::new(1, 0)]
            .into_iter()
            .collect::<BTreeSet<_>>();
        manager.set_aggregate_interest_positions_with_priority(
            positions.clone(),
            positions.clone(),
            vec![ChunkPos::new(0, 0)],
        );
        let first_generation = manager.ticket_generation();
        manager.set_aggregate_interest_positions_with_priority(
            positions.clone(),
            positions,
            vec![ChunkPos::new(1, 0)],
        );

        assert_ne!(manager.ticket_generation(), first_generation);
    }

    #[test]
    fn player_ticket_admission_keeps_only_four_cold_promotions_active() {
        let mut manager = ChunkDistanceManager::new();
        let center = ChunkPos::new(0, 0);
        let positions = (-2..=2)
            .flat_map(|z| (-2..=2).map(move |x| ChunkPos::new(x, z)))
            .collect::<BTreeSet<_>>();

        manager.set_aggregate_interest_positions_with_priority(
            positions.clone(),
            positions,
            vec![center],
        );

        let diagnostics = manager.player_promotion_diagnostics();
        assert_eq!(diagnostics.desired, 25);
        assert_eq!(diagnostics.queued, 21);
        assert_eq!(diagnostics.active, 4);
        assert_eq!(diagnostics.max_active, 4);
        assert_eq!(diagnostics.admitted_total, 4);
        assert!(
            manager
                .active_player_promotion_positions()
                .contains(&center)
        );
        assert_eq!(manager.player_ticket_positions.len(), 4);
        assert_eq!(manager.ticketed_chunk_count(), 4);
    }

    #[test]
    fn completed_player_promotion_refills_one_slot_without_dropping_ticket() {
        let mut manager = ChunkDistanceManager::new();
        let positions = (-2..=2)
            .flat_map(|z| (-2..=2).map(move |x| ChunkPos::new(x, z)))
            .collect::<BTreeSet<_>>();
        manager.set_aggregate_interest_positions_with_priority(
            positions.clone(),
            positions,
            vec![ChunkPos::new(0, 0)],
        );
        let completed = *manager
            .active_player_promotion_positions()
            .iter()
            .next()
            .unwrap();

        assert!(manager.complete_player_promotion(completed));

        let diagnostics = manager.player_promotion_diagnostics();
        assert_eq!(diagnostics.active, 4);
        assert_eq!(diagnostics.queued, 20);
        assert_eq!(diagnostics.admitted_total, 5);
        assert!(manager.player_ticket_positions.contains(&completed));
        assert_eq!(manager.player_ticket_positions.len(), 5);
    }

    #[test]
    fn interest_jump_clears_unadmitted_player_intent_before_ticketing() {
        let mut manager = ChunkDistanceManager::new();
        let old_positions = (-2..=2)
            .flat_map(|z| (-2..=2).map(move |x| ChunkPos::new(x, z)))
            .collect::<BTreeSet<_>>();
        manager.set_aggregate_interest_positions_with_priority(
            old_positions.clone(),
            old_positions.clone(),
            vec![ChunkPos::new(0, 0)],
        );
        let old_admitted = manager.player_ticket_positions.clone();
        let new_positions = (9_998..=10_002)
            .flat_map(|z| (9_998..=10_002).map(move |x| ChunkPos::new(x, z)))
            .collect::<BTreeSet<_>>();

        manager.set_aggregate_interest_positions_with_priority(
            new_positions.clone(),
            new_positions,
            vec![ChunkPos::new(10_000, 10_000)],
        );

        let diagnostics = manager.player_promotion_diagnostics();
        assert_eq!(diagnostics.cancelled_before_admission, 21);
        assert_eq!(diagnostics.active, 4);
        assert_eq!(diagnostics.queued, 21);
        assert!(
            old_admitted
                .iter()
                .all(|pos| manager.ticket_count_at(*pos) == 0)
        );
    }
}
