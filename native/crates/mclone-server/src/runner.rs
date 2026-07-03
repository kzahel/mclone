//! Integrated server runner boundary.
//!
//! The runner owns the `IntegratedServer` and its tick loop. Client/runtime code
//! talks to it through encoded command protocol frames and decoded update
//! envelopes. Local integrated mode still accounts update queue bytes using the
//! shared update codec, but the runtime pump does not decode update payloads on
//! the render thread.

use std::error::Error;
use std::fmt;

use mclone_protocol::{ClientCommand, ProtocolCodecError, ServerUpdate};
#[cfg(not(target_arch = "wasm32"))]
use mclone_protocol::{decode_client_command, encode_client_command, encode_server_update};

#[cfg(not(target_arch = "wasm32"))]
use crate::IntegratedServer;
#[cfg(not(target_arch = "wasm32"))]
use crate::SimulationCadence;
use crate::{
    ChunkLoadingProgressSnapshot, ChunkLoadingProgressStats, ChunkSchedulerMetrics,
    ChunkStoreError, LightStatusMailboxKind, NaturalSpawningDiagnostics,
    PlayerChunkTrackingDiagnostics, ServerPhysicsTickDiagnostics, ServerSimulationTickReport,
    ServerSimulationTickTiming, SimulationCadenceConfig, WorldgenMailboxKind,
};

pub type ServerRunnerResult<T> = Result<T, ServerRunnerError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerRunnerKind {
    InlineFallback,
    NativeThread,
    WebWorker,
    RemoteWebSocket,
}

impl ServerRunnerKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::InlineFallback => "inline-fallback",
            Self::NativeThread => "native-thread",
            Self::WebWorker => "web-worker",
            Self::RemoteWebSocket => "remote-websocket",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerFrameTransportKind {
    None,
    MessageTransfer,
    SharedMemory,
    WebSocket,
}

impl Default for WorkerFrameTransportKind {
    fn default() -> Self {
        Self::None
    }
}

impl WorkerFrameTransportKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MessageTransfer => "message-transfer",
            Self::SharedMemory => "shared-memory",
            Self::WebSocket => "websocket",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkerFrameMetrics {
    pub transport_kind: WorkerFrameTransportKind,
    pub request_frames: usize,
    pub request_bytes: usize,
    pub response_frames: usize,
    pub response_bytes: usize,
    pub max_pending_frames: usize,
    pub last_request_us: u128,
    pub total_request_us: u128,
    pub max_request_us: u128,
    pub shared_buffer_pool_hits: usize,
    pub shared_buffer_pool_misses: usize,
    pub shared_buffer_pool_drops: usize,
    pub shared_buffer_capacity_bytes: usize,
    pub max_shared_buffer_capacity_bytes: usize,
    pub shared_buffer_pooled_response_frames: usize,
    pub shared_buffer_fallback_response_frames: usize,
}

impl WorkerFrameMetrics {
    pub const fn message_transfer() -> Self {
        Self {
            transport_kind: WorkerFrameTransportKind::MessageTransfer,
            request_frames: 0,
            request_bytes: 0,
            response_frames: 0,
            response_bytes: 0,
            max_pending_frames: 0,
            last_request_us: 0,
            total_request_us: 0,
            max_request_us: 0,
            shared_buffer_pool_hits: 0,
            shared_buffer_pool_misses: 0,
            shared_buffer_pool_drops: 0,
            shared_buffer_capacity_bytes: 0,
            max_shared_buffer_capacity_bytes: 0,
            shared_buffer_pooled_response_frames: 0,
            shared_buffer_fallback_response_frames: 0,
        }
    }

    pub const fn shared_memory() -> Self {
        Self {
            transport_kind: WorkerFrameTransportKind::SharedMemory,
            request_frames: 0,
            request_bytes: 0,
            response_frames: 0,
            response_bytes: 0,
            max_pending_frames: 0,
            last_request_us: 0,
            total_request_us: 0,
            max_request_us: 0,
            shared_buffer_pool_hits: 0,
            shared_buffer_pool_misses: 0,
            shared_buffer_pool_drops: 0,
            shared_buffer_capacity_bytes: 0,
            max_shared_buffer_capacity_bytes: 0,
            shared_buffer_pooled_response_frames: 0,
            shared_buffer_fallback_response_frames: 0,
        }
    }

    pub const fn websocket() -> Self {
        Self {
            transport_kind: WorkerFrameTransportKind::WebSocket,
            request_frames: 0,
            request_bytes: 0,
            response_frames: 0,
            response_bytes: 0,
            max_pending_frames: 0,
            last_request_us: 0,
            total_request_us: 0,
            max_request_us: 0,
            shared_buffer_pool_hits: 0,
            shared_buffer_pool_misses: 0,
            shared_buffer_pool_drops: 0,
            shared_buffer_capacity_bytes: 0,
            max_shared_buffer_capacity_bytes: 0,
            shared_buffer_pooled_response_frames: 0,
            shared_buffer_fallback_response_frames: 0,
        }
    }

    pub fn record_request(&mut self, bytes: usize) {
        self.request_frames = self.request_frames.saturating_add(1);
        self.request_bytes = self.request_bytes.saturating_add(bytes);
    }

    pub fn record_response(&mut self, bytes: usize) {
        self.response_frames = self.response_frames.saturating_add(1);
        self.response_bytes = self.response_bytes.saturating_add(bytes);
    }

    pub fn observe_pending_frames(&mut self, pending_frames: usize) {
        self.max_pending_frames = self.max_pending_frames.max(pending_frames);
    }

    pub fn record_request_time_us(&mut self, request_us: u128) {
        self.last_request_us = request_us;
        self.total_request_us = self.total_request_us.saturating_add(request_us);
        self.max_request_us = self.max_request_us.max(request_us);
    }

    pub fn record_shared_buffer_pool_hit(&mut self) {
        self.shared_buffer_pool_hits = self.shared_buffer_pool_hits.saturating_add(1);
    }

    pub fn record_shared_buffer_pool_miss(&mut self) {
        self.shared_buffer_pool_misses = self.shared_buffer_pool_misses.saturating_add(1);
    }

    pub fn record_shared_buffer_pool_drop(&mut self, released_bytes: usize) {
        self.shared_buffer_pool_drops = self.shared_buffer_pool_drops.saturating_add(1);
        self.shared_buffer_capacity_bytes = self
            .shared_buffer_capacity_bytes
            .saturating_sub(released_bytes);
    }

    pub fn record_shared_buffer_capacity_delta(&mut self, bytes: usize) {
        self.shared_buffer_capacity_bytes = self.shared_buffer_capacity_bytes.saturating_add(bytes);
        self.max_shared_buffer_capacity_bytes = self
            .max_shared_buffer_capacity_bytes
            .max(self.shared_buffer_capacity_bytes);
    }

    pub fn record_shared_buffer_pooled_response(&mut self) {
        self.shared_buffer_pooled_response_frames =
            self.shared_buffer_pooled_response_frames.saturating_add(1);
    }

