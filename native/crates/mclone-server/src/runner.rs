//! Integrated server runner boundary.
//!
//! The runner owns the `IntegratedServer` and its tick loop. Client/runtime code
//! talks to it through encoded protocol frames so local integrated mode exercises
//! the same command/update wire shape as remote transports.

use std::error::Error;
use std::fmt;

use mclone_protocol::{ClientCommand, ProtocolCodecError, ServerUpdate};
#[cfg(not(target_arch = "wasm32"))]
use mclone_protocol::{
    decode_client_command, decode_server_update, encode_client_command, encode_server_update,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::IntegratedServer;
use crate::{
    ChunkSchedulerMetrics, ChunkStoreError, LightStatusMailboxKind, PlayerChunkTrackingDiagnostics,
    ServerSimulationTickReport, ServerSimulationTickTiming, WorldgenMailboxKind,
};

pub type ServerRunnerResult<T> = Result<T, ServerRunnerError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerRunnerKind {
    InlineFallback,
    NativeThread,
    WebWorker,
}

impl ServerRunnerKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::InlineFallback => "inline-fallback",
            Self::NativeThread => "native-thread",
            Self::WebWorker => "web-worker",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerFrameTransportKind {
    None,
    MessageTransfer,
    SharedMemory,
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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
    pub awaiting_tick: bool,
    pub command_queue_depth: usize,
    pub update_queue_depth: usize,
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
    pub last_tick: ServerRunnerTickDiagnostics,
    pub last_error: Option<String>,
}

impl ServerRunnerDiagnostics {
    pub fn initial(kind: ServerRunnerKind, seed: i64, day_time: u64) -> Self {
        Self {
            kind,
            running: false,
            seed,
            day_time,
            awaiting_tick: false,
            command_queue_depth: 0,
            update_queue_depth: 0,
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
            last_tick: ServerRunnerTickDiagnostics::default(),
            last_error: None,
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
    fn drain_updates(&mut self) -> ServerRunnerResult<Vec<ServerUpdate>>;
    fn poll_diagnostics(&self) -> ServerRunnerResult<ServerRunnerDiagnostics>;
    fn request_shutdown(&mut self);
    fn join_shutdown(&mut self) -> ServerRunnerResult<()>;
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::INITIAL_DAY_TIME;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct NativeIntegratedServerRunnerConfig {
        pub seed: i64,
        pub lighting_enabled: bool,
        pub day_time: Option<u64>,
        pub day_time_frozen: bool,
        pub tick_interval: Duration,
    }

    impl NativeIntegratedServerRunnerConfig {
        pub const fn new(seed: i64) -> Self {
            Self {
                seed,
                lighting_enabled: true,
                day_time: None,
                day_time_frozen: false,
                tick_interval: Duration::from_millis(50),
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

        pub const fn with_tick_interval(mut self, tick_interval: Duration) -> Self {
            self.tick_interval = tick_interval;
            self
        }

        const fn initial_day_time(self) -> u64 {
            match self.day_time {
                Some(day_time) => day_time,
                None => INITIAL_DAY_TIME,
            }
        }
    }

    #[derive(Debug)]
    enum NativeRunnerControl {
        Command(Vec<u8>),
        Shutdown,
    }

    #[derive(Debug)]
    pub struct NativeIntegratedServerRunner {
        command_tx: Option<mpsc::Sender<NativeRunnerControl>>,
        update_rx: mpsc::Receiver<Vec<u8>>,
        command_queue_depth: Arc<AtomicUsize>,
        update_queue_depth: Arc<AtomicUsize>,
        diagnostics: Arc<Mutex<ServerRunnerDiagnostics>>,
        join: Option<JoinHandle<ServerRunnerResult<()>>>,
        shutdown_requested: bool,
    }

    impl NativeIntegratedServerRunner {
        pub fn new(config: NativeIntegratedServerRunnerConfig) -> ServerRunnerResult<Self> {
            let (command_tx, command_rx) = mpsc::channel();
            let (update_tx, update_rx) = mpsc::channel();
            let (ready_tx, ready_rx) = mpsc::channel();
            let command_queue_depth = Arc::new(AtomicUsize::new(0));
            let update_queue_depth = Arc::new(AtomicUsize::new(0));
            let diagnostics = Arc::new(Mutex::new(ServerRunnerDiagnostics::initial(
                ServerRunnerKind::NativeThread,
                config.seed,
                config.initial_day_time(),
            )));

            let thread_command_depth = Arc::clone(&command_queue_depth);
            let thread_update_depth = Arc::clone(&update_queue_depth);
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
                diagnostics,
                join: Some(join),
                shutdown_requested: false,
            })
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

        fn drain_updates(&mut self) -> ServerRunnerResult<Vec<ServerUpdate>> {
            let mut updates = Vec::new();
            loop {
                match self.update_rx.try_recv() {
                    Ok(frame) => {
                        self.update_queue_depth.fetch_sub(1, Ordering::SeqCst);
                        updates.push(decode_server_update(&frame)?);
                    }
                    Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => {
                        return Ok(updates);
                    }
                }
            }
        }

        fn poll_diagnostics(&self) -> ServerRunnerResult<ServerRunnerDiagnostics> {
            let mut diagnostics = self
                .diagnostics
                .lock()
                .map_err(|_| ServerRunnerError::DiagnosticsPoisoned)?
                .clone();
            diagnostics.command_queue_depth = self.command_queue_depth.load(Ordering::SeqCst);
            diagnostics.update_queue_depth = self.update_queue_depth.load(Ordering::SeqCst);
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
        update_tx: mpsc::Sender<Vec<u8>>,
        command_queue_depth: Arc<AtomicUsize>,
        update_queue_depth: Arc<AtomicUsize>,
        diagnostics: Arc<Mutex<ServerRunnerDiagnostics>>,
        ready_tx: mpsc::Sender<Result<(), String>>,
    ) -> ServerRunnerResult<()> {
        let mut server = IntegratedServer::new(config.seed);
        server.set_lighting_enabled(config.lighting_enabled);
        server.set_day_time_frozen(config.day_time_frozen);
        if let Some(day_time) = config.day_time {
            server.set_day_time(day_time);
        }
        refresh_diagnostics(
            &diagnostics,
            &server,
            &command_queue_depth,
            &update_queue_depth,
            None,
            true,
            Some(false),
            None,
        );
        let _ = ready_tx.send(Ok(()));

        let result = run_native_integrated_server_loop(
            &mut server,
            config.tick_interval,
            command_rx,
            update_tx,
            &command_queue_depth,
            &update_queue_depth,
            &diagnostics,
        );
        refresh_diagnostics(
            &diagnostics,
            &server,
            &command_queue_depth,
            &update_queue_depth,
            None,
            false,
            Some(false),
            result.as_ref().err().map(ToString::to_string),
        );
        result
    }

    fn run_native_integrated_server_loop(
        server: &mut IntegratedServer,
        tick_interval: Duration,
        command_rx: mpsc::Receiver<NativeRunnerControl>,
        update_tx: mpsc::Sender<Vec<u8>>,
        command_queue_depth: &AtomicUsize,
        update_queue_depth: &AtomicUsize,
        diagnostics: &Arc<Mutex<ServerRunnerDiagnostics>>,
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
                    diagnostics,
                    next_tick - now,
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
                diagnostics,
            )? {
                return Ok(());
            }

            let wall_start = Instant::now();
            let report = server.try_simulation_tick_report()?;
            let wall_us = wall_start.elapsed().as_micros();
            publish_updates(&update_tx, update_queue_depth, &report.updates)?;
            refresh_diagnostics(
                diagnostics,
                server,
                command_queue_depth,
                update_queue_depth,
                Some(ServerRunnerTickDiagnostics::from_report(&report, wall_us)),
                true,
                Some(false),
                None,
            );

            next_tick += tick_interval;
            if next_tick <= Instant::now() {
                next_tick = Instant::now() + tick_interval;
            }
        }
    }

    fn recv_until_next_tick(
        server: &mut IntegratedServer,
        command_rx: &mpsc::Receiver<NativeRunnerControl>,
        update_tx: &mpsc::Sender<Vec<u8>>,
        command_queue_depth: &AtomicUsize,
        update_queue_depth: &AtomicUsize,
        diagnostics: &Arc<Mutex<ServerRunnerDiagnostics>>,
        timeout: Duration,
    ) -> ServerRunnerResult<bool> {
        match command_rx.recv_timeout(timeout) {
            Ok(control) => process_control(
                server,
                control,
                update_tx,
                command_queue_depth,
                update_queue_depth,
                diagnostics,
            ),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(true),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(false),
        }
    }

    fn drain_available_commands(
        server: &mut IntegratedServer,
        command_rx: &mpsc::Receiver<NativeRunnerControl>,
        update_tx: &mpsc::Sender<Vec<u8>>,
        command_queue_depth: &AtomicUsize,
        update_queue_depth: &AtomicUsize,
        diagnostics: &Arc<Mutex<ServerRunnerDiagnostics>>,
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
                        diagnostics,
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
        update_tx: &mpsc::Sender<Vec<u8>>,
        command_queue_depth: &AtomicUsize,
        update_queue_depth: &AtomicUsize,
        diagnostics: &Arc<Mutex<ServerRunnerDiagnostics>>,
    ) -> ServerRunnerResult<bool> {
        match control {
            NativeRunnerControl::Shutdown => Ok(false),
            NativeRunnerControl::Command(frame) => {
                refresh_diagnostics(
                    diagnostics,
                    server,
                    command_queue_depth,
                    update_queue_depth,
                    None,
                    true,
                    Some(true),
                    None,
                );
                let result = decode_client_command(&frame)
                    .map_err(ServerRunnerError::from)
                    .and_then(|command| server.try_handle_command(command).map_err(Into::into));
                command_queue_depth.fetch_sub(1, Ordering::SeqCst);
                let updates = result?;
                publish_updates(update_tx, update_queue_depth, &updates)?;
                refresh_diagnostics(
                    diagnostics,
                    server,
                    command_queue_depth,
                    update_queue_depth,
                    None,
                    true,
                    Some(true),
                    None,
                );
                Ok(true)
            }
        }
    }

    fn publish_updates(
        update_tx: &mpsc::Sender<Vec<u8>>,
        update_queue_depth: &AtomicUsize,
        updates: &[ServerUpdate],
    ) -> ServerRunnerResult<()> {
        for update in updates {
            let frame = encode_server_update(update)?;
            update_queue_depth.fetch_add(1, Ordering::SeqCst);
            if update_tx.send(frame).is_err() {
                update_queue_depth.fetch_sub(1, Ordering::SeqCst);
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
        tick: Option<ServerRunnerTickDiagnostics>,
        running: bool,
        awaiting_tick: Option<bool>,
        last_error: Option<String>,
    ) {
        let Ok(mut diagnostics) = diagnostics.lock() else {
            return;
        };
        diagnostics.running = running;
        diagnostics.day_time = server.day_time();
        diagnostics.command_queue_depth = command_queue_depth.load(Ordering::SeqCst);
        diagnostics.update_queue_depth = update_queue_depth.load(Ordering::SeqCst);
        diagnostics.pending_jobs = server.pending_job_count();
        diagnostics.pending_publications = server.pending_publication_count();
        diagnostics.worldgen_mailbox_kind = server.scheduler().worldgen_mailbox_kind();
        diagnostics.light_status_mailbox_kind = server.scheduler().light_status_mailbox_kind();
        diagnostics.worldgen_mailbox_pending_jobs =
            server.scheduler().worldgen_mailbox_pending_count();
        diagnostics.light_status_mailbox_pending_statuses =
            server.scheduler().light_status_mailbox_pending_count();
        diagnostics.worldgen_job_frame_metrics =
            server.scheduler().worldgen_mailbox_frame_metrics();
        diagnostics.light_status_job_frame_metrics =
            server.scheduler().light_status_mailbox_frame_metrics();
        diagnostics.scheduler_metrics = server.scheduler().metrics();
        diagnostics.chunk_tracking = server.chunk_tracking_diagnostics();
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
            let _ = runner.drain_updates().unwrap();
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

            assert_eq!(metrics.request_frames, 2);
            assert_eq!(metrics.request_bytes, 18);
            assert_eq!(metrics.response_frames, 1);
            assert_eq!(metrics.response_bytes, 13);
            assert_eq!(metrics.max_pending_frames, 3);
            assert_eq!(metrics.last_request_us, 2);
            assert_eq!(metrics.total_request_us, 7);
            assert_eq!(metrics.max_request_us, 5);

            let shared = WorkerFrameMetrics::shared_memory();
            assert_eq!(
                shared.transport_kind,
                WorkerFrameTransportKind::SharedMemory
            );
            assert_eq!(shared.transport_kind.label(), "shared-memory");
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
pub use native::{NativeIntegratedServerRunner, NativeIntegratedServerRunnerConfig};
