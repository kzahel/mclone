use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use js_sys::{Array, ArrayBuffer, Function, Object, Promise, Reflect, Uint8Array};
use mclone_app_runtime::client_connection::{
    ClientConnectionDrainResult, ClientConnectionQueueMetrics, QueuedServerUpdate,
};
use mclone_protocol::{
    ClientCommand, ClientDisconnectReason, ClientIdentity, DisconnectReason, ServerUpdate,
    decode_client_command, decode_server_update, encode_client_command, encode_server_update,
};
use mclone_server::{
    ServerRunnerDiagnostics, ServerRunnerKind, WorkerFrameMetrics, WorkerFrameTransportKind,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

const DEFAULT_REMOTE_WORKER_URL: &str = "./mclone-remote-websocket-worker.js";
const DEFAULT_BINDGEN_JS_URL: &str = "./pkg/mclone_web_client.js";
const DEFAULT_BINDGEN_WASM_URL: &str = "./pkg/mclone_web_client_bg.wasm";
const MAX_MAIN_UPDATE_FRAMES: usize = 4_096;
const MAX_MAIN_UPDATE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
struct WebSocketWorkerConfig {
    url: String,
    worker_url: String,
    bindgen_js_url: String,
    bindgen_wasm_url: String,
    identity: ClientIdentity,
}

impl WebSocketWorkerConfig {
    fn production_defaults(url: impl Into<String>, identity: ClientIdentity) -> Self {
        Self {
            url: url.into(),
            worker_url: DEFAULT_REMOTE_WORKER_URL.to_owned(),
            bindgen_js_url: DEFAULT_BINDGEN_JS_URL.to_owned(),
            bindgen_wasm_url: DEFAULT_BINDGEN_WASM_URL.to_owned(),
            identity,
        }
    }
}

pub struct WebSocketServerSession {
    config: WebSocketWorkerConfig,
    worker: Worker,
    running: Rc<RefCell<bool>>,
    command_queue_depth: Rc<RefCell<usize>>,
    queued_updates: Rc<RefCell<VecDeque<QueuedWebSocketUpdate>>>,
    queued_update_bytes: Rc<RefCell<usize>>,
    frame_metrics: Rc<RefCell<WorkerFrameMetrics>>,
    last_error: Rc<RefCell<Option<String>>>,
    day_time: Rc<RefCell<u64>>,
    message_closure: Closure<dyn FnMut(MessageEvent)>,
    error_closure: Closure<dyn FnMut(ErrorEvent)>,
    shutdown_requested: bool,
}

impl std::fmt::Debug for WebSocketServerSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocketServerSession")
            .field("url", &self.config.url)
            .field("worker_url", &self.config.worker_url)
            .field("running", &self.running.borrow())
            .field("command_queue_depth", &self.command_queue_depth.borrow())
            .field("queued_updates", &self.queued_updates.borrow().len())
            .field("queued_update_bytes", &self.queued_update_bytes.borrow())
            .field("frame_metrics", &self.frame_metrics.borrow())
            .field("last_error", &self.last_error.borrow())
            .finish_non_exhaustive()
    }
}

impl WebSocketServerSession {
    pub async fn connect(url: impl Into<String>) -> Result<Self, String> {
        let profile = mclone_app_runtime::local_profile::load_or_create_web_local_player_profile()
            .map_err(|error| format!("load browser player profile: {error:#}"))?;
        Self::connect_with_identity(url, profile.client_identity()).await
    }

    pub async fn connect_with_identity(
        url: impl Into<String>,
        identity: ClientIdentity,
    ) -> Result<Self, String> {
        Self::connect_with_config(WebSocketWorkerConfig::production_defaults(url, identity)).await
    }

