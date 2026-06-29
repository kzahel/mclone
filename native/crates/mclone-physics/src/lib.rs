#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use mclone_core::{Aabb, Vec3d};
#[cfg(feature = "rapier")]
use rapier3d::prelude as rapier;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicsBodyId(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicsColliderId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PhysicsBackendKind {
    Noop,
    #[cfg(feature = "rapier")]
    Rapier,
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
    backend: PhysicsWorldBackend,
}

impl PhysicsWorld {
    pub fn new(kind: PhysicsBackendKind) -> Self {
        match kind {
            PhysicsBackendKind::Noop => Self::noop(),
            #[cfg(feature = "rapier")]
            PhysicsBackendKind::Rapier => Self::rapier(),
        }
    }

    pub fn noop() -> Self {
        Self {
            backend: PhysicsWorldBackend::Noop(NoopPhysicsWorld::new()),
        }
    }

    #[cfg(feature = "rapier")]
    pub fn rapier() -> Self {
        Self {
            backend: PhysicsWorldBackend::Rapier(RapierPhysicsWorld::new()),
        }
    }

    pub fn backend_kind(&self) -> PhysicsBackendKind {
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

enum PhysicsWorldBackend {
    Noop(NoopPhysicsWorld),
    #[cfg(feature = "rapier")]
    Rapier(RapierPhysicsWorld),
}

impl PhysicsWorldBackend {
    fn backend_kind(&self) -> PhysicsBackendKind {
        match self {
            Self::Noop(backend) => backend.backend_kind(),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.backend_kind(),
        }
    }

    fn spawn_body(&mut self, spawn: PhysicsBodySpawn) -> PhysicsBodyId {
        match self {
            Self::Noop(backend) => backend.spawn_body(spawn),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.spawn_body(spawn),
        }
    }

    fn remove_body(&mut self, id: PhysicsBodyId) -> Option<PhysicsBodySpawn> {
        match self {
            Self::Noop(backend) => backend.remove_body(id),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.remove_body(id),
        }
    }

    fn body(&self, id: PhysicsBodyId) -> Option<&PhysicsBodySpawn> {
        match self {
            Self::Noop(backend) => backend.body(id),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.body(id),
        }
    }

    fn set_body_pose(&mut self, id: PhysicsBodyId, pose: PhysicsBodyPose) -> bool {
        match self {
            Self::Noop(backend) => backend.set_body_pose(id, pose),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.set_body_pose(id, pose),
        }
    }

    fn add_terrain_patch(&mut self, patch: PhysicsTerrainPatch) -> PhysicsColliderId {
        match self {
            Self::Noop(backend) => backend.add_terrain_patch(patch),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.add_terrain_patch(patch),
        }
    }

    fn update_terrain_patch(&mut self, id: PhysicsColliderId, patch: PhysicsTerrainPatch) -> bool {
        match self {
            Self::Noop(backend) => backend.update_terrain_patch(id, patch),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.update_terrain_patch(id, patch),
        }
    }

    fn remove_terrain_patch(&mut self, id: PhysicsColliderId) -> Option<PhysicsTerrainPatch> {
        match self {
            Self::Noop(backend) => backend.remove_terrain_patch(id),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.remove_terrain_patch(id),
        }
    }

    fn terrain_patch(&self, id: PhysicsColliderId) -> Option<&PhysicsTerrainPatch> {
        match self {
            Self::Noop(backend) => backend.terrain_patch(id),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.terrain_patch(id),
        }
    }

    fn step(&mut self, dt_seconds: f64) -> PhysicsStepReport {
        match self {
            Self::Noop(backend) => backend.step(dt_seconds),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.step(dt_seconds),
        }
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

#[cfg(feature = "rapier")]
struct RapierPhysicsWorld {
    gravity: rapier::Vector,
    integration_parameters: rapier::IntegrationParameters,
    physics_pipeline: rapier::PhysicsPipeline,
    island_manager: rapier::IslandManager,
    broad_phase: rapier::DefaultBroadPhase,
    narrow_phase: rapier::NarrowPhase,
    rigid_body_set: rapier::RigidBodySet,
    collider_set: rapier::ColliderSet,
    impulse_joint_set: rapier::ImpulseJointSet,
    multibody_joint_set: rapier::MultibodyJointSet,
    ccd_solver: rapier::CCDSolver,
    next_body_id: u64,
    next_collider_id: u64,
    bodies: BTreeMap<PhysicsBodyId, RapierBodyRecord>,
    terrain_patches: BTreeMap<PhysicsColliderId, RapierTerrainPatchRecord>,
}

#[cfg(feature = "rapier")]
struct RapierBodyRecord {
    handle: rapier::RigidBodyHandle,
    spawn: PhysicsBodySpawn,
}

#[cfg(feature = "rapier")]
struct RapierTerrainPatchRecord {
    handle: Option<rapier::ColliderHandle>,
    patch: PhysicsTerrainPatch,
}

#[cfg(feature = "rapier")]
impl RapierPhysicsWorld {
    fn new() -> Self {
        Self {
            gravity: rapier::Vector::new(0.0, -9.81, 0.0),
            integration_parameters: rapier::IntegrationParameters::default(),
            physics_pipeline: rapier::PhysicsPipeline::new(),
            island_manager: rapier::IslandManager::new(),
            broad_phase: rapier::DefaultBroadPhase::new(),
            narrow_phase: rapier::NarrowPhase::new(),
            rigid_body_set: rapier::RigidBodySet::new(),
            collider_set: rapier::ColliderSet::new(),
            impulse_joint_set: rapier::ImpulseJointSet::new(),
            multibody_joint_set: rapier::MultibodyJointSet::new(),
            ccd_solver: rapier::CCDSolver::new(),
            next_body_id: 1,
            next_collider_id: 1,
            bodies: BTreeMap::new(),
            terrain_patches: BTreeMap::new(),
        }
    }

    const fn backend_kind(&self) -> PhysicsBackendKind {
        PhysicsBackendKind::Rapier
    }

    fn spawn_body(&mut self, spawn: PhysicsBodySpawn) -> PhysicsBodyId {
        let id = PhysicsBodyId(self.next_body_id);
        self.next_body_id = self.next_body_id.wrapping_add(1).max(1);
        let rigid_body = rapier_body_builder(spawn).build();
        let body_handle = self.rigid_body_set.insert(rigid_body);
        let collider = rapier_collider_builder(spawn.shape).build();
        self.collider_set
            .insert_with_parent(collider, body_handle, &mut self.rigid_body_set);
        self.bodies.insert(
            id,
            RapierBodyRecord {
                handle: body_handle,
                spawn,
            },
        );
        id
    }

    fn remove_body(&mut self, id: PhysicsBodyId) -> Option<PhysicsBodySpawn> {
        let record = self.bodies.remove(&id)?;
        self.rigid_body_set.remove(
            record.handle,
            &mut self.island_manager,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            true,
        );
        Some(record.spawn)
    }

    fn body(&self, id: PhysicsBodyId) -> Option<&PhysicsBodySpawn> {
        self.bodies.get(&id).map(|record| &record.spawn)
    }

    fn set_body_pose(&mut self, id: PhysicsBodyId, pose: PhysicsBodyPose) -> bool {
        let Some(record) = self.bodies.get_mut(&id) else {
            return false;
        };
        let Some(body) = self.rigid_body_set.get_mut(record.handle) else {
            return false;
        };
        body.set_position(rapier_pose(pose), true);
        record.spawn.pose = pose;
        true
    }

    fn add_terrain_patch(&mut self, patch: PhysicsTerrainPatch) -> PhysicsColliderId {
        let id = PhysicsColliderId(self.next_collider_id);
        self.next_collider_id = self.next_collider_id.wrapping_add(1).max(1);
        let handle = self.insert_terrain_collider(patch);
        self.terrain_patches
            .insert(id, RapierTerrainPatchRecord { handle, patch });
        id
    }

    fn update_terrain_patch(&mut self, id: PhysicsColliderId, patch: PhysicsTerrainPatch) -> bool {
        let Some(old_handle) = self
            .terrain_patches
            .get_mut(&id)
            .map(|existing| existing.handle.take())
        else {
            return false;
        };
        if let Some(handle) = old_handle {
            self.collider_set.remove(
                handle,
                &mut self.island_manager,
                &mut self.rigid_body_set,
                true,
            );
        }
        let handle = self.insert_terrain_collider(patch);
        let existing = self
            .terrain_patches
            .get_mut(&id)
            .expect("terrain patch record survives collider replacement");
        existing.handle = handle;
        existing.patch = patch;
        true
    }

    fn remove_terrain_patch(&mut self, id: PhysicsColliderId) -> Option<PhysicsTerrainPatch> {
        let record = self.terrain_patches.remove(&id)?;
        if let Some(handle) = record.handle {
            self.collider_set.remove(
                handle,
                &mut self.island_manager,
                &mut self.rigid_body_set,
                true,
            );
        }
        Some(record.patch)
    }

    fn terrain_patch(&self, id: PhysicsColliderId) -> Option<&PhysicsTerrainPatch> {
        self.terrain_patches.get(&id).map(|record| &record.patch)
    }

    fn step(&mut self, dt_seconds: f64) -> PhysicsStepReport {
        self.integration_parameters.dt = to_rapier_real(dt_seconds.max(0.0));
        let before = self
            .bodies
            .iter()
            .filter_map(|(id, record)| {
                self.rigid_body_set
                    .get(record.handle)
                    .map(|body| (*id, physics_pose(body)))
            })
            .collect::<BTreeMap<_, _>>();
        self.physics_pipeline.step(
            self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            &(),
            &(),
        );

        let mut body_pose_update_count = 0;
        let mut active_body_count = 0;
        for (id, record) in &mut self.bodies {
            let Some(body) = self.rigid_body_set.get(record.handle) else {
                continue;
            };
            if !body.is_sleeping() {
                active_body_count += 1;
            }
            let pose = physics_pose(body);
            if before
                .get(id)
                .is_some_and(|before_pose| !poses_almost_equal(*before_pose, pose))
            {
                body_pose_update_count += 1;
            }
            record.spawn.pose = pose;
        }

        PhysicsStepReport {
            backend: self.backend_kind(),
            dt_seconds,
            body_count: self.bodies.len(),
            collider_count: self.collider_set.len(),
            active_body_count,
            terrain_patch_count: self.terrain_patches.len(),
            body_pose_update_count,
        }
    }

    fn insert_terrain_collider(
        &mut self,
        patch: PhysicsTerrainPatch,
    ) -> Option<rapier::ColliderHandle> {
        if patch.solid_cell_count == 0 {
            return None;
        }
        Some(
            self.collider_set
                .insert(rapier_terrain_collider_builder(patch)?.build()),
        )
    }
}

#[cfg(feature = "rapier")]
fn rapier_body_builder(spawn: PhysicsBodySpawn) -> rapier::RigidBodyBuilder {
    let builder = match spawn.kind {
        PhysicsBodyKind::Dynamic => rapier::RigidBodyBuilder::dynamic(),
        PhysicsBodyKind::Kinematic => rapier::RigidBodyBuilder::kinematic_velocity_based(),
        PhysicsBodyKind::Fixed => rapier::RigidBodyBuilder::fixed(),
    };
    builder
        .pose(rapier_pose(spawn.pose))
        .linvel(rapier_vec(spawn.velocity.linear))
        .angvel(rapier_vec(spawn.velocity.angular))
}

#[cfg(feature = "rapier")]
fn rapier_collider_builder(shape: PhysicsShape) -> rapier::ColliderBuilder {
    match shape {
        PhysicsShape::Cuboid { half_extents } => rapier::ColliderBuilder::cuboid(
            positive_rapier_real(half_extents.x),
            positive_rapier_real(half_extents.y),
            positive_rapier_real(half_extents.z),
        ),
        PhysicsShape::Ball { radius } => {
            rapier::ColliderBuilder::ball(positive_rapier_real(radius))
        }
        PhysicsShape::CapsuleY {
            half_height,
            radius,
        } => rapier::ColliderBuilder::capsule_y(
            positive_rapier_real(half_height),
            positive_rapier_real(radius),
        ),
    }
}

#[cfg(feature = "rapier")]
fn rapier_terrain_collider_builder(patch: PhysicsTerrainPatch) -> Option<rapier::ColliderBuilder> {
    let bounds = patch.bounds;
    if !bounds.is_finite() {
        return None;
    }
    let width = bounds.max_x - bounds.min_x;
    let height = bounds.max_y - bounds.min_y;
    let depth = bounds.max_z - bounds.min_z;
    if width <= 0.0 || height <= 0.0 || depth <= 0.0 {
        return None;
    }
    let center = Vec3d::new(
        (bounds.min_x + bounds.max_x) * 0.5,
        (bounds.min_y + bounds.max_y) * 0.5,
        (bounds.min_z + bounds.max_z) * 0.5,
    );
    Some(
        rapier::ColliderBuilder::cuboid(
            positive_rapier_real(width * 0.5),
            positive_rapier_real(height * 0.5),
            positive_rapier_real(depth * 0.5),
        )
        .translation(rapier_vec(center)),
    )
}

#[cfg(feature = "rapier")]
fn rapier_pose(pose: PhysicsBodyPose) -> rapier::Pose {
    rapier::Pose::from_parts(rapier_vec(pose.position), rapier_rotation(pose.rotation))
}

#[cfg(feature = "rapier")]
fn physics_pose(body: &rapier::RigidBody) -> PhysicsBodyPose {
    let translation = body.translation();
    let rotation = body.rotation();
    PhysicsBodyPose {
        position: Vec3d::new(
            f64::from(translation.x),
            f64::from(translation.y),
            f64::from(translation.z),
        ),
        rotation: PhysicsRotation {
            x: f64::from(rotation.x),
            y: f64::from(rotation.y),
            z: f64::from(rotation.z),
            w: f64::from(rotation.w),
        },
    }
}

#[cfg(feature = "rapier")]
fn rapier_rotation(rotation: PhysicsRotation) -> rapier::Rotation {
    if !rotation.x.is_finite()
        || !rotation.y.is_finite()
        || !rotation.z.is_finite()
        || !rotation.w.is_finite()
    {
        return rapier::Rotation::IDENTITY;
    }
    let quaternion = rapier::Rotation::from_xyzw(
        to_rapier_real(rotation.x),
        to_rapier_real(rotation.y),
        to_rapier_real(rotation.z),
        to_rapier_real(rotation.w),
    );
    if quaternion.length_squared() <= rapier::Real::EPSILON {
        rapier::Rotation::IDENTITY
    } else {
        quaternion.normalize()
    }
}

#[cfg(feature = "rapier")]
fn rapier_vec(value: Vec3d) -> rapier::Vector {
    rapier::Vector::new(
        to_rapier_real(value.x),
        to_rapier_real(value.y),
        to_rapier_real(value.z),
    )
}

#[cfg(feature = "rapier")]
fn to_rapier_real(value: f64) -> rapier::Real {
    value as rapier::Real
}

#[cfg(feature = "rapier")]
fn positive_rapier_real(value: f64) -> rapier::Real {
    to_rapier_real(value.max(1.0e-4))
}

#[cfg(feature = "rapier")]
fn poses_almost_equal(left: PhysicsBodyPose, right: PhysicsBodyPose) -> bool {
    left.position.distance_to_sqr(right.position) <= 1.0e-12
        && (left.rotation.x - right.rotation.x).abs() <= 1.0e-9
        && (left.rotation.y - right.rotation.y).abs() <= 1.0e-9
        && (left.rotation.z - right.rotation.z).abs() <= 1.0e-9
        && (left.rotation.w - right.rotation.w).abs() <= 1.0e-9
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

    #[cfg(feature = "rapier")]
    #[test]
    fn rapier_world_simulates_dynamic_cube_against_static_patch() {
        let mut world = PhysicsWorld::new(PhysicsBackendKind::Rapier);
        let terrain_patch =
            PhysicsTerrainPatch::new(Aabb::new(-8.0, -0.5, -8.0, 8.0, 0.0, 8.0), 1, 16 * 16);
        let terrain = world.add_terrain_patch(terrain_patch);
        let body = world.spawn_body(PhysicsBodySpawn::dynamic_cube(
            Vec3d::new(0.0, 4.0, 0.0),
            0.5,
            Vec3d::ZERO,
        ));

        let mut updated_while_falling = false;
        for _ in 0..240 {
            let report = world.step(1.0 / 60.0);
            assert_eq!(report.backend, PhysicsBackendKind::Rapier);
            assert_eq!(report.body_count, 1);
            assert_eq!(report.terrain_patch_count, 1);
            assert!(report.collider_count >= 2);
            updated_while_falling |= report.body_pose_update_count > 0;
        }

        let pose = world.body_pose(body).expect("dynamic body pose");
        assert!(updated_while_falling);
        assert!(
            pose.position.y > 0.45 && pose.position.y < 0.75,
            "cube should settle near terrain top, got y={}",
            pose.position.y
        );
        assert_eq!(world.terrain_patch(terrain).copied(), Some(terrain_patch));
    }
}
