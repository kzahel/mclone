use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use js_sys::{Array, Object, Reflect, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

use crate::{WasmServerJobWorkerConfig, WorkerFrameMetrics};

pub(crate) struct WasmJobWorker {
    worker: Worker,
    job_kind: &'static str,
    bindgen_js_url: String,
    bindgen_wasm_url: String,
    next_request_id: u32,
    pending_count: Rc<Cell<usize>>,
    request_start_ms_by_id: Rc<RefCell<BTreeMap<u32, f64>>>,
    frame_metrics: Rc<RefCell<WorkerFrameMetrics>>,
    completed_frames: Rc<RefCell<VecDeque<Vec<u8>>>>,
    last_error: Rc<RefCell<Option<String>>>,
    message_closure: Closure<dyn FnMut(MessageEvent)>,
    error_closure: Closure<dyn FnMut(ErrorEvent)>,
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

        let pending_count = Rc::new(Cell::new(0usize));
        let request_start_ms_by_id: Rc<RefCell<BTreeMap<u32, f64>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let frame_metrics = Rc::new(RefCell::new(WorkerFrameMetrics::message_transfer()));
        let completed_frames = Rc::new(RefCell::new(VecDeque::new()));
        let last_error = Rc::new(RefCell::new(None));

        let message_closure = {
            let pending_count = Rc::clone(&pending_count);
            let request_start_ms_by_id = Rc::clone(&request_start_ms_by_id);
            let frame_metrics = Rc::clone(&frame_metrics);
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
                    let reason = string_prop(&data, "reason")
                        .unwrap_or_else(|| "server job worker returned an error".to_owned());
                    *last_error.borrow_mut() = Some(reason);
                    return;
                }
                let Some(frame) = reflect_get(&data, "frame") else {
                    *last_error.borrow_mut() =
                        Some("server job worker returned no frame".to_owned());
                    return;
                };
                if !frame.is_instance_of::<Uint8Array>() {
                    *last_error.borrow_mut() =
                        Some("server job worker frame was not a Uint8Array".to_owned());
                    return;
                }
                let frame = Uint8Array::new(&frame);
                frame_metrics
                    .borrow_mut()
                    .record_response(frame.byte_length() as usize);
                completed_frames.borrow_mut().push_back(frame.to_vec());
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
            next_request_id: 1,
            pending_count,
            request_start_ms_by_id,
            frame_metrics,
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

        let request_bytes = frame.len();
        let bytes = Uint8Array::from(frame.as_slice());
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
            .map_err(|error| format!("failed to post server job frame: {error:?}"))?;
        self.request_start_ms_by_id
            .borrow_mut()
            .insert(request_id, js_sys::Date::now());
        self.pending_count
            .set(self.pending_count.get().saturating_add(1));
        let pending_count = self.pending_count.get();
        let mut frame_metrics = self.frame_metrics.borrow_mut();
        frame_metrics.record_request(request_bytes);
        frame_metrics.observe_pending_frames(pending_count);
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
}

impl std::fmt::Debug for WasmJobWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmJobWorker")
            .field("job_kind", &self.job_kind)
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
