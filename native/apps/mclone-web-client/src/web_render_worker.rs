//! Rust ownership of the production browser render-worker lifecycle.
//!
//! JavaScript supplies only a generic, explicitly polled transport object. This
//! coordinator owns worker generations, request identity, active/standby
//! scheduling, world release, failure recovery, asset-candidate activation, and
//! public compile diagnostics.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use mclone_app_runtime::scene_session_runtime::RuntimeRenderPriority;
use wasm_bindgen::{JsCast, JsValue};

use crate::render_worker_coordinator::{
    RenderWorkerActions, RenderWorkerAssetSwapState, RenderWorkerCoordinatorState,
    RenderWorkerGeneration, RenderWorkerLifecycle, RenderWorkerPriority, RenderWorkerRequest,
};
use crate::web_compile_timing::WebCompileTiming;

const WORKER_TIMEOUT_MS: u64 = 20_000;
const MAX_RETAINED_COMPILE_TIMINGS: usize = 16;

#[derive(Clone)]
enum WorkerAssetPayload {
    Reference(Vec<u8>),
    Selection {
        authored: Vec<u8>,
        reference: Vec<u8>,
        fallback: Vec<u8>,
        authored_enabled: bool,
        reference_enabled: bool,
        epoch: u64,
    },
}

impl WorkerAssetPayload {
    fn byte_length(&self) -> usize {
        match self {
            Self::Reference(bytes) => bytes.len(),
            Self::Selection {
                authored,
                reference,
                fallback,
                ..
            } => authored
                .len()
                .saturating_add(reference.len())
                .saturating_add(fallback.len()),
        }
    }
}

struct PolledWorkerTransport {
    value: JsValue,
}

impl PolledWorkerTransport {
    fn create(factory: &Function) -> Result<Self, String> {
        let value = factory
            .call0(&JsValue::NULL)
            .map_err(|error| format!("create render-worker transport: {error:?}"))?;
        for method in ["post", "poll", "terminate"] {
            if !reflect_get(&value, method).is_some_and(|value| value.is_function()) {
                return Err(format!(
                    "render-worker transport is missing callable {method}()"
                ));
            }
        }
        Ok(Self { value })
    }

    fn post(&self, message: &JsValue, transfer: &Array) -> Result<(), String> {
        method(&self.value, "post")?
            .call2(&self.value, message, transfer)
            .map(|_| ())
            .map_err(|error| format!("post render-worker message: {error:?}"))
    }

    fn poll(&self) -> Result<Option<JsValue>, String> {
        let event = method(&self.value, "poll")?
            .call0(&self.value)
            .map_err(|error| format!("poll render-worker transport: {error:?}"))?;
        Ok((!event.is_null() && !event.is_undefined()).then_some(event))
    }

    fn terminate(&self) {
        if let Ok(terminate) = method(&self.value, "terminate") {
            let _ = terminate.call0(&self.value);
        }
    }
}

struct PendingRequest {
    message: JsValue,
    timing: WebCompileTiming,
}

struct WorkerRuntime {
    state: RenderWorkerCoordinatorState,
    transport: PolledWorkerTransport,
    payload: WorkerAssetPayload,
    pending: BTreeMap<u64, PendingRequest>,
    ready_report: Option<JsValue>,
    failure: Option<String>,
}

impl WorkerRuntime {
    fn new(
        identity: RenderWorkerGeneration,
        factory: &Function,
        bindgen_js_url: &str,
        bindgen_wasm_url: &str,
        payload: WorkerAssetPayload,
    ) -> Result<Self, String> {
        let transport = PolledWorkerTransport::create(factory)?;
        let (message, transfer) = init_message(bindgen_js_url, bindgen_wasm_url, &payload)?;
        transport.post(&message, &transfer)?;
        Ok(Self {
            state: RenderWorkerCoordinatorState::new(identity.worker_generation),
            transport,
            payload,
            pending: BTreeMap::new(),
            ready_report: None,
            failure: None,
        })
    }
}

