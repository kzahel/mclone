mod connection;
mod dedicated_smoke;
mod session;
mod websocket_connection;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use mclone_protocol::PROTOCOL_VERSION;
use mclone_server::{
    IntegratedServer, SimulationCadence, SimulationCadenceConfig, WorldGenerationProfile,
};

use crate::connection::{
    DedicatedConnectionId, DedicatedNetwork, DedicatedNetworkEvent, DedicatedOutboundQueueMetrics,
};
use crate::session::{DedicatedSession, DedicatedSessionDiagnostics};

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:25565";
const DEFAULT_SEED: i64 = 12345;
const DEFAULT_WORLD_ROOT: &str = "worlds";
const DEFAULT_WORLD_NAME: &str = "world";
const AUTOSAVE_INTERVAL_GAMEPLAY_TICKS: u64 = 6_000;

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
         The server accepts persistent native TCP command streams from multiple clients. --generation-profile authored-only makes absent chunks deterministic void instead of running overworld generation. --world-dir opens a persistent SQLite-backed world; --world-root/--world-name select a named world directory. Without a world argument, or with --transient, the server uses explicit transient storage. --listen-ws accepts browser clients into the same authoritative host as native peers. --serve-once is intended for loopback smokes and exits after the first connection closes."
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
        return run_server_with_websocket(
            listener,
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
    server.disable_local_player();
    server.set_world_generation_profile(profile)?;
    Ok(server)
}

fn run_server_with_websocket(
    listener: TcpListener,
    listen_ws: &str,
    seed: i64,
    profile: WorldGenerationProfile,
    world: DedicatedWorldSelection,
    mode: ServerRunMode,
) -> Result<()> {
    let ws_listener = TcpListener::bind(listen_ws)
        .with_context(|| format!("failed to bind dedicated websocket listener to {listen_ws}"))?;
    let ws_addr = ws_listener
        .local_addr()
        .context("failed to read websocket listen addr")?;
    println!("mclone dedicated websocket listening on ws://{ws_addr}");
    run_server_loop_with_listeners(listener, Some(ws_listener), seed, profile, world, mode)
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
    run_server_loop_with_listeners(listener, None, seed, profile, world, mode)
}

