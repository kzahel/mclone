//! Worker-resident Rust authority for render-section and far-LOD compilation.
//!
//! The browser Worker loads this crate's independent Wasm instance and forwards
//! opaque structured-clone frames here. This actor owns asset/session lifetime,
//! request dispatch, diagnostics, and the external shared-buffer publication
//! protocol. JavaScript retains only browser module loading and `postMessage`.

use std::collections::BTreeMap;

use js_sys::{Array, ArrayBuffer, Int32Array, Object, Reflect, SharedArrayBuffer, Uint8Array};
use wasm_bindgen::{JsCast, prelude::*};

use crate::web_canvas::{WebRenderCompilerSession, packed_compile_report_summary_from_bytes};
use crate::web_render_compiler_abi::*;

#[wasm_bindgen]
pub struct WebRenderWorkerActor {
    compiler_template: Option<WebRenderCompilerSession>,
    compiler_sessions: BTreeMap<String, WebRenderCompilerSession>,
    init_request_id: JsValue,
    init_error: Option<String>,
    worker_compile_count: usize,
    worker_asset_load_count: usize,
    worker_asset_pack_init_byte_length: usize,
    worker_asset_pack_file_count: usize,
    worker_asset_epoch: u64,
}

#[wasm_bindgen]
impl WebRenderWorkerActor {
    /// Build the worker-side actor from the Rust-authored initialization frame.
    /// Initialization failures remain actor state so JavaScript can publish the
    /// normal Rust-authored readiness report without reconstructing its schema.
    #[wasm_bindgen(constructor)]
    pub fn new(init_message: JsValue) -> Self {
        let init_request_id = property(&init_message, "requestId");
        let worker_asset_epoch = number(&init_message, "assetEpoch").max(0.0) as u64;
        match compiler_template_from_init(&init_message) {
            Ok(template) => Self {
                worker_asset_load_count: template.asset_load_count(),
                worker_asset_pack_init_byte_length: template.asset_pack_byte_length(),
                worker_asset_pack_file_count: template.asset_pack_file_count(),
                compiler_template: Some(template),
                compiler_sessions: BTreeMap::new(),
                init_request_id,
                init_error: None,
                worker_compile_count: 0,
                worker_asset_epoch,
            },
            Err(error) => Self {
                compiler_template: None,
                compiler_sessions: BTreeMap::new(),
                init_request_id,
                init_error: Some(js_error_string(error)),
                worker_compile_count: 0,
                worker_asset_load_count: 0,
                worker_asset_pack_init_byte_length: 0,
                worker_asset_pack_file_count: 0,
                worker_asset_epoch,
            },
        }
    }

    #[wasm_bindgen(js_name = readyReport)]
    pub fn ready_report(&self) -> Result<JsValue, JsValue> {
        self.build_ready_report().map_err(JsValue::from)
    }

    /// Dispatch one opaque coordinator frame. `undefined` means the frame was a
    /// one-way world-release notification and the browser broker should publish
    /// nothing. Compile failures are returned as ordinary actor reports after
    /// marking the shared result arena failed.
    #[wasm_bindgen(js_name = handleMessage)]
    pub fn handle_message(&mut self, message: JsValue) -> Result<JsValue, JsValue> {
        match string(&message, "kind").as_deref() {
            Some("release-render-compiler-world") => {
                self.compiler_sessions
                    .remove(&string(&message, "worldInstanceId").unwrap_or_default());
                Ok(JsValue::UNDEFINED)
            }
            Some("compile-render-sections") => {
                self.worker_compile_count = self.worker_compile_count.saturating_add(1);
                match self.compile_report(&message) {
                    Ok(report) => Ok(report),
                    Err(reason) => {
                        let _ = mark_shared_compile_result_failed(&message);
                        self.build_compile_failure_report(&message, reason)
                            .map_err(JsValue::from)
                    }
                }
            }
            other => self
                .build_compile_failure_report(
                    &message,
                    format!("unexpected render compiler message kind {other:?}"),
                )
                .map_err(JsValue::from),
        }
    }
}

impl WebRenderWorkerActor {
    fn build_ready_report(&self) -> Result<JsValue, String> {
        let report = Object::new();
        set_bool(&report, "ok", self.init_error.is_none())?;
        set_string(&report, "kind", "render-compiler-ready")?;
        set_value(&report, "requestId", &self.init_request_id)?;
        self.set_common_metrics(&report)?;
        set_bool(
            &report,
            "persistentAssetCatalog",
            self.compiler_template.is_some(),
        )?;
        if let Some(reason) = &self.init_error {
            set_string(&report, "reason", reason)?;
        }
        Ok(report.into())
    }

