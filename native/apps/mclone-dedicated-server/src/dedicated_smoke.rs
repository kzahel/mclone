use std::collections::BTreeMap;
use std::net::{SocketAddr, TcpListener};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use mclone_core::ChunkPos;
use mclone_net::NativeClientSession;
use mclone_protocol::{ChunkView, ClientCommand, ServerUpdate, SetCarriedItemCommand};
use mclone_server::{IntegratedServer, PlayerChunkTrackingDiagnostics};

use crate::connection::{DedicatedConnectionId, DedicatedNetwork, DedicatedNetworkEvent};
use crate::session::DedicatedSession;

const CLIENT_COUNT: usize = 2;
const EVENT_TIMEOUT: Duration = Duration::from_secs(30);
const CLIENT_REPORT_TIMEOUT: Duration = Duration::from_secs(30);

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

    for control in controls {
        let _ = control.send(SmokeClientControl::Close);
    }
    smoke_server.drain_disconnects(CLIENT_COUNT)?;
    join_clients(clients)?;

    print_smoke_report(
        seed,
        &[
            PhaseReport {
                name: "initial_disjoint_views",
                diagnostics: phase_one_diagnostics,
                client_reports: phase_one_reports,
            },
            PhaseReport {
                name: "one_client_moves_view",
                diagnostics: phase_two_diagnostics,
                client_reports: phase_two_reports,
            },
        ],
    );

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

fn wait_for_client_reports(
    result_rx: &Receiver<std::result::Result<SmokeClientReport, String>>,
) -> Result<Vec<SmokeClientReport>> {
    let mut reports = Vec::new();
    while reports.len() < CLIENT_COUNT {
        match result_rx.recv_timeout(CLIENT_REPORT_TIMEOUT) {
            Ok(Ok(report)) => reports.push(report),
            Ok(Err(message)) => bail!(message),
            Err(err) => bail!("timed out waiting for smoke client reports: {err}"),
        }
    }
    reports.sort_by_key(|report| report.index);
    Ok(reports)
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
    assert_tracking_diagnostics(diagnostics)?;
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
    assert_tracking_diagnostics(diagnostics)?;
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

fn assert_tracking_diagnostics(diagnostics: &PlayerChunkTrackingDiagnostics) -> Result<()> {
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
    if diagnostics.aggregate_player_ticket_chunks != CLIENT_COUNT {
        bail!(
            "expected {CLIENT_COUNT} aggregate player-ticket chunks, got {}",
            diagnostics.aggregate_player_ticket_chunks
        );
    }
    if diagnostics.total_player_visible_chunks != CLIENT_COUNT {
        bail!(
            "expected {CLIENT_COUNT} total player-visible chunks, got {}",
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
                "          \"unload_count\": {}",
                unload_count(&report.updates)
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
