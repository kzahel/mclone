//! Basic falling-block ticks.
//!
//! Mirrors the shared Java `FallingBlock` scheduling rule for the terrain-MVP
//! ids we currently expose as placeable blocks. This deliberately stops short
//! of a full `FallingBlockEntity` port.

use std::collections::{BTreeSet, HashSet};

use mclone_core::{BlockPos, ChunkPos, Direction};
use mclone_worldgen::block::{
    GRAVEL, RED_SAND, RawBlockId, SAND, has_fluid, material_blocks_motion,
};

use crate::ChunkScheduler;

const FALLING_BLOCK_DELAY_AFTER_PLACE: i32 = 2;
const MAX_SCHEDULED_BLOCK_TICKS_PER_TICK: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScheduledBlockTickRequest {
    pub(crate) pos: BlockPos,
    pub(crate) delay: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct BlockTickPhaseReport {
    pub(crate) executed_ticks: usize,
    pub(crate) due_ticks: usize,
    pub(crate) deferred_due_ticks: usize,
    pub(crate) mutated_blocks: usize,
    pub(crate) scheduled_ticks: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct BlockTickKey {
    pos: BlockPos,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScheduledBlockTick {
    trigger_tick: u64,
    sequence: u64,
    key: BlockTickKey,
}

impl Ord for ScheduledBlockTick {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.trigger_tick
            .cmp(&other.trigger_tick)
            .then_with(|| self.sequence.cmp(&other.sequence))
            .then_with(|| self.key.pos.x.cmp(&other.key.pos.x))
            .then_with(|| self.key.pos.y.cmp(&other.key.pos.y))
            .then_with(|| self.key.pos.z.cmp(&other.key.pos.z))
    }
}

impl PartialOrd for ScheduledBlockTick {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Default)]
pub(crate) struct BlockTickList {
    scheduled_ticks: BTreeSet<ScheduledBlockTick>,
    scheduled_keys: HashSet<BlockTickKey>,
    next_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DueBlockTicks {
    pub(crate) positions: Vec<BlockPos>,
    pub(crate) deferred_due_ticks: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FallingBlockMove {
    pub(crate) from: BlockPos,
    pub(crate) to: BlockPos,
    pub(crate) block: RawBlockId,
}

impl BlockTickList {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn schedule_tick(&mut self, pos: BlockPos, delay: i32, game_time: u64) {
        let key = BlockTickKey { pos };
        if !self.scheduled_keys.insert(key) {
            return;
        }

        let entry = ScheduledBlockTick {
            trigger_tick: game_time.saturating_add(delay.max(0) as u64),
            sequence: self.next_sequence,
            key,
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.scheduled_ticks.insert(entry);
    }

    pub(crate) fn drain_due(
        &mut self,
        game_time: u64,
        block_ticking_chunks: &[ChunkPos],
    ) -> DueBlockTicks {
        let ticking_chunks = block_ticking_chunks
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut deferred_due_ticks = 0;
        let mut due_ticks = Vec::new();
        for entry in self
            .scheduled_ticks
            .iter()
            .take_while(|entry| entry.trigger_tick <= game_time)
        {
            if ticking_chunks.contains(&entry.key.pos.chunk_pos()) {
                due_ticks.push(*entry);
                if due_ticks.len() == MAX_SCHEDULED_BLOCK_TICKS_PER_TICK {
                    break;
                }
            } else {
                deferred_due_ticks += 1;
            }
        }

        for entry in &due_ticks {
            self.scheduled_ticks.remove(entry);
            self.scheduled_keys.remove(&entry.key);
        }

        DueBlockTicks {
            positions: due_ticks
                .into_iter()
                .map(|entry| entry.key.pos)
                .collect::<Vec<_>>(),
            deferred_due_ticks,
        }
    }

    pub(crate) fn size(&self) -> usize {
        self.scheduled_keys.len()
    }
}

pub(crate) fn block_tick_requests_after_block_change(
    pos: BlockPos,
    new_block: RawBlockId,
) -> Vec<ScheduledBlockTickRequest> {
    let mut requests = Vec::new();
    if is_basic_falling_block(new_block) {
        requests.push(ScheduledBlockTickRequest {
            pos,
            delay: FALLING_BLOCK_DELAY_AFTER_PLACE,
        });
    }
    for direction in Direction::ALL {
        requests.push(ScheduledBlockTickRequest {
            pos: pos.relative(direction),
            delay: FALLING_BLOCK_DELAY_AFTER_PLACE,
        });
    }
    requests
}

pub(crate) fn basic_falling_block_move(
    scheduler: &ChunkScheduler,
    pos: BlockPos,
) -> Option<FallingBlockMove> {
    let Some(block) = scheduler.block_at_world(pos) else {
        return None;
    };
    if !is_basic_falling_block(block) || !falling_block_can_fall_from(scheduler, pos) {
        return None;
    }

    let Some(destination) = falling_block_resting_pos(scheduler, pos) else {
        return None;
    };
    if destination == pos {
        return None;
    }

    // TODO(falling-blocks): replace this instant drop with a real
    // `FallingBlockEntity` port, and expand coverage beyond sand/red sand/gravel
    // to concrete powder, anvils, dragon egg, dripstone, and scaffolding.
    Some(FallingBlockMove {
        from: pos,
        to: destination,
        block,
    })
}

fn falling_block_can_fall_from(scheduler: &ChunkScheduler, pos: BlockPos) -> bool {
    pos.y >= 0 && is_free_for_falling_block(scheduler.block_at_world(pos.below()))
}

fn falling_block_resting_pos(scheduler: &ChunkScheduler, pos: BlockPos) -> Option<BlockPos> {
    let mut current = pos;
    while current.y > 0 {
        let below = current.below();
        match scheduler.block_at_world(below) {
            Some(block) if is_free_block(block) => current = below,
            Some(_) => return Some(current),
            None => return Some(current),
        }
    }
    Some(current)
}

fn is_free_for_falling_block(block: Option<RawBlockId>) -> bool {
    block.is_some_and(is_free_block)
}

fn is_free_block(block: RawBlockId) -> bool {
    is_replaceable_block(block) || has_fluid(block)
}

fn is_replaceable_block(block: RawBlockId) -> bool {
    !material_blocks_motion(block)
}

pub(crate) const fn is_basic_falling_block(block: RawBlockId) -> bool {
    matches!(block, SAND | RED_SAND | GRAVEL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_worldgen::block::{AIR, CAVE_AIR, DIRT, WATER};

    #[test]
    fn basic_falling_set_is_intentionally_narrow() {
        assert!(is_basic_falling_block(SAND));
        assert!(is_basic_falling_block(RED_SAND));
        assert!(is_basic_falling_block(GRAVEL));
        assert!(!is_basic_falling_block(DIRT));
    }

    #[test]
    fn falling_blocks_can_enter_air_and_liquid() {
        assert!(is_free_for_falling_block(Some(AIR)));
        assert!(is_free_for_falling_block(Some(CAVE_AIR)));
        assert!(is_free_for_falling_block(Some(WATER)));
        assert!(!is_free_for_falling_block(Some(DIRT)));
        assert!(!is_free_for_falling_block(None));
    }

    #[test]
    fn block_change_schedules_changed_falling_block_and_neighbors() {
        let pos = BlockPos::new(1, 2, 3);

        assert_eq!(
            block_tick_requests_after_block_change(pos, SAND),
            vec![
                ScheduledBlockTickRequest { pos, delay: 2 },
                ScheduledBlockTickRequest {
                    pos: BlockPos::new(1, 1, 3),
                    delay: 2
                },
                ScheduledBlockTickRequest {
                    pos: BlockPos::new(1, 3, 3),
                    delay: 2
                },
                ScheduledBlockTickRequest {
                    pos: BlockPos::new(1, 2, 2),
                    delay: 2
                },
                ScheduledBlockTickRequest {
                    pos: BlockPos::new(1, 2, 4),
                    delay: 2
                },
                ScheduledBlockTickRequest {
                    pos: BlockPos::new(0, 2, 3),
                    delay: 2
                },
                ScheduledBlockTickRequest {
                    pos: BlockPos::new(2, 2, 3),
                    delay: 2
                }
            ]
        );
        assert_eq!(
            block_tick_requests_after_block_change(pos, DIRT),
            vec![
                ScheduledBlockTickRequest {
                    pos: BlockPos::new(1, 1, 3),
                    delay: 2
                },
                ScheduledBlockTickRequest {
                    pos: BlockPos::new(1, 3, 3),
                    delay: 2
                },
                ScheduledBlockTickRequest {
                    pos: BlockPos::new(1, 2, 2),
                    delay: 2
                },
                ScheduledBlockTickRequest {
                    pos: BlockPos::new(1, 2, 4),
                    delay: 2
                },
                ScheduledBlockTickRequest {
                    pos: BlockPos::new(0, 2, 3),
                    delay: 2
                },
                ScheduledBlockTickRequest {
                    pos: BlockPos::new(2, 2, 3),
                    delay: 2
                }
            ]
        );
    }
}