fn run_server_loop_with_listeners(
    listener: TcpListener,
    websocket_listener: Option<TcpListener>,
    seed: i64,
    profile: WorldGenerationProfile,
    world: DedicatedWorldSelection,
    mode: ServerRunMode,
) -> Result<()> {
    let network = DedicatedNetwork::start_with_websocket(listener, websocket_listener)?;
    let mut server = open_dedicated_server(seed, profile, &world)?;
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
    let mut outbound = BTreeMap::<DedicatedConnectionId, connection::DedicatedOutbound>::new();
    let mut session_command_counts = BTreeMap::<DedicatedConnectionId, usize>::new();
    let mut summary = DedicatedServerSummary::default();
    let cadence_config = SimulationCadenceConfig::default();
    let host_interval = Duration::from_secs_f64(1.0 / f64::from(cadence_config.host_rate_hz));
    let mut cadence = SimulationCadence::new(cadence_config)
        .expect("default dedicated simulation cadence must be valid");
    let mut next_host_boundary = Instant::now();
    let mut pending_events = VecDeque::new();
    #[cfg(test)]
    let mut completed_connections = 0_usize;

    loop {
        let now = Instant::now();
        if now < next_host_boundary {
            if let Some(event) = network.recv_timeout(next_host_boundary - now)? {
                pending_events.push_back(event);
            }
            continue;
        }

        while let Some(event) = network.recv_timeout(Duration::ZERO)? {
            pending_events.push_back(event);
        }

        let mut active_summary_connections = BTreeSet::new();
        let mut exit_after_boundary = false;
        let mut exit_error = None;

        while let Some(event) = pending_events.pop_front() {
            match event {
                DedicatedNetworkEvent::Connected {
                    id,
                    peer_addr,
                    outbound: connection_outbound,
                } => {
                    let player_id = server.add_dedicated_player();
                    sessions.insert(id, DedicatedSession::new(player_id));
                    outbound.insert(id, connection_outbound);
                    session_command_counts.insert(id, 0);
                    log::info!("accepted dedicated client {id} from {peer_addr}");
                }
                DedicatedNetworkEvent::Command {
                    id,
                    peer_addr,
                    command,
                } => {
                    let Some(session) = sessions.get_mut(&id) else {
                        let message = format!("received command for disconnected client {id}");
                        if let Some(connection_outbound) = outbound.remove(&id) {
                            let _ = connection_outbound.close(message.clone());
                        }
                        log::warn!("{message}");
                        continue;
                    };
                    match session.handle_client_command(server, command) {
                        Ok(()) => {
                            summary.record_command();
                            let connection_command_count =
                                session_command_counts.entry(id).or_default();
                            *connection_command_count = connection_command_count.saturating_add(1);
                            if should_emit_active_summary(*connection_command_count) {
                                active_summary_connections.insert(id);
                            }
                        }
                        Err(err) => {
                            let message = format!("{err:#}");
                            if let Some(connection_outbound) = outbound.remove(&id) {
                                let _ = connection_outbound.close(message.clone());
                            }
                            remove_session_player(server, &mut sessions, id);
                            session_command_counts.remove(&id);
                            if mode == ServerRunMode::ServeOnce {
                                exit_error = Some(
                                    anyhow::anyhow!(err)
                                        .context(format!("failed to serve {id} {peer_addr}")),
                                );
                                exit_after_boundary = true;
                            } else {
                                log::warn!(
                                    "failed to serve dedicated client {id} {peer_addr}: {message}"
                                );
                            }
                        }
                    }
                }
                DedicatedNetworkEvent::Disconnected {
                    id,
                    peer_addr,
                    command_count,
                    reason,
                } => {
                    outbound.remove(&id);
                    remove_session_player(server, &mut sessions, id);
                    let connection_command_count =
                        session_command_counts.remove(&id).unwrap_or(command_count);
                    println!(
                        "{}",
                        summary.marker_line("final", id, connection_command_count, sessions.len())
                    );
                    #[cfg(test)]
                    {
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
                                let message = reason.map_or_else(
                                    || format!(
                                        "connection {id} {peer_addr} closed without client command"
                                    ),
                                    |reason| format!(
                                        "connection {id} {peer_addr} closed without client command: {reason}"
                                    ),
                                );
                                exit_error = Some(anyhow::anyhow!(message));
                            }
                            exit_after_boundary = true;
                        }
                        #[cfg(test)]
                        ServerRunMode::UntilDisconnects(target) => {
                            if completed_connections >= target {
                                exit_after_boundary = true;
                            }
                        }
                    }
                }
                DedicatedNetworkEvent::AcceptFailed { message } => {
                    if mode == ServerRunMode::ServeOnce {
                        exit_error = Some(anyhow::anyhow!(message));
                        exit_after_boundary = true;
                    } else {
                        log::warn!("{message}");
                    }
                }
            }
        }

        let diagnostics = advance_dedicated_host_frame(server, &mut sessions, &mut cadence)?;
        summary.record_tick(diagnostics);

        let mut failed_publications = Vec::new();
        for (&id, session) in &sessions {
            let updates = server
                .try_drain_updates_for_player(session.player_id())
                .with_context(|| format!("failed to drain publications for client {id}"))?;
            if updates.is_empty() {
                continue;
            }
            let update_count = updates.len();
            let publish_result = outbound
                .get(&id)
                .context("dedicated connection has no outbound writer")
                .and_then(|connection_outbound| {
                    connection_outbound.publish(updates)?;
                    Ok(connection_outbound.queue_metrics())
                });
            match publish_result {
                Ok(queue_metrics) => {
                    summary.record_publication(update_count);
                    summary.record_outbound_pressure(queue_metrics);
                }
                Err(error) => failed_publications.push((id, update_count, error)),
            }
        }
        for (id, update_count, error) in failed_publications {
            summary.record_publication_disconnect(error.to_string().contains("queue reached"));
            remove_session_player(server, &mut sessions, id);
            outbound.remove(&id);
            let command_count = session_command_counts.remove(&id).unwrap_or_default();
            println!(
                "{}",
                summary.marker_line("final", id, command_count, sessions.len())
            );
            log::warn!("dedicated client {id} could not queue {update_count} updates: {error:#}");
        }

        for id in active_summary_connections {
            let command_count = session_command_counts.get(&id).copied().unwrap_or_default();
            println!(
                "{}",
                summary.marker_line("active", id, command_count, sessions.len())
            );
        }

        if let Some(error) = exit_error {
            return Err(error);
        }
        if exit_after_boundary {
            return Ok(());
        }

        next_host_boundary += host_interval;
        let max_catch_up = host_interval * cadence_config.max_catch_up_host_frames;
        if Instant::now().saturating_duration_since(next_host_boundary) > max_catch_up {
            next_host_boundary = Instant::now() + host_interval;
        }
    }
}

