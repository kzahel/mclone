#![forbid(unsafe_code)]

use mclone_core::{ChunkPos, ChunkSnapshot};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkInterest {
    pub center: ChunkPos,
    pub radius_chunks: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientCommand {
    SetChunkInterest(ChunkInterest),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerUpdate {
    ChunkSnapshot(ChunkSnapshot),
    ChunkUnload { pos: ChunkPos },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_interest_is_data_only() {
        let interest = ChunkInterest {
            center: ChunkPos::new(0, 0),
            radius_chunks: 8,
        };
        assert_eq!(interest.radius_chunks, 8);
    }

    #[test]
    fn distinguishes_interest_commands_from_server_updates() {
        let interest = ChunkInterest {
            center: ChunkPos::new(1, -2),
            radius_chunks: 3,
        };
        let command = ClientCommand::SetChunkInterest(interest.clone());
        let unload = ServerUpdate::ChunkUnload {
            pos: ChunkPos::new(4, 5),
        };

        assert_eq!(command, ClientCommand::SetChunkInterest(interest));
        assert_eq!(
            unload,
            ServerUpdate::ChunkUnload {
                pos: ChunkPos::new(4, 5)
            }
        );
    }

    #[test]
    fn server_update_can_publish_chunk_snapshot() {
        let snapshot = mclone_core::ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            mclone_core::ChunkStatus::Surface,
            mclone_core::ChunkRevision(1),
            0,
            16,
            &vec![mclone_core::AIR_BLOCK_STATE_ID; mclone_core::CHUNK_SECTION_VOLUME],
        );

        assert_eq!(
            ServerUpdate::ChunkSnapshot(snapshot.clone()),
            ServerUpdate::ChunkSnapshot(snapshot)
        );
    }
}
