//! Shared startup/gameplay reconciliation of the engine camera pose and chunk
//! interest against neutral runtime/player facts (docs/tactical/167 Slice 4,
//! docs/tactical/170 Slice 1).
//!
//! Desktop, Android, and XR previously carried three near-identical copies of
//! "sync the player pose to the server, accept any pending server position
//! correction, then follow chunk interest to the final camera position". That
//! server/interest follow is host-neutral, so it lives here. Physical view-pose
//! inputs stay platform-local: flat spectator placement, Android touch/startup
//! camera options, and the XR `XrStartupViewPose` + tracking-origin policy.

use anyhow::{Context, Result};
use mclone_core::{ChunkPos, HorizontalTopology};
use mclone_protocol::{ClientCommand, PlayerPositionUpdate};
use mclone_render_session::EngineCameraController;

use crate::monotonic::MonotonicClockHandle;
use crate::{
    GameplayCommandSubmission, GameplayCommandTiming, GameplayCommandUpdatePolicy, elapsed_ms,
};

/// The target-neutral runtime facts/actions needed to reconcile the engine
/// camera. Native and browser session owners implement this narrow contract;
/// the reconcile policy itself has no filesystem, thread, or platform API
/// dependency.
pub trait EngineCameraRuntime {
    fn camera_topology(&self) -> HorizontalTopology {
        HorizontalTopology::UNBOUNDED
    }

    fn send_camera_command(&mut self, command: ClientCommand) -> Result<()>;

