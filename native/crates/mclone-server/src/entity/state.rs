use mclone_core::{AnimationClipId, AnimationState, BlockPos, ChunkPos, Vec3d};
use mclone_protocol::{
    DeerSnapshotData, EntityId, EntityKind, EntityPersistentId, EntityRotation, EntitySnapshot,
    EntityUpdate, ItemStackSnapshot, MallardNestSnapshotData, MallardNestUpdateData,
    MallardSnapshotData, MallardUpdateData,
};

use super::metadata::EntityMetadata;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ServerEntityState {
    pub(crate) id: EntityId,
    pub(crate) persistent_id: EntityPersistentId,
    pub(crate) kind: EntityKind,
    pub(crate) item_stack: Option<ItemStackSnapshot>,
    pub(crate) mallard: Option<MallardSnapshotData>,
    pub(crate) mallard_nest: Option<MallardNestSnapshotData>,
    pub(crate) deer: Option<DeerSnapshotData>,
    pub(crate) animation: Option<AnimationState>,
    pub(crate) position: Vec3d,
    pub(crate) y_rot_degrees: f32,
    pub(crate) x_rot_degrees: f32,
    pub(crate) rotation: Option<EntityRotation>,
    pub(crate) on_ground: bool,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) tick_count: u64,
    pub(crate) alive: bool,
    pub(crate) hidden_from_clients: bool,
}

impl ServerEntityState {
    pub(crate) fn from_metadata(
        id: EntityId,
        persistent_id: EntityPersistentId,
        metadata: EntityMetadata,
        position: Vec3d,
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        rotation: Option<EntityRotation>,
        on_ground: bool,
    ) -> Self {
        Self {
            id,
            persistent_id,
            kind: metadata.kind,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            deer: None,
            animation: default_animation_for_kind(metadata.kind),
            position,
            y_rot_degrees,
            x_rot_degrees,
            rotation,
            on_ground,
            width: metadata.dimensions.width,
            height: metadata.dimensions.height,
            tick_count: 0,
            alive: true,
            hidden_from_clients: false,
        }
    }

    pub(crate) fn chunk_pos(self) -> ChunkPos {
        BlockPos::containing(self.position).chunk_pos()
    }

    pub(crate) const fn client_visible(self) -> bool {
        self.alive && !self.hidden_from_clients
    }

    pub(crate) fn snapshot(self) -> EntitySnapshot {
        EntitySnapshot {
            id: self.id,
            persistent_id: self.persistent_id,
            kind: self.kind,
            item_stack: self.item_stack,
            mallard: self.mallard,
            mallard_nest: self.mallard_nest,
            deer: self.deer,
            animation: self.animation,
            position: self.position,
            y_rot_degrees: self.y_rot_degrees,
            x_rot_degrees: self.x_rot_degrees,
            rotation: self.rotation,
            on_ground: self.on_ground,
            width: self.width,
            height: self.height,
            tick_count: self.tick_count,
        }
    }

    pub(crate) fn update(self) -> EntityUpdate {
        EntityUpdate {
            id: self.id,
            item_stack: self.item_stack,
            mallard: self.mallard.map(|data| MallardUpdateData {
                life_stage: data.life_stage,
                in_water: data.in_water,
            }),
            mallard_nest: self.mallard_nest.map(|data| MallardNestUpdateData {
                incubation_progress: data.incubation_progress,
                incubation_required: data.incubation_required,
                attended: data.attended,
            }),
            deer: self.deer,
            animation: self.animation,
            position: self.position,
            y_rot_degrees: self.y_rot_degrees,
            x_rot_degrees: self.x_rot_degrees,
            rotation: self.rotation,
            on_ground: self.on_ground,
            tick_count: self.tick_count,
        }
    }
}

const fn default_animation_for_kind(kind: EntityKind) -> Option<AnimationState> {
    let clip = match kind {
        EntityKind::Cow | EntityKind::Chicken | EntityKind::Mannequin => {
            AnimationClipId::from_static("walk")
        }
        EntityKind::Mallard => AnimationClipId::from_static("waddle"),
        EntityKind::Deer => AnimationClipId::from_static("idle"),
        EntityKind::Bee => AnimationClipId::from_static("hover"),
        EntityKind::Rabbit => AnimationClipId::from_static("idle"),
        EntityKind::MallardNest
        | EntityKind::DeerBed
        | EntityKind::BeeNest
        | EntityKind::BeeHotel
        | EntityKind::RabbitBurrow
        | EntityKind::WildlifeRemains
        | EntityKind::SleepingMat
        | EntityKind::DebugCube
        | EntityKind::Item => return None,
    };
    Some(AnimationState::distance(clip, 0))
}
