use anyhow::{Context, Result, bail};
use mclone_client::ClientRuntime;
use mclone_core::ChunkPos;
use mclone_protocol::{ClientCommand, ServerUpdate};
use mclone_server::ServerRunnerDiagnostics;

use crate::{
    RuntimeExchange, RuntimeStepReport, SingleViewRuntime,
    chunk_tracking_radius_for_render_distance, chunk_view,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SingleViewHostMode {
    LocalIntegrated,
    RemoteDedicated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SingleViewHostOptions {
    pub center: ChunkPos,
    pub render_distance: u32,
    pub chunk_tracking_radius: u32,
    pub render_compile_worker_count: usize,
}

impl SingleViewHostOptions {
    pub fn new(center: ChunkPos, render_distance: u32) -> Self {
        Self {
            center,
            render_distance,
            chunk_tracking_radius: chunk_tracking_radius_for_render_distance(render_distance),
            render_compile_worker_count:
                crate::render_assets::DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
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

    pub fn chunk_view_command(&self) -> ClientCommand {
        ClientCommand::SetChunkView(chunk_view(
            self.center,
            self.render_distance,
            self.chunk_tracking_radius,
        ))
    }
}

pub trait RemoteDedicatedServerSession {
    fn send_command(&mut self, command: ClientCommand) -> Result<Vec<ServerUpdate>>;
    fn reconnect(&mut self) -> Result<()>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum RemoteDedicatedExchangeReport<E> {
    Applied {
        changed: bool,
        report: RuntimeStepReport,
    },
    ReconnectAndResync {
        command: ClientCommand,
        error: E,
    },
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

pub fn build_remote_dedicated_client_runtime<S>(
    options: SingleViewHostOptions,
    session: &mut S,
) -> Result<ClientRuntime>
where
    S: RemoteDedicatedServerSession + ?Sized,
{
    let mut runtime = SingleViewRuntime::remote_dedicated(
        options.center,
        options.render_distance,
        options.chunk_tracking_radius,
    );
    let Some(command) = runtime.set_chunk_view_command(
        options.center,
        options.render_distance,
        options.chunk_tracking_radius,
    ) else {
        return Ok(runtime.client().clone());
    };
    let updates = session
        .send_command(command)
        .context("failed to initialize remote dedicated chunk view")?;
    runtime.client_mut().apply_updates(updates);
    Ok(runtime.client().clone())
}

pub fn dispatch_remote_dedicated_command<S>(
    runtime: &mut SingleViewRuntime,
    session: &mut S,
    command: ClientCommand,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession + ?Sized,
{
    let result = session
        .send_command(command)
        .map(|updates| command_exchange(updates, true, true));
    match resolve_remote_dedicated_exchange_report(runtime, result)? {
        RemoteDedicatedExchangeReport::Applied { changed, .. } => Ok(changed),
        RemoteDedicatedExchangeReport::ReconnectAndResync { command, error: _ } => {
            reconnect_remote_dedicated_session_and_resync(runtime, session, command)
        }
    }
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

pub fn resolve_remote_dedicated_exchange_report<E>(
    runtime: &mut SingleViewRuntime,
    result: std::result::Result<RuntimeExchange, E>,
) -> Result<RemoteDedicatedExchangeReport<E>>
where
    E: std::fmt::Display,
{
    match result {
        Ok(exchange) => {
            let (changed, report) =
                apply_remote_dedicated_command_exchange_report(runtime, exchange);
            Ok(RemoteDedicatedExchangeReport::Applied { changed, report })
        }
        Err(error) => {
            let command = prepare_remote_dedicated_resync_command_for_error(runtime, &error)?;
            Ok(RemoteDedicatedExchangeReport::ReconnectAndResync { command, error })
        }
    }
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
    let updates = session
        .send_command(command)
        .context("failed to resync remote dedicated chunk view after reconnect")?;
    runtime.apply_server_updates(updates);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_client::ClientHost;
    use mclone_core::{
        AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkSnapshot, ChunkStatus,
    };
    use mclone_protocol::ChunkView;

    #[derive(Debug)]
    struct ScriptedRemoteSession {
        sends: Vec<Result<Vec<ServerUpdate>>>,
        reconnects: usize,
    }

    impl ScriptedRemoteSession {
        fn new(sends: Vec<Result<Vec<ServerUpdate>>>) -> Self {
            Self {
                sends: sends.into_iter().rev().collect(),
                reconnects: 0,
            }
        }
    }

    impl RemoteDedicatedServerSession for ScriptedRemoteSession {
        fn send_command(&mut self, _command: ClientCommand) -> Result<Vec<ServerUpdate>> {
            self.sends
                .pop()
                .expect("scripted remote session missing send result")
        }

        fn reconnect(&mut self) -> Result<()> {
            self.reconnects += 1;
            Ok(())
        }
    }

    #[test]
    fn remote_dedicated_client_runtime_loads_initial_chunk_view() {
        let center = ChunkPos::new(0, 0);
        let mut session = ScriptedRemoteSession::new(vec![Ok(vec![ServerUpdate::ChunkSnapshot(
            empty_test_snapshot(center),
        )])]);

        let client = build_remote_dedicated_client_runtime(
            SingleViewHostOptions::new(center, 0),
            &mut session,
        )
        .unwrap();

        assert_eq!(client.host(), ClientHost::RemoteDedicated);
        assert_eq!(
            client.chunk_view(),
            Some(&ChunkView {
                center,
                render_distance: 0,
                chunk_tracking_radius: 0,
            })
        );
        assert!(client.chunk_snapshot(center).is_some());
    }

    #[test]
    fn remote_dedicated_dispatch_applies_updates() {
        let center = ChunkPos::new(0, 0);
        let mut runtime = SingleViewRuntime::remote_dedicated(center, 0, 0);
        let mut session =
            ScriptedRemoteSession::new(vec![Ok(vec![ServerUpdate::TimeUpdate { day_time: 6000 }])]);

        assert!(
            dispatch_remote_dedicated_command(
                &mut runtime,
                &mut session,
                ClientCommand::SetChunkView(ChunkView {
                    center,
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }),
            )
            .unwrap()
        );

        assert_eq!(runtime.day_time(), 6000);
        assert_eq!(runtime.command_count(), 1);
    }

    #[test]
    fn host_exchange_helpers_preserve_command_and_drain_accounting() {
        assert_eq!(
            command_exchange(vec![ServerUpdate::TimeUpdate { day_time: 42 }], false, true),
            RuntimeExchange::new(
                vec![ServerUpdate::TimeUpdate { day_time: 42 }],
                1,
                false,
                true
            )
        );
        assert_eq!(
            deferred_command_exchange(),
            RuntimeExchange::new(Vec::new(), 1, true, false)
        );
        assert_eq!(
            update_drain_exchange(vec![ServerUpdate::TimeUpdate { day_time: 43 }], false),
            RuntimeExchange::updates(vec![ServerUpdate::TimeUpdate { day_time: 43 }], false)
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
        let mut session = ScriptedRemoteSession::new(vec![
            Err(anyhow::anyhow!("dropped connection")),
            Ok(vec![ServerUpdate::ChunkSnapshot(empty_test_snapshot(
                moved_center,
            ))]),
        ]);

        assert!(dispatch_remote_dedicated_command(&mut runtime, &mut session, command).unwrap());

        assert_eq!(session.reconnects, 1);
        assert!(runtime.client().chunk_snapshot(initial_center).is_none());
        assert!(runtime.client().chunk_snapshot(moved_center).is_some());
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
