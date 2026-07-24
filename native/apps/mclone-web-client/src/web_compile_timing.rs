//! 070 Stage 3: the per-compile timing instrumentation, moved out of
//! `www/mclone-web-app.js` into Rust. The former JS bag (`createCompileTiming` /
//! `updateCompileTimingFrom*` / `normalizeRenderCompilerMetrics` /
//! `publicCompileTiming`) was a ~90-field record mutated in place across the compile
//! lifecycle, fed by three untyped JS report objects (the request doorbell, the
//! render-compile worker report, and the apply frame) and re-coerced with
//! `Number(...)`/`Boolean(...)` at every read. That coercion now lives here behind
//! typed accessors; `WebCompileTiming` owns the bag and emits the public snapshot the
//! HUD and the smoke harness read. The Rust render-worker coordinator now owns the
//! pending timing map and lifecycle measurements as well. The public snapshot shape is
//! contract — `assertCompileTimingDiagnostics` in `scripts/browser-smoke.mjs` checks
//! ~40 of its fields — so this remains a faithful port of the former JS coercion
//! semantics (`Number(x) || 0`, `|| null`, `Boolean(x)`, `String(x || "unknown")`, and
//! the two `sharedResult`/`transferredResponse` fallbacks).

use js_sys::{Object, Reflect};
use wasm_bindgen::prelude::*;

/// A per-compile timing record (formerly the JS `createCompileTiming` bag). Mutated across
/// the compile lifecycle from typed report readers, then projected to a rounded public
/// snapshot for `runtime.state`.
#[wasm_bindgen]
pub struct WebCompileTiming {
    sequence: f64,
    trigger: String,
    status: String,
    request_id: Option<f64>,
    target_center_x: Option<f64>,
    target_center_z: Option<f64>,
    loaded_center_before_x: Option<f64>,
    loaded_center_before_z: Option<f64>,
    loaded_center_after_x: Option<f64>,
    loaded_center_after_z: Option<f64>,
    started_at_ms: f64,
    finished_at_ms: Option<f64>,
    total_ms: f64,
    begin_request_ms: f64,
    worker_round_trip_ms: f64,
    decode_finish_apply_ms: f64,
    packed_byte_length: f64,
    // `renderCompilerMetrics` starts null in JS and becomes an object only once the worker
    // reports; this flag reproduces that null-until-reported behaviour in the snapshot.
    worker_reported: bool,
    // render-compiler metrics (mirror of `normalizeRenderCompilerMetrics`).
    render_compiler_transport_kind: String,
    render_compiler_shared_memory_supported: bool,
    render_compiler_worker_init_count: f64,
    render_compiler_worker_wasm_init_count: f64,
    render_compiler_worker_asset_load_count: f64,
    render_compiler_worker_asset_pack_init_byte_length: f64,
    render_compiler_worker_asset_pack_file_count: f64,
    render_compiler_persistent_asset_catalog: bool,
    render_compiler_compile_count: f64,
    render_compiler_worker_compile_count: f64,
    render_compiler_asset_pack_send_count: f64,
    render_compiler_request_asset_pack_byte_length: f64,
    render_compiler_request_target_sections_byte_length: f64,
    render_compiler_request_snapshot_input_byte_length: f64,
    render_compiler_request_byte_length: f64,
    render_compiler_transferred_request_byte_length: f64,
    render_compiler_transferred_response_byte_length: f64,
    render_compiler_transferred_request_byte_count: f64,
    render_compiler_transferred_response_byte_count: f64,
    render_compiler_shared_input_buffer_used: bool,
    render_compiler_shared_input_byte_length: f64,
    render_compiler_shared_input_buffer_capacity_bytes: f64,
    render_compiler_snapshot_input_chunk_count: f64,
    render_compiler_snapshot_input_cloned_column_count: f64,
    render_compiler_snapshot_input_compile_used: bool,
    render_compiler_generated_view_fallback_used: bool,
    render_compiler_shared_result_buffer_used: bool,
    render_compiler_shared_result_byte_length: f64,
    render_compiler_shared_result_buffer_capacity_bytes: f64,
    render_compiler_shared_result_overflow: bool,
    render_compiler_shared_result_response_count: f64,
    render_compiler_shared_result_byte_count: f64,
    render_compiler_shared_result_overflow_count: f64,
    // compile scope / apply results.
    chunk_view_update_count: f64,
    center_loaded: bool,
    runner_settled: bool,
    view_dirty_chunk_count: f64,
    view_removal_chunk_count: f64,
    loaded_dirty_chunk_count: f64,
    removal_dirty_chunk_count: f64,
    stale_dirty_chunk_count: f64,
    loaded_dirty_section_count: f64,
    removal_dirty_section_count: f64,
    stale_dirty_section_count: f64,
    ready_compile_section_count: f64,
    deferred_compile_section_count: f64,
    budgeted_loaded_chunk_count: f64,
    budgeted_dirty_section_chunk_count: f64,
    submitted_compile_section_count: f64,
    uploaded_section_count: f64,
    removed_section_count: f64,
    accepted_compile_section_count: f64,
    stale_compile_section_count: f64,
    worker_section_count: f64,
    worker_non_empty_section_count: f64,
    worker_visibility_graph_build_count: f64,
    worker_visibility_graph_total_ms: f64,
    worker_visibility_graph_worst_ms: f64,
    worker_vertex_count: f64,
    worker_index_count: f64,
    worker_grass_patch_count: f64,
    worker_face_count: f64,
    max_frame_gap_ms: f64,
    frame_count_before: f64,
    frame_count_after: Option<f64>,
    render_count_before: f64,
    render_count_after: Option<f64>,
    reason: Option<String>,
}

