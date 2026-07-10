use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use mclone_app_runtime::DEFAULT_STARTUP_READINESS_TIMEOUT;
use mclone_core::{ChunkPos, Vec3d};
use mclone_net::{
    NativeClientSession, NativeTransportError, complete_server_handshake,
    try_read_client_command_frame, write_server_update_batch,
};
use mclone_protocol::{
    AcceptTeleportCommand, ChunkView, ClientCommand, MovePlayerCommand, PlayerAppearance,
    PlayerModelKind, PlayerPositionUpdate, ServerUpdate, SetPlayerAppearanceCommand,
};
use mclone_render_session::EngineCameraViewMode;
use mclone_server::{IntegratedServer, ServerPlayerId};

use crate::cli::{
    HeadlessScreenshotOptions, HeadlessScreenshotUi, RemotePlayerVisualSmokeOptions,
    StartupWaitPolicy,
};
use crate::offscreen_flat_client::run_offscreen_flat_client_screenshot;

const REMOTE_SETTLE_MS: u64 = 250;
const REMOTE_ACTOR_MOVE_STEP_MS: u64 = 50;
const REMOTE_ACTOR_MOVE_STEP_BLOCKS: f64 = 0.08;
const SERVER_JOB_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RemotePlayerVisualSmokeReport {
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) byte_len: usize,
    pub(crate) remote_player_count: usize,
    pub(crate) actor_count: usize,
    pub(crate) drawn_actor_count: usize,
    pub(crate) remote_actor_figures: Vec<String>,
    pub(crate) remote_actor_walk_animation_distances: Vec<f32>,
}

pub(crate) fn run_remote_player_visual_smoke(
    options: &RemotePlayerVisualSmokeOptions,
) -> Result<RemotePlayerVisualSmokeReport> {
    let mut server = LoopbackDedicatedServer::start(options)?;
    let mut remote_actor = match RemoteActorClient::spawn(server.addr(), options) {
        Ok(remote_actor) => remote_actor,
        Err(err) => {
            let server_stop_result = server.stop();
            return match server_stop_result {
                Ok(()) => Err(err),
                Err(server_err) => {
                    Err(err).context(format!("loopback smoke server also failed: {server_err:#}"))
                }
            };
        }
    };

    let mut scene = options.scene.clone();
    scene.remote_addr = Some(server.addr().to_string());
    let mut render_options = options.render_options;
    render_options.force_fullbright = true;

    let screenshot_result = run_offscreen_flat_client_screenshot(&HeadlessScreenshotOptions {
        path: options.path.clone(),
        width: options.width,
        height: options.height,
        scene,
        render_options,
        startup_wait: StartupWaitPolicy::OFFSCREEN_SCREENSHOT_DEFAULT,
        camera_view: EngineCameraViewMode::FirstPerson,
        ui: HeadlessScreenshotUi::None,
        hud: false,
        frame_pipeline_overlay: false,
        debug_pane: false,
        player_collision_box: false,
        blink_debug: false,
        scripted_interaction: false,
        remote_settle_ms: REMOTE_SETTLE_MS,
        eye: None,
        target: None,
    });

    let remote_actor_stop_result = remote_actor.stop();
    let server_stop_result = server.stop();
    let screenshot = screenshot_result?;
    remote_actor_stop_result?;
    server_stop_result?;

    let remote_actor_figures = screenshot
        .remote_actor_figures
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect::<Vec<_>>();
    validate_remote_player_visual_smoke(&screenshot, &remote_actor_figures)?;

    Ok(RemotePlayerVisualSmokeReport {
        path: screenshot.path,
        width: screenshot.width,
        height: screenshot.height,
        byte_len: screenshot.byte_len,
        remote_player_count: screenshot.remote_player_count,
        actor_count: screenshot.summary.actor_count,
        drawn_actor_count: screenshot.summary.drawn_actor_count,
        remote_actor_figures,
        remote_actor_walk_animation_distances: screenshot.remote_actor_walk_animation_distances,
    })
}

fn validate_remote_player_visual_smoke(
    screenshot: &crate::offscreen_flat_client::OffscreenFlatClientScreenshotReport,
    remote_actor_figures: &[String],
) -> Result<()> {
    if screenshot.remote_player_count == 0 {
        bail!("remote player visual smoke captured no remote players");
    }
    if screenshot.summary.actor_count == 0 {
        bail!("remote player visual smoke submitted no actors");
    }
    if screenshot.summary.drawn_actor_count == 0 {
        bail!("remote player visual smoke drew no actors");
    }
    let expected = mclone_assets::upright_bear_figure_id().as_str();
    if !remote_actor_figures
        .iter()
        .any(|figure| figure.as_str() == expected)
    {
        bail!(
            "remote player visual smoke expected remote actor figure `{expected}`, got {:?}",
            remote_actor_figures
        );
    }
    if !screenshot
        .remote_actor_walk_animation_distances
        .iter()
        .any(|distance| distance.is_finite() && *distance > 0.0)
    {
        bail!(
            "remote player visual smoke expected positive walk animation distance, got {:?}",
            screenshot.remote_actor_walk_animation_distances
        );
    }
    Ok(())
}