    async fn connect_with_config(config: WebSocketWorkerConfig) -> Result<Self, String> {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        options.set_name("mclone-remote-websocket");
        let worker = Worker::new_with_options(&config.worker_url, &options)
            .map_err(|error| format!("failed to spawn remote websocket worker: {error:?}"))?;

        let running = Rc::new(RefCell::new(false));
        let command_queue_depth = Rc::new(RefCell::new(0_usize));
        let queued_updates = Rc::new(RefCell::new(VecDeque::new()));
        let queued_update_bytes = Rc::new(RefCell::new(0_usize));
        let frame_metrics = Rc::new(RefCell::new(WorkerFrameMetrics::websocket()));
        let last_error = Rc::new(RefCell::new(None));
        let day_time = Rc::new(RefCell::new(0_u64));
        let pending_ready: Rc<RefCell<Option<PendingWorkerReady>>> = Rc::new(RefCell::new(None));

        let message_closure = {
            let worker = worker.clone();
            let running = Rc::clone(&running);
            let command_queue_depth = Rc::clone(&command_queue_depth);
            let queued_updates = Rc::clone(&queued_updates);
            let queued_update_bytes = Rc::clone(&queued_update_bytes);
            let frame_metrics = Rc::clone(&frame_metrics);
            let last_error = Rc::clone(&last_error);
            let pending_ready = Rc::clone(&pending_ready);
            Closure::wrap(Box::new(move |event: MessageEvent| {
                let result = handle_worker_message(
                    event.data(),
                    &running,
                    &command_queue_depth,
                    &queued_updates,
                    &queued_update_bytes,
                    &frame_metrics,
                    &pending_ready,
                );
                if let Err(error) = result {
                    let _ = queue_transport_disconnect(
                        &error,
                        &queued_updates,
                        &queued_update_bytes,
                        &frame_metrics,
                    );
                    record_worker_error(&running, &last_error, &pending_ready, error);
                    worker.terminate();
                }
            }) as Box<dyn FnMut(_)>)
        };
        worker.set_onmessage(Some(message_closure.as_ref().unchecked_ref()));

        let error_closure = {
            let running = Rc::clone(&running);
            let queued_updates = Rc::clone(&queued_updates);
            let queued_update_bytes = Rc::clone(&queued_update_bytes);
            let frame_metrics = Rc::clone(&frame_metrics);
            let last_error = Rc::clone(&last_error);
            let pending_ready = Rc::clone(&pending_ready);
            Closure::wrap(Box::new(move |event: ErrorEvent| {
                let message = if event.message().is_empty() {
                    "remote websocket worker failed".to_owned()
                } else {
                    event.message()
                };
                let _ = queue_transport_disconnect(
                    &message,
                    &queued_updates,
                    &queued_update_bytes,
                    &frame_metrics,
                );
                record_worker_error(&running, &last_error, &pending_ready, message);
            }) as Box<dyn FnMut(_)>)
        };
        worker.set_onerror(Some(error_closure.as_ref().unchecked_ref()));

        let ready = Promise::new(&mut {
            let pending_ready = Rc::clone(&pending_ready);
            move |resolve: Function, reject: Function| {
                *pending_ready.borrow_mut() = Some(PendingWorkerReady { resolve, reject });
            }
        });
        let start = Object::new();
        set_string(&start, "kind", "start")?;
        set_string(&start, "url", &config.url)?;
        set_string(&start, "bindgenJsUrl", &config.bindgen_js_url)?;
        set_string(&start, "bindgenWasmUrl", &config.bindgen_wasm_url)?;
        set_string(&start, "displayName", &config.identity.display_name)?;
        let profile_id = Uint8Array::from(config.identity.profile_id.bytes().as_slice());
        Reflect::set(&start, &JsValue::from_str("profileId"), &profile_id)
            .map_err(|error| format!("failed to attach remote profile UUID: {error:?}"))?;
        if let Err(error) = worker.post_message(&start) {
            worker.terminate();
            return Err(format!(
                "failed to start remote websocket worker: {error:?}"
            ));
        }
        JsFuture::from(ready).await.map_err(|error| {
            worker.terminate();
            format!(
                "remote websocket worker startup failed: {}",
                js_error_string(&error)
            )
        })?;

        Ok(Self {
            config,
            worker,
            running,
            command_queue_depth,
            queued_updates,
            queued_update_bytes,
            frame_metrics,
            last_error,
            day_time,
            message_closure,
            error_closure,
            shutdown_requested: false,
        })
    }

