//! Shared startup/gameplay reconciliation of the engine camera pose and chunk
//! interest against a native scene runtime (docs/tactical/167 Slice 4).
//!
//! Desktop, Android, and XR previously carried three near-identical copies of
//! "sync the player pose to the server, accept any pending server position
//! correction, then follow chunk interest to the final camera position". That
//! server/interest follow is host-neutral, so it lives here. Physical view-pose
//! inputs stay platform-local: flat spectator placement, Android touch/startup
//! camera options, and the XR `XrStartupViewPose` + tracking-origin policy.

use anyhow::{Context, Result};
use mclone_render_session::EngineCameraController;

use crate::GameplayCommandUpdatePolicy;
use crate::host_mode::RemoteDedicatedServerSession;
use crate::native_session_runtime::NativeSceneRuntime;

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

/// Sync the camera pose to the server, accept pending server position
/// corrections, and follow chunk interest to the final camera position. Returns
/// whether anything changed (server pose, an accepted correction, or the
/// interest center).
pub fn commit_engine_camera_player_pose<S>(
    runtime: &mut NativeSceneRuntime<S>,
    camera: &mut EngineCameraController,
    context: EngineCameraCommitContext,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession,
{
    let server_changed = sync_engine_camera_player_pose(runtime, camera, context)?;
    let interest_changed = update_interest_from_engine_camera(runtime, camera, context)?;
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
    let changed = if let Some(report) = camera.next_pose_sync_command() {
        runtime
            .send_gameplay_command_with_update_policy(report.command, context.pose_sync_policy)
            .with_context(|| format!("failed to sync {} player pose to server", context.lane))?
    } else {
        false
    };
    Ok(changed || apply_pending_engine_camera_position_updates(runtime, camera, context)?)
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
    let snapshot = camera.snapshot();
    let center = snapshot.chunk_pos;
    if runtime.set_interest_center(center)? {
        log::info!(
            "{} chunk interest moved to ({}, {}) at camera position ({:.1}, {:.1}, {:.1})",
            context.lane,
            center.x,
            center.z,
            snapshot.eye.x,
            snapshot.eye.y,
            snapshot.eye.z
        );
        return Ok(true);
    }
    Ok(false)
}
