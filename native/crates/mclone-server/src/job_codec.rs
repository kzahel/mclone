#![allow(dead_code)]

use std::collections::BTreeMap;

use mclone_core::{ChunkPos, ChunkSnapshot, PackedLightSection};
use mclone_protocol::{ServerUpdate, decode_server_update, encode_server_update};
use mclone_worldgen::feature::{DecorationStep, FeatureDecorationTiming};
use mclone_worldgen::levelgen::{
    GeneratedChunk, MutableChunkBlockBuffer, OverworldDependencyGenerationTiming,
    OverworldFeatureBatchTiming, OverworldFeatureDependencyCache,
    OverworldFeatureDependencyCacheReport, ScheduledTick, SurfaceFillTiming,
};

use crate::ChunkJobId;
use crate::level_light_bridge::LevelLightComputationTiming;
use crate::light_mailbox::CompletedLightStatus;
use crate::light_status::{PendingLightStatus, PendingLightStatusBatch};
use crate::light_world::RetainedInitialLightState;

const WORLDGEN_REQUEST_MAGIC: u32 = 0x5747_4A52;
const WORLDGEN_RESPONSE_MAGIC: u32 = 0x5747_4A53;
const LIGHT_REQUEST_MAGIC: u32 = 0x4C54_4A52;
const LIGHT_RESPONSE_MAGIC: u32 = 0x4C54_4A53;
const JOB_FRAME_VERSION: u32 = 1;

pub(crate) fn encode_worldgen_request(
    job_id: ChunkJobId,
    seed: i64,
    targets: &[ChunkPos],
    dependencies: Vec<MutableChunkBlockBuffer>,
) -> Result<Vec<u8>, String> {
    let mut writer = FrameWriter::new(WORLDGEN_REQUEST_MAGIC);
    writer.write_u64(job_id.0);
    writer.write_i64(seed);
    writer.write_len("worldgen targets", targets.len())?;
    for target in targets {
        writer.write_chunk_pos(*target);
    }
    writer.write_len("worldgen dependencies", dependencies.len())?;
    for dependency in &dependencies {
        writer.write_mutable_chunk(dependency)?;
    }
    Ok(writer.into_bytes())
}

pub(crate) fn decode_worldgen_response(bytes: &[u8]) -> Result<WorldgenJobFrame, String> {
    let mut reader = FrameReader::new(bytes, WORLDGEN_RESPONSE_MAGIC)?;
    let job_id = ChunkJobId(reader.read_u64()?);
    let generated_chunks = reader.read_map("generated chunks", |reader| {
        let pos = reader.read_chunk_pos()?;
        let chunk = reader.read_generated_chunk()?;
        Ok((pos, chunk))
    })?;
    let retained_dependencies = reader.read_map("retained dependencies", |reader| {
        let pos = reader.read_chunk_pos()?;
        let chunk = reader.read_mutable_chunk()?;
        Ok((pos, chunk))
    })?;
    let cache_report = reader.read_feature_cache_report()?;
    let timing = reader.read_feature_batch_timing()?;
    reader.finish()?;
    Ok(WorldgenJobFrame {
        job_id,
        generated_chunks,
        retained_dependencies,
        cache_report,
        timing,
    })
}

pub fn compute_worldgen_job_frame(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut reader = FrameReader::new(bytes, WORLDGEN_REQUEST_MAGIC)?;
    let job_id = ChunkJobId(reader.read_u64()?);
    let seed = reader.read_i64()?;
    let targets = reader.read_vec("worldgen targets", FrameReader::read_chunk_pos)?;
    let dependencies = reader.read_vec("worldgen dependencies", FrameReader::read_mutable_chunk)?;
    reader.finish()?;

    let mut dependency_cache = OverworldFeatureDependencyCache::new();
    let result =
        dependency_cache.generate_features_chunks_with_dependencies(seed, targets, dependencies);

    let mut writer = FrameWriter::new(WORLDGEN_RESPONSE_MAGIC);
    writer.write_u64(job_id.0);
    writer.write_len("generated chunks", result.chunks.len())?;
    for (pos, chunk) in &result.chunks {
        writer.write_chunk_pos(*pos);
        writer.write_generated_chunk(chunk)?;
    }
    writer.write_len("retained dependencies", result.retained_dependencies.len())?;
    for (pos, dependency) in &result.retained_dependencies {
        writer.write_chunk_pos(*pos);
        writer.write_mutable_chunk(dependency)?;
    }
    writer.write_feature_cache_report(result.cache_report);
    writer.write_feature_batch_timing(result.timing)?;
    Ok(writer.into_bytes())
}

