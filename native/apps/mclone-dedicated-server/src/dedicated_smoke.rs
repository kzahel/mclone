use std::collections::BTreeMap;
use std::net::{SocketAddr, TcpListener};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockHitResult, BlockPos, BlockStateId, ChunkPos, ChunkSnapshot, Direction,
    SECTION_HEIGHT, Vec3d, block_to_section_coord, chunk_section_index, local_block_coord,
    local_section_block_coord,
};
use mclone_net::NativeClientSession;
use mclone_protocol::{
    AcceptTeleportCommand, ChunkView, ClientCommand, InteractionHand, MovePlayerCommand,
    PlayerActionCommand, PlayerActionKind, ServerUpdate, SetCarriedItemCommand, UseItemOnCommand,
};
use mclone_server::{IntegratedServer, PlayerChunkTrackingDiagnostics};

use crate::connection::{DedicatedConnectionId, DedicatedNetwork, DedicatedNetworkEvent};
use crate::session::DedicatedSession;

const CLIENT_COUNT: usize = 2;
const EVENT_TIMEOUT: Duration = Duration::from_secs(30);
const CLIENT_REPORT_TIMEOUT: Duration = Duration::from_secs(30);
const DIRT_BLOCK_STATE_ID: BlockStateId = BlockStateId(5);

#[derive(Debug)]
enum SmokeClientControl {
    Command(ClientCommand),
    Close,
}

#[derive(Debug)]
struct SmokeClientReport {
    index: usize,
    updates: Vec<ServerUpdate>,
}

#[derive(Debug)]
struct PhaseReport {
    name: &'static str,
    diagnostics: PlayerChunkTrackingDiagnostics,
    client_reports: Vec<SmokeClientReport>,
}

#[derive(Clone, Copy, Debug)]
struct PlacementTarget {
    clicked: BlockPos,
    placed: BlockPos,
}

