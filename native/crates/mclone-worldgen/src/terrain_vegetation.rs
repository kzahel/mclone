use std::error::Error;
use std::fmt;

use crate::levelgen::{
    MCLONE_OVERWORLD_VEGETATION_REVISION, McloneOverworldSamplingTopology,
    McloneOverworldVegetationPlanCache, McloneTreeArchetype, McloneTreeBounds, McloneTreeFamily,
    McloneTreeId, McloneTreeOccurrence, McloneTreeRecord, McloneVegetationPlanCacheReport,
    McloneVegetationSource,
};
use crate::placement::BlockPos;
#[cfg(test)]
use crate::terrain_preview::TERRAIN_PREVIEW_MAX_CELLS_PER_AXIS;
use crate::terrain_preview::{
    TerrainPreviewContentStage, TerrainPreviewProfile, TerrainPreviewRequest,
    TerrainPreviewSurfaceQuality, TerrainPreviewVegetationProduct,
};

pub const TERRAIN_VEGETATION_COMPILER_SOURCE_REVISION: &str =
    "mclone-terrain-vegetation-compiler-v1";
pub const TERRAIN_VEGETATION_PRODUCT_REVISION: u32 = 1;
pub const MCHV_WIRE_VERSION: u16 = 1;
pub const MCHV_OCCURRENCE_BYTES: usize = 80;
pub const MCHV_RESIDENT_RESULT_CAPACITY: usize = 256 * 1024;
pub const MCHV_MAX_RESULT_CAPACITY: usize = 1024 * 1024;
pub const MCHV_MAX_OCCURRENCES: usize = 8_192;
pub const MCHV_MAX_ERROR_BYTES: usize = 4 * 1024;

const MCHV_MAGIC: [u8; 4] = *b"MCHV";
const MCHV_HEADER_BYTES: usize = 12;
const MCHV_HEADER_RESERVED: u8 = 0;
const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TerrainVegetationSourceIdentity {
    pub profile: TerrainPreviewProfile,
    pub seed: i64,
    pub topology: McloneOverworldSamplingTopology,
    pub content_stage: TerrainPreviewContentStage,
    pub surface_quality: TerrainPreviewSurfaceQuality,
    pub terrain_source_revision: u64,
    pub compiler_source_revision: u64,
    pub vegetation_plan_revision: u64,
    pub product_revision: u32,
}

impl TerrainVegetationSourceIdentity {
    pub fn for_request(request: TerrainPreviewRequest) -> Result<Self, String> {
        request.validate()?;
        Ok(Self {
            profile: request.profile,
            seed: request.seed,
            topology: request.topology,
            content_stage: request.content_stage,
            surface_quality: request.surface_quality,
            terrain_source_revision: revision_fingerprint(request.profile.source_revision()),
            compiler_source_revision: revision_fingerprint(
                TERRAIN_VEGETATION_COMPILER_SOURCE_REVISION,
            ),
            vegetation_plan_revision: revision_fingerprint(MCLONE_OVERWORLD_VEGETATION_REVISION),
            product_revision: TERRAIN_VEGETATION_PRODUCT_REVISION,
        })
    }

    pub fn validate_request(self, request: TerrainPreviewRequest) -> Result<(), String> {
        self.validate()?;
        let expected = Self::for_request(request)?;
        if self != expected {
            return Err(format!(
                "terrain vegetation source identity {self:?} does not match request source {expected:?}"
            ));
        }
        Ok(())
    }

    pub fn validate(self) -> Result<(), String> {
        let expected = Self {
            terrain_source_revision: revision_fingerprint(self.profile.source_revision()),
            compiler_source_revision: revision_fingerprint(
                TERRAIN_VEGETATION_COMPILER_SOURCE_REVISION,
            ),
            vegetation_plan_revision: revision_fingerprint(MCLONE_OVERWORLD_VEGETATION_REVISION),
            product_revision: TERRAIN_VEGETATION_PRODUCT_REVISION,
            ..self
        };
        if self != expected {
            return Err(format!(
                "terrain vegetation source revisions {self:?} do not match compiler revisions {expected:?}"
            ));
        }
        Ok(())
    }

