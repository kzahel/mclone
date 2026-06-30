#![forbid(unsafe_code)]

use std::collections::BTreeMap;
#[cfg(feature = "rapier")]
use std::time::Instant;

use mclone_core::{Aabb, Vec3d};
#[cfg(feature = "rapier")]
use rapier3d::prelude as rapier;

pub const PHYSICS_TERRAIN_SECTION_WIDTH: usize = 16;
pub const PHYSICS_TERRAIN_SECTION_VOLUME: usize =
    PHYSICS_TERRAIN_SECTION_WIDTH * PHYSICS_TERRAIN_SECTION_WIDTH * PHYSICS_TERRAIN_SECTION_WIDTH;

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

    pub const fn dynamic_capsule_y(
        position: Vec3d,
        half_height: f64,
        radius: f64,
        velocity: Vec3d,
    ) -> Self {
        Self {
            kind: PhysicsBodyKind::Dynamic,
            shape: PhysicsShape::CapsuleY {
                half_height,
                radius,
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

#[derive(Clone, Debug, PartialEq)]
pub struct PhysicsTerrainSection {
    pub origin: Vec3d,
    pub voxel_size: f64,
    solid_cells: Vec<u8>,
    solid_cell_count: usize,
}

impl PhysicsTerrainSection {
    pub fn new(origin: Vec3d, voxel_size: f64) -> Self {
        Self {
            origin,
            voxel_size: voxel_size.max(1.0e-4),
            solid_cells: vec![0; PHYSICS_TERRAIN_SECTION_VOLUME],
            solid_cell_count: 0,
        }
    }

    pub fn bounds(&self) -> Aabb {
        let width = PHYSICS_TERRAIN_SECTION_WIDTH as f64 * self.voxel_size;
        Aabb::new(
            self.origin.x,
            self.origin.y,
            self.origin.z,
            self.origin.x + width,
            self.origin.y + width,
            self.origin.z + width,
        )
    }

    pub const fn solid_cell_count(&self) -> usize {
        self.solid_cell_count
    }

    pub fn set_solid(&mut self, x: usize, y: usize, z: usize, solid: bool) -> bool {
        let Some(index) = terrain_section_index(x, y, z) else {
            return false;
        };
        let old_solid = self.solid_cells[index] != 0;
        if old_solid == solid {
            return true;
        }
        self.solid_cells[index] = u8::from(solid);
        if solid {
            self.solid_cell_count += 1;
        } else {
            self.solid_cell_count -= 1;
        }
        true
    }

    pub fn is_solid(&self, x: usize, y: usize, z: usize) -> bool {
        terrain_section_index(x, y, z).is_some_and(|index| self.solid_cells[index] != 0)
    }

    #[cfg(feature = "rapier")]
    fn solid_cells(&self) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        self.solid_cells
            .iter()
            .enumerate()
            .filter(|(_, solid)| **solid != 0)
            .map(|(index, _)| terrain_section_coords(index))
    }
}

const fn terrain_section_index(x: usize, y: usize, z: usize) -> Option<usize> {
    if x < PHYSICS_TERRAIN_SECTION_WIDTH
        && y < PHYSICS_TERRAIN_SECTION_WIDTH
        && z < PHYSICS_TERRAIN_SECTION_WIDTH
    {
        Some((y * PHYSICS_TERRAIN_SECTION_WIDTH + z) * PHYSICS_TERRAIN_SECTION_WIDTH + x)
    } else {
        None
    }
}

#[cfg(feature = "rapier")]
const fn terrain_section_coords(index: usize) -> (usize, usize, usize) {
    let x = index % PHYSICS_TERRAIN_SECTION_WIDTH;
    let yz = index / PHYSICS_TERRAIN_SECTION_WIDTH;
    let z = yz % PHYSICS_TERRAIN_SECTION_WIDTH;
    let y = yz / PHYSICS_TERRAIN_SECTION_WIDTH;
    (x, y, z)
}

#[cfg(feature = "rapier")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PhysicsTerrainColliderCandidate {
    MergedCuboids,
    Voxels,
    Trimesh,
}

#[cfg(feature = "rapier")]
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicsTerrainProbeReport {
    pub candidate: PhysicsTerrainColliderCandidate,
    pub solid_cell_count: usize,
    pub primitive_count: usize,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub build_micros: u128,
    pub simulation_micros: u128,
    pub collider_count: usize,
    pub body_pose_update_count: usize,
    pub cube_final_y: f64,
    pub capsule_final_y: f64,
}

#[cfg(feature = "rapier")]
pub fn run_rapier_section_terrain_probe(
    section: &PhysicsTerrainSection,
) -> Vec<PhysicsTerrainProbeReport> {
    [
        PhysicsTerrainColliderCandidate::MergedCuboids,
        PhysicsTerrainColliderCandidate::Voxels,
        PhysicsTerrainColliderCandidate::Trimesh,
    ]
    .into_iter()
    .filter_map(|candidate| run_rapier_section_terrain_candidate_probe(section, candidate))
    .collect()
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

    pub fn body_velocity(&self, id: PhysicsBodyId) -> Option<PhysicsBodyVelocity> {
        self.backend.body_velocity(id)
    }

    pub fn set_body_pose(&mut self, id: PhysicsBodyId, pose: PhysicsBodyPose) -> bool {
        self.backend.set_body_pose(id, pose)
    }

    pub fn set_body_velocity(&mut self, id: PhysicsBodyId, velocity: PhysicsBodyVelocity) -> bool {
        self.backend.set_body_velocity(id, velocity)
    }

    pub fn gravity(&self) -> Vec3d {
        self.backend.gravity()
    }

    pub fn set_gravity(&mut self, gravity: Vec3d) -> bool {
        self.backend.set_gravity(gravity)
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

    pub fn add_terrain_section(&mut self, section: PhysicsTerrainSection) -> PhysicsColliderId {
        self.backend.add_terrain_section(section)
    }

    pub fn update_terrain_section(
        &mut self,
        id: PhysicsColliderId,
        section: PhysicsTerrainSection,
    ) -> bool {
        self.backend.update_terrain_section(id, section)
    }

    pub fn remove_terrain_section(
        &mut self,
        id: PhysicsColliderId,
    ) -> Option<PhysicsTerrainSection> {
        self.backend.remove_terrain_section(id)
    }

    pub fn terrain_section(&self, id: PhysicsColliderId) -> Option<&PhysicsTerrainSection> {
        self.backend.terrain_section(id)
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

    fn body_velocity(&self, id: PhysicsBodyId) -> Option<PhysicsBodyVelocity> {
        match self {
            Self::Noop(backend) => backend.body_velocity(id),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.body_velocity(id),
        }
    }

    fn set_body_pose(&mut self, id: PhysicsBodyId, pose: PhysicsBodyPose) -> bool {
        match self {
            Self::Noop(backend) => backend.set_body_pose(id, pose),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.set_body_pose(id, pose),
        }
    }

    fn set_body_velocity(&mut self, id: PhysicsBodyId, velocity: PhysicsBodyVelocity) -> bool {
        match self {
            Self::Noop(backend) => backend.set_body_velocity(id, velocity),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.set_body_velocity(id, velocity),
        }
    }

    fn gravity(&self) -> Vec3d {
        match self {
            Self::Noop(backend) => backend.gravity(),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.gravity(),
        }
    }

    fn set_gravity(&mut self, gravity: Vec3d) -> bool {
        match self {
            Self::Noop(backend) => backend.set_gravity(gravity),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.set_gravity(gravity),
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

    fn add_terrain_section(&mut self, section: PhysicsTerrainSection) -> PhysicsColliderId {
        match self {
            Self::Noop(backend) => backend.add_terrain_section(section),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.add_terrain_section(section),
        }
    }

    fn update_terrain_section(
        &mut self,
        id: PhysicsColliderId,
        section: PhysicsTerrainSection,
    ) -> bool {
        match self {
            Self::Noop(backend) => backend.update_terrain_section(id, section),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.update_terrain_section(id, section),
        }
    }

    fn remove_terrain_section(&mut self, id: PhysicsColliderId) -> Option<PhysicsTerrainSection> {
        match self {
            Self::Noop(backend) => backend.remove_terrain_section(id),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.remove_terrain_section(id),
        }
    }

    fn terrain_section(&self, id: PhysicsColliderId) -> Option<&PhysicsTerrainSection> {
        match self {
            Self::Noop(backend) => backend.terrain_section(id),
            #[cfg(feature = "rapier")]
            Self::Rapier(backend) => backend.terrain_section(id),
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
    gravity: Vec3d,
    bodies: BTreeMap<PhysicsBodyId, PhysicsBodySpawn>,
    terrain_patches: BTreeMap<PhysicsColliderId, PhysicsTerrainPatch>,
    terrain_sections: BTreeMap<PhysicsColliderId, PhysicsTerrainSection>,
}

impl NoopPhysicsWorld {
    fn new() -> Self {
        Self {
            next_body_id: 1,
            next_collider_id: 1,
            gravity: Vec3d::new(0.0, -9.81, 0.0),
            bodies: BTreeMap::new(),
            terrain_patches: BTreeMap::new(),
            terrain_sections: BTreeMap::new(),
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

    fn body_velocity(&self, id: PhysicsBodyId) -> Option<PhysicsBodyVelocity> {
        self.bodies.get(&id).map(|body| body.velocity)
    }

    fn set_body_pose(&mut self, id: PhysicsBodyId, pose: PhysicsBodyPose) -> bool {
        let Some(body) = self.bodies.get_mut(&id) else {
            return false;
        };
        body.pose = pose;
        true
    }

    fn set_body_velocity(&mut self, id: PhysicsBodyId, velocity: PhysicsBodyVelocity) -> bool {
        if !velocity.linear.is_finite() || !velocity.angular.is_finite() {
            return false;
        }
        let Some(body) = self.bodies.get_mut(&id) else {
            return false;
        };
        body.velocity = velocity;
        true
    }

    const fn gravity(&self) -> Vec3d {
        self.gravity
    }

    fn set_gravity(&mut self, gravity: Vec3d) -> bool {
        if !gravity.is_finite() {
            return false;
        }
        self.gravity = gravity;
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

    fn add_terrain_section(&mut self, section: PhysicsTerrainSection) -> PhysicsColliderId {
        let id = PhysicsColliderId(self.next_collider_id);
        self.next_collider_id = self.next_collider_id.wrapping_add(1).max(1);
        self.terrain_sections.insert(id, section);
        id
    }

    fn update_terrain_section(
        &mut self,
        id: PhysicsColliderId,
        section: PhysicsTerrainSection,
    ) -> bool {
        let Some(existing) = self.terrain_sections.get_mut(&id) else {
            return false;
        };
        *existing = section;
        true
    }

    fn remove_terrain_section(&mut self, id: PhysicsColliderId) -> Option<PhysicsTerrainSection> {
        self.terrain_sections.remove(&id)
    }

    fn terrain_section(&self, id: PhysicsColliderId) -> Option<&PhysicsTerrainSection> {
        self.terrain_sections.get(&id)
    }

    fn step(&mut self, dt_seconds: f64) -> PhysicsStepReport {
        let terrain_collider_count = self.terrain_patches.len() + self.terrain_sections.len();
        PhysicsStepReport {
            backend: self.backend_kind(),
            dt_seconds,
            body_count: self.bodies.len(),
            collider_count: terrain_collider_count,
            active_body_count: 0,
            terrain_patch_count: terrain_collider_count,
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
    terrain_sections: BTreeMap<PhysicsColliderId, RapierTerrainSectionRecord>,
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
struct RapierTerrainSectionRecord {
    handle: Option<rapier::ColliderHandle>,
    section: PhysicsTerrainSection,
}

#[cfg(feature = "rapier")]
struct RapierTerrainCandidateBuild {
    candidate: PhysicsTerrainColliderCandidate,
    collider: rapier::Collider,
    primitive_count: usize,
    vertex_count: usize,
    triangle_count: usize,
    build_micros: u128,
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
            terrain_sections: BTreeMap::new(),
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

    fn body_velocity(&self, id: PhysicsBodyId) -> Option<PhysicsBodyVelocity> {
        let record = self.bodies.get(&id)?;
        let body = self.rigid_body_set.get(record.handle)?;
        Some(physics_body_velocity(body))
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

    fn set_body_velocity(&mut self, id: PhysicsBodyId, velocity: PhysicsBodyVelocity) -> bool {
        if !velocity.linear.is_finite() || !velocity.angular.is_finite() {
            return false;
        }
        let Some(record) = self.bodies.get_mut(&id) else {
            return false;
        };
        let Some(body) = self.rigid_body_set.get_mut(record.handle) else {
            return false;
        };
        body.set_linvel(rapier_vec(velocity.linear), true);
        body.set_angvel(rapier_vec(velocity.angular), true);
        record.spawn.velocity = velocity;
        true
    }

    fn gravity(&self) -> Vec3d {
        physics_vec(self.gravity)
    }

    fn set_gravity(&mut self, gravity: Vec3d) -> bool {
        if !gravity.is_finite() {
            return false;
        }
        self.gravity = rapier_vec(gravity);
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

    fn add_terrain_section(&mut self, section: PhysicsTerrainSection) -> PhysicsColliderId {
        let id = PhysicsColliderId(self.next_collider_id);
        self.next_collider_id = self.next_collider_id.wrapping_add(1).max(1);
        let handle = self.insert_terrain_section_collider(&section);
        self.terrain_sections
            .insert(id, RapierTerrainSectionRecord { handle, section });
        id
    }

    fn update_terrain_section(
        &mut self,
        id: PhysicsColliderId,
        section: PhysicsTerrainSection,
    ) -> bool {
        let Some(old_handle) = self
            .terrain_sections
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
        let handle = self.insert_terrain_section_collider(&section);
        let existing = self
            .terrain_sections
            .get_mut(&id)
            .expect("terrain section record survives collider replacement");
        existing.handle = handle;
        existing.section = section;
        true
    }

    fn remove_terrain_section(&mut self, id: PhysicsColliderId) -> Option<PhysicsTerrainSection> {
        let record = self.terrain_sections.remove(&id)?;
        if let Some(handle) = record.handle {
            self.collider_set.remove(
                handle,
                &mut self.island_manager,
                &mut self.rigid_body_set,
                true,
            );
        }
        Some(record.section)
    }

    fn terrain_section(&self, id: PhysicsColliderId) -> Option<&PhysicsTerrainSection> {
        self.terrain_sections.get(&id).map(|record| &record.section)
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
            terrain_patch_count: self.terrain_patches.len() + self.terrain_sections.len(),
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

    fn insert_terrain_section_collider(
        &mut self,
        section: &PhysicsTerrainSection,
    ) -> Option<rapier::ColliderHandle> {
        if section.solid_cell_count() == 0 {
            return None;
        }
        Some(
            self.collider_set
                .insert(rapier_terrain_section_collider_builder(section)?.build()),
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
fn rapier_terrain_section_collider_builder(
    section: &PhysicsTerrainSection,
) -> Option<rapier::ColliderBuilder> {
    let shapes = rapier_section_merged_x_run_shapes(section);
    (!shapes.is_empty()).then(|| rapier::ColliderBuilder::compound(shapes))
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
fn physics_body_velocity(body: &rapier::RigidBody) -> PhysicsBodyVelocity {
    PhysicsBodyVelocity {
        linear: physics_vec(body.linvel()),
        angular: physics_vec(body.angvel()),
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
fn physics_vec(value: rapier::Vector) -> Vec3d {
    Vec3d::new(f64::from(value.x), f64::from(value.y), f64::from(value.z))
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

#[cfg(feature = "rapier")]
fn run_rapier_section_terrain_candidate_probe(
    section: &PhysicsTerrainSection,
    candidate: PhysicsTerrainColliderCandidate,
) -> Option<PhysicsTerrainProbeReport> {
    let build = build_rapier_section_terrain_candidate(section, candidate)?;
    let mut world = RapierPhysicsWorld::new();
    world.collider_set.insert(build.collider);

    let center = section.bounds();
    let spawn_y = section.origin.y + section.voxel_size * 5.0;
    let cube = world.spawn_body(PhysicsBodySpawn::dynamic_cube(
        Vec3d::new(
            (center.min_x + center.max_x) * 0.5 - 2.0,
            spawn_y,
            (center.min_z + center.max_z) * 0.5,
        ),
        0.5,
        Vec3d::ZERO,
    ));
    let capsule = world.spawn_body(PhysicsBodySpawn::dynamic_capsule_y(
        Vec3d::new(
            (center.min_x + center.max_x) * 0.5 + 2.0,
            spawn_y,
            (center.min_z + center.max_z) * 0.5,
        ),
        0.5,
        0.25,
        Vec3d::ZERO,
    ));

    let step_start = Instant::now();
    let mut body_pose_update_count = 0;
    for _ in 0..240 {
        body_pose_update_count += world.step(1.0 / 60.0).body_pose_update_count;
    }
    let simulation_micros = step_start.elapsed().as_micros();

    Some(PhysicsTerrainProbeReport {
        candidate: build.candidate,
        solid_cell_count: section.solid_cell_count(),
        primitive_count: build.primitive_count,
        vertex_count: build.vertex_count,
        triangle_count: build.triangle_count,
        build_micros: build.build_micros,
        simulation_micros,
        collider_count: world.collider_set.len(),
        body_pose_update_count,
        cube_final_y: world
            .body(cube)
            .expect("probe cube body survives")
            .pose
            .position
            .y,
        capsule_final_y: world
            .body(capsule)
            .expect("probe capsule body survives")
            .pose
            .position
            .y,
    })
}

#[cfg(feature = "rapier")]
fn build_rapier_section_terrain_candidate(
    section: &PhysicsTerrainSection,
    candidate: PhysicsTerrainColliderCandidate,
) -> Option<RapierTerrainCandidateBuild> {
    let start = Instant::now();
    let (builder, primitive_count, vertex_count, triangle_count) = match candidate {
        PhysicsTerrainColliderCandidate::MergedCuboids => {
            let shapes = rapier_section_merged_x_run_shapes(section);
            let primitive_count = shapes.len();
            if primitive_count == 0 {
                return None;
            }
            (
                rapier::ColliderBuilder::compound(shapes),
                primitive_count,
                0,
                0,
            )
        }
        PhysicsTerrainColliderCandidate::Voxels => {
            let voxels = rapier_section_voxels(section);
            let primitive_count = voxels.len();
            if primitive_count == 0 {
                return None;
            }
            (
                rapier::ColliderBuilder::voxels(
                    rapier::Vector::splat(to_rapier_real(section.voxel_size)),
                    &voxels,
                )
                .translation(rapier_vec(section.origin)),
                primitive_count,
                0,
                0,
            )
        }
        PhysicsTerrainColliderCandidate::Trimesh => {
            let (vertices, indices) = rapier_section_visible_face_mesh(section);
            let vertex_count = vertices.len();
            let triangle_count = indices.len();
            if triangle_count == 0 {
                return None;
            }
            (
                rapier::ColliderBuilder::trimesh(vertices, indices).ok()?,
                triangle_count,
                vertex_count,
                triangle_count,
            )
        }
    };

    let collider = builder.build();

    Some(RapierTerrainCandidateBuild {
        candidate,
        collider,
        primitive_count,
        vertex_count,
        triangle_count,
        build_micros: start.elapsed().as_micros(),
    })
}

#[cfg(feature = "rapier")]
fn rapier_section_merged_x_run_shapes(
    section: &PhysicsTerrainSection,
) -> Vec<(rapier::Pose, rapier::SharedShape)> {
    let mut shapes = Vec::new();
    let voxel_size = section.voxel_size;
    for y in 0..PHYSICS_TERRAIN_SECTION_WIDTH {
        for z in 0..PHYSICS_TERRAIN_SECTION_WIDTH {
            let mut x = 0;
            while x < PHYSICS_TERRAIN_SECTION_WIDTH {
                if !section.is_solid(x, y, z) {
                    x += 1;
                    continue;
                }
                let start_x = x;
                while x < PHYSICS_TERRAIN_SECTION_WIDTH && section.is_solid(x, y, z) {
                    x += 1;
                }
                let run_width = x - start_x;
                let center = Vec3d::new(
                    section.origin.x + (start_x as f64 + run_width as f64 * 0.5) * voxel_size,
                    section.origin.y + (y as f64 + 0.5) * voxel_size,
                    section.origin.z + (z as f64 + 0.5) * voxel_size,
                );
                shapes.push((
                    rapier::Pose::from_parts(rapier_vec(center), rapier::Rotation::IDENTITY),
                    rapier::SharedShape::cuboid(
                        positive_rapier_real(run_width as f64 * voxel_size * 0.5),
                        positive_rapier_real(voxel_size * 0.5),
                        positive_rapier_real(voxel_size * 0.5),
                    ),
                ));
            }
        }
    }
    shapes
}

#[cfg(feature = "rapier")]
fn rapier_section_voxels(section: &PhysicsTerrainSection) -> Vec<rapier::IVector> {
    section
        .solid_cells()
        .map(|(x, y, z)| rapier::IVector::new(x as i32, y as i32, z as i32))
        .collect()
}

#[cfg(feature = "rapier")]
fn rapier_section_visible_face_mesh(
    section: &PhysicsTerrainSection,
) -> (Vec<rapier::Vector>, Vec<[u32; 3]>) {
    const FACES: [((isize, isize, isize), [[f64; 3]; 4]); 6] = [
        (
            (-1, 0, 0),
            [
                [0.0, 0.0, 1.0],
                [0.0, 1.0, 1.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
            ],
        ),
        (
            (1, 0, 0),
            [
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
                [1.0, 0.0, 1.0],
            ],
        ),
        (
            (0, -1, 0),
            [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
        ),
        (
            (0, 1, 0),
            [
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
        ),
        (
            (0, 0, -1),
            [
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
            ],
        ),
        (
            (0, 0, 1),
            [
                [1.0, 0.0, 1.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
        ),
    ];

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let voxel_size = section.voxel_size;
    for (x, y, z) in section.solid_cells() {
        for ((dx, dy, dz), corners) in FACES {
            if terrain_neighbor_solid(section, x, y, z, dx, dy, dz) {
                continue;
            }
            let base_index = vertices.len() as u32;
            for [cx, cy, cz] in corners {
                vertices.push(rapier_vec(Vec3d::new(
                    section.origin.x + (x as f64 + cx) * voxel_size,
                    section.origin.y + (y as f64 + cy) * voxel_size,
                    section.origin.z + (z as f64 + cz) * voxel_size,
                )));
            }
            indices.push([base_index, base_index + 1, base_index + 2]);
            indices.push([base_index, base_index + 2, base_index + 3]);
        }
    }
    (vertices, indices)
}

#[cfg(feature = "rapier")]
fn terrain_neighbor_solid(
    section: &PhysicsTerrainSection,
    x: usize,
    y: usize,
    z: usize,
    dx: isize,
    dy: isize,
    dz: isize,
) -> bool {
    let nx = x as isize + dx;
    let ny = y as isize + dy;
    let nz = z as isize + dz;
    if nx < 0 || ny < 0 || nz < 0 {
        return false;
    }
    section.is_solid(nx as usize, ny as usize, nz as usize)
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
        assert_eq!(
            world.body_velocity(body),
            Some(PhysicsBodyVelocity {
                linear: Vec3d::new(100.0, 0.0, 0.0),
                angular: Vec3d::ZERO,
            })
        );

        let pose = PhysicsBodyPose::new(Vec3d::new(8.0, 9.0, 10.0), PhysicsRotation::IDENTITY);
        assert!(world.set_body_pose(body, pose));
        assert_eq!(world.body_pose(body), Some(pose));
        let velocity = PhysicsBodyVelocity {
            linear: Vec3d::new(1.0, 2.0, 3.0),
            angular: Vec3d::new(0.1, 0.2, 0.3),
        };
        assert!(world.set_body_velocity(body, velocity));
        assert_eq!(world.body_velocity(body), Some(velocity));

        let report = world.step(1.0 / 20.0);
        assert_eq!(report.body_count, 1);
        assert_eq!(report.active_body_count, 0);
        assert_eq!(report.body_pose_update_count, 0);
        assert_eq!(world.body_pose(body), Some(pose));
        assert!(!world.set_body_pose(PhysicsBodyId(99), pose));
        assert!(!world.set_body_velocity(PhysicsBodyId(99), velocity));
    }

    #[test]
    fn noop_world_tracks_configured_gravity() {
        let mut world = PhysicsWorld::noop();
        assert_eq!(world.gravity(), Vec3d::new(0.0, -9.81, 0.0));

        let gravity = Vec3d::new(0.0, -32.0, 0.0);
        assert!(world.set_gravity(gravity));
        assert_eq!(world.gravity(), gravity);

        assert!(!world.set_gravity(Vec3d::new(f64::NAN, 0.0, 0.0)));
        assert_eq!(world.gravity(), gravity);
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

    #[test]
    fn terrain_section_tracks_solid_cells_and_bounds() {
        let mut section = PhysicsTerrainSection::new(Vec3d::new(-8.0, -1.0, -8.0), 1.0);

        assert_eq!(section.solid_cell_count(), 0);
        assert_eq!(
            section.bounds(),
            Aabb::new(-8.0, -1.0, -8.0, 8.0, 15.0, 8.0)
        );
        assert!(!section.is_solid(2, 3, 4));
        assert!(!section.is_solid(PHYSICS_TERRAIN_SECTION_WIDTH, 0, 0));

        assert!(section.set_solid(2, 3, 4, true));
        assert!(section.is_solid(2, 3, 4));
        assert_eq!(section.solid_cell_count(), 1);
        assert!(section.set_solid(2, 3, 4, true));
        assert_eq!(section.solid_cell_count(), 1);

        assert!(section.set_solid(2, 3, 4, false));
        assert!(!section.is_solid(2, 3, 4));
        assert_eq!(section.solid_cell_count(), 0);
        assert!(!section.set_solid(PHYSICS_TERRAIN_SECTION_WIDTH, 0, 0, true));
    }

    #[test]
    fn noop_world_tracks_terrain_section_lifecycle() {
        let mut world = PhysicsWorld::noop();
        let mut section = PhysicsTerrainSection::new(Vec3d::new(0.0, 0.0, 0.0), 1.0);
        assert!(section.set_solid(0, 0, 0, true));

        let terrain = world.add_terrain_section(section.clone());
        assert_eq!(terrain, PhysicsColliderId(1));
        assert_eq!(world.terrain_section(terrain), Some(&section));

        let mut updated = PhysicsTerrainSection::new(Vec3d::new(16.0, 0.0, 0.0), 1.0);
        assert!(updated.set_solid(1, 0, 0, true));
        assert!(updated.set_solid(2, 0, 0, true));
        assert!(world.update_terrain_section(terrain, updated.clone()));
        assert_eq!(world.terrain_section(terrain), Some(&updated));
        assert!(!world.update_terrain_section(PhysicsColliderId(99), updated.clone()));

        let report = world.step(0.05);
        assert_eq!(report.collider_count, 1);
        assert_eq!(report.terrain_patch_count, 1);
        assert_eq!(world.remove_terrain_section(terrain), Some(updated));
        assert_eq!(world.remove_terrain_section(terrain), None);
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

    #[cfg(feature = "rapier")]
    #[test]
    fn rapier_world_simulates_dynamic_cube_against_static_section() {
        let mut world = PhysicsWorld::new(PhysicsBackendKind::Rapier);
        let mut section = PhysicsTerrainSection::new(Vec3d::ZERO, 1.0);
        for x in 0..PHYSICS_TERRAIN_SECTION_WIDTH {
            for z in 0..PHYSICS_TERRAIN_SECTION_WIDTH {
                assert!(section.set_solid(x, 0, z, true));
            }
        }
        let terrain = world.add_terrain_section(section.clone());
        let body = world.spawn_body(PhysicsBodySpawn::dynamic_cube(
            Vec3d::new(8.0, 4.0, 8.0),
            0.5,
            Vec3d::ZERO,
        ));

        let mut updated_while_falling = false;
        for _ in 0..160 {
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
            pose.position.y > 1.45 && pose.position.y < 1.75,
            "cube should settle on section floor top, got y={}",
            pose.position.y
        );
        assert_eq!(world.terrain_section(terrain), Some(&section));
    }

    #[cfg(feature = "rapier")]
    #[test]
    fn rapier_world_uses_configured_gravity_for_dynamic_bodies() {
        fn body_y_after_one_tick(gravity: Vec3d) -> f64 {
            let mut world = PhysicsWorld::new(PhysicsBackendKind::Rapier);
            assert!(world.set_gravity(gravity));
            assert!(world.gravity().distance_to_sqr(gravity) < 1.0e-10);
            let body = world.spawn_body(PhysicsBodySpawn::dynamic_cube(
                Vec3d::new(0.0, 8.0, 0.0),
                0.5,
                Vec3d::ZERO,
            ));

            world.step(0.05);
            world.body_pose(body).expect("body pose").position.y
        }

        let default_gravity_y = body_y_after_one_tick(Vec3d::new(0.0, -9.81, 0.0));
        let minecraft_gravity_y = body_y_after_one_tick(Vec3d::new(0.0, -32.0, 0.0));

        assert!(
            minecraft_gravity_y < default_gravity_y,
            "stronger Minecraft gravity should move the body farther down: default={default_gravity_y}, minecraft={minecraft_gravity_y}"
        );
    }

    #[cfg(feature = "rapier")]
    #[test]
    fn rapier_world_sets_body_velocity() {
        let mut world = PhysicsWorld::new(PhysicsBackendKind::Rapier);
        let body = world.spawn_body(PhysicsBodySpawn::dynamic_cube(
            Vec3d::ZERO,
            0.5,
            Vec3d::ZERO,
        ));
        let velocity = PhysicsBodyVelocity {
            linear: Vec3d::new(3.0, 4.0, 5.0),
            angular: Vec3d::new(0.25, 0.5, 0.75),
        };

        assert!(world.set_body_velocity(body, velocity));
        let actual = world.body_velocity(body).expect("body velocity");
        assert!(actual.linear.distance_to_sqr(velocity.linear) < 1.0e-10);
        assert!(actual.angular.distance_to_sqr(velocity.angular) < 1.0e-10);
    }

    #[cfg(feature = "rapier")]
    #[test]
    fn rapier_section_terrain_probe_compares_candidate_shapes() {
        let mut section = PhysicsTerrainSection::new(Vec3d::new(-8.0, -1.0, -8.0), 1.0);
        for x in 0..PHYSICS_TERRAIN_SECTION_WIDTH {
            for z in 0..PHYSICS_TERRAIN_SECTION_WIDTH {
                assert!(section.set_solid(x, 0, z, true));
            }
        }
        for y in 1..4 {
            assert!(section.set_solid(15, y, 15, true));
        }

        let reports = run_rapier_section_terrain_probe(&section);
        assert_eq!(section.solid_cell_count(), 259);
        assert_eq!(reports.len(), 3);

        let merged = report_for_candidate(&reports, PhysicsTerrainColliderCandidate::MergedCuboids);
        assert_eq!(merged.primitive_count, 19);
        assert_eq!(merged.vertex_count, 0);
        assert_eq!(merged.triangle_count, 0);

        let voxels = report_for_candidate(&reports, PhysicsTerrainColliderCandidate::Voxels);
        assert_eq!(voxels.primitive_count, section.solid_cell_count());

        let trimesh = report_for_candidate(&reports, PhysicsTerrainColliderCandidate::Trimesh);
        assert_eq!(trimesh.primitive_count, 1_176);
        assert_eq!(trimesh.vertex_count, 2_352);
        assert_eq!(trimesh.triangle_count, 1_176);

        for report in reports {
            assert_eq!(report.solid_cell_count, section.solid_cell_count());
            assert_eq!(report.collider_count, 3);
            assert!(report.body_pose_update_count > 0);
            assert!(
                report.cube_final_y > 0.45 && report.cube_final_y < 0.8,
                "{:?} cube should settle on the floor, got y={}",
                report.candidate,
                report.cube_final_y
            );
            assert!(
                report.capsule_final_y > 0.65 && report.capsule_final_y < 1.0,
                "{:?} capsule should settle on the floor, got y={}",
                report.candidate,
                report.capsule_final_y
            );
        }
    }

    #[cfg(feature = "rapier")]
    fn report_for_candidate(
        reports: &[PhysicsTerrainProbeReport],
        candidate: PhysicsTerrainColliderCandidate,
    ) -> &PhysicsTerrainProbeReport {
        reports
            .iter()
            .find(|report| report.candidate == candidate)
            .expect("candidate report")
    }
}