pub(crate) fn run_multi_client_smoke(seed: i64) -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .context("failed to bind dedicated multi-client smoke listener")?;
    let addr = listener
        .local_addr()
        .context("failed to read dedicated multi-client smoke listen addr")?;
    let network = DedicatedNetwork::start(listener)?;
    let mut smoke_server = SmokeServer::new(seed, network);

    let (ready_tx, ready_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let mut controls = Vec::new();
    let mut clients = Vec::new();
    for index in 0..CLIENT_COUNT {
        let (control_tx, control_rx) = mpsc::channel();
        controls.push(control_tx);
        clients.push(spawn_smoke_client(
            index,
            addr,
            control_rx,
            ready_tx.clone(),
            result_tx.clone(),
        ));
    }
    drop(ready_tx);
    drop(result_tx);

    wait_for_ready_clients(&ready_rx)?;
    let mut phases = Vec::new();

    send_phase_commands(
        &controls,
        vec![
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(4, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
        ],
    )?;
    let phase_one_diagnostics = smoke_server.process_commands(CLIENT_COUNT)?;
    let phase_one_reports = wait_for_client_reports(&result_rx)?;
    assert_phase_one(&phase_one_diagnostics, &phase_one_reports)?;
    let spawn_ack_commands = teleport_ack_commands(&phase_one_reports)?;
    phases.push(PhaseReport {
        name: "initial_disjoint_views",
        diagnostics: phase_one_diagnostics,
        client_reports: phase_one_reports,
    });

    send_indexed_phase_commands(&controls, spawn_ack_commands)?;
    let spawn_ack_diagnostics = smoke_server.process_commands(CLIENT_COUNT)?;
    let spawn_ack_reports = wait_for_client_reports(&result_rx)?;
    assert_spawn_ack_phase(&spawn_ack_diagnostics, &spawn_ack_reports)?;
    phases.push(PhaseReport {
        name: "clients_accept_spawn_teleports",
        diagnostics: spawn_ack_diagnostics,
        client_reports: spawn_ack_reports,
    });

    send_phase_commands(
        &controls,
        vec![
            ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(1, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }),
            ClientCommand::SetCarriedItem(SetCarriedItemCommand { slot: 1 }),
        ],
    )?;
    let phase_two_diagnostics = smoke_server.process_commands(CLIENT_COUNT)?;
    let phase_two_reports = wait_for_client_reports(&result_rx)?;
    assert_phase_two(&phase_two_diagnostics, &phase_two_reports)?;
    let break_targets = break_targets_from_snapshot(
        client_report(&phase_two_reports, 0)?,
        ChunkPos::new(1, 0),
        2,
    )?;
    let placement_target = placement_target_from_snapshot(
        client_report(&phase_two_reports, 0)?,
        ChunkPos::new(1, 0),
        &break_targets,
    )?;
    phases.push(PhaseReport {
        name: "one_client_moves_view",
        diagnostics: phase_two_diagnostics,
        client_reports: phase_two_reports,
    });

    let non_overlapping_target = break_targets[0];
    send_client_command(
        &controls,
        0,
        move_near_block_command(non_overlapping_target),
    )?;
    let actor_move_diagnostics = smoke_server.process_commands(1)?;
    let actor_move_reports = wait_for_client_reports_count(&result_rx, 1)?;
    assert_actor_move_phase(&actor_move_diagnostics, &actor_move_reports, 2)?;
    phases.push(PhaseReport {
        name: "actor_moves_for_non_overlapping_delta",
        diagnostics: actor_move_diagnostics,
        client_reports: actor_move_reports,
    });

    send_client_command(&controls, 0, break_block_command(non_overlapping_target))?;
    let _non_overlap_break_diagnostics = smoke_server.process_commands(1)?;
    let actor_break_reports = wait_for_client_reports_count(&result_rx, 1)?;
    send_client_command(&controls, 1, poll_command(2))?;
    let non_overlap_poll_diagnostics = smoke_server.process_commands(1)?;
    let observer_poll_reports = wait_for_client_reports_count(&result_rx, 1)?;
    let non_overlap_reports = merge_phase_reports(actor_break_reports, observer_poll_reports);
    assert_non_overlapping_block_delta_phase(
        &non_overlap_poll_diagnostics,
        &non_overlap_reports,
        non_overlapping_target,
    )?;
    phases.push(PhaseReport {
        name: "non_overlapping_block_delta",
        diagnostics: non_overlap_poll_diagnostics,
        client_reports: non_overlap_reports,
    });

    send_client_command(
        &controls,
        1,
        ClientCommand::SetChunkView(ChunkView {
            center: ChunkPos::new(1, 0),
            render_distance: 0,
            chunk_tracking_radius: 0,
        }),
    )?;
    let overlap_setup_diagnostics = smoke_server.process_commands(1)?;
    let overlap_setup_reports = wait_for_client_reports_count(&result_rx, 1)?;
    assert_overlap_setup_phase(&overlap_setup_diagnostics, &overlap_setup_reports)?;
    phases.push(PhaseReport {
        name: "observer_overlaps_actor_chunk",
        diagnostics: overlap_setup_diagnostics,
        client_reports: overlap_setup_reports,
    });

    let overlapping_target = break_targets[1];
    send_client_command(&controls, 0, move_near_block_command(overlapping_target))?;
    let overlap_move_diagnostics = smoke_server.process_commands(1)?;
    let overlap_move_reports = wait_for_client_reports_count(&result_rx, 1)?;
    assert_actor_move_phase(&overlap_move_diagnostics, &overlap_move_reports, 1)?;
    phases.push(PhaseReport {
        name: "actor_moves_for_overlapping_delta",
        diagnostics: overlap_move_diagnostics,
        client_reports: overlap_move_reports,
    });

    send_client_command(&controls, 0, break_block_command(overlapping_target))?;
    let _overlap_break_diagnostics = smoke_server.process_commands(1)?;
    let actor_overlap_break_reports = wait_for_client_reports_count(&result_rx, 1)?;
    send_client_command(&controls, 1, poll_command(3))?;
    let overlap_poll_diagnostics = smoke_server.process_commands(1)?;
    let observer_overlap_poll_reports = wait_for_client_reports_count(&result_rx, 1)?;
    let overlap_reports =
        merge_phase_reports(actor_overlap_break_reports, observer_overlap_poll_reports);
    assert_overlapping_block_delta_phase(
        &overlap_poll_diagnostics,
        &overlap_reports,
        overlapping_target,
    )?;
    phases.push(PhaseReport {
        name: "overlapping_block_delta",
        diagnostics: overlap_poll_diagnostics,
        client_reports: overlap_reports,
    });

    send_client_command(
        &controls,
        0,
        move_near_block_command(placement_target.clicked),
    )?;
    let place_move_diagnostics = smoke_server.process_commands(1)?;
    let place_move_reports = wait_for_client_reports_count(&result_rx, 1)?;
    assert_actor_move_phase(&place_move_diagnostics, &place_move_reports, 1)?;
    phases.push(PhaseReport {
        name: "actor_moves_for_place",
        diagnostics: place_move_diagnostics,
        client_reports: place_move_reports,
    });

    send_client_command(&controls, 0, poll_command(8))?;
    let empty_slot_diagnostics = smoke_server.process_commands(1)?;
    let empty_slot_reports = wait_for_client_reports_count(&result_rx, 1)?;
    assert_no_delta_phase(&empty_slot_diagnostics, &empty_slot_reports, 1)?;
    phases.push(PhaseReport {
        name: "actor_selects_empty_slot",
        diagnostics: empty_slot_diagnostics,
        client_reports: empty_slot_reports,
    });

    send_client_command(&controls, 0, use_item_on_command(placement_target.clicked))?;
    let _empty_place_diagnostics = smoke_server.process_commands(1)?;
    let actor_empty_place_reports = wait_for_client_reports_count(&result_rx, 1)?;
    send_client_command(&controls, 1, poll_command(4))?;
    let empty_place_poll_diagnostics = smoke_server.process_commands(1)?;
    let observer_empty_place_reports = wait_for_client_reports_count(&result_rx, 1)?;
    let empty_place_reports =
        merge_phase_reports(actor_empty_place_reports, observer_empty_place_reports);
    assert_no_delta_phase(&empty_place_poll_diagnostics, &empty_place_reports, 1)?;
    phases.push(PhaseReport {
        name: "empty_slot_place_rejected",
        diagnostics: empty_place_poll_diagnostics,
        client_reports: empty_place_reports,
    });

    send_phase_commands(
        &controls,
        vec![
            ClientCommand::SetCarriedItem(SetCarriedItemCommand { slot: 1 }),
            ClientCommand::SetCarriedItem(SetCarriedItemCommand { slot: 8 }),
        ],
    )?;
    let selected_slot_diagnostics = smoke_server.process_commands(CLIENT_COUNT)?;
    let selected_slot_reports = wait_for_client_reports(&result_rx)?;
    assert_no_delta_phase(&selected_slot_diagnostics, &selected_slot_reports, 1)?;
    phases.push(PhaseReport {
        name: "per_player_selected_slots",
        diagnostics: selected_slot_diagnostics,
        client_reports: selected_slot_reports,
    });

    send_client_command(&controls, 0, move_far_from_block_command())?;
    let far_move_diagnostics = smoke_server.process_commands(1)?;
    let far_move_reports = wait_for_client_reports_count(&result_rx, 1)?;
    assert_actor_move_phase(&far_move_diagnostics, &far_move_reports, 1)?;
    phases.push(PhaseReport {
        name: "actor_moves_out_of_reach",
        diagnostics: far_move_diagnostics,
        client_reports: far_move_reports,
    });

    send_client_command(&controls, 0, use_item_on_command(placement_target.clicked))?;
    let _far_place_diagnostics = smoke_server.process_commands(1)?;
    let actor_far_place_reports = wait_for_client_reports_count(&result_rx, 1)?;
    send_client_command(&controls, 1, poll_command(4))?;
    let far_place_poll_diagnostics = smoke_server.process_commands(1)?;
    let observer_far_place_reports = wait_for_client_reports_count(&result_rx, 1)?;
    let far_place_reports =
        merge_phase_reports(actor_far_place_reports, observer_far_place_reports);
    assert_no_delta_phase(&far_place_poll_diagnostics, &far_place_reports, 1)?;
    phases.push(PhaseReport {
        name: "far_place_rejected",
        diagnostics: far_place_poll_diagnostics,
        client_reports: far_place_reports,
    });

    send_client_command(
        &controls,
        0,
        move_near_block_command(placement_target.clicked),
    )?;
    let near_place_move_diagnostics = smoke_server.process_commands(1)?;
    let near_place_move_reports = wait_for_client_reports_count(&result_rx, 1)?;
    assert_actor_move_phase(&near_place_move_diagnostics, &near_place_move_reports, 1)?;
    phases.push(PhaseReport {
        name: "actor_returns_for_place",
        diagnostics: near_place_move_diagnostics,
        client_reports: near_place_move_reports,
    });

    send_client_command(&controls, 0, use_item_on_command(placement_target.clicked))?;
    let _place_diagnostics = smoke_server.process_commands(1)?;
    let actor_place_reports = wait_for_client_reports_count(&result_rx, 1)?;
    send_client_command(&controls, 1, poll_command(4))?;
    let place_poll_diagnostics = smoke_server.process_commands(1)?;
    let observer_place_reports = wait_for_client_reports_count(&result_rx, 1)?;
    let place_reports = merge_phase_reports(actor_place_reports, observer_place_reports);
    assert_place_block_delta_phase(&place_poll_diagnostics, &place_reports, placement_target)?;
    phases.push(PhaseReport {
        name: "overlapping_place_delta",
        diagnostics: place_poll_diagnostics,
        client_reports: place_reports,
    });

    for control in controls {
        let _ = control.send(SmokeClientControl::Close);
    }
    smoke_server.drain_disconnects(CLIENT_COUNT)?;
    join_clients(clients)?;

    print_smoke_report(seed, &phases);

    Ok(())
}