fn advance_dedicated_host_frame(
    server: &mut IntegratedServer,
    sessions: &mut BTreeMap<DedicatedConnectionId, DedicatedSession>,
    cadence: &mut SimulationCadence,
) -> Result<DedicatedSessionDiagnostics> {
    for session in sessions.values_mut() {
        session.finish_tick_boundary(server)?;
    }

    let frame = cadence.advance_host_frame();
    if frame.gameplay_ticks != 1 || frame.physics_steps != 3 {
        bail!(
            "dedicated default cadence produced unsupported frame gameplay_ticks={} physics_steps={}",
            frame.gameplay_ticks,
            frame.physics_steps
        );
    }
    let report = server
        .try_simulation_tick_report_global()
        .context("failed to advance autonomous dedicated simulation")?;
    if should_autosave(report.simulation_tick) {
        server
            .save_dirty_chunks()
            .context("failed to queue dedicated autosave")?;
    }
    Ok(DedicatedSessionDiagnostics::from_report(
        &report,
        server.pending_job_count(),
        server.pending_publication_count(),
    ))
}

const fn should_autosave(simulation_tick: u64) -> bool {
    simulation_tick > 0 && simulation_tick.is_multiple_of(AUTOSAVE_INTERVAL_GAMEPLAY_TICKS)
}

#[derive(Clone, Debug, Default)]
struct DedicatedServerSummary {
    command_count: usize,
    update_batches: usize,
    updates_sent: usize,
    max_update_batch: usize,
    max_outbound_queue_frames: usize,
    max_outbound_queue_bytes: usize,
    publication_disconnects: usize,
    slow_consumer_disconnects: usize,
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
    fn record_command(&mut self) {
        self.command_count = self.command_count.saturating_add(1);
    }

    fn record_publication(&mut self, update_count: usize) {
        self.update_batches = self.update_batches.saturating_add(1);
        self.updates_sent = self.updates_sent.saturating_add(update_count);
        self.max_update_batch = self.max_update_batch.max(update_count);
    }

    fn record_outbound_pressure(&mut self, metrics: DedicatedOutboundQueueMetrics) {
        self.max_outbound_queue_frames = self
            .max_outbound_queue_frames
            .max(metrics.max_queued_frames);
        self.max_outbound_queue_bytes = self.max_outbound_queue_bytes.max(metrics.max_queued_bytes);
    }

    fn record_publication_disconnect(&mut self, slow_consumer: bool) {
        self.publication_disconnects = self.publication_disconnects.saturating_add(1);
        if slow_consumer {
            self.slow_consumer_disconnects = self.slow_consumer_disconnects.saturating_add(1);
        }
    }

