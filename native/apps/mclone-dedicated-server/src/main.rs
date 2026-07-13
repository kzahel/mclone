mod connection;
mod dedicated_smoke;
mod session;
mod websocket_bridge;

use std::collections::BTreeMap;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result, bail};
use mclone_protocol::PROTOCOL_VERSION;
use mclone_server::{IntegratedServer, WorldGenerationProfile};

use crate::connection::{DedicatedConnectionId, DedicatedNetwork, DedicatedNetworkEvent};
use crate::session::{DedicatedSession, DedicatedSessionDiagnostics};
use crate::websocket_bridge::{WebSocketBridgeMode, run_websocket_bridge};

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:25565";
const DEFAULT_SEED: i64 = 12345;
const DEFAULT_WORLD_ROOT: &str = "worlds";
const DEFAULT_WORLD_NAME: &str = "world";

#[cfg(test)]
static DEDICATED_NETWORK_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn main() -> Result<()> {
    env_logger::init();
    run_server(Cli::parse(std::env::args().skip(1))?)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Cli {
    listen: String,
    listen_ws: Option<String>,
    seed: i64,
    world_generation_profile: WorldGenerationProfile,
    serve_once: bool,
    multi_client_smoke: bool,
    world: DedicatedWorldSelection,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            listen: DEFAULT_LISTEN_ADDR.to_owned(),
            listen_ws: None,
            seed: DEFAULT_SEED,
            world_generation_profile: WorldGenerationProfile::default(),
            serve_once: false,
            multi_client_smoke: false,
            world: DedicatedWorldSelection::Transient,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DedicatedWorldSelection {
    Transient,
    Persistent { dir: PathBuf },
}

impl DedicatedWorldSelection {
    fn is_persistent(&self) -> bool {
        matches!(self, Self::Persistent { .. })
    }

    fn description(&self) -> String {
        match self {
            Self::Transient => "transient".to_owned(),
            Self::Persistent { dir } => format!("persistent {}", dir.display()),
        }
    }
}

impl Cli {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut cli = Self::default();
        let mut args = args.into_iter();
        let mut explicit_transient = false;
        let mut world_dir: Option<PathBuf> = None;
        let mut world_root: Option<PathBuf> = None;
        let mut world_name: Option<String> = None;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--listen" => {
                    cli.listen = args.next().context("--listen requires HOST:PORT")?;
                }
                "--listen-ws" => {
                    cli.listen_ws = Some(args.next().context("--listen-ws requires HOST:PORT")?);
                }
                "--seed" => {
                    cli.seed = parse_i64_arg("--seed", args.next())?;
                }
                "--generation-profile" => {
                    let value = args
                        .next()
                        .context("--generation-profile requires overworld or authored-only")?;
                    cli.world_generation_profile =
                        WorldGenerationProfile::parse_label(&value).map_err(anyhow::Error::msg)?;
                }
                "--serve-once" => {
                    cli.serve_once = true;
                }
                "--multi-client-smoke" => {
                    cli.multi_client_smoke = true;
                }
                "--transient" => {
                    explicit_transient = true;
                }
                "--world-dir" => {
                    world_dir = Some(PathBuf::from(
                        args.next().context("--world-dir requires PATH")?,
                    ));
                }
                "--world-root" => {
                    world_root = Some(PathBuf::from(
                        args.next().context("--world-root requires PATH")?,
                    ));
                }
                "--world-name" => {
                    world_name = Some(parse_world_name_arg(
                        "--world-name",
                        args.next().context("--world-name requires NAME")?,
                    )?);
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => bail!("unknown argument `{arg}`; pass --help for usage"),
            }
        }

        cli.world = resolve_world_selection(explicit_transient, world_dir, world_root, world_name)?;
        Ok(cli)
    }
}