#[derive(Debug)]
struct SmokeServer {
    network: DedicatedNetwork,
    server: IntegratedServer,
    sessions: BTreeMap<DedicatedConnectionId, DedicatedSession>,
    disconnected_connections: usize,
}

impl SmokeServer {
    fn new(seed: i64, network: DedicatedNetwork) -> Self {
        let mut server = IntegratedServer::new(seed);
        server.set_lighting_enabled(false);
        Self {
            network,
            server,
            sessions: BTreeMap::new(),
            disconnected_connections: 0,
        }
    }

    fn process_commands(&mut self, command_count: usize) -> Result<PlayerChunkTrackingDiagnostics> {
        let mut processed_commands = 0;
        let mut last_diagnostics = None;
        while processed_commands < command_count {
            let event = self.recv_event()?;
            if let Some(diagnostics) = self.handle_event(event, false)? {
                processed_commands += 1;
                last_diagnostics = Some(diagnostics);
            }
        }

        last_diagnostics.context("smoke phase did not process any client commands")
    }

    fn drain_disconnects(&mut self, expected: usize) -> Result<()> {
        let initial_disconnects = self.disconnected_connections;
        while self.disconnected_connections - initial_disconnects < expected {
            let event = self.recv_event()?;
            self.handle_event(event, true)?;
        }
        Ok(())
    }