#[wasm_bindgen]
impl WebCompileTiming {
    /// Begin a timing. `target_center_*` start null (set by {@link update_from_request},
    /// always called immediately after); JS passes the `loaded_center_before_*` it reads off
    /// its own `loadedCenter`, the frame/render counters, and `performance.now()`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        sequence: f64,
        trigger: String,
        loaded_center_before_x: Option<f64>,
        loaded_center_before_z: Option<f64>,
        frame_count_before: f64,
        render_count_before: f64,
        started_at_ms: f64,
    ) -> Self {
        Self {
            sequence,
            trigger,
            status: "running".to_owned(),
            request_id: None,
            target_center_x: None,
            target_center_z: None,
            loaded_center_before_x,
            loaded_center_before_z,
            loaded_center_after_x: None,
            loaded_center_after_z: None,
            started_at_ms,
            finished_at_ms: None,
            total_ms: 0.0,
            begin_request_ms: 0.0,
            worker_round_trip_ms: 0.0,
            decode_finish_apply_ms: 0.0,
            packed_byte_length: 0.0,
            worker_reported: false,
            render_compiler_transport_kind: "unknown".to_owned(),
            render_compiler_shared_memory_supported: false,
            render_compiler_worker_init_count: 0.0,
            render_compiler_worker_wasm_init_count: 0.0,
            render_compiler_worker_asset_load_count: 0.0,
            render_compiler_worker_asset_pack_init_byte_length: 0.0,
            render_compiler_worker_asset_pack_file_count: 0.0,
            render_compiler_persistent_asset_catalog: false,
            render_compiler_compile_count: 0.0,
            render_compiler_worker_compile_count: 0.0,
            render_compiler_asset_pack_send_count: 0.0,
            render_compiler_request_asset_pack_byte_length: 0.0,
            render_compiler_request_target_sections_byte_length: 0.0,
            render_compiler_request_snapshot_input_byte_length: 0.0,
            render_compiler_request_byte_length: 0.0,
            render_compiler_transferred_request_byte_length: 0.0,
            render_compiler_transferred_response_byte_length: 0.0,
            render_compiler_transferred_request_byte_count: 0.0,
            render_compiler_transferred_response_byte_count: 0.0,
            render_compiler_shared_input_buffer_used: false,
            render_compiler_shared_input_byte_length: 0.0,
            render_compiler_shared_input_buffer_capacity_bytes: 0.0,
            render_compiler_snapshot_input_chunk_count: 0.0,
            render_compiler_snapshot_input_cloned_column_count: 0.0,
            render_compiler_snapshot_input_compile_used: false,
            render_compiler_generated_view_fallback_used: false,
            render_compiler_shared_result_buffer_used: false,
            render_compiler_shared_result_byte_length: 0.0,
            render_compiler_shared_result_buffer_capacity_bytes: 0.0,
            render_compiler_shared_result_overflow: false,
            render_compiler_shared_result_response_count: 0.0,
            render_compiler_shared_result_byte_count: 0.0,
            render_compiler_shared_result_overflow_count: 0.0,
            chunk_view_update_count: 0.0,
            center_loaded: false,
            runner_settled: false,
            view_dirty_chunk_count: 0.0,
            view_removal_chunk_count: 0.0,
            loaded_dirty_chunk_count: 0.0,
            removal_dirty_chunk_count: 0.0,
            stale_dirty_chunk_count: 0.0,
            loaded_dirty_section_count: 0.0,
            removal_dirty_section_count: 0.0,
            stale_dirty_section_count: 0.0,
            ready_compile_section_count: 0.0,
            deferred_compile_section_count: 0.0,
            budgeted_loaded_chunk_count: 0.0,
            budgeted_dirty_section_chunk_count: 0.0,
            submitted_compile_section_count: 0.0,
            uploaded_section_count: 0.0,
            removed_section_count: 0.0,
            accepted_compile_section_count: 0.0,
            stale_compile_section_count: 0.0,
            worker_section_count: 0.0,
            worker_non_empty_section_count: 0.0,
            worker_visibility_graph_build_count: 0.0,
            worker_visibility_graph_total_ms: 0.0,
            worker_visibility_graph_worst_ms: 0.0,
            worker_vertex_count: 0.0,
            worker_index_count: 0.0,
            worker_grass_patch_count: 0.0,
            worker_face_count: 0.0,
            max_frame_gap_ms: 0.0,
            frame_count_before,
            frame_count_after: None,
            render_count_before,
            render_count_after: None,
            reason: None,
        }
    }

    /// Merge the request doorbell (the armed-this-frame report). `updateCompileTimingFromRequest`.
    #[wasm_bindgen(js_name = updateFromRequest)]
    pub fn update_from_request(&mut self, request: &JsValue) {
        self.request_id = coerce_number_or_null(request, "requestId");
        self.target_center_x = Some(coerce_number(request, "centerX"));
        self.target_center_z = Some(coerce_number(request, "centerZ"));
        self.submitted_compile_section_count =
            coerce_number(request, "submittedCompileSectionCount");
        self.update_scope(request);
    }

    /// JS-measured begin-request milliseconds (`Number.isFinite(syncMs) ? syncMs : 0`).
    #[wasm_bindgen(js_name = setBeginRequestMs)]
    pub fn set_begin_request_ms(&mut self, ms: f64) {
        self.begin_request_ms = if ms.is_finite() { ms } else { 0.0 };
    }

    /// JS-measured worker round-trip milliseconds (stored raw; rounded in the snapshot).
    #[wasm_bindgen(js_name = setWorkerRoundTripMs)]
    pub fn set_worker_round_trip_ms(&mut self, ms: f64) {
        self.worker_round_trip_ms = ms;
    }

    /// Merge the render-compile worker report (`updateCompileTimingFromWorker` +
    /// `normalizeRenderCompilerMetrics`). `source` is `report.renderCompilerMetrics ?? report`.
    #[wasm_bindgen(js_name = updateFromWorker)]
    pub fn update_from_worker(&mut self, report: &JsValue) {
        self.worker_reported = true;
        let source = reflect_get(report, "renderCompilerMetrics").unwrap_or_else(|| report.clone());
        let summary = reflect_get(report, "summary").unwrap_or(JsValue::UNDEFINED);

        self.render_compiler_transport_kind = coerce_string(&source, "transportKind", "unknown");
        self.render_compiler_shared_memory_supported =
            coerce_bool(&source, "sharedMemorySupported");
        self.render_compiler_worker_init_count = coerce_number(&source, "workerInitCount");
        self.render_compiler_worker_wasm_init_count = coerce_number(&source, "workerWasmInitCount");
        self.render_compiler_worker_asset_load_count =
            coerce_number(&source, "workerAssetLoadCount");
        self.render_compiler_worker_asset_pack_init_byte_length =
            coerce_number(&source, "workerAssetPackInitByteLength");
        self.render_compiler_worker_asset_pack_file_count =
            coerce_number(&source, "workerAssetPackFileCount");
        self.render_compiler_persistent_asset_catalog =
            coerce_bool(&source, "persistentAssetCatalog");
        self.render_compiler_compile_count = coerce_number(&source, "compileCount");
        self.render_compiler_worker_compile_count = coerce_number(&source, "workerCompileCount");
        self.render_compiler_asset_pack_send_count = coerce_number(&source, "assetPackSendCount");
        self.render_compiler_request_asset_pack_byte_length =
            coerce_number(&source, "requestAssetPackByteLength");
        self.render_compiler_request_target_sections_byte_length =
            coerce_number(&source, "requestTargetSectionsByteLength");
        self.render_compiler_request_snapshot_input_byte_length =
            coerce_number(&source, "requestSnapshotInputByteLength");
        self.render_compiler_request_byte_length = coerce_number(&source, "requestByteLength");
        self.render_compiler_transferred_request_byte_length =
            coerce_number(&source, "transferredRequestByteLength");
        // `Number(transferredResponseByteLength) || (sharedResultBufferUsed ? 0 : Number(packedByteLength)) || 0`
        self.render_compiler_transferred_response_byte_length = {
            let primary = js_number(&source, "transferredResponseByteLength");
            if truthy(primary) {
                primary
            } else {
                let fallback = if coerce_bool(&source, "sharedResultBufferUsed") {
                    0.0
                } else {
                    js_number(&source, "packedByteLength")
                };
                if truthy(fallback) { fallback } else { 0.0 }
            }
        };
        self.render_compiler_transferred_request_byte_count =
            coerce_number(&source, "transferredRequestByteCount");
        self.render_compiler_transferred_response_byte_count =
            coerce_number(&source, "transferredResponseByteCount");
        self.render_compiler_shared_input_buffer_used =
            coerce_bool(&source, "sharedInputBufferUsed");
        self.render_compiler_shared_input_byte_length =
            coerce_number(&source, "sharedInputByteLength");
        self.render_compiler_shared_input_buffer_capacity_bytes =
            coerce_number(&source, "sharedInputBufferCapacityBytes");
        self.render_compiler_snapshot_input_chunk_count =
            coerce_number(&source, "snapshotInputChunkCount");
        self.render_compiler_snapshot_input_cloned_column_count =
            coerce_number(&source, "snapshotInputClonedColumnCount");
        self.render_compiler_snapshot_input_compile_used =
            coerce_bool(&source, "snapshotInputCompileUsed");
        self.render_compiler_generated_view_fallback_used =
            coerce_bool(&source, "generatedViewFallbackUsed");
        self.render_compiler_shared_result_buffer_used =
            coerce_bool(&source, "sharedResultBufferUsed");
        // `Number(sharedResultByteLength) || (sharedResultBufferUsed ? Number(packedByteLength) : 0) || 0`
        self.render_compiler_shared_result_byte_length = {
            let primary = js_number(&source, "sharedResultByteLength");
            if truthy(primary) {
                primary
            } else {
                let fallback = if coerce_bool(&source, "sharedResultBufferUsed") {
                    js_number(&source, "packedByteLength")
                } else {
                    0.0
                };
                if truthy(fallback) { fallback } else { 0.0 }
            }
        };
        self.render_compiler_shared_result_buffer_capacity_bytes =
            coerce_number(&source, "sharedResultBufferCapacityBytes");
        self.render_compiler_shared_result_overflow = coerce_bool(&source, "sharedResultOverflow");
        self.render_compiler_shared_result_response_count =
            coerce_number(&source, "sharedResultResponseCount");
        self.render_compiler_shared_result_byte_count =
            coerce_number(&source, "sharedResultByteCount");
        self.render_compiler_shared_result_overflow_count =
            coerce_number(&source, "sharedResultOverflowCount");

        // `packedByteLength = sharedResultByteLength || transferredResponseByteLength || packedByteLength`
        self.packed_byte_length = if truthy(self.render_compiler_shared_result_byte_length) {
            self.render_compiler_shared_result_byte_length
        } else if truthy(self.render_compiler_transferred_response_byte_length) {
            self.render_compiler_transferred_response_byte_length
        } else {
            self.packed_byte_length
        };

        self.worker_section_count = coerce_number(&summary, "sectionCount");
        self.worker_non_empty_section_count = coerce_number(&summary, "nonEmptySectionCount");
        self.worker_visibility_graph_build_count =
            coerce_number(&summary, "visibilityGraphBuildCount");
        self.worker_visibility_graph_total_ms = coerce_number(&summary, "visibilityGraphTotalMs");
        self.worker_visibility_graph_worst_ms = coerce_number(&summary, "visibilityGraphWorstMs");
        self.worker_vertex_count = coerce_number(&summary, "vertexCount");
        self.worker_index_count = coerce_number(&summary, "indexCount");
        self.worker_grass_patch_count = coerce_number(&summary, "grassPatchCount");
        self.worker_face_count = coerce_number(&summary, "faceCount");
    }

    /// JS-measured decode/finish/apply milliseconds (`Number.isFinite(syncMs) ? syncMs : 0`).
    #[wasm_bindgen(js_name = setDecodeFinishApplyMs)]
    pub fn set_decode_finish_apply_ms(&mut self, ms: f64) {
        self.decode_finish_apply_ms = if ms.is_finite() { ms } else { 0.0 };
    }

    /// Merge the Rust apply report for the compile applied this frame (`updateCompileTimingFromReport`).
    #[wasm_bindgen(js_name = updateFromReport)]
    pub fn update_from_report(&mut self, report: &JsValue) {
        self.loaded_center_after_x = Some(coerce_number(report, "centerX"));
        self.loaded_center_after_z = Some(coerce_number(report, "centerZ"));
        self.center_loaded = true;
        self.runner_settled = true;
        self.uploaded_section_count = coerce_number(report, "uploadedSectionCount");
        self.removed_section_count = coerce_number(report, "removedSectionCount");
        self.accepted_compile_section_count = coerce_number(report, "acceptedCompileSectionCount");
        self.stale_compile_section_count = coerce_number(report, "staleCompileSectionCount");
        self.submitted_compile_section_count = coerce_number_or_keep(
            report,
            "submittedCompileSectionCount",
            self.submitted_compile_section_count,
        );
        self.packed_byte_length =
            coerce_number_or_keep(report, "workerPackedByteLength", self.packed_byte_length);
        self.worker_visibility_graph_build_count = coerce_number_or_keep(
            report,
            "workerVisibilityGraphBuildCount",
            self.worker_visibility_graph_build_count,
        );
        self.worker_visibility_graph_total_ms = coerce_number_or_keep(
            report,
            "workerVisibilityGraphTotalMs",
            self.worker_visibility_graph_total_ms,
        );
        self.worker_visibility_graph_worst_ms = coerce_number_or_keep(
            report,
            "workerVisibilityGraphWorstMs",
            self.worker_visibility_graph_worst_ms,
        );
        self.update_scope(report);
    }

    /// Charge a presentation frame gap to this in-flight timing (`maxFrameGapMs` fold).
    #[wasm_bindgen(js_name = observeFrameGap)]
    pub fn observe_frame_gap(&mut self, frame_gap_ms: f64) {
        if frame_gap_ms.is_finite() {
            self.max_frame_gap_ms = self.max_frame_gap_ms.max(frame_gap_ms);
        }
    }

    /// Finalize: stamp status/reason/timing and the after-counters (`finishCompileTiming`).
    #[wasm_bindgen(js_name = finish)]
    pub fn finish(
        &mut self,
        status: String,
        reason: Option<String>,
        finished_at_ms: f64,
        frame_count_after: f64,
        render_count_after: f64,
        last_frame_gap_ms: f64,
    ) {
        self.status = status;
        self.reason = reason;
        self.finished_at_ms = Some(finished_at_ms);
        self.total_ms = finished_at_ms - self.started_at_ms;
        self.frame_count_after = Some(frame_count_after);
        self.render_count_after = Some(render_count_after);
        let gap = if last_frame_gap_ms.is_finite() {
            last_frame_gap_ms
        } else {
            0.0
        };
        self.max_frame_gap_ms = self.max_frame_gap_ms.max(gap);
    }

    /// Project the rounded public snapshot consumed by `runtime.state` / HUD / smoke harness
    /// (`publicCompileTiming`). `now_ms` is `performance.now()`, used to compute the running
    /// `totalMs` of a not-yet-finished timing.
    #[wasm_bindgen(js_name = publicSnapshot)]
    pub fn public_snapshot(&self, now_ms: f64) -> Result<JsValue, JsValue> {
        let total_ms = match self.finished_at_ms {
            Some(_) => self.total_ms,
            None => now_ms - self.started_at_ms,
        };
        let object = Object::new();
        set_number(&object, "sequence", self.sequence)?;
        set_string(&object, "trigger", &self.trigger)?;
        set_string(&object, "status", &self.status)?;
        set_optional_number(&object, "requestId", self.request_id)?;
        set_optional_number(&object, "targetCenterX", self.target_center_x)?;
        set_optional_number(&object, "targetCenterZ", self.target_center_z)?;
        set_optional_number(&object, "loadedCenterBeforeX", self.loaded_center_before_x)?;
        set_optional_number(&object, "loadedCenterBeforeZ", self.loaded_center_before_z)?;
        set_optional_number(&object, "loadedCenterAfterX", self.loaded_center_after_x)?;
        set_optional_number(&object, "loadedCenterAfterZ", self.loaded_center_after_z)?;
        set_number(&object, "totalMs", round_timing(total_ms))?;
        set_number(
            &object,
            "beginRequestMs",
            round_timing(self.begin_request_ms),
        )?;
        set_number(
            &object,
            "workerRoundTripMs",
            round_timing(self.worker_round_trip_ms),
        )?;
        set_number(&object, "packedByteLength", self.packed_byte_length)?;
        set_string(
            &object,
            "renderCompilerTransportKind",
            &self.render_compiler_transport_kind,
        )?;
        set_bool(
            &object,
            "renderCompilerSharedMemorySupported",
            self.render_compiler_shared_memory_supported,
        )?;
        set_number(
            &object,
            "renderCompilerWorkerInitCount",
            self.render_compiler_worker_init_count,
        )?;
        set_number(
            &object,
            "renderCompilerWorkerWasmInitCount",
            self.render_compiler_worker_wasm_init_count,
        )?;
        set_number(
            &object,
            "renderCompilerWorkerAssetLoadCount",
            self.render_compiler_worker_asset_load_count,
        )?;
        set_number(
            &object,
            "renderCompilerWorkerAssetPackInitByteLength",
            self.render_compiler_worker_asset_pack_init_byte_length,
        )?;
        set_number(
            &object,
            "renderCompilerWorkerAssetPackFileCount",
            self.render_compiler_worker_asset_pack_file_count,
        )?;
        set_bool(
            &object,
            "renderCompilerPersistentAssetCatalog",
            self.render_compiler_persistent_asset_catalog,
        )?;
        set_number(
            &object,
            "renderCompilerCompileCount",
            self.render_compiler_compile_count,
        )?;
        set_number(
            &object,
            "renderCompilerWorkerCompileCount",
            self.render_compiler_worker_compile_count,
        )?;
        set_number(
            &object,
            "renderCompilerAssetPackSendCount",
            self.render_compiler_asset_pack_send_count,
        )?;
        set_number(
            &object,
            "renderCompilerRequestAssetPackByteLength",
            self.render_compiler_request_asset_pack_byte_length,
        )?;
        set_number(
            &object,
            "renderCompilerRequestTargetSectionsByteLength",
            self.render_compiler_request_target_sections_byte_length,
        )?;
        set_number(
            &object,
            "renderCompilerRequestSnapshotInputByteLength",
            self.render_compiler_request_snapshot_input_byte_length,
        )?;
        set_number(
            &object,
            "renderCompilerRequestByteLength",
            self.render_compiler_request_byte_length,
        )?;
        set_number(
            &object,
            "renderCompilerTransferredRequestByteLength",
            self.render_compiler_transferred_request_byte_length,
        )?;
        set_number(
            &object,
            "renderCompilerTransferredResponseByteLength",
            self.render_compiler_transferred_response_byte_length,
        )?;
        set_number(
            &object,
            "renderCompilerTransferredRequestByteCount",
            self.render_compiler_transferred_request_byte_count,
        )?;
        set_number(
            &object,
            "renderCompilerTransferredResponseByteCount",
            self.render_compiler_transferred_response_byte_count,
        )?;
        set_bool(
            &object,
            "renderCompilerSharedInputBufferUsed",
            self.render_compiler_shared_input_buffer_used,
        )?;
        set_number(
            &object,
            "renderCompilerSharedInputByteLength",
            self.render_compiler_shared_input_byte_length,
        )?;
        set_number(
            &object,
            "renderCompilerSharedInputBufferCapacityBytes",
            self.render_compiler_shared_input_buffer_capacity_bytes,
        )?;
        set_number(
            &object,
            "renderCompilerSnapshotInputChunkCount",
            self.render_compiler_snapshot_input_chunk_count,
        )?;
        set_number(
            &object,
            "renderCompilerSnapshotInputClonedColumnCount",
            self.render_compiler_snapshot_input_cloned_column_count,
        )?;
        set_bool(
            &object,
            "renderCompilerSnapshotInputCompileUsed",
            self.render_compiler_snapshot_input_compile_used,
        )?;
        set_bool(
            &object,
            "renderCompilerGeneratedViewFallbackUsed",
            self.render_compiler_generated_view_fallback_used,
        )?;
        set_bool(
            &object,
            "renderCompilerSharedResultBufferUsed",
            self.render_compiler_shared_result_buffer_used,
        )?;
        set_number(
            &object,
            "renderCompilerSharedResultByteLength",
            self.render_compiler_shared_result_byte_length,
        )?;
        set_number(
            &object,
            "renderCompilerSharedResultBufferCapacityBytes",
            self.render_compiler_shared_result_buffer_capacity_bytes,
        )?;
        set_bool(
            &object,
            "renderCompilerSharedResultOverflow",
            self.render_compiler_shared_result_overflow,
        )?;
        set_number(
            &object,
            "renderCompilerSharedResultResponseCount",
            self.render_compiler_shared_result_response_count,
        )?;
        set_number(
            &object,
            "renderCompilerSharedResultByteCount",
            self.render_compiler_shared_result_byte_count,
        )?;
        set_number(
            &object,
            "renderCompilerSharedResultOverflowCount",
            self.render_compiler_shared_result_overflow_count,
        )?;
        let metrics = if self.worker_reported {
            self.metrics_object()?
        } else {
            JsValue::NULL
        };
        Reflect::set(
            &object,
            &JsValue::from_str("renderCompilerMetrics"),
            &metrics,
        )
        .map_err(|error| JsValue::from_str(&format!("set renderCompilerMetrics: {error:?}")))?;
        set_number(
            &object,
            "decodeFinishApplyMs",
            round_timing(self.decode_finish_apply_ms),
        )?;
        set_number(
            &object,
            "chunkViewUpdateCount",
            self.chunk_view_update_count,
        )?;
        set_bool(&object, "centerLoaded", self.center_loaded)?;
        set_bool(&object, "runnerSettled", self.runner_settled)?;
        set_number(&object, "viewDirtyChunkCount", self.view_dirty_chunk_count)?;
        set_number(
            &object,
            "viewRemovalChunkCount",
            self.view_removal_chunk_count,
        )?;
        set_number(
            &object,
            "loadedDirtyChunkCount",
            self.loaded_dirty_chunk_count,
        )?;
        set_number(
            &object,
            "removalDirtyChunkCount",
            self.removal_dirty_chunk_count,
        )?;
        set_number(
            &object,
            "staleDirtyChunkCount",
            self.stale_dirty_chunk_count,
        )?;
        set_number(
            &object,
            "loadedDirtySectionCount",
            self.loaded_dirty_section_count,
        )?;
        set_number(
            &object,
            "removalDirtySectionCount",
            self.removal_dirty_section_count,
        )?;
        set_number(
            &object,
            "staleDirtySectionCount",
            self.stale_dirty_section_count,
        )?;
        set_number(
            &object,
            "readyCompileSectionCount",
            self.ready_compile_section_count,
        )?;
        set_number(
            &object,
            "deferredCompileSectionCount",
            self.deferred_compile_section_count,
        )?;
        set_number(
            &object,
            "budgetedLoadedChunkCount",
            self.budgeted_loaded_chunk_count,
        )?;
        set_number(
            &object,
            "budgetedDirtySectionChunkCount",
            self.budgeted_dirty_section_chunk_count,
        )?;
        set_number(
            &object,
            "submittedCompileSectionCount",
            self.submitted_compile_section_count,
        )?;
        set_number(&object, "uploadedSectionCount", self.uploaded_section_count)?;
        set_number(&object, "removedSectionCount", self.removed_section_count)?;
        set_number(
            &object,
            "acceptedCompileSectionCount",
            self.accepted_compile_section_count,
        )?;
        set_number(
            &object,
            "staleCompileSectionCount",
            self.stale_compile_section_count,
        )?;
        set_number(&object, "workerSectionCount", self.worker_section_count)?;
        set_number(
            &object,
            "workerNonEmptySectionCount",
            self.worker_non_empty_section_count,
        )?;
        set_number(
            &object,
            "workerVisibilityGraphBuildCount",
            self.worker_visibility_graph_build_count,
        )?;
        set_number(
            &object,
            "workerVisibilityGraphTotalMs",
            round_timing(self.worker_visibility_graph_total_ms),
        )?;
        set_number(
            &object,
            "workerVisibilityGraphWorstMs",
            round_timing(self.worker_visibility_graph_worst_ms),
        )?;
        set_number(&object, "workerVertexCount", self.worker_vertex_count)?;
        set_number(&object, "workerIndexCount", self.worker_index_count)?;
        set_number(
            &object,
            "workerGrassPatchCount",
            self.worker_grass_patch_count,
        )?;
        set_number(&object, "workerFaceCount", self.worker_face_count)?;
        set_number(
            &object,
            "maxFrameGapMs",
            round_timing(self.max_frame_gap_ms),
        )?;
        set_number(&object, "frameCountBefore", self.frame_count_before)?;
        set_optional_number(&object, "frameCountAfter", self.frame_count_after)?;
        set_number(&object, "renderCountBefore", self.render_count_before)?;
        set_optional_number(&object, "renderCountAfter", self.render_count_after)?;
        set_optional_string(&object, "reason", self.reason.as_deref())?;
        Ok(object.into())
    }
}

