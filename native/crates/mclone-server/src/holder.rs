//! Per-chunk holder state.
//!
//! Move-only home for `ChunkHolder` and its `ChunkStatusSlot`, mirroring Java's
//! `server/level/ChunkHolder`: ticket level, per-status scheduling slots, the
//! published snapshot / live + dependency block buffers, and residency/dirty
//! bookkeeping. The scheduler drives these via `pub(crate)` mutators; the public
//! read accessors stay `pub`.

use std::collections::BTreeMap;

use mclone_core::{ChunkPos, ChunkRevision, ChunkSnapshot, ChunkStatus};
use mclone_worldgen::levelgen::MutableChunkBlockBuffer;

use crate::{
    ChunkJobId, ChunkResidency, ChunkStatusStep, FullChunkStatus, UNLOADED_CHUNK_LEVEL,
    full_chunk_status_for_ticket_level, mutable_buffer_from_snapshot,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkStatusSlot {
    pub status: ChunkStatus,
    pub step: ChunkStatusStep,
    pub revision: Option<ChunkRevision>,
    pub job_id: Option<ChunkJobId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkHolder {
    pos: ChunkPos,
    pub(crate) ticket_level: i32,
    pub(crate) target_status: Option<ChunkStatus>,
    status_slots: BTreeMap<ChunkStatus, ChunkStatusSlot>,
    pub(crate) published_snapshot: Option<ChunkSnapshot>,
    pub(crate) live_blocks: Option<MutableChunkBlockBuffer>,
    pub(crate) dependency_buffer: Option<MutableChunkBlockBuffer>,
    pub(crate) client_visible: bool,
    residency: ChunkResidency,
    pub(crate) dirty: bool,
}

impl ChunkHolder {
    pub(crate) fn new(pos: ChunkPos) -> Self {
        Self {
            pos,
            ticket_level: UNLOADED_CHUNK_LEVEL,
            target_status: None,
            status_slots: BTreeMap::new(),
            published_snapshot: None,
            live_blocks: None,
            dependency_buffer: None,
            client_visible: false,
            residency: ChunkResidency::NotResident,
            dirty: false,
        }
    }

    pub const fn pos(&self) -> ChunkPos {
        self.pos
    }

    pub fn target_status(&self) -> Option<ChunkStatus> {
        self.target_status
    }

    pub fn ticket_level(&self) -> i32 {
        self.ticket_level
    }

    pub fn full_status(&self) -> FullChunkStatus {
        full_chunk_status_for_ticket_level(self.ticket_level)
    }

    pub fn status_slot(&self, status: ChunkStatus) -> Option<&ChunkStatusSlot> {
        self.status_slots.get(&status)
    }

    pub fn ready_status_count(&self) -> usize {
        self.status_slots
            .values()
            .filter(|slot| slot.step == ChunkStatusStep::Ready)
            .count()
    }

    pub fn residency(&self) -> ChunkResidency {
        self.residency
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn snapshot(&self) -> Option<&ChunkSnapshot> {
        self.published_snapshot.as_ref()
    }

    pub fn has_dependency_buffer(&self) -> bool {
        self.dependency_buffer.is_some()
    }

    pub fn is_client_visible(&self) -> bool {
        self.client_visible
    }

    pub(crate) fn mark_scheduled(&mut self, status: ChunkStatus) {
        self.status_slots.insert(
            status,
            ChunkStatusSlot {
                status,
                step: ChunkStatusStep::Scheduled,
                revision: None,
                job_id: None,
            },
        );
    }

    pub(crate) fn mark_ready(&mut self, status: ChunkStatus, revision: Option<ChunkRevision>) {
        let job_id = self.status_slots.get(&status).and_then(|slot| slot.job_id);
        self.status_slots.insert(
            status,
            ChunkStatusSlot {
                status,
                step: ChunkStatusStep::Ready,
                revision,
                job_id,
            },
        );
    }

    pub(crate) fn assign_status_job(&mut self, status: ChunkStatus, job_id: ChunkJobId) {
        self.status_slots
            .entry(status)
            .or_insert(ChunkStatusSlot {
                status,
                step: ChunkStatusStep::Scheduled,
                revision: None,
                job_id: None,
            })
            .job_id = Some(job_id);
    }

    pub(crate) fn publish_snapshot(
        &mut self,
        snapshot: ChunkSnapshot,
        residency: ChunkResidency,
        dirty: bool,
    ) {
        self.mark_ready(snapshot.status, Some(snapshot.revision));
        self.live_blocks = Some(mutable_buffer_from_snapshot(&snapshot));
        self.published_snapshot = Some(snapshot);
        self.residency = residency;
        self.dirty = dirty;
    }

    pub(crate) fn mark_saved(&mut self) {
        self.dirty = false;
        self.residency = ChunkResidency::Saved;
    }

    pub(crate) fn set_ticket_level(&mut self, ticket_level: i32) {
        self.ticket_level = ticket_level;
    }

    pub(crate) fn set_target_status(&mut self, target_status: ChunkStatus) {
        if self
            .target_status
            .is_none_or(|current| current < target_status)
        {
            self.target_status = Some(target_status);
        }
    }

    pub(crate) fn set_dependency_buffer(&mut self, buffer: MutableChunkBlockBuffer) {
        self.dependency_buffer = Some(buffer);
    }

    pub(crate) fn dependency_buffer(&self) -> Option<&MutableChunkBlockBuffer> {
        self.dependency_buffer.as_ref()
    }

    pub(crate) fn set_client_visible(&mut self, client_visible: bool) {
        self.client_visible = client_visible;
    }
}