    pub fn stable_fingerprint(self) -> u64 {
        let mut hash = FNV1A64_OFFSET;
        for byte in [
            profile_tag(self.profile),
            topology_tag(self.topology),
            content_stage_tag(self.content_stage),
            surface_quality_tag(self.surface_quality),
        ]
        .into_iter()
        .chain(self.seed.to_le_bytes())
        .chain(self.terrain_source_revision.to_le_bytes())
        .chain(self.compiler_source_revision.to_le_bytes())
        .chain(self.vegetation_plan_revision.to_le_bytes())
        .chain(self.product_revision.to_le_bytes())
        {
            hash = fnv1a64_byte(hash, byte);
        }
        hash
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainVegetationCompilerReport {
    pub compiled_jobs: u64,
    pub source_initializations: u64,
    pub source_resets: u64,
    pub cell_requests: u64,
    pub cell_hits: u64,
    pub cell_misses: u64,
    pub retained_cells: usize,
    pub retained_preliminary_candidates: usize,
}

#[derive(Clone, Debug)]
pub struct TerrainVegetationCompilerSession {
    source: Option<TerrainVegetationSourceIdentity>,
    cache: Option<McloneOverworldVegetationPlanCache>,
    report: TerrainVegetationCompilerReport,
}

impl Default for TerrainVegetationCompilerSession {
    fn default() -> Self {
        Self {
            source: None,
            cache: None,
            report: TerrainVegetationCompilerReport::default(),
        }
    }
}

impl TerrainVegetationCompilerSession {
    pub fn new(source: TerrainVegetationSourceIdentity) -> Self {
        let mut session = Self::default();
        session.install_source(source, false);
        session
    }

    pub const fn source(&self) -> Option<TerrainVegetationSourceIdentity> {
        self.source
    }

    pub const fn report(&self) -> TerrainVegetationCompilerReport {
        self.report
    }

    pub fn reset(&mut self, source: TerrainVegetationSourceIdentity) {
        self.install_source(source, self.source.is_some());
    }

    pub fn compile(
        &mut self,
        source: TerrainVegetationSourceIdentity,
        request: TerrainPreviewRequest,
    ) -> Result<TerrainPreviewVegetationProduct, TerrainVegetationCompileError> {
        source
            .validate_request(request)
            .map_err(TerrainVegetationCompileError::InvalidSource)?;
        if self.source != Some(source) {
            self.install_source(source, self.source.is_some());
        }
        let cache = self
            .cache
            .as_mut()
            .expect("installing a terrain vegetation source creates its cache");
        let product = TerrainPreviewVegetationProduct::compile_with_cache(request, cache)
            .map_err(TerrainVegetationCompileError::Compile)?;
        let job_report = product.cache_report();
        let retained = cache.report();
        self.report.compiled_jobs = self.report.compiled_jobs.saturating_add(1);
        self.report.cell_requests = self
            .report
            .cell_requests
            .saturating_add(job_report.cell_requests);
        self.report.cell_hits = self.report.cell_hits.saturating_add(job_report.cell_hits);
        self.report.cell_misses = self
            .report
            .cell_misses
            .saturating_add(job_report.cell_misses);
        self.report.retained_cells = retained.retained_cells;
        self.report.retained_preliminary_candidates = retained.retained_preliminary_candidates;
        Ok(product)
    }

    fn install_source(&mut self, source: TerrainVegetationSourceIdentity, reset: bool) {
        self.source = Some(source);
        self.cache = Some(McloneOverworldVegetationPlanCache::new(
            McloneVegetationSource::new(source.seed, source.topology),
        ));
        if reset {
            self.report.source_resets = self.report.source_resets.saturating_add(1);
        } else {
            self.report.source_initializations =
                self.report.source_initializations.saturating_add(1);
        }
        self.report.retained_cells = 0;
        self.report.retained_preliminary_candidates = 0;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerrainVegetationCompileError {
    InvalidSource(String),
    Compile(String),
}

impl fmt::Display for TerrainVegetationCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSource(error) => write!(formatter, "invalid vegetation source: {error}"),
            Self::Compile(error) => write!(formatter, "vegetation compile failed: {error}"),
        }
    }
}

impl Error for TerrainVegetationCompileError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainVegetationProductReceipt {
    pub source_fingerprint: u64,
    pub record_hash: u64,
    pub family_counts: [u32; 3],
    pub record_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainVegetationCoverageReceipt {
    pub source_fingerprint: u64,
    pub record_hash: u64,
    pub family_counts: [u32; 3],
    pub record_count: u32,
    pub product_count: u32,
}

pub fn terrain_vegetation_product_receipt(
    source: TerrainVegetationSourceIdentity,
    product: &TerrainPreviewVegetationProduct,
) -> TerrainVegetationProductReceipt {
    let mut family_counts = [0_u32; 3];
    let mut record_hash = FNV1A64_OFFSET;
    for occurrence in product.occurrences() {
        family_counts[family_index(occurrence.record.family)] =
            family_counts[family_index(occurrence.record.family)].saturating_add(1);
        hash_occurrence(&mut record_hash, occurrence);
    }
    TerrainVegetationProductReceipt {
        source_fingerprint: source.stable_fingerprint(),
        record_hash,
        family_counts,
        record_count: u32::try_from(product.occurrences().len()).unwrap_or(u32::MAX),
    }
}

pub fn terrain_vegetation_coverage_receipt<'a>(
    source: TerrainVegetationSourceIdentity,
    products: impl IntoIterator<Item = &'a TerrainPreviewVegetationProduct>,
) -> Result<TerrainVegetationCoverageReceipt, String> {
    source.validate()?;
    let mut products = products.into_iter().collect::<Vec<_>>();
    for product in &products {
        source.validate_request(product.request().request())?;
    }
    products.sort_by_key(|product| request_order_key(product.request().request()));

    let mut record_hash = FNV1A64_OFFSET;
    let mut source_bytes = FrameWriter::default();
    source_bytes.write_source(source);
    hash_bytes(&mut record_hash, &source_bytes.finish());
    let mut family_counts = [0_u32; 3];
    let mut record_count = 0_u32;
    for product in &products {
        let mut request_bytes = FrameWriter::default();
        request_bytes.write_request(product.request().request());
        request_bytes.write_u32(
            u32::try_from(product.occurrences().len())
                .map_err(|_| "terrain vegetation coverage record count exceeds u32")?,
        );
        hash_bytes(&mut record_hash, &request_bytes.finish());
        for occurrence in product.occurrences() {
            family_counts[family_index(occurrence.record.family)] =
                family_counts[family_index(occurrence.record.family)].saturating_add(1);
            record_count = record_count.saturating_add(1);
            hash_occurrence(&mut record_hash, occurrence);
        }
    }
    Ok(TerrainVegetationCoverageReceipt {
        source_fingerprint: source.stable_fingerprint(),
        record_hash,
        family_counts,
        record_count,
        product_count: u32::try_from(products.len()).unwrap_or(u32::MAX),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MchvActorIdentity {
    pub executor_generation: u32,
    pub source_epoch: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MchvJobIdentity {
    pub actor: MchvActorIdentity,
    pub request_id: u32,
    pub physical_slot: u32,
    pub slot_generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MchvFailureKind {
    Protocol,
    Source,
    Compile,
    Transport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MchvFrame {
    Initialize {
        actor: MchvActorIdentity,
        source: TerrainVegetationSourceIdentity,
    },
    Compile {
        job: MchvJobIdentity,
        source: TerrainVegetationSourceIdentity,
        request: TerrainPreviewRequest,
    },
    Shutdown {
        actor: MchvActorIdentity,
    },
    Ready {
        actor: MchvActorIdentity,
        source: TerrainVegetationSourceIdentity,
    },
    Completed {
        job: MchvJobIdentity,
        source: TerrainVegetationSourceIdentity,
        product: TerrainPreviewVegetationProduct,
        compile_micros: u64,
    },
    Failed {
        actor: MchvActorIdentity,
        request_id: u32,
        kind: MchvFailureKind,
        message: String,
    },
    ShutdownComplete {
        actor: MchvActorIdentity,
    },
}

impl MchvFrame {
    const fn tag(&self) -> u8 {
        match self {
            Self::Initialize { .. } => 1,
            Self::Compile { .. } => 2,
            Self::Shutdown { .. } => 3,
            Self::Ready { .. } => 4,
            Self::Completed { .. } => 5,
            Self::Failed { .. } => 6,
            Self::ShutdownComplete { .. } => 7,
        }
    }
}

pub fn encode_mchv_frame(frame: &MchvFrame) -> Result<Vec<u8>, String> {
    let mut payload = FrameWriter::default();
    match frame {
        MchvFrame::Initialize { actor, source } | MchvFrame::Ready { actor, source } => {
            payload.write_actor(*actor);
            payload.write_source(*source);
        }
        MchvFrame::Compile {
            job,
            source,
            request,
        } => {
            source.validate_request(*request)?;
            payload.write_job(*job);
            payload.write_source(*source);
            payload.write_request(*request);
        }
        MchvFrame::Shutdown { actor } | MchvFrame::ShutdownComplete { actor } => {
            payload.write_actor(*actor);
        }
        MchvFrame::Completed {
            job,
            source,
            product,
            compile_micros,
        } => {
            source.validate_request(product.request().request())?;
            payload.write_job(*job);
            payload.write_source(*source);
            payload.write_u64(*compile_micros);
            payload.write_product(*source, product)?;
        }
        MchvFrame::Failed {
            actor,
            request_id,
            kind,
            message,
        } => {
            if message.len() > MCHV_MAX_ERROR_BYTES {
                return Err(format!(
                    "MCHV failure message has {} bytes, maximum is {MCHV_MAX_ERROR_BYTES}",
                    message.len()
                ));
            }
            payload.write_actor(*actor);
            payload.write_u32(*request_id);
            payload.write_u8(failure_kind_tag(*kind));
            payload.write_bytes(&[0, 0, 0]);
            payload.write_len_prefixed(message.as_bytes())?;
        }
    }
    let payload = payload.finish();
    let frame_len = MCHV_HEADER_BYTES
        .checked_add(payload.len())
        .ok_or("MCHV frame length overflow")?;
    if frame_len > MCHV_MAX_RESULT_CAPACITY {
        return Err(format!(
            "MCHV frame has {frame_len} bytes, maximum is {MCHV_MAX_RESULT_CAPACITY}"
        ));
    }
    let payload_len =
        u32::try_from(payload.len()).map_err(|_| "MCHV payload length exceeds u32")?;
    let mut encoded = Vec::with_capacity(frame_len);
    encoded.extend_from_slice(&MCHV_MAGIC);
    encoded.extend_from_slice(&MCHV_WIRE_VERSION.to_le_bytes());
    encoded.push(frame.tag());
    encoded.push(MCHV_HEADER_RESERVED);
    encoded.extend_from_slice(&payload_len.to_le_bytes());
    encoded.extend_from_slice(&payload);
    Ok(encoded)
}

pub fn decode_mchv_frame(bytes: &[u8]) -> Result<MchvFrame, String> {
    if bytes.len() < MCHV_HEADER_BYTES {
        return Err("MCHV frame is truncated before its header".to_owned());
    }
    if bytes.len() > MCHV_MAX_RESULT_CAPACITY {
        return Err(format!(
            "MCHV frame has {} bytes, maximum is {MCHV_MAX_RESULT_CAPACITY}",
            bytes.len()
        ));
    }
    if bytes[0..4] != MCHV_MAGIC {
        return Err("MCHV frame has invalid magic".to_owned());
    }
    let version = u16::from_le_bytes(bytes[4..6].try_into().expect("checked MCHV header"));
    if version != MCHV_WIRE_VERSION {
        return Err(format!(
            "MCHV wire version {version} is unsupported; expected {MCHV_WIRE_VERSION}"
        ));
    }
    let tag = bytes[6];
    if bytes[7] != MCHV_HEADER_RESERVED {
        return Err("MCHV header reserved byte is non-zero".to_owned());
    }
    let payload_len =
        u32::from_le_bytes(bytes[8..12].try_into().expect("checked MCHV header")) as usize;
    if payload_len != bytes.len() - MCHV_HEADER_BYTES {
        return Err(format!(
            "MCHV payload declares {payload_len} bytes, frame contains {}",
            bytes.len() - MCHV_HEADER_BYTES
        ));
    }
    let mut reader = FrameReader::new(&bytes[MCHV_HEADER_BYTES..]);
    let frame = match tag {
        1 => {
            let actor = reader.read_actor()?;
            let source = reader.read_source()?;
            MchvFrame::Initialize { actor, source }
        }
        2 => {
            let job = reader.read_job()?;
            let source = reader.read_source()?;
            let request = reader.read_request()?;
            source.validate_request(request)?;
            MchvFrame::Compile {
                job,
                source,
                request,
            }
        }
        3 => MchvFrame::Shutdown {
            actor: reader.read_actor()?,
        },
        4 => {
            let actor = reader.read_actor()?;
            let source = reader.read_source()?;
            MchvFrame::Ready { actor, source }
        }
        5 => {
            let job = reader.read_job()?;
            let source = reader.read_source()?;
            let compile_micros = reader.read_u64()?;
            let product = reader.read_product(source)?;
            MchvFrame::Completed {
                job,
                source,
                product,
                compile_micros,
            }
        }
        6 => {
            let actor = reader.read_actor()?;
            let request_id = reader.read_u32()?;
            let kind = failure_kind_from_tag(reader.read_u8()?)?;
            reader.read_zeroes(3, "MCHV failure reserved bytes")?;
            let message_bytes =
                reader.read_len_prefixed(MCHV_MAX_ERROR_BYTES, "failure message")?;
            let message = String::from_utf8(message_bytes.to_vec())
                .map_err(|_| "MCHV failure message is not UTF-8".to_owned())?;
            MchvFrame::Failed {
                actor,
                request_id,
                kind,
                message,
            }
        }
        7 => MchvFrame::ShutdownComplete {
            actor: reader.read_actor()?,
        },
        other => return Err(format!("MCHV frame kind {other} is unsupported")),
    };
    reader.finish()?;
    Ok(frame)
}

#[derive(Default)]
struct FrameWriter {
    bytes: Vec<u8>,
}

impl FrameWriter {
    fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn write_u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn write_len_prefixed(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.write_u32(
            u32::try_from(bytes.len()).map_err(|_| "MCHV byte value length exceeds u32")?,
        );
        self.write_bytes(bytes);
        Ok(())
    }

    fn write_actor(&mut self, actor: MchvActorIdentity) {
        self.write_u32(actor.executor_generation);
        self.write_u32(actor.source_epoch);
    }

    fn write_job(&mut self, job: MchvJobIdentity) {
        self.write_actor(job.actor);
        self.write_u32(job.request_id);
        self.write_u32(job.physical_slot);
        self.write_u32(job.slot_generation);
    }

    fn write_source(&mut self, source: TerrainVegetationSourceIdentity) {
        self.write_u8(profile_tag(source.profile));
        self.write_u8(topology_tag(source.topology));
        self.write_u8(content_stage_tag(source.content_stage));
        self.write_u8(surface_quality_tag(source.surface_quality));
        self.write_i64(source.seed);
        self.write_u64(source.terrain_source_revision);
        self.write_u64(source.compiler_source_revision);
        self.write_u64(source.vegetation_plan_revision);
        self.write_u32(source.product_revision);
    }

    fn write_request(&mut self, request: TerrainPreviewRequest) {
        self.write_u8(profile_tag(request.profile));
        self.write_u8(topology_tag(request.topology));
        self.write_u8(content_stage_tag(request.content_stage));
        self.write_u8(surface_quality_tag(request.surface_quality));
        self.write_i64(request.seed);
        self.write_i32(request.center_x);
        self.write_i32(request.center_z);
        self.write_u32(request.sample_spacing);
        self.write_u32(request.cells_per_axis);
    }

    fn write_product(
        &mut self,
        source: TerrainVegetationSourceIdentity,
        product: &TerrainPreviewVegetationProduct,
    ) -> Result<(), String> {
        let count = product.occurrences().len();
        if count > MCHV_MAX_OCCURRENCES {
            return Err(format!(
                "MCHV vegetation product contains {count} occurrences, maximum is {MCHV_MAX_OCCURRENCES}"
            ));
        }
        self.write_request(product.request().request());
        self.write_u8(u8::from(product.summary_available()));
        self.write_u8(u8::from(product.records_requested()));
        self.write_u8(u8::from(product.records_aggregated()));
        self.write_u8(0);
        let cache = product.cache_report();
        self.write_u64(cache.cell_requests);
        self.write_u64(cache.cell_hits);
        self.write_u64(cache.cell_misses);
        self.write_u64(
            u64::try_from(cache.retained_cells)
                .map_err(|_| "MCHV retained vegetation cell count exceeds u64")?,
        );
        self.write_u64(
            u64::try_from(cache.retained_preliminary_candidates)
                .map_err(|_| "MCHV retained candidate count exceeds u64")?,
        );
        let receipt = terrain_vegetation_product_receipt(source, product);
        self.write_u64(receipt.source_fingerprint);
        self.write_u64(receipt.record_hash);
        for count in receipt.family_counts {
            self.write_u32(count);
        }
        self.write_u32(receipt.record_count);
        self.write_u32(u32::try_from(count).map_err(|_| "MCHV occurrence count exceeds u32")?);
        for occurrence in product.occurrences() {
            self.write_occurrence(occurrence);
        }
        Ok(())
    }

    fn write_occurrence(&mut self, occurrence: &McloneTreeOccurrence) {
        let record = occurrence.record;
        self.write_i32(record.id.planning_cell_x);
        self.write_i32(record.id.planning_cell_z);
        self.write_u8(record.id.candidate_slot);
        self.write_u16(record.id.vegetation_revision);
        self.write_i32(record.canonical_base.x);
        self.write_i32(record.canonical_base.y);
        self.write_i32(record.canonical_base.z);
        self.write_u8(family_tag(record.family));
        self.write_u8(archetype_tag(record.archetype));
        self.write_u16(record.trunk_height);
        self.write_u16(record.crown_radius);
        self.write_u16(record.crown_depth);
        self.write_u8(record.orientation);
        self.write_u8(record.landmark_rank);
        self.write_u64(record.variant_seed);
        self.write_i32(record.bounds.min_x);
        self.write_i32(record.bounds.min_y);
        self.write_i32(record.bounds.min_z);
        self.write_i32(record.bounds.max_x);
        self.write_i32(record.bounds.max_y);
        self.write_i32(record.bounds.max_z);
        self.write_i64(occurrence.x_lift);
        self.write_bytes(&[0; 7]);
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

struct FrameReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> FrameReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_bytes(&mut self, len: usize, label: &str) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| format!("{label} offset overflow"))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| format!("{label} is truncated"))?;
        self.offset = end;
        Ok(bytes)
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.read_bytes(1, "MCHV u8")?[0])
    }

    fn read_u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(
            self.read_bytes(2, "MCHV u16")?
                .try_into()
                .expect("checked u16 length"),
        ))
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.read_bytes(4, "MCHV u32")?
                .try_into()
                .expect("checked u32 length"),
        ))
    }