    fn record_tick(&mut self, diagnostics: DedicatedSessionDiagnostics) {
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
            "MCLONE_DEDICATED_SERVER_SUMMARY phase={phase} connection={connection_id} connection_commands={connection_command_count} commands={} update_batches={} updates_sent={} max_update_batch={} max_outbound_queue_frames={} max_outbound_queue_bytes={} publication_disconnects={} slow_consumer_disconnects={} tick_total_ms={:.3} tick_max_ms={:.3} scheduler_tick_ms={:.3} scheduler_publish_completed_ms={:.3} completed_feature_jobs_drained={} feature_chunks_published={} feature_chunks_skipped={} completed_light_statuses_drained={} light_statuses_published={} light_statuses_skipped={} pending_jobs={} pending_publications={} pending_worldgen_publication_jobs={} pending_worldgen_publication_chunks={} pending_light_publications={} active_sessions={} last_simulation_tick={}",
            self.command_count,
            self.update_batches,
            self.updates_sent,
            self.max_update_batch,
            self.max_outbound_queue_frames,
            self.max_outbound_queue_bytes,
            self.publication_disconnects,
            self.slow_consumer_disconnects,
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
    use mclone_net::NativeClientIoSession;
    use mclone_protocol::{
        AcceptTeleportCommand, ChunkView, ClientCommand, MovePlayerCommand, PlayerActionCommand,
        PlayerActionKind, RemotePlayerId, ServerUpdate,
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
        summary.record_command();
        summary.record_publication(11);
        summary.record_outbound_pressure(DedicatedOutboundQueueMetrics {
            queued_frames: 1,
            queued_bytes: 256,
            max_queued_frames: 3,
            max_queued_bytes: 1_024,
        });
        summary.record_tick(DedicatedSessionDiagnostics {
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
        });

        let line = summary.marker_line("active", DedicatedConnectionId::test_new(12), 1, 0);

        assert!(line.starts_with("MCLONE_DEDICATED_SERVER_SUMMARY "));
        assert!(line.contains("phase=active"));
        assert!(line.contains("connection=#12"));
        assert!(line.contains("commands=1"));
        assert!(line.contains("updates_sent=11"));
        assert!(line.contains("max_outbound_queue_frames=3"));
        assert!(line.contains("max_outbound_queue_bytes=1024"));
        assert!(line.contains("tick_total_ms=2.500"));
        assert!(line.contains("scheduler_tick_ms=1.250"));
        assert!(line.contains("feature_chunks_published=3"));
        assert!(line.contains("light_statuses_published=5"));
        assert!(line.contains("pending_worldgen_publication_chunks=7"));
        assert!(line.contains("last_simulation_tick=7"));
    }

    #[test]
    fn dedicated_host_advances_without_clients() {
        let mut server = IntegratedServer::new(DEFAULT_SEED);
        server.disable_local_player();
        let start_day_time = server.day_time();
        let mut sessions = BTreeMap::new();
        let mut cadence = SimulationCadence::default();

        for _ in 0..20 {
            advance_dedicated_host_frame(&mut server, &mut sessions, &mut cadence).unwrap();
        }

        assert_eq!(server.simulation_tick(), 20);
        assert_eq!(server.day_time(), start_day_time + 20);
    }

    #[test]
    fn aggregate_command_volume_does_not_advance_host_clock() {
        let mut server = IntegratedServer::new(DEFAULT_SEED);
        server.disable_local_player();
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();
        let id_a = DedicatedConnectionId::test_new(1);
        let id_b = DedicatedConnectionId::test_new(2);
        let mut sessions = BTreeMap::from([
            (id_a, DedicatedSession::new(player_a)),
            (id_b, DedicatedSession::new(player_b)),
        ]);

        for index in 0..100 {
            let id = if index % 2 == 0 { id_a } else { id_b };
            sessions
                .get_mut(&id)
                .unwrap()
                .handle_client_command(
                    &mut server,
                    ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                        position: Vec3d::new(f64::from(index), 64.0, 0.0),
                        on_ground: true,
                    }),
                )
                .unwrap();
        }
        assert_eq!(server.simulation_tick(), 0);

