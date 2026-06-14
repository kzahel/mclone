#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{ChunkPos, ChunkRevision, ChunkSnapshot, ChunkStatus};
use mclone_protocol::{ChunkInterest, ClientCommand, ServerUpdate};
use mclone_worldgen::levelgen::generate_overworld_surface_chunk;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerMode {
    Integrated,
    Dedicated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkStatusStep {
    Scheduled,
    Ready,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChunkSchedulerEvent {
    StatusChanged {
        pos: ChunkPos,
        status: ChunkStatus,
        step: ChunkStatusStep,
    },
    SnapshotReady(ChunkSnapshot),
    Unloaded {
        pos: ChunkPos,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkStatusSlot {
    pub status: ChunkStatus,
    pub step: ChunkStatusStep,
    pub revision: Option<ChunkRevision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkHolder {
    pos: ChunkPos,
    target_status: Option<ChunkStatus>,
    status_slots: BTreeMap<ChunkStatus, ChunkStatusSlot>,
    published_snapshot: Option<ChunkSnapshot>,
}

impl ChunkHolder {
    fn new(pos: ChunkPos) -> Self {
        Self {
            pos,
            target_status: None,
            status_slots: BTreeMap::new(),
            published_snapshot: None,
        }
    }

    pub const fn pos(&self) -> ChunkPos {
        self.pos
    }

    pub fn target_status(&self) -> Option<ChunkStatus> {
        self.target_status
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

    fn has_ready_status(&self, status: ChunkStatus) -> bool {
        self.status_slots
            .get(&status)
            .is_some_and(|slot| slot.step == ChunkStatusStep::Ready)
    }

    fn mark_scheduled(&mut self, status: ChunkStatus) {
        self.status_slots.insert(
            status,
            ChunkStatusSlot {
                status,
                step: ChunkStatusStep::Scheduled,
                revision: None,
            },
        );
    }

    fn mark_ready(&mut self, status: ChunkStatus, revision: Option<ChunkRevision>) {
        self.status_slots.insert(
            status,
            ChunkStatusSlot {
                status,
                step: ChunkStatusStep::Ready,
                revision,
            },
        );
    }
}

#[derive(Debug)]
pub struct ChunkScheduler {
    seed: i64,
    holders: BTreeMap<ChunkPos, ChunkHolder>,
    next_revision: u64,
}

impl ChunkScheduler {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            holders: BTreeMap::new(),
            next_revision: 1,
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub fn apply_interest(&mut self, interest: ChunkInterest) -> Vec<ChunkSchedulerEvent> {
        let desired_chunks = interest_positions(&interest);
        let desired_set = desired_chunks.iter().copied().collect::<BTreeSet<_>>();
        let mut events = Vec::new();

        for pos in self
            .holders
            .keys()
            .copied()
            .filter(|pos| !desired_set.contains(pos))
            .collect::<Vec<_>>()
        {
            let holder = self.holders.remove(&pos).expect("holder key disappeared");
            if holder.published_snapshot.is_some() {
                events.push(ChunkSchedulerEvent::Unloaded { pos });
            }
        }

        for pos in desired_chunks {
            events.extend(self.schedule_chunk(pos, ChunkStatus::Surface));
        }

        events
    }

    pub fn holder(&self, pos: ChunkPos) -> Option<&ChunkHolder> {
        self.holders.get(&pos)
    }

    pub fn holder_count(&self) -> usize {
        self.holders.len()
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.holders
            .values()
            .filter(|holder| holder.published_snapshot.is_some())
            .count()
    }

    fn schedule_chunk(
        &mut self,
        pos: ChunkPos,
        target_status: ChunkStatus,
    ) -> Vec<ChunkSchedulerEvent> {
        let mut events = Vec::new();
        self.holders
            .entry(pos)
            .or_insert_with(|| ChunkHolder::new(pos))
            .target_status = Some(target_status);

        for status in status_path_to(target_status) {
            if self
                .holders
                .get(&pos)
                .expect("holder must exist before scheduling")
                .has_ready_status(status)
            {
                continue;
            }

            {
                let holder = self
                    .holders
                    .get_mut(&pos)
                    .expect("holder must exist before scheduling");
                holder.mark_scheduled(status);
            }
            events.push(ChunkSchedulerEvent::StatusChanged {
                pos,
                status,
                step: ChunkStatusStep::Scheduled,
            });

            if status == ChunkStatus::Surface {
                let revision = ChunkRevision(self.next_revision);
                self.next_revision += 1;

                let chunk = generate_overworld_surface_chunk(self.seed, pos.x, pos.z);
                let snapshot = chunk.to_chunk_snapshot(revision, ChunkStatus::Surface);
                let holder = self
                    .holders
                    .get_mut(&pos)
                    .expect("holder must exist before publishing");
                holder.mark_ready(status, Some(revision));
                holder.published_snapshot = Some(snapshot.clone());
                events.push(ChunkSchedulerEvent::StatusChanged {
                    pos,
                    status,
                    step: ChunkStatusStep::Ready,
                });
                events.push(ChunkSchedulerEvent::SnapshotReady(snapshot));
            } else {
                let holder = self
                    .holders
                    .get_mut(&pos)
                    .expect("holder must exist before marking ready");
                holder.mark_ready(status, None);
                events.push(ChunkSchedulerEvent::StatusChanged {
                    pos,
                    status,
                    step: ChunkStatusStep::Ready,
                });
            }
        }

        events
    }
}

#[derive(Debug)]
pub struct IntegratedServer {
    seed: i64,
    scheduler: ChunkScheduler,
}

impl IntegratedServer {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            scheduler: ChunkScheduler::new(seed),
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub fn handle_command(&mut self, command: ClientCommand) -> Vec<ServerUpdate> {
        match command {
            ClientCommand::SetChunkInterest(interest) => self.set_chunk_interest(interest),
        }
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.scheduler.loaded_chunk_count()
    }

    pub fn scheduler(&self) -> &ChunkScheduler {
        &self.scheduler
    }

    fn set_chunk_interest(&mut self, interest: ChunkInterest) -> Vec<ServerUpdate> {
        self.scheduler
            .apply_interest(interest)
            .into_iter()
            .filter_map(server_update_from_scheduler_event)
            .collect()
    }
}

fn server_update_from_scheduler_event(event: ChunkSchedulerEvent) -> Option<ServerUpdate> {
    match event {
        ChunkSchedulerEvent::SnapshotReady(snapshot) => Some(ServerUpdate::ChunkSnapshot(snapshot)),
        ChunkSchedulerEvent::Unloaded { pos } => Some(ServerUpdate::ChunkUnload { pos }),
        ChunkSchedulerEvent::StatusChanged { .. } => None,
    }
}

fn interest_positions(interest: &ChunkInterest) -> Vec<ChunkPos> {
    let radius = i32::try_from(interest.radius_chunks).expect("chunk interest radius exceeds i32");
    let min_x = interest.center.x - radius;
    let max_x = interest.center.x + radius;
    let min_z = interest.center.z - radius;
    let max_z = interest.center.z + radius;
    (min_x..=max_x)
        .flat_map(|x| (min_z..=max_z).map(move |z| ChunkPos::new(x, z)))
        .collect()
}

fn status_path_to(target_status: ChunkStatus) -> Vec<ChunkStatus> {
    ALL_STATUSES
        .iter()
        .copied()
        .take_while(|status| *status <= target_status)
        .collect()
}

const ALL_STATUSES: [ChunkStatus; 5] = [
    ChunkStatus::Terrain,
    ChunkStatus::Surface,
    ChunkStatus::Features,
    ChunkStatus::Light,
    ChunkStatus::Full,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_integrated_and_dedicated_modes() {
        assert_ne!(ServerMode::Integrated, ServerMode::Dedicated);
    }

    #[test]
    fn integrated_server_publishes_interested_chunks() {
        let mut server = IntegratedServer::new(12_345);

        let updates = server.handle_command(ClientCommand::SetChunkInterest(ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 1,
        }));

        assert_eq!(updates.len(), 9);
        assert_eq!(server.loaded_chunk_count(), 9);
        assert_eq!(server.scheduler().holder_count(), 9);
        assert!(
            updates
                .iter()
                .all(|update| matches!(update, ServerUpdate::ChunkSnapshot(_)))
        );
    }

    #[test]
    fn duplicate_interest_does_not_regenerate_loaded_chunks() {
        let mut server = IntegratedServer::new(12_345);
        let interest = ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 0,
        };

        assert_eq!(
            server
                .handle_command(ClientCommand::SetChunkInterest(interest.clone()))
                .len(),
            1
        );
        assert_eq!(
            server
                .handle_command(ClientCommand::SetChunkInterest(interest))
                .len(),
            0
        );
    }

    #[test]
    fn chunk_scheduler_records_holder_status_slots_in_order() {
        let mut scheduler = ChunkScheduler::new(12_345);

        let events = scheduler.apply_interest(ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 0,
        });

        assert_eq!(
            events
                .iter()
                .filter_map(|event| {
                    if let ChunkSchedulerEvent::StatusChanged { status, step, .. } = event {
                        Some((*status, *step))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>(),
            vec![
                (ChunkStatus::Terrain, ChunkStatusStep::Scheduled),
                (ChunkStatus::Terrain, ChunkStatusStep::Ready),
                (ChunkStatus::Surface, ChunkStatusStep::Scheduled),
                (ChunkStatus::Surface, ChunkStatusStep::Ready),
            ]
        );
        assert!(matches!(
            events.last(),
            Some(ChunkSchedulerEvent::SnapshotReady(snapshot))
                if snapshot.pos == ChunkPos::new(0, 0)
                    && snapshot.status == ChunkStatus::Surface
        ));

        let holder = scheduler.holder(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(holder.target_status(), Some(ChunkStatus::Surface));
        assert_eq!(holder.ready_status_count(), 2);
        assert_eq!(
            holder.status_slot(ChunkStatus::Terrain),
            Some(&ChunkStatusSlot {
                status: ChunkStatus::Terrain,
                step: ChunkStatusStep::Ready,
                revision: None,
            })
        );
        assert_eq!(
            holder.status_slot(ChunkStatus::Surface),
            Some(&ChunkStatusSlot {
                status: ChunkStatus::Surface,
                step: ChunkStatusStep::Ready,
                revision: Some(ChunkRevision(1)),
            })
        );
    }

    #[test]
    fn chunk_scheduler_coalesces_duplicate_status_requests() {
        let mut scheduler = ChunkScheduler::new(12_345);
        let interest = ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 0,
        };

        assert_eq!(scheduler.apply_interest(interest.clone()).len(), 5);
        assert_eq!(scheduler.apply_interest(interest), Vec::new());
        assert_eq!(scheduler.loaded_chunk_count(), 1);
    }

    #[test]
    fn changed_interest_unloads_chunks_outside_view() {
        let mut server = IntegratedServer::new(12_345);
        server.handle_command(ClientCommand::SetChunkInterest(ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 0,
        }));

        let updates = server.handle_command(ClientCommand::SetChunkInterest(ChunkInterest {
            center: ChunkPos::new(1, 0),
            radius_chunks: 0,
        }));

        assert_eq!(
            updates,
            vec![
                ServerUpdate::ChunkUnload {
                    pos: ChunkPos::new(0, 0)
                },
                ServerUpdate::ChunkSnapshot(
                    generate_overworld_surface_chunk(12_345, 1, 0)
                        .to_chunk_snapshot(ChunkRevision(2), ChunkStatus::Surface)
                )
            ]
        );
    }
}