    fn read_i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(
            self.read_bytes(4, "MCHV i32")?
                .try_into()
                .expect("checked i32 length"),
        ))
    }

    fn read_u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(
            self.read_bytes(8, "MCHV u64")?
                .try_into()
                .expect("checked u64 length"),
        ))
    }

    fn read_i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(
            self.read_bytes(8, "MCHV i64")?
                .try_into()
                .expect("checked i64 length"),
        ))
    }

    fn read_zeroes(&mut self, len: usize, label: &str) -> Result<(), String> {
        if self.read_bytes(len, label)?.iter().any(|byte| *byte != 0) {
            return Err(format!("{label} are non-zero"));
        }
        Ok(())
    }

    fn read_len_prefixed(&mut self, maximum: usize, label: &str) -> Result<&'a [u8], String> {
        let len = self.read_u32()? as usize;
        if len > maximum {
            return Err(format!("{label} has {len} bytes, maximum is {maximum}"));
        }
        self.read_bytes(len, label)
    }

    fn read_actor(&mut self) -> Result<MchvActorIdentity, String> {
        Ok(MchvActorIdentity {
            executor_generation: self.read_u32()?,
            source_epoch: self.read_u32()?,
        })
    }

    fn read_job(&mut self) -> Result<MchvJobIdentity, String> {
        Ok(MchvJobIdentity {
            actor: self.read_actor()?,
            request_id: self.read_u32()?,
            physical_slot: self.read_u32()?,
            slot_generation: self.read_u32()?,
        })
    }

    fn read_source(&mut self) -> Result<TerrainVegetationSourceIdentity, String> {
        Ok(TerrainVegetationSourceIdentity {
            profile: profile_from_tag(self.read_u8()?)?,
            topology: topology_from_tag(self.read_u8()?)?,
            content_stage: content_stage_from_tag(self.read_u8()?)?,
            surface_quality: surface_quality_from_tag(self.read_u8()?)?,
            seed: self.read_i64()?,
            terrain_source_revision: self.read_u64()?,
            compiler_source_revision: self.read_u64()?,
            vegetation_plan_revision: self.read_u64()?,
            product_revision: self.read_u32()?,
        })
    }

    fn read_request(&mut self) -> Result<TerrainPreviewRequest, String> {
        let request = TerrainPreviewRequest {
            profile: profile_from_tag(self.read_u8()?)?,
            topology: topology_from_tag(self.read_u8()?)?,
            content_stage: content_stage_from_tag(self.read_u8()?)?,
            surface_quality: surface_quality_from_tag(self.read_u8()?)?,
            seed: self.read_i64()?,
            center_x: self.read_i32()?,
            center_z: self.read_i32()?,
            sample_spacing: self.read_u32()?,
            cells_per_axis: self.read_u32()?,
        };
        request.validate()?;
        Ok(request)
    }

    fn read_product(
        &mut self,
        source: TerrainVegetationSourceIdentity,
    ) -> Result<TerrainPreviewVegetationProduct, String> {
        let request = self.read_request()?;
        source.validate_request(request)?;
        let summary_available = self.read_bool("summary available")?;
        let records_requested = self.read_bool("records requested")?;
        let records_aggregated = self.read_bool("records aggregated")?;
        self.read_zeroes(1, "MCHV product reserved byte")?;
        let cache_report = McloneVegetationPlanCacheReport {
            cell_requests: self.read_u64()?,
            cell_hits: self.read_u64()?,
            cell_misses: self.read_u64()?,
            retained_cells: usize::try_from(self.read_u64()?)
                .map_err(|_| "MCHV retained vegetation cell count exceeds usize")?,
            retained_preliminary_candidates: usize::try_from(self.read_u64()?)
                .map_err(|_| "MCHV retained candidate count exceeds usize")?,
        };
        let encoded_source_fingerprint = self.read_u64()?;
        let encoded_record_hash = self.read_u64()?;
        let encoded_family_counts = [self.read_u32()?, self.read_u32()?, self.read_u32()?];
        let encoded_record_count = self.read_u32()?;
        let count = self.read_u32()? as usize;
        if count > MCHV_MAX_OCCURRENCES {
            return Err(format!(
                "MCHV vegetation product contains {count} occurrences, maximum is {MCHV_MAX_OCCURRENCES}"
            ));
        }
        let occurrence_bytes = count
            .checked_mul(MCHV_OCCURRENCE_BYTES)
            .ok_or("MCHV occurrence byte length overflow")?;
        if occurrence_bytes > self.bytes.len().saturating_sub(self.offset) {
            return Err("MCHV vegetation occurrences are truncated".to_owned());
        }
        let mut occurrences = Vec::with_capacity(count);
        for _ in 0..count {
            occurrences.push(self.read_occurrence()?);
        }
        let product = TerrainPreviewVegetationProduct::from_codec_parts(
            request,
            summary_available,
            records_requested,
            records_aggregated,
            occurrences,
            cache_report,
        )?;
        let receipt = terrain_vegetation_product_receipt(source, &product);
        if (
            encoded_source_fingerprint,
            encoded_record_hash,
            encoded_family_counts,
            encoded_record_count,
        ) != (
            receipt.source_fingerprint,
            receipt.record_hash,
            receipt.family_counts,
            receipt.record_count,
        ) {
            return Err("MCHV vegetation product receipt does not match its records".to_owned());
        }
        Ok(product)
    }

    fn read_bool(&mut self, label: &str) -> Result<bool, String> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(format!("MCHV {label} flag {other} is invalid")),
        }
    }

    fn read_occurrence(&mut self) -> Result<McloneTreeOccurrence, String> {
        let id = McloneTreeId {
            planning_cell_x: self.read_i32()?,
            planning_cell_z: self.read_i32()?,
            candidate_slot: self.read_u8()?,
            vegetation_revision: self.read_u16()?,
        };
        let canonical_base = BlockPos::new(self.read_i32()?, self.read_i32()?, self.read_i32()?);
        let family = family_from_tag(self.read_u8()?)?;
        let archetype = archetype_from_tag(self.read_u8()?)?;
        let trunk_height = self.read_u16()?;
        let crown_radius = self.read_u16()?;
        let crown_depth = self.read_u16()?;
        let orientation = self.read_u8()?;
        let landmark_rank = self.read_u8()?;
        let variant_seed = self.read_u64()?;
        let bounds = McloneTreeBounds::new(
            self.read_i32()?,
            self.read_i32()?,
            self.read_i32()?,
            self.read_i32()?,
            self.read_i32()?,
            self.read_i32()?,
        )
        .map_err(|error| error.to_string())?;
        let x_lift = self.read_i64()?;
        self.read_zeroes(7, "MCHV occurrence reserved bytes")?;
        let working_bounds = McloneTreeBounds::new(
            shift_x(bounds.min_x, x_lift)?,
            bounds.min_y,
            bounds.min_z,
            shift_x(bounds.max_x, x_lift)?,
            bounds.max_y,
            bounds.max_z,
        )
        .map_err(|error| error.to_string())?;
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

    fn finish(self) -> Result<(), String> {
        if self.offset != self.bytes.len() {
            return Err(format!(
                "MCHV frame has {} trailing bytes",
                self.bytes.len() - self.offset
            ));
        }
        Ok(())
    }
}

