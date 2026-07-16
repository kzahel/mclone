use std::collections::VecDeque;
#[cfg(test)]
use std::io::{Read, Write};
use std::time::{Duration, Instant};

use anyhow::Result;
#[cfg(test)]
use anyhow::{Context, bail};
use mclone_protocol::{
    ClientCommand, DisconnectReason, DisconnectReasonCode, PlayerActionKind,
    SequencedMovePlayerCommand, ServerUpdate, SessionCapabilities,
};
use mclone_server::{
    ChunkSchedulerPublicationDiagnostics, RealmServer, ServerPlayerId, ServerSimulationTickReport,
};

#[cfg(test)]
use mclone_net::{try_read_client_command_frame, write_server_update_batch};

#[cfg(test)]
pub(crate) fn serve_connection(
    stream: &mut (impl Read + Write),
    server: &mut RealmServer,
) -> Result<usize> {
    let player_id = server.add_player();
    let result = DedicatedSession::new(player_id).serve(stream, server);
    server.remove_player(player_id);
    result
}

#[derive(Debug)]
pub(crate) struct DedicatedSession {
    player_id: ServerPlayerId,
    capabilities: SessionCapabilities,
    connection: DedicatedConnectionState,
    liveness: DedicatedLiveness,
}

pub(crate) const DEDICATED_KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DedicatedCommandOutcome {
    Continue,
    Disconnect(DisconnectReason),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DedicatedLivenessAction {
    None,
    Send(ServerUpdate),
    Disconnect(DisconnectReason),
}

impl DedicatedSession {
    pub(crate) fn new(player_id: ServerPlayerId) -> Self {
        Self::new_with_capabilities(player_id, SessionCapabilities::DEVELOPMENT_DEFAULT)
    }

    pub(crate) fn new_with_capabilities(
        player_id: ServerPlayerId,
        capabilities: SessionCapabilities,
    ) -> Self {
        Self {
            player_id,
            capabilities,
            connection: DedicatedConnectionState::default(),
            liveness: DedicatedLiveness::new(Instant::now(), DEDICATED_KEEP_ALIVE_INTERVAL),
        }
    }

    pub(crate) const fn player_id(&self) -> ServerPlayerId {
        self.player_id
    }

    #[cfg(test)]
    fn serve(
        &mut self,
        stream: &mut (impl Read + Write),
        server: &mut RealmServer,
    ) -> Result<usize> {
        let mut command_count = 0;
        let mut update_count = 0;
        while let Some(command) =
            try_read_client_command_frame(stream).context("failed to read client command")?
        {
            command_count += 1;
            if let DedicatedCommandOutcome::Disconnect(_) = self
                .handle_client_command(server, command)
                .context("failed to apply client command")?
            {
                break;
            }
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
        server: &mut RealmServer,
        command: ClientCommand,
    ) -> Result<DedicatedCommandOutcome> {
        match command {
            ClientCommand::KeepAlive { id } => return Ok(self.liveness.handle_response(id)),
            ClientCommand::Disconnect(_) => {
                return Ok(DedicatedCommandOutcome::Disconnect(DisconnectReason::new(
                    DisconnectReasonCode::ClientQuit,
                    "client quit",
                )));
            }
            _ => {}
        }
        if command_requires_debug_actions(&command)
            && !self
                .capabilities
                .contains(SessionCapabilities::DEBUG_ACTIONS)
        {
            return Ok(DedicatedCommandOutcome::Disconnect(DisconnectReason::new(
                DisconnectReasonCode::ProtocolViolation,
                "client sent a debug command without the negotiated capability",
            )));
        }
        self.connection
            .handle_command(server, self.player_id, command)?;
        Ok(DedicatedCommandOutcome::Continue)
    }

    pub(crate) fn poll_liveness(&mut self, now: Instant) -> DedicatedLivenessAction {
        self.liveness.poll(now)
    }

    pub(crate) fn finish_tick_boundary(&mut self, server: &mut RealmServer) -> Result<()> {
        self.connection.flush_movement(server, self.player_id)?;
        self.connection.mark_tick_boundary();
        Ok(())
    }
}

fn command_requires_debug_actions(command: &ClientCommand) -> bool {
    match command {
        ClientCommand::SetDebugHotbarSlot(_) | ClientCommand::ShootDebugPhysicsCube => true,
        ClientCommand::PlayerAction(command) => command.kind == PlayerActionKind::DebugInstantBreak,
        _ => false,
    }
}

#[derive(Debug)]
struct DedicatedLiveness {
    interval: Duration,
    last_challenge_at: Instant,
    pending: Option<(u64, Instant)>,
    next_id: u64,
    latency: Option<Duration>,
}

impl DedicatedLiveness {
    fn new(now: Instant, interval: Duration) -> Self {
        Self {
            interval,
            last_challenge_at: now,
            pending: None,
            next_id: 1,
            latency: None,
        }
    }

    fn poll(&mut self, now: Instant) -> DedicatedLivenessAction {
        if now.saturating_duration_since(self.last_challenge_at) < self.interval {
            return DedicatedLivenessAction::None;
        }
        self.last_challenge_at = now;
        if self.pending.is_some() {
            return DedicatedLivenessAction::Disconnect(DisconnectReason::timeout(
                "client did not answer the keepalive challenge",
            ));
        }

        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.pending = Some((id, now));
        DedicatedLivenessAction::Send(ServerUpdate::KeepAlive { id })
    }

    fn handle_response(&mut self, id: u64) -> DedicatedCommandOutcome {
        let Some((expected, sent_at)) = self.pending else {
            return DedicatedCommandOutcome::Disconnect(DisconnectReason::new(
                DisconnectReasonCode::ProtocolViolation,
                "client answered a keepalive that was not pending",
            ));
        };
        if id != expected {
            return DedicatedCommandOutcome::Disconnect(DisconnectReason::new(
                DisconnectReasonCode::ProtocolViolation,
                "client answered the wrong keepalive challenge",
            ));
        }
        self.pending = None;
        self.latency = Some(Instant::now().saturating_duration_since(sent_at));
        DedicatedCommandOutcome::Continue
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
    pending_movement: VecDeque<SequencedMovePlayerCommand>,
    received_move_packet_count: u32,
    known_move_packet_count: u32,
}

impl DedicatedConnectionState {
    fn handle_command(
        &mut self,
        server: &mut RealmServer,
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

    fn stage_movement(&mut self, command: SequencedMovePlayerCommand) {
        self.received_move_packet_count = self.received_move_packet_count.saturating_add(1);
        self.pending_movement.push_back(command);
    }

    fn flush_movement(
        &mut self,
        server: &mut RealmServer,
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

    #[test]
    fn dedicated_liveness_challenges_then_times_out_one_interval_later() {
        let now = Instant::now();
        let interval = Duration::from_millis(10);
        let mut liveness = DedicatedLiveness::new(now, interval);

        assert_eq!(
            liveness.poll(now + interval - Duration::from_millis(1)),
            DedicatedLivenessAction::None
        );
        assert_eq!(
            liveness.poll(now + interval),
            DedicatedLivenessAction::Send(ServerUpdate::KeepAlive { id: 1 })
        );
        let DedicatedLivenessAction::Disconnect(reason) = liveness.poll(now + interval * 2) else {
            panic!("unanswered keepalive should disconnect");
        };
        assert_eq!(reason.code, DisconnectReasonCode::Timeout);
    }

    #[test]
    fn dedicated_liveness_accepts_only_the_pending_challenge() {
        let now = Instant::now();
        let interval = Duration::from_millis(10);
        let mut liveness = DedicatedLiveness::new(now, interval);
        assert!(matches!(
            liveness.poll(now + interval),
            DedicatedLivenessAction::Send(ServerUpdate::KeepAlive { id: 1 })
        ));
        assert_eq!(
            liveness.handle_response(1),
            DedicatedCommandOutcome::Continue
        );
        assert!(matches!(
            liveness.poll(now + interval * 2),
            DedicatedLivenessAction::Send(ServerUpdate::KeepAlive { id: 2 })
        ));

        let DedicatedCommandOutcome::Disconnect(reason) = liveness.handle_response(99) else {
            panic!("wrong keepalive response should disconnect");
        };
        assert_eq!(reason.code, DisconnectReasonCode::ProtocolViolation);
    }

    #[test]
    fn dedicated_session_rejects_debug_commands_without_capability() {
        let mut server = RealmServer::new(DEFAULT_SEED);
        let player_id = server.add_player_with_capabilities(SessionCapabilities::NONE);
        let mut session =
            DedicatedSession::new_with_capabilities(player_id, SessionCapabilities::NONE);

        let DedicatedCommandOutcome::Disconnect(reason) = session
            .handle_client_command(&mut server, ClientCommand::ShootDebugPhysicsCube)
            .unwrap()
        else {
            panic!("unnegotiated debug command should disconnect");
        };
        assert_eq!(reason.code, DisconnectReasonCode::ProtocolViolation);
    }

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
        let mut server = RealmServer::new(DEFAULT_SEED);

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
        let mut server = RealmServer::new(DEFAULT_SEED);
        let player_id = server.add_player();
        let mut connection = DedicatedConnectionState::default();

        connection
            .handle_command(
                &mut server,
                player_id,
                ClientCommand::move_player(MovePlayerCommand::Pos {
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
        let mut server = RealmServer::new(DEFAULT_SEED);
        let player_id = server.add_player();
        let mut session = DedicatedSession::new(player_id);

        session
            .connection
            .handle_command(
                &mut server,
                player_id,
                ClientCommand::move_player(MovePlayerCommand::Pos {
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
        let mut server = RealmServer::new(DEFAULT_SEED);
        let player_a = server.add_player();
        let player_b = server.add_player();
        let mut session_a = DedicatedSession::new(player_a);
        let mut session_b = DedicatedSession::new(player_b);

        session_a
            .handle_client_command(
                &mut server,
                ClientCommand::move_player(MovePlayerCommand::Pos {
                    position: Vec3d::new(1.0, 64.0, 1.0),
                    on_ground: true,
                }),
            )
            .unwrap();
        session_b
            .handle_client_command(
                &mut server,
                ClientCommand::move_player(MovePlayerCommand::Pos {
                    position: Vec3d::new(-2.0, 70.0, 3.0),
                    on_ground: false,
                }),
            )
            .unwrap();
        session_a.finish_tick_boundary(&mut server).unwrap();
        session_b.finish_tick_boundary(&mut server).unwrap();

        assert_eq!(
            server.player_position(player_a),
            Some(Vec3d::new(1.0, 64.0, 1.0))
        );
        assert_eq!(
            server.player_position(player_b),
            Some(Vec3d::new(-2.0, 70.0, 3.0))
        );
    }

    #[test]
    fn dedicated_sessions_keep_independent_chunk_views() {
        let mut server = RealmServer::new(DEFAULT_SEED);
        server.set_lighting_enabled(false);
        let player_a = server.add_player();
        let player_b = server.add_player();
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
            &ClientCommand::move_player(MovePlayerCommand::Pos {
                position: Vec3d::new(1.0, 64.0, 1.0),
                on_ground: true,
            }),
        )
        .unwrap();
        write_client_command_frame(
            &mut request,
            &ClientCommand::move_player(MovePlayerCommand::Rot {
                y_rot_degrees: 90.0,
                x_rot_degrees: 15.0,
                on_ground: true,
            }),
        )
        .unwrap();
        let mut stream = ScriptedStream::new(request);
        let mut server = RealmServer::new(DEFAULT_SEED);

        assert_eq!(serve_connection(&mut stream, &mut server).unwrap(), 5);

        let written = stream.written();
        let mut publications = std::io::Cursor::new(written);
        let first = read_server_update_batch(&mut publications).unwrap();
        let second = read_server_update_batch(&mut publications).unwrap();
        assert!(matches!(
            first.first(),
            Some(ServerUpdate::SessionConfiguration(_))
        ));
        assert!(matches!(first.get(1), Some(ServerUpdate::SessionReady)));
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
        server: &mut RealmServer,
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
