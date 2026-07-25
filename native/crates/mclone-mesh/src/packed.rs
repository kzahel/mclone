use std::fmt;

use crate::{
    GrassPatch, RenderSectionKey, TexturedChunkVertex, TexturedRenderSectionMesh,
    TexturedVisibleChunkMesh, VisibilitySet,
};

const MAGIC: &[u8; 8] = b"MCLMSH01";
const HEADER_BYTES: usize = MAGIC.len() + std::mem::size_of::<u32>();
const SECTION_HEADER_BYTES: usize =
    3 * std::mem::size_of::<i32>() + std::mem::size_of::<u64>() + 5 * std::mem::size_of::<u32>();
const TEXTURED_VERTEX_BYTES: usize = 10 * std::mem::size_of::<u32>();
const GRASS_PATCH_BYTES: usize = 8 * std::mem::size_of::<u32>();

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackedTexturedSectionError {
    message: String,
}

impl PackedTexturedSectionError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PackedTexturedSectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for PackedTexturedSectionError {}

/// Encode complete textured render sections into the stable browser Worker
/// wire format. Empty sections remain explicit so the renderer can release a
/// previously drawable GPU range.
pub fn pack_textured_render_sections(sections: &[TexturedRenderSectionMesh]) -> Vec<u8> {
    let payload_bytes = sections.iter().fold(HEADER_BYTES, |bytes, section| {
        bytes
            .saturating_add(SECTION_HEADER_BYTES)
            .saturating_add(
                section
                    .mesh
                    .vertices
                    .len()
                    .saturating_mul(TEXTURED_VERTEX_BYTES),
            )
            .saturating_add(
                section
                    .mesh
                    .indices
                    .len()
                    .saturating_mul(std::mem::size_of::<u32>()),
            )
            .saturating_add(
                section
                    .grass_patches
                    .len()
                    .saturating_mul(GRASS_PATCH_BYTES),
            )
    });
    let mut bytes = Vec::with_capacity(payload_bytes);
    bytes.extend_from_slice(MAGIC);
    push_u32(&mut bytes, sections.len().min(u32::MAX as usize) as u32);
    for section in sections {
        push_i32(&mut bytes, section.key.chunk_x);
        push_i32(&mut bytes, section.key.section_y);
        push_i32(&mut bytes, section.key.chunk_z);
        push_u64(&mut bytes, section.visibility.bits());
        push_u32(&mut bytes, section.mesh.solid_index_count);
        push_u32(&mut bytes, section.mesh.opaque_index_count);
        push_u32(
            &mut bytes,
            section.mesh.vertices.len().min(u32::MAX as usize) as u32,
        );
        push_u32(
            &mut bytes,
            section.mesh.indices.len().min(u32::MAX as usize) as u32,
        );
        push_u32(
            &mut bytes,
            section.grass_patches.len().min(u32::MAX as usize) as u32,
        );
        for vertex in &section.mesh.vertices {
            for value in vertex
                .position
                .into_iter()
                .chain(vertex.uv)
                .chain(vertex.color)
            {
                push_u32(&mut bytes, value.to_bits());
            }
            push_u32(&mut bytes, vertex.packed_light);
        }
        for index in &section.mesh.indices {
            push_u32(&mut bytes, *index);
        }
        for patch in &section.grass_patches {
            for value in patch.root {
                push_i32(&mut bytes, value);
            }
            push_u32(&mut bytes, patch.packed_tint);
            push_u32(&mut bytes, patch.packed_light);
            push_u32(&mut bytes, patch.seed);
            push_u32(&mut bytes, patch.flags);
            push_u32(&mut bytes, patch.reserved);
        }
    }
    bytes
}

