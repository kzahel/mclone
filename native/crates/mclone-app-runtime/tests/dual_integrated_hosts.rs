#![cfg(not(target_arch = "wasm32"))]

use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use mclone_app_runtime::client_connection::{
    ClientConnection, ConnectionUpdateDrainMode, IntegratedRunnerConnection,
    pump_client_connection_updates_report,
};
use mclone_app_runtime::{RuntimeUpdatePumpBudget, SingleViewRuntime};
use mclone_core::ChunkPos;
use mclone_protocol::{ChunkView, ClientCommand};
use mclone_server::{NativeIntegratedServerRunner, NativeIntegratedServerRunnerConfig};

const FIRST_SEED: i64 = 12_345;
const SECOND_SEED: i64 = -98_765;
const FIRST_CENTER: ChunkPos = ChunkPos::new(0, 0);
const SECOND_CENTER: ChunkPos = ChunkPos::new(32, -24);

struct WarmIntegratedHost {
    seed: i64,
    center: ChunkPos,
    runtime: SingleViewRuntime,
    connection: IntegratedRunnerConnection<NativeIntegratedServerRunner>,
}

impl WarmIntegratedHost {
    fn start(seed: i64, center: ChunkPos) -> Result<Self> {
        let runner = NativeIntegratedServerRunner::new(
            NativeIntegratedServerRunnerConfig::new(seed)
                .with_lighting_enabled(false)
                .with_local_integrated_chunk_tracking(),
        )
        .with_context(|| format!("start integrated host for seed {seed}"))?;
        let mut connection = IntegratedRunnerConnection::new(runner);
        connection
            .send_command_only(ClientCommand::SetChunkView(ChunkView {
                center,
                render_distance: 0,
                chunk_tracking_radius: 0,
            }))
            .with_context(|| format!("request initial view for seed {seed}"))?;

        Ok(Self {
            seed,
            center,
            runtime: SingleViewRuntime::local_integrated_with_seed(seed, center, 0, 0),
            connection,
        })
    }

    fn pump_ready_updates(&mut self) -> Result<()> {
        pump_client_connection_updates_report(
            &mut self.runtime,
            &mut self.connection,
            RuntimeUpdatePumpBudget::unlimited(),
            ConnectionUpdateDrainMode::ReadyOnly,
        )?;
        Ok(())
    }

    fn is_warm(&self) -> Result<bool> {
        let diagnostics = self.connection.poll_diagnostics()?;
        Ok(diagnostics.seed == self.seed
            && diagnostics.command_queue_depth == 0
            && diagnostics.update_queue_depth == 0
            && !diagnostics.awaiting_tick
            && diagnostics.pending_jobs == 0
            && diagnostics.pending_publications == 0
            && self.runtime.client().chunk_snapshot(self.center).is_some())
    }
}

#[test]
fn two_integrated_hosts_warm_independently_and_survive_active_selection_swaps() -> Result<()> {
    let mut hosts = [
        WarmIntegratedHost::start(FIRST_SEED, FIRST_CENTER)?,
        WarmIntegratedHost::start(SECOND_SEED, SECOND_CENTER)?,
    ];
    let deadline = Instant::now() + Duration::from_secs(120);

    loop {
        for host in &mut hosts {
            host.pump_ready_updates()?;
        }
        if hosts
            .iter()
            .map(WarmIntegratedHost::is_warm)
            .try_fold(true, |all_warm, warm| warm.map(|warm| all_warm && warm))?
        {
            break;
        }
        if Instant::now() >= deadline {
            let diagnostics = hosts
                .iter()
                .map(|host| host.connection.poll_diagnostics())
                .collect::<Result<Vec<_>, _>>()?;
            bail!("timed out warming two integrated hosts: {diagnostics:#?}");
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    assert!(
        hosts[0]
            .runtime
            .client()
            .chunk_snapshot(FIRST_CENTER)
            .is_some()
    );
    assert!(
        hosts[0]
            .runtime
            .client()
            .chunk_snapshot(SECOND_CENTER)
            .is_none(),
        "first replica must not receive the second host's chunk view"
    );
    assert!(
        hosts[1]
            .runtime
            .client()
            .chunk_snapshot(SECOND_CENTER)
            .is_some()
    );
    assert!(
        hosts[1]
            .runtime
            .client()
            .chunk_snapshot(FIRST_CENTER)
            .is_none(),
        "second replica must not receive the first host's chunk view"
    );

    let first_update_count = hosts[0].runtime.update_count();
    let second_update_count = hosts[1].runtime.update_count();
    let mut active_host = 0_usize;
    assert_eq!(hosts[active_host].seed, FIRST_SEED);
    active_host = 1;
    assert_eq!(hosts[active_host].seed, SECOND_SEED);
    active_host = 0;
    assert_eq!(hosts[active_host].seed, FIRST_SEED);
    assert_eq!(hosts[0].runtime.update_count(), first_update_count);
    assert_eq!(hosts[1].runtime.update_count(), second_update_count);

    Ok(())
}