fn resolve_world_selection(
    explicit_transient: bool,
    world_dir: Option<PathBuf>,
    world_root: Option<PathBuf>,
    world_name: Option<String>,
) -> Result<DedicatedWorldSelection> {
    if explicit_transient && (world_dir.is_some() || world_root.is_some() || world_name.is_some()) {
        bail!("--transient cannot be combined with persistent world arguments");
    }
    if world_dir.is_some() && (world_root.is_some() || world_name.is_some()) {
        bail!("--world-dir cannot be combined with --world-root or --world-name");
    }
    if explicit_transient {
        return Ok(DedicatedWorldSelection::Transient);
    }
    if let Some(dir) = world_dir {
        return Ok(DedicatedWorldSelection::Persistent { dir });
    }
    if world_root.is_some() || world_name.is_some() {
        let root = world_root.unwrap_or_else(|| PathBuf::from(DEFAULT_WORLD_ROOT));
        let name = world_name.unwrap_or_else(|| DEFAULT_WORLD_NAME.to_owned());
        return Ok(DedicatedWorldSelection::Persistent {
            dir: root.join(name),
        });
    }
    Ok(DedicatedWorldSelection::Transient)
}

fn parse_world_name_arg(flag: &str, value: String) -> Result<String> {
    if value.is_empty() || value == "." || value == ".." {
        bail!("{flag} must be a non-empty world name");
    }
    if value.contains('/') || value.contains('\\') {
        bail!("{flag} must be a world name, not a path");
    }
    Ok(value)
}

fn parse_i64_arg(flag: &str, value: Option<String>) -> Result<i64> {
    let value = value.with_context(|| format!("{flag} requires a value"))?;
    value
        .parse::<i64>()
        .with_context(|| format!("{flag} requires a signed 64-bit integer, got `{value}`"))
}

fn print_help() {
    println!(
        "mclone-dedicated-server\n\n\
         Usage:\n\
           mclone-dedicated-server [--listen 127.0.0.1:25565] [--seed 12345] [--generation-profile overworld|authored-only] [--world-dir ./worlds/world] [--serve-once]\n\
           mclone-dedicated-server [--listen 127.0.0.1:25565] [--listen-ws 127.0.0.1:25566] [--seed 12345] [--world-root ./worlds] [--world-name world]\n\
           mclone-dedicated-server --multi-client-smoke [--seed 12345]\n\n\
         The server accepts persistent native TCP command streams from multiple clients. --generation-profile authored-only makes absent chunks deterministic void instead of running overworld generation. --world-dir opens a persistent SQLite-backed world; --world-root/--world-name select a named world directory. Without a world argument, or with --transient, the server uses explicit transient storage. --listen-ws enables a WebSocket bridge for browser clients. --serve-once is intended for loopback smokes and exits after the first connection closes."
    );
}

fn run_server(cli: Cli) -> Result<()> {
    if cli.multi_client_smoke {
        if cli.serve_once {
            bail!("--multi-client-smoke cannot be combined with --serve-once");
        }
        if cli.listen_ws.is_some() {
            bail!("--multi-client-smoke cannot be combined with --listen-ws");
        }
        if cli.listen != DEFAULT_LISTEN_ADDR {
            bail!("--multi-client-smoke binds its own ephemeral loopback listener");
        }
        if cli.world.is_persistent() {
            bail!("--multi-client-smoke cannot be combined with persistent world arguments");
        }
        if cli.world_generation_profile != WorldGenerationProfile::Overworld {
            bail!("--multi-client-smoke requires --generation-profile overworld");
        }
        return dedicated_smoke::run_multi_client_smoke(cli.seed);
    }

    let listener = TcpListener::bind(&cli.listen)
        .with_context(|| format!("failed to bind dedicated server to {}", cli.listen))?;
    let local_addr = listener
        .local_addr()
        .context("failed to read listen addr")?;
    println!(
        "mclone dedicated server listening on {local_addr} seed={} protocol {} world={} generation={}",
        cli.seed,
        PROTOCOL_VERSION,
        cli.world.description(),
        cli.world_generation_profile.label(),
    );

    let mode = if cli.serve_once {
        ServerRunMode::ServeOnce
    } else {
        ServerRunMode::Forever
    };
    if let Some(listen_ws) = cli.listen_ws.as_deref() {
        return run_server_with_websocket_bridge(
            listener,
            local_addr,
            listen_ws,
            cli.seed,
            cli.world_generation_profile,
            cli.world,
            mode,
        );
    }
    run_server_loop_with_profile(
        listener,
        cli.seed,
        cli.world_generation_profile,
        cli.world,
        mode,
    )
}