struct LoopbackDedicatedServer {
    addr: SocketAddr,
    running: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<Result<()>>>,
}

impl LoopbackDedicatedServer {
    fn start(options: &RemotePlayerVisualSmokeOptions) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .context("failed to bind loopback remote-player visual smoke server")?;
        listener
            .set_nonblocking(true)
            .context("failed to set loopback smoke server nonblocking")?;
        let addr = listener
            .local_addr()
            .context("failed to read loopback smoke server address")?;
        let running = Arc::new(AtomicBool::new(true));
        let accept_running = Arc::clone(&running);
        let scene = options.scene.clone();
        let accept_thread = thread::spawn(move || accept_loop(listener, scene, accept_running));
        Ok(Self {
            addr,
            running,
            accept_thread: Some(accept_thread),
        })
    }

    const fn addr(&self) -> SocketAddr {
        self.addr
    }

    fn stop(&mut self) -> Result<()> {
        self.running.store(false, Ordering::Release);
        let Some(handle) = self.accept_thread.take() else {
            return Ok(());
        };
        join_result(handle, "loopback smoke server accept thread")
    }
}

impl Drop for LoopbackDedicatedServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn new_integrated_server(scene: &crate::cli::SceneOptions) -> IntegratedServer {
    let mut server = IntegratedServer::new(scene.seed);
    server.set_lighting_enabled(scene.lighting_enabled);
    if let Some(day_time) = scene.day_time_override {
        server.set_day_time(day_time);
    }
    server.set_day_time_frozen(scene.freeze_time);
    server
}

