use mclone_core::{BlockPos, ChunkPos, Vec3d};
use mclone_protocol::{
    EntityId, EntityKind, EntityPersistentId, EntityRotation, EntitySnapshot, EntityUpdate,
    ItemStackSnapshot,
};

use super::metadata::EntityMetadata;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ServerEntityState {
    pub(crate) id: EntityId,
    pub(crate) persistent_id: EntityPersistentId,
    pub(crate) kind: EntityKind,
    pub(crate) item_stack: Option<ItemStackSnapshot>,
    pub(crate) position: Vec3d,
    pub(crate) y_rot_degrees: f32,
    pub(crate) x_rot_degrees: f32,
    pub(crate) rotation: Option<EntityRotation>,
    pub(crate) on_ground: bool,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) tick_count: u64,
    pub(crate) alive: bool,
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
            position,
            y_rot_degrees,
            x_rot_degrees,
            rotation,
            on_ground,
            width: metadata.dimensions.width,
            height: metadata.dimensions.height,
            tick_count: 0,
            alive: true,
        }
    }

    pub(crate) fn chunk_pos(self) -> ChunkPos {
        BlockPos::containing(self.position).chunk_pos()
    }

    pub(crate) fn snapshot(self) -> EntitySnapshot {
        EntitySnapshot {
            id: self.id,
            persistent_id: self.persistent_id,
            kind: self.kind,
            item_stack: self.item_stack,
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
            position: self.position,
            y_rot_degrees: self.y_rot_degrees,
            x_rot_degrees: self.x_rot_degrees,
            rotation: self.rotation,
            on_ground: self.on_ground,
            tick_count: self.tick_count,
        }
    }
}
