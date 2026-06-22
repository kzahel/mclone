use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use js_sys::{Array, Atomics, Int32Array, Object, Reflect, SharedArrayBuffer, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

use crate::{WasmServerJobWorkerConfig, WorkerFrameMetrics, WorkerFrameTransportKind};

const SHARED_CONTROL_SLOTS: u32 = 4;
const SHARED_CONTROL_BYTES: u32 = SHARED_CONTROL_SLOTS * 4;
const SHARED_STATUS_INDEX: u32 = 0;
const SHARED_REQUEST_BYTES_INDEX: u32 = 1;
const SHARED_RESPONSE_BYTES_INDEX: u32 = 2;
const SHARED_STATUS_PENDING: i32 = 1;
const SHARED_STATUS_COMPLETE: i32 = 2;
const MAX_SHARED_POOL_SLOTS: usize = 2;
const MIN_SHARED_REQUEST_BYTES: u32 = 4 * 1024;
const DEFAULT_SHARED_RESPONSE_BYTES: u32 = 8 * 1024 * 1024;
const WORLDGEN_SHARED_RESPONSE_BYTES: u32 = 64 * 1024 * 1024;
const LIGHT_STATUS_SHARED_RESPONSE_BYTES: u32 = 2 * 1024 * 1024;

pub(crate) struct WasmJobWorker {
    worker: Worker,
    job_kind: &'static str,
    bindgen_js_url: String,
    bindgen_wasm_url: String,
    transport_kind: WorkerFrameTransportKind,
    next_request_id: u32,
    pending_count: Rc<Cell<usize>>,
    request_start_ms_by_id: Rc<RefCell<BTreeMap<u32, f64>>>,
    frame_metrics: Rc<RefCell<WorkerFrameMetrics>>,
    shared_pool: Rc<RefCell<Vec<SharedJobSlot>>>,
    shared_inflight: Rc<RefCell<BTreeMap<u32, SharedJobSlot>>>,
    completed_frames: Rc<RefCell<VecDeque<Vec<u8>>>>,
    last_error: Rc<RefCell<Option<String>>>,
    message_closure: Closure<dyn FnMut(MessageEvent)>,
    error_closure: Closure<dyn FnMut(ErrorEvent)>,
}

struct SharedJobSlot {
    control_buffer: SharedArrayBuffer,
    request_buffer: SharedArrayBuffer,
    response_buffer: SharedArrayBuffer,
    request_capacity: u32,
    response_capacity: u32,
}

impl SharedJobSlot {
    fn new(request_capacity: u32, response_capacity: u32) -> Self {
        Self {
            control_buffer: SharedArrayBuffer::new(SHARED_CONTROL_BYTES),
            request_buffer: SharedArrayBuffer::new(request_capacity),
            response_buffer: SharedArrayBuffer::new(response_capacity),
            request_capacity,
            response_capacity,
        }
    }

    fn capacity_bytes(&self) -> usize {
        (SHARED_CONTROL_BYTES as usize)
            .saturating_add(self.request_capacity as usize)
            .saturating_add(self.response_capacity as usize)
    }
}

impl WasmJobWorker {
    pub(crate) fn new(
        name: &'static str,
        job_kind: &'static str,
        config: &WasmServerJobWorkerConfig,
    ) -> Result<Self, String> {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        options.set_name(name);
        let worker = Worker::new_with_options(&config.worker_url, &options)
            .map_err(|error| format!("failed to spawn {name}: {error:?}"))?;

        let transport_kind = if shared_memory_transport_available() {
            WorkerFrameTransportKind::SharedMemory
        } else {
            WorkerFrameTransportKind::MessageTransfer
        };
        let pending_count = Rc::new(Cell::new(0usize));
        let request_start_ms_by_id: Rc<RefCell<BTreeMap<u32, f64>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let frame_metrics = Rc::new(RefCell::new(match transport_kind {
            WorkerFrameTransportKind::SharedMemory => WorkerFrameMetrics::shared_memory(),
            _ => WorkerFrameMetrics::message_transfer(),
        }));
        let shared_pool = Rc::new(RefCell::new(Vec::new()));
        let shared_inflight = Rc::new(RefCell::new(BTreeMap::new()));
        let completed_frames = Rc::new(RefCell::new(VecDeque::new()));
        let last_error = Rc::new(RefCell::new(None));

        let message_closure = {
            let pending_count = Rc::clone(&pending_count);
            let request_start_ms_by_id = Rc::clone(&request_start_ms_by_id);
            let frame_metrics = Rc::clone(&frame_metrics);
            let shared_pool = Rc::clone(&shared_pool);
            let shared_inflight = Rc::clone(&shared_inflight);
            let completed_frames = Rc::clone(&completed_frames);
            let last_error = Rc::clone(&last_error);
            Closure::wrap(Box::new(move |event: MessageEvent| {
                pending_count.set(pending_count.get().saturating_sub(1));
                let data = event.data();
                let request_id = number_prop(&data, "requestId")
                    .map(|value| value as u32)
                    .unwrap_or(0);
                if let Some(start_ms) = request_start_ms_by_id.borrow_mut().remove(&request_id) {
                    let request_us =
                        ((js_sys::Date::now() - start_ms).max(0.0_f64) * 1000.0) as u128;
                    frame_metrics
                        .borrow_mut()
                        .record_request_time_us(request_us);
                }
                if bool_prop(&data, "ok") == Some(false) {
                    if let Some(slot) = shared_inflight.borrow_mut().remove(&request_id) {
                        release_shared_slot(slot, &shared_pool, &frame_metrics);
                    }
                    let reason = string_prop(&data, "reason")
                        .unwrap_or_else(|| "server job worker returned an error".to_owned());
                    *last_error.borrow_mut() = Some(reason);
                    return;
                }
                let frame_result =
                    if string_prop(&data, "transportKind").as_deref() == Some("shared-memory") {
                        let mut slot = shared_inflight.borrow_mut().remove(&request_id);
                        let response = shared_response_frame(&data);
                        if let Some(mut slot) = slot.take() {
                            if let Ok(response) = &response {
                                record_shared_response_metrics(&frame_metrics, response);
                                if !response.pooled_response {
                                    grow_slot_response_buffer(
                                        &mut slot,
                                        response.frame.len(),
                                        &frame_metrics,
                                    );
                                }
                            }
                            release_shared_slot(slot, &shared_pool, &frame_metrics);
                        }
                        response.map(|response| response.frame)
                    } else {
                        transferred_response_frame(&data)
                    };
                match frame_result {
                    Ok(frame) => {
                        frame_metrics.borrow_mut().record_response(frame.len());
                        completed_frames.borrow_mut().push_back(frame);
                    }
                    Err(error) => {
                        *last_error.borrow_mut() = Some(error);
                    }
                }
            }) as Box<dyn FnMut(_)>)
        };
        worker.set_onmessage(Some(message_closure.as_ref().unchecked_ref()));

        let error_closure = {
            let last_error = Rc::clone(&last_error);
            Closure::wrap(Box::new(move |event: ErrorEvent| {
                let message = if event.message().is_empty() {
                    "server job worker failed".to_owned()
                } else {
                    event.message()
                };
                *last_error.borrow_mut() = Some(message);
            }) as Box<dyn FnMut(_)>)
        };
        worker.set_onerror(Some(error_closure.as_ref().unchecked_ref()));

        Ok(Self {
            worker,
            job_kind,
            bindgen_js_url: config.bindgen_js_url.clone(),
            bindgen_wasm_url: config.bindgen_wasm_url.clone(),
            transport_kind,
            next_request_id: 1,
            pending_count,
            request_start_ms_by_id,
            frame_metrics,
            shared_pool,
            shared_inflight,
            completed_frames,
            last_error,
            message_closure,
            error_closure,
        })
    }

    pub(crate) fn post_frame(&mut self, frame: Vec<u8>) -> Result<(), String> {
        self.raise_last_error()?;
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);

        match self.transport_kind {
            WorkerFrameTransportKind::SharedMemory => self.post_shared_frame(request_id, &frame)?,
            _ => self.post_transferred_frame(request_id, &frame)?,
        }
        self.request_start_ms_by_id
            .borrow_mut()
            .insert(request_id, js_sys::Date::now());
        self.pending_count
            .set(self.pending_count.get().saturating_add(1));
        let pending_count = self.pending_count.get();
        let mut frame_metrics = self.frame_metrics.borrow_mut();
        frame_metrics.record_request(frame.len());
        frame_metrics.observe_pending_frames(pending_count);
        Ok(())
    }

    fn post_transferred_frame(&self, request_id: u32, frame: &[u8]) -> Result<(), String> {
        let bytes = Uint8Array::from(frame);
        let transfer = Array::new();
        transfer.push(&bytes.buffer());
        let message = Object::new();
        set_string(&message, "kind", self.job_kind)?;
        set_number(&message, "requestId", f64::from(request_id))?;
        set_string(&message, "bindgenJsUrl", &self.bindgen_js_url)?;
        set_string(&message, "bindgenWasmUrl", &self.bindgen_wasm_url)?;
        Reflect::set(&message, &JsValue::from_str("frame"), &bytes)
            .map_err(|error| format!("failed to attach server job frame: {error:?}"))?;
        self.worker
            .post_message_with_transfer(&message, &transfer)
            .map_err(|error| format!("failed to post server job frame: {error:?}"))
    }

    fn post_shared_frame(&mut self, request_id: u32, frame: &[u8]) -> Result<(), String> {
        let request_bytes = u32::try_from(frame.len()).map_err(|_| {
            format!(
                "server job frame is too large for shared transport: {} bytes",
                frame.len()
            )
        })?;
        let request_bytes_i32 = i32::try_from(request_bytes).map_err(|_| {
            format!(
                "server job frame is too large for shared byte counters: {} bytes",
                frame.len()
            )
        })?;
        let slot = self.acquire_shared_slot(request_bytes);
        let control = Int32Array::new(slot.control_buffer.as_ref());
        Atomics::store(&control, SHARED_STATUS_INDEX, SHARED_STATUS_PENDING).map_err(|error| {
            format!(
                "failed to initialize shared server job status: {}",
                js_error_string(&error)
            )
        })?;
        Atomics::store(&control, SHARED_REQUEST_BYTES_INDEX, request_bytes_i32).map_err(
            |error| {
                format!(
                    "failed to initialize shared server job byte count: {}",
                    js_error_string(&error)
                )
            },
        )?;
        Atomics::store(&control, SHARED_RESPONSE_BYTES_INDEX, 0).map_err(|error| {
            format!(
                "failed to clear shared server job response byte count: {}",
                js_error_string(&error)
            )
        })?;

        let request_view = Uint8Array::new_with_byte_offset_and_length(
            slot.request_buffer.as_ref(),
            0,
            request_bytes,
        );
        request_view.copy_from(frame);

        let message = Object::new();
        set_string(&message, "kind", self.job_kind)?;
        set_string(
            &message,
            "transportKind",
            WorkerFrameTransportKind::SharedMemory.label(),
        )?;
        set_number(&message, "requestId", f64::from(request_id))?;
        set_string(&message, "bindgenJsUrl", &self.bindgen_js_url)?;
        set_string(&message, "bindgenWasmUrl", &self.bindgen_wasm_url)?;
        set_number(&message, "requestBytes", f64::from(request_bytes))?;
        set_number(
            &message,
            "responseCapacity",
            f64::from(slot.response_capacity),
        )?;
        Reflect::set(
            &message,
            &JsValue::from_str("controlBuffer"),
            slot.control_buffer.as_ref(),
        )
        .map_err(|error| format!("failed to attach shared server job control buffer: {error:?}"))?;
        Reflect::set(
            &message,
            &JsValue::from_str("requestBuffer"),
            slot.request_buffer.as_ref(),
        )
        .map_err(|error| format!("failed to attach shared server job request buffer: {error:?}"))?;
        Reflect::set(
            &message,
            &JsValue::from_str("responseBuffer"),
            slot.response_buffer.as_ref(),
        )
        .map_err(|error| {
            format!("failed to attach shared server job response buffer: {error:?}")
        })?;
        if let Err(error) = self.worker.post_message(&message) {
            release_shared_slot(slot, &self.shared_pool, &self.frame_metrics);
            return Err(format!("failed to post shared server job frame: {error:?}"));
        }
        self.shared_inflight.borrow_mut().insert(request_id, slot);
        Ok(())
    }

    pub(crate) fn drain_frames(&mut self) -> Result<Vec<Vec<u8>>, String> {
        self.raise_last_error()?;
        Ok(self.completed_frames.borrow_mut().drain(..).collect())
    }

    pub(crate) fn pending_count(&self) -> usize {
        self.pending_count.get()
    }

    pub(crate) fn has_completed_frame(&self) -> bool {
        !self.completed_frames.borrow().is_empty()
    }

    pub(crate) fn frame_metrics(&self) -> WorkerFrameMetrics {
        *self.frame_metrics.borrow()
    }

    fn raise_last_error(&self) -> Result<(), String> {
        if let Some(error) = self.last_error.borrow_mut().take() {
            Err(error)
        } else {
            Ok(())
        }
    }

    fn acquire_shared_slot(&mut self, request_bytes: u32) -> SharedJobSlot {
        let mut slot = if let Some(slot) = self.shared_pool.borrow_mut().pop() {
            self.frame_metrics
                .borrow_mut()
                .record_shared_buffer_pool_hit();
            slot
        } else {
            self.frame_metrics
                .borrow_mut()
                .record_shared_buffer_pool_miss();
            let slot = SharedJobSlot::new(
                shared_capacity_for_len(request_bytes, MIN_SHARED_REQUEST_BYTES),
                initial_shared_response_capacity(self.job_kind),
            );
            self.frame_metrics
                .borrow_mut()
                .record_shared_buffer_capacity_delta(slot.capacity_bytes());
            slot
        };
        ensure_slot_request_capacity(&mut slot, request_bytes, &self.frame_metrics);
        slot
    }
}