struct CoordinatorInner {
    factory: Function,
    bindgen_js_url: String,
    bindgen_wasm_url: String,
    swap: RenderWorkerAssetSwapState,
    workers: BTreeMap<u64, WorkerRuntime>,
    failed_requests: BTreeMap<(u64, u32), String>,
    qualified_worlds: BTreeSet<u64>,
    request_worlds: BTreeMap<u32, BTreeSet<u64>>,
    qualified_local_request_id_collision_count: usize,
    compile_sequence: u64,
    compile_count: usize,
    completed_timing_count: usize,
    worker_init_count: usize,
    asset_pack_send_count: usize,
    transferred_request_byte_count: usize,
    shared_result_response_count: usize,
    shared_result_byte_count: usize,
    shared_result_overflow_count: usize,
    stale_completion_count: usize,
    completed_timings: VecDeque<JsValue>,
    last_compile_report: Option<JsValue>,
    last_frame_count: u64,
    last_render_count: u64,
    last_frame_gap_ms: f64,
    terminated: bool,
}

/// Cloneable host handle for the one render-worker coordinator belonging to a
/// production web scene host.
#[derive(Clone)]
pub(crate) struct WebRenderWorkerCoordinator {
    inner: Rc<RefCell<CoordinatorInner>>,
}

/// A world-qualified render-worker capability held by one scene runtime.
pub(crate) struct WebRenderWorkerWorldHandle {
    coordinator: WebRenderWorkerCoordinator,
    world_instance_id: u64,
    priority: Cell<RuntimeRenderPriority>,
}

impl std::fmt::Debug for WebRenderWorkerWorldHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WebRenderWorkerWorldHandle")
            .field("world_instance_id", &self.world_instance_id)
            .field("priority", &self.priority.get())
            .finish_non_exhaustive()
    }
}

impl Drop for WebRenderWorkerWorldHandle {
    fn drop(&mut self) {
        self.coordinator.release_world(self.world_instance_id);
    }
}

impl WebRenderWorkerCoordinator {
    pub(crate) fn new(
        factory: Function,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
        reference_pack: Vec<u8>,
    ) -> Result<Self, String> {
        let identity = RenderWorkerGeneration {
            asset_epoch: 0,
            worker_generation: 1,
        };
        let payload = WorkerAssetPayload::Reference(reference_pack);
        let runtime = WorkerRuntime::new(
            identity,
            &factory,
            &bindgen_js_url,
            &bindgen_wasm_url,
            payload.clone(),
        )?;
        let transferred_request_byte_count = payload.byte_length();
        Ok(Self {
            inner: Rc::new(RefCell::new(CoordinatorInner {
                factory,
                bindgen_js_url,
                bindgen_wasm_url,
                swap: RenderWorkerAssetSwapState::new(0, 1),
                workers: BTreeMap::from([(1, runtime)]),
                failed_requests: BTreeMap::new(),
                qualified_worlds: BTreeSet::new(),
                request_worlds: BTreeMap::new(),
                qualified_local_request_id_collision_count: 0,
                compile_sequence: 0,
                compile_count: 0,
                completed_timing_count: 0,
                worker_init_count: 1,
                asset_pack_send_count: 1,
                transferred_request_byte_count,
                shared_result_response_count: 0,
                shared_result_byte_count: 0,
                shared_result_overflow_count: 0,
                stale_completion_count: 0,
                completed_timings: VecDeque::new(),
                last_compile_report: None,
                last_frame_count: 0,
                last_render_count: 0,
                last_frame_gap_ms: 0.0,
                terminated: false,
            })),
        })
    }

    pub(crate) fn world_handle(
        &self,
        world_instance_id: u64,
        priority: RuntimeRenderPriority,
    ) -> WebRenderWorkerWorldHandle {
        WebRenderWorkerWorldHandle {
            coordinator: self.clone(),
            world_instance_id,
            priority: Cell::new(priority),
        }
    }

    pub(crate) fn poll(
        &self,
        _frame_now_ms: f64,
        frame_count: u64,
        render_count: u64,
        last_frame_gap_ms: f64,
    ) -> Result<(), String> {
        let now_ms = monotonic_now_ms();
        let mut inner = self.inner.borrow_mut();
        if inner.terminated {
            return Ok(());
        }
        inner.last_frame_count = frame_count;
        inner.last_render_count = render_count;
        inner.last_frame_gap_ms = last_frame_gap_ms;
        let generation_ids = inner.workers.keys().copied().collect::<Vec<_>>();
        for generation in generation_ids {
            inner.poll_generation(generation, now_ms)?;
        }
        inner.recover_failed_active()?;
        Ok(())
    }