const fn revision_fingerprint(value: &str) -> u64 {
    let bytes = value.as_bytes();
    let mut hash = FNV1A64_OFFSET;
    let mut index = 0;
    while index < bytes.len() {
        hash = fnv1a64_byte(hash, bytes[index]);
        index += 1;
    }
    hash
}

const fn fnv1a64_byte(hash: u64, byte: u8) -> u64 {
    (hash ^ byte as u64).wrapping_mul(FNV1A64_PRIME)
}

fn hash_occurrence(hash: &mut u64, occurrence: &McloneTreeOccurrence) {
    let mut encoded = FrameWriter::default();
    encoded.write_occurrence(occurrence);
    for byte in &encoded.finish()[..MCHV_OCCURRENCE_BYTES - 7] {
        *hash = fnv1a64_byte(*hash, *byte);
    }
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash = fnv1a64_byte(*hash, *byte);
    }
}

fn request_order_key(request: TerrainPreviewRequest) -> (u8, u8, u8, u8, i64, u32, i32, i32, u32) {
    (
        profile_tag(request.profile),
        topology_tag(request.topology),
        content_stage_tag(request.content_stage),
        surface_quality_tag(request.surface_quality),
        request.seed,
        request.sample_spacing,
        request.center_z,
        request.center_x,
        request.cells_per_axis,
    )
}