        advance_dedicated_host_frame(
            &mut server,
            &mut sessions,
            &mut SimulationCadence::default(),
        )
        .unwrap();
        assert_eq!(server.simulation_tick(), 1);
    }

    #[test]
    fn dedicated_chunk_view_returns_before_snapshot_and_pushes_later() {
        let mut server = IntegratedServer::new(DEFAULT_SEED);
        server.disable_local_player();
        server.set_lighting_enabled(false);
        let player = server.add_dedicated_player();
        server.try_drain_updates_for_player(player).unwrap();
        let id = DedicatedConnectionId::test_new(1);
        let mut sessions = BTreeMap::from([(id, DedicatedSession::new(player))]);

        let command_start = Instant::now();
        sessions
            .get_mut(&id)
            .unwrap()
            .handle_client_command(
                &mut server,
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }),
            )
            .unwrap();
        assert!(command_start.elapsed() < Duration::from_millis(100));
        assert!(
            server
                .try_drain_updates_for_player(player)
                .unwrap()
                .iter()
                .all(|update| !matches!(update, ServerUpdate::ChunkSnapshot(_)))
        );

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut cadence = SimulationCadence::default();
        loop {
            advance_dedicated_host_frame(&mut server, &mut sessions, &mut cadence).unwrap();
            let updates = server.try_drain_updates_for_player(player).unwrap();
            if chunk_snapshot_opt(&updates, ChunkPos::new(0, 0)).is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "snapshot was never published");
            std::thread::yield_now();
        }
    }

    #[test]
    fn idle_observer_receives_remote_player_movement() {
        let mut server = IntegratedServer::new(DEFAULT_SEED);
        server.disable_local_player();
        server.set_lighting_enabled(false);
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();
        let id_a = DedicatedConnectionId::test_new(1);
        let id_b = DedicatedConnectionId::test_new(2);
        let mut sessions = BTreeMap::from([
            (id_a, DedicatedSession::new(player_a)),
            (id_b, DedicatedSession::new(player_b)),
        ]);
        for id in [id_a, id_b] {
            sessions
                .get_mut(&id)
                .unwrap()
                .handle_client_command(
                    &mut server,
                    ClientCommand::SetChunkView(ChunkView {
                        center: ChunkPos::new(0, 0),
                        render_distance: 0,
                        chunk_tracking_radius: 0,
                    }),
                )
                .unwrap();
        }

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut cadence = SimulationCadence::default();
        let mut spawn_a = None;
        let mut spawn_b = None;
        while spawn_a.is_none() || spawn_b.is_none() {
            advance_dedicated_host_frame(&mut server, &mut sessions, &mut cadence).unwrap();
            for (player, spawn) in [(player_a, &mut spawn_a), (player_b, &mut spawn_b)] {
                for update in server.try_drain_updates_for_player(player).unwrap() {
                    if let ServerUpdate::PlayerPosition(position) = update {
                        *spawn = Some(position);
                    }
                }
            }
            assert!(
                Instant::now() < deadline,
                "spawn positions were never published"
            );
            std::thread::yield_now();
        }
        for (id, spawn) in [(id_a, spawn_a.unwrap()), (id_b, spawn_b.unwrap())] {
            sessions
                .get_mut(&id)
                .unwrap()
                .handle_client_command(
                    &mut server,
                    ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                        id: spawn.teleport_id,
                    }),
                )
                .unwrap();
        }
        advance_dedicated_host_frame(&mut server, &mut sessions, &mut cadence).unwrap();
        server.try_drain_updates_for_player(player_a).unwrap();
        server.try_drain_updates_for_player(player_b).unwrap();

        let spawn_position = spawn_a.unwrap().position;
        let moved = Vec3d::new(spawn_position.x + 0.25, spawn_position.y, spawn_position.z);
        sessions
            .get_mut(&id_a)
            .unwrap()
            .handle_client_command(
                &mut server,
                ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                    position: moved,
                    y_rot_degrees: 90.0,
                    x_rot_degrees: -15.0,
                    on_ground: true,
                }),
            )
            .unwrap();
        advance_dedicated_host_frame(&mut server, &mut sessions, &mut cadence).unwrap();
        let observer_updates = server.try_drain_updates_for_player(player_b).unwrap();
        assert!(observer_updates.iter().any(|update| matches!(
            update,
            ServerUpdate::RemotePlayerUpdate(remote)
                if remote.id == RemotePlayerId(player_a.as_u64())
                    && remote.position == moved
        )));
    }

    #[test]
    fn zero_command_client_receives_periodic_time_push() {
        let _guard = DEDICATED_NETWORK_TEST_LOCK.lock().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            run_server_loop(
                listener,
                DEFAULT_SEED,
                DedicatedWorldSelection::Transient,
                ServerRunMode::UntilDisconnects(1),
            )
            .unwrap();
        });
        let mut session = NativeClientIoSession::connect(addr).unwrap();
        let updates = wait_for_remote_updates(&mut session, |updates| {
            let mut times = updates.iter().filter_map(|update| match update {
                ServerUpdate::TimeUpdate { day_time } => Some(*day_time),
                _ => None,
            });
            let Some(first) = times.next() else {
                return false;
            };
            times.any(|time| time >= first + 20)
        });
        assert!(
            updates
                .iter()
                .filter(|update| matches!(update, ServerUpdate::TimeUpdate { .. }))
                .count()
                >= 2
        );
        drop(session);
        server.join().unwrap();
    }

    #[test]
    fn dedicated_autosave_period_is_six_thousand_gameplay_ticks() {
        assert!(!should_autosave(0));
        assert!(!should_autosave(5_999));
        assert!(should_autosave(6_000));
        assert!(!should_autosave(6_001));
        assert!(should_autosave(12_000));
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
            let mut session = NativeClientIoSession::connect(addr).unwrap();
            session
                .send_command_only(ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }))
                .unwrap();
            wait_for_remote_updates(&mut session, |updates| {
                chunk_snapshot_opt(updates, ChunkPos::new(0, 0)).is_some()
            })
        });
        let client_b = std::thread::spawn(move || {
            let mut session = NativeClientIoSession::connect(addr).unwrap();
            session
                .send_command_only(ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(1, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }))
                .unwrap();
            wait_for_remote_updates(&mut session, |updates| {
                chunk_snapshot_opt(updates, ChunkPos::new(1, 0)).is_some()
            })
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
            let mut session = NativeClientIoSession::connect(addr).unwrap();
            session
                .send_command_only(ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }))
                .unwrap();
            let updates = wait_for_remote_updates(&mut session, |updates| {
                chunk_snapshot_opt(updates, ChunkPos::new(0, 0)).is_some()
                    && player_position_teleport_id_opt(updates).is_some()
            });
            let snapshot = chunk_snapshot(&updates, ChunkPos::new(0, 0));
            let target = first_non_air_block(snapshot);
            let teleport_id = player_position_teleport_id(&updates);
            session
                .send_command_only(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                    id: teleport_id,
                }))
                .unwrap();
            session
                .send_command_only(move_near_block_command(target))
                .unwrap();
            session
                .send_command_only(break_block_command(target))
                .unwrap();
            let break_updates = wait_for_remote_updates(&mut session, |updates| {
                has_block_delta_for_block(updates, target, AIR_BLOCK_STATE_ID)
            });
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
        let mut session = NativeClientIoSession::connect(addr).unwrap();
        session
            .send_command_only(ClientCommand::SetChunkView(ChunkView {
                center: target.chunk_pos(),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }))
            .unwrap();
        let updates = wait_for_remote_updates(&mut session, |updates| {
            chunk_snapshot_opt(updates, target.chunk_pos()).is_some()
        });
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

        let mut session = NativeClientIoSession::connect(addr).unwrap();
        session
            .send_command_only(ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(manifest.center_chunk[0], manifest.center_chunk[1]),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }))
            .unwrap();
        let updates = wait_for_remote_updates(&mut session, |updates| {
            chunk_snapshot_opt(updates, ChunkPos::new(0, 0)).is_some()
                && player_position_teleport_id_opt(updates).is_some()
        });
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
        chunk_snapshot_opt(updates, pos)
            .unwrap_or_else(|| panic!("missing chunk snapshot for {pos:?}"))
    }

    fn chunk_snapshot_opt(updates: &[ServerUpdate], pos: ChunkPos) -> Option<&ChunkSnapshot> {
        updates.iter().find_map(|update| match update {
            ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == pos => Some(snapshot),
            _ => None,
        })
    }

    fn player_position_teleport_id(updates: &[ServerUpdate]) -> u32 {
        player_position_teleport_id_opt(updates).expect("missing player position update")
    }

    fn player_position_teleport_id_opt(updates: &[ServerUpdate]) -> Option<u32> {
        updates.iter().find_map(|update| match update {
            ServerUpdate::PlayerPosition(update) => Some(update.teleport_id),
            _ => None,
        })
    }

    fn wait_for_remote_updates(
        session: &mut NativeClientIoSession,
        ready: impl Fn(&[ServerUpdate]) -> bool,
    ) -> Vec<ServerUpdate> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut updates = Vec::new();
        loop {
            while let Some(batch) = session.try_drain_update_batch().unwrap() {
                updates.extend(batch.into_updates());
            }
            if ready(&updates) {
                return updates;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for autonomous dedicated publication; received {} updates",
                updates.len()
            );
            std::thread::sleep(Duration::from_millis(1));
        }
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