    pub(crate) async fn prepare_asset_candidate(
        &self,
        epoch: u64,
        authored: Vec<u8>,
        reference: Vec<u8>,
        fallback: Vec<u8>,
        authored_enabled: bool,
        reference_enabled: bool,
    ) -> Result<(), String> {
        let payload = WorkerAssetPayload::Selection {
            authored,
            reference,
            fallback,
            authored_enabled,
            reference_enabled,
            epoch,
        };
        {
            let mut inner = self.inner.borrow_mut();
            if inner.terminated {
                return Err("render-worker coordinator is terminated".to_owned());
            }
            let candidate = inner.swap.begin_candidate(epoch);
            let worker = WorkerRuntime::new(
                candidate,
                &inner.factory,
                &inner.bindgen_js_url,
                &inner.bindgen_wasm_url,
                payload,
            )?;
            inner.transferred_request_byte_count = inner
                .transferred_request_byte_count
                .saturating_add(worker.payload.byte_length());
            inner.worker_init_count = inner.worker_init_count.saturating_add(1);
            inner.asset_pack_send_count = inner.asset_pack_send_count.saturating_add(1);
            inner.workers.insert(candidate.worker_generation, worker);
        }

        let deadline = monotonic_now_ms() + WORKER_TIMEOUT_MS as f64;
        loop {
            let now = monotonic_now_ms();
            let (frame_count, render_count, frame_gap_ms) = {
                let inner = self.inner.borrow();
                (
                    inner.last_frame_count,
                    inner.last_render_count,
                    inner.last_frame_gap_ms,
                )
            };
            self.poll(now, frame_count, render_count, frame_gap_ms)?;
            let mut inner = self.inner.borrow_mut();
            let candidate = inner
                .swap
                .candidate()
                .ok_or_else(|| "render-worker asset candidate was lost".to_owned())?;
            let candidate_runtime = inner
                .workers
                .get(&candidate.worker_generation)
                .ok_or_else(|| "render-worker asset candidate runtime was lost".to_owned())?;
            if let Some(reason) = candidate_runtime.failure.clone() {
                let actions = inner.swap.rollback(epoch);
                inner.terminate_generation(actions.terminate_generation);
                return Err(reason);
            }
            let candidate_ready =
                candidate_runtime.state.lifecycle() == RenderWorkerLifecycle::Ready;
            let active_idle = inner
                .workers
                .get(&inner.swap.active().worker_generation)
                .is_some_and(|worker| worker.state.is_idle());
            if candidate_ready && active_idle {
                inner.swap.mark_candidate_ready(candidate.worker_generation);
                inner.swap.activate_candidate(epoch);
                return Ok(());
            }
            drop(inner);
            if now >= deadline {
                let mut inner = self.inner.borrow_mut();
                let actions = inner.swap.rollback(epoch);
                inner.terminate_generation(actions.terminate_generation);
                return Err(format!(
                    "timed out preparing render-worker asset candidate for epoch {epoch}"
                ));
            }
            crate::web_canvas::wait_for_remote_worker_turn(0).await?;
        }
    }

    pub(crate) fn settle_asset_epoch(&self, epoch: u64, active: bool) {
        let mut inner = self.inner.borrow_mut();
        let actions = if active {
            inner.swap.commit(epoch)
        } else {
            inner.swap.rollback(epoch)
        };
        inner.terminate_generation(actions.terminate_generation);
    }

    pub(crate) fn active_generation(&self) -> u64 {
        self.inner.borrow().swap.active().worker_generation
    }

    pub(crate) fn pending_request_count(&self) -> usize {
        self.inner
            .borrow()
            .workers
            .values()
            .map(|worker| worker.state.pending_request_count())
            .sum()
    }