fn accept_loop(
    listener: TcpListener,
    scene: crate::cli::SceneOptions,
    running: Arc<AtomicBool>,
) -> Result<()> {
    let mut server = new_integrated_server(&scene);
    let (command_tx, command_rx) = mpsc::channel();
    let mut connections = BTreeMap::new();
    let mut connection_threads = Vec::new();

    while running.load(Ordering::Acquire) {
        drain_server_commands(&mut server, &mut connections, &command_rx)?;
        match listener.accept() {
            Ok((stream, _addr)) => {
                if !running.load(Ordering::Acquire) {
                    break;
                }
                let player_id = server.add_dedicated_player();
                connections.insert(player_id, DedicatedConnectionState::default());
                let command_tx = command_tx.clone();
                connection_threads.push(thread::spawn(move || {
                    serve_connection_io(stream, player_id, command_tx)
                }));
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(err) => return Err(err).context("failed to accept loopback smoke client"),
        }
    }

    drain_server_commands(&mut server, &mut connections, &command_rx)?;
    for connection in connection_threads {
        join_result(connection, "loopback smoke client connection")?;
    }
    Ok(())
}

enum ServerIoCommand {
    ClientCommand {
        player_id: ServerPlayerId,
        command: ClientCommand,
        response_tx: mpsc::Sender<std::result::Result<Vec<ServerUpdate>, String>>,
    },
    Disconnect {
        player_id: ServerPlayerId,
    },
}

fn drain_server_commands(
    server: &mut IntegratedServer,
    connections: &mut BTreeMap<ServerPlayerId, DedicatedConnectionState>,
    command_rx: &mpsc::Receiver<ServerIoCommand>,
) -> Result<()> {
    while let Ok(command) = command_rx.try_recv() {
        match command {
            ServerIoCommand::ClientCommand {
                player_id,
                command,
                response_tx,
            } => {
                let result = connections
                    .get_mut(&player_id)
                    .ok_or_else(|| anyhow!("unknown loopback smoke player {player_id}"))
                    .and_then(|connection| {
                        handle_client_command(connection, server, player_id, command)
                    })
                    .map_err(|err| format!("{err:#}"));
                let _ = response_tx.send(result);
            }
            ServerIoCommand::Disconnect { player_id } => {
                connections.remove(&player_id);
                server.remove_dedicated_player(player_id);
            }
        }
    }
    Ok(())
}

fn serve_connection_io(
    mut stream: TcpStream,
    player_id: ServerPlayerId,
    command_tx: mpsc::Sender<ServerIoCommand>,
) -> Result<()> {
    let _ = stream.set_nodelay(true);
    stream
        .set_nonblocking(false)
        .context("failed to set loopback smoke client stream blocking")?;
    complete_server_handshake(&mut stream).context("failed native TCP handshake")?;

    loop {
        let command = match try_read_client_command_frame(&mut stream) {
            Ok(Some(command)) => command,
            Ok(None) => break,
            Err(NativeTransportError::Io(err)) if err.kind() == ErrorKind::ConnectionReset => {
                break;
            }
            Err(err) => return Err(err).context("failed to read loopback smoke client command"),
        };
        let updates = request_server_command(&command_tx, player_id, command)?;
        write_server_update_batch(&mut stream, &updates)
            .context("failed to write loopback smoke server updates")?;
    }
    let _ = command_tx.send(ServerIoCommand::Disconnect { player_id });
    Ok(())
}

fn request_server_command(
    command_tx: &mpsc::Sender<ServerIoCommand>,
    player_id: ServerPlayerId,
    command: ClientCommand,
) -> Result<Vec<ServerUpdate>> {
    let (response_tx, response_rx) = mpsc::channel();
    command_tx
        .send(ServerIoCommand::ClientCommand {
            player_id,
            command,
            response_tx,
        })
        .map_err(|_| anyhow!("loopback smoke server command channel closed"))?;
    response_rx
        .recv()
        .context("loopback smoke server response channel closed")?
        .map_err(|message| anyhow!(message))
}

fn handle_client_command(
    connection: &mut DedicatedConnectionState,
    server: &mut IntegratedServer,
    player_id: ServerPlayerId,
    command: ClientCommand,
) -> Result<Vec<ServerUpdate>> {
    let mut updates = connection.handle_command(server, player_id, command)?;
    updates.extend(connection.flush_movement(server, player_id)?);
    updates.extend(wait_for_server_jobs(server, player_id)?);
    updates.extend(
        server
            .try_simulation_tick_report_for_player(player_id)
            .context("failed to tick loopback smoke server session")?
            .updates,
    );
    connection.mark_tick_boundary();
    server
        .save_dirty_chunks()
        .context("failed to save loopback smoke chunks")?;
    Ok(updates)
}

#[derive(Debug, Default)]
struct DedicatedConnectionState {
    pending_movement: Vec<MovePlayerCommand>,
}

impl DedicatedConnectionState {
    fn handle_command(
        &mut self,
        server: &mut IntegratedServer,
        player_id: ServerPlayerId,
        command: ClientCommand,
    ) -> Result<Vec<ServerUpdate>> {
        match command {
            ClientCommand::MovePlayer(command) => {
                self.pending_movement.push(command);
                Ok(Vec::new())
            }
            command => {
                let mut updates = self.flush_movement(server, player_id)?;
                updates.extend(server.try_handle_command_for_player(player_id, command)?);
                Ok(updates)
            }
        }
    }

    fn flush_movement(
        &mut self,
        server: &mut IntegratedServer,
        player_id: ServerPlayerId,
    ) -> Result<Vec<ServerUpdate>> {
        let mut updates = Vec::new();
        for command in self.pending_movement.drain(..) {
            updates.extend(
                server
                    .try_handle_command_for_player(player_id, ClientCommand::MovePlayer(command))?,
            );
        }
        Ok(updates)
    }

    fn mark_tick_boundary(&mut self) {}
}

fn wait_for_server_jobs(
    server: &mut IntegratedServer,
    player_id: ServerPlayerId,
) -> Result<Vec<ServerUpdate>> {
    let deadline = Instant::now() + SERVER_JOB_TIMEOUT;
    let mut updates = Vec::new();
    loop {
        updates.extend(
            server
                .try_poll_for_player(player_id)
                .context("failed to poll loopback smoke server jobs")?,
        );
        if server.pending_job_count() == 0 {
            return Ok(updates);
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for loopback smoke server jobs");
        }
        if server.pending_publication_count() == 0 {
            thread::sleep(Duration::from_millis(1));
        }
    }
}

struct RemoteActorClient {
    release_tx: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<Result<()>>>,
}

impl RemoteActorClient {
    fn spawn(addr: SocketAddr, options: &RemotePlayerVisualSmokeOptions) -> Result<Self> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let scene = options.scene.clone();
        let thread = thread::spawn(move || {
            let result = run_remote_actor_client(addr, &scene, ready_tx.clone(), release_rx);
            if let Err(err) = &result {
                let _ = ready_tx.send(Err(format!("{err:#}")));
            }
            result
        });

        let _ready = ready_rx
            .recv_timeout(DEFAULT_STARTUP_READINESS_TIMEOUT)
            .context("timed out waiting for loopback remote actor client")?
            .map_err(|message| anyhow!(message))?;

        Ok(Self {
            release_tx: Some(release_tx),
            thread: Some(thread),
        })
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(release_tx) = self.release_tx.take() {
            let _ = release_tx.send(());
        }
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        join_result(thread, "loopback remote actor client")
    }
}