    fn compile_report(&mut self, message: &JsValue) -> Result<JsValue, String> {
        let world_instance_id = string(message, "worldInstanceId").unwrap_or_default();
        if world_instance_id.is_empty() {
            return Err("render compile request is missing its world instance id".to_owned());
        }
        if !self.compiler_sessions.contains_key(&world_instance_id) {
            let session = self
                .compiler_template
                .as_ref()
                .ok_or_else(|| "render compiler assets are not initialized".to_owned())?
                .fork_world_session();
            self.compiler_sessions
                .insert(world_instance_id.clone(), session);
        }

        let work_kind = WorkKind::from_message(message);
        let target_sections = normalize_target_sections(&property(message, "targetSections"));
        let center_x = number(message, "centerX") as i32;
        let center_z = number(message, "centerZ") as i32;
        let radius_chunks = number(message, "radiusChunks").max(0.0) as u32;
        let request_asset_pack_byte_length = byte_length(&property(message, "assetPack"));
        let request_target_sections_byte_length = target_sections.byte_length();
        let shared_input = shared_input(message)?;
        let request_snapshot_input_byte_length = shared_input
            .as_ref()
            .map_or(0, |input| input.bytes.byte_length());
        let shared_input_buffer_capacity_bytes = shared_input
            .as_ref()
            .map_or(0, |input| input.capacity_bytes);
        let snapshot_input_compile_used = work_kind == WorkKind::RenderSections
            && shared_input.is_some()
            && target_sections.length() > 0;
        let generated_targeted_compile_used = work_kind == WorkKind::RenderSections
            && !snapshot_input_compile_used
            && target_sections.length() > 0;

        let (packed, summary, upserts, evictions, mirror_chunks, grass_patches_requested) = {
            let session = self
                .compiler_sessions
                .get_mut(&world_instance_id)
                .expect("world compiler session was inserted above");
            let packed = match work_kind {
                WorkKind::FarLod => session
                    .compile_far_lod_tile_bytes(
                        string(message, "farLodSeed").unwrap_or_else(|| "0".to_owned()),
                        string(message, "farLodGenerationProfile")
                            .unwrap_or_else(|| "overworld".to_owned()),
                        number(message, "farLodChunkX") as i32,
                        number(message, "farLodChunkZ") as i32,
                        nonzero_number(message, "farLodLevel", 1.0) as u8,
                        nonzero_number(message, "farLodSampleSpacingBlocks", 4.0) as u32,
                        nonzero_number(message, "farLodWestSampleSpacingBlocks", 4.0) as u32,
                        nonzero_number(message, "farLodEastSampleSpacingBlocks", 4.0) as u32,
                        nonzero_number(message, "farLodNorthSampleSpacingBlocks", 4.0) as u32,
                        nonzero_number(message, "farLodSouthSampleSpacingBlocks", 4.0) as u32,
                    )
                    .map_err(js_error_string)?,
                WorkKind::RenderSections if snapshot_input_compile_used => session
                    .compile_snapshot_sections_for_targets_bytes(
                        &shared_input
                            .as_ref()
                            .expect("snapshot compile has shared input")
                            .bytes,
                        &target_sections,
                    )
                    .map_err(js_error_string)?,
                WorkKind::RenderSections if generated_targeted_compile_used => {
                    let targets =
                        crate::web_canvas::render_section_keys_from_int32_array(&target_sections)?;
                    session
                        .compile_generated_chunk_sections_bytes(
                            center_x,
                            center_z,
                            radius_chunks,
                            Some(&targets),
                        )
                        .map_err(js_error_string)?
                }
                WorkKind::RenderSections => session
                    .compile_generated_chunk_sections_bytes(center_x, center_z, radius_chunks, None)
                    .map_err(js_error_string)?,
            };
            let summary = match work_kind {
                WorkKind::FarLod => far_lod_summary(packed.len())?,
                WorkKind::RenderSections => packed_compile_report_summary_from_bytes(&packed)?,
            };
            (
                packed,
                summary,
                session.last_delta_upsert_count(),
                session.last_delta_eviction_count(),
                session.mirror_chunk_count(),
                session.grass_patches_enabled(),
            )
        };

        let response = write_shared_compile_result(message, &packed)?;
        let report = Object::new();
        set_bool(&report, "ok", true)?;
        copy_property(&report, message, "requestId")?;
        copy_property(&report, message, "clientRequestId")?;
        set_string(&report, "worldInstanceId", &world_instance_id)?;
        copy_property(&report, message, "worldPriority")?;
        set_string(&report, "workKind", work_kind.label())?;
        self.set_common_metrics(&report)?;
        set_bool(&report, "persistentAssetCatalog", true)?;
        set_number(
            &report,
            "compilerWorldSessionCount",
            self.compiler_sessions.len() as f64,
        )?;
        set_number(
            &report,
            "requestAssetPackByteLength",
            f64::from(request_asset_pack_byte_length),
        )?;
        set_number(
            &report,
            "requestTargetSectionsByteLength",
            f64::from(request_target_sections_byte_length),
        )?;
        set_number(
            &report,
            "requestSnapshotInputByteLength",
            f64::from(request_snapshot_input_byte_length),
        )?;
        set_number(
            &report,
            "requestByteLength",
            f64::from(
                request_asset_pack_byte_length
                    .saturating_add(request_target_sections_byte_length)
                    .saturating_add(request_snapshot_input_byte_length),
            ),
        )?;
        set_number(
            &report,
            "transferredRequestByteLength",
            f64::from(request_asset_pack_byte_length),
        )?;
        set_number(&report, "transferredResponseByteLength", 0.0)?;
        set_bool(&report, "sharedInputBufferUsed", shared_input.is_some())?;
        set_number(
            &report,
            "sharedInputByteLength",
            f64::from(request_snapshot_input_byte_length),
        )?;
        set_number(
            &report,
            "sharedInputBufferCapacityBytes",
            f64::from(shared_input_buffer_capacity_bytes),
        )?;
        copy_number_property(&report, message, "snapshotInputChunkCount")?;
        copy_number_property(&report, message, "snapshotInputClonedColumnCount")?;
        set_number(&report, "snapshotInputUpsertCount", upserts as f64)?;
        set_number(&report, "snapshotInputEvictionCount", evictions as f64)?;
        set_number(&report, "snapshotMirrorChunkCount", mirror_chunks as f64)?;
        set_bool(&report, "grassPatchesRequested", grass_patches_requested)?;
        set_bool(
            &report,
            "snapshotInputCompileUsed",
            snapshot_input_compile_used,
        )?;
        set_bool(
            &report,
            "generatedViewFallbackUsed",
            work_kind == WorkKind::RenderSections && !snapshot_input_compile_used,
        )?;
        set_bool(&report, "sharedResultBufferUsed", true)?;
        set_number(
            &report,
            "sharedResultByteLength",
            f64::from(response.byte_length),
        )?;
        set_number(
            &report,
            "sharedResultBufferCapacityBytes",
            f64::from(response.capacity_bytes),
        )?;
        set_bool(&report, "sharedResultOverflow", response.overflow)?;
        set_number(&report, "centerX", f64::from(center_x))?;
        set_number(&report, "centerZ", f64::from(center_z))?;
        set_number(&report, "radiusChunks", f64::from(radius_chunks))?;
        set_number(
            &report,
            "targetSectionCount",
            f64::from(target_sections.length() / 3),
        )?;
        set_bool(
            &report,
            "targetedCompileUsed",
            snapshot_input_compile_used || generated_targeted_compile_used,
        )?;
        set_bool(&report, "farLodCompileUsed", work_kind == WorkKind::FarLod)?;
        set_value(&report, "summary", &summary)?;
        set_value(&report, "sharedResultBuffer", response.buffer.as_ref())?;
        Ok(report.into())
    }

