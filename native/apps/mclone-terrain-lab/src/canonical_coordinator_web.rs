use std::collections::{BTreeSet, VecDeque};

use js_sys::{Function, Reflect, Uint8Array};
use mclone_terrain_view::canonical_terrain_chunk_order;
use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};

use crate::{
    canonical_web::{
        CanonicalChunkCoordinate, CanonicalPackedAcceptReport, CanonicalPackedPrepareReport,
        CanonicalTerrainLab,
    },
    canonical_worker_web::{
        CanonicalTerrainWorkerResponse, canonical_terrain_worker_begin_frame,
        canonical_terrain_worker_compile_frame, canonical_terrain_worker_init_frame,
    },
};

const CANONICAL_PENDING_HIGH_WATER: usize = 2;
const CANONICAL_WORKER_MAX_BATCH: usize = 16;

pub(crate) struct CanonicalCoverageRequest {
    pub center_x: i32,
    pub center_z: i32,
    pub radius: u32,
    pub profile: String,
    pub visual_profile: String,
    pub texture_presentation: String,
    pub seed: String,
    pub stage: String,
    pub water_visible: bool,
    pub vegetation_visible: bool,
    pub cache_enabled: bool,
    pub cache_epoch: u32,
    pub authored_bytes: Uint8Array,
    pub reference_bytes: Uint8Array,
    pub provisional_bytes: Uint8Array,
    pub diagnostic_bytes: Uint8Array,
}