impl Drop for RemoteActorClient {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RemoteActorReady {
    position: Vec3d,
}

fn run_remote_actor_client(
    addr: SocketAddr,
    scene: &crate::cli::SceneOptions,
    ready_tx: mpsc::Sender<std::result::Result<RemoteActorReady, String>>,
    release_rx: mpsc::Receiver<()>,
) -> Result<()> {
    let mut session =
        NativeClientSession::connect(addr).context("failed to connect remote actor client")?;
    let mut current_position = Vec3d::ZERO;
    let mut saw_position = false;

    let updates = session
        .send_command(&ClientCommand::SetChunkView(smoke_chunk_view(scene)?))
        .context("remote actor failed to set chunk view")?;
    let position_update =
        accept_player_position_updates(&mut session, &updates, current_position, saw_position)?;
    current_position = position_update.position;
    saw_position = position_update.saw_position;
    if !saw_position {
        bail!("remote actor did not receive an initial player position");
    }

    let updates = session
        .send_command(&ClientCommand::SetPlayerAppearance(
            SetPlayerAppearanceCommand {
                appearance: PlayerAppearance {
                    model: PlayerModelKind::UprightBear,
                },
            },
        ))
        .context("remote actor failed to set upright bear appearance")?;
    let position_update =
        accept_player_position_updates(&mut session, &updates, current_position, saw_position)?;
    current_position = position_update.position;
    saw_position = position_update.saw_position;

    let updates = session
        .send_command(&ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
            position: current_position,
            y_rot_degrees: 180.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        }))
        .context("remote actor failed to publish position")?;
    let position_update =
        accept_player_position_updates(&mut session, &updates, current_position, saw_position)?;
    current_position = position_update.position;

    ready_tx
        .send(Ok(RemoteActorReady {
            position: current_position,
        }))
        .map_err(|_| anyhow!("remote actor ready receiver dropped"))?;
    loop {
        match release_rx.recv_timeout(Duration::from_millis(REMOTE_ACTOR_MOVE_STEP_MS)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let next_position = Vec3d::new(
            current_position.x,
            current_position.y,
            current_position.z + REMOTE_ACTOR_MOVE_STEP_BLOCKS,
        );
        let updates = session
            .send_command(&ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                position: next_position,
                y_rot_degrees: 180.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            }))
            .context("remote actor failed to publish walking position")?;
        let position_update =
            accept_player_position_updates(&mut session, &updates, next_position, saw_position)?;
        current_position = position_update.position;
        saw_position = position_update.saw_position;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AcceptedPositionUpdates {
    position: Vec3d,
    saw_position: bool,
}

fn accept_player_position_updates(
    session: &mut NativeClientSession,
    updates: &[ServerUpdate],
    mut position: Vec3d,
    mut saw_position: bool,
) -> Result<AcceptedPositionUpdates> {
    for update in updates {
        let ServerUpdate::PlayerPosition(update) = update else {
            continue;
        };
        position = apply_position_update(position, *update);
        saw_position = true;
        session
            .send_command(&ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: update.teleport_id,
            }))
            .context("remote actor failed to accept teleport")?;
    }
    Ok(AcceptedPositionUpdates {
        position,
        saw_position,
    })
}

fn apply_position_update(current: Vec3d, update: PlayerPositionUpdate) -> Vec3d {
    Vec3d::new(
        if update.relative.x {
            current.x + update.position.x
        } else {
            update.position.x
        },
        if update.relative.y {
            current.y + update.position.y
        } else {
            update.position.y
        },
        if update.relative.z {
            current.z + update.position.z
        } else {
            update.position.z
        },
    )
}

fn smoke_chunk_view(scene: &crate::cli::SceneOptions) -> Result<ChunkView> {
    let render_distance =
        u32::try_from(scene.render_distance).context("render distance must be non-negative")?;
    Ok(ChunkView {
        center: ChunkPos::new(scene.chunk_x, scene.chunk_z),
        render_distance,
        chunk_tracking_radius: render_distance,
    })
}

fn join_result(handle: JoinHandle<Result<()>>, label: &str) -> Result<()> {
    match handle.join() {
        Ok(result) => result.with_context(|| format!("{label} failed")),
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("unknown panic payload");
            bail!("{label} panicked: {message}");
        }
    }
}
