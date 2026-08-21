use mclone_worldgen::levelgen::{
    McloneTreeArchetype, McloneTreeBounds, McloneTreeFamily, McloneTreeId, McloneTreeOccurrence,
    McloneTreeRecord,
};
use mclone_worldgen::placement::BlockPos;

use crate::CanonicalExactSurfaceColumn;

const CANONICAL_BATCH_MAGIC: [u8; 4] = *b"MCTB";
const CANONICAL_BATCH_VERSION: u16 = 4;
const CANONICAL_BATCH_HEADER_BYTES: usize = 72;
const CANONICAL_BATCH_ADMISSION_HEADER_BYTES: usize = 28;
const CANONICAL_BATCH_SURFACE_COLUMN_BYTES: usize = 4;
const CANONICAL_BATCH_NATURAL_TREE_HEADER_BYTES: usize = 101;
const TRANSFER_MS_OFFSET: usize = 40;

#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalEncodedNaturalTree {
    pub occurrence: McloneTreeOccurrence,
    pub packed_sections: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalEncodedAdmission {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub raw_cache_hit: bool,
    pub retained_dependency_chunks: u32,
    pub packed_sections: Vec<u8>,
    pub natural_trees: Vec<CanonicalEncodedNaturalTree>,
    pub surface_columns: Vec<CanonicalExactSurfaceColumn>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalEncodedBatch {
    pub generation_ms: f64,
    pub presentation_ms: f64,
    pub mesh_ms: f64,
    pub pack_ms: f64,
    pub transfer_ms: f64,
    pub deduplicated_target_chunks: u32,
    pub raw_cache_chunks: u32,
    pub raw_cache_bytes: u64,
    pub admissions: Vec<CanonicalEncodedAdmission>,
}

pub fn encode_canonical_batch(batch: &CanonicalEncodedBatch) -> Result<Vec<u8>, String> {
    validate_timings(batch)?;
    let admission_count = u32::try_from(batch.admissions.len())
        .map_err(|_| "canonical batch has more than u32 admissions".to_owned())?;
    let capacity =
        batch
            .admissions
            .iter()
            .try_fold(CANONICAL_BATCH_HEADER_BYTES, |bytes, admission| {
                let packed_bytes =
                    u32::try_from(admission.packed_sections.len()).map_err(|_| {
                        "canonical packed admission exceeds the u32 byte-length contract".to_owned()
                    })?;
                let natural_tree_bytes =
                    admission
                        .natural_trees
                        .iter()
                        .try_fold(0_usize, |bytes, tree| {
                            let packed_bytes =
                                u32::try_from(tree.packed_sections.len()).map_err(|_| {
                                    "canonical packed natural tree exceeds the u32 byte-length \
                                     contract"
                                        .to_owned()
                                })?;
                            bytes
                                .checked_add(CANONICAL_BATCH_NATURAL_TREE_HEADER_BYTES)
                                .and_then(|bytes| bytes.checked_add(packed_bytes as usize))
                                .ok_or_else(|| {
                                    "canonical natural-tree byte length overflowed usize".to_owned()
                                })
                        })?;
                let surface_column_bytes = admission
                    .surface_columns
                    .len()
                    .checked_mul(CANONICAL_BATCH_SURFACE_COLUMN_BYTES)
                    .ok_or_else(|| {
                        "canonical surface-column byte length overflowed usize".to_owned()
                    })?;
                bytes
                    .checked_add(CANONICAL_BATCH_ADMISSION_HEADER_BYTES)
                    .and_then(|bytes| bytes.checked_add(packed_bytes as usize))
                    .and_then(|bytes| bytes.checked_add(surface_column_bytes))
                    .and_then(|bytes| bytes.checked_add(natural_tree_bytes))
                    .ok_or_else(|| "canonical batch byte length overflowed usize".to_owned())
            })?;
    let mut encoded = Vec::with_capacity(capacity);
    encoded.extend_from_slice(&CANONICAL_BATCH_MAGIC);
    push_u16(&mut encoded, CANONICAL_BATCH_VERSION);
    push_u16(&mut encoded, 0);
    push_f64(&mut encoded, batch.generation_ms);
    push_f64(&mut encoded, batch.presentation_ms);
    push_f64(&mut encoded, batch.mesh_ms);
    push_f64(&mut encoded, batch.pack_ms);
    push_f64(&mut encoded, batch.transfer_ms);
    push_u32(&mut encoded, batch.deduplicated_target_chunks);
    push_u32(&mut encoded, batch.raw_cache_chunks);
    push_u64(&mut encoded, batch.raw_cache_bytes);
    push_u32(&mut encoded, admission_count);
    push_u32(&mut encoded, 0);
    for admission in &batch.admissions {
        push_i32(&mut encoded, admission.chunk_x);
        push_i32(&mut encoded, admission.chunk_z);
        encoded.push(u8::from(admission.raw_cache_hit));
        encoded.extend_from_slice(&[0; 3]);
        push_u32(&mut encoded, admission.retained_dependency_chunks);
        push_u32(
            &mut encoded,
            admission.packed_sections.len().try_into().map_err(|_| {
                "canonical packed admission exceeds the u32 byte-length contract".to_owned()
            })?,
        );
        push_u32(
            &mut encoded,
            admission
                .natural_trees
                .len()
                .try_into()
                .map_err(|_| "canonical admission has more than u32 natural trees".to_owned())?,
        );
        push_u32(
            &mut encoded,
            admission
                .surface_columns
                .len()
                .try_into()
                .map_err(|_| "canonical admission has more than u32 surface columns".to_owned())?,
        );
        encoded.extend_from_slice(&admission.packed_sections);
        for column in &admission.surface_columns {
            push_i16(&mut encoded, column.solid_top_y);
            encoded.push(column.side_material.unwrap_or_default());
            encoded.push(u8::from(column.water) | (u8::from(column.side_material.is_some()) << 1));
        }
        for tree in &admission.natural_trees {
            encode_tree_occurrence(&mut encoded, tree.occurrence);
            push_u32(
                &mut encoded,
                tree.packed_sections.len().try_into().map_err(|_| {
                    "canonical packed natural tree exceeds the u32 byte-length contract".to_owned()
                })?,
            );
            encoded.extend_from_slice(&tree.packed_sections);
        }
    }
    debug_assert_eq!(encoded.len(), capacity);
    Ok(encoded)
}

pub fn patch_canonical_batch_transfer_ms(
    encoded: &mut [u8],
    transfer_ms: f64,
) -> Result<(), String> {
    if !transfer_ms.is_finite() || transfer_ms < 0.0 {
        return Err("canonical batch transfer time is not finite and non-negative".to_owned());
    }
    let destination = encoded
        .get_mut(TRANSFER_MS_OFFSET..TRANSFER_MS_OFFSET + 8)
        .ok_or_else(|| "canonical batch is too short to patch transfer time".to_owned())?;
    destination.copy_from_slice(&transfer_ms.to_le_bytes());
    Ok(())
}

pub fn decode_canonical_batch(bytes: &[u8]) -> Result<CanonicalEncodedBatch, String> {
    let mut decoder = Decoder::new(bytes);
    if decoder.read_exact(4)? != CANONICAL_BATCH_MAGIC {
        return Err("canonical batch magic does not match MCTB".to_owned());
    }
    let version = decoder.read_u16()?;
    if version != CANONICAL_BATCH_VERSION {
        return Err(format!(
            "unsupported canonical batch version {version}, expected {CANONICAL_BATCH_VERSION}"
        ));
    }
    let _reserved = decoder.read_u16()?;
    let mut batch = CanonicalEncodedBatch {
        generation_ms: decoder.read_f64()?,
        presentation_ms: decoder.read_f64()?,
        mesh_ms: decoder.read_f64()?,
        pack_ms: decoder.read_f64()?,
        transfer_ms: decoder.read_f64()?,
        deduplicated_target_chunks: decoder.read_u32()?,
        raw_cache_chunks: decoder.read_u32()?,
        raw_cache_bytes: decoder.read_u64()?,
        admissions: Vec::new(),
    };
    let admission_count = decoder.read_u32()? as usize;
    let _reserved = decoder.read_u32()?;
    batch.admissions.reserve(admission_count);
    for _ in 0..admission_count {
        let chunk_x = decoder.read_i32()?;
        let chunk_z = decoder.read_i32()?;
        let raw_cache_hit = match decoder.read_u8()? {
            0 => false,
            1 => true,
            other => {
                return Err(format!(
                    "canonical batch raw-cache flag {other} is not 0 or 1"
                ));
            }
        };
        let _reserved = decoder.read_exact(3)?;
        let retained_dependency_chunks = decoder.read_u32()?;
        let packed_byte_length = decoder.read_u32()? as usize;
        let natural_tree_count = decoder.read_u32()? as usize;
        let surface_column_count = decoder.read_u32()? as usize;
        let packed_sections = decoder.read_exact(packed_byte_length)?.to_vec();
        let mut surface_columns = Vec::with_capacity(surface_column_count);
        for _ in 0..surface_column_count {
            let solid_top_y = decoder.read_i16()?;
            let material = decoder.read_u8()?;
            let flags = decoder.read_u8()?;
            if flags & !0b11 != 0 {
                return Err(format!(
                    "canonical surface-column flags {flags:#04x} contain unknown bits"
                ));
            }
            surface_columns.push(CanonicalExactSurfaceColumn {
                solid_top_y,
                side_material: (flags & 0b10 != 0).then_some(material),
                water: flags & 0b01 != 0,
            });
        }
        let mut natural_trees = Vec::with_capacity(natural_tree_count);
        for _ in 0..natural_tree_count {
            let occurrence = decoder.read_tree_occurrence()?;
            let packed_byte_length = decoder.read_u32()? as usize;
            let packed_sections = decoder.read_exact(packed_byte_length)?.to_vec();
            natural_trees.push(CanonicalEncodedNaturalTree {
                occurrence,
                packed_sections,
            });
        }
        batch.admissions.push(CanonicalEncodedAdmission {
            chunk_x,
            chunk_z,
            raw_cache_hit,
            retained_dependency_chunks,
            packed_sections,
            natural_trees,
            surface_columns,
        });
    }
    if decoder.remaining() != 0 {
        return Err(format!(
            "canonical batch has {} trailing bytes",
            decoder.remaining()
        ));
    }
    validate_timings(&batch)?;
    Ok(batch)
}

fn validate_timings(batch: &CanonicalEncodedBatch) -> Result<(), String> {
    for (label, value) in [
        ("generation", batch.generation_ms),
        ("presentation", batch.presentation_ms),
        ("mesh", batch.mesh_ms),
        ("pack", batch.pack_ms),
        ("transfer", batch.transfer_ms),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(format!(
                "canonical batch {label} time is not finite and non-negative"
            ));
        }
    }
    Ok(())
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_i16(bytes: &mut Vec<u8>, value: i16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_i64(bytes: &mut Vec<u8>, value: i64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_f64(bytes: &mut Vec<u8>, value: f64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn encode_tree_occurrence(bytes: &mut Vec<u8>, occurrence: McloneTreeOccurrence) {
    let record = occurrence.record;
    push_i32(bytes, record.id.planning_cell_x);
    push_i32(bytes, record.id.planning_cell_z);
    bytes.push(record.id.candidate_slot);
    push_u16(bytes, record.id.vegetation_revision);
    push_i32(bytes, record.canonical_base.x);
    push_i32(bytes, record.canonical_base.y);
    push_i32(bytes, record.canonical_base.z);
    bytes.push(tree_family_tag(record.family));
    bytes.push(tree_archetype_tag(record.archetype));
    push_u16(bytes, record.trunk_height);
    push_u16(bytes, record.crown_radius);
    push_u16(bytes, record.crown_depth);
    bytes.push(record.orientation);
    bytes.push(record.landmark_rank);
    push_u64(bytes, record.variant_seed);
    encode_tree_bounds(bytes, record.bounds);
    push_i64(bytes, occurrence.x_lift);
    encode_tree_bounds(bytes, occurrence.working_bounds);
}

fn encode_tree_bounds(bytes: &mut Vec<u8>, bounds: McloneTreeBounds) {
    push_i32(bytes, bounds.min_x);
    push_i32(bytes, bounds.min_y);
    push_i32(bytes, bounds.min_z);
    push_i32(bytes, bounds.max_x);
    push_i32(bytes, bounds.max_y);
    push_i32(bytes, bounds.max_z);
}

const fn tree_family_tag(family: McloneTreeFamily) -> u8 {
    match family {
        McloneTreeFamily::TemperateBroadleaf => 0,
        McloneTreeFamily::CoolWetConifer => 1,
        McloneTreeFamily::WarmDryAcacia => 2,
        McloneTreeFamily::HumidJungleBroadleaf => 3,
    }
}

const fn tree_archetype_tag(archetype: McloneTreeArchetype) -> u8 {
    match archetype {
        McloneTreeArchetype::RoundedBroadleaf => 0,
        McloneTreeArchetype::LayeredConifer => 1,
        McloneTreeArchetype::ForkedAcacia => 2,
        McloneTreeArchetype::LayeredJungle => 3,
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.cursor)
    }

    fn read_exact(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or_else(|| "canonical batch cursor overflowed usize".to_owned())?;
        let bytes = self.bytes.get(self.cursor..end).ok_or_else(|| {
            format!(
                "canonical batch ended at byte {} while reading {length} bytes",
                self.cursor
            )
        })?;
        self.cursor = end;
        Ok(bytes)
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.read_exact(2)?.try_into().unwrap()))
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.read_exact(4)?.try_into().unwrap()))
    }

    fn read_i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.read_exact(2)?.try_into().unwrap()))
    }

    fn read_i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.read_exact(4)?.try_into().unwrap()))
    }

    fn read_i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(self.read_exact(8)?.try_into().unwrap()))
    }

    fn read_u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.read_exact(8)?.try_into().unwrap()))
    }

    fn read_f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_le_bytes(self.read_exact(8)?.try_into().unwrap()))
    }

    fn read_tree_occurrence(&mut self) -> Result<McloneTreeOccurrence, String> {
        let id = McloneTreeId {
            planning_cell_x: self.read_i32()?,
            planning_cell_z: self.read_i32()?,
            candidate_slot: self.read_u8()?,
            vegetation_revision: self.read_u16()?,
        };
        let canonical_base = BlockPos::new(self.read_i32()?, self.read_i32()?, self.read_i32()?);
        let family = match self.read_u8()? {
            0 => McloneTreeFamily::TemperateBroadleaf,
            1 => McloneTreeFamily::CoolWetConifer,
            2 => McloneTreeFamily::WarmDryAcacia,
            3 => McloneTreeFamily::HumidJungleBroadleaf,
            other => {
                return Err(format!(
                    "canonical natural-tree family tag {other} is invalid"
                ));
            }
        };
        let archetype = match self.read_u8()? {
            0 => McloneTreeArchetype::RoundedBroadleaf,
            1 => McloneTreeArchetype::LayeredConifer,
            2 => McloneTreeArchetype::ForkedAcacia,
            3 => McloneTreeArchetype::LayeredJungle,
            other => {
                return Err(format!(
                    "canonical natural-tree archetype tag {other} is invalid"
                ));
            }
        };
        let trunk_height = self.read_u16()?;
        let crown_radius = self.read_u16()?;
        let crown_depth = self.read_u16()?;
        let orientation = self.read_u8()?;
        let landmark_rank = self.read_u8()?;
        let variant_seed = self.read_u64()?;
        let bounds = self.read_tree_bounds()?;
        let x_lift = self.read_i64()?;
        let working_bounds = self.read_tree_bounds()?;
        Ok(McloneTreeOccurrence {
            record: McloneTreeRecord {
                id,
                canonical_base,
                family,
                archetype,
                trunk_height,
                crown_radius,
                crown_depth,
                orientation,
                landmark_rank,
                variant_seed,
                bounds,
            },
            x_lift,
            working_bounds,
        })
    }

    fn read_tree_bounds(&mut self) -> Result<McloneTreeBounds, String> {
        McloneTreeBounds::new(
            self.read_i32()?,
            self.read_i32()?,
            self.read_i32()?,
            self.read_i32()?,
            self.read_i32()?,
            self.read_i32()?,
        )
        .map_err(|error| format!("canonical natural-tree bounds are invalid: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use mclone_worldgen::levelgen::{
        McloneTreeArchetype, McloneTreeBounds, McloneTreeFamily, McloneTreeId, McloneTreeRecord,
    };
    use mclone_worldgen::placement::BlockPos;

    use super::*;

    fn tree_fixture() -> CanonicalEncodedNaturalTree {
        let bounds = McloneTreeBounds::new(-20, 63, 19, -15, 72, 24).unwrap();
        CanonicalEncodedNaturalTree {
            occurrence: McloneTreeOccurrence {
                record: McloneTreeRecord {
                    id: McloneTreeId {
                        planning_cell_x: -3,
                        planning_cell_z: 4,
                        candidate_slot: 7,
                        vegetation_revision: 2,
                    },
                    canonical_base: BlockPos::new(-18, 64, 21),
                    family: McloneTreeFamily::CoolWetConifer,
                    archetype: McloneTreeArchetype::LayeredConifer,
                    trunk_height: 7,
                    crown_radius: 3,
                    crown_depth: 6,
                    orientation: 2,
                    landmark_rank: 1,
                    variant_seed: 0xfedc_ba98_7654_3210,
                    bounds,
                },
                x_lift: 1_i64 << 32,
                working_bounds: McloneTreeBounds::new(i32::MAX - 5, 63, 19, i32::MAX, 72, 24)
                    .unwrap(),
            },
            packed_sections: vec![9, 8, 7, 6, 5],
        }
    }

    fn fixture() -> CanonicalEncodedBatch {
        CanonicalEncodedBatch {
            generation_ms: 12.25,
            presentation_ms: 2.5,
            mesh_ms: 8.75,
            pack_ms: 1.125,
            transfer_ms: 0.5,
            deduplicated_target_chunks: 17,
            raw_cache_chunks: 81,
            raw_cache_bytes: 123_456,
            admissions: vec![
                CanonicalEncodedAdmission {
                    chunk_x: -19,
                    chunk_z: 21,
                    raw_cache_hit: false,
                    retained_dependency_chunks: 9,
                    packed_sections: vec![1, 2, 3, 4],
                    natural_trees: vec![tree_fixture()],
                    surface_columns: vec![CanonicalExactSurfaceColumn {
                        solid_top_y: 63,
                        side_material: Some(4),
                        water: false,
                    }],
                },
                CanonicalEncodedAdmission {
                    chunk_x: -18,
                    chunk_z: 21,
                    raw_cache_hit: true,
                    retained_dependency_chunks: 12,
                    packed_sections: vec![5; 257],
                    natural_trees: Vec::new(),
                    surface_columns: vec![CanonicalExactSurfaceColumn {
                        solid_top_y: -12,
                        side_material: None,
                        water: true,
                    }],
                },
            ],
        }
    }

    #[test]
    fn canonical_batch_round_trips_without_domain_loss() {
        let fixture = fixture();
        let encoded = encode_canonical_batch(&fixture).unwrap();
        assert_eq!(decode_canonical_batch(&encoded).unwrap(), fixture);
    }

    #[test]
    fn transfer_time_can_be_patched_after_shared_publication() {
        let mut encoded = encode_canonical_batch(&fixture()).unwrap();
        patch_canonical_batch_transfer_ms(&mut encoded, 7.75).unwrap();
        assert_eq!(decode_canonical_batch(&encoded).unwrap().transfer_ms, 7.75);
    }

    #[test]
    fn malformed_batches_are_rejected_before_admission() {
        let encoded = encode_canonical_batch(&fixture()).unwrap();
        assert!(decode_canonical_batch(&encoded[..encoded.len() - 1]).is_err());
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] = b'X';
        assert!(decode_canonical_batch(&wrong_magic).is_err());
        let mut trailing = encoded;
        trailing.push(0);
        assert!(decode_canonical_batch(&trailing).is_err());
    }
}