pub(crate) fn encode_light_status_request(
    batch: PendingLightStatusBatch,
) -> Result<Vec<u8>, String> {
    let mut writer = FrameWriter::new(LIGHT_REQUEST_MAGIC);
    let statuses = batch.into_statuses();
    writer.write_len("light statuses", statuses.len())?;
    for status in &statuses {
        writer.write_pending_light_status(status)?;
    }
    Ok(writer.into_bytes())
}

pub(crate) fn decode_light_status_response(
    bytes: &[u8],
) -> Result<Vec<CompletedLightStatus>, String> {
    let mut reader = FrameReader::new(bytes, LIGHT_RESPONSE_MAGIC)?;
    let completed = reader.read_vec(
        "completed light statuses",
        FrameReader::read_completed_light_status,
    )?;
    reader.finish()?;
    Ok(completed)
}

pub fn compute_light_status_job_frame(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut reader = FrameReader::new(bytes, LIGHT_REQUEST_MAGIC)?;
    let statuses = reader.read_vec("light statuses", FrameReader::read_pending_light_status)?;
    reader.finish()?;

    let mut light_state = RetainedInitialLightState::new();
    let completed =
        CompletedLightStatus::from_batch(&mut light_state, PendingLightStatusBatch::new(statuses));

    let mut writer = FrameWriter::new(LIGHT_RESPONSE_MAGIC);
    writer.write_len("completed light statuses", completed.len())?;
    for status in &completed {
        writer.write_completed_light_status(status)?;
    }
    Ok(writer.into_bytes())
}

pub(crate) struct WorldgenJobFrame {
    pub(crate) job_id: ChunkJobId,
    pub(crate) generated_chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    pub(crate) retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    pub(crate) cache_report: OverworldFeatureDependencyCacheReport,
    pub(crate) timing: OverworldFeatureBatchTiming,
}

struct FrameWriter {
    bytes: Vec<u8>,
}