fn shift_x(value: i32, lift: i64) -> Result<i32, String> {
    i32::try_from(
        i64::from(value)
            .checked_add(lift)
            .ok_or("X lift overflow")?,
    )
    .map_err(|_| "X lift exceeds i32 coordinates".to_owned())
}

const fn profile_tag(profile: TerrainPreviewProfile) -> u8 {
    match profile {
        TerrainPreviewProfile::McloneOverworldV1 => 1,
        TerrainPreviewProfile::VanillaOverworld => 2,
        TerrainPreviewProfile::ContinentalEcoregionCandidate => 3,
    }
}

fn profile_from_tag(tag: u8) -> Result<TerrainPreviewProfile, String> {
    match tag {
        1 => Ok(TerrainPreviewProfile::McloneOverworldV1),
        2 => Ok(TerrainPreviewProfile::VanillaOverworld),
        3 => Ok(TerrainPreviewProfile::ContinentalEcoregionCandidate),
        other => Err(format!("MCHV terrain profile tag {other} is invalid")),
    }
}

const fn topology_tag(topology: McloneOverworldSamplingTopology) -> u8 {
    match topology {
        McloneOverworldSamplingTopology::Unbounded => 1,
        McloneOverworldSamplingTopology::PeriodicX => 2,
    }
}

