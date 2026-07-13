use std::collections::VecDeque;
#[cfg(test)]
use std::io::{Read, Write};

use anyhow::Result;
#[cfg(test)]
use anyhow::{Context, bail};
use mclone_protocol::{ClientCommand, MovePlayerCommand};
use mclone_server::{
    ChunkSchedulerPublicationDiagnostics, IntegratedServer, ServerPlayerId,
    ServerSimulationTickReport,
};

#[cfg(test)]
use mclone_net::{try_read_client_command_frame, write_server_update_batch};

#[cfg(test)]
pub(crate) fn serve_connection(
    stream: &mut (impl Read + Write),
    server: &mut IntegratedServer,
) -> Result<usize> {
    let player_id = server.add_dedicated_player();
    let result = DedicatedSession::new(player_id).serve(stream, server);
    server.remove_dedicated_player(player_id);
    result
}

#[derive(Debug)]
pub(crate) struct DedicatedSession {
    player_id: ServerPlayerId,
    connection: DedicatedConnectionState,
}

impl DedicatedSession {
    pub(crate) fn new(player_id: ServerPlayerId) -> Self {
        Self {
            player_id,
            connection: DedicatedConnectionState::default(),
        }
    }

    pub(crate) const fn player_id(&self) -> ServerPlayerId {
        self.player_id
    }

    #[cfg(test)]
    fn serve(
        &mut self,
        stream: &mut (impl Read + Write),
        server: &mut IntegratedServer,
    ) -> Result<usize> {
        let mut command_count = 0;
        let mut update_count = 0;
        while let Some(command) =
            try_read_client_command_frame(stream).context("failed to read client command")?
        {
            command_count += 1;
            self.handle_client_command(server, command)
                .context("failed to apply client command")?;
            self.finish_tick_boundary(server)?;
            server
                .try_simulation_tick_report_global()
                .context("failed to advance compatibility server tick")?;
            let updates = server
                .try_drain_updates_for_player(self.player_id)
                .context("failed to drain compatibility server updates")?;
            update_count += updates.len();
            write_server_update_batch(stream, &updates)
                .context("failed to write server update batch")?;
        }

        if command_count == 0 {
            bail!("connection closed without client command");
        }
        Ok(update_count)
    }

    pub(crate) fn handle_client_command(
        &mut self,
        server: &mut IntegratedServer,
        command: ClientCommand,
    ) -> Result<()> {
        self.connection
            .handle_command(server, self.player_id, command)
    }

