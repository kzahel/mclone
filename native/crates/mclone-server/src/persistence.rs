use std::fmt;
use std::io;

#[cfg(any(test, not(target_arch = "wasm32")))]
use std::io::{Read, Write};

#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use mclone_core::{ChunkPos, ChunkSnapshot};

#[cfg(any(test, not(target_arch = "wasm32")))]
use mclone_core::{
    BlockStateId, ChunkRevision, ChunkStatus, LIGHT_DATA_LAYER_BYTE_COUNT, PackedChunkSection,
    PackedLightSection,
};

#[cfg(any(test, not(target_arch = "wasm32")))]
const SNAPSHOT_MAGIC: &[u8; 12] = b"MCLONESNAP\0\0";
#[cfg(any(test, not(target_arch = "wasm32")))]
const SNAPSHOT_FORMAT_VERSION: u32 = 2;

pub type ChunkStoreResult<T> = Result<T, ChunkStoreError>;

#[derive(Debug)]
pub enum ChunkStoreError {
    Io(io::Error),
    InvalidData(String),
}

impl fmt::Display for ChunkStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::InvalidData(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ChunkStoreError {}

impl From<io::Error> for ChunkStoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub trait ChunkSnapshotStore: fmt::Debug {
    fn load_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkSnapshot>>;
    fn save_chunk(&mut self, snapshot: &ChunkSnapshot) -> ChunkStoreResult<()>;
}

#[derive(Debug, Default)]
pub struct NullChunkSnapshotStore;

impl ChunkSnapshotStore for NullChunkSnapshotStore {
    fn load_chunk(&mut self, _pos: ChunkPos) -> ChunkStoreResult<Option<ChunkSnapshot>> {
        Ok(None)
    }

    fn save_chunk(&mut self, _snapshot: &ChunkSnapshot) -> ChunkStoreResult<()> {
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct FilesystemChunkSnapshotStore {
    root: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FilesystemChunkSnapshotStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn chunk_path(&self, pos: ChunkPos) -> PathBuf {
        self.root.join(format!("c.{}.{}.mcsnap", pos.x, pos.z))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ChunkSnapshotStore for FilesystemChunkSnapshotStore {
    fn load_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkSnapshot>> {
        let path = self.chunk_path(pos);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut reader = BufReader::new(file);
        let snapshot = read_snapshot(&mut reader)?;
        if snapshot.pos != pos {
            return Err(ChunkStoreError::InvalidData(format!(
                "chunk snapshot file {} contained position {:?}, expected {:?}",
                path.display(),
                snapshot.pos,
                pos
            )));
        }
        Ok(Some(snapshot))
    }

    fn save_chunk(&mut self, snapshot: &ChunkSnapshot) -> ChunkStoreResult<()> {
        fs::create_dir_all(&self.root)?;
        let path = self.chunk_path(snapshot.pos);
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        write_snapshot(&mut writer, snapshot)
    }
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_snapshot(writer: &mut impl Write, snapshot: &ChunkSnapshot) -> ChunkStoreResult<()> {
    writer.write_all(SNAPSHOT_MAGIC)?;
    write_u32(writer, SNAPSHOT_FORMAT_VERSION)?;
    write_i32(writer, snapshot.pos.x)?;
    write_i32(writer, snapshot.pos.z)?;
    write_u8(writer, status_to_u8(snapshot.status))?;
    write_u64(writer, snapshot.revision.0)?;
    write_i32(writer, snapshot.min_y)?;
    write_i32(writer, snapshot.height)?;
    write_len(writer, snapshot.sections.len(), "section count")?;
    for section in &snapshot.sections {
        write_section(writer, section)?;
    }
    write_bool(writer, snapshot.light_correct)?;
    write_len(writer, snapshot.light_sections.len(), "light section count")?;
    for section in &snapshot.light_sections {
        write_light_section(writer, section)?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_snapshot(reader: &mut impl Read) -> ChunkStoreResult<ChunkSnapshot> {
    let mut magic = [0_u8; SNAPSHOT_MAGIC.len()];
    reader.read_exact(&mut magic)?;
    if &magic != SNAPSHOT_MAGIC {
        return Err(ChunkStoreError::InvalidData(
            "chunk snapshot had invalid magic".to_owned(),
        ));
    }

    let version = read_u32(reader)?;
    if !(1..=SNAPSHOT_FORMAT_VERSION).contains(&version) {
        return Err(ChunkStoreError::InvalidData(format!(
            "unsupported chunk snapshot format version {version}"
        )));
    }

    let pos = ChunkPos::new(read_i32(reader)?, read_i32(reader)?);
    let status = status_from_u8(read_u8(reader)?)?;
    let revision = ChunkRevision(read_u64(reader)?);
    let min_y = read_i32(reader)?;
    let height = read_i32(reader)?;
    let section_count = read_len(reader)?;
    let mut sections = Vec::with_capacity(section_count);
    for _ in 0..section_count {
        sections.push(read_section(reader)?);
    }
    let (light_correct, light_sections) = if version >= 2 {
        let light_correct = read_bool(reader)?;
        let light_section_count = read_len(reader)?;
        let mut light_sections = Vec::with_capacity(light_section_count);
        for _ in 0..light_section_count {
            light_sections.push(read_light_section(reader)?);
        }
        (light_correct, light_sections)
    } else {
        (false, Vec::new())
    };

    Ok(ChunkSnapshot {
        pos,
        status,
        revision,
        min_y,
        height,
        sections,
        light_correct,
        light_sections,
    })
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_section(writer: &mut impl Write, section: &PackedChunkSection) -> ChunkStoreResult<()> {
    write_i32(writer, section.section_y)?;
    write_len(writer, section.palette_state_ids.len(), "palette length")?;
    for state_id in &section.palette_state_ids {
        write_u32(writer, state_id.0)?;
    }
    write_u8(writer, section.bits_per_block)?;
    write_len(
        writer,
        section.packed_block_indices.len(),
        "packed block index length",
    )?;
    for word in &section.packed_block_indices {
        write_u64(writer, *word)?;
    }
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_section(reader: &mut impl Read) -> ChunkStoreResult<PackedChunkSection> {
    let section_y = read_i32(reader)?;
    let palette_len = read_len(reader)?;
    let mut palette_state_ids = Vec::with_capacity(palette_len);
    for _ in 0..palette_len {
        palette_state_ids.push(BlockStateId(read_u32(reader)?));
    }
    let bits_per_block = read_u8(reader)?;
    let packed_len = read_len(reader)?;
    let mut packed_block_indices = Vec::with_capacity(packed_len);
    for _ in 0..packed_len {
        packed_block_indices.push(read_u64(reader)?);
    }
    Ok(PackedChunkSection {
        section_y,
        palette_state_ids,
        bits_per_block,
        packed_block_indices,
    })
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_light_section(
    writer: &mut impl Write,
    section: &PackedLightSection,
) -> ChunkStoreResult<()> {
    write_i32(writer, section.section_y)?;
    write_optional_light_layer(writer, &section.sky, "sky light layer")?;
    write_optional_light_layer(writer, &section.block, "block light layer")?;
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_light_section(reader: &mut impl Read) -> ChunkStoreResult<PackedLightSection> {
    let section_y = read_i32(reader)?;
    let sky = read_optional_light_layer(reader)?;
    let block = read_optional_light_layer(reader)?;
    Ok(PackedLightSection::new(section_y, sky, block))
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_optional_light_layer(
    writer: &mut impl Write,
    layer: &Option<Vec<u8>>,
    name: &str,
) -> ChunkStoreResult<()> {
    match layer {
        Some(bytes) => {
            if bytes.len() != LIGHT_DATA_LAYER_BYTE_COUNT {
                return Err(ChunkStoreError::InvalidData(format!(
                    "{name} has {} bytes; expected {LIGHT_DATA_LAYER_BYTE_COUNT}",
                    bytes.len()
                )));
            }
            write_bool(writer, true)?;
            writer.write_all(bytes)?;
        }
        None => write_bool(writer, false)?,
    }
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_optional_light_layer(reader: &mut impl Read) -> ChunkStoreResult<Option<Vec<u8>>> {
    if !read_bool(reader)? {
        return Ok(None);
    }
    let mut bytes = vec![0; LIGHT_DATA_LAYER_BYTE_COUNT];
    reader.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_len(writer: &mut impl Write, value: usize, name: &str) -> ChunkStoreResult<()> {
    let value = u32::try_from(value)
        .map_err(|_| ChunkStoreError::InvalidData(format!("{name} {value} does not fit in u32")))?;
    write_u32(writer, value)
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_len(reader: &mut impl Read) -> ChunkStoreResult<usize> {
    usize::try_from(read_u32(reader)?).map_err(|_| {
        ChunkStoreError::InvalidData("chunk snapshot length does not fit in usize".to_owned())
    })
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn status_to_u8(status: ChunkStatus) -> u8 {
    match status {
        ChunkStatus::Terrain => 0,
        ChunkStatus::Surface => 1,
        ChunkStatus::Features => 2,
        ChunkStatus::Light => 3,
        ChunkStatus::Full => 4,
    }
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn status_from_u8(value: u8) -> ChunkStoreResult<ChunkStatus> {
    match value {
        0 => Ok(ChunkStatus::Terrain),
        1 => Ok(ChunkStatus::Surface),
        2 => Ok(ChunkStatus::Features),
        3 => Ok(ChunkStatus::Light),
        4 => Ok(ChunkStatus::Full),
        _ => Err(ChunkStoreError::InvalidData(format!(
            "unknown chunk status id {value}"
        ))),
    }
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_u8(writer: &mut impl Write, value: u8) -> ChunkStoreResult<()> {
    writer.write_all(&[value])?;
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_u8(reader: &mut impl Read) -> ChunkStoreResult<u8> {
    let mut bytes = [0_u8; 1];
    reader.read_exact(&mut bytes)?;
    Ok(bytes[0])
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_bool(writer: &mut impl Write, value: bool) -> ChunkStoreResult<()> {
    write_u8(writer, u8::from(value))
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_bool(reader: &mut impl Read) -> ChunkStoreResult<bool> {
    match read_u8(reader)? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(ChunkStoreError::InvalidData(format!(
            "boolean value must be 0 or 1, got {value}"
        ))),
    }
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_u32(writer: &mut impl Write, value: u32) -> ChunkStoreResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_u32(reader: &mut impl Read) -> ChunkStoreResult<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_i32(writer: &mut impl Write, value: i32) -> ChunkStoreResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_i32(reader: &mut impl Read) -> ChunkStoreResult<i32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(i32::from_le_bytes(bytes))
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn write_u64(writer: &mut impl Write, value: u64) -> ChunkStoreResult<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn read_u64(reader: &mut impl Read) -> ChunkStoreResult<u64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, LIGHT_DATA_LAYER_BYTE_COUNT};

    #[test]
    fn binary_snapshot_format_roundtrips_sections() {
        let mut block_state_ids = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        block_state_ids[7] = BlockStateId(3);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(-2, 5),
            ChunkStatus::Surface,
            ChunkRevision(42),
            0,
            16,
            &block_state_ids,
        )
        .with_light_sections(
            false,
            vec![
                PackedLightSection::new(0, Some(vec![0xFF; LIGHT_DATA_LAYER_BYTE_COUNT]), None),
                PackedLightSection::new(
                    1,
                    Some(vec![0x22; LIGHT_DATA_LAYER_BYTE_COUNT]),
                    Some(vec![0x33; LIGHT_DATA_LAYER_BYTE_COUNT]),
                ),
            ],
        );
        let mut bytes = Vec::new();

        write_snapshot(&mut bytes, &snapshot).unwrap();
        let decoded = read_snapshot(&mut bytes.as_slice()).unwrap();

        assert_eq!(decoded, snapshot);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn filesystem_chunk_store_roundtrips_snapshot() {
        let root = unique_temp_dir("filesystem_chunk_store_roundtrips_snapshot");
        let mut store = FilesystemChunkSnapshotStore::new(&root);
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(3, -4),
            ChunkStatus::Surface,
            ChunkRevision(9),
            0,
            16,
            &vec![BlockStateId(1); CHUNK_SECTION_VOLUME],
        );

        store.save_chunk(&snapshot).unwrap();
        assert_eq!(
            store.load_chunk(ChunkPos::new(3, -4)).unwrap(),
            Some(snapshot)
        );
        assert_eq!(store.load_chunk(ChunkPos::new(0, 0)).unwrap(), None);

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn unique_temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("mclone-{name}-{}-{nanos}", std::process::id()));
        path
    }
}
