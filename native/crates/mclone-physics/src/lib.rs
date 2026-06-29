#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use mclone_core::{Aabb, Vec3d};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicsBodyId(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicsColliderId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PhysicsBackendKind {
    Noop,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PhysicsBodyKind {
    Dynamic,
    Kinematic,
    Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhysicsShape {
    Cuboid { half_extents: Vec3d },
    Ball { radius: f64 },
    CapsuleY { half_height: f64, radius: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsRotation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl PhysicsRotation {
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };
}

impl Default for PhysicsRotation {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicsBodyPose {
    pub position: Vec3d,
    pub rotation: PhysicsRotation,
}

impl PhysicsBodyPose {
    pub const fn new(position: Vec3d, rotation: PhysicsRotation) -> Self {
        Self { position, rotation }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicsBodyVelocity {
    pub linear: Vec3d,
    pub angular: Vec3d,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsBodySpawn {
    pub kind: PhysicsBodyKind,
    pub shape: PhysicsShape,
    pub pose: PhysicsBodyPose,
    pub velocity: PhysicsBodyVelocity,
}

impl PhysicsBodySpawn {
    pub const fn dynamic_cube(position: Vec3d, half_extent: f64, velocity: Vec3d) -> Self {
        Self {
            kind: PhysicsBodyKind::Dynamic,
            shape: PhysicsShape::Cuboid {
                half_extents: Vec3d::new(half_extent, half_extent, half_extent),
            },
            pose: PhysicsBodyPose::new(position, PhysicsRotation::IDENTITY),
            velocity: PhysicsBodyVelocity {
                linear: velocity,
                angular: Vec3d::ZERO,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsTerrainPatch {
    pub bounds: Aabb,
    pub revision: u64,
    pub solid_cell_count: usize,
}

impl PhysicsTerrainPatch {
    pub const fn new(bounds: Aabb, revision: u64, solid_cell_count: usize) -> Self {
        Self {
            bounds,
            revision,
            solid_cell_count,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicsStepReport {
    pub backend: PhysicsBackendKind,
    pub dt_seconds: f64,
    pub body_count: usize,
    pub collider_count: usize,
    pub active_body_count: usize,
    pub terrain_patch_count: usize,
    pub body_pose_update_count: usize,
}

pub struct PhysicsWorld {
    backend: NoopPhysicsWorld,
}

impl PhysicsWorld {
    pub fn new(kind: PhysicsBackendKind) -> Self {
        match kind {
            PhysicsBackendKind::Noop => Self::noop(),
        }
    }

    pub fn noop() -> Self {
        Self {
            backend: NoopPhysicsWorld::new(),
        }
    }

    pub const fn backend_kind(&self) -> PhysicsBackendKind {
        self.backend.backend_kind()
    }

    pub fn spawn_body(&mut self, spawn: PhysicsBodySpawn) -> PhysicsBodyId {
        self.backend.spawn_body(spawn)
    }

    pub fn remove_body(&mut self, id: PhysicsBodyId) -> Option<PhysicsBodySpawn> {
        self.backend.remove_body(id)
    }

    pub fn body(&self, id: PhysicsBodyId) -> Option<&PhysicsBodySpawn> {
        self.backend.body(id)
    }

    pub fn body_pose(&self, id: PhysicsBodyId) -> Option<PhysicsBodyPose> {
        self.backend.body(id).map(|body| body.pose)
    }

    pub fn set_body_pose(&mut self, id: PhysicsBodyId, pose: PhysicsBodyPose) -> bool {
        self.backend.set_body_pose(id, pose)
    }

    pub fn add_terrain_patch(&mut self, patch: PhysicsTerrainPatch) -> PhysicsColliderId {
        self.backend.add_terrain_patch(patch)
    }

    pub fn update_terrain_patch(
        &mut self,
        id: PhysicsColliderId,
        patch: PhysicsTerrainPatch,
    ) -> bool {
        self.backend.update_terrain_patch(id, patch)
    }

    pub fn remove_terrain_patch(&mut self, id: PhysicsColliderId) -> Option<PhysicsTerrainPatch> {
        self.backend.remove_terrain_patch(id)
    }

    pub fn terrain_patch(&self, id: PhysicsColliderId) -> Option<&PhysicsTerrainPatch> {
        self.backend.terrain_patch(id)
    }

    pub fn step(&mut self, dt_seconds: f64) -> PhysicsStepReport {
        self.backend.step(dt_seconds)
    }
}

#[derive(Default)]
struct NoopPhysicsWorld {
    next_body_id: u64,
    next_collider_id: u64,
    bodies: BTreeMap<PhysicsBodyId, PhysicsBodySpawn>,
    terrain_patches: BTreeMap<PhysicsColliderId, PhysicsTerrainPatch>,
}

impl NoopPhysicsWorld {
    fn new() -> Self {
        Self {
            next_body_id: 1,
            next_collider_id: 1,
            bodies: BTreeMap::new(),
            terrain_patches: BTreeMap::new(),
        }
    }

    const fn backend_kind(&self) -> PhysicsBackendKind {
        PhysicsBackendKind::Noop
    }

    fn spawn_body(&mut self, spawn: PhysicsBodySpawn) -> PhysicsBodyId {
        let id = PhysicsBodyId(self.next_body_id);
        self.next_body_id = self.next_body_id.wrapping_add(1).max(1);
        self.bodies.insert(id, spawn);
        id
    }

    fn remove_body(&mut self, id: PhysicsBodyId) -> Option<PhysicsBodySpawn> {
        self.bodies.remove(&id)
    }

    fn body(&self, id: PhysicsBodyId) -> Option<&PhysicsBodySpawn> {
        self.bodies.get(&id)
    }

    fn set_body_pose(&mut self, id: PhysicsBodyId, pose: PhysicsBodyPose) -> bool {
        let Some(body) = self.bodies.get_mut(&id) else {
            return false;
        };
        body.pose = pose;
        true
    }

    fn add_terrain_patch(&mut self, patch: PhysicsTerrainPatch) -> PhysicsColliderId {
        let id = PhysicsColliderId(self.next_collider_id);
        self.next_collider_id = self.next_collider_id.wrapping_add(1).max(1);
        self.terrain_patches.insert(id, patch);
        id
    }

    fn update_terrain_patch(&mut self, id: PhysicsColliderId, patch: PhysicsTerrainPatch) -> bool {
        let Some(existing) = self.terrain_patches.get_mut(&id) else {
            return false;
        };
        *existing = patch;
        true
    }

    fn remove_terrain_patch(&mut self, id: PhysicsColliderId) -> Option<PhysicsTerrainPatch> {
        self.terrain_patches.remove(&id)
    }

    fn terrain_patch(&self, id: PhysicsColliderId) -> Option<&PhysicsTerrainPatch> {
        self.terrain_patches.get(&id)
    }

    fn step(&mut self, dt_seconds: f64) -> PhysicsStepReport {
        PhysicsStepReport {
            backend: self.backend_kind(),
            dt_seconds,
            body_count: self.bodies.len(),
            collider_count: self.terrain_patches.len(),
            active_body_count: 0,
            terrain_patch_count: self.terrain_patches.len(),
            body_pose_update_count: 0,
        }
    }
}

impl Default for PhysicsBackendKind {
    fn default() -> Self {
        Self::Noop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch(revision: u64, solid_cell_count: usize) -> PhysicsTerrainPatch {
        PhysicsTerrainPatch::new(
            Aabb::new(0.0, 0.0, 0.0, 16.0, 16.0, 16.0),
            revision,
            solid_cell_count,
        )
    }

    #[test]
    fn noop_world_allocates_stable_body_ids() {
        let mut world = PhysicsWorld::noop();

        let first = world.spawn_body(PhysicsBodySpawn::dynamic_cube(
            Vec3d::new(1.0, 2.0, 3.0),
            0.5,
            Vec3d::new(0.0, 1.0, 0.0),
        ));
        let second = world.spawn_body(PhysicsBodySpawn::dynamic_cube(
            Vec3d::new(4.0, 5.0, 6.0),
            0.5,
            Vec3d::ZERO,
        ));

        assert_eq!(first, PhysicsBodyId(1));
        assert_eq!(second, PhysicsBodyId(2));
        assert_eq!(
            world.body_pose(first),
            Some(PhysicsBodyPose::new(
                Vec3d::new(1.0, 2.0, 3.0),
                PhysicsRotation::IDENTITY
            ))
        );
    }

    #[test]
    fn noop_world_updates_body_pose_without_simulating() {
        let mut world = PhysicsWorld::noop();
        let body = world.spawn_body(PhysicsBodySpawn::dynamic_cube(
            Vec3d::ZERO,
            0.5,
            Vec3d::new(100.0, 0.0, 0.0),
        ));

        let pose = PhysicsBodyPose::new(Vec3d::new(8.0, 9.0, 10.0), PhysicsRotation::IDENTITY);
        assert!(world.set_body_pose(body, pose));
        assert_eq!(world.body_pose(body), Some(pose));

        let report = world.step(1.0 / 20.0);
        assert_eq!(report.body_count, 1);
        assert_eq!(report.active_body_count, 0);
        assert_eq!(report.body_pose_update_count, 0);
        assert_eq!(world.body_pose(body), Some(pose));
        assert!(!world.set_body_pose(PhysicsBodyId(99), pose));
    }

    #[test]
    fn noop_world_tracks_terrain_patch_lifecycle() {
        let mut world = PhysicsWorld::noop();
        let first = world.add_terrain_patch(patch(1, 12));
        let second = world.add_terrain_patch(patch(2, 24));

        assert_eq!(first, PhysicsColliderId(1));
        assert_eq!(second, PhysicsColliderId(2));
        assert_eq!(world.terrain_patch(first), Some(&patch(1, 12)));

        assert!(world.update_terrain_patch(first, patch(3, 48)));
        assert_eq!(world.terrain_patch(first), Some(&patch(3, 48)));
        assert!(!world.update_terrain_patch(PhysicsColliderId(99), patch(4, 1)));

        assert_eq!(world.remove_terrain_patch(second), Some(patch(2, 24)));
        assert_eq!(world.remove_terrain_patch(second), None);

        let report = world.step(0.05);
        assert_eq!(report.collider_count, 1);
        assert_eq!(report.terrain_patch_count, 1);
    }

    #[test]
    fn noop_world_removes_bodies() {
        let mut world = PhysicsWorld::new(PhysicsBackendKind::Noop);
        let spawn = PhysicsBodySpawn::dynamic_cube(Vec3d::new(1.0, 2.0, 3.0), 0.5, Vec3d::ZERO);
        let body = world.spawn_body(spawn);

        assert_eq!(world.remove_body(body), Some(spawn));
        assert_eq!(world.remove_body(body), None);
        assert_eq!(world.step(0.05).body_count, 0);
    }
}