    pub(crate) fn write_report(
        &self,
        object: &Object,
        command_count: usize,
        accepted_compile_section_count: usize,
        mesh_build_count: usize,
        pending_compile_job_count: usize,
    ) -> Result<(), String> {
        let now_ms = monotonic_now_ms();
        let inner = self.inner.borrow();
        set_number(
            object,
            "renderWorkerPendingRequestCount",
            inner
                .workers
                .values()
                .map(|worker| worker.state.pending_request_count())
                .sum::<usize>() as f64,
        )?;
        set_number(
            object,
            "renderWorkerGeneration",
            inner.swap.active().worker_generation as f64,
        )?;
        set_number(
            object,
            "renderWorkerStaleCompletionCount",
            inner.stale_completion_count as f64,
        )?;
        set_number(
            object,
            "compileTimingCount",
            inner.completed_timing_count as f64,
        )?;
        let timings = Array::new();
        for timing in &inner.completed_timings {
            timings.push(timing);
        }
        set_value(object, "compileTimings", &timings)?;
        set_value(
            object,
            "lastCompileTiming",
            inner.completed_timings.back().unwrap_or(&JsValue::NULL),
        )?;
        let active_timing = inner
            .workers
            .values()
            .find_map(|worker| {
                worker
                    .state
                    .active_request()
                    .and_then(|request| worker.pending.get(&request.broker_request_id))
            })
            .and_then(|pending| pending.timing.public_snapshot(now_ms).ok())
            .unwrap_or(JsValue::NULL);
        set_value(object, "activeCompileTiming", &active_timing)?;
        if accepted_compile_section_count > 0
            && let Some(report) = inner.last_compile_report.as_ref()
        {
            let report_object: &Object = report.unchecked_ref();
            set_number(report_object, "commandCount", command_count as f64)?;
            set_number(
                report_object,
                "acceptedCompileSectionCount",
                accepted_compile_section_count as f64,
            )?;
            set_number(report_object, "meshBuildCount", mesh_build_count as f64)?;
            set_number(
                report_object,
                "pendingCompileJobCount",
                pending_compile_job_count as f64,
            )?;
        }
        set_value(
            object,
            "lastCompileReport",
            inner.last_compile_report.as_ref().unwrap_or(&JsValue::NULL),
        )
    }

    pub(crate) fn terminate(&self) {
        let mut inner = self.inner.borrow_mut();
        if inner.terminated {
            return;
        }
        inner.terminated = true;
        let mut failed = Vec::new();
        for worker in inner.workers.values_mut() {
            let actions = worker.state.terminate();
            worker.transport.terminate();
            failed.extend(actions.failed);
        }
        for request in failed {
            inner.failed_requests.insert(
                (request.world_instance_id, request.client_request_id),
                "render-worker coordinator terminated".to_owned(),
            );
        }
        inner.workers.clear();
    }

    fn release_world(&self, world_instance_id: u64) {
        let mut inner = self.inner.borrow_mut();
        let generation = inner.swap.active().worker_generation;
        let releases = inner
            .workers
            .get_mut(&generation)
            .map(|worker| worker.state.request_world_release(world_instance_id))
            .unwrap_or_default();
        let _ = inner.post_world_releases(generation, releases);
    }
}

impl WebRenderWorkerWorldHandle {
    pub(crate) fn wake(&self, doorbell: &Object) -> Result<(), String> {
        self.coordinator.inner.borrow_mut().enqueue(
            self.world_instance_id,
            self.priority.get(),
            doorbell,
            monotonic_now_ms(),
        )
    }

    pub(crate) fn set_priority(&self, priority: RuntimeRenderPriority) {
        self.priority.set(priority);
        let mut inner = self.coordinator.inner.borrow_mut();
        let generation = inner.swap.active().worker_generation;
        if let Some(worker) = inner.workers.get_mut(&generation) {
            worker
                .state
                .set_world_priority(self.world_instance_id, worker_priority(priority));
        }
    }

    pub(crate) fn take_failure(&self, client_request_id: u32) -> Option<String> {
        self.coordinator
            .inner
            .borrow_mut()
            .failed_requests
            .remove(&(self.world_instance_id, client_request_id))
    }

    pub(crate) fn active_generation(&self) -> u64 {
        self.coordinator.active_generation()
    }
}

impl CoordinatorInner {
    fn enqueue(
        &mut self,
        world_instance_id: u64,
        priority: RuntimeRenderPriority,
        doorbell: &Object,
        now_ms: f64,
    ) -> Result<(), String> {
        if self.terminated {
            return Err("render-worker coordinator is terminated".to_owned());
        }
        let client_request_id = number(doorbell, "requestId") as u32;
        if client_request_id == 0 {
            return Err("render-worker doorbell has no client request id".to_owned());
        }
        self.qualified_worlds.insert(world_instance_id);
        let worlds = self.request_worlds.entry(client_request_id).or_default();
        if !worlds.is_empty() && !worlds.contains(&world_instance_id) {
            self.qualified_local_request_id_collision_count = self
                .qualified_local_request_id_collision_count
                .saturating_add(1);
        }
        worlds.insert(world_instance_id);

        let generation = self.swap.active().worker_generation;
        let worker = self
            .workers
            .get_mut(&generation)
            .ok_or_else(|| "active render-worker generation is missing".to_owned())?;
        let (request, actions) = worker.state.enqueue(
            client_request_id,
            world_instance_id,
            worker_priority(priority),
            now_ms.max(0.0) as u64,
        );
        set_number(doorbell, "requestId", request.broker_request_id as f64)?;
        set_number(doorbell, "clientRequestId", f64::from(client_request_id))?;
        set_string(doorbell, "kind", "compile-render-sections")?;
        set_string(doorbell, "worldInstanceId", &world_instance_id.to_string())?;
        set_string(doorbell, "worldPriority", priority.label())?;
        set_string(doorbell, "bindgenJsUrl", &self.bindgen_js_url)?;
        set_string(doorbell, "bindgenWasmUrl", &self.bindgen_wasm_url)?;

        self.compile_sequence = self.compile_sequence.saturating_add(1);
        self.compile_count = self.compile_count.saturating_add(1);
        let mut timing = WebCompileTiming::new(
            self.compile_sequence as f64,
            "stream".to_owned(),
            None,
            None,
            self.last_frame_count as f64,
            self.last_render_count as f64,
            now_ms,
        );
        timing.update_from_request(doorbell.as_ref());
        timing.set_begin_request_ms(0.0);
        worker.pending.insert(
            request.broker_request_id,
            PendingRequest {
                message: doorbell.clone().into(),
                timing,
            },
        );
        self.apply_actions(generation, actions)
    }