enum PendingCanonicalAdmission {
    Warm(CanonicalChunkCoordinate),
    Mesh {
        coordinate: CanonicalChunkCoordinate,
        fingerprint: String,
        retained_dependency_chunks: u32,
        packed_sections: Uint8Array,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanonicalCoverageReport {
    epoch: u32,
    requested_chunks: usize,
    published_chunks: usize,
    queued_chunks: usize,
    resident_hits: usize,
    cache_hits: usize,
    admission_frames: usize,
    max_frame_admissions: usize,
    stale_chunks: usize,
    generation_ms: f64,
    worker_presentation_ms: f64,
    worker_mesh_ms: f64,
    worker_pack_ms: f64,
    worker_transfer_ms: f64,
    mesh_upload_ms: f64,
    main_decode_ms: f64,
    max_admission_ms: f64,
    mesh_target_chunks: usize,
    warm_hits: usize,
    warm_chunks: usize,
    first_chunk_ms: Option<f64>,
    complete_ms: Option<f64>,
    vertex_count: u32,
    index_count: u32,
    retained_dependency_chunks: u32,
    cached_chunks: u32,
    resident_raw_bytes: u64,
    cache_raw_bytes: f64,
    resident_mesh_used_bytes: u64,
    tracked_bytes: f64,
    complete: bool,
    needs_pump: bool,
    render_changed: bool,
    error: Option<String>,
}

pub(crate) struct CanonicalTerrainWorkerCoordinator {
    transport: JsValue,
    epoch: u32,
    resident_identity: Option<String>,
    presentation_identity: Option<String>,
    worker_ready: bool,
    water_visible: bool,
    vegetation_visible: bool,
    cache_enabled: bool,
    begin_sent: bool,
    session_began: bool,
    worker_in_flight: bool,
    coordinates: Vec<CanonicalChunkCoordinate>,
    missing: Vec<CanonicalChunkCoordinate>,
    next_missing_index: usize,
    batch_index: u32,
    pending: VecDeque<PendingCanonicalAdmission>,
    resident: BTreeSet<(i32, i32)>,
    request_started_ms: f64,
    report: CanonicalCoverageReport,
}

impl CanonicalTerrainWorkerCoordinator {
    pub(crate) fn new(transport: JsValue) -> Self {
        Self {
            transport,
            epoch: 0,
            resident_identity: None,
            presentation_identity: None,
            worker_ready: false,
            water_visible: true,
            vegetation_visible: true,
            cache_enabled: true,
            begin_sent: false,
            session_began: false,
            worker_in_flight: false,
            coordinates: Vec::new(),
            missing: Vec::new(),
            next_missing_index: 0,
            batch_index: 0,
            pending: VecDeque::new(),
            resident: BTreeSet::new(),
            request_started_ms: 0.0,
            report: empty_report(),
        }
    }

    pub(crate) fn begin(
        &mut self,
        renderer: &mut CanonicalTerrainLab,
        request: CanonicalCoverageRequest,
    ) -> Result<CanonicalCoverageReport, String> {
        self.epoch = self.epoch.wrapping_add(1).max(1);
        let resident_identity = format!(
            "{}:{}:{}:{}:{}:{}:{}",
            request.profile,
            request.seed,
            request.stage,
            request.visual_profile,
            request.texture_presentation,
            request.cache_epoch,
            request.cache_enabled
        );
        let presentation_identity =
            format!("{}:{}", request.water_visible, request.vegetation_visible);
        let hard_reset = self.resident_identity.as_deref() != Some(resident_identity.as_str());
        let presentation_reset =
            self.presentation_identity.as_deref() != Some(presentation_identity.as_str());
        self.resident_identity = Some(resident_identity);
        self.presentation_identity = Some(presentation_identity);
        if hard_reset || presentation_reset {
            renderer
                .reset_profile(request.seed.clone(), request.profile.clone())
                .map_err(js_message)?;
            self.resident.clear();
        }

        self.coordinates =
            canonical_terrain_chunk_order(request.center_x, request.center_z, request.radius)
                .into_iter()
                .map(|position| CanonicalChunkCoordinate {
                    chunk_x: position.x,
                    chunk_z: position.z,
                })
                .collect();
        let desired = self
            .coordinates
            .iter()
            .map(CanonicalChunkCoordinate::tuple)
            .collect::<BTreeSet<_>>();
        self.resident.retain(|position| desired.contains(position));
        let coordinates_json =
            serde_json::to_string(&self.coordinates).map_err(|error| error.to_string())?;
        let prepared_json = renderer
            .prepare_packed_chunks(coordinates_json)
            .map_err(js_message)?;
        let prepared = serde_json::from_str::<CanonicalPackedPrepareReport>(&prepared_json)
            .map_err(|error| format!("invalid canonical prepare report: {error}"))?;
        let warm = prepared
            .warm_available
            .iter()
            .map(CanonicalChunkCoordinate::tuple)
            .collect::<BTreeSet<_>>();

        self.pending.clear();
        self.missing.clear();
        for coordinate in &self.coordinates {
            let position = coordinate.tuple();
            if self.resident.contains(&position) {
                continue;
            }
            if warm.contains(&position) {
                self.pending
                    .push_back(PendingCanonicalAdmission::Warm(*coordinate));
            } else {
                self.missing.push(*coordinate);
            }
        }
        self.next_missing_index = 0;
        self.batch_index = 0;
        self.water_visible = request.water_visible;
        self.vegetation_visible = request.vegetation_visible;
        self.cache_enabled = request.cache_enabled;
        self.begin_sent = false;
        self.session_began = false;
        self.worker_in_flight = false;
        self.request_started_ms = now_ms();
        self.report = CanonicalCoverageReport {
            epoch: self.epoch,
            requested_chunks: self.coordinates.len(),
            published_chunks: self.resident.len(),
            queued_chunks: self.coordinates.len().saturating_sub(self.resident.len()),
            resident_hits: self.resident.len(),
            cache_hits: 0,
            admission_frames: 0,
            max_frame_admissions: 0,
            stale_chunks: 0,
            generation_ms: 0.0,
            worker_presentation_ms: 0.0,
            worker_mesh_ms: 0.0,
            worker_pack_ms: 0.0,
            worker_transfer_ms: 0.0,
            mesh_upload_ms: 0.0,
            main_decode_ms: 0.0,
            max_admission_ms: 0.0,
            mesh_target_chunks: 0,
            warm_hits: 0,
            warm_chunks: prepared.warm_chunks,
            first_chunk_ms: (!self.resident.is_empty()).then_some(0.0),
            complete_ms: None,
            vertex_count: prepared.vertex_count,
            index_count: prepared.index_count,
            retained_dependency_chunks: 0,
            cached_chunks: 0,
            resident_raw_bytes: 0,
            cache_raw_bytes: 0.0,
            resident_mesh_used_bytes: prepared.resident_mesh_used_bytes,
            tracked_bytes: prepared.resident_mesh_used_bytes as f64,
            complete: false,
            needs_pump: true,
            render_changed: false,
            error: None,
        };

        if hard_reset {
            self.worker_ready = false;
            let frame = canonical_terrain_worker_init_frame(
                self.epoch,
                request.authored_bytes,
                request.reference_bytes,
                request.provisional_bytes,
                request.diagnostic_bytes,
                request.visual_profile,
                request.texture_presentation,
                request.seed,
                request.profile,
                request.stage,
            )
            .map_err(js_message)?;
            self.post(frame)?;
        } else if self.worker_ready {
            self.send_begin(
                request.water_visible,
                request.vegetation_visible,
                request.cache_enabled,
            )?;
        }
        self.finish_if_ready();
        Ok(self.report.clone())
    }

    pub(crate) fn pump(
        &mut self,
        renderer: &mut CanonicalTerrainLab,
    ) -> Result<CanonicalCoverageReport, String> {
        self.report.render_changed = false;
        self.poll_responses()?;
        if self.report.error.is_none() {
            self.admit_one(renderer)?;
            self.pump_worker()?;
            self.finish_if_ready();
        }
        self.report.needs_pump = !self.report.complete && self.report.error.is_none();
        Ok(self.report.clone())
    }

    pub(crate) fn terminate(&self) -> Result<(), String> {
        let method = method(&self.transport, "terminate")?;
        method
            .call0(&self.transport)
            .map(|_| ())
            .map_err(js_message)
    }

    fn poll_responses(&mut self) -> Result<(), String> {
        let poll = method(&self.transport, "poll")?;
        for _ in 0..64 {
            let event = poll.call0(&self.transport).map_err(js_message)?;
            if event.is_null() || event.is_undefined() {
                break;
            }
            match string_property(&event, "kind")?.as_str() {
                "message" => {
                    let message =
                        Reflect::get(&event, &JsValue::from_str("data")).map_err(js_message)?;
                    self.accept_response(message)?;
                }
                "error" => {
                    let message = string_property(&event, "message")?;
                    self.report.error = Some(message);
                }
                other => {
                    self.report.error = Some(format!("unsupported browser Worker event {other:?}"));
                }
            }
        }
        Ok(())
    }

    fn accept_response(&mut self, message: JsValue) -> Result<(), String> {
        let response = CanonicalTerrainWorkerResponse::decode(message).map_err(js_message)?;
        let kind = response.kind().map_err(js_message)?;
        let response_epoch = response.epoch().map_err(js_message)?;
        if kind == "ready" {
            self.worker_ready = true;
            if !self.begin_sent {
                self.send_begin(
                    self.water_visible,
                    self.vegetation_visible,
                    self.cache_enabled,
                )?;
            }
            return Ok(());
        }
        if response_epoch != self.epoch || kind == "stale" {
            self.report.stale_chunks += 1;
            return Ok(());
        }
        match kind.as_str() {
            "began" => {
                self.session_began = true;
                self.report.cached_chunks = response.raw_cache_chunks().map_err(js_message)?;
                self.report.cache_raw_bytes = response.raw_cache_bytes().map_err(js_message)?;
            }
            "error" => {
                self.report.error = Some(response.message().map_err(js_message)?);
            }
            "batch" => {
                self.worker_in_flight = false;
                self.report.generation_ms += response.generation_ms().map_err(js_message)?;
                self.report.worker_presentation_ms +=
                    response.presentation_ms().map_err(js_message)?;
                self.report.worker_mesh_ms += response.mesh_ms().map_err(js_message)?;
                self.report.worker_pack_ms += response.pack_ms().map_err(js_message)?;
                self.report.worker_transfer_ms += response.transfer_ms().map_err(js_message)?;
                self.report.mesh_target_chunks +=
                    response.deduplicated_target_chunks().map_err(js_message)? as usize;
                self.report.cached_chunks = response.raw_cache_chunks().map_err(js_message)?;
                self.report.cache_raw_bytes = response.raw_cache_bytes().map_err(js_message)?;
                let admission_count = response.admission_count().map_err(js_message)?;
                for index in 0..admission_count {
                    let raw_cache_hit = response
                        .admission_raw_cache_hit(index)
                        .map_err(js_message)?;
                    if raw_cache_hit {
                        self.report.cache_hits += 1;
                    }
                    self.pending.push_back(PendingCanonicalAdmission::Mesh {
                        coordinate: CanonicalChunkCoordinate {
                            chunk_x: response.admission_chunk_x(index).map_err(js_message)?,
                            chunk_z: response.admission_chunk_z(index).map_err(js_message)?,
                        },
                        fingerprint: response.admission_fingerprint(index).map_err(js_message)?,
                        retained_dependency_chunks: response
                            .admission_retained_dependency_chunks(index)
                            .map_err(js_message)?,
                        packed_sections: response
                            .admission_packed_sections(index)
                            .map_err(js_message)?,
                    });
                }
            }
            other => {
                self.report.error =
                    Some(format!("unsupported canonical Worker response {other:?}"));
            }
        }
        self.update_tracked_bytes();
        Ok(())
    }

    fn send_begin(
        &mut self,
        water_visible: bool,
        vegetation_visible: bool,
        cache_enabled: bool,
    ) -> Result<(), String> {
        if self.begin_sent {
            return Ok(());
        }
        let coordinates_json =
            serde_json::to_string(&self.coordinates).map_err(|error| error.to_string())?;
        let frame = canonical_terrain_worker_begin_frame(
            self.epoch,
            coordinates_json,
            water_visible,
            vegetation_visible,
            cache_enabled,
        )
        .map_err(js_message)?;
        self.post(frame)?;
        self.begin_sent = true;
        Ok(())
    }

    fn pump_worker(&mut self) -> Result<(), String> {
        if !self.session_began
            || self.worker_in_flight
            || self.pending.len() >= CANONICAL_PENDING_HIGH_WATER
            || self.next_missing_index >= self.missing.len()
        {
            return Ok(());
        }
        let remaining = self.missing.len() - self.next_missing_index;
        let batch_size = CANONICAL_WORKER_MAX_BATCH
            .min(2_usize.saturating_pow(self.batch_index))
            .min(remaining);
        self.batch_index = self.batch_index.saturating_add(1);
        let batch = &self.missing[self.next_missing_index..self.next_missing_index + batch_size];
        self.next_missing_index += batch_size;
        let frame = canonical_terrain_worker_compile_frame(
            self.epoch,
            serde_json::to_string(batch).map_err(|error| error.to_string())?,
        )
        .map_err(js_message)?;
        self.post(frame)?;
        self.worker_in_flight = true;
        Ok(())
    }

    fn admit_one(&mut self, renderer: &mut CanonicalTerrainLab) -> Result<(), String> {
        let Some(admission) = self.pending.pop_front() else {
            return Ok(());
        };
        let started = now_ms();
        let (coordinate, retained_dependency_chunks, accepted_json, warm) = match admission {
            PendingCanonicalAdmission::Warm(coordinate) => {
                let accepted = renderer
                    .activate_packed_chunk(coordinate.chunk_x, coordinate.chunk_z)
                    .map_err(js_message)?;
                (coordinate, 0, accepted, true)
            }
            PendingCanonicalAdmission::Mesh {
                coordinate,
                fingerprint,
                retained_dependency_chunks,
                packed_sections,
            } => {
                let accepted = renderer
                    .accept_packed_mesh(
                        coordinate.chunk_x,
                        coordinate.chunk_z,
                        fingerprint,
                        packed_sections,
                    )
                    .map_err(js_message)?;
                (coordinate, retained_dependency_chunks, accepted, false)
            }
        };
        let accepted = serde_json::from_str::<CanonicalPackedAcceptReport>(&accepted_json)
            .map_err(|error| format!("invalid canonical accept report: {error}"))?;
        self.resident.insert(coordinate.tuple());
        self.report.published_chunks = self.resident.len();
        self.report.queued_chunks = self
            .report
            .requested_chunks
            .saturating_sub(self.report.published_chunks);
        self.report.admission_frames += 1;
        self.report.max_frame_admissions = 1;
        self.report.max_admission_ms = self.report.max_admission_ms.max(now_ms() - started);
        if warm {
            self.report.warm_hits += 1;
        } else {
            self.report.retained_dependency_chunks = retained_dependency_chunks;
        }
        self.report.mesh_upload_ms += accepted.mesh_upload_ms;
        self.report.main_decode_ms += accepted.decode_ms;
        self.report.vertex_count = accepted.vertex_count;
        self.report.index_count = accepted.index_count;
        self.report.warm_chunks = accepted.warm_chunks;
        self.report.resident_mesh_used_bytes = accepted.resident_mesh_used_bytes;
        self.report
            .first_chunk_ms
            .get_or_insert_with(|| now_ms() - self.request_started_ms);
        self.report.render_changed = true;
        self.update_tracked_bytes();
        Ok(())
    }

    fn finish_if_ready(&mut self) {
        if self.report.error.is_some()
            || self.report.complete
            || !self.session_began
            || self.worker_in_flight
            || !self.pending.is_empty()
            || self.next_missing_index < self.missing.len()
        {
            return;
        }
        self.report.complete = true;
        self.report.needs_pump = false;
        self.report.queued_chunks = 0;
        self.report.complete_ms = Some(now_ms() - self.request_started_ms);
    }

    fn update_tracked_bytes(&mut self) {
        self.report.tracked_bytes = self.report.resident_raw_bytes as f64
            + self.report.cache_raw_bytes
            + self.report.resident_mesh_used_bytes as f64;
    }

    fn post(&self, frame: JsValue) -> Result<(), String> {
        method(&self.transport, "post")?
            .call1(&self.transport, &frame)
            .map(|_| ())
            .map_err(js_message)
    }
}

impl CanonicalChunkCoordinate {
    pub(crate) const fn tuple(&self) -> (i32, i32) {
        (self.chunk_x, self.chunk_z)
    }
}

fn empty_report() -> CanonicalCoverageReport {
    CanonicalCoverageReport {
        epoch: 0,
        requested_chunks: 0,
        published_chunks: 0,
        queued_chunks: 0,
        resident_hits: 0,
        cache_hits: 0,
        admission_frames: 0,
        max_frame_admissions: 0,
        stale_chunks: 0,
        generation_ms: 0.0,
        worker_presentation_ms: 0.0,
        worker_mesh_ms: 0.0,
        worker_pack_ms: 0.0,
        worker_transfer_ms: 0.0,
        mesh_upload_ms: 0.0,
        main_decode_ms: 0.0,
        max_admission_ms: 0.0,
        mesh_target_chunks: 0,
        warm_hits: 0,
        warm_chunks: 0,
        first_chunk_ms: None,
        complete_ms: None,
        vertex_count: 0,
        index_count: 0,
        retained_dependency_chunks: 0,
        cached_chunks: 0,
        resident_raw_bytes: 0,
        cache_raw_bytes: 0.0,
        resident_mesh_used_bytes: 0,
        tracked_bytes: 0.0,
        complete: false,
        needs_pump: false,
        render_changed: false,
        error: None,
    }
}

fn method(value: &JsValue, name: &str) -> Result<Function, String> {
    Reflect::get(value, &JsValue::from_str(name))
        .map_err(js_message)?
        .dyn_into::<Function>()
        .map_err(|_| format!("browser Worker transport has no {name}() method"))
}

fn string_property(value: &JsValue, name: &str) -> Result<String, String> {
    Reflect::get(value, &JsValue::from_str(name))
        .map_err(js_message)?
        .as_string()
        .ok_or_else(|| format!("browser Worker event {name:?} is not a string"))
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

fn js_message(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            Reflect::get(&value, &JsValue::from_str("message"))
                .ok()
                .and_then(|message| message.as_string())
        })
        .unwrap_or_else(|| "browser Worker operation failed".to_owned())
}