    fn recv_event(&self) -> Result<DedicatedNetworkEvent> {
        self.network
            .recv_timeout(EVENT_TIMEOUT)?
            .context("timed out waiting for dedicated smoke network event")
    }

    fn handle_event(
        &mut self,
        event: DedicatedNetworkEvent,
        allow_disconnect: bool,
    ) -> Result<Option<PlayerChunkTrackingDiagnostics>> {
        match event {
            DedicatedNetworkEvent::Connected { id, .. } => {
                let player_id = self.server.add_dedicated_player();
                self.sessions.insert(id, DedicatedSession::new(player_id));
                Ok(None)
            }
            DedicatedNetworkEvent::Command {
                id,
                command,
                response,
                ..
            } => {
                let Some(session) = self.sessions.get_mut(&id) else {
                    let message = format!("received smoke command for disconnected client {id}");
                    let _ = response.send(Err(message.clone()));
                    bail!(message);
                };
                let updates = session
                    .handle_client_command(&mut self.server, command)
                    .context("failed to apply dedicated smoke client command")?;
                response.send(Ok(updates)).map_err(|_| {
                    anyhow::anyhow!("smoke client {id} disconnected before response")
                })?;
                Ok(Some(self.server.chunk_tracking_diagnostics()))
            }
            DedicatedNetworkEvent::Disconnected {
                id,
                peer_addr,
                command_count,
                reason,
            } => {
                if let Some(session) = self.sessions.remove(&id) {
                    self.server.remove_dedicated_player(session.player_id());
                }
                self.disconnected_connections += 1;
                if !allow_disconnect {
                    let reason = reason.unwrap_or_else(|| "connection closed".to_owned());
                    bail!(
                        "smoke client {id} {peer_addr} disconnected after {command_count} commands before phase completed: {reason}"
                    );
                }
                Ok(None)
            }
            DedicatedNetworkEvent::AcceptFailed { message } => bail!(message),
        }
    }
}

fn spawn_smoke_client(
    index: usize,
    addr: SocketAddr,
    control_rx: Receiver<SmokeClientControl>,
    ready_tx: Sender<std::result::Result<usize, String>>,
    result_tx: Sender<std::result::Result<SmokeClientReport, String>>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("mclone-dedicated-smoke-client-{index}"))
        .spawn(move || {
            let mut session = match NativeClientSession::connect(addr) {
                Ok(session) => {
                    let _ = ready_tx.send(Ok(index));
                    session
                }
                Err(err) => {
                    let _ = ready_tx.send(Err(format!(
                        "smoke client {index} failed to connect to {addr}: {err}"
                    )));
                    return;
                }
            };

            while let Ok(control) = control_rx.recv() {
                match control {
                    SmokeClientControl::Command(command) => {
                        let result = session
                            .send_command(&command)
                            .map(|updates| SmokeClientReport { index, updates })
                            .map_err(|err| format!("smoke client {index} command failed: {err}"));
                        let should_stop = result.is_err();
                        if result_tx.send(result).is_err() || should_stop {
                            return;
                        }
                    }
                    SmokeClientControl::Close => return,
                }
            }
        })
        .expect("failed to spawn dedicated smoke client thread")
}

fn wait_for_ready_clients(ready_rx: &Receiver<std::result::Result<usize, String>>) -> Result<()> {
    let mut ready_clients = Vec::new();
    while ready_clients.len() < CLIENT_COUNT {
        match ready_rx.recv_timeout(EVENT_TIMEOUT) {
            Ok(Ok(index)) => ready_clients.push(index),
            Ok(Err(message)) => bail!(message),
            Err(err) => bail!("timed out waiting for smoke clients to connect: {err}"),
        }
    }
    ready_clients.sort_unstable();
    if ready_clients != (0..CLIENT_COUNT).collect::<Vec<_>>() {
        bail!("unexpected smoke client readiness set: {ready_clients:?}");
    }
    Ok(())
}

fn send_phase_commands(
    controls: &[Sender<SmokeClientControl>],
    commands: Vec<ClientCommand>,
) -> Result<()> {
    if controls.len() != commands.len() {
        bail!(
            "smoke command count {} did not match client count {}",
            commands.len(),
            controls.len()
        );
    }
    for (control, command) in controls.iter().zip(commands) {
        control
            .send(SmokeClientControl::Command(command))
            .context("failed to send smoke client command")?;
    }
    Ok(())
}

fn send_client_command(
    controls: &[Sender<SmokeClientControl>],
    index: usize,
    command: ClientCommand,
) -> Result<()> {
    controls
        .get(index)
        .with_context(|| format!("missing smoke client control {index}"))?
        .send(SmokeClientControl::Command(command))
        .with_context(|| format!("failed to send smoke client {index} command"))
}

fn send_indexed_phase_commands(
    controls: &[Sender<SmokeClientControl>],
    commands: Vec<(usize, ClientCommand)>,
) -> Result<()> {
    for (index, command) in commands {
        send_client_command(controls, index, command)?;
    }
    Ok(())
}

fn wait_for_client_reports(
    result_rx: &Receiver<std::result::Result<SmokeClientReport, String>>,
) -> Result<Vec<SmokeClientReport>> {
    wait_for_client_reports_count(result_rx, CLIENT_COUNT)
}