    fn poll_generation(&mut self, generation: u64, now_ms: f64) -> Result<(), String> {
        loop {
            let event = self
                .workers
                .get(&generation)
                .ok_or_else(|| format!("render-worker generation {generation} is missing"))?
                .transport
                .poll()?;
            let Some(event) = event else {
                break;
            };
            match string(&event, "kind").as_deref() {
                Some("error") => {
                    let reason = string(&event, "message")
                        .unwrap_or_else(|| "browser render Worker failed".to_owned());
                    self.fail_generation(generation, reason)?;
                    break;
                }
                Some("message") => {
                    let data = reflect_get(&event, "data").unwrap_or(JsValue::UNDEFINED);
                    self.handle_worker_message(generation, data, now_ms)?;
                }
                _ => {}
            }
        }
        let timed_out = self.workers.get(&generation).is_some_and(|worker| {
            worker
                .state
                .timed_out(now_ms.max(0.0) as u64, WORKER_TIMEOUT_MS)
        });
        if timed_out {
            self.fail_generation(
                generation,
                format!("render-worker generation {generation} timed out"),
            )?;
        }
        Ok(())
    }

    fn handle_worker_message(
        &mut self,
        generation: u64,
        data: JsValue,
        now_ms: f64,
    ) -> Result<(), String> {
        if string(&data, "kind").as_deref() == Some("render-compiler-ready") {
            if !boolean(&data, "ok") {
                return self.fail_generation(
                    generation,
                    string(&data, "reason")
                        .unwrap_or_else(|| "render-worker initialization failed".to_owned()),
                );
            }
            let actions = {
                let worker = self
                    .workers
                    .get_mut(&generation)
                    .ok_or_else(|| format!("render-worker generation {generation} is missing"))?;
                worker.ready_report = Some(data);
                worker.state.mark_ready(generation)
            };
            return self.apply_actions(generation, actions);
        }

        let broker_request_id = number(&data, "requestId") as u64;
        let completion = {
            let worker = self
                .workers
                .get_mut(&generation)
                .ok_or_else(|| format!("render-worker generation {generation} is missing"))?;
            worker.state.complete(generation, broker_request_id)
        };
        if completion.stale {
            self.stale_completion_count = self.stale_completion_count.saturating_add(1);
            return Ok(());
        }
        let mut pending = self
            .workers
            .get_mut(&generation)
            .and_then(|worker| worker.pending.remove(&broker_request_id))
            .ok_or_else(|| format!("render-worker request {broker_request_id} was not pending"))?;
        let report = self.enrich_report(&pending.message, data)?;
        // The running public snapshot already computes `now - started_at`; use it
        // as the worker round trip before finalizing the timing.
        let running = pending
            .timing
            .public_snapshot(now_ms)
            .map_err(|error| format!("snapshot running render-worker timing: {error:?}"))?;
        let elapsed = number(&running, "totalMs").max(0.001);
        pending.timing.set_worker_round_trip_ms(elapsed);
        pending.timing.update_from_worker(&report);
        pending.timing.set_decode_finish_apply_ms(0.0);
        pending.timing.finish(
            "accepted".to_owned(),
            None,
            now_ms,
            self.last_frame_count as f64,
            self.last_render_count as f64,
            self.last_frame_gap_ms,
        );
        let snapshot = pending
            .timing
            .public_snapshot(now_ms)
            .map_err(|error| format!("snapshot completed render-worker timing: {error:?}"))?;
        self.completed_timing_count = self.completed_timing_count.saturating_add(1);
        self.completed_timings.push_back(snapshot);
        while self.completed_timings.len() > MAX_RETAINED_COMPILE_TIMINGS {
            self.completed_timings.pop_front();
        }
        self.last_compile_report = Some(report);
        self.apply_actions(generation, completion.actions)
    }