fn topology_from_tag(tag: u8) -> Result<McloneOverworldSamplingTopology, String> {
    match tag {
        1 => Ok(McloneOverworldSamplingTopology::Unbounded),
        2 => Ok(McloneOverworldSamplingTopology::PeriodicX),
        other => Err(format!("MCHV sampling topology tag {other} is invalid")),
    }
}

const fn content_stage_tag(stage: TerrainPreviewContentStage) -> u8 {
    match stage {
        TerrainPreviewContentStage::Base => 0,
        TerrainPreviewContentStage::Hydrology => 1,
        TerrainPreviewContentStage::Structured => 2,
        TerrainPreviewContentStage::Surface => 3,
        TerrainPreviewContentStage::Cover => 4,
    }
}

fn content_stage_from_tag(tag: u8) -> Result<TerrainPreviewContentStage, String> {
    match tag {
        0 => Ok(TerrainPreviewContentStage::Base),
        1 => Ok(TerrainPreviewContentStage::Hydrology),
        2 => Ok(TerrainPreviewContentStage::Structured),
        3 => Ok(TerrainPreviewContentStage::Surface),
        4 => Ok(TerrainPreviewContentStage::Cover),
        other => Err(format!("MCHV content stage tag {other} is invalid")),
    }
}

const fn surface_quality_tag(quality: TerrainPreviewSurfaceQuality) -> u8 {
    match quality {
        TerrainPreviewSurfaceQuality::Basic => 0,
        TerrainPreviewSurfaceQuality::Inferred => 1,
    }
}

fn surface_quality_from_tag(tag: u8) -> Result<TerrainPreviewSurfaceQuality, String> {
    match tag {
        0 => Ok(TerrainPreviewSurfaceQuality::Basic),
        1 => Ok(TerrainPreviewSurfaceQuality::Inferred),
        other => Err(format!("MCHV surface quality tag {other} is invalid")),
    }
}

const fn family_tag(family: McloneTreeFamily) -> u8 {
    match family {
        McloneTreeFamily::TemperateBroadleaf => 0,
        McloneTreeFamily::CoolWetConifer => 1,
        McloneTreeFamily::WarmDryAcacia => 2,
    }
}

const fn family_index(family: McloneTreeFamily) -> usize {
    family_tag(family) as usize
}