fn wait_for_client_reports_count(
    result_rx: &Receiver<std::result::Result<SmokeClientReport, String>>,
    count: usize,
) -> Result<Vec<SmokeClientReport>> {
    let mut reports = Vec::new();
    while reports.len() < count {
        match result_rx.recv_timeout(CLIENT_REPORT_TIMEOUT) {
            Ok(Ok(report)) => reports.push(report),
            Ok(Err(message)) => bail!(message),
            Err(err) => bail!("timed out waiting for smoke client reports: {err}"),
        }
    }
    reports.sort_by_key(|report| report.index);
    Ok(reports)
}

fn merge_phase_reports(
    mut first: Vec<SmokeClientReport>,
    second: Vec<SmokeClientReport>,
) -> Vec<SmokeClientReport> {
    first.extend(second);
    first.sort_by_key(|report| report.index);
    first
}

fn join_clients(clients: Vec<JoinHandle<()>>) -> Result<()> {
    for client in clients {
        client
            .join()
            .map_err(|_| anyhow::anyhow!("dedicated smoke client thread panicked"))?;
    }
    Ok(())
}

fn assert_phase_one(
    diagnostics: &PlayerChunkTrackingDiagnostics,
    reports: &[SmokeClientReport],
) -> Result<()> {
    assert_tracking_diagnostics(diagnostics, 2, 2)?;
    let first = client_report(reports, 0)?;
    let second = client_report(reports, 1)?;

    ensure_snapshot_only(first, ChunkPos::new(0, 0), ChunkPos::new(4, 0))?;
    ensure_snapshot_only(second, ChunkPos::new(4, 0), ChunkPos::new(0, 0))?;
    Ok(())
}

fn assert_phase_two(
    diagnostics: &PlayerChunkTrackingDiagnostics,
    reports: &[SmokeClientReport],
) -> Result<()> {
    assert_tracking_diagnostics(diagnostics, 2, 2)?;
    let moved = client_report(reports, 0)?;
    let stationary = client_report(reports, 1)?;

    if !has_unload(&moved.updates, ChunkPos::new(0, 0)) {
        bail!("moving smoke client did not receive unload for its old chunk");
    }
    if !has_snapshot(&moved.updates, ChunkPos::new(1, 0)) {
        bail!("moving smoke client did not receive snapshot for its new chunk");
    }
    if has_unload(&stationary.updates, ChunkPos::new(4, 0)) {
        bail!("stationary smoke client received unload for its still-tracked chunk");
    }
    if has_snapshot(&stationary.updates, ChunkPos::new(1, 0)) {
        bail!("stationary smoke client received moving client's new chunk snapshot");
    }
    Ok(())
}

fn assert_spawn_ack_phase(
    diagnostics: &PlayerChunkTrackingDiagnostics,
    reports: &[SmokeClientReport],
) -> Result<()> {
    assert_tracking_diagnostics(diagnostics, 2, 2)?;
    for report in reports {
        if has_snapshot(&report.updates, ChunkPos::new(0, 0))
            || has_snapshot(&report.updates, ChunkPos::new(4, 0))
            || has_any_unload(&report.updates)
            || has_any_section_block_updates(&report.updates)
        {
            bail!(
                "teleport ack for smoke client {} unexpectedly produced chunk updates",
                report.index
            );
        }
    }
    Ok(())
}

fn assert_actor_move_phase(
    diagnostics: &PlayerChunkTrackingDiagnostics,
    reports: &[SmokeClientReport],
    expected_aggregate_chunks: usize,
) -> Result<()> {
    assert_tracking_diagnostics(diagnostics, expected_aggregate_chunks, 2)?;
    let actor = client_report(reports, 0)?;
    if has_any_section_block_updates(&actor.updates) {
        bail!("actor movement phase unexpectedly produced block deltas");
    }
    Ok(())
}

fn assert_non_overlapping_block_delta_phase(
    diagnostics: &PlayerChunkTrackingDiagnostics,
    reports: &[SmokeClientReport],
    target: BlockPos,
) -> Result<()> {
    assert_tracking_diagnostics(diagnostics, 2, 2)?;
    let actor = client_report(reports, 0)?;
    let observer = client_report(reports, 1)?;
    if !has_air_delta_for_block(&actor.updates, target) {
        bail!("actor did not receive air block delta for non-overlapping break at {target:?}");
    }
    if has_any_section_block_updates(&observer.updates) {
        bail!("non-overlapping observer received block delta for actor-only chunk");
    }
    Ok(())
}

fn assert_overlap_setup_phase(
    diagnostics: &PlayerChunkTrackingDiagnostics,
    reports: &[SmokeClientReport],
) -> Result<()> {
    assert_tracking_diagnostics(diagnostics, 1, 2)?;
    let observer = client_report(reports, 1)?;
    if !has_snapshot(&observer.updates, ChunkPos::new(1, 0)) {
        bail!("observer did not receive snapshot when moving into actor's chunk");
    }
    if !has_unload(&observer.updates, ChunkPos::new(4, 0)) {
        bail!("observer did not receive unload for previous chunk when overlapping actor");
    }
    Ok(())
}

