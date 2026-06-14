#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use mclone_core::{ChunkPos, ChunkSnapshot};
use mclone_protocol::{ChunkInterest, ClientCommand, ServerUpdate};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientHost {
    LocalIntegrated,
    RemoteDedicated,
}

#[derive(Clone, Debug)]
pub struct ClientRuntime {
    host: ClientHost,
    chunk_interest: Option<ChunkInterest>,
    chunks: BTreeMap<ChunkPos, ChunkSnapshot>,
}

impl ClientRuntime {
    pub fn new(host: ClientHost) -> Self {
        Self {
            host,
            chunk_interest: None,
            chunks: BTreeMap::new(),
        }
    }

    pub fn local_integrated() -> Self {
        Self::new(ClientHost::LocalIntegrated)
    }

    pub const fn host(&self) -> ClientHost {
        self.host
    }

    pub fn set_chunk_interest(&mut self, interest: ChunkInterest) -> ClientCommand {
        self.chunk_interest = Some(interest.clone());
        ClientCommand::SetChunkInterest(interest)
    }

    pub fn chunk_interest(&self) -> Option<&ChunkInterest> {
        self.chunk_interest.as_ref()
    }

    pub fn apply_update(&mut self, update: ServerUpdate) {
        match update {
            ServerUpdate::ChunkSnapshot(snapshot) => {
                self.chunks.insert(snapshot.pos, snapshot);
            }
            ServerUpdate::ChunkUnload { pos } => {
                self.chunks.remove(&pos);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkStatus};

    #[test]
    fn distinguishes_local_and_remote_hosts() {
        assert_ne!(ClientHost::LocalIntegrated, ClientHost::RemoteDedicated);
    }

    #[test]
    fn set_chunk_interest_returns_protocol_command() {
        let mut runtime = ClientRuntime::local_integrated();
        let interest = ChunkInterest {
            center: ChunkPos::new(2, -3),
            radius_chunks: 4,
        };

        assert_eq!(
            runtime.set_chunk_interest(interest.clone()),
            ClientCommand::SetChunkInterest(interest.clone())
        );
        assert_eq!(runtime.chunk_interest(), Some(&interest));
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
}