impl WebCompileTiming {
    /// Shared scope fields read from both the request doorbell and the apply report
    /// (`updateCompileScopeTiming`).
    fn update_scope(&mut self, source: &JsValue) {
        self.view_dirty_chunk_count = coerce_number(source, "viewDirtyChunkCount");
        self.view_removal_chunk_count = coerce_number(source, "viewRemovalChunkCount");
        self.loaded_dirty_chunk_count = coerce_number(source, "loadedDirtyChunkCount");
        self.removal_dirty_chunk_count = coerce_number(source, "removalDirtyChunkCount");
        self.stale_dirty_chunk_count = coerce_number(source, "staleDirtyChunkCount");
        self.loaded_dirty_section_count = coerce_number(source, "loadedDirtySectionCount");
        self.removal_dirty_section_count = coerce_number(source, "removalDirtySectionCount");
        self.stale_dirty_section_count = coerce_number(source, "staleDirtySectionCount");
        self.ready_compile_section_count = coerce_number(source, "readyCompileSectionCount");
        self.deferred_compile_section_count = coerce_number(source, "deferredCompileSectionCount");
        self.budgeted_loaded_chunk_count = coerce_number(source, "budgetedLoadedChunkCount");
        self.budgeted_dirty_section_chunk_count =
            coerce_number(source, "budgetedDirtySectionChunkCount");
    }