    pub fn queue_command(&mut self, command: ClientCommand) -> Result<bool, String> {
        if self.shutdown_requested || !*self.running.borrow() {
            return Err(self
                .last_error
                .borrow()
                .clone()
                .unwrap_or_else(|| "remote websocket worker is not running".to_owned()));
        }
        let frame = encode_client_command(&command)
            .map_err(|error| format!("encode remote command: {error}"))?;
        let protocol_codec_roundtrip = decode_client_command(&frame)
            .map(|decoded| decoded == command)
            .unwrap_or(false);
        let bytes = Uint8Array::from(frame.as_slice());
        let transfer = Array::new();
        transfer.push(&bytes.buffer());
        let message = Object::new();
        set_string(&message, "kind", "command")?;
        Reflect::set(&message, &JsValue::from_str("frame"), &bytes)
            .map_err(|error| format!("failed to attach remote command frame: {error:?}"))?;
        self.worker
            .post_message_with_transfer(&message, &transfer)
            .map_err(|error| format!("failed to enqueue remote command: {error:?}"))?;
        {
            let mut depth = self.command_queue_depth.borrow_mut();
            *depth = depth.saturating_add(1);
            self.frame_metrics
                .borrow_mut()
                .observe_pending_frames(*depth);
        }
        self.frame_metrics.borrow_mut().record_request(frame.len());
        Ok(protocol_codec_roundtrip)
    }

    pub fn drain_next_queued_update(&mut self) -> Result<ClientConnectionDrainResult, String> {
        let Some(queued) = self.queued_updates.borrow_mut().pop_front() else {
            return Ok(ClientConnectionDrainResult::default());
        };
        {
            let mut bytes = self.queued_update_bytes.borrow_mut();
            *bytes = bytes.saturating_sub(queued.encoded_len());
        }
        if queued.batch_drained {
            self.acknowledge_batch(queued.batch_sequence)?;
        }
        let remaining_depth = self.queued_updates.borrow().len();
        let remaining_bytes = *self.queued_update_bytes.borrow();
        let transport_drained = remaining_depth == 0 && *self.command_queue_depth.borrow() == 0;
        let queued_age = queued.queued_age();
        let producer_decode_ms = queued.producer_decode_ms;
        let inbound_frame_sequence = queued.batch_sequence;
        let update = match queued.frame {
            Some(frame) => {
                let encoded_len = frame.len();
                let update = decode_server_update(&frame)
                    .map_err(|error| format!("decode worker remote update: {error}"))?;
                if let ServerUpdate::TimeUpdate {
                    game_time,
                    day_time,
                    daylight_cycle_running,
                } = update
                {
                    *self.day_time.borrow_mut() = day_time;
                    QueuedServerUpdate::single(
                        ServerUpdate::TimeUpdate {
                            game_time,
                            day_time,
                            daylight_cycle_running,
                        },
                        encoded_len,
                        queued_age,
                        transport_drained,
                    )
                } else {
                    QueuedServerUpdate::single(update, encoded_len, queued_age, transport_drained)
                }
            }
            None => QueuedServerUpdate::empty(transport_drained),
        }
        .with_remote_metadata(Some(inbound_frame_sequence), 0.0, producer_decode_ms);
        Ok(ClientConnectionDrainResult::with_update(
            update,
            remaining_depth,
            remaining_bytes,
        ))
    }

    pub fn queued_update_metrics(&self) -> ClientConnectionQueueMetrics {
        ClientConnectionQueueMetrics::new(
            self.queued_updates.borrow().len(),
            *self.queued_update_bytes.borrow(),
        )
    }

    pub fn diagnostics(&self) -> ServerRunnerDiagnostics {
        let day_time = *self.day_time.borrow();
        let mut diagnostics =
            ServerRunnerDiagnostics::initial(ServerRunnerKind::RemoteWebSocket, 0, day_time);
        diagnostics.running = *self.running.borrow();
        diagnostics.command_queue_depth = *self.command_queue_depth.borrow();
        diagnostics.update_queue_depth = self.queued_updates.borrow().len();
        diagnostics.update_queue_bytes = *self.queued_update_bytes.borrow();
        diagnostics.runner_frame_metrics = *self.frame_metrics.borrow();
        diagnostics.runner_frame_metrics.observe_pending_frames(
            diagnostics
                .command_queue_depth
                .saturating_add(diagnostics.update_queue_depth),
        );
        diagnostics.last_error = self.last_error.borrow().clone();
        diagnostics
    }

    pub fn request_shutdown(&mut self) {
        if self.shutdown_requested {
            return;
        }
        let _ = self.queue_command(ClientCommand::Disconnect(ClientDisconnectReason::Quit));
        self.shutdown_requested = true;
        let message = Object::new();
        let posted = set_string(&message, "kind", "shutdown").is_ok()
            && self.worker.post_message(&message).is_ok();
        if !posted {
            self.worker.terminate();
        }
        *self.running.borrow_mut() = false;
    }

