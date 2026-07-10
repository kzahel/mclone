//! Target-neutral deferred destruction contract.
//!
//! Large chunk snapshots can be expensive to destroy on a frame thread.  The
//! scene-session shell only knows this bounded service: native assembly may
//! drain it on a thread, while a browser adapter may drain it between frames or
//! in a worker.  Capacity and fallback accounting stay visible to diagnostics
//! on every target.

use std::collections::VecDeque;

use mclone_core::ChunkSnapshot;

/// Slice 0 observed a maximum live backlog of 295 items.  Keep more than an
/// order of magnitude of headroom while retaining a hard, testable bound.
pub const DEFAULT_DEFERRED_DROP_MAX_ITEMS: usize = 4_096;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeferredDropBacklog {
    pub pending_items: usize,
    pub max_items: usize,
    pub inline_fallback_items: usize,
}

pub trait DeferredDropService: std::fmt::Debug {
    /// Queue ownership for later destruction. Returns `true` when accepted;
    /// rejection destroys the value on the caller and is accounted as fallback.
    fn enqueue(&mut self, snapshot: ChunkSnapshot, item_count: usize) -> bool;

    fn backlog(&self) -> DeferredDropBacklog;

    /// Drain approximately `item_budget` worth of queued snapshot contents.
    /// A snapshot is atomic, so the first item may exceed the budget to ensure
    /// forward progress.
    fn drain(&mut self, item_budget: usize) -> usize;
}

#[derive(Debug)]
pub struct BoundedDeferredDropQueue {
    queue: VecDeque<(ChunkSnapshot, usize)>,
    pending_items: usize,
    max_items: usize,
    inline_fallback_items: usize,
}

impl Default for BoundedDeferredDropQueue {
    fn default() -> Self {
        Self::new(DEFAULT_DEFERRED_DROP_MAX_ITEMS)
    }
}

impl BoundedDeferredDropQueue {
    pub fn new(max_items: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            pending_items: 0,
            max_items,
            inline_fallback_items: 0,
        }
    }
}

impl DeferredDropService for BoundedDeferredDropQueue {
    fn enqueue(&mut self, snapshot: ChunkSnapshot, item_count: usize) -> bool {
        if item_count == 0 {
            return true;
        }
        let Some(next_items) = self.pending_items.checked_add(item_count) else {
            self.inline_fallback_items = self.inline_fallback_items.saturating_add(item_count);
            drop(snapshot);
            return false;
        };
        if next_items > self.max_items {
            self.inline_fallback_items = self.inline_fallback_items.saturating_add(item_count);
            drop(snapshot);
            return false;
        }
        self.pending_items = next_items;
        self.queue.push_back((snapshot, item_count));
        true
    }

    fn backlog(&self) -> DeferredDropBacklog {
        DeferredDropBacklog {
            pending_items: self.pending_items,
            max_items: self.max_items,
            inline_fallback_items: self.inline_fallback_items,
        }
    }

    fn drain(&mut self, item_budget: usize) -> usize {
        let mut drained = 0_usize;
        while let Some((_, item_count)) = self.queue.front() {
            if drained > 0 && drained.saturating_add(*item_count) > item_budget {
                break;
            }
            let (_, item_count) = self.queue.pop_front().expect("front item exists");
            self.pending_items = self.pending_items.saturating_sub(item_count);
            drained = drained.saturating_add(item_count);
            if drained >= item_budget {
                break;
            }
        }
        drained
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> ChunkSnapshot {
        ChunkSnapshot::from_block_state_ids(
            mclone_core::ChunkPos::new(0, 0),
            mclone_core::ChunkStatus::Full,
            mclone_core::ChunkRevision(1),
            0,
            16,
            &vec![mclone_core::AIR_BLOCK_STATE_ID; mclone_core::CHUNK_SECTION_VOLUME],
        )
    }

    #[test]
    fn queue_is_bounded_and_accounts_inline_fallback() {
        let mut queue = BoundedDeferredDropQueue::new(3);
        assert!(queue.enqueue(snapshot(), 2));
        assert!(!queue.enqueue(snapshot(), 2));
        assert_eq!(
            queue.backlog(),
            DeferredDropBacklog {
                pending_items: 2,
                max_items: 3,
                inline_fallback_items: 2,
            }
        );
        assert_eq!(queue.drain(2), 2);
        assert_eq!(queue.backlog().pending_items, 0);
    }

    #[test]
    fn replacement_drops_the_old_epoch_queue() {
        let mut queue: Box<dyn DeferredDropService> = Box::new(BoundedDeferredDropQueue::new(8));
        assert!(queue.enqueue(snapshot(), 4));
        queue = Box::new(BoundedDeferredDropQueue::new(8));
        assert_eq!(queue.backlog().pending_items, 0);
    }
}