fn open_dedicated_server(
    seed: i64,
    profile: WorldGenerationProfile,
    world: &DedicatedWorldSelection,
) -> Result<IntegratedServer> {
    let mut server = match world {
        DedicatedWorldSelection::Transient => Ok(IntegratedServer::new(seed)),
        DedicatedWorldSelection::Persistent { dir } => {
            IntegratedServer::try_with_threaded_sqlite_world_dir(seed, dir)
                .with_context(|| format!("failed to open dedicated world at {}", dir.display()))
        }
    }?;
    server.set_world_generation_profile(profile)?;
    Ok(server)
}

fn run_server_with_websocket_bridge(
    listener: TcpListener,
    upstream_addr: std::net::SocketAddr,
    listen_ws: &str,
    seed: i64,
    profile: WorldGenerationProfile,
    world: DedicatedWorldSelection,
    mode: ServerRunMode,
) -> Result<()> {
    let ws_listener = TcpListener::bind(listen_ws)
        .with_context(|| format!("failed to bind dedicated websocket bridge to {listen_ws}"))?;
    let ws_addr = ws_listener
        .local_addr()
        .context("failed to read websocket listen addr")?;
    println!("mclone dedicated websocket listening on ws://{ws_addr} upstream={upstream_addr}");

    let (ready_tx, ready_rx) = mpsc::channel();
    let server_thread = thread::Builder::new()
        .name("mclone-dedicated-server-loop".to_owned())
        .spawn(move || {
            run_server_loop_with_ready(listener, seed, profile, world, mode, Some(ready_tx))
        })
        .context("failed to spawn dedicated server loop for websocket bridge")?;
    match ready_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(message)) => {
            let _ = server_thread.join();
            bail!(message);
        }
        Err(_) => {
            return match server_thread.join() {
                Ok(Err(error)) => Err(error),
                Ok(Ok(())) => bail!("dedicated server loop exited during startup"),
                Err(_) => bail!("dedicated server loop thread panicked during startup"),
            };
        }
    }
    let bridge_mode = match mode {
        ServerRunMode::Forever => WebSocketBridgeMode::Forever,
        ServerRunMode::ServeOnce => WebSocketBridgeMode::ServeOnce,
        #[cfg(test)]
        ServerRunMode::UntilDisconnects(_) => WebSocketBridgeMode::ServeOnce,
    };
    let bridge_result = run_websocket_bridge(ws_listener, upstream_addr, bridge_mode);
    if bridge_result.is_ok() && mode == ServerRunMode::ServeOnce {
        server_thread
            .join()
            .map_err(|_| anyhow::anyhow!("dedicated server loop thread panicked"))??;
    }
    bridge_result
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServerRunMode {
    Forever,
    ServeOnce,
    #[cfg(test)]
    UntilDisconnects(usize),
}

#[cfg(test)]
fn run_server_loop(
    listener: TcpListener,
    seed: i64,
    world: DedicatedWorldSelection,
    mode: ServerRunMode,
) -> Result<()> {
    run_server_loop_with_profile(
        listener,
        seed,
        WorldGenerationProfile::Overworld,
        world,
        mode,
    )
}

fn run_server_loop_with_profile(
    listener: TcpListener,
    seed: i64,
    profile: WorldGenerationProfile,
    world: DedicatedWorldSelection,
    mode: ServerRunMode,
) -> Result<()> {
    run_server_loop_with_ready(listener, seed, profile, world, mode, None)
}

fn run_server_loop_with_ready(
    listener: TcpListener,
    seed: i64,
    profile: WorldGenerationProfile,
    world: DedicatedWorldSelection,
    mode: ServerRunMode,
    ready_tx: Option<mpsc::Sender<std::result::Result<(), String>>>,
) -> Result<()> {
    let network = match DedicatedNetwork::start(listener) {
        Ok(network) => network,
        Err(error) => {
            if let Some(ready_tx) = ready_tx {
                let _ = ready_tx.send(Err(format!("{error:#}")));
            }
            return Err(error);
        }
    };
    let mut server = match open_dedicated_server(seed, profile, &world) {
        Ok(server) => {
            if let Some(ready_tx) = ready_tx {
                let _ = ready_tx.send(Ok(()));
            }
            server
        }
        Err(error) => {
            if let Some(ready_tx) = ready_tx {
                let _ = ready_tx.send(Err(format!("{error:#}")));
            }
            return Err(error);
        }
    };
    let loop_result = run_server_loop_inner(network, &mut server, mode);
    let shutdown_result = server
        .shutdown_persistence()
        .context("failed to save and close dedicated world persistence");
    if let Err(error) = shutdown_result {
        if loop_result.is_err() {
            log::warn!("{error:#}");
            loop_result
        } else {
            Err(error)
        }
    } else {
        loop_result
    }
}

