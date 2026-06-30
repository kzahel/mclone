//! Feature-gated server-owned physics runtime.
//!
//! This is intentionally narrow: one debug cube, a capped terrain-section island,
//! and diagnostics. Entity publication and renderer presentation are later
//! slices so this module can prove the authoritative simulation boundary first.

use std::fmt;

use mclone_core::{BlockPos, ChunkPos, Vec3d, block_to_section_coord};
use mclone_physics::{
    PhysicsBackendKind, PhysicsBodyId, PhysicsBodyKind, PhysicsBodyPose, PhysicsBodySpawn,
    PhysicsBodyVelocity, PhysicsColliderId, PhysicsRotation, PhysicsShape, PhysicsStepReport,
    PhysicsTerrainSection, PhysicsWorld,
};

use crate::{ChunkScheduler, ServerPhysicsTickDiagnostics};

const SERVER_GAMEPLAY_TICKS_PER_SECOND: f64 = 20.0;
const SERVER_GAMEPLAY_DT_SECONDS: f64 = 1.0 / SERVER_GAMEPLAY_TICKS_PER_SECOND;
const SERVER_PHYSICS_STEPS_PER_SECOND: f64 = 60.0;
const SERVER_PHYSICS_SUBSTEPS_PER_SERVER_TICK: usize = 3;
const SERVER_PHYSICS_SUBSTEP_DT_SECONDS: f64 = 1.0 / SERVER_PHYSICS_STEPS_PER_SECOND;
const SERVER_PHYSICS_MINECRAFT_GRAVITY_BLOCKS_PER_TICK: f64 = 0.08;
const SERVER_PHYSICS_GRAVITY: Vec3d = Vec3d::new(
    0.0,
    -SERVER_PHYSICS_MINECRAFT_GRAVITY_BLOCKS_PER_TICK
        * SERVER_GAMEPLAY_TICKS_PER_SECOND
        * SERVER_GAMEPLAY_TICKS_PER_SECOND,
    0.0,
);
const SERVER_PHYSICS_MINECRAFT_VERTICAL_DRAG: f64 = 0.98;
const DEBUG_CUBE_HALF_EXTENT: f64 = 0.5;
const DEBUG_CUBE_INITIAL_ANGULAR_VELOCITY: Vec3d = Vec3d::new(5.0, 3.0, -4.0);
const DEBUG_CUBE_TERRAIN_SECTION_RADIUS_XZ: i32 = 1;
const DEBUG_CUBE_TERRAIN_SECTION_RADIUS_Y: i32 = 1;
const DEBUG_PLAYER_WIDTH: f64 = 0.6;
const DEBUG_PLAYER_HEIGHT: f64 = 1.8;

pub(crate) struct ServerPhysicsRuntime {
    world: PhysicsWorld,
    debug_cube_body: Option<PhysicsBodyId>,
    debug_player_body: Option<PhysicsBodyId>,
    debug_cube_terrain: Vec<PhysicsColliderId>,
    last_diagnostics: ServerPhysicsTickDiagnostics,
}