    fn acknowledge_batch(&self, batch_sequence: u64) -> Result<(), String> {
        let message = Object::new();
        set_string(&message, "kind", "updates-drained")?;
        set_number(&message, "batchSequence", batch_sequence as f64)?;
        self.worker
            .post_message(&message)
            .map_err(|error| format!("failed to acknowledge drained remote batch: {error:?}"))
    }
}

fn queue_transport_disconnect(
    message: &str,
    queued_updates: &Rc<RefCell<VecDeque<QueuedWebSocketUpdate>>>,
    queued_update_bytes: &Rc<RefCell<usize>>,
    frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
) -> Result<(), String> {
    let update = ServerUpdate::Disconnect(DisconnectReason::transport_error(message));
    let frame = encode_server_update(&update)
        .map_err(|error| format!("encode remote transport disconnect: {error}"))?;
    let next_frames = queued_updates.borrow().len().saturating_add(1);
    let next_bytes = queued_update_bytes.borrow().saturating_add(frame.len());
    if next_frames > MAX_MAIN_UPDATE_FRAMES || next_bytes > MAX_MAIN_UPDATE_BYTES {
        return Err("remote main update queue could not retain disconnect reason".to_owned());
    }
    queued_updates
        .borrow_mut()
        .push_back(QueuedWebSocketUpdate {
            frame: Some(frame.clone()),
            queued_at_ms: js_sys::Date::now(),
            batch_sequence: 0,
            batch_drained: false,
            producer_decode_ms: 0.0,
        });
    *queued_update_bytes.borrow_mut() = next_bytes;
    let mut metrics = frame_metrics.borrow_mut();
    metrics.record_inbound(frame.len());
    metrics.observe_pending_frames(next_frames);
    Ok(())
}

impl Drop for WebSocketServerSession {
    fn drop(&mut self) {
        self.request_shutdown();
        self.worker.set_onmessage(None);
        self.worker.set_onerror(None);
        let _ = &self.message_closure;
        let _ = &self.error_closure;
    }
}

struct PendingWorkerReady {
    resolve: Function,
    reject: Function,
}

#[derive(Debug)]
struct QueuedWebSocketUpdate {
    frame: Option<Vec<u8>>,
    queued_at_ms: f64,
    batch_sequence: u64,
    batch_drained: bool,
    producer_decode_ms: f64,
}

impl QueuedWebSocketUpdate {
    fn encoded_len(&self) -> usize {
        self.frame.as_ref().map_or(0, Vec::len)
    }

    fn queued_age(&self) -> Duration {
        let elapsed_ms = (js_sys::Date::now() - self.queued_at_ms).max(0.0);
        if elapsed_ms.is_finite() {
            Duration::from_secs_f64(elapsed_ms / 1_000.0)
        } else {
            Duration::ZERO
        }
    }
}

fn handle_worker_message(
    value: JsValue,
    running: &Rc<RefCell<bool>>,
    command_queue_depth: &Rc<RefCell<usize>>,
    queued_updates: &Rc<RefCell<VecDeque<QueuedWebSocketUpdate>>>,
    queued_update_bytes: &Rc<RefCell<usize>>,
    frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
    pending_ready: &Rc<RefCell<Option<PendingWorkerReady>>>,
) -> Result<(), String> {
    match required_string(&value, "kind")?.as_str() {
        "ready" => {
            *running.borrow_mut() = true;
            if let Some(ready) = pending_ready.borrow_mut().take() {
                let _ = ready.resolve.call0(&JsValue::NULL);
            }
            Ok(())
        }
        "command-sent" => {
            let mut depth = command_queue_depth.borrow_mut();
            *depth = depth.saturating_sub(1);
            frame_metrics.borrow_mut().observe_pending_frames(*depth);
            Ok(())
        }
        "updates" => {
            queue_worker_updates(&value, queued_updates, queued_update_bytes, frame_metrics)
        }
        "error" | "closed" => Err(required_string(&value, "message")?),
        kind => Err(format!(
            "unexpected remote websocket worker message `{kind}`"
        )),
    }
}

