#![forbid(unsafe_code)]

use std::collections::BTreeMap;

mod interaction;

use mclone_core::{CHUNK_WIDTH, ChunkPos, ChunkSnapshot, SECTION_HEIGHT};
use mclone_protocol::{ChunkView, ClientCommand, SectionBlockUpdate, ServerUpdate};

pub use interaction::{
    CREATIVE_PICK_RANGE, ClientInteractionController, DEFAULT_DEBUG_PLACE_BLOCK,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientHost {
    LocalIntegrated,
    RemoteDedicated,
}

#[derive(Clone, Debug)]
pub struct ClientRuntime {
    host: ClientHost,
    chunk_view: Option<ChunkView>,
    chunks: BTreeMap<ChunkPos, ChunkSnapshot>,
    day_time: u64,
}

impl ClientRuntime {
    pub fn new(host: ClientHost) -> Self {
        Self {
            host,
            chunk_view: None,
            chunks: BTreeMap::new(),
            day_time: 0,
        }
    }

    pub fn local_integrated() -> Self {
        Self::new(ClientHost::LocalIntegrated)
    }

    pub const fn host(&self) -> ClientHost {
        self.host
    }

    pub fn set_chunk_view(&mut self, view: ChunkView) -> ClientCommand {
        self.chunk_view = Some(view.clone());
        ClientCommand::SetChunkView(view)
    }

    pub fn chunk_view(&self) -> Option<&ChunkView> {
        self.chunk_view.as_ref()
    }

    pub fn apply_update(&mut self, update: ServerUpdate) {
        match update {
            ServerUpdate::ChunkSnapshot(snapshot) => {
                self.chunks.insert(snapshot.pos, snapshot);
            }
            ServerUpdate::ChunkUnload { pos } => {
                self.chunks.remove(&pos);
            }
            ServerUpdate::SectionBlockUpdates {
                pos,
                section_y,
                updates,
            } => {
                self.apply_section_block_updates(pos, section_y, &updates);
            }
            ServerUpdate::TimeUpdate { day_time } => {
                self.day_time = day_time;
            }
        }
    }

    pub fn apply_updates(&mut self, updates: impl IntoIterator<Item = ServerUpdate>) {
        for update in updates {
            self.apply_update(update);
        }
    }

    pub fn chunk_snapshot(&self, pos: ChunkPos) -> Option<&ChunkSnapshot> {
        self.chunks.get(&pos)
    }

    pub fn chunk_snapshots(&self) -> impl Iterator<Item = &ChunkSnapshot> {
        self.chunks.values()
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Latest authoritative world day-time (ticks) from the server.
    pub const fn day_time(&self) -> u64 {
        self.day_time
    }

    /// Celestial phase in `[0, 1)` for the current day-time. See
    /// [`mclone_core::time::time_of_day`].
    pub fn time_of_day(&self) -> f32 {
        mclone_core::time::time_of_day(self.day_time)
    }

    /// Celestial rig rotation in radians. See [`mclone_core::time::sun_angle`].
    pub fn sun_angle(&self) -> f32 {
        mclone_core::time::sun_angle(self.day_time)
    }

    pub fn apply_section_block_updates(
        &mut self,
        pos: ChunkPos,
        section_y: i32,
        updates: &[SectionBlockUpdate],
    ) -> bool {
        let Some(snapshot) = self.chunks.get_mut(&pos) else {
            return false;
        };
        let mut changed = false;
        for update in updates {
            if update.local_x as i32 >= CHUNK_WIDTH
                || update.local_y as i32 >= SECTION_HEIGHT
                || update.local_z as i32 >= CHUNK_WIDTH
            {
                continue;
            }
            changed |= snapshot.patch_section_block(
                section_y,
                update.local_x as i32,
                update.local_y as i32,
                update.local_z as i32,
                update.block_state,
            );
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{
        AIR_BLOCK_STATE_ID, BlockStateId, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkStatus,
        chunk_section_index,
    };

    #[test]
    fn distinguishes_local_and_remote_hosts() {
        assert_ne!(ClientHost::LocalIntegrated, ClientHost::RemoteDedicated);
    }

    #[test]
    fn set_chunk_view_returns_protocol_command() {
        let mut runtime = ClientRuntime::local_integrated();
        let view = ChunkView {
            center: ChunkPos::new(2, -3),
            render_distance: 4,
            chunk_tracking_radius: 5,
        };

        assert_eq!(
            runtime.set_chunk_view(view.clone()),
            ClientCommand::SetChunkView(view.clone())
        );
        assert_eq!(runtime.chunk_view(), Some(&view));
    }

    #[test]
    fn client_runtime_hydrates_and_unloads_chunk_snapshots() {
        let mut runtime = ClientRuntime::local_integrated();
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        );

        runtime.apply_update(ServerUpdate::ChunkSnapshot(snapshot.clone()));

        assert_eq!(runtime.loaded_chunk_count(), 1);
        assert_eq!(runtime.chunk_snapshot(ChunkPos::new(0, 0)), Some(&snapshot));

        runtime.apply_update(ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(0, 0),
        });

        assert_eq!(runtime.loaded_chunk_count(), 0);
        assert_eq!(runtime.chunk_snapshot(ChunkPos::new(0, 0)), None);
    }

    #[test]
    fn client_runtime_tracks_day_time_from_server() {
        let mut runtime = ClientRuntime::local_integrated();
        assert_eq!(runtime.day_time(), 0);

        runtime.apply_update(ServerUpdate::TimeUpdate { day_time: 6_000 });

        assert_eq!(runtime.day_time(), 6_000);
        // dayTime 6000 is noon, which the smoothed curve maps to phase ~0.0.
        assert!(runtime.time_of_day().abs() < 1e-4);
        assert!(runtime.sun_angle().abs() < 1e-3);
    }

    #[test]
    fn client_runtime_applies_section_block_updates_to_loaded_snapshot() {
        let mut runtime = ClientRuntime::local_integrated();
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Surface,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        );
        runtime.apply_update(ServerUpdate::ChunkSnapshot(snapshot));

        runtime.apply_update(ServerUpdate::SectionBlockUpdates {
            pos: ChunkPos::new(0, 0),
            section_y: 0,
            updates: vec![SectionBlockUpdate {
                local_x: 1,
                local_y: 2,
                local_z: 3,
                block_state: BlockStateId(42),
            }],
        });

        let snapshot = runtime.chunk_snapshot(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(snapshot.sections.len(), 1);
        assert_eq!(
            snapshot.sections[0].unpack_block_state_ids()[chunk_section_index(1, 2, 3)],
            BlockStateId(42)
        );
    }

    #[test]
    fn client_runtime_ignores_section_block_updates_for_unloaded_chunks() {
        let mut runtime = ClientRuntime::local_integrated();

        assert!(!runtime.apply_section_block_updates(
            ChunkPos::new(5, 6),
            0,
            &[SectionBlockUpdate {
                local_x: 1,
                local_y: 2,
                local_z: 3,
                block_state: BlockStateId(42),
            }],
        ));
        assert_eq!(runtime.loaded_chunk_count(), 0);
    }
}