fn family_from_tag(tag: u8) -> Result<McloneTreeFamily, String> {
    match tag {
        0 => Ok(McloneTreeFamily::TemperateBroadleaf),
        1 => Ok(McloneTreeFamily::CoolWetConifer),
        2 => Ok(McloneTreeFamily::WarmDryAcacia),
        other => Err(format!("MCHV tree family tag {other} is invalid")),
    }
}

const fn archetype_tag(archetype: McloneTreeArchetype) -> u8 {
    match archetype {
        McloneTreeArchetype::RoundedBroadleaf => 0,
        McloneTreeArchetype::LayeredConifer => 1,
        McloneTreeArchetype::ForkedAcacia => 2,
    }
}

fn archetype_from_tag(tag: u8) -> Result<McloneTreeArchetype, String> {
    match tag {
        0 => Ok(McloneTreeArchetype::RoundedBroadleaf),
        1 => Ok(McloneTreeArchetype::LayeredConifer),
        2 => Ok(McloneTreeArchetype::ForkedAcacia),
        other => Err(format!("MCHV tree archetype tag {other} is invalid")),
    }
}

const fn failure_kind_tag(kind: MchvFailureKind) -> u8 {
    match kind {
        MchvFailureKind::Protocol => 1,
        MchvFailureKind::Source => 2,
        MchvFailureKind::Compile => 3,
        MchvFailureKind::Transport => 4,
    }
}