    /// Rebuild the nested `renderCompilerMetrics` object from the stored scalars (which are a
    /// 1:1 mirror of `normalizeRenderCompilerMetrics`, only ever set together in
    /// `update_from_worker`).
    fn metrics_object(&self) -> Result<JsValue, JsValue> {
        let object = Object::new();
        set_string(
            &object,
            "transportKind",
            &self.render_compiler_transport_kind,
        )?;
        set_bool(
            &object,
            "sharedMemorySupported",
            self.render_compiler_shared_memory_supported,
        )?;
        set_number(
            &object,
            "workerInitCount",
            self.render_compiler_worker_init_count,
        )?;
        set_number(
            &object,
            "workerWasmInitCount",
            self.render_compiler_worker_wasm_init_count,
        )?;
        set_number(
            &object,
            "workerAssetLoadCount",
            self.render_compiler_worker_asset_load_count,
        )?;
        set_number(
            &object,
            "workerAssetPackInitByteLength",
            self.render_compiler_worker_asset_pack_init_byte_length,
        )?;
        set_number(
            &object,
            "workerAssetPackFileCount",
            self.render_compiler_worker_asset_pack_file_count,
        )?;
        set_bool(
            &object,
            "persistentAssetCatalog",
            self.render_compiler_persistent_asset_catalog,
        )?;
        set_number(&object, "compileCount", self.render_compiler_compile_count)?;
        set_number(
            &object,
            "workerCompileCount",
            self.render_compiler_worker_compile_count,
        )?;
        set_number(
            &object,
            "assetPackSendCount",
            self.render_compiler_asset_pack_send_count,
        )?;
        set_number(
            &object,
            "requestAssetPackByteLength",
            self.render_compiler_request_asset_pack_byte_length,
        )?;
        set_number(
            &object,
            "requestTargetSectionsByteLength",
            self.render_compiler_request_target_sections_byte_length,
        )?;
        set_number(
            &object,
            "requestSnapshotInputByteLength",
            self.render_compiler_request_snapshot_input_byte_length,
        )?;
        set_number(
            &object,
            "requestByteLength",
            self.render_compiler_request_byte_length,
        )?;
        set_number(
            &object,
            "transferredRequestByteLength",
            self.render_compiler_transferred_request_byte_length,
        )?;
        set_number(
            &object,
            "transferredResponseByteLength",
            self.render_compiler_transferred_response_byte_length,
        )?;
        set_number(
            &object,
            "transferredRequestByteCount",
            self.render_compiler_transferred_request_byte_count,
        )?;
        set_number(
            &object,
            "transferredResponseByteCount",
            self.render_compiler_transferred_response_byte_count,
        )?;
        set_bool(
            &object,
            "sharedInputBufferUsed",
            self.render_compiler_shared_input_buffer_used,
        )?;
        set_number(
            &object,
            "sharedInputByteLength",
            self.render_compiler_shared_input_byte_length,
        )?;
        set_number(
            &object,
            "sharedInputBufferCapacityBytes",
            self.render_compiler_shared_input_buffer_capacity_bytes,
        )?;
        set_number(
            &object,
            "snapshotInputChunkCount",
            self.render_compiler_snapshot_input_chunk_count,
        )?;
        set_number(
            &object,
            "snapshotInputClonedColumnCount",
            self.render_compiler_snapshot_input_cloned_column_count,
        )?;
        set_bool(
            &object,
            "snapshotInputCompileUsed",
            self.render_compiler_snapshot_input_compile_used,
        )?;
        set_bool(
            &object,
            "generatedViewFallbackUsed",
            self.render_compiler_generated_view_fallback_used,
        )?;
        set_bool(
            &object,
            "sharedResultBufferUsed",
            self.render_compiler_shared_result_buffer_used,
        )?;
        set_number(
            &object,
            "sharedResultByteLength",
            self.render_compiler_shared_result_byte_length,
        )?;
        set_number(
            &object,
            "sharedResultBufferCapacityBytes",
            self.render_compiler_shared_result_buffer_capacity_bytes,
        )?;
        set_bool(
            &object,
            "sharedResultOverflow",
            self.render_compiler_shared_result_overflow,
        )?;
        set_number(
            &object,
            "sharedResultResponseCount",
            self.render_compiler_shared_result_response_count,
        )?;
        set_number(
            &object,
            "sharedResultByteCount",
            self.render_compiler_shared_result_byte_count,
        )?;
        set_number(
            &object,
            "sharedResultOverflowCount",
            self.render_compiler_shared_result_overflow_count,
        )?;
        Ok(object.into())
    }
}

