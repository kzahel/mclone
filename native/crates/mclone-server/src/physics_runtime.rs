//! Feature-gated server-owned physics runtime.
//!
//! This is intentionally narrow: one debug cube, one terrain section collider,
//! and diagnostics. Entity publication and renderer presentation are later
//! slices so this module can prove the authoritative simulation boundary first.

use std::fmt;

use mclone_core::{BlockPos, Vec3d};
use mclone_physics::{
    PhysicsBackendKind, PhysicsBodyId, PhysicsBodyPose, PhysicsBodySpawn, PhysicsColliderId,
    PhysicsStepReport, PhysicsWorld,
};

use crate::{ChunkScheduler, ServerPhysicsTickDiagnostics};

const SERVER_PHYSICS_DT_SECONDS: f64 = 1.0 / 20.0;
const DEBUG_CUBE_HALF_EXTENT: f64 = 0.5;

pub(crate) struct ServerPhysicsRuntime {
    world: PhysicsWorld,
    debug_cube_body: Option<PhysicsBodyId>,
    debug_cube_terrain: Option<PhysicsColliderId>,
    last_diagnostics: ServerPhysicsTickDiagnostics,
}

impl ServerPhysicsRuntime {
    pub(crate) fn new() -> Self {
        Self {
            world: PhysicsWorld::new(PhysicsBackendKind::Rapier),
            debug_cube_body: None,
            debug_cube_terrain: None,
            last_diagnostics: ServerPhysicsTickDiagnostics {
                enabled: true,
                ..ServerPhysicsTickDiagnostics::default()
            },
        }
    }

    pub(crate) fn spawn_debug_cube(
        &mut self,
        scheduler: &ChunkScheduler,
        position: Vec3d,
        velocity: Vec3d,
    ) -> bool {
        if !position.is_finite() || !velocity.is_finite() {
            return false;
        }
        let Some(terrain) =
            scheduler.physics_terrain_section_at_block(BlockPos::containing(position))
        else {
            return false;
        };

        self.clear_debug_cube();
        let terrain = self.world.add_terrain_section(terrain);
        let body = self.world.spawn_body(PhysicsBodySpawn::dynamic_cube(
            position,
            DEBUG_CUBE_HALF_EXTENT,
            velocity,
        ));
        self.debug_cube_terrain = Some(terrain);
        self.debug_cube_body = Some(body);
        let report = self.world.step(0.0);
        self.last_diagnostics = self.diagnostics_from_report(report);
        true
    }

    pub(crate) fn step(&mut self) -> ServerPhysicsTickDiagnostics {
        let report = self.world.step(SERVER_PHYSICS_DT_SECONDS);
        self.last_diagnostics = self.diagnostics_from_report(report);
        self.last_diagnostics
    }

    pub(crate) fn diagnostics(&self) -> ServerPhysicsTickDiagnostics {
        self.last_diagnostics
    }

    pub(crate) fn debug_cube_pose(&self) -> Option<PhysicsBodyPose> {
        self.debug_cube_body
            .and_then(|body| self.world.body_pose(body))
    }

    fn clear_debug_cube(&mut self) {
        if let Some(body) = self.debug_cube_body.take() {
            self.world.remove_body(body);
        }
        if let Some(terrain) = self.debug_cube_terrain.take() {
            self.world.remove_terrain_section(terrain);
        }
    }

    fn diagnostics_from_report(&self, report: PhysicsStepReport) -> ServerPhysicsTickDiagnostics {
        ServerPhysicsTickDiagnostics {
            enabled: true,
            body_count: report.body_count,
            collider_count: report.collider_count,
            active_body_count: report.active_body_count,
            terrain_collider_count: report.terrain_patch_count,
            body_pose_update_count: report.body_pose_update_count,
            test_cube_spawned: self.debug_cube_body.is_some(),
            test_cube_position: self.debug_cube_pose().map(|pose| pose.position),
        }
    }
}

impl Default for ServerPhysicsRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ServerPhysicsRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerPhysicsRuntime")
            .field("debug_cube_body", &self.debug_cube_body)
            .field("debug_cube_terrain", &self.debug_cube_terrain)
            .field("last_diagnostics", &self.last_diagnostics)
            .finish_non_exhaustive()
    }
}