fn queue_worker_updates(
    value: &JsValue,
    queued_updates: &Rc<RefCell<VecDeque<QueuedWebSocketUpdate>>>,
    queued_update_bytes: &Rc<RefCell<usize>>,
    frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
) -> Result<(), String> {
    let frames_value = Reflect::get(value, &JsValue::from_str("frames"))
        .map_err(|error| format!("failed to read remote worker frames: {error:?}"))?;
    if !Array::is_array(&frames_value) {
        return Err("remote worker updates did not contain a frame array".to_owned());
    }
    let frames = Array::from(&frames_value);
    let batch_sequence = required_number(value, "batchSequence")? as u64;
    let received_bytes = required_number(value, "receivedBytes")?.max(0.0) as usize;
    let decode_ms = required_number(value, "decodeMs")?.max(0.0);
    let mut decoded_frames = Vec::with_capacity(frames.length() as usize);
    let mut canonical_bytes = 0_usize;
    for frame in frames.iter() {
        if !frame.is_instance_of::<ArrayBuffer>() && !frame.is_instance_of::<Uint8Array>() {
            return Err("remote worker update frame was not binary".to_owned());
        }
        let bytes = if frame.is_instance_of::<ArrayBuffer>() {
            Uint8Array::new(&frame).to_vec()
        } else {
            Uint8Array::new(&frame).to_vec()
        };
        canonical_bytes = canonical_bytes.saturating_add(bytes.len());
        decoded_frames.push(bytes);
    }
    let next_frames = queued_updates
        .borrow()
        .len()
        .saturating_add(decoded_frames.len().max(1));
    let next_bytes = queued_update_bytes.borrow().saturating_add(canonical_bytes);
    if next_frames > MAX_MAIN_UPDATE_FRAMES || next_bytes > MAX_MAIN_UPDATE_BYTES {
        return Err(format!(
            "remote main update queue exceeded bounds: {next_frames} frames, {next_bytes} bytes"
        ));
    }

    let queued_at_ms = js_sys::Date::now();
    let frame_count = decoded_frames.len();
    let mut queue = queued_updates.borrow_mut();
    if frame_count == 0 {
        queue.push_back(QueuedWebSocketUpdate {
            frame: None,
            queued_at_ms,
            batch_sequence,
            batch_drained: true,
            producer_decode_ms: decode_ms,
        });
    } else {
        for (index, frame) in decoded_frames.into_iter().enumerate() {
            queue.push_back(QueuedWebSocketUpdate {
                frame: Some(frame),
                queued_at_ms,
                batch_sequence,
                batch_drained: index + 1 == frame_count,
                producer_decode_ms: if index == 0 { decode_ms } else { 0.0 },
            });
        }
    }
    *queued_update_bytes.borrow_mut() = next_bytes;
    let mut metrics = frame_metrics.borrow_mut();
    metrics.transport_kind = WorkerFrameTransportKind::WebSocket;
    metrics.record_inbound(received_bytes);
    metrics.observe_pending_frames(next_frames);
    Ok(())
}

fn record_worker_error(
    running: &Rc<RefCell<bool>>,
    last_error: &Rc<RefCell<Option<String>>>,
    pending_ready: &Rc<RefCell<Option<PendingWorkerReady>>>,
    error: String,
) {
    *running.borrow_mut() = false;
    *last_error.borrow_mut() = Some(error.clone());
    if let Some(ready) = pending_ready.borrow_mut().take() {
        let _ = ready
            .reject
            .call1(&JsValue::NULL, &JsValue::from_str(&error));
    }
}

fn set_string(object: &Object, name: &str, value: &str) -> Result<(), String> {
    Reflect::set(object, &JsValue::from_str(name), &JsValue::from_str(value))
        .map(|_| ())
        .map_err(|error| format!("failed to set remote worker field {name}: {error:?}"))
}

fn set_number(object: &Object, name: &str, value: f64) -> Result<(), String> {
    Reflect::set(object, &JsValue::from_str(name), &JsValue::from_f64(value))
        .map(|_| ())
        .map_err(|error| format!("failed to set remote worker field {name}: {error:?}"))
}

fn required_string(value: &JsValue, name: &str) -> Result<String, String> {
    Reflect::get(value, &JsValue::from_str(name))
        .map_err(|error| format!("failed to read remote worker field {name}: {error:?}"))?
        .as_string()
        .ok_or_else(|| format!("remote worker field {name} was not a string"))
}

fn required_number(value: &JsValue, name: &str) -> Result<f64, String> {
    Reflect::get(value, &JsValue::from_str(name))
        .map_err(|error| format!("failed to read remote worker field {name}: {error:?}"))?
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("remote worker field {name} was not a finite number"))
}

fn js_error_string(value: &JsValue) -> String {
    value.as_string().unwrap_or_else(|| format!("{value:?}"))
}
