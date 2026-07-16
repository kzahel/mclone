use std::time::Duration;

use anyhow::{Context, Result, bail};
use mclone_core::ChunkPos;
use mclone_protocol::{ClientCommand, ServerUpdate};
use mclone_server::ServerRunnerDiagnostics;

use crate::{
    RuntimeExchange, RuntimeStepReport, SingleViewRuntime,
    chunk_tracking_radius_for_render_distance, chunk_view,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SingleViewHostMode {
    #[default]
    LocalIntegrated,
    RemoteDedicated,
}

impl SingleViewHostMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::LocalIntegrated => "local-integrated",
            Self::RemoteDedicated => "remote-dedicated",
        }
    }

    pub const fn server_owned_lanes_are_remote(self) -> bool {
        matches!(self, Self::RemoteDedicated)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SingleViewHostOptions {
    pub center: ChunkPos,
    pub render_distance: u32,
    pub chunk_tracking_radius: u32,
    pub render_compile_worker_count: usize,
    pub render_compile_max_pending_jobs: Option<usize>,
    pub render_compile_worker_timing_enabled: bool,
}

impl SingleViewHostOptions {
    pub fn new(center: ChunkPos, render_distance: u32) -> Self {
        Self {
            center,
            render_distance,
            chunk_tracking_radius: chunk_tracking_radius_for_render_distance(render_distance),
            render_compile_worker_count: crate::DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
            render_compile_max_pending_jobs: Some(
                crate::DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS,
            ),
            render_compile_worker_timing_enabled: true,
        }
    }

    pub const fn with_chunk_tracking_radius(mut self, chunk_tracking_radius: u32) -> Self {
        self.chunk_tracking_radius = chunk_tracking_radius;
        self
    }

    pub const fn with_render_compile_worker_count(
        mut self,
        render_compile_worker_count: usize,
    ) -> Self {
        self.render_compile_worker_count = render_compile_worker_count;
        self
    }

    pub const fn with_render_compile_max_pending_jobs(
        mut self,
        render_compile_max_pending_jobs: Option<usize>,
    ) -> Self {
        self.render_compile_max_pending_jobs = render_compile_max_pending_jobs;
        self
    }

    pub const fn with_render_compile_worker_timing_enabled(mut self, enabled: bool) -> Self {
        self.render_compile_worker_timing_enabled = enabled;
        self
    }

    pub fn chunk_view_command(&self) -> ClientCommand {
        ClientCommand::SetChunkView(chunk_view(
            self.center,
            self.render_distance,
            self.chunk_tracking_radius,
        ))
    }
}

/// Runtime-facing remote session adapter over an ordered server-update stream.
///
/// Receipt is always ready-only. Socket reads and frame decode belong to the
/// producer, so a drawable caller can never turn this poll into transport IO.
pub trait RemoteDedicatedServerSession {
    fn send_command_only(&mut self, command: ClientCommand) -> Result<()>;
    fn try_drain_update_batch(&mut self) -> Result<Option<RemoteServerUpdateBatch>>;
    fn pending_update_metrics(&self) -> RemoteUpdateQueueMetrics;
    fn reconnect(&mut self) -> Result<()>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RemoteUpdateQueueMetrics {
    pub frame_depth: usize,
    pub update_depth: usize,
    pub update_bytes: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RemoteServerUpdate {
    pub update: ServerUpdate,
    pub encoded_len: Option<usize>,
    pub queued_age: Duration,
    pub inbound_frame_sequence: Option<u64>,
    pub producer_read_ms: f64,
    pub producer_decode_ms: f64,
}

impl RemoteServerUpdate {
    pub fn legacy(update: ServerUpdate) -> Self {
        Self {
            update,
            encoded_len: None,
            queued_age: Duration::ZERO,
            inbound_frame_sequence: None,
            producer_read_ms: 0.0,
            producer_decode_ms: 0.0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteServerUpdateBatch {
    pub updates: Vec<RemoteServerUpdate>,
    pub inbound_frame_sequence: Option<u64>,
    pub producer_read_ms: f64,
    pub producer_decode_ms: f64,
}

impl RemoteServerUpdateBatch {
    pub fn from_updates(updates: Vec<ServerUpdate>) -> Self {
        Self {
            updates: updates
                .into_iter()
                .map(RemoteServerUpdate::legacy)
                .collect(),
            inbound_frame_sequence: None,
            producer_read_ms: 0.0,
            producer_decode_ms: 0.0,
        }
    }

    pub fn into_updates(self) -> Vec<ServerUpdate> {
        self.updates
            .into_iter()
            .map(|update| update.update)
            .collect()
    }
}

pub fn command_exchange(
    updates: Vec<ServerUpdate>,
    protocol_codec_roundtrip: bool,
    transport_drained: bool,
) -> RuntimeExchange {
    RuntimeExchange::new(updates, 1, protocol_codec_roundtrip, transport_drained)
}

pub fn deferred_command_exchange() -> RuntimeExchange {
    RuntimeExchange::new(Vec::new(), 1, true, false)
}

pub fn update_drain_exchange(
    updates: Vec<ServerUpdate>,
    transport_drained: bool,
) -> RuntimeExchange {
    RuntimeExchange::updates(updates, transport_drained)
}

pub fn diagnostics_command_update_queues_drained(diagnostics: &ServerRunnerDiagnostics) -> bool {
    diagnostics.command_queue_depth == 0 && diagnostics.update_queue_depth == 0
}

pub fn diagnostics_worker_exchange_drained(diagnostics: &ServerRunnerDiagnostics) -> bool {
    diagnostics_command_update_queues_drained(diagnostics)
        && diagnostics.pending_jobs == 0
        && diagnostics.pending_publications == 0
}

pub fn apply_remote_dedicated_command_updates(
    runtime: &mut SingleViewRuntime,
    updates: Vec<ServerUpdate>,
) -> bool {
    apply_remote_dedicated_command_exchange(runtime, command_exchange(updates, true, true))
}

pub fn apply_remote_dedicated_command_exchange(
    runtime: &mut SingleViewRuntime,
    exchange: RuntimeExchange,
) -> bool {
    apply_remote_dedicated_command_exchange_report(runtime, exchange).0
}

pub fn apply_remote_dedicated_command_exchange_report(
    runtime: &mut SingleViewRuntime,
    exchange: RuntimeExchange,
) -> (bool, RuntimeStepReport) {
    let changed = !exchange.updates.is_empty();
    let report = runtime.apply_exchange(exchange);
    (changed, report)
}

pub fn prepare_remote_dedicated_resync_command(
    runtime: &mut SingleViewRuntime,
    err: anyhow::Error,
) -> Result<ClientCommand> {
    prepare_remote_dedicated_resync_command_for_error(runtime, err)
}

pub fn prepare_remote_dedicated_resync_command_for_error(
    runtime: &mut SingleViewRuntime,
    err: impl std::fmt::Display,
) -> Result<ClientCommand> {
    let Some(view) = runtime.client().chunk_view().cloned() else {
        bail!("remote dedicated command failed before a chunk view was established: {err:#}");
    };

    log::warn!("remote dedicated command failed; reconnecting and resyncing chunk view: {err}");
    runtime.clear_client_replica_and_mark_render_dirty();
    Ok(ClientCommand::SetChunkView(view))
}

pub fn reconnect_remote_dedicated_session_and_resync<S>(
    runtime: &mut SingleViewRuntime,
    session: &mut S,
    command: ClientCommand,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession + ?Sized,
{
    session.reconnect()?;
    session
        .send_command_only(command)
        .context("failed to resync remote dedicated chunk view after reconnect")?;
    runtime.apply_exchange(deferred_command_exchange());
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{
        AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkSnapshot, ChunkStatus,
    };
    use mclone_protocol::ChunkView;

    fn time_update(day_time: u64) -> ServerUpdate {
        ServerUpdate::TimeUpdate {
            game_time: day_time.saturating_add(100),
            day_time,
            daylight_cycle_running: true,
        }
    }

    #[derive(Debug)]
    struct ScriptedRemoteSession {
        batches: Vec<Option<Result<RemoteServerUpdateBatch>>>,
        send_only_count: usize,
        poll_count: usize,
        reconnects: usize,
    }

    impl ScriptedRemoteSession {
        fn new(batches: Vec<Option<Result<Vec<ServerUpdate>>>>) -> Self {
            Self {
                batches: batches
                    .into_iter()
                    .map(|batch| {
                        batch.map(|result| result.map(RemoteServerUpdateBatch::from_updates))
                    })
                    .rev()
                    .collect(),
                send_only_count: 0,
                poll_count: 0,
                reconnects: 0,
            }
        }
    }

    impl RemoteDedicatedServerSession for ScriptedRemoteSession {
        fn send_command_only(&mut self, _command: ClientCommand) -> Result<()> {
            self.send_only_count += 1;
            Ok(())
        }

        fn try_drain_update_batch(&mut self) -> Result<Option<RemoteServerUpdateBatch>> {
            self.poll_count += 1;
            match self.batches.pop().unwrap_or(None) {
                Some(result) => result.map(Some),
                None => Ok(None),
            }
        }

        fn pending_update_metrics(&self) -> RemoteUpdateQueueMetrics {
            RemoteUpdateQueueMetrics::default()
        }

        fn reconnect(&mut self) -> Result<()> {
            self.reconnects += 1;
            Ok(())
        }
    }

    #[test]
    fn remote_session_send_and_ready_poll_are_independent() {
        let center = ChunkPos::new(0, 0);
        let mut session = ScriptedRemoteSession::new(vec![
            None,
            Some(Ok(vec![ServerUpdate::ChunkSnapshot(empty_test_snapshot(
                center,
            ))])),
        ]);

        session
            .send_command_only(ClientCommand::SetChunkView(ChunkView {
                center,
                render_distance: 0,
                chunk_tracking_radius: 0,
            }))
            .unwrap();
        assert_eq!(session.send_only_count, 1);
        assert!(session.try_drain_update_batch().unwrap().is_none());
        let batch = session.try_drain_update_batch().unwrap().unwrap();
        assert_eq!(batch.updates.len(), 1);
        assert_eq!(session.poll_count, 2);
    }

    #[test]
    fn host_exchange_helpers_preserve_command_and_drain_accounting() {
        assert_eq!(
            command_exchange(vec![time_update(42)], false, true),
            RuntimeExchange::new(vec![time_update(42)], 1, false, true)
        );
        assert_eq!(
            deferred_command_exchange(),
            RuntimeExchange::new(Vec::new(), 1, true, false)
        );
        assert_eq!(
            update_drain_exchange(vec![time_update(43)], false),
            RuntimeExchange::updates(vec![time_update(43)], false)
        );
    }

    #[test]
    fn remote_dedicated_dispatch_reconnects_and_resyncs_current_chunk_view() {
        let initial_center = ChunkPos::new(0, 0);
        let moved_center = ChunkPos::new(1, 0);
        let mut runtime = SingleViewRuntime::remote_dedicated(initial_center, 0, 0);
        let command = runtime.set_chunk_view_command(moved_center, 0, 0).unwrap();
        runtime.apply_server_updates(vec![ServerUpdate::ChunkSnapshot(empty_test_snapshot(
            initial_center,
        ))]);
        let mut session = ScriptedRemoteSession::new(Vec::new());
        let resync =
            prepare_remote_dedicated_resync_command_for_error(&mut runtime, "dropped connection")
                .unwrap();
        assert_eq!(resync, command);
        assert!(
            reconnect_remote_dedicated_session_and_resync(&mut runtime, &mut session, resync)
                .unwrap()
        );

        assert_eq!(session.reconnects, 1);
        assert_eq!(session.send_only_count, 1);
        assert!(runtime.client().chunk_snapshot(initial_center).is_none());
        assert!(runtime.client().chunk_snapshot(moved_center).is_none());
        assert_eq!(runtime.command_count(), 1);
    }

    fn empty_test_snapshot(pos: ChunkPos) -> ChunkSnapshot {
        ChunkSnapshot::from_block_state_ids(
            pos,
            ChunkStatus::Full,
            ChunkRevision(1),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        )
    }
}