    fn build_compile_failure_report(
        &self,
        message: &JsValue,
        reason: String,
    ) -> Result<JsValue, String> {
        let report = Object::new();
        set_bool(&report, "ok", false)?;
        copy_property(&report, message, "requestId")?;
        copy_property(&report, message, "clientRequestId")?;
        copy_property(&report, message, "worldInstanceId")?;
        copy_property(&report, message, "worldPriority")?;
        self.set_common_metrics(&report)?;
        set_bool(
            &report,
            "persistentAssetCatalog",
            self.compiler_template.is_some(),
        )?;
        set_number(
            &report,
            "compilerWorldSessionCount",
            self.compiler_sessions.len() as f64,
        )?;
        set_string(&report, "reason", &reason)?;
        Ok(report.into())
    }

    fn set_common_metrics(&self, report: &Object) -> Result<(), String> {
        set_string(report, "transportKind", RENDER_COMPILER_TRANSPORT_KIND)?;
        set_bool(report, "sharedMemorySupported", shared_memory_supported())?;
        set_number(report, "workerWasmInitCount", 1.0)?;
        set_number(
            report,
            "workerCompileCount",
            self.worker_compile_count as f64,
        )?;
        set_number(
            report,
            "workerAssetLoadCount",
            self.worker_asset_load_count as f64,
        )?;
        set_number(
            report,
            "workerAssetPackInitByteLength",
            self.worker_asset_pack_init_byte_length as f64,
        )?;
        set_number(
            report,
            "workerAssetPackFileCount",
            self.worker_asset_pack_file_count as f64,
        )?;
        set_number(report, "assetEpoch", self.worker_asset_epoch as f64)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkKind {
    RenderSections,
    FarLod,
}

impl WorkKind {
    fn from_message(message: &JsValue) -> Self {
        if string(message, "workKind").as_deref() == Some("far-lod") {
            Self::FarLod
        } else {
            Self::RenderSections
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::RenderSections => "render-sections",
            Self::FarLod => "far-lod",
        }
    }
}

struct SharedInput {
    bytes: Uint8Array,
    capacity_bytes: u32,
}

struct SharedCompileResult {
    byte_length: u32,
    capacity_bytes: u32,
    overflow: bool,
    buffer: SharedArrayBuffer,
}

fn compiler_template_from_init(message: &JsValue) -> Result<WebRenderCompilerSession, JsValue> {
    let authored = uint8_array_property(message, "authoredPack");
    let reference = uint8_array_property(message, "referencePack");
    let fallback = uint8_array_property(message, "fallbackPack");
    match (authored, reference, fallback) {
        (Some(authored), Some(reference), Some(fallback)) => {
            WebRenderCompilerSession::new_selected(
                authored,
                reference,
                fallback,
                boolean(message, "authoredEnabled"),
                boolean(message, "referenceEnabled"),
            )
        }
        _ => {
            WebRenderCompilerSession::new(uint8_array_property(message, "assetPack").ok_or_else(
                || JsValue::from_str("render compiler init is missing its asset pack"),
            )?)
        }
    }
}

fn shared_input(message: &JsValue) -> Result<Option<SharedInput>, String> {
    if !shared_memory_supported() {
        return Ok(None);
    }
    let Some(buffer) = shared_array_buffer_property(message, "sharedInputBuffer") else {
        return Ok(None);
    };
    let control = shared_control(
        message,
        "sharedInputControlBuffer",
        RENDER_COMPILER_SHARED_INPUT_CONTROL_WORDS,
    );
    let control_byte_length = control
        .as_ref()
        .map(|control| atomic_load(control, RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX))
        .transpose()?
        .unwrap_or(0);
    let control_status = control
        .as_ref()
        .map(|control| atomic_load(control, RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX))
        .transpose()?
        .unwrap_or(RENDER_COMPILER_SHARED_INPUT_READY);
    let message_byte_length = number(message, "sharedInputByteLength") as i64;
    let byte_length = if message_byte_length != 0 {
        message_byte_length
    } else {
        i64::from(control_byte_length)
    };
    if control_status != RENDER_COMPILER_SHARED_INPUT_READY
        || byte_length <= 0
        || byte_length > i64::from(buffer.byte_length())
    {
        return Ok(None);
    }
    let byte_length = byte_length as u32;
    Ok(Some(SharedInput {
        bytes: Uint8Array::new_with_byte_offset_and_length(buffer.as_ref(), 0, byte_length),
        capacity_bytes: buffer.byte_length(),
    }))
}

fn write_shared_compile_result(
    message: &JsValue,
    packed: &[u8],
) -> Result<SharedCompileResult, String> {
    let packed_byte_length = u32::try_from(packed.len()).map_err(|_| {
        format!(
            "render compile result of {} bytes exceeded u32",
            packed.len()
        )
    })?;
    let provided = shared_array_buffer_property(message, "sharedResultResponseBuffer")
        .ok_or_else(|| "render compile request has no shared result response buffer".to_owned())?;
    let overflow = provided.byte_length() < packed_byte_length;
    let buffer = if overflow {
        SharedArrayBuffer::new(packed_byte_length)
    } else {
        provided
    };
    if packed_byte_length > 0 {
        let view =
            Uint8Array::new_with_byte_offset_and_length(buffer.as_ref(), 0, packed_byte_length);
        view.copy_from(packed);
    }
    if let Some(control) = shared_control(
        message,
        "sharedResultControlBuffer",
        RENDER_COMPILER_SHARED_RESULT_CONTROL_WORDS,
    ) {
        atomic_store(
            &control,
            RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX,
            packed_byte_length as i32,
        )?;
        atomic_store(
            &control,
            RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX,
            buffer.byte_length() as i32,
        )?;
        atomic_store(
            &control,
            RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX,
            if overflow {
                RENDER_COMPILER_SHARED_RESULT_OVERFLOW
            } else {
                RENDER_COMPILER_SHARED_RESULT_COMPLETE
            },
        )?;
        atomic_notify(&control, RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX)?;
    }
    Ok(SharedCompileResult {
        byte_length: packed_byte_length,
        capacity_bytes: buffer.byte_length(),
        overflow,
        buffer,
    })
}

fn mark_shared_compile_result_failed(message: &JsValue) -> Result<(), String> {
    let Some(control) = shared_control(
        message,
        "sharedResultControlBuffer",
        RENDER_COMPILER_SHARED_RESULT_CONTROL_WORDS,
    ) else {
        return Ok(());
    };
    atomic_store(
        &control,
        RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX,
        RENDER_COMPILER_SHARED_RESULT_FAILED,
    )?;
    atomic_notify(&control, RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX)
}

fn shared_control(message: &JsValue, key: &str, minimum_words: u32) -> Option<Int32Array> {
    let buffer = shared_array_buffer_property(message, key)?;
    (buffer.byte_length() >= minimum_words * 4).then(|| Int32Array::new(buffer.as_ref()))
}

fn normalize_target_sections(value: &JsValue) -> Int32Array {
    if let Some(array) = value.dyn_ref::<Int32Array>() {
        return array.clone();
    }
    if Array::is_array(value) {
        let values = Array::from(value)
            .iter()
            .map(|value| value.as_f64().unwrap_or(0.0) as i32)
            .collect::<Vec<_>>();
        return Int32Array::from(values.as_slice());
    }
    if value.is_instance_of::<ArrayBuffer>() {
        return Int32Array::new(value);
    }
    if ArrayBuffer::is_view(value) {
        let buffer = property(value, "buffer");
        let byte_offset = number(value, "byteOffset").max(0.0) as u32;
        let length = number(value, "byteLength").max(0.0) as u32 / 4;
        return Int32Array::new_with_byte_offset_and_length(&buffer, byte_offset, length);
    }
    Int32Array::new_with_length(0)
}

fn far_lod_summary(packed_byte_length: usize) -> Result<JsValue, String> {
    let summary = Object::new();
    set_bool(&summary, "farLodTile", true)?;
    set_number(&summary, "packedByteLength", packed_byte_length as f64)?;
    Ok(summary.into())
}

fn property(value: &JsValue, key: &str) -> JsValue {
    Reflect::get(value, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
}

fn string(value: &JsValue, key: &str) -> Option<String> {
    property(value, key).as_string()
}

fn number(value: &JsValue, key: &str) -> f64 {
    property(value, key)
        .as_f64()
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
}

fn nonzero_number(value: &JsValue, key: &str, fallback: f64) -> f64 {
    let value = number(value, key);
    if value == 0.0 { fallback } else { value }
}

fn boolean(value: &JsValue, key: &str) -> bool {
    property(value, key).as_bool().unwrap_or(false)
}

fn byte_length(value: &JsValue) -> u32 {
    number(value, "byteLength").max(0.0) as u32
}

fn uint8_array_property(value: &JsValue, key: &str) -> Option<Uint8Array> {
    property(value, key).dyn_into::<Uint8Array>().ok()
}

fn shared_array_buffer_property(value: &JsValue, key: &str) -> Option<SharedArrayBuffer> {
    property(value, key).dyn_into::<SharedArrayBuffer>().ok()
}

fn set_value(object: &Object, key: &str, value: &JsValue) -> Result<(), String> {
    Reflect::set(object, &JsValue::from_str(key), value)
        .map(|_| ())
        .map_err(|error| format!("failed to set render-worker report field {key}: {error:?}"))
}

fn set_bool(object: &Object, key: &str, value: bool) -> Result<(), String> {
    set_value(object, key, &JsValue::from_bool(value))
}

fn set_number(object: &Object, key: &str, value: f64) -> Result<(), String> {
    set_value(object, key, &JsValue::from_f64(value))
}

fn set_string(object: &Object, key: &str, value: &str) -> Result<(), String> {
    set_value(object, key, &JsValue::from_str(value))
}

fn copy_property(object: &Object, source: &JsValue, key: &str) -> Result<(), String> {
    set_value(object, key, &property(source, key))
}

fn copy_number_property(object: &Object, source: &JsValue, key: &str) -> Result<(), String> {
    set_number(object, key, number(source, key))
}

fn js_error_string(error: JsValue) -> String {
    for key in ["stack", "message"] {
        if let Some(value) = string(&error, key) {
            return value;
        }
    }
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