fn assert_overlapping_block_delta_phase(
    diagnostics: &PlayerChunkTrackingDiagnostics,
    reports: &[SmokeClientReport],
    target: BlockPos,
) -> Result<()> {
    assert_tracking_diagnostics(diagnostics, 1, 2)?;
    let actor = client_report(reports, 0)?;
    let observer = client_report(reports, 1)?;
    if !has_air_delta_for_block(&actor.updates, target) {
        bail!("actor did not receive air block delta for overlapping break at {target:?}");
    }
    if !has_air_delta_for_block(&observer.updates, target) {
        bail!("overlapping observer did not receive actor block delta at {target:?}");
    }
    Ok(())
}

fn assert_no_delta_phase(
    diagnostics: &PlayerChunkTrackingDiagnostics,
    reports: &[SmokeClientReport],
    expected_aggregate_chunks: usize,
) -> Result<()> {
    assert_tracking_diagnostics(diagnostics, expected_aggregate_chunks, 2)?;
    for report in reports {
        if has_any_section_block_updates(&report.updates) {
            bail!(
                "smoke client {} received unexpected block deltas",
                report.index
            );
        }
    }
    Ok(())
}

fn assert_place_block_delta_phase(
    diagnostics: &PlayerChunkTrackingDiagnostics,
    reports: &[SmokeClientReport],
    target: PlacementTarget,
) -> Result<()> {
    assert_tracking_diagnostics(diagnostics, 1, 2)?;
    let actor = client_report(reports, 0)?;
    let observer = client_report(reports, 1)?;
    if !has_block_delta_for_block(&actor.updates, target.placed, DIRT_BLOCK_STATE_ID) {
        bail!(
            "actor did not receive DIRT placement delta at {:?}",
            target.placed
        );
    }
    if !has_block_delta_for_block(&observer.updates, target.placed, DIRT_BLOCK_STATE_ID) {
        bail!(
            "observer did not receive DIRT placement delta at {:?}",
            target.placed
        );
    }
    Ok(())
}

fn assert_tracking_diagnostics(
    diagnostics: &PlayerChunkTrackingDiagnostics,
    expected_aggregate_chunks: usize,
    expected_total_visible_chunks: usize,
) -> Result<()> {
    if diagnostics.player_count < CLIENT_COUNT {
        bail!(
            "expected at least {CLIENT_COUNT} tracked players, got {}",
            diagnostics.player_count
        );
    }
    let visible_players = diagnostics
        .players
        .iter()
        .filter(|player| player.visible_chunks > 0)
        .count();
    if visible_players != CLIENT_COUNT {
        bail!("expected {CLIENT_COUNT} players with visible chunks, got {visible_players}");
    }
    if diagnostics.aggregate_player_ticket_chunks != expected_aggregate_chunks {
        bail!(
            "expected {expected_aggregate_chunks} aggregate player-ticket chunks, got {}",
            diagnostics.aggregate_player_ticket_chunks
        );
    }
    if diagnostics.total_player_visible_chunks != expected_total_visible_chunks {
        bail!(
            "expected {expected_total_visible_chunks} total player-visible chunks, got {}",
            diagnostics.total_player_visible_chunks
        );
    }
    if diagnostics.total_outbound_queue_depth != 0 {
        bail!(
            "expected empty outbound queues after smoke command drain, got {}",
            diagnostics.total_outbound_queue_depth
        );
    }
    Ok(())
}

fn client_report(reports: &[SmokeClientReport], index: usize) -> Result<&SmokeClientReport> {
    reports
        .iter()
        .find(|report| report.index == index)
        .with_context(|| format!("missing smoke client {index} report"))
}

fn teleport_ack_commands(reports: &[SmokeClientReport]) -> Result<Vec<(usize, ClientCommand)>> {
    let mut commands = Vec::new();
    for index in 0..CLIENT_COUNT {
        let report = client_report(reports, index)?;
        let teleport_id = report
            .updates
            .iter()
            .find_map(|update| match update {
                ServerUpdate::PlayerPosition(update) => Some(update.teleport_id),
                _ => None,
            })
            .with_context(|| format!("smoke client {index} did not receive spawn teleport"))?;
        commands.push((
            index,
            ClientCommand::AcceptTeleport(AcceptTeleportCommand { id: teleport_id }),
        ));
    }
    Ok(commands)
}

fn ensure_snapshot_only(
    report: &SmokeClientReport,
    expected: ChunkPos,
    unexpected: ChunkPos,
) -> Result<()> {
    if !has_snapshot(&report.updates, expected) {
        bail!(
            "smoke client {} did not receive expected snapshot for {:?}",
            report.index,
            expected
        );
    }
    if has_snapshot(&report.updates, unexpected) {
        bail!(
            "smoke client {} received another client's snapshot for {:?}",
            report.index,
            unexpected
        );
    }
    Ok(())
}