fn reflect_get(value: &JsValue, name: &str) -> Option<JsValue> {
    Reflect::get(value, &JsValue::from_str(name))
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

/// Raw `Number(value[name])` — NaN when absent/non-numeric.
fn js_number(value: &JsValue, name: &str) -> f64 {
    reflect_get(value, name)
        .and_then(|value| value.as_f64())
        .unwrap_or(f64::NAN)
}

/// JS truthiness for a number: non-zero and not NaN.
fn truthy(value: f64) -> bool {
    value != 0.0 && !value.is_nan()
}

/// `Number(value[name]) || 0`.
fn coerce_number(value: &JsValue, name: &str) -> f64 {
    let number = js_number(value, name);
    if truthy(number) { number } else { 0.0 }
}

/// `Number(value[name]) || null`.
fn coerce_number_or_null(value: &JsValue, name: &str) -> Option<f64> {
    let number = js_number(value, name);
    if truthy(number) { Some(number) } else { None }
}

/// `Number(value[name]) || current`.
fn coerce_number_or_keep(value: &JsValue, name: &str, current: f64) -> f64 {
    let number = js_number(value, name);
    if truthy(number) { number } else { current }
}

/// `Boolean(value[name])` (the boundary values are real JS booleans).
fn coerce_bool(value: &JsValue, name: &str) -> bool {
    reflect_get(value, name)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

/// `String(value[name] || fallback)` (empty/absent falls back).
fn coerce_string(value: &JsValue, name: &str, fallback: &str) -> String {
    reflect_get(value, name)
        .and_then(|value| value.as_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

/// `Number.isFinite(value) ? Math.round(value * 10) / 10 : 0` (timing values are non-negative).
fn round_timing(value: f64) -> f64 {
    if value.is_finite() {
        (value * 10.0).round() / 10.0
    } else {
        0.0
    }
}

fn set_number(object: &Object, name: &str, value: f64) -> Result<(), JsValue> {
    Reflect::set(object, &JsValue::from_str(name), &JsValue::from_f64(value))
        .map_err(|error| JsValue::from_str(&format!("set {name}: {error:?}")))?;
    Ok(())
}

fn set_optional_number(object: &Object, name: &str, value: Option<f64>) -> Result<(), JsValue> {
    let js = value.map_or(JsValue::NULL, JsValue::from_f64);
    Reflect::set(object, &JsValue::from_str(name), &js)
        .map_err(|error| JsValue::from_str(&format!("set {name}: {error:?}")))?;
    Ok(())
}

fn set_bool(object: &Object, name: &str, value: bool) -> Result<(), JsValue> {
    Reflect::set(object, &JsValue::from_str(name), &JsValue::from_bool(value))
        .map_err(|error| JsValue::from_str(&format!("set {name}: {error:?}")))?;
    Ok(())
}

fn set_string(object: &Object, name: &str, value: &str) -> Result<(), JsValue> {
    Reflect::set(object, &JsValue::from_str(name), &JsValue::from_str(value))
        .map_err(|error| JsValue::from_str(&format!("set {name}: {error:?}")))?;
    Ok(())
}

fn set_optional_string(object: &Object, name: &str, value: Option<&str>) -> Result<(), JsValue> {
    let js = value.map_or(JsValue::NULL, JsValue::from_str);
    Reflect::set(object, &JsValue::from_str(name), &js)
        .map_err(|error| JsValue::from_str(&format!("set {name}: {error:?}")))?;
    Ok(())
}