    fn enrich_report(&mut self, doorbell: &JsValue, report: JsValue) -> Result<JsValue, String> {
        let report_object: &Object = report.unchecked_ref();
        let shared_bytes = number(&report, "sharedResultByteLength") as usize;
        let overflow = boolean(&report, "sharedResultOverflow");
        if boolean(&report, "sharedResultBufferUsed") {
            self.shared_result_response_count = self.shared_result_response_count.saturating_add(1);
            self.shared_result_byte_count =
                self.shared_result_byte_count.saturating_add(shared_bytes);
            self.shared_result_overflow_count = self
                .shared_result_overflow_count
                .saturating_add(usize::from(overflow));
        }
        let metrics = Object::new();
        for (name, value) in [
            ("workerInitCount", self.worker_init_count as f64),
            ("compileCount", self.compile_count as f64),
            ("qualifiedWorldCount", self.qualified_worlds.len() as f64),
            (
                "qualifiedLocalRequestIdCollisionCount",
                self.qualified_local_request_id_collision_count as f64,
            ),
            ("assetPackSendCount", self.asset_pack_send_count as f64),
            (
                "transferredRequestByteCount",
                self.transferred_request_byte_count as f64,
            ),
            ("transferredResponseByteCount", 0.0),
            (
                "sharedResultResponseCount",
                self.shared_result_response_count as f64,
            ),
            (
                "sharedResultByteCount",
                self.shared_result_byte_count as f64,
            ),
            (
                "sharedResultOverflowCount",
                self.shared_result_overflow_count as f64,
            ),
        ] {
            set_number(&metrics, name, value)?;
            set_number(report_object, name, value)?;
        }
        let target_bytes = byte_length(&reflect_get(doorbell, "targetSections"));
        let input_bytes = number(doorbell, "sharedInputByteLength");
        let request_bytes = target_bytes + input_bytes;
        for (name, value) in [
            ("requestAssetPackByteLength", 0.0),
            ("requestTargetSectionsByteLength", target_bytes),
            ("requestSnapshotInputByteLength", input_bytes),
            ("requestByteLength", request_bytes),
            ("transferredRequestByteLength", 0.0),
            ("transferredResponseByteLength", 0.0),
            ("sharedInputByteLength", input_bytes),
            (
                "sharedInputBufferCapacityBytes",
                number(doorbell, "sharedInputBufferCapacityBytes"),
            ),
            (
                "snapshotInputChunkCount",
                number(doorbell, "snapshotInputChunkCount"),
            ),
            (
                "snapshotInputClonedColumnCount",
                number(doorbell, "snapshotInputClonedColumnCount"),
            ),
            (
                "sharedResultByteLength",
                number(&report, "sharedResultByteLength"),
            ),
            (
                "sharedResultBufferCapacityBytes",
                number(&report, "sharedResultBufferCapacityBytes"),
            ),
            (
                "packedByteLength",
                number(&report, "sharedResultByteLength"),
            ),
        ] {
            set_number(&metrics, name, value)?;
            set_number(report_object, name, value)?;
        }
        for name in [
            "transportKind",
            "workerWasmInitCount",
            "workerCompileCount",
            "workerAssetLoadCount",
            "workerAssetPackInitByteLength",
            "workerAssetPackFileCount",
            "persistentAssetCatalog",
            "sharedMemorySupported",
            "snapshotInputCompileUsed",
            "generatedViewFallbackUsed",
            "sharedResultBufferUsed",
            "sharedResultOverflow",
        ] {
            if let Some(value) = reflect_get(&report, name) {
                set_value(&metrics, name, &value)?;
            }
        }
        set_bool(&metrics, "sharedInputBufferUsed", input_bytes > 0.0)?;
        set_bool(report_object, "sharedInputBufferUsed", input_bytes > 0.0)?;
        set_bool(report_object, "workerCompileUsed", true)?;
        set_value(report_object, "renderCompilerMetrics", &metrics)?;
        Reflect::delete_property(report_object, &JsValue::from_str("sharedResultBuffer"))
            .map_err(|error| format!("delete shared render-result buffer: {error:?}"))?;
        for name in [
            "commandCount",
            "acceptedCompileSectionCount",
            "meshBuildCount",
            "pendingCompileJobCount",
        ] {
            set_number(
                report_object,
                name,
                self.last_compile_report
                    .as_ref()
                    .map(|prior| number(prior, name))
                    .unwrap_or(0.0),
            )?;
        }
        Ok(report)
    }