fn break_targets_from_snapshot(
    report: &SmokeClientReport,
    pos: ChunkPos,
    count: usize,
) -> Result<Vec<BlockPos>> {
    let snapshot = report
        .updates
        .iter()
        .find_map(|update| match update {
            ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == pos => Some(snapshot),
            _ => None,
        })
        .with_context(|| format!("client {} missing snapshot for {pos:?}", report.index))?;
    let targets = non_air_blocks(snapshot, count);
    if targets.len() != count {
        bail!(
            "snapshot for {pos:?} had {} non-air break targets, expected {count}",
            targets.len()
        );
    }
    Ok(targets)
}

fn placement_target_from_snapshot(
    report: &SmokeClientReport,
    pos: ChunkPos,
    excluded: &[BlockPos],
) -> Result<PlacementTarget> {
    let snapshot = report
        .updates
        .iter()
        .find_map(|update| match update {
            ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == pos => Some(snapshot),
            _ => None,
        })
        .with_context(|| format!("client {} missing snapshot for {pos:?}", report.index))?;

    for y in (snapshot.min_y..snapshot.min_y + snapshot.height - 1).rev() {
        for local_z in 0..16 {
            for local_x in 0..16 {
                let clicked = BlockPos::new(
                    snapshot.pos.min_block_x() + local_x,
                    y,
                    snapshot.pos.min_block_z() + local_z,
                );
                if excluded.contains(&clicked) {
                    continue;
                }
                let placed = clicked.relative(Direction::Up);
                if snapshot_block_state(snapshot, clicked) != AIR_BLOCK_STATE_ID
                    && snapshot_block_state(snapshot, placed) == AIR_BLOCK_STATE_ID
                    && !excluded.contains(&placed)
                {
                    return Ok(PlacementTarget { clicked, placed });
                }
            }
        }
    }

    bail!("snapshot for {pos:?} did not contain a solid block with air above")
}

fn non_air_blocks(snapshot: &ChunkSnapshot, count: usize) -> Vec<BlockPos> {
    let mut targets = Vec::new();
    for section in &snapshot.sections {
        let block_state_ids = section.unpack_block_state_ids();
        for local_y in 0..SECTION_HEIGHT {
            for local_z in 0..16 {
                for local_x in 0..16 {
                    let index = chunk_section_index(local_x, local_y, local_z);
                    if block_state_ids[index] == AIR_BLOCK_STATE_ID {
                        continue;
                    }
                    targets.push(BlockPos::new(
                        snapshot.pos.min_block_x() + local_x,
                        section.section_y * SECTION_HEIGHT + local_y,
                        snapshot.pos.min_block_z() + local_z,
                    ));
                    if targets.len() == count {
                        return targets;
                    }
                }
            }
        }
    }
    targets
}

fn snapshot_block_state(snapshot: &ChunkSnapshot, pos: BlockPos) -> BlockStateId {
    if pos.chunk_pos() != snapshot.pos
        || pos.y < snapshot.min_y
        || pos.y >= snapshot.min_y + snapshot.height
    {
        return AIR_BLOCK_STATE_ID;
    }
    let section_y = block_to_section_coord(pos.y);
    let Some(section) = snapshot
        .sections
        .iter()
        .find(|section| section.section_y == section_y)
    else {
        return AIR_BLOCK_STATE_ID;
    };
    let local_x = local_block_coord(pos.x);
    let local_y = local_section_block_coord(pos.y);
    let local_z = local_block_coord(pos.z);
    let blocks = section.unpack_block_state_ids();
    blocks[chunk_section_index(local_x, local_y, local_z)]
}

fn move_near_block_command(pos: BlockPos) -> ClientCommand {
    ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
        position: Vec3d::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5),
        y_rot_degrees: 0.0,
        x_rot_degrees: 0.0,
        on_ground: true,
    })
}

fn move_far_from_block_command() -> ClientCommand {
    ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
        position: Vec3d::new(2048.0, 128.0, 2048.0),
        y_rot_degrees: 0.0,
        x_rot_degrees: 0.0,
        on_ground: true,
    })
}

fn break_block_command(pos: BlockPos) -> ClientCommand {
    ClientCommand::PlayerAction(PlayerActionCommand {
        pos,
        direction: Direction::Up,
        kind: PlayerActionKind::DebugInstantBreak,
    })
}

fn use_item_on_command(clicked: BlockPos) -> ClientCommand {
    ClientCommand::UseItemOn(UseItemOnCommand {
        hand: InteractionHand::MainHand,
        hit: BlockHitResult::new(
            Vec3d::new(
                clicked.x as f64 + 0.5,
                clicked.y as f64 + 1.0,
                clicked.z as f64 + 0.5,
            ),
            Direction::Up,
            clicked,
            false,
        ),
    })
}

fn poll_command(slot: u8) -> ClientCommand {
    ClientCommand::SetCarriedItem(SetCarriedItemCommand { slot })
}

fn has_snapshot(updates: &[ServerUpdate], pos: ChunkPos) -> bool {
    updates.iter().any(
        |update| matches!(update, ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == pos),
    )
}

fn has_unload(updates: &[ServerUpdate], pos: ChunkPos) -> bool {
    updates.iter().any(
        |update| matches!(update, ServerUpdate::ChunkUnload { pos: unloaded } if *unloaded == pos),
    )
}