    pub(crate) fn finish_tick_boundary(&mut self, server: &mut IntegratedServer) -> Result<()> {
        self.connection.flush_movement(server, self.player_id)?;
        self.connection.mark_tick_boundary();
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DedicatedSessionDiagnostics {
    pub simulation_tick: u64,
    pub tick_total_us: u128,
    pub scheduler_tick_us: u128,
    pub scheduler_publish_completed_us: u128,
    pub publication: ChunkSchedulerPublicationDiagnostics,
    pub pending_jobs_after: usize,
    pub pending_publications_after: usize,
}

impl DedicatedSessionDiagnostics {
    pub(crate) fn from_report(
        report: &ServerSimulationTickReport,
        pending_jobs_after: usize,
        pending_publications_after: usize,
    ) -> Self {
        Self {
            simulation_tick: report.simulation_tick,
            tick_total_us: report.timing.total_us,
            scheduler_tick_us: report.timing.scheduler_tick_us,
            scheduler_publish_completed_us: report.timing.scheduler_publish_completed_us,
            publication: report.scheduler_publication.clone(),
            pending_jobs_after,
            pending_publications_after,
        }
    }
}

#[derive(Debug, Default)]
struct DedicatedConnectionState {
    pending_movement: VecDeque<MovePlayerCommand>,
    received_move_packet_count: u32,
    known_move_packet_count: u32,
}

impl DedicatedConnectionState {
    fn handle_command(
        &mut self,
        server: &mut IntegratedServer,
        player_id: ServerPlayerId,
        command: ClientCommand,
    ) -> Result<()> {
        match command {
            ClientCommand::MovePlayer(command) => {
                self.stage_movement(command);
                Ok(())
            }
            command => {
                self.flush_movement(server, player_id)?;
                server.try_enqueue_command_for_player(player_id, command)?;
                Ok(())
            }
        }
    }

    fn stage_movement(&mut self, command: MovePlayerCommand) {
        self.received_move_packet_count = self.received_move_packet_count.saturating_add(1);
        self.pending_movement.push_back(command);
    }

    fn flush_movement(
        &mut self,
        server: &mut IntegratedServer,
        player_id: ServerPlayerId,
    ) -> Result<()> {
        while let Some(command) = self.pending_movement.pop_front() {
            server.try_enqueue_command_for_player(player_id, ClientCommand::MovePlayer(command))?;
        }
        Ok(())
    }

    fn mark_tick_boundary(&mut self) {
        self.known_move_packet_count = self.received_move_packet_count;
    }

    #[cfg(test)]
    fn buffered_move_packet_count(&self) -> usize {
        self.pending_movement.len()
    }

    #[cfg(test)]
    const fn received_move_packet_count(&self) -> u32 {
        self.received_move_packet_count
    }

    #[cfg(test)]
    const fn known_move_packet_count(&self) -> u32 {
        self.known_move_packet_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{ChunkPos, Vec3d};
    use mclone_net::{read_server_update_batch, write_client_command_frame};
    use mclone_protocol::{
        ChunkView, ClientCommand, MovePlayerCommand, ServerUpdate, SetCarriedItemCommand,
    };

    const DEFAULT_SEED: i64 = 12345;

    #[derive(Debug)]
    struct ScriptedStream {
        read: std::io::Cursor<Vec<u8>>,
        written: Vec<u8>,
    }

    impl ScriptedStream {
        fn new(read: Vec<u8>) -> Self {
            Self {
                read: std::io::Cursor::new(read),
                written: Vec::new(),
            }
        }

        fn written(self) -> Vec<u8> {
            self.written
        }
    }

    impl Read for ScriptedStream {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.read.read(buf)
        }
    }

    impl Write for ScriptedStream {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn serve_connection_does_not_wait_for_chunk_snapshot() {
        let mut request = Vec::new();
        write_client_command_frame(
            &mut request,
            &ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        )
        .unwrap();
        let mut stream = ScriptedStream::new(request);
        let mut server = IntegratedServer::new(DEFAULT_SEED);

        let update_count = serve_connection(&mut stream, &mut server).unwrap();

        let updates =
            read_server_update_batch(&mut std::io::Cursor::new(stream.written())).unwrap();
        assert_eq!(updates.len(), update_count);
        assert!(
            updates
                .iter()
                .all(|update| !matches!(update, ServerUpdate::ChunkSnapshot(_)))
        );
        assert!(server.pending_job_count() > 0);
        assert!(
            updates
                .iter()
                .any(|update| matches!(update, ServerUpdate::TimeUpdate { .. }))
        );
    }

    #[test]
    fn dedicated_connection_buffers_movement_until_flush_or_ordered_command() {
        let mut server = IntegratedServer::new(DEFAULT_SEED);
        let player_id = server.add_dedicated_player();
        let mut connection = DedicatedConnectionState::default();

        connection
            .handle_command(
                &mut server,
                player_id,
                ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                    position: Vec3d::new(1.0, 64.0, 1.0),
                    on_ground: true,
                }),
            )
            .unwrap();

        assert_eq!(connection.buffered_move_packet_count(), 1);
        assert_eq!(connection.received_move_packet_count(), 1);
        assert_eq!(connection.known_move_packet_count(), 0);

        connection
            .handle_command(
                &mut server,
                player_id,
                ClientCommand::SetCarriedItem(SetCarriedItemCommand { slot: 1 }),
            )
            .unwrap();

        assert_eq!(connection.buffered_move_packet_count(), 0);
        assert_eq!(connection.received_move_packet_count(), 1);
        assert_eq!(connection.known_move_packet_count(), 0);
    }

    #[test]
    fn dedicated_session_tick_marks_connection_movement_boundary() {
        let mut server = IntegratedServer::new(DEFAULT_SEED);
        let player_id = server.add_dedicated_player();
        let mut session = DedicatedSession::new(player_id);

        session
            .connection
            .handle_command(
                &mut server,
                player_id,
                ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                    position: Vec3d::new(1.0, 64.0, 1.0),
                    on_ground: true,
                }),
            )
            .unwrap();

        assert_eq!(session.connection.received_move_packet_count(), 1);
        assert_eq!(session.connection.known_move_packet_count(), 0);

        session.finish_tick_boundary(&mut server).unwrap();