/// Decode the stable browser Worker wire format without relying on native
/// structure layout or unsafe byte casts.
pub fn unpack_textured_render_sections(
    bytes: &[u8],
) -> Result<Vec<TexturedRenderSectionMesh>, PackedTexturedSectionError> {
    let mut input = PackedInput::new(bytes);
    if input.take(MAGIC.len())? != MAGIC {
        return Err(PackedTexturedSectionError::new(
            "invalid packed textured-section magic or version",
        ));
    }
    let section_count = input.u32()? as usize;
    let mut sections = Vec::with_capacity(section_count);
    for _ in 0..section_count {
        let key = RenderSectionKey::new(input.i32()?, input.i32()?, input.i32()?);
        let visibility = VisibilitySet::from_bits(input.u64()?);
        let solid_index_count = input.u32()?;
        let opaque_index_count = input.u32()?;
        let vertex_count = input.u32()? as usize;
        let index_count = input.u32()? as usize;
        let grass_count = input.u32()? as usize;
        let required = vertex_count
            .checked_mul(TEXTURED_VERTEX_BYTES)
            .and_then(|value| {
                index_count
                    .checked_mul(std::mem::size_of::<u32>())
                    .and_then(|indices| value.checked_add(indices))
            })
            .and_then(|value| {
                grass_count
                    .checked_mul(GRASS_PATCH_BYTES)
                    .and_then(|grass| value.checked_add(grass))
            })
            .ok_or_else(|| {
                PackedTexturedSectionError::new("packed textured-section sizes overflow")
            })?;
        if input.remaining() < required {
            return Err(PackedTexturedSectionError::new(
                "packed textured-section payload is truncated",
            ));
        }
        if solid_index_count > opaque_index_count || opaque_index_count as usize > index_count {
            return Err(PackedTexturedSectionError::new(format!(
                "invalid packed index layers {solid_index_count}/{opaque_index_count}/{index_count}"
            )));
        }
        let mut vertices = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            vertices.push(TexturedChunkVertex {
                position: [
                    f32::from_bits(input.u32()?),
                    f32::from_bits(input.u32()?),
                    f32::from_bits(input.u32()?),
                ],
                uv: [f32::from_bits(input.u32()?), f32::from_bits(input.u32()?)],
                color: [
                    f32::from_bits(input.u32()?),
                    f32::from_bits(input.u32()?),
                    f32::from_bits(input.u32()?),
                    f32::from_bits(input.u32()?),
                ],
                packed_light: input.u32()?,
            });
        }
        let mut indices = Vec::with_capacity(index_count);
        for _ in 0..index_count {
            indices.push(input.u32()?);
        }
        let mut grass_patches = Vec::with_capacity(grass_count);
        for _ in 0..grass_count {
            grass_patches.push(GrassPatch {
                root: [input.i32()?, input.i32()?, input.i32()?],
                packed_tint: input.u32()?,
                packed_light: input.u32()?,
                seed: input.u32()?,
                flags: input.u32()?,
                reserved: input.u32()?,
            });
        }
        sections.push(TexturedRenderSectionMesh {
            key,
            mesh: TexturedVisibleChunkMesh {
                vertices,
                indices,
                solid_index_count,
                opaque_index_count,
            },
            grass_patches,
            visibility,
        });
    }
    if input.remaining() != 0 {
        return Err(PackedTexturedSectionError::new(format!(
            "packed textured-section payload has {} trailing bytes",
            input.remaining()
        )));
    }
    Ok(sections)
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

struct PackedInput<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> PackedInput<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], PackedTexturedSectionError> {
        let end = self.offset.checked_add(count).ok_or_else(|| {
            PackedTexturedSectionError::new("packed textured-section offset overflow")
        })?;
        let value = self.bytes.get(self.offset..end).ok_or_else(|| {
            PackedTexturedSectionError::new("packed textured-section payload is truncated")
        })?;
        self.offset = end;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, PackedTexturedSectionError> {
        let bytes: [u8; 4] = self
            .take(std::mem::size_of::<u32>())?
            .try_into()
            .expect("the packed u32 slice has a fixed length");
        Ok(u32::from_le_bytes(bytes))
    }

    fn i32(&mut self) -> Result<i32, PackedTexturedSectionError> {
        let bytes: [u8; 4] = self
            .take(std::mem::size_of::<i32>())?
            .try_into()
            .expect("the packed i32 slice has a fixed length");
        Ok(i32::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, PackedTexturedSectionError> {
        let bytes: [u8; 8] = self
            .take(std::mem::size_of::<u64>())?
            .try_into()
            .expect("the packed u64 slice has a fixed length");
        Ok(u64::from_le_bytes(bytes))
    }
}