    pub fn record_shared_buffer_fallback_response(&mut self) {
        self.shared_buffer_fallback_response_frames = self
            .shared_buffer_fallback_response_frames
            .saturating_add(1);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ServerRunnerTickDiagnostics {
    pub simulation_tick: u64,
    pub chunk_tick: u64,
    pub block_tick_chunks: usize,
    pub entity_tick_chunks: usize,
    pub pending_unloads_processed: usize,
    pub scheduler_event_count: usize,
    pub fluid_due_ticks: usize,
    pub fluid_ticks_executed: usize,
    pub deferred_fluid_ticks: usize,
    pub fluid_mutated_blocks: usize,
    pub fluid_snapshot_events: usize,
    pub fluid_event_count: usize,
    pub scheduled_fluid_ticks: usize,
    pub natural_spawning: NaturalSpawningDiagnostics,
    pub physics: ServerPhysicsTickDiagnostics,
    pub wall_us: u128,
    pub timing: ServerSimulationTickTiming,
}

impl ServerRunnerTickDiagnostics {
    pub fn from_report(report: &ServerSimulationTickReport, wall_us: u128) -> Self {
        Self {
            simulation_tick: report.simulation_tick,
            chunk_tick: report.chunk_tick,
            block_tick_chunks: report.block_tick_chunks,
            entity_tick_chunks: report.entity_tick_chunks,
            pending_unloads_processed: report.pending_unloads_processed,
            scheduler_event_count: report.scheduler_event_count,
            fluid_due_ticks: report.fluid_due_ticks,
            fluid_ticks_executed: report.fluid_ticks_executed,
            deferred_fluid_ticks: report.deferred_fluid_ticks,
            fluid_mutated_blocks: report.fluid_mutated_blocks,
            fluid_snapshot_events: report.fluid_snapshot_events,
            fluid_event_count: report.fluid_event_count,
            scheduled_fluid_ticks: report.scheduled_fluid_ticks,
            natural_spawning: report.natural_spawning,
            physics: report.physics,
            wall_us,
            timing: report.timing,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerRunnerDiagnostics {
    pub kind: ServerRunnerKind,
    pub running: bool,
    pub seed: i64,
    pub day_time: u64,
    pub simulation_cadence: SimulationCadenceConfig,
    pub host_tick_interval: std::time::Duration,
    pub awaiting_tick: bool,
    pub command_queue_depth: usize,
    pub update_queue_depth: usize,
    pub update_queue_bytes: usize,
    pub pending_jobs: usize,
    pub pending_publications: usize,
    pub worldgen_mailbox_kind: WorldgenMailboxKind,
    pub light_status_mailbox_kind: LightStatusMailboxKind,
    pub worldgen_mailbox_pending_jobs: usize,
    pub light_status_mailbox_pending_statuses: usize,
    pub runner_frame_metrics: WorkerFrameMetrics,
    pub worldgen_job_frame_metrics: WorkerFrameMetrics,
    pub light_status_job_frame_metrics: WorkerFrameMetrics,
    pub scheduler_metrics: ChunkSchedulerMetrics,
    pub chunk_tracking: PlayerChunkTrackingDiagnostics,
    pub loading_progress: Option<ChunkLoadingProgressStats>,
    pub loading_progress_snapshot: Option<ChunkLoadingProgressSnapshot>,
    pub view_readiness_snapshot: Option<ChunkLoadingProgressSnapshot>,
    pub diagnostics_detail_refreshes: u64,
    pub diagnostics_detail_age_ms: f64,
    pub last_tick: ServerRunnerTickDiagnostics,
    pub last_error: Option<String>,
    diagnostics_detail_refreshed_at: Option<std::time::Instant>,
}

impl ServerRunnerDiagnostics {
    pub fn initial(kind: ServerRunnerKind, seed: i64, day_time: u64) -> Self {
        Self {
            kind,
            running: false,
            seed,
            day_time,
            simulation_cadence: SimulationCadenceConfig::default(),
            host_tick_interval: std::time::Duration::from_millis(50),
            awaiting_tick: false,
            command_queue_depth: 0,
            update_queue_depth: 0,
            update_queue_bytes: 0,
            pending_jobs: 0,
            pending_publications: 0,
            worldgen_mailbox_kind: WorldgenMailboxKind::Inline,
            light_status_mailbox_kind: LightStatusMailboxKind::Inline,
            worldgen_mailbox_pending_jobs: 0,
            light_status_mailbox_pending_statuses: 0,
            runner_frame_metrics: WorkerFrameMetrics::default(),
            worldgen_job_frame_metrics: WorkerFrameMetrics::default(),
            light_status_job_frame_metrics: WorkerFrameMetrics::default(),
            scheduler_metrics: ChunkSchedulerMetrics::default(),
            chunk_tracking: PlayerChunkTrackingDiagnostics::default(),
            loading_progress: None,
            loading_progress_snapshot: None,
            view_readiness_snapshot: None,
            diagnostics_detail_refreshes: 0,
            diagnostics_detail_age_ms: 0.0,
            last_tick: ServerRunnerTickDiagnostics::default(),
            last_error: None,
            diagnostics_detail_refreshed_at: None,
        }
    }
}

#[derive(Debug)]
pub enum ServerRunnerError {
    Protocol(ProtocolCodecError),
    ChunkStore(ChunkStoreError),
    CommandChannelClosed,
    UpdateChannelClosed,
    DiagnosticsPoisoned,
    InvalidCadenceConfig,
    ThreadSpawn(std::io::Error),
    ThreadStart(String),
    ThreadPanicked,
}

impl fmt::Display for ServerRunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => write!(f, "integrated server runner protocol failed: {error}"),
            Self::ChunkStore(error) => {
                write!(f, "integrated server runner storage failed: {error}")
            }
            Self::CommandChannelClosed => {
                f.write_str("integrated server runner command channel closed")
            }
            Self::UpdateChannelClosed => {
                f.write_str("integrated server runner update channel closed")
            }
            Self::DiagnosticsPoisoned => {
                f.write_str("integrated server runner diagnostics were poisoned")
            }
            Self::InvalidCadenceConfig => f.write_str("invalid integrated server runner cadence"),
            Self::ThreadSpawn(error) => write!(
                f,
                "failed to spawn integrated server runner thread: {error}"
            ),
            Self::ThreadStart(message) => {
                write!(f, "integrated server runner failed to start: {message}")
            }
            Self::ThreadPanicked => f.write_str("integrated server runner thread panicked"),
        }
    }
}

impl Error for ServerRunnerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Protocol(error) => Some(error),
            Self::ChunkStore(error) => Some(error),
            Self::ThreadSpawn(error) => Some(error),
            Self::CommandChannelClosed
            | Self::UpdateChannelClosed
            | Self::DiagnosticsPoisoned
            | Self::InvalidCadenceConfig
            | Self::ThreadStart(_)
            | Self::ThreadPanicked => None,
        }
    }
}

impl From<ProtocolCodecError> for ServerRunnerError {
    fn from(value: ProtocolCodecError) -> Self {
        Self::Protocol(value)
    }
}

impl From<ChunkStoreError> for ServerRunnerError {
    fn from(value: ChunkStoreError) -> Self {
        Self::ChunkStore(value)
    }
}

pub trait IntegratedServerRunner {
    fn kind(&self) -> ServerRunnerKind;
    fn send_command(&mut self, command: ClientCommand) -> ServerRunnerResult<()>;
    fn try_recv_update(&mut self) -> ServerRunnerResult<Option<ServerUpdateEnvelope>>;

    fn drain_updates(&mut self) -> ServerRunnerResult<Vec<ServerUpdate>> {
        let mut updates = Vec::new();
        while let Some(envelope) = self.try_recv_update()? {
            updates.push(envelope.update);
        }
        Ok(updates)
    }

