#![cfg(not(target_arch = "wasm32"))]

use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use mclone_app_runtime::client_connection::{
    ClientConnection, ConnectionUpdateDrainMode, IntegratedRunnerConnection,
    pump_client_connection_updates_report,
};
use mclone_app_runtime::{RuntimeUpdatePumpBudget, SingleViewRuntime};
use mclone_core::{AIR_BLOCK_STATE_ID, BlockPos, ChunkPos, Direction, Vec3d, chunk_section_index};
use mclone_protocol::{
    AcceptTeleportCommand, ChunkView, ClientCommand, MovePlayerCommand, PlayerActionCommand,
    PlayerActionKind,
};
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
        Self::start_with_config(seed, center, NativeIntegratedServerRunnerConfig::new(seed))
    }

    fn start_persistent(
        seed: i64,
        center: ChunkPos,
        world_dir: impl Into<std::path::PathBuf>,
    ) -> Result<Self> {
        Self::start_with_config(
            seed,
            center,
            NativeIntegratedServerRunnerConfig::new(seed).with_persistent_world_dir(world_dir),
        )
    }

    fn start_with_config(
        seed: i64,
        center: ChunkPos,
        config: NativeIntegratedServerRunnerConfig,
    ) -> Result<Self> {
        let runner = NativeIntegratedServerRunner::new(
            config
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

    fn acknowledge_initial_position(&mut self) -> Result<()> {
        for update in self.runtime.drain_player_position_updates() {
            self.connection
                .send_command_only(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                    id: update.teleport_id,
                }))?;
        }
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

fn nth_non_air_block(host: &WarmIntegratedHost, mut wanted: usize) -> Option<BlockPos> {
    let snapshot = host.runtime.client().chunk_snapshot(host.center)?;
    for section in snapshot.sections.iter().rev() {
        let blocks = section.unpack_block_state_ids();
        for local_y in (0..16).rev() {
            for local_z in 0..16 {
                for local_x in 0..16 {
                    if blocks[chunk_section_index(local_x, local_y, local_z)] != AIR_BLOCK_STATE_ID
                    {
                        if wanted > 0 {
                            wanted -= 1;
                            continue;
                        }
                        return Some(BlockPos::new(
                            snapshot.pos.min_block_x() + local_x,
                            section.section_y * 16 + local_y,
                            snapshot.pos.min_block_z() + local_z,
                        ));
                    }
                }
            }
        }
    }
    None
}

fn first_non_air_block(host: &WarmIntegratedHost) -> Option<BlockPos> {
    nth_non_air_block(host, 0)
}

fn break_block(host: &mut WarmIntegratedHost, target: BlockPos) -> Result<()> {
    host.connection
        .send_command_only(ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
            position: Vec3d::new(
                f64::from(target.x) + 0.5,
                f64::from(target.y),
                f64::from(target.z) + 0.5,
            ),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        }))?;
    host.connection
        .send_command_only(ClientCommand::PlayerAction(PlayerActionCommand {
            pos: target,
            direction: Direction::Up,
            kind: PlayerActionKind::DebugInstantBreak,
        }))?;
    Ok(())
}

fn unique_temp_dir(label: &str) -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "mclone-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ))
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

#[test]
fn two_live_integrated_hosts_keep_persistent_edits_in_distinct_roots() -> Result<()> {
    let root = unique_temp_dir("dual-integrated-persistence");
    let first_root = root.join("first");
    let second_root = root.join("second");
    let seed = 67_890;
    let center = ChunkPos::new(0, 0);
    let mut hosts = [
        WarmIntegratedHost::start_persistent(seed, center, first_root.clone())?,
        WarmIntegratedHost::start_persistent(seed, center, second_root.clone())?,
    ];
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        for host in &mut hosts {
            host.pump_ready_updates()?;
            host.acknowledge_initial_position()?;
        }
        if hosts
            .iter()
            .map(WarmIntegratedHost::is_warm)
            .try_fold(true, |all_warm, warm| warm.map(|warm| all_warm && warm))?
        {
            break;
        }
        if Instant::now() >= deadline {
            bail!("timed out warming persistent dual hosts");
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    let first_target =
        first_non_air_block(&hosts[0]).context("generated chunk contains a block")?;
    let second_target =
        nth_non_air_block(&hosts[1], 1).context("generated chunk contains a second block")?;
    let first_untouched_state = hosts[1]
        .runtime
        .client()
        .block_state_at_block_pos(first_target)
        .context("second host has the matching loaded block")?;
    let second_untouched_state = hosts[0]
        .runtime
        .client()
        .block_state_at_block_pos(second_target)
        .context("first host has the second matching loaded block")?;
    assert_ne!(first_untouched_state, AIR_BLOCK_STATE_ID);
    assert_ne!(second_untouched_state, AIR_BLOCK_STATE_ID);
    let edit_deadline = Instant::now() + Duration::from_secs(120);
    break_block(&mut hosts[0], first_target)?;
    loop {
        for host in &mut hosts {
            host.pump_ready_updates()?;
        }
        if hosts[0]
            .runtime
            .client()
            .block_state_at_block_pos(first_target)
            == Some(AIR_BLOCK_STATE_ID)
        {
            break;
        }
        if Instant::now() >= edit_deadline {
            bail!("timed out applying the first host's persistent edit");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        hosts[1]
            .runtime
            .client()
            .block_state_at_block_pos(first_target),
        Some(first_untouched_state),
        "the live second host must not observe the first root's edit"
    );
    break_block(&mut hosts[1], second_target)?;
    loop {
        for host in &mut hosts {
            host.pump_ready_updates()?;
        }
        if hosts[1]
            .runtime
            .client()
            .block_state_at_block_pos(second_target)
            == Some(AIR_BLOCK_STATE_ID)
        {
            break;
        }
        if Instant::now() >= edit_deadline {
            bail!("timed out applying the second host's persistent edit");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        hosts[0]
            .runtime
            .client()
            .block_state_at_block_pos(second_target),
        Some(second_untouched_state),
        "the live first host must not observe the second root's edit"
    );
    for host in &mut hosts {
        assert!(host.connection.flush_persistence()? >= 1);
    }
    drop(hosts);

    let mut reopened = [
        WarmIntegratedHost::start_persistent(seed, center, first_root)?,
        WarmIntegratedHost::start_persistent(seed, center, second_root)?,
    ];
    let reopen_deadline = Instant::now() + Duration::from_secs(120);
    loop {
        for host in &mut reopened {
            host.pump_ready_updates()?;
            host.acknowledge_initial_position()?;
        }
        if reopened
            .iter()
            .map(WarmIntegratedHost::is_warm)
            .try_fold(true, |all_warm, warm| warm.map(|warm| all_warm && warm))?
        {
            break;
        }
        if Instant::now() >= reopen_deadline {
            bail!("timed out reopening persistent dual hosts");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        reopened[0]
            .runtime
            .client()
            .block_state_at_block_pos(first_target),
        Some(AIR_BLOCK_STATE_ID),
    );
    assert_eq!(
        reopened[1]
            .runtime
            .client()
            .block_state_at_block_pos(first_target),
        Some(first_untouched_state),
    );
    assert_eq!(
        reopened[0]
            .runtime
            .client()
            .block_state_at_block_pos(second_target),
        Some(second_untouched_state),
    );
    assert_eq!(
        reopened[1]
            .runtime
            .client()
            .block_state_at_block_pos(second_target),
        Some(AIR_BLOCK_STATE_ID),
    );
    drop(reopened);
    let _ = std::fs::remove_dir_all(root);
    Ok(())
}