    fn apply_actions(
        &mut self,
        generation: u64,
        actions: RenderWorkerActions,
    ) -> Result<(), String> {
        for request in actions.failed {
            self.fail_request(generation, request, "render-worker generation failed");
        }
        if let Some(request) = actions.dispatch {
            let message = self
                .workers
                .get(&generation)
                .and_then(|worker| worker.pending.get(&request.broker_request_id))
                .map(|pending| pending.message.clone())
                .ok_or_else(|| {
                    format!(
                        "render-worker request {} has no message",
                        request.broker_request_id
                    )
                })?;
            self.workers
                .get(&generation)
                .ok_or_else(|| format!("render-worker generation {generation} is missing"))?
                .transport
                .post(&message, &Array::new())?;
        }
        self.post_world_releases(generation, actions.release_worlds)
    }

    fn post_world_releases(&mut self, generation: u64, releases: Vec<u64>) -> Result<(), String> {
        for world in releases {
            let message = Object::new();
            set_string(&message, "kind", "release-render-compiler-world")?;
            set_string(&message, "worldInstanceId", &world.to_string())?;
            if let Some(worker) = self.workers.get(&generation) {
                worker.transport.post(&message.into(), &Array::new())?;
            }
        }
        Ok(())
    }

    fn fail_generation(&mut self, generation: u64, reason: String) -> Result<(), String> {
        let actions = {
            let Some(worker) = self.workers.get_mut(&generation) else {
                return Ok(());
            };
            if worker.failure.is_some() {
                return Ok(());
            }
            worker.failure = Some(reason.clone());
            worker.transport.terminate();
            worker.state.fail(generation)
        };
        for request in actions.failed {
            self.fail_request(generation, request, &reason);
        }
        Ok(())
    }

    fn fail_request(&mut self, generation: u64, request: RenderWorkerRequest, reason: &str) {
        let pending = self
            .workers
            .get_mut(&generation)
            .and_then(|worker| worker.pending.remove(&request.broker_request_id));
        if let Some(mut pending) = pending {
            let now = monotonic_now_ms();
            pending.timing.finish(
                "failed".to_owned(),
                Some(reason.to_owned()),
                now,
                self.last_frame_count as f64,
                self.last_render_count as f64,
                self.last_frame_gap_ms,
            );
            if let Ok(snapshot) = pending.timing.public_snapshot(now) {
                self.completed_timing_count = self.completed_timing_count.saturating_add(1);
                self.completed_timings.push_back(snapshot);
                while self.completed_timings.len() > MAX_RETAINED_COMPILE_TIMINGS {
                    self.completed_timings.pop_front();
                }
            }
        }
        self.failed_requests.insert(
            (request.world_instance_id, request.client_request_id),
            reason.to_owned(),
        );
    }

    fn recover_failed_active(&mut self) -> Result<(), String> {
        let active = self.swap.active();
        let active_failed = self
            .workers
            .get(&active.worker_generation)
            .is_some_and(|worker| worker.failure.is_some());
        if !active_failed {
            return Ok(());
        }
        if self.swap.candidate().is_some() || self.swap.previous().is_some() {
            return Err(format!(
                "active render-worker generation {} failed during asset replacement",
                active.worker_generation
            ));
        }
        let payload = self
            .workers
            .get(&active.worker_generation)
            .map(|worker| worker.payload.clone())
            .ok_or_else(|| "failed render-worker generation is missing".to_owned())?;
        let actions = self.swap.restart_active();
        let replacement = actions
            .activated
            .expect("active restart produces a replacement generation");
        let worker = WorkerRuntime::new(
            replacement,
            &self.factory,
            &self.bindgen_js_url,
            &self.bindgen_wasm_url,
            payload.clone(),
        )?;
        self.worker_init_count = self.worker_init_count.saturating_add(1);
        self.asset_pack_send_count = self.asset_pack_send_count.saturating_add(1);
        self.transferred_request_byte_count = self
            .transferred_request_byte_count
            .saturating_add(payload.byte_length());
        self.terminate_generation(actions.terminate_generation);
        self.workers.insert(replacement.worker_generation, worker);
        Ok(())
    }