impl FrameWriter {
    fn new(magic: u32) -> Self {
        let mut writer = Self { bytes: Vec::new() };
        writer.write_u32(magic);
        writer.write_u32(JOB_FRAME_VERSION);
        writer
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn write_bool(&mut self, value: bool) {
        self.write_u8(u8::from(value));
    }

    fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn write_i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u128(&mut self, value: u128) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_len(&mut self, field: &str, len: usize) -> Result<(), String> {
        let len = u32::try_from(len).map_err(|_| format!("{field} length {len} exceeds u32"))?;
        self.write_u32(len);
        Ok(())
    }

    fn write_bytes(&mut self, field: &str, bytes: &[u8]) -> Result<(), String> {
        self.write_len(field, bytes.len())?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn write_string(&mut self, field: &str, value: &str) -> Result<(), String> {
        self.write_bytes(field, value.as_bytes())
    }

    fn write_chunk_pos(&mut self, pos: ChunkPos) {
        self.write_i32(pos.x);
        self.write_i32(pos.z);
    }

    fn write_tick(&mut self, tick: &ScheduledTick) -> Result<(), String> {
        self.write_i32(tick.x);
        self.write_i32(tick.y);
        self.write_i32(tick.z);
        self.write_string("scheduled tick target", &tick.target)?;
        self.write_i32(tick.delay);
        Ok(())
    }

    fn write_ticks(&mut self, field: &str, ticks: &[ScheduledTick]) -> Result<(), String> {
        self.write_len(field, ticks.len())?;
        for tick in ticks {
            self.write_tick(tick)?;
        }
        Ok(())
    }

    fn write_generated_chunk(&mut self, chunk: &GeneratedChunk) -> Result<(), String> {
        self.write_i32(chunk.chunk_x);
        self.write_i32(chunk.chunk_z);
        self.write_i32(chunk.min_y);
        self.write_i32(chunk.height);
        self.write_bytes("generated chunk blocks", chunk.blocks())?;
        self.write_ticks("generated chunk block ticks", chunk.block_ticks())?;
        self.write_ticks("generated chunk liquid ticks", chunk.liquid_ticks())?;
        Ok(())
    }

    fn write_mutable_chunk(&mut self, chunk: &MutableChunkBlockBuffer) -> Result<(), String> {
        self.write_i32(chunk.chunk_x);
        self.write_i32(chunk.chunk_z);
        self.write_i32(chunk.min_y);
        self.write_i32(chunk.height);
        self.write_bytes("mutable chunk blocks", &chunk.blocks)?;
        self.write_bool(chunk.has_primed_worldgen_heightmaps());
        let glow_faces = chunk.glow_lichen_faces().collect::<Vec<_>>();
        self.write_len("glow lichen faces", glow_faces.len())?;
        for (index, faces) in glow_faces {
            let index = u32::try_from(index)
                .map_err(|_| format!("glow lichen face index {index} exceeds u32"))?;
            self.write_u32(index);
            self.write_u8(faces);
        }
        self.write_ticks("mutable chunk block ticks", chunk.block_ticks())?;
        self.write_ticks("mutable chunk liquid ticks", chunk.liquid_ticks())?;
        Ok(())
    }

    fn write_snapshot(&mut self, snapshot: &ChunkSnapshot) -> Result<(), String> {
        let frame = encode_server_update(&ServerUpdate::ChunkSnapshot(snapshot.clone()))
            .map_err(|error| error.to_string())?;
        self.write_bytes("chunk snapshot", &frame)
    }

    fn write_light_section(&mut self, section: &PackedLightSection) -> Result<(), String> {
        self.write_i32(section.section_y);
        self.write_optional_light_layer("sky light layer", section.sky.as_deref())?;
        self.write_optional_light_layer("block light layer", section.block.as_deref())?;
        Ok(())
    }

    fn write_optional_light_layer(
        &mut self,
        field: &str,
        layer: Option<&[u8]>,
    ) -> Result<(), String> {
        self.write_bool(layer.is_some());
        if let Some(layer) = layer {
            self.write_bytes(field, layer)?;
        }
        Ok(())
    }

    fn write_pending_light_status(&mut self, status: &PendingLightStatus) -> Result<(), String> {
        self.write_chunk_pos(status.pos);
        self.write_snapshot(&status.feature_snapshot)?;
        self.write_bytes("light status raw blocks", status.raw_blocks())?;
        self.write_len(
            "light status neighbor blocks",
            status.neighbor_blocks().len(),
        )?;
        for (pos, blocks) in status.neighbor_blocks() {
            self.write_chunk_pos(*pos);
            self.write_bytes("light status neighbor block data", blocks)?;
        }
        Ok(())
    }

    fn write_completed_light_status(
        &mut self,
        status: &CompletedLightStatus,
    ) -> Result<(), String> {
        self.write_chunk_pos(status.pos);
        self.write_snapshot(&status.feature_snapshot)?;
        self.write_len("completed light sections", status.light_sections.len())?;
        for section in &status.light_sections {
            self.write_light_section(section)?;
        }
        self.write_bool(status.batch_compute_leader);
        self.write_u128(status.compute_us);
        self.write_level_light_timing(status.timing);
        Ok(())
    }

    fn write_feature_cache_report(&mut self, report: OverworldFeatureDependencyCacheReport) {
        self.write_u64(report.requested_dependency_chunks as u64);
        self.write_u64(report.cache_hits as u64);
        self.write_u64(report.generated_dependency_chunks as u64);
        self.write_u64(report.retained_dependency_chunks as u64);
    }

    fn write_surface_fill_timing(&mut self, timing: SurfaceFillTiming) {
        self.write_u128(timing.chunk_alloc_us);
        self.write_u128(timing.noise_columns_us);
        self.write_u128(timing.terrain_fill_us);
        self.write_u64(timing.non_air_blocks_written as u64);
    }

    fn write_dependency_timing(&mut self, timing: OverworldDependencyGenerationTiming) {
        self.write_u128(timing.generator_setup_us);
        self.write_u128(timing.surface_fill_us);
        self.write_surface_fill_timing(timing.surface_fill);
        self.write_u128(timing.surface_bedrock_us);
        self.write_u128(timing.air_carvers_us);
        self.write_u128(timing.liquid_carvers_us);
        self.write_u128(timing.heightmap_prime_us);
    }

    fn write_feature_batch_timing(
        &mut self,
        timing: OverworldFeatureBatchTiming,
    ) -> Result<(), String> {
        self.write_u128(timing.seed_dependency_insert_us);
        self.write_u128(timing.plan_us);
        self.write_u128(timing.dependency_cache_hit_clone_us);
        self.write_u128(timing.dependency_generate_us);
        self.write_dependency_timing(timing.dependency_generation);
        self.write_u128(timing.dependency_insert_clone_us);
        self.write_u128(timing.dependency_retain_us);
        self.write_u128(timing.retained_dependency_clone_us);
        self.write_u128(timing.feature_region_init_us);
        self.write_u128(timing.feature_decoration_us);
        self.write_len(
            "feature decoration timing steps",
            timing.feature_decoration_steps.step_us.len(),
        )?;
        for step_us in timing.feature_decoration_steps.step_us {
            self.write_u128(step_us);
        }
        self.write_u128(timing.target_extract_us);
        Ok(())
    }

    fn write_level_light_timing(&mut self, timing: LevelLightComputationTiming) {
        self.write_u128(timing.total_us);
        self.write_u128(timing.world_init_us);
        self.write_u128(timing.active_sections_us);
        self.write_u128(timing.sky_source_scan_us);
        self.write_u128(timing.block_source_scan_us);
        self.write_u128(timing.engine_init_us);
        self.write_u128(timing.section_setup_us);
        self.write_u128(timing.sky_source_enqueue_us);
        self.write_u128(timing.block_source_enqueue_us);
        self.write_u128(timing.run_updates_us);
        self.write_u64(timing.run_update_iterations as u64);
        self.write_u64(timing.block_run_update_calls as u64);
        self.write_u64(timing.sky_run_update_calls as u64);
        self.write_u64(timing.block_run_update_processed_nodes as u64);
        self.write_u64(timing.sky_run_update_processed_nodes as u64);
        self.write_u64(timing.max_block_run_update_queue_before as u64);
        self.write_u64(timing.max_sky_run_update_queue_before as u64);
        self.write_u64(timing.final_block_run_update_queue_after as u64);
        self.write_u64(timing.final_sky_run_update_queue_after as u64);
        self.write_u128(timing.block_run_updates_us);
        self.write_u128(timing.sky_run_updates_us);
        self.write_u128(timing.collect_sections_us);
    }
}

struct FrameReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> FrameReader<'a> {
    fn new(bytes: &'a [u8], expected_magic: u32) -> Result<Self, String> {
        let mut reader = Self { bytes, offset: 0 };
        let magic = reader.read_u32()?;
        if magic != expected_magic {
            return Err(format!(
                "unexpected job frame magic {magic:#010x}; expected {expected_magic:#010x}"
            ));
        }
        let version = reader.read_u32()?;
        if version != JOB_FRAME_VERSION {
            return Err(format!("unsupported job frame version {version}"));
        }
        Ok(reader)
    }

    fn finish(&self) -> Result<(), String> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(format!(
                "job frame had {} trailing bytes",
                self.bytes.len() - self.offset
            ))
        }
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| "job frame offset overflow".to_owned())?;
        if end > self.bytes.len() {
            return Err(format!(
                "job frame ended while reading {len} bytes at offset {}",
                self.offset
            ));
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn read_bool(&mut self) -> Result<bool, String> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(format!("invalid bool byte {value}")),
        }
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_i32(&mut self) -> Result<i32, String> {
        let mut bytes = [0; 4];
        bytes.copy_from_slice(self.read_exact(4)?);
        Ok(i32::from_le_bytes(bytes))
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        let mut bytes = [0; 4];
        bytes.copy_from_slice(self.read_exact(4)?);
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_i64(&mut self) -> Result<i64, String> {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(self.read_exact(8)?);
        Ok(i64::from_le_bytes(bytes))
    }

    fn read_u64(&mut self) -> Result<u64, String> {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(self.read_exact(8)?);
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_u128(&mut self) -> Result<u128, String> {
        let mut bytes = [0; 16];
        bytes.copy_from_slice(self.read_exact(16)?);
        Ok(u128::from_le_bytes(bytes))
    }

    fn read_len(&mut self, field: &str) -> Result<usize, String> {
        usize::try_from(self.read_u32()?).map_err(|_| format!("{field} length exceeds usize"))
    }

    fn read_bytes(&mut self, field: &str) -> Result<Vec<u8>, String> {
        let len = self.read_len(field)?;
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_string(&mut self, field: &str) -> Result<String, String> {
        String::from_utf8(self.read_bytes(field)?)
            .map_err(|error| format!("{field} was not valid UTF-8: {error}"))
    }

    fn read_vec<T>(
        &mut self,
        field: &str,
        mut read_item: impl FnMut(&mut Self) -> Result<T, String>,
    ) -> Result<Vec<T>, String> {
        let len = self.read_len(field)?;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(read_item(self)?);
        }
        Ok(out)
    }

    fn read_map<K: Ord, V>(
        &mut self,
        field: &str,
        mut read_item: impl FnMut(&mut Self) -> Result<(K, V), String>,
    ) -> Result<BTreeMap<K, V>, String> {
        let len = self.read_len(field)?;
        let mut out = BTreeMap::new();
        for _ in 0..len {
            let (key, value) = read_item(self)?;
            out.insert(key, value);
        }
        Ok(out)
    }

    fn read_chunk_pos(&mut self) -> Result<ChunkPos, String> {
        Ok(ChunkPos::new(self.read_i32()?, self.read_i32()?))
    }

    fn read_tick(&mut self) -> Result<ScheduledTick, String> {
        let x = self.read_i32()?;
        let y = self.read_i32()?;
        let z = self.read_i32()?;
        let target = self.read_string("scheduled tick target")?;
        let delay = self.read_i32()?;
        Ok(ScheduledTick::new(x, y, z, target, delay))
    }

    fn read_ticks(&mut self, field: &str) -> Result<Vec<ScheduledTick>, String> {
        self.read_vec(field, FrameReader::read_tick)
    }

    fn read_generated_chunk(&mut self) -> Result<GeneratedChunk, String> {
        let chunk_x = self.read_i32()?;
        let chunk_z = self.read_i32()?;
        let min_y = self.read_i32()?;
        let height = self.read_i32()?;
        let blocks = self.read_bytes("generated chunk blocks")?;
        let block_ticks = self.read_ticks("generated chunk block ticks")?;
        let liquid_ticks = self.read_ticks("generated chunk liquid ticks")?;
        Ok(GeneratedChunk::from_raw_parts_with_ticks(
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks,
            block_ticks,
            liquid_ticks,
        ))
    }

    fn read_mutable_chunk(&mut self) -> Result<MutableChunkBlockBuffer, String> {
        let chunk_x = self.read_i32()?;
        let chunk_z = self.read_i32()?;
        let min_y = self.read_i32()?;
        let height = self.read_i32()?;
        let blocks = self.read_bytes("mutable chunk blocks")?;
        let prime_worldgen_heightmaps = self.read_bool()?;
        let glow_lichen_faces = self.read_map("glow lichen faces", |reader| {
            let index = reader.read_u32()? as usize;
            let faces = reader.read_u8()?;
            Ok((index, faces))
        })?;
        let block_ticks = self.read_ticks("mutable chunk block ticks")?;
        let liquid_ticks = self.read_ticks("mutable chunk liquid ticks")?;
        Ok(MutableChunkBlockBuffer::from_raw_parts_with_metadata(
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks,
            glow_lichen_faces,
            block_ticks,
            liquid_ticks,
            prime_worldgen_heightmaps,
        ))
    }

    fn read_snapshot(&mut self) -> Result<ChunkSnapshot, String> {
        let frame = self.read_bytes("chunk snapshot")?;
        match decode_server_update(&frame).map_err(|error| error.to_string())? {
            ServerUpdate::ChunkSnapshot(snapshot) => Ok(snapshot),
            update => Err(format!(
                "expected chunk snapshot frame, decoded update {update:?}"
            )),
        }
    }

    fn read_light_section(&mut self) -> Result<PackedLightSection, String> {
        let section_y = self.read_i32()?;
        let sky = self.read_optional_light_layer("sky light layer")?;
        let block = self.read_optional_light_layer("block light layer")?;
        Ok(PackedLightSection::new(section_y, sky, block))
    }

    fn read_optional_light_layer(&mut self, field: &str) -> Result<Option<Vec<u8>>, String> {
        if self.read_bool()? {
            Ok(Some(self.read_bytes(field)?))
        } else {
            Ok(None)
        }
    }

    fn read_pending_light_status(&mut self) -> Result<PendingLightStatus, String> {
        let pos = self.read_chunk_pos()?;
        let feature_snapshot = self.read_snapshot()?;
        let raw_blocks = self.read_bytes("light status raw blocks")?;
        let neighbor_blocks = self.read_vec("light status neighbor blocks", |reader| {
            let pos = reader.read_chunk_pos()?;
            let blocks = reader.read_bytes("light status neighbor block data")?;
            Ok((pos, blocks))
        })?;
        Ok(PendingLightStatus::from_parts(
            pos,
            feature_snapshot,
            raw_blocks,
            neighbor_blocks,
        ))
    }

    fn read_completed_light_status(&mut self) -> Result<CompletedLightStatus, String> {
        let pos = self.read_chunk_pos()?;
        let feature_snapshot = self.read_snapshot()?;
        let light_sections =
            self.read_vec("completed light sections", FrameReader::read_light_section)?;
        let batch_compute_leader = self.read_bool()?;
        let compute_us = self.read_u128()?;
        let timing = self.read_level_light_timing()?;
        Ok(CompletedLightStatus {
            pos,
            feature_snapshot,
            light_sections,
            batch_compute_leader,
            compute_us,
            timing,
        })
    }

    fn read_feature_cache_report(
        &mut self,
    ) -> Result<OverworldFeatureDependencyCacheReport, String> {
        Ok(OverworldFeatureDependencyCacheReport {
            requested_dependency_chunks: self.read_u64()? as usize,
            cache_hits: self.read_u64()? as usize,
            generated_dependency_chunks: self.read_u64()? as usize,
            retained_dependency_chunks: self.read_u64()? as usize,
        })
    }

    fn read_surface_fill_timing(&mut self) -> Result<SurfaceFillTiming, String> {
        Ok(SurfaceFillTiming {
            chunk_alloc_us: self.read_u128()?,
            noise_columns_us: self.read_u128()?,
            terrain_fill_us: self.read_u128()?,
            non_air_blocks_written: self.read_u64()? as usize,
        })
    }

    fn read_dependency_timing(&mut self) -> Result<OverworldDependencyGenerationTiming, String> {
        Ok(OverworldDependencyGenerationTiming {
            generator_setup_us: self.read_u128()?,
            surface_fill_us: self.read_u128()?,
            surface_fill: self.read_surface_fill_timing()?,
            surface_bedrock_us: self.read_u128()?,
            air_carvers_us: self.read_u128()?,
            liquid_carvers_us: self.read_u128()?,
            heightmap_prime_us: self.read_u128()?,
        })
    }

    fn read_feature_batch_timing(&mut self) -> Result<OverworldFeatureBatchTiming, String> {
        let seed_dependency_insert_us = self.read_u128()?;
        let plan_us = self.read_u128()?;
        let dependency_cache_hit_clone_us = self.read_u128()?;
        let dependency_generate_us = self.read_u128()?;
        let dependency_generation = self.read_dependency_timing()?;
        let dependency_insert_clone_us = self.read_u128()?;
        let dependency_retain_us = self.read_u128()?;
        let retained_dependency_clone_us = self.read_u128()?;
        let feature_region_init_us = self.read_u128()?;
        let feature_decoration_us = self.read_u128()?;
        let step_count = self.read_len("feature decoration timing steps")?;
        if step_count != DecorationStep::COUNT {
            return Err(format!(
                "feature decoration timing had {step_count} steps; expected {}",
                DecorationStep::COUNT
            ));
        }
        let mut step_us = [0; DecorationStep::COUNT];
        for value in &mut step_us {
            *value = self.read_u128()?;
        }
        let target_extract_us = self.read_u128()?;
        Ok(OverworldFeatureBatchTiming {
            seed_dependency_insert_us,
            plan_us,
            dependency_cache_hit_clone_us,
            dependency_generate_us,
            dependency_generation,
            dependency_insert_clone_us,
            dependency_retain_us,
            retained_dependency_clone_us,
            feature_region_init_us,
            feature_decoration_us,
            feature_decoration_steps: FeatureDecorationTiming { step_us },
            target_extract_us,
        })
    }

    fn read_level_light_timing(&mut self) -> Result<LevelLightComputationTiming, String> {
        Ok(LevelLightComputationTiming {
            total_us: self.read_u128()?,
            world_init_us: self.read_u128()?,
            active_sections_us: self.read_u128()?,
            sky_source_scan_us: self.read_u128()?,
            block_source_scan_us: self.read_u128()?,
            engine_init_us: self.read_u128()?,
            section_setup_us: self.read_u128()?,
            sky_source_enqueue_us: self.read_u128()?,
            block_source_enqueue_us: self.read_u128()?,
            run_updates_us: self.read_u128()?,
            run_update_iterations: self.read_u64()? as usize,
            block_run_update_calls: self.read_u64()? as usize,
            sky_run_update_calls: self.read_u64()? as usize,
            block_run_update_processed_nodes: self.read_u64()? as usize,
            sky_run_update_processed_nodes: self.read_u64()? as usize,
            max_block_run_update_queue_before: self.read_u64()? as usize,
            max_sky_run_update_queue_before: self.read_u64()? as usize,
            final_block_run_update_queue_after: self.read_u64()? as usize,
            final_sky_run_update_queue_after: self.read_u64()? as usize,
            block_run_updates_us: self.read_u128()?,
            sky_run_updates_us: self.read_u128()?,
            collect_sections_us: self.read_u128()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, ChunkRevision, ChunkStatus};

    #[test]
    fn worldgen_job_frame_crosses_command_update_boundary() {
        let request =
            encode_worldgen_request(ChunkJobId(7), 12_345, &[ChunkPos::new(0, 0)], Vec::new())
                .unwrap();
        let response = compute_worldgen_job_frame(&request).unwrap();
        let decoded = decode_worldgen_response(&response).unwrap();

        assert_eq!(decoded.job_id, ChunkJobId(7));
        assert!(decoded.generated_chunks.contains_key(&ChunkPos::new(0, 0)));
        assert_eq!(
            decoded.cache_report.retained_dependency_chunks,
            decoded.cache_report.requested_dependency_chunks
        );
        assert!(decoded.cache_report.retained_dependency_chunks > 0);
    }

    #[test]
    fn light_status_job_frame_roundtrips_completed_status() {
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Features,
            ChunkRevision(3),
            0,
            16,
            &vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME],
        );
        let pending = PendingLightStatus::from_parts(
            ChunkPos::new(0, 0),
            snapshot.clone(),
            vec![0; CHUNK_SECTION_VOLUME],
            Vec::new(),
        );
        let request =
            encode_light_status_request(PendingLightStatusBatch::new(vec![pending])).unwrap();
        let response = compute_light_status_job_frame(&request).unwrap();
        let completed = decode_light_status_response(&response).unwrap();

        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].pos, ChunkPos::new(0, 0));
        assert_eq!(completed[0].feature_snapshot, snapshot);
        assert!(completed[0].batch_compute_leader);
    }
}