    fn send_camera_command_with_policy_timed(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission>;

    fn drain_camera_position_updates(&mut self) -> Vec<PlayerPositionUpdate>;

    fn set_camera_interest_center_timed(
        &mut self,
        center: ChunkPos,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<(bool, GameplayCommandTiming)>;
}

#[cfg(not(target_arch = "wasm32"))]
impl<S> EngineCameraRuntime for crate::native_service_assembly::NativeSceneServices<S>
where
    S: crate::host_mode::RemoteDedicatedServerSession,
{
    fn camera_topology(&self) -> HorizontalTopology {
        self.client().topology()
    }

    fn send_camera_command(&mut self, command: ClientCommand) -> Result<()> {
        self.send_gameplay_command(command)
    }

    fn send_camera_command_with_policy_timed(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission> {
        self.send_gameplay_command_with_update_policy_timed(command, policy)
    }

    fn drain_camera_position_updates(&mut self) -> Vec<PlayerPositionUpdate> {
        self.drain_player_position_updates()
    }

    fn set_camera_interest_center_timed(
        &mut self,
        center: ChunkPos,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<(bool, GameplayCommandTiming)> {
        self.set_interest_center_with_update_policy_timed(center, policy)
    }
}

impl EngineCameraRuntime for crate::scene_session_runtime::SceneSessionRuntime {
    fn camera_topology(&self) -> HorizontalTopology {
        self.client().topology()
    }

    fn send_camera_command(&mut self, command: ClientCommand) -> Result<()> {
        self.send_gameplay_command(command)
    }

    fn send_camera_command_with_policy_timed(
        &mut self,
        command: ClientCommand,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<GameplayCommandSubmission> {
        self.send_gameplay_command_with_update_policy_timed(command, policy)
    }

    fn drain_camera_position_updates(&mut self) -> Vec<PlayerPositionUpdate> {
        self.drain_player_position_updates()
    }

    fn set_camera_interest_center_timed(
        &mut self,
        center: ChunkPos,
        policy: GameplayCommandUpdatePolicy,
    ) -> Result<(bool, GameplayCommandTiming)> {
        self.set_interest_center_with_update_policy_timed(center, policy)
    }
}

/// Platform-supplied labels plus the one genuinely divergent policy for the
/// shared camera/interest reconciliation. Everything else about the reconcile is
/// identical across desktop, Android, and XR.
#[derive(Clone, Copy, Debug)]
pub struct EngineCameraCommitContext {
    /// Short lane label used in interest/correction log lines and error context
    /// (for example `"desktop"`, `"Android"`, `"XR terrain"`).
    pub lane: &'static str,
    /// Update policy for the movement pose-sync command. Desktop and XR send the
    /// command only (`SendOnly`); Android drains server updates immediately. This
    /// is an explicit caller policy, not a hidden platform fork
    /// (docs/tactical/167).
    pub pose_sync_policy: GameplayCommandUpdatePolicy,
}

impl EngineCameraCommitContext {
    pub const fn new(lane: &'static str, pose_sync_policy: GameplayCommandUpdatePolicy) -> Self {
        Self {
            lane,
            pose_sync_policy,
        }
    }

    /// Desktop/XR default: send the movement pose-sync command without draining
    /// server updates inline.
    pub const fn send_only(lane: &'static str) -> Self {
        Self::new(lane, GameplayCommandUpdatePolicy::SendOnly)
    }

    /// Android default: drain server updates immediately after the pose-sync
    /// command.
    pub const fn drain_immediately(lane: &'static str) -> Self {
        Self::new(lane, GameplayCommandUpdatePolicy::DrainImmediately)
    }
}

/// Optional host-neutral attribution for one camera pose/interest commit.
///
/// XR folds this into its locomotion/frame diagnostics. Flat hosts normally
/// pass `None`, keeping the reconcile policy and instrumentation on one path
/// without requiring every caller to retain timing state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EngineCameraCommitTiming {
    pub server_command_ms: f64,
    pub server_command: GameplayCommandTiming,
    pub position_updates_ms: f64,
    pub interest_ms: f64,
    pub interest_command: GameplayCommandTiming,
}

/// Sync the camera pose to the server, accept pending server position
/// corrections, and follow chunk interest to the final camera position. Returns
/// whether anything changed (server pose, an accepted correction, or the
/// interest center).
pub fn commit_engine_camera_player_pose<R>(
    runtime: &mut R,
    camera: &mut EngineCameraController,
    context: EngineCameraCommitContext,
    clock: &MonotonicClockHandle,
    mut timing: Option<&mut EngineCameraCommitTiming>,
) -> Result<bool>
where
    R: EngineCameraRuntime + ?Sized,
{
    if let Some(timing) = timing.as_deref_mut() {
        *timing = EngineCameraCommitTiming::default();
    }
    let server_changed = sync_engine_camera_player_pose_with_timing(
        runtime,
        camera,
        context,
        Some(clock),
        timing.as_deref_mut(),
    )?;
    let interest_changed = update_interest_from_engine_camera_with_timing(
        runtime,
        camera,
        context,
        Some(clock),
        timing,
    )?;
    Ok(server_changed || interest_changed)
}

/// Send the pending movement pose-sync command (if any) under the caller's
/// policy, then apply any queued server position corrections.
pub fn sync_engine_camera_player_pose<R>(
    runtime: &mut R,
    camera: &mut EngineCameraController,
    context: EngineCameraCommitContext,
) -> Result<bool>
where
    R: EngineCameraRuntime + ?Sized,
{
    sync_engine_camera_player_pose_with_timing(runtime, camera, context, None, None)
}

fn sync_engine_camera_player_pose_with_timing<R>(
    runtime: &mut R,
    camera: &mut EngineCameraController,
    context: EngineCameraCommitContext,
    clock: Option<&MonotonicClockHandle>,
    mut timing: Option<&mut EngineCameraCommitTiming>,
) -> Result<bool>
where
    R: EngineCameraRuntime + ?Sized,
{
    let command_start = timing
        .is_some()
        .then(|| clock.expect("timed reconcile needs clock").now());
    let topology = runtime.camera_topology();
    let submitted = if let Some(report) = camera.next_pose_sync_command_in(topology) {
        let submission = runtime
            .send_camera_command_with_policy_timed(report.command, context.pose_sync_policy)
            .with_context(|| format!("failed to sync {} player pose to server", context.lane))?;
        if let Some(timing) = timing.as_deref_mut() {
            timing.server_command = submission.timing;
        }
        true
    } else {
        false
    };
    if let (Some(start), Some(timing)) = (command_start, timing.as_deref_mut()) {
        timing.server_command_ms = elapsed_ms(
            clock
                .expect("timed reconcile needs clock")
                .elapsed_since(start),
        );
    }
    let position_updates_start = timing
        .is_some()
        .then(|| clock.expect("timed reconcile needs clock").now());
    let position_updates_changed =
        apply_pending_engine_camera_position_updates(runtime, camera, context)?;
    if let (Some(start), Some(timing)) = (position_updates_start, timing) {
        timing.position_updates_ms = elapsed_ms(
            clock
                .expect("timed reconcile needs clock")
                .elapsed_since(start),
        );
    }
    Ok(submitted || position_updates_changed)
}

/// Accept every queued server player-position correction, acknowledge it,
/// resync the corrected pose, and follow chunk interest if the correction moved
/// the camera across a chunk boundary.
pub fn apply_pending_engine_camera_position_updates<R>(
    runtime: &mut R,
    camera: &mut EngineCameraController,
    context: EngineCameraCommitContext,
) -> Result<bool>
where
    R: EngineCameraRuntime + ?Sized,
{
    let mut changed = false;
    let topology = runtime.camera_topology();
    for update in runtime.drain_camera_position_updates() {
        let accepted = camera.accept_position_update_in(topology, update);
        runtime
            .send_camera_command(accepted.accept_command)
            .with_context(|| {
                format!(
                    "failed to acknowledge {} player position correction",
                    context.lane
                )
            })?;
        let resync = camera
            .corrected_pose_sync_command_in(topology)
            .context("corrected player pose left the active topology")?;
        runtime
            .send_camera_command(resync.command)
            .with_context(|| format!("failed to sync corrected {} player pose", context.lane))?;
        log::warn!(
            "accepted {} server player position correction id={} feet=({:.2}, {:.2}, {:.2})",
            context.lane,
            accepted.update.teleport_id,
            accepted.feet_position.x,
            accepted.feet_position.y,
            accepted.feet_position.z
        );
        changed = true;
    }
    if changed {
        changed |= update_interest_from_engine_camera(runtime, camera, context)?;
    }
    Ok(changed)
}

/// Move the runtime's chunk interest center to the camera's current chunk,
/// returning whether the center changed. Physical camera placement is owned by
/// the caller; this only follows it.
pub fn update_interest_from_engine_camera<R>(
    runtime: &mut R,
    camera: &EngineCameraController,
    context: EngineCameraCommitContext,
) -> Result<bool>
where
    R: EngineCameraRuntime + ?Sized,
{
    update_interest_from_engine_camera_with_timing(runtime, camera, context, None, None)
}

fn update_interest_from_engine_camera_with_timing<R>(
    runtime: &mut R,
    camera: &EngineCameraController,
    context: EngineCameraCommitContext,
    clock: Option<&MonotonicClockHandle>,
    timing: Option<&mut EngineCameraCommitTiming>,
) -> Result<bool>
where
    R: EngineCameraRuntime + ?Sized,
{
    let interest_start = timing
        .is_some()
        .then(|| clock.expect("timed reconcile needs clock").now());
    let snapshot = camera.snapshot();
    let center = runtime
        .camera_topology()
        .canonicalize_chunk(snapshot.chunk_pos)
        .context("camera interest center left the active topology")?;
    let (changed, command_timing) =
        runtime.set_camera_interest_center_timed(center, GameplayCommandUpdatePolicy::SendOnly)?;
    if let (Some(start), Some(timing)) = (interest_start, timing) {
        timing.interest_ms = elapsed_ms(
            clock
                .expect("timed reconcile needs clock")
                .elapsed_since(start),
        );
        timing.interest_command = command_timing;
    }
    if changed {
        log::info!(
            "{} chunk interest moved to ({}, {}) at camera position ({:.1}, {:.1}, {:.1})",
            context.lane,
            center.x,
            center.z,
            snapshot.eye.x,
            snapshot.eye.y,
            snapshot.eye.z
        );
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monotonic::{MonotonicClock, MonotonicInstant};

    #[derive(Default)]
    struct CameraFacts {
        commands: Vec<ClientCommand>,
        interest: Option<ChunkPos>,
    }

    impl EngineCameraRuntime for CameraFacts {
        fn send_camera_command(&mut self, command: ClientCommand) -> Result<()> {
            self.commands.push(command);
            Ok(())
        }

        fn send_camera_command_with_policy_timed(
            &mut self,
            command: ClientCommand,
            _policy: GameplayCommandUpdatePolicy,
        ) -> Result<GameplayCommandSubmission> {
            self.commands.push(command);
            Ok(GameplayCommandSubmission::default())
        }

        fn drain_camera_position_updates(&mut self) -> Vec<PlayerPositionUpdate> {
            Vec::new()
        }

        fn set_camera_interest_center_timed(
            &mut self,
            center: ChunkPos,
            _policy: GameplayCommandUpdatePolicy,
        ) -> Result<(bool, GameplayCommandTiming)> {
            let changed = self.interest.replace(center) != Some(center);
            Ok((changed, GameplayCommandTiming::default()))
        }
    }

    struct FixedClock;

    impl MonotonicClock for FixedClock {
        fn now(&self) -> MonotonicInstant {
            MonotonicInstant::from_nanos(10)
        }
    }

    #[test]
    fn camera_reconcile_uses_only_neutral_runtime_facts() {
        let mut facts = CameraFacts::default();
        let mut camera = EngineCameraController::spawn_for_chunk(ChunkPos::new(2, -3));
        let clock = MonotonicClockHandle::new(FixedClock);
        let mut timing = EngineCameraCommitTiming::default();

        let changed = commit_engine_camera_player_pose(
            &mut facts,
            &mut camera,
            EngineCameraCommitContext::send_only("test"),
            &clock,
            Some(&mut timing),
        )
        .unwrap();

        assert!(changed);
        assert!(!facts.commands.is_empty());
        assert_eq!(facts.interest, Some(ChunkPos::new(2, -3)));
        assert_eq!(timing.position_updates_ms, 0.0);
    }
}