        assert_eq!(session.connection.received_move_packet_count(), 1);
        assert_eq!(session.connection.known_move_packet_count(), 1);
    }

    #[test]
    fn dedicated_sessions_route_movement_to_assigned_server_players() {
        let mut server = IntegratedServer::new(DEFAULT_SEED);
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();
        let mut session_a = DedicatedSession::new(player_a);
        let mut session_b = DedicatedSession::new(player_b);

        session_a
            .handle_client_command(
                &mut server,
                ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                    position: Vec3d::new(1.0, 64.0, 1.0),
                    on_ground: true,
                }),
            )
            .unwrap();
        session_b
            .handle_client_command(
                &mut server,
                ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                    position: Vec3d::new(-2.0, 70.0, 3.0),
                    on_ground: false,
                }),
            )
            .unwrap();
        session_a.finish_tick_boundary(&mut server).unwrap();
        session_b.finish_tick_boundary(&mut server).unwrap();

        assert_eq!(
            server.dedicated_player_position(player_a),
            Some(Vec3d::new(1.0, 64.0, 1.0))
        );
        assert_eq!(
            server.dedicated_player_position(player_b),
            Some(Vec3d::new(-2.0, 70.0, 3.0))
        );
    }

    #[test]
    fn dedicated_sessions_keep_independent_chunk_views() {
        let mut server = IntegratedServer::new(DEFAULT_SEED);
        server.set_lighting_enabled(false);
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();
        let mut session_a = DedicatedSession::new(player_a);
        let mut session_b = DedicatedSession::new(player_b);

        session_a
            .handle_client_command(
                &mut server,
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }),
            )
            .unwrap();
        session_b
            .handle_client_command(
                &mut server,
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(4, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }),
            )
            .unwrap();
        session_a.finish_tick_boundary(&mut server).unwrap();
        session_b.finish_tick_boundary(&mut server).unwrap();
        let [updates_a, updates_b] =
            collect_player_updates_until_idle(&mut server, [player_a, player_b]);

        assert!(has_snapshot(&updates_a, ChunkPos::new(0, 0)));
        assert!(!has_snapshot(&updates_a, ChunkPos::new(4, 0)));
        assert!(has_world_info(&updates_a));
        assert!(has_snapshot(&updates_b, ChunkPos::new(4, 0)));
        assert!(!has_snapshot(&updates_b, ChunkPos::new(0, 0)));
        assert!(has_world_info(&updates_b));

        session_a
            .handle_client_command(
                &mut server,
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(1, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }),
            )
            .unwrap();
        session_b
            .handle_client_command(
                &mut server,
                ClientCommand::SetCarriedItem(SetCarriedItemCommand { slot: 1 }),
            )
            .unwrap();
        session_a.finish_tick_boundary(&mut server).unwrap();
        session_b.finish_tick_boundary(&mut server).unwrap();
        let [updates_a, updates_b] =
            collect_player_updates_until_idle(&mut server, [player_a, player_b]);

        assert!(has_unload(&updates_a, ChunkPos::new(0, 0)));
        assert!(has_snapshot(&updates_a, ChunkPos::new(1, 0)));
        assert!(!has_unload(&updates_b, ChunkPos::new(4, 0)));
        assert!(!has_snapshot(&updates_b, ChunkPos::new(1, 0)));
    }

    #[test]
    fn serve_connection_accepts_persistent_client_command_frames() {
        let mut request = Vec::new();
        write_client_command_frame(
            &mut request,
            &ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                position: Vec3d::new(1.0, 64.0, 1.0),
                on_ground: true,
            }),
        )
        .unwrap();
        write_client_command_frame(
            &mut request,
            &ClientCommand::MovePlayer(MovePlayerCommand::Rot {
                y_rot_degrees: 90.0,
                x_rot_degrees: 15.0,
                on_ground: true,
            }),
        )
        .unwrap();
        let mut stream = ScriptedStream::new(request);
        let mut server = IntegratedServer::new(DEFAULT_SEED);

        assert_eq!(serve_connection(&mut stream, &mut server).unwrap(), 3);

        let written = stream.written();
        let mut publications = std::io::Cursor::new(written);
        let first = read_server_update_batch(&mut publications).unwrap();
        let second = read_server_update_batch(&mut publications).unwrap();
        assert!(
            first
                .iter()
                .any(|update| matches!(update, ServerUpdate::WorldInfo { .. }))
        );
        assert!(
            first
                .iter()
                .any(|update| matches!(update, ServerUpdate::TimeUpdate { .. }))
        );
        assert!(second.is_empty());
    }

    fn has_snapshot(updates: &[ServerUpdate], pos: ChunkPos) -> bool {
        updates.iter().any(
            |update| matches!(update, ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == pos),
        )
    }

    fn has_unload(updates: &[ServerUpdate], pos: ChunkPos) -> bool {
        updates
            .iter()
            .any(|update| matches!(update, ServerUpdate::ChunkUnload { pos: unloaded } if *unloaded == pos))
    }

    fn has_world_info(updates: &[ServerUpdate]) -> bool {
        updates
            .iter()
            .any(|update| matches!(update, ServerUpdate::WorldInfo { .. }))
    }

    fn collect_player_updates_until_idle<const N: usize>(
        server: &mut IntegratedServer,
        players: [ServerPlayerId; N],
    ) -> [Vec<ServerUpdate>; N] {
        let mut collected = std::array::from_fn(|_| Vec::new());
        for _ in 0..60_000 {
            server
                .try_simulation_tick_report_global()
                .expect("advance dedicated test host");
            for (index, player_id) in players.iter().copied().enumerate() {
                collected[index].extend(
                    server
                        .try_drain_updates_for_player(player_id)
                        .expect("drain dedicated test player"),
                );
            }
            if server.pending_job_count() == 0 {
                return collected;
            }
            std::thread::yield_now();
        }
        panic!("timed out advancing dedicated test host until jobs completed");
    }
}
