use mclone_core::{BlockPos, ChunkPos, Vec3d};
use mclone_protocol::{
    EntityId, EntityKind, EntityRotation, EntitySnapshot, EntityUpdate, ItemStackSnapshot,
};

use super::metadata::EntityMetadata;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ServerEntityState {
    pub(crate) id: EntityId,
    pub(crate) kind: EntityKind,
    pub(crate) item_stack: Option<ItemStackSnapshot>,
    pub(crate) position: Vec3d,
    pub(crate) y_rot_degrees: f32,
    pub(crate) x_rot_degrees: f32,
    pub(crate) rotation: Option<EntityRotation>,
    pub(crate) on_ground: bool,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) age_ticks: u64,
    pub(crate) alive: bool,
}

impl ServerEntityState {
    pub(crate) fn from_metadata(
        id: EntityId,
        metadata: EntityMetadata,
        position: Vec3d,
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        rotation: Option<EntityRotation>,
        on_ground: bool,
    ) -> Self {
        Self {
            id,
            kind: metadata.kind,
            item_stack: None,
            position,
            y_rot_degrees,
            x_rot_degrees,
            rotation,
            on_ground,
            width: metadata.dimensions.width,
            height: metadata.dimensions.height,
            age_ticks: 0,
            alive: true,
        }
    }

    pub(crate) fn chunk_pos(self) -> ChunkPos {
        BlockPos::containing(self.position).chunk_pos()
    }

    pub(crate) fn snapshot(self) -> EntitySnapshot {
        EntitySnapshot {
            id: self.id,
            kind: self.kind,
            item_stack: self.item_stack,
            position: self.position,
            y_rot_degrees: self.y_rot_degrees,
            x_rot_degrees: self.x_rot_degrees,
            rotation: self.rotation,
            on_ground: self.on_ground,
            width: self.width,
            height: self.height,
            age_ticks: self.age_ticks,
        }
    }

    pub(crate) fn update(self) -> EntityUpdate {
        EntityUpdate {
            id: self.id,
            item_stack: self.item_stack,
            position: self.position,
            y_rot_degrees: self.y_rot_degrees,
            x_rot_degrees: self.x_rot_degrees,
            rotation: self.rotation,
            on_ground: self.on_ground,
            age_ticks: self.age_ticks,
        }
    }
}