fn has_any_unload(updates: &[ServerUpdate]) -> bool {
    updates
        .iter()
        .any(|update| matches!(update, ServerUpdate::ChunkUnload { .. }))
}

fn has_any_section_block_updates(updates: &[ServerUpdate]) -> bool {
    updates
        .iter()
        .any(|update| matches!(update, ServerUpdate::SectionBlockUpdates { .. }))
}

fn has_air_delta_for_block(updates: &[ServerUpdate], pos: BlockPos) -> bool {
    has_block_delta_for_block(updates, pos, AIR_BLOCK_STATE_ID)
}

fn has_block_delta_for_block(
    updates: &[ServerUpdate],
    pos: BlockPos,
    block_state: BlockStateId,
) -> bool {
    let chunk_pos = pos.chunk_pos();
    let section_y = block_to_section_coord(pos.y);
    let local_x = local_block_coord(pos.x) as u8;
    let local_y = local_section_block_coord(pos.y) as u8;
    let local_z = local_block_coord(pos.z) as u8;

    updates.iter().any(|update| match update {
        ServerUpdate::SectionBlockUpdates {
            pos,
            section_y: update_section_y,
            updates,
        } if *pos == chunk_pos && *update_section_y == section_y => updates.iter().any(|update| {
            update.local_x == local_x
                && update.local_y == local_y
                && update.local_z == local_z
                && update.block_state == block_state
        }),
        _ => false,
    })
}

fn snapshot_count(updates: &[ServerUpdate]) -> usize {
    updates
        .iter()
        .filter(|update| matches!(update, ServerUpdate::ChunkSnapshot(_)))
        .count()
}

fn unload_count(updates: &[ServerUpdate]) -> usize {
    updates
        .iter()
        .filter(|update| matches!(update, ServerUpdate::ChunkUnload { .. }))
        .count()
}

fn block_delta_count(updates: &[ServerUpdate]) -> usize {
    updates
        .iter()
        .map(|update| match update {
            ServerUpdate::SectionBlockUpdates { updates, .. } => updates.len(),
            _ => 0,
        })
        .sum()
}

fn print_smoke_report(seed: i64, phases: &[PhaseReport]) {
    let recorded_unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    println!("{{");
    println!("  \"smoke\": \"native_dedicated_multi_client\",");
    println!("  \"recorded_unix_seconds\": {recorded_unix_seconds},");
    println!("  \"seed\": {seed},");
    println!("  \"client_count\": {CLIENT_COUNT},");
    println!("  \"lighting_enabled\": false,");
    println!("  \"phases\": [");
    for (phase_index, phase) in phases.iter().enumerate() {
        let comma = if phase_index + 1 == phases.len() {
            ""
        } else {
            ","
        };
        println!("    {{");
        println!("      \"name\": \"{}\",", phase.name);
        print_diagnostics(&phase.diagnostics, "      ");
        println!("      \"clients\": [");
        for (client_index, report) in phase.client_reports.iter().enumerate() {
            let client_comma = if client_index + 1 == phase.client_reports.len() {
                ""
            } else {
                ","
            };
            println!("        {{");
            println!("          \"index\": {},", report.index);
            println!("          \"update_count\": {},", report.updates.len());
            println!(
                "          \"snapshot_count\": {},",
                snapshot_count(&report.updates)
            );
            println!(
                "          \"unload_count\": {},",
                unload_count(&report.updates)
            );
            println!(
                "          \"block_delta_count\": {}",
                block_delta_count(&report.updates)
            );
            println!("        }}{client_comma}");
        }
        println!("      ]");
        println!("    }}{comma}");
    }
    println!("  ]");
    println!("}}");
}

fn print_diagnostics(diagnostics: &PlayerChunkTrackingDiagnostics, indent: &str) {
    println!("{indent}\"diagnostics\": {{");
    println!("{indent}  \"player_count\": {},", diagnostics.player_count);
    println!(
        "{indent}  \"aggregate_player_ticket_chunks\": {},",
        diagnostics.aggregate_player_ticket_chunks
    );
    println!(
        "{indent}  \"total_player_visible_chunks\": {},",
        diagnostics.total_player_visible_chunks
    );
    println!(
        "{indent}  \"total_outbound_queue_depth\": {},",
        diagnostics.total_outbound_queue_depth
    );
    println!(
        "{indent}  \"max_player_visible_chunks\": {},",
        diagnostics.max_player_visible_chunks
    );
    println!(
        "{indent}  \"max_outbound_queue_depth\": {},",
        diagnostics.max_outbound_queue_depth
    );
    println!("{indent}  \"players\": [");
    for (index, player) in diagnostics.players.iter().enumerate() {
        let comma = if index + 1 == diagnostics.players.len() {
            ""
        } else {
            ","
        };
        println!("{indent}    {{");
        println!("{indent}      \"id\": {},", player.player_id.as_u64());
        println!(
            "{indent}      \"visible_chunks\": {},",
            player.visible_chunks
        );
        println!(
            "{indent}      \"outbound_queue_depth\": {}",
            player.outbound_queue_depth
        );
        println!("{indent}    }}{comma}");
    }
    println!("{indent}  ]");
    println!("{indent}}},");
}
