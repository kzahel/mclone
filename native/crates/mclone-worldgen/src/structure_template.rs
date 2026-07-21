use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use mclone_core::{BlockPos, ChunkPos};

use crate::block::{
    OAK_LOG_X, OAK_LOG_Z, RawBlockId, SPRUCE_LOG_X, SPRUCE_LOG_Z, WALL_TORCH_EAST,
    WALL_TORCH_NORTH, WALL_TORCH_SOUTH, WALL_TORCH_WEST,
};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemplateRotation {
    #[default]
    None,
    Clockwise90,
    Clockwise180,
    CounterClockwise90,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemplateMirror {
    #[default]
    None,
    X,
    Z,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemplateMaterialRole {
    Foundation,
    Wall,
    TimberY,
    TimberX,
    TimberZ,
    Roof,
    Floor,
    Accent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructureMaterialTheme {
    id: String,
    materials: BTreeMap<TemplateMaterialRole, RawBlockId>,
}

impl StructureMaterialTheme {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            materials: BTreeMap::new(),
        }
    }

    pub fn with(mut self, role: TemplateMaterialRole, block: RawBlockId) -> Self {
        self.materials.insert(role, block);
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn resolve(&self, role: TemplateMaterialRole) -> Option<RawBlockId> {
        self.materials.get(&role).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemplateBlockState {
    Exact(RawBlockId),
    Role(TemplateMaterialRole),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemplateBlock {
    pub local_pos: BlockPos,
    pub state: TemplateBlockState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateMarker {
    pub local_pos: BlockPos,
    pub kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructureTemplate {
    id: String,
    size: [i32; 3],
    blocks: Vec<TemplateBlock>,
    markers: Vec<TemplateMarker>,
}

impl StructureTemplate {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn size(&self) -> [i32; 3] {
        self.size
    }

    pub fn blocks(&self) -> &[TemplateBlock] {
        &self.blocks
    }

    pub fn markers(&self) -> &[TemplateMarker] {
        &self.markers
    }

    pub fn place(
        &self,
        settings: &StructurePlaceSettings<'_>,
    ) -> Result<PlacedStructureTemplate, TemplateError> {
        let transformed_size = transformed_size(self.size, settings.rotation);
        let bounds = TemplateBoundingBox {
            min: settings.origin,
            max_exclusive: settings.origin.offset(
                transformed_size[0],
                transformed_size[1],
                transformed_size[2],
            ),
        };
        let mut blocks = BTreeMap::new();
        for block in &self.blocks {
            let local_pos = transform_pos(
                block.local_pos,
                self.size,
                settings.mirror,
                settings.rotation,
            );
            let raw = match block.state {
                TemplateBlockState::Exact(raw) => raw,
                TemplateBlockState::Role(role) => {
                    settings.theme.resolve(role).ok_or_else(|| {
                        TemplateError::MissingMaterialRole {
                            theme: settings.theme.id().to_owned(),
                            role,
                        }
                    })?
                }
            };
            blocks.insert(
                settings
                    .origin
                    .offset(local_pos.x, local_pos.y, local_pos.z),
                transform_block_state(raw, settings.mirror, settings.rotation),
            );
        }

        let mut markers = self
            .markers
            .iter()
            .map(|marker| {
                let local_pos = transform_pos(
                    marker.local_pos,
                    self.size,
                    settings.mirror,
                    settings.rotation,
                );
                PlacedTemplateMarker {
                    pos: settings
                        .origin
                        .offset(local_pos.x, local_pos.y, local_pos.z),
                    kind: marker.kind.clone(),
                }
            })
            .collect::<Vec<_>>();
        markers.sort_by(|left, right| {
            left.pos
                .cmp(&right.pos)
                .then_with(|| left.kind.cmp(&right.kind))
        });

        Ok(PlacedStructureTemplate {
            template_id: self.id.clone(),
            theme_id: settings.theme.id().to_owned(),
            bounds,
            blocks: blocks
                .into_iter()
                .map(|(pos, block)| PlacedTemplateBlock { pos, block })
                .collect(),
            markers,
        })
    }
}

#[derive(Clone, Debug)]
pub struct StructureTemplateBuilder {
    id: String,
    size: [i32; 3],
    blocks: BTreeMap<BlockPos, TemplateBlockState>,
    markers: Vec<TemplateMarker>,
}

impl StructureTemplateBuilder {
    pub fn new(id: impl Into<String>, size: [i32; 3]) -> Result<Self, TemplateError> {
        if size.into_iter().any(|axis| axis <= 0) {
            return Err(TemplateError::InvalidSize(size));
        }
        Ok(Self {
            id: id.into(),
            size,
            blocks: BTreeMap::new(),
            markers: Vec::new(),
        })
    }

    pub fn set(
        &mut self,
        pos: BlockPos,
        state: TemplateBlockState,
    ) -> Result<&mut Self, TemplateError> {
        self.validate_pos(pos)?;
        self.blocks.insert(pos, state);
        Ok(self)
    }

    pub fn fill_box(
        &mut self,
        min: BlockPos,
        max_exclusive: BlockPos,
        state: TemplateBlockState,
    ) -> Result<&mut Self, TemplateError> {
        if min.x >= max_exclusive.x || min.y >= max_exclusive.y || min.z >= max_exclusive.z {
            return Err(TemplateError::InvalidBox { min, max_exclusive });
        }
        self.validate_pos(min)?;
        self.validate_pos(max_exclusive.offset(-1, -1, -1))?;
        for y in min.y..max_exclusive.y {
            for z in min.z..max_exclusive.z {
                for x in min.x..max_exclusive.x {
                    self.blocks.insert(BlockPos::new(x, y, z), state);
                }
            }
        }
        Ok(self)
    }

    pub fn marker(
        &mut self,
        pos: BlockPos,
        kind: impl Into<String>,
    ) -> Result<&mut Self, TemplateError> {
        self.validate_pos(pos)?;
        self.markers.push(TemplateMarker {
            local_pos: pos,
            kind: kind.into(),
        });
        Ok(self)
    }

    pub fn build(self) -> StructureTemplate {
        StructureTemplate {
            id: self.id,
            size: self.size,
            blocks: self
                .blocks
                .into_iter()
                .map(|(local_pos, state)| TemplateBlock { local_pos, state })
                .collect(),
            markers: self.markers,
        }
    }

    fn validate_pos(&self, pos: BlockPos) -> Result<(), TemplateError> {
        if pos.x < 0
            || pos.y < 0
            || pos.z < 0
            || pos.x >= self.size[0]
            || pos.y >= self.size[1]
            || pos.z >= self.size[2]
        {
            return Err(TemplateError::PositionOutsideTemplate {
                pos,
                size: self.size,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StructurePlaceSettings<'a> {
    pub origin: BlockPos,
    pub rotation: TemplateRotation,
    pub mirror: TemplateMirror,
    pub theme: &'a StructureMaterialTheme,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlacedTemplateBlock {
    pub pos: BlockPos,
    pub block: RawBlockId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacedTemplateMarker {
    pub pos: BlockPos,
    pub kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacedStructureTemplate {
    pub template_id: String,
    pub theme_id: String,
    pub bounds: TemplateBoundingBox,
    pub blocks: Vec<PlacedTemplateBlock>,
    pub markers: Vec<PlacedTemplateMarker>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemplateBoundingBox {
    pub min: BlockPos,
    pub max_exclusive: BlockPos,
}

impl TemplateBoundingBox {
    pub fn touched_chunks(self) -> BTreeSet<ChunkPos> {
        let min = self.min.chunk_pos();
        let max = self.max_exclusive.offset(-1, 0, -1).chunk_pos();
        let mut chunks = BTreeSet::new();
        for chunk_z in min.z..=max.z {
            for chunk_x in min.x..=max.x {
                chunks.insert(ChunkPos::new(chunk_x, chunk_z));
            }
        }
        chunks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateError {
    InvalidSize([i32; 3]),
    InvalidBox {
        min: BlockPos,
        max_exclusive: BlockPos,
    },
    PositionOutsideTemplate {
        pos: BlockPos,
        size: [i32; 3],
    },
    MissingMaterialRole {
        theme: String,
        role: TemplateMaterialRole,
    },
}

impl fmt::Display for TemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize(size) => write!(formatter, "template size {size:?} must be positive"),
            Self::InvalidBox { min, max_exclusive } => write!(
                formatter,
                "template box {min:?}..{max_exclusive:?} must have positive extent"
            ),
            Self::PositionOutsideTemplate { pos, size } => {
                write!(
                    formatter,
                    "template position {pos:?} is outside size {size:?}"
                )
            }
            Self::MissingMaterialRole { theme, role } => {
                write!(formatter, "theme `{theme}` does not define role {role:?}")
            }
        }
    }
}

impl Error for TemplateError {}

fn transformed_size(size: [i32; 3], rotation: TemplateRotation) -> [i32; 3] {
    match rotation {
        TemplateRotation::None | TemplateRotation::Clockwise180 => size,
        TemplateRotation::Clockwise90 | TemplateRotation::CounterClockwise90 => {
            [size[2], size[1], size[0]]
        }
    }
}

fn transform_pos(
    pos: BlockPos,
    size: [i32; 3],
    mirror: TemplateMirror,
    rotation: TemplateRotation,
) -> BlockPos {
    let mirrored_x = if mirror == TemplateMirror::X {
        size[0] - 1 - pos.x
    } else {
        pos.x
    };
    let mirrored_z = if mirror == TemplateMirror::Z {
        size[2] - 1 - pos.z
    } else {
        pos.z
    };
    match rotation {
        TemplateRotation::None => BlockPos::new(mirrored_x, pos.y, mirrored_z),
        TemplateRotation::Clockwise90 => BlockPos::new(size[2] - 1 - mirrored_z, pos.y, mirrored_x),
        TemplateRotation::Clockwise180 => {
            BlockPos::new(size[0] - 1 - mirrored_x, pos.y, size[2] - 1 - mirrored_z)
        }
        TemplateRotation::CounterClockwise90 => {
            BlockPos::new(mirrored_z, pos.y, size[0] - 1 - mirrored_x)
        }
    }
}

fn transform_block_state(
    block: RawBlockId,
    mirror: TemplateMirror,
    rotation: TemplateRotation,
) -> RawBlockId {
    match block {
        OAK_LOG_X | OAK_LOG_Z | SPRUCE_LOG_X | SPRUCE_LOG_Z => {
            if matches!(
                rotation,
                TemplateRotation::Clockwise90 | TemplateRotation::CounterClockwise90
            ) {
                match block {
                    OAK_LOG_X => OAK_LOG_Z,
                    OAK_LOG_Z => OAK_LOG_X,
                    SPRUCE_LOG_X => SPRUCE_LOG_Z,
                    SPRUCE_LOG_Z => SPRUCE_LOG_X,
                    _ => unreachable!(),
                }
            } else {
                block
            }
        }
        WALL_TORCH_NORTH | WALL_TORCH_EAST | WALL_TORCH_SOUTH | WALL_TORCH_WEST => {
            transform_wall_torch(block, mirror, rotation)
        }
        _ => block,
    }
}

fn transform_wall_torch(
    block: RawBlockId,
    mirror: TemplateMirror,
    rotation: TemplateRotation,
) -> RawBlockId {
    let (mut x, mut z) = match block {
        WALL_TORCH_NORTH => (0, -1),
        WALL_TORCH_EAST => (1, 0),
        WALL_TORCH_SOUTH => (0, 1),
        WALL_TORCH_WEST => (-1, 0),
        _ => unreachable!(),
    };
    match mirror {
        TemplateMirror::None => {}
        TemplateMirror::X => x = -x,
        TemplateMirror::Z => z = -z,
    }
    (x, z) = match rotation {
        TemplateRotation::None => (x, z),
        TemplateRotation::Clockwise90 => (-z, x),
        TemplateRotation::Clockwise180 => (-x, -z),
        TemplateRotation::CounterClockwise90 => (z, -x),
    };
    match (x, z) {
        (0, -1) => WALL_TORCH_NORTH,
        (1, 0) => WALL_TORCH_EAST,
        (0, 1) => WALL_TORCH_SOUTH,
        (-1, 0) => WALL_TORCH_WEST,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{BRICKS, OAK_LOG_X, OAK_LOG_Z, STONE, WALL_TORCH_EAST};

    fn theme() -> StructureMaterialTheme {
        StructureMaterialTheme::new("test-theme")
            .with(TemplateMaterialRole::Foundation, STONE)
            .with(TemplateMaterialRole::TimberX, OAK_LOG_X)
    }

    #[test]
    fn later_builder_writes_replace_earlier_box_cells_deterministically() {
        let mut builder = StructureTemplateBuilder::new("override", [3, 2, 3]).unwrap();
        builder
            .fill_box(
                BlockPos::ZERO,
                BlockPos::new(3, 1, 3),
                TemplateBlockState::Role(TemplateMaterialRole::Foundation),
            )
            .unwrap()
            .set(BlockPos::new(1, 0, 1), TemplateBlockState::Exact(BRICKS))
            .unwrap();
        let template = builder.build();
        let theme = theme();
        let settings = StructurePlaceSettings {
            origin: BlockPos::new(10, 20, 30),
            rotation: TemplateRotation::None,
            mirror: TemplateMirror::None,
            theme: &theme,
        };
        let placed = template.place(&settings).unwrap();

        assert_eq!(placed.blocks.len(), 9);
        assert_eq!(placed.blocks[4].pos, BlockPos::new(11, 20, 31));
        assert_eq!(placed.blocks[4].block, BRICKS);
        assert_eq!(template.place(&settings).unwrap(), placed);
    }

    #[test]
    fn rotation_changes_non_square_bounds_and_transforms_markers() {
        let mut builder = StructureTemplateBuilder::new("turn", [2, 3, 4]).unwrap();
        builder
            .set(
                BlockPos::new(0, 0, 0),
                TemplateBlockState::Role(TemplateMaterialRole::TimberX),
            )
            .unwrap()
            .marker(BlockPos::new(1, 1, 3), "entrance")
            .unwrap();
        let placed = builder
            .build()
            .place(&StructurePlaceSettings {
                origin: BlockPos::new(15, 64, -2),
                rotation: TemplateRotation::Clockwise90,
                mirror: TemplateMirror::None,
                theme: &theme(),
            })
            .unwrap();

        assert_eq!(placed.bounds.max_exclusive, BlockPos::new(19, 67, 0));
        assert_eq!(placed.blocks[0].pos, BlockPos::new(18, 64, -2));
        assert_eq!(placed.blocks[0].block, OAK_LOG_Z);
        assert_eq!(placed.markers[0].pos, BlockPos::new(15, 65, -1));
    }

    #[test]
    fn mirror_then_rotation_transforms_facing_state() {
        let mut builder = StructureTemplateBuilder::new("facing", [1, 1, 1]).unwrap();
        builder
            .set(BlockPos::ZERO, TemplateBlockState::Exact(WALL_TORCH_EAST))
            .unwrap();
        let placed = builder
            .build()
            .place(&StructurePlaceSettings {
                origin: BlockPos::ZERO,
                rotation: TemplateRotation::Clockwise90,
                mirror: TemplateMirror::X,
                theme: &theme(),
            })
            .unwrap();

        assert_eq!(placed.blocks[0].block, WALL_TORCH_NORTH);
    }

    #[test]
    fn bounding_box_reports_cross_chunk_reach() {
        let bounds = TemplateBoundingBox {
            min: BlockPos::new(-1, 60, 15),
            max_exclusive: BlockPos::new(17, 70, 33),
        };
        assert_eq!(
            bounds.touched_chunks(),
            BTreeSet::from([
                ChunkPos::new(-1, 0),
                ChunkPos::new(-1, 1),
                ChunkPos::new(-1, 2),
                ChunkPos::new(0, 0),
                ChunkPos::new(0, 1),
                ChunkPos::new(0, 2),
                ChunkPos::new(1, 0),
                ChunkPos::new(1, 1),
                ChunkPos::new(1, 2),
            ])
        );
    }

    #[test]
    fn invalid_bounds_and_missing_roles_are_rejected() {
        assert!(matches!(
            StructureTemplateBuilder::new("bad", [1, 0, 1]),
            Err(TemplateError::InvalidSize([1, 0, 1]))
        ));
        let mut builder = StructureTemplateBuilder::new("missing", [1, 1, 1]).unwrap();
        builder
            .set(
                BlockPos::ZERO,
                TemplateBlockState::Role(TemplateMaterialRole::Wall),
            )
            .unwrap();
        assert!(matches!(
            builder.build().place(&StructurePlaceSettings {
                origin: BlockPos::ZERO,
                rotation: TemplateRotation::None,
                mirror: TemplateMirror::None,
                theme: &theme(),
            }),
            Err(TemplateError::MissingMaterialRole { .. })
        ));
    }
}