    fn terminate_generation(&mut self, generation: Option<u64>) {
        let Some(generation) = generation else {
            return;
        };
        if let Some(mut worker) = self.workers.remove(&generation) {
            worker.transport.terminate();
            let actions = worker.state.terminate();
            for request in actions.failed {
                self.failed_requests.insert(
                    (request.world_instance_id, request.client_request_id),
                    "render-worker generation retired".to_owned(),
                );
            }
        }
    }
}

fn init_message(
    bindgen_js_url: &str,
    bindgen_wasm_url: &str,
    payload: &WorkerAssetPayload,
) -> Result<(JsValue, Array), String> {
    let message = Object::new();
    let transfer = Array::new();
    set_string(&message, "kind", "init-render-compiler")?;
    set_number(&message, "requestId", 0.0)?;
    set_string(&message, "bindgenJsUrl", bindgen_js_url)?;
    set_string(&message, "bindgenWasmUrl", bindgen_wasm_url)?;
    match payload {
        WorkerAssetPayload::Reference(bytes) => {
            attach_bytes(&message, &transfer, "assetPack", bytes)?;
        }
        WorkerAssetPayload::Selection {
            authored,
            reference,
            fallback,
            authored_enabled,
            reference_enabled,
            epoch,
        } => {
            attach_bytes(&message, &transfer, "authoredPack", authored)?;
            attach_bytes(&message, &transfer, "referencePack", reference)?;
            attach_bytes(&message, &transfer, "fallbackPack", fallback)?;
            set_bool(&message, "authoredEnabled", *authored_enabled)?;
            set_bool(&message, "referenceEnabled", *reference_enabled)?;
            set_number(&message, "assetEpoch", *epoch as f64)?;
        }
    }
    Ok((message.into(), transfer))
}

fn attach_bytes(
    message: &Object,
    transfer: &Array,
    name: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let array = Uint8Array::from(bytes);
    transfer.push(&array.buffer());
    set_value(message, name, array.as_ref())
}

fn worker_priority(priority: RuntimeRenderPriority) -> RenderWorkerPriority {
    match priority {
        RuntimeRenderPriority::Active => RenderWorkerPriority::Active,
        RuntimeRenderPriority::Standby => RenderWorkerPriority::Standby,
    }
}

fn method(value: &JsValue, name: &str) -> Result<Function, String> {
    reflect_get(value, name)
        .and_then(|value| value.dyn_into::<Function>().ok())
        .ok_or_else(|| format!("render-worker transport method {name} is missing"))
}

fn reflect_get(value: &JsValue, name: &str) -> Option<JsValue> {
    Reflect::get(value, &JsValue::from_str(name)).ok()
}

fn string(value: &JsValue, name: &str) -> Option<String> {
    reflect_get(value, name).and_then(|value| value.as_string())
}

fn number(value: &JsValue, name: &str) -> f64 {
    reflect_get(value, name)
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
}

fn boolean(value: &JsValue, name: &str) -> bool {
    reflect_get(value, name).is_some_and(|value| value.as_bool().unwrap_or(false))
}

fn byte_length(value: &Option<JsValue>) -> f64 {
    value
        .as_ref()
        .map(|value| number(value, "byteLength"))
        .unwrap_or(0.0)
}

fn monotonic_now_ms() -> f64 {
    let global = js_sys::global();
    let Some(performance) = reflect_get(&global, "performance") else {
        return js_sys::Date::now();
    };
    let Ok(now) = method(&performance, "now") else {
        return js_sys::Date::now();
    };
    now.call0(&performance)
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or_else(js_sys::Date::now)
}

fn set_value(object: &Object, name: &str, value: &JsValue) -> Result<(), String> {
    Reflect::set(object, &JsValue::from_str(name), value)
        .map(|_| ())
        .map_err(|error| format!("set render-worker field {name}: {error:?}"))
}

fn set_number(object: &Object, name: &str, value: f64) -> Result<(), String> {
    set_value(object, name, &JsValue::from_f64(value))
}

fn set_string(object: &Object, name: &str, value: &str) -> Result<(), String> {
    set_value(object, name, &JsValue::from_str(value))
}

fn set_bool(object: &Object, name: &str, value: bool) -> Result<(), String> {
    set_value(object, name, &JsValue::from_bool(value))
}
