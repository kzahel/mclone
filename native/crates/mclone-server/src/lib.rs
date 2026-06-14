#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use mclone_core::{ChunkPos, ChunkRevision, ChunkStatus};
use mclone_protocol::{ChunkInterest, ClientCommand, ServerUpdate};
use mclone_worldgen::levelgen::generate_overworld_surface_chunk;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerMode {
    Integrated,
    Dedicated,
}

#[derive(Debug)]
pub struct IntegratedServer {
    seed: i64,
    loaded_chunks: BTreeSet<ChunkPos>,
    next_revision: u64,
}

impl IntegratedServer {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            loaded_chunks: BTreeSet::new(),
            next_revision: 1,
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
        self.loaded_chunks.len()
    }

    fn set_chunk_interest(&mut self, interest: ChunkInterest) -> Vec<ServerUpdate> {
        let desired_chunks = interest_positions(&interest);
        let desired_set = desired_chunks.iter().copied().collect::<BTreeSet<_>>();
        let mut updates = Vec::new();

        for pos in self
            .loaded_chunks
            .difference(&desired_set)
            .copied()
            .collect::<Vec<_>>()
        {
            self.loaded_chunks.remove(&pos);
            updates.push(ServerUpdate::ChunkUnload { pos });
        }

        for pos in desired_chunks {
            if self.loaded_chunks.contains(&pos) {
                continue;
            }
            let chunk = generate_overworld_surface_chunk(self.seed, pos.x, pos.z);
            let snapshot =
                chunk.to_chunk_snapshot(ChunkRevision(self.next_revision), ChunkStatus::Surface);
            self.next_revision += 1;
            self.loaded_chunks.insert(pos);
            updates.push(ServerUpdate::ChunkSnapshot(snapshot));
        }

        updates
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