impl ServerPhysicsRuntime {
    pub(crate) fn new() -> Self {
        let mut world = PhysicsWorld::new(PhysicsBackendKind::Rapier);
        let _ = world.set_gravity(SERVER_PHYSICS_GRAVITY);
        Self {
            world,
            debug_cube_body: None,
            debug_player_body: None,
            debug_cube_terrain: Vec::new(),
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
        player_position: Option<Vec3d>,
    ) -> bool {
        if !position.is_finite() || !velocity.is_finite() {
            return false;
        }

        let terrain_sections = debug_cube_terrain_sections(scheduler, position);
        if terrain_sections.is_empty() {
            return false;
        }

        self.clear_debug_cube();
        if let Some(player_position) = player_position {
            self.sync_player_collider(player_position);
        } else {
            self.clear_player_collider();
        }
        self.debug_cube_terrain = terrain_sections
            .into_iter()
            .map(|section| self.world.add_terrain_section(section))
            .collect();
        let body = self.world.spawn_body(PhysicsBodySpawn {
            kind: PhysicsBodyKind::Dynamic,
            shape: PhysicsShape::Cuboid {
                half_extents: Vec3d::new(
                    DEBUG_CUBE_HALF_EXTENT,
                    DEBUG_CUBE_HALF_EXTENT,
                    DEBUG_CUBE_HALF_EXTENT,
                ),
            },
            pose: PhysicsBodyPose::new(position, PhysicsRotation::IDENTITY),
            velocity: PhysicsBodyVelocity {
                linear: velocity,
                angular: DEBUG_CUBE_INITIAL_ANGULAR_VELOCITY,
            },
        });
        self.debug_cube_body = Some(body);
        let report = self.world.step(0.0);
        self.last_diagnostics = self.diagnostics_from_report(report);
        true
    }

    pub(crate) fn sync_player_collider(&mut self, player_position: Vec3d) -> bool {
        if !player_position.is_finite() {
            return false;
        }
        let pose = PhysicsBodyPose::new(
            player_position.add(Vec3d::new(0.0, DEBUG_PLAYER_HEIGHT * 0.5, 0.0)),
            PhysicsRotation::IDENTITY,
        );
        if let Some(body) = self.debug_player_body {
            return self.world.set_body_pose(body, pose);
        }
        let body = self.world.spawn_body(PhysicsBodySpawn {
            kind: PhysicsBodyKind::Fixed,
            shape: PhysicsShape::Cuboid {
                half_extents: Vec3d::new(
                    DEBUG_PLAYER_WIDTH * 0.5,
                    DEBUG_PLAYER_HEIGHT * 0.5,
                    DEBUG_PLAYER_WIDTH * 0.5,
                ),
            },
            pose,
            velocity: PhysicsBodyVelocity::default(),
        });
        self.debug_player_body = Some(body);
        true
    }

    pub(crate) fn step(&mut self) -> ServerPhysicsTickDiagnostics {
        let mut report = PhysicsStepReport::default();
        let mut body_pose_update_count = 0;
        for _ in 0..SERVER_PHYSICS_SUBSTEPS_PER_SERVER_TICK {
            report = self.world.step(SERVER_PHYSICS_SUBSTEP_DT_SECONDS);
            body_pose_update_count += report.body_pose_update_count;
        }
        report.body_pose_update_count = body_pose_update_count;
        self.apply_debug_cube_vertical_drag(SERVER_GAMEPLAY_DT_SECONDS);
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

    #[cfg(test)]
    pub(crate) fn debug_cube_velocity(&self) -> Option<PhysicsBodyVelocity> {
        self.debug_cube_body
            .and_then(|body| self.world.body_velocity(body))
    }

    fn clear_debug_cube(&mut self) {
        if let Some(body) = self.debug_cube_body.take() {
            self.world.remove_body(body);
        }
        for terrain in self.debug_cube_terrain.drain(..) {
            self.world.remove_terrain_section(terrain);
        }
    }

    fn clear_player_collider(&mut self) {
        if let Some(body) = self.debug_player_body.take() {
            self.world.remove_body(body);
        }
    }

    fn apply_debug_cube_vertical_drag(&mut self, dt_seconds: f64) {
        let Some(body) = self.debug_cube_body else {
            return;
        };
        let Some(mut velocity) = self.world.body_velocity(body) else {
            return;
        };
        let tick_scale = dt_seconds * SERVER_GAMEPLAY_TICKS_PER_SECOND;
        if tick_scale <= 0.0 {
            return;
        }
        velocity.linear.y *= SERVER_PHYSICS_MINECRAFT_VERTICAL_DRAG.powf(tick_scale);
        self.world.set_body_velocity(body, velocity);
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

fn debug_cube_terrain_sections(
    scheduler: &ChunkScheduler,
    position: Vec3d,
) -> Vec<PhysicsTerrainSection> {
    let block_pos = BlockPos::containing(position);
    let center_chunk = block_pos.chunk_pos();
    let center_section_y = block_to_section_coord(block_pos.y);
    let mut sections = Vec::new();

    for section_y in (center_section_y - DEBUG_CUBE_TERRAIN_SECTION_RADIUS_Y)
        ..=(center_section_y + DEBUG_CUBE_TERRAIN_SECTION_RADIUS_Y)
    {
        for chunk_z in chunk_range(center_chunk.z, DEBUG_CUBE_TERRAIN_SECTION_RADIUS_XZ) {
            for chunk_x in chunk_range(center_chunk.x, DEBUG_CUBE_TERRAIN_SECTION_RADIUS_XZ) {
                let Some(section) =
                    scheduler.physics_terrain_section(ChunkPos::new(chunk_x, chunk_z), section_y)
                else {
                    continue;
                };
                if section.solid_cell_count() == 0 {
                    continue;
                }
                sections.push(section);
            }
        }
    }
    sections
}

fn chunk_range(center: i32, radius: i32) -> std::ops::RangeInclusive<i32> {
    (center - radius)..=(center + radius)
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
            .field("debug_player_body", &self.debug_player_body)
            .field("debug_cube_terrain", &self.debug_cube_terrain)
            .field("last_diagnostics", &self.last_diagnostics)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physics_substeps_cover_one_gameplay_tick() {
        assert_eq!(SERVER_PHYSICS_SUBSTEPS_PER_SERVER_TICK, 3);
        let substep_seconds =
            SERVER_PHYSICS_SUBSTEP_DT_SECONDS * SERVER_PHYSICS_SUBSTEPS_PER_SERVER_TICK as f64;
        assert!((substep_seconds - SERVER_GAMEPLAY_DT_SECONDS).abs() < f64::EPSILON);
    }
}