    fn poll_diagnostics(&self) -> ServerRunnerResult<ServerRunnerDiagnostics>;
    fn request_shutdown(&mut self);
    fn join_shutdown(&mut self) -> ServerRunnerResult<()>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerUpdateEnvelope {
    pub update: ServerUpdate,
    pub encoded_len: usize,
    pub queued_age: std::time::Duration,
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::INITIAL_DAY_TIME;
    use crate::player_chunk_tracking::PlayerChunkTrackingPolicy;

    const DIAGNOSTICS_DETAIL_REFRESH_INTERVAL: Duration = Duration::from_millis(500);

    fn duration_ms(duration: Duration) -> f64 {
        duration.as_secs_f64() * 1000.0
    }

    #[derive(Debug, Default)]
    struct DiagnosticsDetailSampler {
        last_refresh: Option<Instant>,
    }

    impl DiagnosticsDetailSampler {
        fn should_refresh(&self, now: Instant, force: bool) -> bool {
            if force {
                return true;
            }
            match self.last_refresh {
                Some(last_refresh) => {
                    now.duration_since(last_refresh) >= DIAGNOSTICS_DETAIL_REFRESH_INTERVAL
                }
                None => true,
            }
        }

        fn mark_refreshed(&mut self, refreshed_at: Instant) {
            self.last_refresh = Some(refreshed_at);
        }
    }

    #[derive(Clone, Debug)]
    struct DiagnosticsDetailSnapshot {
        worldgen_mailbox_kind: WorldgenMailboxKind,
        light_status_mailbox_kind: LightStatusMailboxKind,
        worldgen_mailbox_pending_jobs: usize,
        light_status_mailbox_pending_statuses: usize,
        worldgen_job_frame_metrics: WorkerFrameMetrics,
        light_status_job_frame_metrics: WorkerFrameMetrics,
        scheduler_metrics: ChunkSchedulerMetrics,
        chunk_tracking: PlayerChunkTrackingDiagnostics,
        loading_progress: Option<ChunkLoadingProgressStats>,
        loading_progress_snapshot: Option<ChunkLoadingProgressSnapshot>,
        view_readiness_snapshot: Option<ChunkLoadingProgressSnapshot>,
    }

    impl DiagnosticsDetailSnapshot {
        fn from_server(server: &IntegratedServer) -> Self {
            Self {
                worldgen_mailbox_kind: server.scheduler().worldgen_mailbox_kind(),
                light_status_mailbox_kind: server.scheduler().light_status_mailbox_kind(),
                worldgen_mailbox_pending_jobs: server.scheduler().worldgen_mailbox_pending_count(),
                light_status_mailbox_pending_statuses: server
                    .scheduler()
                    .light_status_mailbox_pending_count(),
                worldgen_job_frame_metrics: server.scheduler().worldgen_mailbox_frame_metrics(),
                light_status_job_frame_metrics: server
                    .scheduler()
                    .light_status_mailbox_frame_metrics(),
                scheduler_metrics: server.scheduler().metrics(),
                chunk_tracking: server.chunk_tracking_diagnostics(),
                loading_progress: server.loading_progress_stats(),
                loading_progress_snapshot: server.loading_progress_snapshot(),
                view_readiness_snapshot: server.view_readiness_snapshot(),
            }
        }
    }

    fn should_force_detail_after_tick(report: &ServerSimulationTickReport) -> bool {
        report.scheduler_event_count > 0
            || report.pending_unloads_processed > 0
            || report.fluid_event_count > 0
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct NativeIntegratedServerRunnerConfig {
        pub seed: i64,
        pub lighting_enabled: bool,
        pub day_time: Option<u64>,
        pub day_time_frozen: bool,
        pub debug_passive_showcase: bool,
        pub tick_interval: Duration,
        pub cadence: SimulationCadenceConfig,
        player_chunk_tracking_policy: PlayerChunkTrackingPolicy,
    }

    impl NativeIntegratedServerRunnerConfig {
        pub const fn new(seed: i64) -> Self {
            Self {
                seed,
                lighting_enabled: true,
                day_time: None,
                day_time_frozen: false,
                debug_passive_showcase: true,
                tick_interval: Duration::from_millis(50),
                cadence: SimulationCadenceConfig::new(20, 20, 60),
                player_chunk_tracking_policy: PlayerChunkTrackingPolicy::dedicated_default(),
            }
        }

        pub const fn with_lighting_enabled(mut self, enabled: bool) -> Self {
            self.lighting_enabled = enabled;
            self
        }

        pub const fn with_day_time(mut self, day_time: Option<u64>) -> Self {
            self.day_time = day_time;
            self
        }

        pub const fn with_day_time_frozen(mut self, frozen: bool) -> Self {
            self.day_time_frozen = frozen;
            self
        }

        pub const fn with_debug_passive_showcase(mut self, enabled: bool) -> Self {
            self.debug_passive_showcase = enabled;
            self
        }

        pub const fn with_local_integrated_chunk_tracking(mut self) -> Self {
            self.player_chunk_tracking_policy =
                PlayerChunkTrackingPolicy::java_max().with_unload_hysteresis_chunks(1);
            self
        }

        pub const fn with_tick_interval(mut self, tick_interval: Duration) -> Self {
            self.tick_interval = tick_interval;
            self
        }

        pub const fn with_cadence(mut self, cadence: SimulationCadenceConfig) -> Self {
            self.cadence = cadence;
            self
        }

        pub fn with_cadence_derived_tick_interval(
            mut self,
            cadence: SimulationCadenceConfig,
        ) -> Self {
            self.cadence = cadence;
            self.tick_interval = host_tick_interval_for_rate_hz(cadence.host_rate_hz);
            self
        }

        const fn initial_day_time(self) -> u64 {
            match self.day_time {
                Some(day_time) => day_time,
                None => INITIAL_DAY_TIME,
            }
        }
    }

    pub fn host_tick_interval_for_rate_hz(host_rate_hz: u32) -> Duration {
        if host_rate_hz == 0 {
            return Duration::ZERO;
        }
        let host_rate_hz = u64::from(host_rate_hz);
        Duration::from_nanos((1_000_000_000 + host_rate_hz / 2) / host_rate_hz)
    }

    #[derive(Debug)]
    enum NativeRunnerControl {
        Command(Vec<u8>),
        SetSimulationCadence {
            cadence: SimulationCadenceConfig,
            tick_interval: Duration,
        },
        Shutdown,
    }

    #[derive(Clone, Debug)]
    struct NativeRunnerTimingState {
        tick_interval: Duration,
        cadence: SimulationCadence,
        cadence_config: SimulationCadenceConfig,
    }

    impl NativeRunnerTimingState {
        fn new(tick_interval: Duration, cadence_config: SimulationCadenceConfig) -> Option<Self> {
            Some(Self {
                tick_interval,
                cadence: SimulationCadence::new(cadence_config)?,
                cadence_config,
            })
        }

        fn set_cadence(
            &mut self,
            cadence_config: SimulationCadenceConfig,
            tick_interval: Duration,
        ) -> ServerRunnerResult<()> {
            let cadence = SimulationCadence::new(cadence_config)
                .ok_or(ServerRunnerError::InvalidCadenceConfig)?;
            self.cadence = cadence;
            self.cadence_config = cadence_config;
            self.tick_interval = tick_interval;
            Ok(())
        }

        fn physics_step_dt_seconds(&self) -> f64 {
            1.0 / f64::from(self.cadence_config.physics_rate_hz)
        }
    }

    #[derive(Debug)]
    pub struct NativeIntegratedServerRunner {
        command_tx: Option<mpsc::Sender<NativeRunnerControl>>,
        update_rx: mpsc::Receiver<NativeQueuedServerUpdate>,
        command_queue_depth: Arc<AtomicUsize>,
        update_queue_depth: Arc<AtomicUsize>,
        update_queue_bytes: Arc<AtomicUsize>,
        diagnostics: Arc<Mutex<ServerRunnerDiagnostics>>,
        join: Option<JoinHandle<ServerRunnerResult<()>>>,
        shutdown_requested: bool,
    }

    struct NativeQueuedServerUpdate {
        update: ServerUpdate,
        encoded_len: usize,
        queued_at: Instant,
    }

    impl NativeIntegratedServerRunner {
        pub fn new(config: NativeIntegratedServerRunnerConfig) -> ServerRunnerResult<Self> {
            let (command_tx, command_rx) = mpsc::channel();
            let (update_tx, update_rx) = mpsc::channel();
            let (ready_tx, ready_rx) = mpsc::channel();
            let command_queue_depth = Arc::new(AtomicUsize::new(0));
            let update_queue_depth = Arc::new(AtomicUsize::new(0));
            let update_queue_bytes = Arc::new(AtomicUsize::new(0));
            let mut initial_diagnostics = ServerRunnerDiagnostics::initial(
                ServerRunnerKind::NativeThread,
                config.seed,
                config.initial_day_time(),
            );
            initial_diagnostics.simulation_cadence = config.cadence;
            initial_diagnostics.host_tick_interval = config.tick_interval;
            let diagnostics = Arc::new(Mutex::new(initial_diagnostics));

            let thread_command_depth = Arc::clone(&command_queue_depth);
            let thread_update_depth = Arc::clone(&update_queue_depth);
            let thread_update_bytes = Arc::clone(&update_queue_bytes);
            let thread_diagnostics = Arc::clone(&diagnostics);
            let join = thread::Builder::new()
                .name("mclone integrated server".to_owned())
                .spawn(move || {
                    run_native_integrated_server(
                        config,
                        command_rx,
                        update_tx,
                        thread_command_depth,
                        thread_update_depth,
                        thread_update_bytes,
                        thread_diagnostics,
                        ready_tx,
                    )
                })
                .map_err(ServerRunnerError::ThreadSpawn)?;

            match ready_rx.recv() {
                Ok(Ok(())) => {}
                Ok(Err(message)) => {
                    let _ = join.join();
                    return Err(ServerRunnerError::ThreadStart(message));
                }
                Err(_) => {
                    return match join.join() {
                        Ok(Err(error)) => Err(error),
                        Ok(Ok(())) => Err(ServerRunnerError::CommandChannelClosed),
                        Err(_) => Err(ServerRunnerError::ThreadPanicked),
                    };
                }
            }

            Ok(Self {
                command_tx: Some(command_tx),
                update_rx,
                command_queue_depth,
                update_queue_depth,
                update_queue_bytes,
                diagnostics,
                join: Some(join),
                shutdown_requested: false,
            })
        }

        pub fn refresh_fast_diagnostics(&self, diagnostics: &mut ServerRunnerDiagnostics) {
            diagnostics.command_queue_depth = self.command_queue_depth.load(Ordering::SeqCst);
            diagnostics.update_queue_depth = self.update_queue_depth.load(Ordering::SeqCst);
            diagnostics.update_queue_bytes = self.update_queue_bytes.load(Ordering::SeqCst);
            if let Some(refreshed_at) = diagnostics.diagnostics_detail_refreshed_at {
                diagnostics.diagnostics_detail_age_ms = duration_ms(refreshed_at.elapsed());
            }
        }

        pub fn set_simulation_cadence(
            &mut self,
            cadence: SimulationCadenceConfig,
        ) -> ServerRunnerResult<()> {
            if self.shutdown_requested {
                return Err(ServerRunnerError::CommandChannelClosed);
            }
            if SimulationCadence::new(cadence).is_none() {
                return Err(ServerRunnerError::InvalidCadenceConfig);
            }
            let tick_interval = host_tick_interval_for_rate_hz(cadence.host_rate_hz);
            self.command_queue_depth.fetch_add(1, Ordering::SeqCst);
            let send_result = self
                .command_tx
                .as_ref()
                .ok_or(ServerRunnerError::CommandChannelClosed)?
                .send(NativeRunnerControl::SetSimulationCadence {
                    cadence,
                    tick_interval,
                });
            if send_result.is_err() {
                self.command_queue_depth.fetch_sub(1, Ordering::SeqCst);
                return Err(ServerRunnerError::CommandChannelClosed);
            }
            Ok(())
        }
    }

    impl IntegratedServerRunner for NativeIntegratedServerRunner {
        fn kind(&self) -> ServerRunnerKind {
            ServerRunnerKind::NativeThread
        }

        fn send_command(&mut self, command: ClientCommand) -> ServerRunnerResult<()> {
            if self.shutdown_requested {
                return Err(ServerRunnerError::CommandChannelClosed);
            }
            let frame = encode_client_command(&command)?;
            self.command_queue_depth.fetch_add(1, Ordering::SeqCst);
            let send_result = self
                .command_tx
                .as_ref()
                .ok_or(ServerRunnerError::CommandChannelClosed)?
                .send(NativeRunnerControl::Command(frame));
            if send_result.is_err() {
                self.command_queue_depth.fetch_sub(1, Ordering::SeqCst);
                return Err(ServerRunnerError::CommandChannelClosed);
            }
            Ok(())
        }

        fn try_recv_update(&mut self) -> ServerRunnerResult<Option<ServerUpdateEnvelope>> {
            match self.update_rx.try_recv() {
                Ok(queued) => {
                    let queued_age = queued.queued_at.elapsed();
                    self.update_queue_depth.fetch_sub(1, Ordering::SeqCst);
                    self.update_queue_bytes
                        .fetch_sub(queued.encoded_len, Ordering::SeqCst);
                    Ok(Some(ServerUpdateEnvelope {
                        update: queued.update,
                        encoded_len: queued.encoded_len,
                        queued_age,
                    }))
                }
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => Ok(None),
            }
        }

        fn poll_diagnostics(&self) -> ServerRunnerResult<ServerRunnerDiagnostics> {
            let mut diagnostics = self
                .diagnostics
                .lock()
                .map_err(|_| ServerRunnerError::DiagnosticsPoisoned)?
                .clone();
            self.refresh_fast_diagnostics(&mut diagnostics);
            Ok(diagnostics)
        }

        fn request_shutdown(&mut self) {
            if self.shutdown_requested {
                return;
            }
            self.shutdown_requested = true;
            if let Some(command_tx) = &self.command_tx {
                let _ = command_tx.send(NativeRunnerControl::Shutdown);
            }
        }

        fn join_shutdown(&mut self) -> ServerRunnerResult<()> {
            self.request_shutdown();
            self.command_tx.take();
            let Some(join) = self.join.take() else {
                return Ok(());
            };
            match join.join() {
                Ok(result) => result,
                Err(_) => Err(ServerRunnerError::ThreadPanicked),
            }
        }
    }

    impl Drop for NativeIntegratedServerRunner {
        fn drop(&mut self) {
            let _ = self.join_shutdown();
        }
    }

    fn run_native_integrated_server(
        config: NativeIntegratedServerRunnerConfig,
        command_rx: mpsc::Receiver<NativeRunnerControl>,
        update_tx: mpsc::Sender<NativeQueuedServerUpdate>,
        command_queue_depth: Arc<AtomicUsize>,
        update_queue_depth: Arc<AtomicUsize>,
        update_queue_bytes: Arc<AtomicUsize>,
        diagnostics: Arc<Mutex<ServerRunnerDiagnostics>>,
        ready_tx: mpsc::Sender<Result<(), String>>,
    ) -> ServerRunnerResult<()> {
        let mut server = IntegratedServer::with_player_chunk_tracking_policy(
            config.seed,
            config.player_chunk_tracking_policy,
        );
        server.set_lighting_enabled(config.lighting_enabled);
        server.set_day_time_frozen(config.day_time_frozen);
        server.set_debug_passive_showcase_enabled(config.debug_passive_showcase);
        if let Some(day_time) = config.day_time {
            server.set_day_time(day_time);
        }
        let Some(timing_state) = NativeRunnerTimingState::new(config.tick_interval, config.cadence)
        else {
            let _ = ready_tx.send(Err("invalid native server cadence config".to_owned()));
            return Ok(());
        };
        let mut diagnostics_detail_sampler = DiagnosticsDetailSampler::default();
        refresh_diagnostics(
            &diagnostics,
            &server,
            &command_queue_depth,
            &update_queue_depth,
            &update_queue_bytes,
            &mut diagnostics_detail_sampler,
            None,
            true,
            Some(false),
            None,
            true,
        );
        let _ = ready_tx.send(Ok(()));

        let result = run_native_integrated_server_loop(
            &mut server,
            timing_state,
            command_rx,
            update_tx,
            &command_queue_depth,
            &update_queue_depth,
            &update_queue_bytes,
            &diagnostics,
            &mut diagnostics_detail_sampler,
        );
        refresh_diagnostics(
            &diagnostics,
            &server,
            &command_queue_depth,
            &update_queue_depth,
            &update_queue_bytes,
            &mut diagnostics_detail_sampler,
            None,
            false,
            Some(false),
            result.as_ref().err().map(ToString::to_string),
            true,
        );
        result
    }

    fn run_native_integrated_server_loop(
        server: &mut IntegratedServer,
        mut timing_state: NativeRunnerTimingState,
        command_rx: mpsc::Receiver<NativeRunnerControl>,
        update_tx: mpsc::Sender<NativeQueuedServerUpdate>,
        command_queue_depth: &AtomicUsize,
        update_queue_depth: &AtomicUsize,
        update_queue_bytes: &AtomicUsize,
        diagnostics: &Arc<Mutex<ServerRunnerDiagnostics>>,
        diagnostics_detail_sampler: &mut DiagnosticsDetailSampler,
    ) -> ServerRunnerResult<()> {
        let mut next_tick = Instant::now();
        loop {
            let now = Instant::now();
            if now < next_tick {
                if !recv_until_next_tick(
                    server,
                    &command_rx,
                    &update_tx,
                    command_queue_depth,
                    update_queue_depth,
                    update_queue_bytes,
                    diagnostics,
                    diagnostics_detail_sampler,
                    next_tick - now,
                    &mut timing_state,
                    &mut next_tick,
                )? {
                    return Ok(());
                }
                continue;
            }

            if !drain_available_commands(
                server,
                &command_rx,
                &update_tx,
                command_queue_depth,
                update_queue_depth,
                update_queue_bytes,
                diagnostics,
                diagnostics_detail_sampler,
                &mut timing_state,
                &mut next_tick,
            )? {
                return Ok(());
            }

            let frame = timing_state.cadence.advance_host_frame();
            let mut last_gameplay_tick = None;
            for _ in 0..frame.gameplay_ticks {
                let wall_start = Instant::now();
                let mut report = server.try_simulation_tick_report_with_physics_steps(0)?;
                let wall_us = wall_start.elapsed().as_micros();
                let updates = std::mem::take(&mut report.updates);
                publish_updates(&update_tx, update_queue_depth, update_queue_bytes, updates)?;
                last_gameplay_tick = Some((report, wall_us));
            }

            let mut physics_wall_us = 0;
            let mut physics_report = None;
            let mut physics_published_updates = false;
            if frame.physics_steps > 0 {
                let wall_start = Instant::now();
                let mut report = server.try_physics_step_report_with_step_dt(
                    frame.physics_steps,
                    timing_state.physics_step_dt_seconds(),
                )?;
                physics_wall_us = wall_start.elapsed().as_micros();
                let updates = std::mem::take(&mut report.updates);
                physics_published_updates = !updates.is_empty();
                publish_updates(&update_tx, update_queue_depth, update_queue_bytes, updates)?;
                physics_report = Some(report);
            }

            if let Some((mut report, wall_us)) = last_gameplay_tick {
                let mut force_detail_after_tick = should_force_detail_after_tick(&report);
                if let Some(physics_report) = physics_report {
                    report.physics = physics_report.physics;
                    report.timing.physics_tick_us = report
                        .timing
                        .physics_tick_us
                        .saturating_add(physics_report.timing.physics_tick_us);
                    report.timing.total_us = report
                        .timing
                        .total_us
                        .saturating_add(physics_report.timing.total_us);
                    force_detail_after_tick |= physics_published_updates;
                }
                refresh_diagnostics(
                    diagnostics,
                    server,
                    command_queue_depth,
                    update_queue_depth,
                    update_queue_bytes,
                    diagnostics_detail_sampler,
                    Some(ServerRunnerTickDiagnostics::from_report(
                        &report,
                        wall_us.saturating_add(physics_wall_us),
                    )),
                    true,
                    Some(false),
                    None,
                    force_detail_after_tick,
                );
            } else if physics_report.is_some() {
                refresh_diagnostics(
                    diagnostics,
                    server,
                    command_queue_depth,
                    update_queue_depth,
                    update_queue_bytes,
                    diagnostics_detail_sampler,
                    None,
                    true,
                    Some(false),
                    None,
                    physics_published_updates,
                );
            }

            next_tick += timing_state.tick_interval;
            if next_tick <= Instant::now() {
                next_tick = Instant::now() + timing_state.tick_interval;
            }
        }
    }

    fn recv_until_next_tick(
        server: &mut IntegratedServer,
        command_rx: &mpsc::Receiver<NativeRunnerControl>,
        update_tx: &mpsc::Sender<NativeQueuedServerUpdate>,
        command_queue_depth: &AtomicUsize,
        update_queue_depth: &AtomicUsize,
        update_queue_bytes: &AtomicUsize,
        diagnostics: &Arc<Mutex<ServerRunnerDiagnostics>>,
        diagnostics_detail_sampler: &mut DiagnosticsDetailSampler,
        timeout: Duration,
        timing_state: &mut NativeRunnerTimingState,
        next_tick: &mut Instant,
    ) -> ServerRunnerResult<bool> {
        match command_rx.recv_timeout(timeout) {
            Ok(control) => process_control(
                server,
                control,
                update_tx,
                command_queue_depth,
                update_queue_depth,
                update_queue_bytes,
                diagnostics,
                diagnostics_detail_sampler,
                timing_state,
                next_tick,
            ),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(true),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(false),
        }
    }

    fn drain_available_commands(
        server: &mut IntegratedServer,
        command_rx: &mpsc::Receiver<NativeRunnerControl>,
        update_tx: &mpsc::Sender<NativeQueuedServerUpdate>,
        command_queue_depth: &AtomicUsize,
        update_queue_depth: &AtomicUsize,
        update_queue_bytes: &AtomicUsize,
        diagnostics: &Arc<Mutex<ServerRunnerDiagnostics>>,
        diagnostics_detail_sampler: &mut DiagnosticsDetailSampler,
        timing_state: &mut NativeRunnerTimingState,
        next_tick: &mut Instant,
    ) -> ServerRunnerResult<bool> {
        loop {
            match command_rx.try_recv() {
                Ok(control) => {
                    if !process_control(
                        server,
                        control,
                        update_tx,
                        command_queue_depth,
                        update_queue_depth,
                        update_queue_bytes,
                        diagnostics,
                        diagnostics_detail_sampler,
                        timing_state,
                        next_tick,
                    )? {
                        return Ok(false);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => return Ok(true),
                Err(mpsc::TryRecvError::Disconnected) => return Ok(false),
            }
        }
    }

    fn process_control(
        server: &mut IntegratedServer,
        control: NativeRunnerControl,
        update_tx: &mpsc::Sender<NativeQueuedServerUpdate>,
        command_queue_depth: &AtomicUsize,
        update_queue_depth: &AtomicUsize,
        update_queue_bytes: &AtomicUsize,
        diagnostics: &Arc<Mutex<ServerRunnerDiagnostics>>,
        diagnostics_detail_sampler: &mut DiagnosticsDetailSampler,
        timing_state: &mut NativeRunnerTimingState,
        next_tick: &mut Instant,
    ) -> ServerRunnerResult<bool> {
        match control {
            NativeRunnerControl::Shutdown => Ok(false),
            NativeRunnerControl::SetSimulationCadence {
                cadence,
                tick_interval,
            } => {
                let result = timing_state.set_cadence(cadence, tick_interval);
                command_queue_depth.fetch_sub(1, Ordering::SeqCst);
                result?;
                *next_tick = Instant::now() + timing_state.tick_interval;
                refresh_cadence_diagnostics(diagnostics, timing_state);
                refresh_diagnostics(
                    diagnostics,
                    server,
                    command_queue_depth,
                    update_queue_depth,
                    update_queue_bytes,
                    diagnostics_detail_sampler,
                    None,
                    true,
                    Some(false),
                    None,
                    true,
                );
                Ok(true)
            }
            NativeRunnerControl::Command(frame) => {
                refresh_diagnostics(
                    diagnostics,
                    server,
                    command_queue_depth,
                    update_queue_depth,
                    update_queue_bytes,
                    diagnostics_detail_sampler,
                    None,
                    true,
                    Some(true),
                    None,
                    false,
                );
                let command = decode_client_command(&frame).map_err(ServerRunnerError::from)?;
                let force_detail_after_command = matches!(command, ClientCommand::SetChunkView(_));
                let result = server
                    .try_handle_command(command)
                    .map_err(ServerRunnerError::from);
                command_queue_depth.fetch_sub(1, Ordering::SeqCst);
                let updates = result?;
                publish_updates(update_tx, update_queue_depth, update_queue_bytes, updates)?;
                refresh_diagnostics(
                    diagnostics,
                    server,
                    command_queue_depth,
                    update_queue_depth,
                    update_queue_bytes,
                    diagnostics_detail_sampler,
                    None,
                    true,
                    Some(true),
                    None,
                    force_detail_after_command,
                );
                Ok(true)
            }
        }
    }

    fn refresh_cadence_diagnostics(
        diagnostics: &Arc<Mutex<ServerRunnerDiagnostics>>,
        timing_state: &NativeRunnerTimingState,
    ) {
        let Ok(mut diagnostics) = diagnostics.lock() else {
            return;
        };
        diagnostics.simulation_cadence = timing_state.cadence_config;
        diagnostics.host_tick_interval = timing_state.tick_interval;
    }

    fn publish_updates(
        update_tx: &mpsc::Sender<NativeQueuedServerUpdate>,
        update_queue_depth: &AtomicUsize,
        update_queue_bytes: &AtomicUsize,
        updates: Vec<ServerUpdate>,
    ) -> ServerRunnerResult<()> {
        for update in updates {
            let encoded_len = encode_server_update(&update)?.len();
            update_queue_depth.fetch_add(1, Ordering::SeqCst);
            update_queue_bytes.fetch_add(encoded_len, Ordering::SeqCst);
            let queued = NativeQueuedServerUpdate {
                update,
                encoded_len,
                queued_at: Instant::now(),
            };
            if update_tx.send(queued).is_err() {
                update_queue_depth.fetch_sub(1, Ordering::SeqCst);
                update_queue_bytes.fetch_sub(encoded_len, Ordering::SeqCst);
                return Err(ServerRunnerError::UpdateChannelClosed);
            }
        }
        Ok(())
    }

    fn refresh_diagnostics(
        diagnostics: &Arc<Mutex<ServerRunnerDiagnostics>>,
        server: &IntegratedServer,
        command_queue_depth: &AtomicUsize,
        update_queue_depth: &AtomicUsize,
        update_queue_bytes: &AtomicUsize,
        diagnostics_detail_sampler: &mut DiagnosticsDetailSampler,
        tick: Option<ServerRunnerTickDiagnostics>,
        running: bool,
        awaiting_tick: Option<bool>,
        last_error: Option<String>,
        force_detail: bool,
    ) {
        let now = Instant::now();
        let day_time = server.day_time();
        let command_queue_depth = command_queue_depth.load(Ordering::SeqCst);
        let update_queue_depth = update_queue_depth.load(Ordering::SeqCst);
        let update_queue_bytes = update_queue_bytes.load(Ordering::SeqCst);
        let pending_jobs = server.pending_job_count();
        let pending_publications = server.pending_publication_count();
        let detail_snapshot = diagnostics_detail_sampler
            .should_refresh(now, force_detail)
            .then(|| DiagnosticsDetailSnapshot::from_server(server));
        if detail_snapshot.is_some() {
            diagnostics_detail_sampler.mark_refreshed(now);
        }
        let Ok(mut diagnostics) = diagnostics.lock() else {
            return;
        };
        diagnostics.running = running;
        diagnostics.day_time = day_time;
        diagnostics.command_queue_depth = command_queue_depth;
        diagnostics.update_queue_depth = update_queue_depth;
        diagnostics.update_queue_bytes = update_queue_bytes;
        diagnostics.pending_jobs = pending_jobs;
        diagnostics.pending_publications = pending_publications;
        if let Some(detail_snapshot) = detail_snapshot {
            diagnostics.worldgen_mailbox_kind = detail_snapshot.worldgen_mailbox_kind;
            diagnostics.light_status_mailbox_kind = detail_snapshot.light_status_mailbox_kind;
            diagnostics.worldgen_mailbox_pending_jobs =
                detail_snapshot.worldgen_mailbox_pending_jobs;
            diagnostics.light_status_mailbox_pending_statuses =
                detail_snapshot.light_status_mailbox_pending_statuses;
            diagnostics.worldgen_job_frame_metrics = detail_snapshot.worldgen_job_frame_metrics;
            diagnostics.light_status_job_frame_metrics =
                detail_snapshot.light_status_job_frame_metrics;
            diagnostics.scheduler_metrics = detail_snapshot.scheduler_metrics;
            diagnostics.chunk_tracking = detail_snapshot.chunk_tracking;
            diagnostics.loading_progress = detail_snapshot.loading_progress;
            diagnostics.loading_progress_snapshot = detail_snapshot.loading_progress_snapshot;
            diagnostics.view_readiness_snapshot = detail_snapshot.view_readiness_snapshot;
            diagnostics.diagnostics_detail_refreshes += 1;
            diagnostics.diagnostics_detail_age_ms = 0.0;
            diagnostics.diagnostics_detail_refreshed_at = Some(now);
        } else if let Some(refreshed_at) = diagnostics.diagnostics_detail_refreshed_at {
            diagnostics.diagnostics_detail_age_ms = duration_ms(refreshed_at.elapsed());
        }
        if let Some(awaiting_tick) = awaiting_tick {
            diagnostics.awaiting_tick = awaiting_tick;
        }
        if let Some(tick) = tick {
            diagnostics.last_tick = tick;
        }
        if let Some(last_error) = last_error {
            diagnostics.last_error = Some(last_error);
        }
    }

    #[cfg(test)]
    mod tests {
        use std::time::{Duration, Instant};

        use mclone_core::ChunkPos;
        use mclone_protocol::{ChunkView, ServerUpdate};

        use super::*;

        #[test]
        fn native_runner_reports_native_thread_kind_and_queue_depths() {
            let mut runner = NativeIntegratedServerRunner::new(
                NativeIntegratedServerRunnerConfig::new(0)
                    .with_lighting_enabled(false)
                    .with_tick_interval(Duration::from_secs(60)),
            )
            .unwrap();

            assert_eq!(runner.kind(), ServerRunnerKind::NativeThread);
            let mut updates = Vec::new();
            drain_runner_until_idle(&mut runner, &mut updates);
            let diagnostics = runner.poll_diagnostics().unwrap();
            assert_eq!(diagnostics.kind, ServerRunnerKind::NativeThread);
            assert!(diagnostics.running);
            assert_eq!(diagnostics.command_queue_depth, 0);
            assert_eq!(diagnostics.update_queue_depth, 0);
            assert_eq!(
                diagnostics.runner_frame_metrics.transport_kind,
                WorkerFrameTransportKind::None
            );
            assert_eq!(
                diagnostics.worldgen_job_frame_metrics.transport_kind,
                WorkerFrameTransportKind::None
            );
            assert_eq!(
                diagnostics.light_status_job_frame_metrics.transport_kind,
                WorkerFrameTransportKind::None
            );

            runner.join_shutdown().unwrap();
        }

        #[test]
        fn native_runner_config_defaults_to_twenty_hz_cadence() {
            let config = NativeIntegratedServerRunnerConfig::new(0);

            assert_eq!(config.cadence, SimulationCadenceConfig::default());
            assert_eq!(
                host_tick_interval_for_rate_hz(60),
                Duration::from_nanos(16_666_667)
            );
            assert_eq!(
                SimulationCadence::new(config.cadence)
                    .expect("default native cadence")
                    .advance_host_frame(),
                crate::SimulationCadenceFrame {
                    gameplay_ticks: 1,
                    physics_steps: 3,
                }
            );
        }

        #[test]
        fn native_runner_rejects_invalid_cadence_config() {
            let result = NativeIntegratedServerRunner::new(
                NativeIntegratedServerRunnerConfig::new(0)
                    .with_cadence(SimulationCadenceConfig::new(0, 20, 60)),
            );

            assert!(matches!(
                result,
                Err(ServerRunnerError::ThreadStart(message))
                    if message == "invalid native server cadence config"
            ));
        }

        #[test]
        fn native_runner_config_can_derive_tick_interval_from_cadence() {
            let cadence = SimulationCadenceConfig::new(60, 20, 60);
            let config = NativeIntegratedServerRunnerConfig::new(0)
                .with_cadence_derived_tick_interval(cadence);

            assert_eq!(config.cadence, cadence);
            assert_eq!(config.tick_interval, Duration::from_nanos(16_666_667));
        }

        #[test]
        fn native_runner_config_can_use_local_integrated_chunk_tracking_policy() {
            let center = ChunkPos::new(0, 0);
            let requested = ChunkView {
                center,
                render_distance: 32,
                chunk_tracking_radius: 32,
            };
            let dedicated_policy =
                NativeIntegratedServerRunnerConfig::new(0).player_chunk_tracking_policy;
            let local_integrated_policy = NativeIntegratedServerRunnerConfig::new(0)
                .with_local_integrated_chunk_tracking()
                .player_chunk_tracking_policy;
            let dedicated = dedicated_policy.clamp_view(&requested);
            let local_integrated = local_integrated_policy.clamp_view(&requested);

            assert_eq!(dedicated.render_distance, 11);
            assert_eq!(dedicated.chunk_tracking_radius, 11);
            assert_eq!(dedicated_policy.unload_hysteresis_chunks(), 0);
            assert_eq!(local_integrated.render_distance, 32);
            assert_eq!(local_integrated.chunk_tracking_radius, 32);
            assert_eq!(local_integrated_policy.unload_hysteresis_chunks(), 1);
        }

        #[test]
        fn native_runner_applies_live_simulation_cadence_control() {
            let mut runner = NativeIntegratedServerRunner::new(
                test_runner_config(0).with_tick_interval(Duration::from_secs(60)),
            )
            .unwrap();
            let initial = runner.poll_diagnostics().unwrap();
            assert_eq!(
                initial.simulation_cadence,
                SimulationCadenceConfig::default()
            );
            assert_eq!(initial.host_tick_interval, Duration::from_secs(60));

            let cadence = SimulationCadenceConfig::new(60, 20, 60);
            runner.set_simulation_cadence(cadence).unwrap();

            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let diagnostics = runner.poll_diagnostics().unwrap();
                if diagnostics.command_queue_depth == 0 && diagnostics.simulation_cadence == cadence
                {
                    assert_eq!(
                        diagnostics.host_tick_interval,
                        Duration::from_nanos(16_666_667)
                    );
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for live cadence control, diagnostics={diagnostics:?}"
                );
                std::thread::sleep(Duration::from_millis(1));
            }

            runner.join_shutdown().unwrap();
        }

        #[test]
        fn native_runner_rejects_invalid_live_simulation_cadence_control() {
            let mut runner = NativeIntegratedServerRunner::new(test_runner_config(0)).unwrap();

            let error = runner
                .set_simulation_cadence(SimulationCadenceConfig::new(30, 20, 60))
                .unwrap_err();

            assert!(matches!(error, ServerRunnerError::InvalidCadenceConfig));
            runner.join_shutdown().unwrap();
        }

        #[test]
        fn worker_frame_metrics_accumulate_message_transfer_frames() {
            let mut metrics = WorkerFrameMetrics::message_transfer();
            assert_eq!(
                metrics.transport_kind,
                WorkerFrameTransportKind::MessageTransfer
            );
            assert_eq!(metrics.transport_kind.label(), "message-transfer");

            metrics.record_request(7);
            metrics.record_request(11);
            metrics.record_response(13);
            metrics.observe_pending_frames(1);
            metrics.observe_pending_frames(3);
            metrics.observe_pending_frames(2);
            metrics.record_request_time_us(5);
            metrics.record_request_time_us(2);
            metrics.record_shared_buffer_pool_hit();
            metrics.record_shared_buffer_pool_miss();
            metrics.record_shared_buffer_capacity_delta(64);
            metrics.record_shared_buffer_capacity_delta(32);
            metrics.record_shared_buffer_pooled_response();
            metrics.record_shared_buffer_fallback_response();
            metrics.record_shared_buffer_pool_drop(16);

            assert_eq!(metrics.request_frames, 2);
            assert_eq!(metrics.request_bytes, 18);
            assert_eq!(metrics.response_frames, 1);
            assert_eq!(metrics.response_bytes, 13);
            assert_eq!(metrics.max_pending_frames, 3);
            assert_eq!(metrics.last_request_us, 2);
            assert_eq!(metrics.total_request_us, 7);
            assert_eq!(metrics.max_request_us, 5);
            assert_eq!(metrics.shared_buffer_pool_hits, 1);
            assert_eq!(metrics.shared_buffer_pool_misses, 1);
            assert_eq!(metrics.shared_buffer_pool_drops, 1);
            assert_eq!(metrics.shared_buffer_capacity_bytes, 80);
            assert_eq!(metrics.max_shared_buffer_capacity_bytes, 96);
            assert_eq!(metrics.shared_buffer_pooled_response_frames, 1);
            assert_eq!(metrics.shared_buffer_fallback_response_frames, 1);

            let shared = WorkerFrameMetrics::shared_memory();
            assert_eq!(
                shared.transport_kind,
                WorkerFrameTransportKind::SharedMemory
            );
            assert_eq!(shared.transport_kind.label(), "shared-memory");
        }

        #[test]
        fn native_runner_publish_updates_queues_decoded_updates_with_encoded_byte_accounting() {
            let (update_tx, update_rx) = mpsc::channel();
            let update_queue_depth = AtomicUsize::new(0);
            let update_queue_bytes = AtomicUsize::new(0);
            let update = ServerUpdate::TimeUpdate { day_time: 42 };
            let expected_len = encode_server_update(&update).unwrap().len();

            publish_updates(
                &update_tx,
                &update_queue_depth,
                &update_queue_bytes,
                vec![update.clone()],
            )
            .unwrap();

            assert_eq!(update_queue_depth.load(Ordering::SeqCst), 1);
            assert_eq!(update_queue_bytes.load(Ordering::SeqCst), expected_len);
            let queued = update_rx.try_recv().unwrap();
            assert_eq!(queued.update, update);
            assert_eq!(queued.encoded_len, expected_len);
            assert!(queued.queued_at.elapsed() < Duration::from_secs(1));
        }

        #[test]
        fn native_runner_crosses_commands_and_updates_over_frames() {
            let mut runner = NativeIntegratedServerRunner::new(test_runner_config(12_345)).unwrap();
            runner
                .send_command(ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }))
                .unwrap();

            let mut updates = drain_until(&mut runner, |updates| {
                updates
                    .iter()
                    .any(|update| matches!(update, ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == ChunkPos::new(0, 0)))
            });

            assert!(updates.iter().any(|update| {
                matches!(update, ServerUpdate::ChunkSnapshot(snapshot) if snapshot.pos == ChunkPos::new(0, 0))
            }));
            drain_runner_until_idle(&mut runner, &mut updates);
            let diagnostics = runner.poll_diagnostics().unwrap();
            assert_eq!(diagnostics.kind, ServerRunnerKind::NativeThread);
            assert_eq!(diagnostics.command_queue_depth, 0);
            assert_eq!(diagnostics.update_queue_depth, 0);
            assert_eq!(diagnostics.pending_jobs, 0);
            assert!(diagnostics.last_tick.simulation_tick > 0);
            runner.join_shutdown().unwrap();
        }

        #[test]
        fn diagnostics_detail_refresh_predicate_tracks_scheduler_visible_activity() {
            assert!(!should_force_detail_after_tick(
                &tick_report_for_detail_refresh(0, 0, 0)
            ));
            assert!(should_force_detail_after_tick(
                &tick_report_for_detail_refresh(1, 0, 0)
            ));
            assert!(should_force_detail_after_tick(
                &tick_report_for_detail_refresh(0, 1, 0)
            ));
            assert!(should_force_detail_after_tick(
                &tick_report_for_detail_refresh(0, 0, 1)
            ));
        }

        #[test]
        fn native_runner_shutdown_joins_cleanly() {
            let mut runner = NativeIntegratedServerRunner::new(test_runner_config(0)).unwrap();
            runner.request_shutdown();
            runner.join_shutdown().unwrap();
            runner.join_shutdown().unwrap();

            let diagnostics = runner.poll_diagnostics().unwrap();
            assert!(!diagnostics.running);
            assert!(diagnostics.last_error.is_none());
        }

        fn test_runner_config(seed: i64) -> NativeIntegratedServerRunnerConfig {
            NativeIntegratedServerRunnerConfig::new(seed)
                .with_lighting_enabled(false)
                .with_tick_interval(Duration::from_millis(1))
        }

        fn tick_report_for_detail_refresh(
            scheduler_event_count: usize,
            pending_unloads_processed: usize,
            fluid_event_count: usize,
        ) -> ServerSimulationTickReport {
            ServerSimulationTickReport {
                simulation_tick: 0,
                chunk_tick: 0,
                block_tick_chunks: 0,
                fluid_ticks_executed: 0,
                fluid_due_ticks: 0,
                deferred_fluid_ticks: 0,
                fluid_mutated_blocks: 0,
                fluid_snapshot_events: 0,
                fluid_event_count,
                scheduled_fluid_ticks: 0,
                entity_tick_chunks: 0,
                natural_spawning: NaturalSpawningDiagnostics::default(),
                physics: ServerPhysicsTickDiagnostics::default(),
                pending_unloads_processed,
                scheduler_event_count,
                chunk_tracking: PlayerChunkTrackingDiagnostics::default(),
                updates: Vec::new(),
                timing: ServerSimulationTickTiming::default(),
            }
        }

        fn drain_until(
            runner: &mut NativeIntegratedServerRunner,
            mut done: impl FnMut(&[ServerUpdate]) -> bool,
        ) -> Vec<ServerUpdate> {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut updates = Vec::new();
            loop {
                updates.extend(runner.drain_updates().unwrap());
                if done(&updates) {
                    return updates;
                }
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for runner updates; diagnostics={:?}",
                    runner.poll_diagnostics().unwrap()
                );
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        fn drain_runner_until_idle(
            runner: &mut NativeIntegratedServerRunner,
            updates: &mut Vec<ServerUpdate>,
        ) {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                updates.extend(runner.drain_updates().unwrap());
                let diagnostics = runner.poll_diagnostics().unwrap();
                if diagnostics.command_queue_depth == 0
                    && diagnostics.update_queue_depth == 0
                    && !diagnostics.awaiting_tick
                    && diagnostics.pending_jobs == 0
                    && diagnostics.pending_publications == 0
                {
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for runner idle; diagnostics={diagnostics:?}"
                );
                if diagnostics.update_queue_depth == 0 {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{
    NativeIntegratedServerRunner, NativeIntegratedServerRunnerConfig,
    host_tick_interval_for_rate_hz,
};