fn run_server_loop_inner(
    network: DedicatedNetwork,
    server: &mut IntegratedServer,
    mode: ServerRunMode,
) -> Result<()> {
    let mut sessions = BTreeMap::<DedicatedConnectionId, DedicatedSession>::new();
    let mut session_command_counts = BTreeMap::<DedicatedConnectionId, usize>::new();
    let mut summary = DedicatedServerSummary::default();
    #[cfg(test)]
    let mut completed_connections = 0_usize;

    loop {
        match network.recv()? {
            DedicatedNetworkEvent::Connected { id, peer_addr } => {
                let player_id = server.add_dedicated_player();
                sessions.insert(id, DedicatedSession::new(player_id));
                session_command_counts.insert(id, 0);
                log::info!("accepted dedicated client {id} from {peer_addr}");
            }
            DedicatedNetworkEvent::Command {
                id,
                peer_addr,
                command,
                response,
            } => {
                let Some(session) = sessions.get_mut(&id) else {
                    let message = format!("received command for disconnected client {id}");
                    let _ = response.send(Err(message.clone()));
                    log::warn!("{message}");
                    continue;
                };
                let result = session.handle_client_command(server, command);
                let diagnostics = session.last_diagnostics();
                match result {
                    Ok(updates) => {
                        let update_count = updates.len();
                        if response.send(Ok(updates)).is_err() {
                            remove_session_player(server, &mut sessions, id);
                            let command_count =
                                session_command_counts.remove(&id).unwrap_or_default();
                            println!(
                                "{}",
                                summary.marker_line("final", id, command_count, sessions.len())
                            );
                            log::warn!(
                                "dedicated client {id} {peer_addr} disconnected before receiving {update_count} updates"
                            );
                        } else {
                            summary.record_command(diagnostics, update_count);
                            let connection_command_count =
                                session_command_counts.entry(id).or_default();
                            *connection_command_count = connection_command_count.saturating_add(1);
                            if should_emit_active_summary(*connection_command_count) {
                                println!(
                                    "{}",
                                    summary.marker_line(
                                        "active",
                                        id,
                                        *connection_command_count,
                                        sessions.len()
                                    )
                                );
                            }
                            log::debug!(
                                "served dedicated client {id} {peer_addr} with {update_count} updates"
                            );
                        }
                    }
                    Err(err) => {
                        let message = format!("{err:#}");
                        let _ = response.send(Err(message.clone()));
                        remove_session_player(server, &mut sessions, id);
                        session_command_counts.remove(&id);
                        if mode == ServerRunMode::ServeOnce {
                            return Err(err)
                                .with_context(|| format!("failed to serve {id} {peer_addr}"));
                        }
                        log::warn!("failed to serve dedicated client {id} {peer_addr}: {message}");
                    }
                }
            }
            DedicatedNetworkEvent::Disconnected {
                id,
                peer_addr,
                command_count,
                reason,
            } => {
                remove_session_player(server, &mut sessions, id);
                let connection_command_count =
                    session_command_counts.remove(&id).unwrap_or(command_count);
                println!(
                    "{}",
                    summary.marker_line("final", id, connection_command_count, sessions.len())
                );
                #[cfg(test)]
                if command_count > 0 {
                    completed_connections += 1;
                }
                if let Some(reason) = reason.as_deref() {
                    log::warn!(
                        "dedicated client {id} {peer_addr} disconnected after {command_count} commands: {reason}"
                    );
                } else {
                    log::info!(
                        "dedicated client {id} {peer_addr} disconnected after {command_count} commands"
                    );
                }
                match mode {
                    ServerRunMode::Forever => {}
                    ServerRunMode::ServeOnce => {
                        if command_count == 0 {
                            if let Some(reason) = reason.as_deref() {
                                bail!(
                                    "connection {id} {peer_addr} closed without client command: {reason}"
                                );
                            }
                            bail!("connection {id} {peer_addr} closed without client command");
                        }
                        return Ok(());
                    }
                    #[cfg(test)]
                    ServerRunMode::UntilDisconnects(target) => {
                        if completed_connections >= target {
                            return Ok(());
                        }
                    }
                }
            }
            DedicatedNetworkEvent::AcceptFailed { message } => {
                if mode == ServerRunMode::ServeOnce {
                    bail!(message);
                }
                log::warn!("{message}");
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
struct DedicatedServerSummary {
    command_count: usize,
    update_batches: usize,
    updates_sent: usize,
    max_update_batch: usize,
    total_tick_us: u128,
    max_tick_us: u128,
    total_scheduler_tick_us: u128,
    total_scheduler_publish_completed_us: u128,
    completed_feature_jobs_drained: usize,
    feature_chunks_published: usize,
    feature_chunks_skipped: usize,
    completed_light_statuses_drained: usize,
    light_statuses_published: usize,
    light_statuses_skipped: usize,
    last: DedicatedSessionDiagnostics,
}

impl DedicatedServerSummary {
    fn record_command(&mut self, diagnostics: DedicatedSessionDiagnostics, update_count: usize) {
        self.command_count = self.command_count.saturating_add(1);
        self.update_batches = self.update_batches.saturating_add(1);
        self.updates_sent = self.updates_sent.saturating_add(update_count);
        self.max_update_batch = self.max_update_batch.max(update_count);
        self.total_tick_us = self.total_tick_us.saturating_add(diagnostics.tick_total_us);
        self.max_tick_us = self.max_tick_us.max(diagnostics.tick_total_us);
        self.total_scheduler_tick_us = self
            .total_scheduler_tick_us
            .saturating_add(diagnostics.scheduler_tick_us);
        self.total_scheduler_publish_completed_us = self
            .total_scheduler_publish_completed_us
            .saturating_add(diagnostics.scheduler_publish_completed_us);
        self.completed_feature_jobs_drained = self
            .completed_feature_jobs_drained
            .saturating_add(diagnostics.publication.completed_feature_jobs_drained);
        self.feature_chunks_published = self
            .feature_chunks_published
            .saturating_add(diagnostics.publication.feature_chunks_published);
        self.feature_chunks_skipped = self
            .feature_chunks_skipped
            .saturating_add(diagnostics.publication.feature_chunks_skipped);
        self.completed_light_statuses_drained = self
            .completed_light_statuses_drained
            .saturating_add(diagnostics.publication.completed_light_statuses_drained);
        self.light_statuses_published = self
            .light_statuses_published
            .saturating_add(diagnostics.publication.light_statuses_published);
        self.light_statuses_skipped = self
            .light_statuses_skipped
            .saturating_add(diagnostics.publication.light_statuses_skipped);
        self.last = diagnostics;
    }

    fn marker_line(
        &self,
        phase: &str,
        connection_id: DedicatedConnectionId,
        connection_command_count: usize,
        active_sessions: usize,
    ) -> String {
        format!(
            "MCLONE_DEDICATED_SERVER_SUMMARY phase={phase} connection={connection_id} connection_commands={connection_command_count} commands={} update_batches={} updates_sent={} max_update_batch={} tick_total_ms={:.3} tick_max_ms={:.3} scheduler_tick_ms={:.3} scheduler_publish_completed_ms={:.3} completed_feature_jobs_drained={} feature_chunks_published={} feature_chunks_skipped={} completed_light_statuses_drained={} light_statuses_published={} light_statuses_skipped={} pending_jobs={} pending_publications={} pending_worldgen_publication_jobs={} pending_worldgen_publication_chunks={} pending_light_publications={} active_sessions={} last_simulation_tick={}",
            self.command_count,
            self.update_batches,
            self.updates_sent,
            self.max_update_batch,
            micros_to_ms(self.total_tick_us),
            micros_to_ms(self.max_tick_us),
            micros_to_ms(self.total_scheduler_tick_us),
            micros_to_ms(self.total_scheduler_publish_completed_us),
            self.completed_feature_jobs_drained,
            self.feature_chunks_published,
            self.feature_chunks_skipped,
            self.completed_light_statuses_drained,
            self.light_statuses_published,
            self.light_statuses_skipped,
            self.last.pending_jobs_after,
            self.last.pending_publications_after,
            self.last.publication.pending_worldgen_publication_jobs,
            self.last.publication.pending_worldgen_publication_chunks,
            self.last.publication.pending_light_publications,
            active_sessions,
            self.last.simulation_tick
        )
    }
}

fn should_emit_active_summary(connection_command_count: usize) -> bool {
    connection_command_count == 1 || connection_command_count % 16 == 0
}

fn micros_to_ms(value: u128) -> f64 {
    value as f64 / 1000.0
}

fn remove_session_player(
    server: &mut IntegratedServer,
    sessions: &mut BTreeMap<DedicatedConnectionId, DedicatedSession>,
    connection_id: DedicatedConnectionId,
) {
    let Some(session) = sessions.remove(&connection_id) else {
        return;
    };
    server.remove_dedicated_player(session.player_id());
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{
        AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, ChunkPos, ChunkSnapshot, Direction, Vec3d,
        block_to_section_coord, chunk_section_index, local_block_coord, local_section_block_coord,
    };
    use mclone_net::NativeClientSession;
    use mclone_protocol::{
        AcceptTeleportCommand, ChunkView, ClientCommand, MovePlayerCommand, PlayerActionCommand,
        PlayerActionKind, ServerUpdate,
    };

    #[test]
    fn cli_defaults_to_localhost_server() {
        assert_eq!(Cli::parse([]).unwrap(), Cli::default());
    }

    #[test]
    fn cli_parses_listen_seed_and_serve_once() {
        assert_eq!(
            Cli::parse([
                "--listen".to_owned(),
                "127.0.0.1:0".to_owned(),
                "--seed".to_owned(),
                "-7".to_owned(),
                "--generation-profile".to_owned(),
                "authored-only".to_owned(),
                "--serve-once".to_owned(),
            ])
            .unwrap(),
            Cli {
                listen: "127.0.0.1:0".to_owned(),
                listen_ws: None,
                seed: -7,
                world_generation_profile: WorldGenerationProfile::authored_only(),
                serve_once: true,
                multi_client_smoke: false,
                world: DedicatedWorldSelection::Transient,
            }
        );
    }

    #[test]
    fn cli_parses_websocket_listener() {
        assert_eq!(
            Cli::parse([
                "--listen".to_owned(),
                "127.0.0.1:0".to_owned(),
                "--listen-ws".to_owned(),
                "127.0.0.1:0".to_owned(),
                "--seed".to_owned(),
                "17".to_owned(),
            ])
            .unwrap(),
            Cli {
                listen: "127.0.0.1:0".to_owned(),
                listen_ws: Some("127.0.0.1:0".to_owned()),
                seed: 17,
                world_generation_profile: WorldGenerationProfile::Overworld,
                serve_once: false,
                multi_client_smoke: false,
                world: DedicatedWorldSelection::Transient,
            }
        );
    }

    #[test]
    fn cli_parses_multi_client_smoke() {
        assert_eq!(
            Cli::parse([
                "--multi-client-smoke".to_owned(),
                "--seed".to_owned(),
                "99".to_owned(),
            ])
            .unwrap(),
            Cli {
                listen: DEFAULT_LISTEN_ADDR.to_owned(),
                listen_ws: None,
                seed: 99,
                world_generation_profile: WorldGenerationProfile::Overworld,
                serve_once: false,
                multi_client_smoke: true,
                world: DedicatedWorldSelection::Transient,
            }
        );
    }

    #[test]
    fn cli_parses_persistent_world_dir() {
        assert_eq!(
            Cli::parse([
                "--world-dir".to_owned(),
                "/tmp/mclone-world".to_owned(),
                "--seed".to_owned(),
                "77".to_owned(),
            ])
            .unwrap(),
            Cli {
                listen: DEFAULT_LISTEN_ADDR.to_owned(),
                listen_ws: None,
                seed: 77,
                world_generation_profile: WorldGenerationProfile::Overworld,
                serve_once: false,
                multi_client_smoke: false,
                world: DedicatedWorldSelection::Persistent {
                    dir: PathBuf::from("/tmp/mclone-world"),
                },
            }
        );
    }

    #[test]
    fn cli_parses_named_world_under_root() {
        assert_eq!(
            Cli::parse([
                "--world-root".to_owned(),
                "/tmp/mclone-worlds".to_owned(),
                "--world-name".to_owned(),
                "alpha".to_owned(),
            ])
            .unwrap()
            .world,
            DedicatedWorldSelection::Persistent {
                dir: PathBuf::from("/tmp/mclone-worlds").join("alpha"),
            }
        );
    }

    #[test]
    fn cli_rejects_transient_with_world_dir() {
        let err = Cli::parse([
            "--transient".to_owned(),
            "--world-dir".to_owned(),
            "/tmp/mclone-world".to_owned(),
        ])
        .unwrap_err()
        .to_string();
        assert!(err.contains("--transient cannot be combined"));
    }

    #[test]
    fn dedicated_server_summary_marker_reports_host_side_scheduler_fields() {
        let mut summary = DedicatedServerSummary::default();
        summary.record_command(
            DedicatedSessionDiagnostics {
                simulation_tick: 7,
                tick_total_us: 2_500,
                scheduler_tick_us: 1_250,
                scheduler_publish_completed_us: 750,
                publication: mclone_server::ChunkSchedulerPublicationDiagnostics {
                    completed_feature_jobs_drained: 2,
                    feature_chunks_published: 3,
                    feature_chunks_skipped: 1,
                    completed_light_statuses_drained: 4,
                    light_statuses_published: 5,
                    light_statuses_skipped: 1,
                    pending_worldgen_publication_jobs: 6,
                    pending_worldgen_publication_chunks: 7,
                    pending_light_publications: 8,
                    ..Default::default()
                },
                pending_jobs_after: 9,
                pending_publications_after: 10,
            },
            11,
        );

        let line = summary.marker_line("active", DedicatedConnectionId::test_new(12), 1, 0);

        assert!(line.starts_with("MCLONE_DEDICATED_SERVER_SUMMARY "));
        assert!(line.contains("phase=active"));
        assert!(line.contains("connection=#12"));
        assert!(line.contains("commands=1"));
        assert!(line.contains("updates_sent=11"));
        assert!(line.contains("tick_total_ms=2.500"));
        assert!(line.contains("scheduler_tick_ms=1.250"));
        assert!(line.contains("feature_chunks_published=3"));
        assert!(line.contains("light_statuses_published=5"));
        assert!(line.contains("pending_worldgen_publication_chunks=7"));
        assert!(line.contains("last_simulation_tick=7"));
    }

    #[test]
    fn server_loop_serves_multiple_persistent_clients() {
        let _guard = DEDICATED_NETWORK_TEST_LOCK.lock().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            run_server_loop(
                listener,
                DEFAULT_SEED,
                DedicatedWorldSelection::Transient,
                ServerRunMode::UntilDisconnects(2),
            )
            .unwrap();
        });

        let client_a = std::thread::spawn(move || {
            let mut session = NativeClientSession::connect(addr).unwrap();
            session
                .send_command(&ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }))
                .unwrap()
        });
        let client_b = std::thread::spawn(move || {
            let mut session = NativeClientSession::connect(addr).unwrap();
            session
                .send_command(&ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(1, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }))
                .unwrap()
        });

        let updates_a = client_a.join().unwrap();
        let updates_b = client_b.join().unwrap();
        server.join().unwrap();

        assert!(
            updates_a
                .iter()
                .any(|update| matches!(update, ServerUpdate::TimeUpdate { .. }))
        );
        assert!(
            updates_b
                .iter()
                .any(|update| matches!(update, ServerUpdate::TimeUpdate { .. }))
        );
    }

    #[test]
    fn persistent_world_dir_survives_dedicated_server_restart() {
        let _guard = DEDICATED_NETWORK_TEST_LOCK.lock().unwrap();
        let root = unique_temp_dir("dedicated-persistent-world-restart");
        let world = DedicatedWorldSelection::Persistent { dir: root.clone() };
        let target = {
            let (server, addr) = spawn_serve_once_server(DEFAULT_SEED, world.clone());
            let mut session = NativeClientSession::connect(addr).unwrap();
            let updates = session
                .send_command(&ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }))
                .unwrap();
            let snapshot = chunk_snapshot(&updates, ChunkPos::new(0, 0));
            let target = first_non_air_block(snapshot);
            let teleport_id = player_position_teleport_id(&updates);
            session
                .send_command(&ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                    id: teleport_id,
                }))
                .unwrap();
            session
                .send_command(&move_near_block_command(target))
                .unwrap();
            let break_updates = session.send_command(&break_block_command(target)).unwrap();
            assert!(has_block_delta_for_block(
                &break_updates,
                target,
                AIR_BLOCK_STATE_ID
            ));
            drop(session);
            server.join().unwrap().unwrap();
            target
        };

        let (server, addr) = spawn_serve_once_server(DEFAULT_SEED, world.clone());
        let mut session = NativeClientSession::connect(addr).unwrap();
        let updates = session
            .send_command(&ClientCommand::SetChunkView(ChunkView {
                center: target.chunk_pos(),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }))
            .unwrap();
        let snapshot = chunk_snapshot(&updates, target.chunk_pos());
        assert_eq!(snapshot_block_state(snapshot, target), AIR_BLOCK_STATE_ID);
        drop(session);
        server.join().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dedicated_server_serves_persistent_authored_fixture() {
        let _guard = DEDICATED_NETWORK_TEST_LOCK.lock().unwrap();
        let root = unique_temp_dir("dedicated-authored-fixture");
        let manifest = mclone_server::write_authored_world_fixture_dir(
            &root,
            mclone_server::AuthoredWorldFixtureKind::Island,
        )
        .unwrap();
        let world = DedicatedWorldSelection::Persistent { dir: root.clone() };
        let (server, addr) = spawn_serve_once_server_with_profile(
            manifest.seed,
            manifest.world_generation_profile,
            world,
        );

        let mut session = NativeClientSession::connect(addr).unwrap();
        let updates = session
            .send_command(&ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(manifest.center_chunk[0], manifest.center_chunk[1]),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }))
            .unwrap();
        let snapshot = chunk_snapshot(&updates, ChunkPos::new(0, 0));
        assert_ne!(
            snapshot_block_state(snapshot, BlockPos::new(8, 64, 8)),
            AIR_BLOCK_STATE_ID
        );
        let expected_spawn = manifest.expected_spawn;
        assert!(updates.iter().any(|update| matches!(
            update,
            ServerUpdate::PlayerPosition(position)
                if position.position == Vec3d::new(
                    expected_spawn[0],
                    expected_spawn[1],
                    expected_spawn[2]
                )
        )));

        drop(session);
        server.join().unwrap().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    fn spawn_serve_once_server(
        seed: i64,
        world: DedicatedWorldSelection,
    ) -> (std::thread::JoinHandle<Result<()>>, std::net::SocketAddr) {
        spawn_serve_once_server_with_profile(seed, WorldGenerationProfile::Overworld, world)
    }

    fn spawn_serve_once_server_with_profile(
        seed: i64,
        profile: WorldGenerationProfile,
        world: DedicatedWorldSelection,
    ) -> (std::thread::JoinHandle<Result<()>>, std::net::SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            run_server_loop_with_profile(listener, seed, profile, world, ServerRunMode::ServeOnce)
        });
        (server, addr)
    }

    fn chunk_snapshot(updates: &[ServerUpdate], pos: ChunkPos) -> &ChunkSnapshot {
        updates
            .iter()
            .find_map(|update| match update {
                ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == pos => Some(snapshot),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing chunk snapshot for {pos:?}"))
    }

    fn player_position_teleport_id(updates: &[ServerUpdate]) -> u32 {
        updates
            .iter()
            .find_map(|update| match update {
                ServerUpdate::PlayerPosition(update) => Some(update.teleport_id),
                _ => None,
            })
            .expect("missing player position update")
    }

    fn first_non_air_block(snapshot: &ChunkSnapshot) -> BlockPos {
        let max_break_y = (snapshot.min_y + snapshot.height - 1).min(255);
        for y in (snapshot.min_y..=max_break_y).rev() {
            for local_z in 0..16 {
                for local_x in 0..16 {
                    let pos = BlockPos::new(
                        snapshot.pos.min_block_x() + local_x,
                        y,
                        snapshot.pos.min_block_z() + local_z,
                    );
                    if snapshot_block_state(snapshot, pos) != AIR_BLOCK_STATE_ID {
                        return pos;
                    }
                }
            }
        }
        panic!("snapshot for {:?} had no non-air blocks", snapshot.pos);
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

    fn break_block_command(pos: BlockPos) -> ClientCommand {
        ClientCommand::PlayerAction(PlayerActionCommand {
            pos,
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        })
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
            } if *pos == chunk_pos && *update_section_y == section_y => {
                updates.iter().any(|update| {
                    update.local_x == local_x
                        && update.local_y == local_y
                        && update.local_z == local_z
                        && update.block_state == block_state
                })
            }
            _ => false,
        })
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("mclone-{name}-{}-{nanos}", std::process::id()));
        path
    }
}