fn failure_kind_from_tag(tag: u8) -> Result<MchvFailureKind, String> {
    match tag {
        1 => Ok(MchvFailureKind::Protocol),
        2 => Ok(MchvFailureKind::Source),
        3 => Ok(MchvFailureKind::Compile),
        4 => Ok(MchvFailureKind::Transport),
        other => Err(format!("MCHV failure kind tag {other} is invalid")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(seed: i64, topology: McloneOverworldSamplingTopology) -> TerrainPreviewRequest {
        TerrainPreviewRequest {
            profile: TerrainPreviewProfile::McloneOverworldV1,
            seed,
            center_x: -128,
            center_z: 128,
            sample_spacing: 4,
            cells_per_axis: 64,
            topology,
            content_stage: TerrainPreviewContentStage::Cover,
            surface_quality: TerrainPreviewSurfaceQuality::Inferred,
        }
    }

    fn identities() -> (MchvActorIdentity, MchvJobIdentity) {
        let actor = MchvActorIdentity {
            executor_generation: 7,
            source_epoch: 11,
        };
        (
            actor,
            MchvJobIdentity {
                actor,
                request_id: 13,
                physical_slot: 17,
                slot_generation: 19,
            },
        )
    }

    #[test]
    fn compiler_session_preserves_semantics_and_reuses_bounded_cache() {
        let request = request(12_345, McloneOverworldSamplingTopology::Unbounded);
        let source = TerrainVegetationSourceIdentity::for_request(request).unwrap();
        let mut session = TerrainVegetationCompilerSession::new(source);
        let cold = session.compile(source, request).unwrap();
        let warm = session.compile(source, request).unwrap();

        assert_eq!(cold.occurrences(), warm.occurrences());
        assert_eq!(
            terrain_vegetation_product_receipt(source, &cold),
            terrain_vegetation_product_receipt(source, &warm)
        );
        assert!(warm.cache_report().cell_hits > 0);
        assert_eq!(session.report().compiled_jobs, 2);
        assert_eq!(session.report().source_initializations, 1);
        assert_eq!(session.report().source_resets, 0);
        assert!(session.report().retained_cells <= 4_096);
    }

    #[test]
    fn compiler_session_resets_every_semantic_source_change() {
        let plane_request = request(12_345, McloneOverworldSamplingTopology::Unbounded);
        let cylinder_request = request(12_345, McloneOverworldSamplingTopology::PeriodicX);
        let seed_request = request(-98_765, McloneOverworldSamplingTopology::Unbounded);
        let mut session = TerrainVegetationCompilerSession::default();

        for request in [plane_request, cylinder_request, plane_request, seed_request] {
            let source = TerrainVegetationSourceIdentity::for_request(request).unwrap();
            session.compile(source, request).unwrap();
        }

        assert_eq!(session.report().compiled_jobs, 4);
        assert_eq!(session.report().source_initializations, 1);
        assert_eq!(session.report().source_resets, 3);
        let expected = TerrainVegetationCompilerSession::new(
            TerrainVegetationSourceIdentity::for_request(seed_request).unwrap(),
        )
        .compile(
            TerrainVegetationSourceIdentity::for_request(seed_request).unwrap(),
            seed_request,
        )
        .unwrap();
        let actual = session
            .compile(
                TerrainVegetationSourceIdentity::for_request(seed_request).unwrap(),
                seed_request,
            )
            .unwrap();
        assert_eq!(actual.occurrences(), expected.occurrences());
    }

    #[test]
    fn coverage_receipt_is_order_independent_and_record_complete() {
        let first_request = request(12_345, McloneOverworldSamplingTopology::Unbounded);
        let second_request = TerrainPreviewRequest {
            center_x: first_request.center_x + 256,
            ..first_request
        };
        let source = TerrainVegetationSourceIdentity::for_request(first_request).unwrap();
        let mut session = TerrainVegetationCompilerSession::new(source);
        let first = session.compile(source, first_request).unwrap();
        let second = session.compile(source, second_request).unwrap();
        let forward = terrain_vegetation_coverage_receipt(source, [&first, &second]).unwrap();
        let reverse = terrain_vegetation_coverage_receipt(source, [&second, &first]).unwrap();

        assert_eq!(forward, reverse);
        assert_eq!(forward.product_count, 2);
        assert_eq!(
            forward.record_count,
            u32::try_from(first.occurrences().len() + second.occurrences().len()).unwrap()
        );
        assert_eq!(
            forward.family_counts.iter().sum::<u32>(),
            forward.record_count
        );
        assert_ne!(
            forward.record_hash,
            terrain_vegetation_coverage_receipt(source, [&first])
                .unwrap()
                .record_hash
        );
    }

    #[test]
    fn every_actor_frame_round_trips() {
        let request = request(12_345, McloneOverworldSamplingTopology::Unbounded);
        let source = TerrainVegetationSourceIdentity::for_request(request).unwrap();
        let product = TerrainVegetationCompilerSession::new(source)
            .compile(source, request)
            .unwrap();
        let (actor, job) = identities();
        let frames = [
            MchvFrame::Initialize { actor, source },
            MchvFrame::Compile {
                job,
                source,
                request,
            },
            MchvFrame::Shutdown { actor },
            MchvFrame::Ready { actor, source },
            MchvFrame::Completed {
                job,
                source,
                product,
                compile_micros: 23,
            },
            MchvFrame::Failed {
                actor,
                request_id: job.request_id,
                kind: MchvFailureKind::Compile,
                message: "fixture failure".to_owned(),
            },
            MchvFrame::ShutdownComplete { actor },
        ];
        for frame in frames {
            let encoded = encode_mchv_frame(&frame).unwrap();
            assert_eq!(decode_mchv_frame(&encoded).unwrap(), frame);
        }
    }

    #[test]
    fn empty_and_maximum_products_round_trip_with_fixed_occurrences() {
        let empty_request = TerrainPreviewRequest::new(12_345, 0, 0, 8)
            .with_content_stage(TerrainPreviewContentStage::Cover);
        let empty_source = TerrainVegetationSourceIdentity::for_request(empty_request).unwrap();
        let empty = TerrainVegetationCompilerSession::new(empty_source)
            .compile(empty_source, empty_request)
            .unwrap();
        let (_, job) = identities();
        let frame = MchvFrame::Completed {
            job,
            source: empty_source,
            product: empty,
            compile_micros: 0,
        };
        assert_eq!(
            decode_mchv_frame(&encode_mchv_frame(&frame).unwrap()).unwrap(),
            frame
        );

        let max_request = TerrainPreviewRequest {
            cells_per_axis: TERRAIN_PREVIEW_MAX_CELLS_PER_AXIS,
            ..request(12_345, McloneOverworldSamplingTopology::Unbounded)
        };
        let source = TerrainVegetationSourceIdentity::for_request(max_request).unwrap();
        let fixture_request = request(12_345, McloneOverworldSamplingTopology::Unbounded);
        let fixture_source = TerrainVegetationSourceIdentity::for_request(fixture_request).unwrap();
        let fixture = TerrainVegetationCompilerSession::new(fixture_source)
            .compile(fixture_source, fixture_request)
            .unwrap();
        let occurrence = *fixture
            .occurrences()
            .first()
            .expect("fixture produces vegetation");
        let product = TerrainPreviewVegetationProduct::from_codec_parts(
            max_request,
            true,
            true,
            false,
            vec![occurrence; MCHV_MAX_OCCURRENCES],
            McloneVegetationPlanCacheReport::default(),
        )
        .unwrap();
        let frame = MchvFrame::Completed {
            job,
            source,
            product,
            compile_micros: 0,
        };
        let encoded = encode_mchv_frame(&frame).unwrap();
        assert!(encoded.len() > MCHV_RESIDENT_RESULT_CAPACITY);
        assert!(encoded.len() < MCHV_MAX_RESULT_CAPACITY);
        assert_eq!(decode_mchv_frame(&encoded).unwrap(), frame);
    }

    #[test]
    fn malformed_envelopes_enums_counts_reserved_and_receipts_fail_closed() {
        let request = request(12_345, McloneOverworldSamplingTopology::Unbounded);
        let source = TerrainVegetationSourceIdentity::for_request(request).unwrap();
        let product = TerrainVegetationCompilerSession::new(source)
            .compile(source, request)
            .unwrap();
        let (_, job) = identities();
        let frame = MchvFrame::Completed {
            job,
            source,
            product,
            compile_micros: 0,
        };
        let encoded = encode_mchv_frame(&frame).unwrap();

        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 0xff;
        assert!(decode_mchv_frame(&wrong_magic).is_err());
        let mut wrong_version = encoded.clone();
        wrong_version[4] = 2;
        assert!(decode_mchv_frame(&wrong_version).is_err());
        let mut wrong_kind = encoded.clone();
        wrong_kind[6] = 99;
        assert!(decode_mchv_frame(&wrong_kind).is_err());
        let mut header_reserved = encoded.clone();
        header_reserved[7] = 1;
        assert!(decode_mchv_frame(&header_reserved).is_err());
        assert!(decode_mchv_frame(&encoded[..encoded.len() - 1]).is_err());

        let compile = encode_mchv_frame(&MchvFrame::Compile {
            job,
            source,
            request,
        })
        .unwrap();
        let mut invalid_profile = compile;
        let compile_request_profile = MCHV_HEADER_BYTES + 20 + 40;
        invalid_profile[compile_request_profile] = 99;
        assert!(decode_mchv_frame(&invalid_profile).is_err());

        let mut invalid_receipt = encoded.clone();
        let completed_receipt_offset = MCHV_HEADER_BYTES + 20 + 40 + 8 + 28 + 4 + 40;
        invalid_receipt[completed_receipt_offset] ^= 1;
        assert!(decode_mchv_frame(&invalid_receipt).is_err());

        let mut occurrence_reserved = encoded;
        *occurrence_reserved
            .last_mut()
            .expect("completed record frame is non-empty") = 1;
        assert!(decode_mchv_frame(&occurrence_reserved).is_err());
    }

    #[test]
    fn source_identity_rejects_wire_or_semantic_forgery() {
        let request = request(12_345, McloneOverworldSamplingTopology::Unbounded);
        let source = TerrainVegetationSourceIdentity::for_request(request).unwrap();
        let forged = TerrainVegetationSourceIdentity {
            compiler_source_revision: source.compiler_source_revision ^ 1,
            ..source
        };
        assert!(forged.validate_request(request).is_err());
        assert_ne!(forged.stable_fingerprint(), source.stable_fingerprint());
        assert_eq!(MCHV_WIRE_VERSION, 1);
    }
}
