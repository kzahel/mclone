//! Shared startup/gameplay reconciliation of the engine camera pose and chunk
//! interest against a native scene runtime (docs/tactical/167 Slice 4).
//!
//! Desktop, Android, and XR previously carried three near-identical copies of
//! "sync the player pose to the server, accept any pending server position
//! correction, then follow chunk interest to the final camera position". That
//! server/interest follow is host-neutral, so it lives here. Physical view-pose
//! inputs stay platform-local: flat spectator placement, Android touch/startup
//! camera options, and the XR `XrStartupViewPose` + tracking-origin policy.

use std::time::Instant;

use anyhow::{Context, Result};
use mclone_render_session::EngineCameraController;

use crate::host_mode::RemoteDedicatedServerSession;
use crate::native_session_runtime::NativeSceneRuntime;
use crate::{GameplayCommandTiming, GameplayCommandUpdatePolicy, elapsed_ms};

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
pub fn commit_engine_camera_player_pose<S>(
    runtime: &mut NativeSceneRuntime<S>,
    camera: &mut EngineCameraController,
    context: EngineCameraCommitContext,
    mut timing: Option<&mut EngineCameraCommitTiming>,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession,
{
    if let Some(timing) = timing.as_deref_mut() {
        *timing = EngineCameraCommitTiming::default();
    }
    let server_changed = sync_engine_camera_player_pose_with_timing(
        runtime,
        camera,
        context,
        timing.as_deref_mut(),
    )?;
    let interest_changed =
        update_interest_from_engine_camera_with_timing(runtime, camera, context, timing)?;
    Ok(server_changed || interest_changed)
}

/// Send the pending movement pose-sync command (if any) under the caller's
/// policy, then apply any queued server position corrections.
pub fn sync_engine_camera_player_pose<S>(
    runtime: &mut NativeSceneRuntime<S>,
    camera: &mut EngineCameraController,
    context: EngineCameraCommitContext,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession,
{
    sync_engine_camera_player_pose_with_timing(runtime, camera, context, None)
}

fn sync_engine_camera_player_pose_with_timing<S>(
    runtime: &mut NativeSceneRuntime<S>,
    camera: &mut EngineCameraController,
    context: EngineCameraCommitContext,
    mut timing: Option<&mut EngineCameraCommitTiming>,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession,
{
    let command_start = timing.is_some().then(Instant::now);
    let changed = if let Some(report) = camera.next_pose_sync_command() {
        let (changed, command_timing) = runtime
            .send_gameplay_command_with_update_policy_timed(
                report.command,
                context.pose_sync_policy,
            )
            .with_context(|| format!("failed to sync {} player pose to server", context.lane))?;
        if let Some(timing) = timing.as_deref_mut() {
            timing.server_command = command_timing;
        }
        changed
    } else {
        false
    };
    if let (Some(start), Some(timing)) = (command_start, timing.as_deref_mut()) {
        timing.server_command_ms = elapsed_ms(start.elapsed());
    }
    let position_updates_start = timing.is_some().then(Instant::now);
    let position_updates_changed =
        apply_pending_engine_camera_position_updates(runtime, camera, context)?;
    if let (Some(start), Some(timing)) = (position_updates_start, timing) {
        timing.position_updates_ms = elapsed_ms(start.elapsed());
    }
    Ok(changed || position_updates_changed)
}

/// Accept every queued server player-position correction, acknowledge it,
/// resync the corrected pose, and follow chunk interest if the correction moved
/// the camera across a chunk boundary.
pub fn apply_pending_engine_camera_position_updates<S>(
    runtime: &mut NativeSceneRuntime<S>,
    camera: &mut EngineCameraController,
    context: EngineCameraCommitContext,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession,
{
    let mut changed = false;
    for update in runtime.drain_player_position_updates() {
        let accepted = camera.accept_position_update(update);
        runtime
            .send_gameplay_command(accepted.accept_command)
            .with_context(|| {
                format!(
                    "failed to acknowledge {} player position correction",
                    context.lane
                )
            })?;
        let resync = camera.corrected_pose_sync_command();
        runtime
            .send_gameplay_command(resync.command)
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
pub fn update_interest_from_engine_camera<S>(
    runtime: &mut NativeSceneRuntime<S>,
    camera: &EngineCameraController,
    context: EngineCameraCommitContext,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession,
{
    update_interest_from_engine_camera_with_timing(runtime, camera, context, None)
}

fn update_interest_from_engine_camera_with_timing<S>(
    runtime: &mut NativeSceneRuntime<S>,
    camera: &EngineCameraController,
    context: EngineCameraCommitContext,
    timing: Option<&mut EngineCameraCommitTiming>,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession,
{
    let interest_start = timing.is_some().then(Instant::now);
    let snapshot = camera.snapshot();
    let center = snapshot.chunk_pos;
    let (changed, command_timing) = runtime.set_interest_center_with_update_policy_timed(
        center,
        GameplayCommandUpdatePolicy::SendOnly,
    )?;
    if let (Some(start), Some(timing)) = (interest_start, timing) {
        timing.interest_ms = elapsed_ms(start.elapsed());
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