impl std::fmt::Debug for WasmJobWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmJobWorker")
            .field("job_kind", &self.job_kind)
            .field("transport_kind", &self.transport_kind)
            .field("pending_count", &self.pending_count())
            .field("frame_metrics", &self.frame_metrics())
            .field("completed_frames", &self.completed_frames.borrow().len())
            .finish_non_exhaustive()
    }
}

impl Drop for WasmJobWorker {
    fn drop(&mut self) {
        self.worker.terminate();
        let _ = &self.message_closure;
        let _ = &self.error_closure;
    }
}

fn reflect_get(value: &JsValue, name: &str) -> Option<JsValue> {
    Reflect::get(value, &JsValue::from_str(name))
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

fn bool_prop(value: &JsValue, name: &str) -> Option<bool> {
    reflect_get(value, name).and_then(|value| value.as_bool())
}

fn number_prop(value: &JsValue, name: &str) -> Option<f64> {
    reflect_get(value, name).and_then(|value| value.as_f64())
}

fn string_prop(value: &JsValue, name: &str) -> Option<String> {
    reflect_get(value, name).and_then(|value| value.as_string())
}

fn transferred_response_frame(value: &JsValue) -> Result<Vec<u8>, String> {
    let Some(frame) = reflect_get(value, "frame") else {
        return Err("server job worker returned no frame".to_owned());
    };
    if !frame.is_instance_of::<Uint8Array>() {
        return Err("server job worker frame was not a Uint8Array".to_owned());
    }
    Ok(Uint8Array::new(&frame).to_vec())
}

struct SharedResponseFrame {
    frame: Vec<u8>,
    pooled_response: bool,
}

fn shared_response_frame(value: &JsValue) -> Result<SharedResponseFrame, String> {
    let Some(control_buffer) = reflect_get(value, "controlBuffer") else {
        return Err("shared server job worker returned no control buffer".to_owned());
    };
    let control = Int32Array::new(&control_buffer);
    let status = Atomics::load(&control, SHARED_STATUS_INDEX).map_err(|error| {
        format!(
            "failed to read shared server job status: {}",
            js_error_string(&error)
        )
    })?;
    if status != SHARED_STATUS_COMPLETE {
        return Err(format!(
            "shared server job worker completed with unexpected status {status}"
        ));
    }
    let response_bytes = Atomics::load(&control, SHARED_RESPONSE_BYTES_INDEX).map_err(|error| {
        format!(
            "failed to read shared server job byte count: {}",
            js_error_string(&error)
        )
    })?;
    if response_bytes < 0 {
        return Err(format!(
            "shared server job worker returned negative byte count {response_bytes}"
        ));
    }
    let Some(frame_buffer) = reflect_get(value, "frameBuffer") else {
        return Err("shared server job worker returned no frame buffer".to_owned());
    };
    if !frame_buffer.is_instance_of::<SharedArrayBuffer>() {
        return Err("shared server job worker frame buffer was not a SharedArrayBuffer".to_owned());
    }
    let frame =
        Uint8Array::new_with_byte_offset_and_length(&frame_buffer, 0, response_bytes as u32);
    Ok(SharedResponseFrame {
        frame: frame.to_vec(),
        pooled_response: bool_prop(value, "pooledResponse").unwrap_or(false),
    })
}

fn initial_shared_response_capacity(job_kind: &str) -> u32 {
    match job_kind {
        "worldgen" => WORLDGEN_SHARED_RESPONSE_BYTES,
        "light-status" => LIGHT_STATUS_SHARED_RESPONSE_BYTES,
        _ => DEFAULT_SHARED_RESPONSE_BYTES,
    }
}

fn shared_capacity_for_len(len: u32, minimum: u32) -> u32 {
    let wanted = len.max(minimum);
    wanted.checked_next_power_of_two().unwrap_or(wanted)
}

fn ensure_slot_request_capacity(
    slot: &mut SharedJobSlot,
    request_bytes: u32,
    frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
) {
    if request_bytes <= slot.request_capacity {
        return;
    }
    let previous_capacity = slot.request_capacity;
    let request_capacity = shared_capacity_for_len(request_bytes, MIN_SHARED_REQUEST_BYTES);
    slot.request_buffer = SharedArrayBuffer::new(request_capacity);
    slot.request_capacity = request_capacity;
    frame_metrics
        .borrow_mut()
        .record_shared_buffer_capacity_delta((request_capacity - previous_capacity) as usize);
}

fn grow_slot_response_buffer(
    slot: &mut SharedJobSlot,
    response_bytes: usize,
    frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
) {
    let Ok(response_bytes) = u32::try_from(response_bytes) else {
        return;
    };
    if response_bytes <= slot.response_capacity {
        return;
    }
    let previous_capacity = slot.response_capacity;
    let response_capacity = shared_capacity_for_len(response_bytes, DEFAULT_SHARED_RESPONSE_BYTES);
    slot.response_buffer = SharedArrayBuffer::new(response_capacity);
    slot.response_capacity = response_capacity;
    frame_metrics
        .borrow_mut()
        .record_shared_buffer_capacity_delta((response_capacity - previous_capacity) as usize);
}

fn release_shared_slot(
    slot: SharedJobSlot,
    shared_pool: &Rc<RefCell<Vec<SharedJobSlot>>>,
    frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
) {
    let mut shared_pool = shared_pool.borrow_mut();
    if shared_pool.len() < MAX_SHARED_POOL_SLOTS {
        shared_pool.push(slot);
    } else {
        frame_metrics
            .borrow_mut()
            .record_shared_buffer_pool_drop(slot.capacity_bytes());
    }
}

fn record_shared_response_metrics(
    frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
    response: &SharedResponseFrame,
) {
    if response.pooled_response {
        frame_metrics
            .borrow_mut()
            .record_shared_buffer_pooled_response();
    } else {
        frame_metrics
            .borrow_mut()
            .record_shared_buffer_fallback_response();
    }
}

fn shared_memory_transport_available() -> bool {
    let global = js_sys::global();
    global_function_available(&global, "SharedArrayBuffer")
        && global_object_method_available(&global, "Atomics", "load")
        && global_object_method_available(&global, "Atomics", "store")
        && global_object_method_available(&global, "Atomics", "notify")
}

fn global_function_available(global: &JsValue, name: &str) -> bool {
    Reflect::get(global, &JsValue::from_str(name))
        .ok()
        .is_some_and(|value| value.is_function())
}

fn global_object_method_available(global: &JsValue, object_name: &str, method_name: &str) -> bool {
    Reflect::get(global, &JsValue::from_str(object_name))
        .ok()
        .and_then(|object| Reflect::get(&object, &JsValue::from_str(method_name)).ok())
        .is_some_and(|value| value.is_function())
}

fn set_number(object: &Object, name: &str, value: f64) -> Result<(), String> {
    Reflect::set(object, &JsValue::from_str(name), &JsValue::from_f64(value))
        .map_err(|error| format!("failed to set {name}: {error:?}"))?;
    Ok(())
}

fn set_string(object: &Object, name: &str, value: &str) -> Result<(), String> {
    Reflect::set(object, &JsValue::from_str(name), &JsValue::from_str(value))
        .map_err(|error| format!("failed to set {name}: {error:?}"))?;
    Ok(())
}

fn js_error_string(value: &JsValue) -> String {
    if let Some(message) = value.as_string() {
        return message;
    }
    if let Some(message) = reflect_get(value, "message").and_then(|value| value.as_string()) {
        return message;
    }
    format!("{value:?}")
}
