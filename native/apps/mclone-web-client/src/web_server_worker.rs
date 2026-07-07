use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use js_sys::{
    Array, Atomics, Function, Int32Array, Object, Promise, Reflect, SharedArrayBuffer, Uint8Array,
};
use mclone_app_runtime::client_connection::{
    ClientConnectionDrainResult, ClientConnectionQueueMetrics, QueuedServerUpdate,
};
use mclone_app_runtime::host_mode::diagnostics_worker_exchange_drained;
use mclone_core::ChunkPos;
use mclone_protocol::{
    ChunkView, ClientCommand, ServerUpdate, decode_client_command, decode_server_update,
    encode_client_command, encode_server_update,
};
use mclone_server::{
    ChunkRecord, ChunkStoreError, ChunkStoreResult, EntityChunkRecord, INITIAL_DAY_TIME,
    IntegratedServer, IntegratedServerRunner, LightStatusMailboxKind, ServerRunnerDiagnostics,
    ServerRunnerError, ServerRunnerKind, ServerRunnerResult, ServerRunnerTickDiagnostics,
    ServerUpdateEnvelope, WasmServerJobWorkerConfig, WorkerFrameMetrics, WorkerFrameTransportKind,
    WorldStore, WorldStoreCompletion, WorldStoreRequest, WorldgenJobSession, WorldgenMailboxKind,
    compute_light_status_job_frame, decode_chunk_record, decode_entity_chunk_record,
    encode_chunk_record, encode_entity_chunk_record,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

const WEB_WORKER_TICK_INTERVAL_MS: u32 = 50;
// Server-worker SharedArrayBuffer ring ABI. This is the Rust copy of the control-word layout
// authored once on the JS side in www/mclone-runner-shared-abi.js (imported by both the
// integrated-server worker and the worldgen/light job worker). The host test
// tests/runner_shared_abi_lock.rs parses both files and fails on any drift, so a mismatched
// status code or word index is a failing test instead of a silent SAB corruption — keep them
// in sync. CONTROL_BYTES is an integer literal (4 i32 control words × 4 bytes) rather than a
// derived `SLOTS * 4`, because the lock test's parser evaluates integer literals/products but
// not const references.
const RUNNER_SHARED_CONTROL_BYTES: u32 = 16;
const RUNNER_SHARED_STATUS_INDEX: u32 = 0;
const RUNNER_SHARED_REQUEST_BYTES_INDEX: u32 = 1;
const RUNNER_SHARED_RESPONSE_BYTES_INDEX: u32 = 2;
const RUNNER_SHARED_STATUS_PENDING: i32 = 1;
const RUNNER_SHARED_STATUS_COMPLETE: i32 = 2;
const RUNNER_SHARED_STATUS_FAILED: i32 = -1;
const MAX_RUNNER_SHARED_POOL_SLOTS: usize = 2;
const MIN_RUNNER_SHARED_REQUEST_BYTES: u32 = 4 * 1024;
const DEFAULT_RUNNER_SHARED_RESPONSE_BYTES: u32 = 2 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebIntegratedServerRunnerConfig {
    pub seed: i64,
    pub worker_url: String,
    pub job_worker_url: String,
    pub bindgen_js_url: String,
    pub bindgen_wasm_url: String,
    pub world_storage: WebIntegratedServerWorldStorage,
    pub runner_transport_kind: Option<WorkerFrameTransportKind>,
    pub runner_initial_response_bytes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebIntegratedServerWorldStorage {
    Transient,
    IndexedDb {
        world_id: String,
        clear_existing: bool,
    },
}

impl WebIntegratedServerRunnerConfig {
    pub fn new(
        seed: i64,
        worker_url: impl Into<String>,
        job_worker_url: impl Into<String>,
        bindgen_js_url: impl Into<String>,
        bindgen_wasm_url: impl Into<String>,
    ) -> Self {
        Self {
            seed,
            worker_url: worker_url.into(),
            job_worker_url: job_worker_url.into(),
            bindgen_js_url: bindgen_js_url.into(),
            bindgen_wasm_url: bindgen_wasm_url.into(),
            world_storage: WebIntegratedServerWorldStorage::Transient,
            runner_transport_kind: None,
            runner_initial_response_bytes: DEFAULT_RUNNER_SHARED_RESPONSE_BYTES,
        }
    }

    pub fn with_indexed_db_world(
        mut self,
        world_id: impl Into<String>,
        clear_existing: bool,
    ) -> Self {
        self.world_storage = WebIntegratedServerWorldStorage::IndexedDb {
            world_id: world_id.into(),
            clear_existing,
        };
        self
    }

    pub const fn with_runner_transport_kind(
        mut self,
        runner_transport_kind: WorkerFrameTransportKind,
    ) -> Self {
        self.runner_transport_kind = Some(runner_transport_kind);
        self
    }

    pub const fn with_runner_initial_response_bytes(
        mut self,
        runner_initial_response_bytes: u32,
    ) -> Self {
        self.runner_initial_response_bytes = runner_initial_response_bytes;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WebWorkerExchange {
    pub updates: Vec<ServerUpdate>,
    pub protocol_codec_roundtrip: bool,
    pub transport_drained: bool,
}

pub struct WebIntegratedServerRunner {
    worker: Worker,
    transport_kind: WorkerFrameTransportKind,
    next_request_id: u32,
    pending: Rc<RefCell<BTreeMap<u32, PendingRequest>>>,
    request_start_ms_by_id: Rc<RefCell<BTreeMap<u32, f64>>>,
    runner_frame_metrics: Rc<RefCell<WorkerFrameMetrics>>,
    shared_pool: Rc<RefCell<Vec<RunnerSharedSlot>>>,
    shared_inflight: Rc<RefCell<BTreeMap<u32, RunnerSharedSlot>>>,
    runner_initial_response_bytes: u32,
    update_frames: Rc<RefCell<Vec<Vec<u8>>>>,
    diagnostics: Rc<RefCell<ServerRunnerDiagnostics>>,
    message_closure: Closure<dyn FnMut(MessageEvent)>,
    error_closure: Closure<dyn FnMut(ErrorEvent)>,
    shutdown_requested: bool,
}

impl fmt::Debug for WebIntegratedServerRunner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebIntegratedServerRunner")
            .field("next_request_id", &self.next_request_id)
            .field("transport_kind", &self.transport_kind)
            .field("pending_requests", &self.pending.borrow().len())
            .field("update_frames", &self.update_frames.borrow().len())
            .field("diagnostics", &self.diagnostics.borrow())
            .field("shutdown_requested", &self.shutdown_requested)
            .finish_non_exhaustive()
    }
}

struct RunnerSharedSlot {
    control_buffer: SharedArrayBuffer,
    request_buffer: SharedArrayBuffer,
    response_buffer: SharedArrayBuffer,
    request_capacity: u32,
    response_capacity: u32,
}

impl RunnerSharedSlot {
    fn new(request_capacity: u32, response_capacity: u32) -> Self {
        Self {
            control_buffer: SharedArrayBuffer::new(RUNNER_SHARED_CONTROL_BYTES),
            request_buffer: SharedArrayBuffer::new(request_capacity),
            response_buffer: SharedArrayBuffer::new(response_capacity),
            request_capacity,
            response_capacity,
        }
    }

    fn capacity_bytes(&self) -> usize {
        (RUNNER_SHARED_CONTROL_BYTES as usize)
            .saturating_add(self.request_capacity as usize)
            .saturating_add(self.response_capacity as usize)
    }
}

impl WebIntegratedServerRunner {
    pub async fn new(config: WebIntegratedServerRunnerConfig) -> Result<Self, String> {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        options.set_name("mclone-integrated-server");
        let worker = Worker::new_with_options(&config.worker_url, &options)
            .map_err(|error| format!("failed to spawn integrated server worker: {error:?}"))?;

        let shared_transport_available = runner_shared_memory_transport_available();
        let transport_kind = match config.runner_transport_kind {
            Some(WorkerFrameTransportKind::SharedMemory) if shared_transport_available => {
                WorkerFrameTransportKind::SharedMemory
            }
            Some(WorkerFrameTransportKind::MessageTransfer) => {
                WorkerFrameTransportKind::MessageTransfer
            }
            Some(WorkerFrameTransportKind::None) => WorkerFrameTransportKind::MessageTransfer,
            None if shared_transport_available => WorkerFrameTransportKind::SharedMemory,
            _ => WorkerFrameTransportKind::MessageTransfer,
        };
        let runner_initial_response_bytes = config
            .runner_initial_response_bytes
            .max(RUNNER_SHARED_CONTROL_BYTES);
        let pending = Rc::new(RefCell::new(BTreeMap::new()));
        let request_start_ms_by_id: Rc<RefCell<BTreeMap<u32, f64>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let runner_frame_metrics = Rc::new(RefCell::new(match transport_kind {
            WorkerFrameTransportKind::SharedMemory => WorkerFrameMetrics::shared_memory(),
            _ => WorkerFrameMetrics::message_transfer(),
        }));
        let shared_pool = Rc::new(RefCell::new(Vec::new()));
        let shared_inflight = Rc::new(RefCell::new(BTreeMap::new()));
        let update_frames = Rc::new(RefCell::new(Vec::new()));
        let diagnostics = Rc::new(RefCell::new(ServerRunnerDiagnostics::initial(
            ServerRunnerKind::WebWorker,
            config.seed,
            INITIAL_DAY_TIME,
        )));

        let message_closure = {
            let worker = worker.clone();
            let pending = Rc::clone(&pending);
            let request_start_ms_by_id = Rc::clone(&request_start_ms_by_id);
            let runner_frame_metrics = Rc::clone(&runner_frame_metrics);
            let shared_pool = Rc::clone(&shared_pool);
            let shared_inflight = Rc::clone(&shared_inflight);
            let update_frames = Rc::clone(&update_frames);
            let diagnostics = Rc::clone(&diagnostics);
            Closure::wrap(Box::new(move |event: MessageEvent| {
                handle_runner_message(
                    event.data(),
                    &worker,
                    &pending,
                    &request_start_ms_by_id,
                    &runner_frame_metrics,
                    &shared_pool,
                    &shared_inflight,
                    &update_frames,
                    &diagnostics,
                );
            }) as Box<dyn FnMut(_)>)
        };
        worker.set_onmessage(Some(message_closure.as_ref().unchecked_ref()));

        let error_closure = {
            let pending = Rc::clone(&pending);
            let diagnostics = Rc::clone(&diagnostics);
            Closure::wrap(Box::new(move |event: ErrorEvent| {
                let message = if event.message().is_empty() {
                    "integrated server worker failed".to_owned()
                } else {
                    event.message()
                };
                diagnostics.borrow_mut().last_error = Some(message.clone());
                reject_all_pending(&pending, &message);
            }) as Box<dyn FnMut(_)>)
        };
        worker.set_onerror(Some(error_closure.as_ref().unchecked_ref()));

        let mut runner = Self {
            worker,
            transport_kind,
            next_request_id: 1,
            pending,
            request_start_ms_by_id,
            runner_frame_metrics,
            shared_pool,
            shared_inflight,
            runner_initial_response_bytes,
            update_frames,
            diagnostics,
            message_closure,
            error_closure,
            shutdown_requested: false,
        };
        runner.start(config).await?;
        Ok(runner)
    }

    pub const fn kind(&self) -> ServerRunnerKind {
        ServerRunnerKind::WebWorker
    }

    pub fn diagnostics(&self) -> ServerRunnerDiagnostics {
        let mut diagnostics = self.diagnostics.borrow().clone();
        let frames = self.update_frames.borrow();
        diagnostics.update_queue_depth = frames.len();
        diagnostics.update_queue_bytes = queued_frame_bytes(&frames);
        drop(frames);
        diagnostics.runner_frame_metrics = *self.runner_frame_metrics.borrow();
        diagnostics
            .runner_frame_metrics
            .observe_pending_frames(diagnostics.update_queue_depth);
        diagnostics
    }

    pub async fn exchange_command(
        &mut self,
        command: ClientCommand,
    ) -> Result<WebWorkerExchange, String> {
        let protocol_codec_roundtrip = self.send_command_acknowledged(command).await?;
        let updates = self.drain_decoded_updates()?;
        let diagnostics = self.diagnostics();
        Ok(WebWorkerExchange {
            updates,
            protocol_codec_roundtrip,
            transport_drained: diagnostics_worker_exchange_drained(&diagnostics),
        })
    }

    pub fn queue_command(&mut self, command: ClientCommand) -> Result<bool, String> {
        if self.shutdown_requested {
            return Err("integrated server worker is shut down".to_owned());
        }
        let frame =
            encode_client_command(&command).map_err(|error| format!("encode command: {error}"))?;
        let protocol_codec_roundtrip = decode_client_command(&frame)
            .map(|decoded| decoded == command)
            .unwrap_or(false);
        self.post_command_frame_fire_and_forget(frame)?;
        Ok(protocol_codec_roundtrip)
    }

    pub async fn send_command_acknowledged(
        &mut self,
        command: ClientCommand,
    ) -> Result<bool, String> {
        if self.shutdown_requested {
            return Err("integrated server worker is shut down".to_owned());
        }
        let frame =
            encode_client_command(&command).map_err(|error| format!("encode command: {error}"))?;
        let protocol_codec_roundtrip = decode_client_command(&frame)
            .map(|decoded| decoded == command)
            .unwrap_or(false);
        self.post_command_frame(frame).await?;
        Ok(protocol_codec_roundtrip)
    }

    pub fn drain_next_queued_update(&mut self) -> Result<ClientConnectionDrainResult, String> {
        let Some(frame) = self.drain_update_frames_budgeted(1).into_iter().next() else {
            return Ok(ClientConnectionDrainResult::default());
        };
        let encoded_len = frame.len();
        let update = decode_server_update(&frame)
            .map_err(|error| format!("decode worker server update: {error}"))?;
        let diagnostics = self.diagnostics();
        Ok(ClientConnectionDrainResult::with_update(
            QueuedServerUpdate::single(
                update,
                encoded_len,
                std::time::Duration::ZERO,
                diagnostics_worker_exchange_drained(&diagnostics),
            ),
            diagnostics.update_queue_depth,
            diagnostics.update_queue_bytes,
        ))
    }

    pub fn queued_update_metrics(&self) -> ClientConnectionQueueMetrics {
        let diagnostics = self.diagnostics();
        ClientConnectionQueueMetrics::new(
            diagnostics.update_queue_depth,
            diagnostics.update_queue_bytes,
        )
    }

    pub fn drain_decoded_updates(&mut self) -> Result<Vec<ServerUpdate>, String> {
        let frames = self.drain_update_frames();
        self.decode_update_frames(frames)
    }

    pub fn drain_decoded_updates_budgeted(
        &mut self,
        max_frames: usize,
    ) -> Result<Vec<ServerUpdate>, String> {
        let frames = self.drain_update_frames_budgeted(max_frames);
        self.decode_update_frames(frames)
    }

    fn decode_update_frames(&self, frames: Vec<Vec<u8>>) -> Result<Vec<ServerUpdate>, String> {
        let mut updates = Vec::with_capacity(frames.len());
        for frame in frames {
            updates.push(
                decode_server_update(&frame)
                    .map_err(|error| format!("decode worker server update: {error}"))?,
            );
        }
        Ok(updates)
    }

    pub fn request_shutdown(&mut self) {
        if self.shutdown_requested {
            return;
        }
        self.shutdown_requested = true;
        let _ = self.post_shutdown_with_request_id(0);
        self.worker.terminate();
        let mut diagnostics = self.diagnostics.borrow_mut();
        diagnostics.running = false;
        diagnostics.awaiting_tick = false;
    }

    pub async fn shutdown_gracefully(&mut self) -> Result<(), String> {
        if self.shutdown_requested {
            return Ok(());
        }
        self.shutdown_requested = true;
        let request_id = self.next_request_id();
        let promise = self.register_pending(request_id);
        if let Err(error) = self.post_shutdown_with_request_id(request_id) {
            self.pending.borrow_mut().remove(&request_id);
            self.worker.terminate();
            return Err(error);
        }
        let response = JsFuture::from(promise).await.map_err(|error| {
            format!("server worker shutdown failed: {}", js_error_string(&error))
        })?;
        ensure_worker_response_ok(&response)?;
        self.worker.terminate();
        let mut diagnostics = self.diagnostics.borrow_mut();
        diagnostics.running = false;
        diagnostics.awaiting_tick = false;
        Ok(())
    }

    fn next_request_id(&mut self) -> u32 {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        request_id
    }

    async fn start(&mut self, config: WebIntegratedServerRunnerConfig) -> Result<(), String> {
        let request_id = self.next_request_id();
        let message = Object::new();
        set_string(&message, "kind", "start")?;
        set_number(&message, "requestId", f64::from(request_id))?;
        set_number(&message, "seed", config.seed as f64)?;
        set_string(&message, "bindgenJsUrl", &config.bindgen_js_url)?;
        set_string(&message, "bindgenWasmUrl", &config.bindgen_wasm_url)?;
        set_string(&message, "jobWorkerUrl", &config.job_worker_url)?;
        match &config.world_storage {
            WebIntegratedServerWorldStorage::Transient => {
                set_string(&message, "worldStorage", "transient")?;
            }
            WebIntegratedServerWorldStorage::IndexedDb {
                world_id,
                clear_existing,
            } => {
                set_string(&message, "worldStorage", "indexeddb")?;
                set_string(&message, "worldId", world_id)?;
                set_bool(&message, "clearWorldStorage", *clear_existing)?;
            }
        }
        set_string(&message, "runnerTransportKind", self.transport_kind.label())?;
        set_number(
            &message,
            "tickIntervalMs",
            f64::from(WEB_WORKER_TICK_INTERVAL_MS),
        )?;
        let response = self.post_request(request_id, &message, None).await?;
        ensure_worker_response_ok(&response)
    }

    async fn post_command_frame(&mut self, frame: Vec<u8>) -> Result<(), String> {
        match self.transport_kind {
            WorkerFrameTransportKind::SharedMemory => self.post_shared_command_frame(frame).await,
            _ => self.post_transferred_command_frame(frame).await,
        }
    }

    async fn post_transferred_command_frame(&mut self, frame: Vec<u8>) -> Result<(), String> {
        let request_id = self.next_request_id();
        let bytes = Uint8Array::from(frame.as_slice());
        let transfer = Array::new();
        transfer.push(&bytes.buffer());
        let message = Object::new();
        set_string(&message, "kind", "command")?;
        set_number(&message, "requestId", f64::from(request_id))?;
        Reflect::set(&message, &JsValue::from_str("frame"), &bytes)
            .map_err(|error| format!("failed to attach command frame: {error:?}"))?;
        let promise = self.register_pending(request_id);
        if let Err(error) = self.worker.post_message_with_transfer(&message, &transfer) {
            self.pending.borrow_mut().remove(&request_id);
            return Err(format!("failed to post to server worker: {error:?}"));
        }
        self.record_runner_request(request_id, frame.len());
        let response = JsFuture::from(promise).await.map_err(|error| {
            format!("server worker request failed: {}", js_error_string(&error))
        })?;
        ensure_worker_response_ok(&response)
    }

    async fn post_shared_command_frame(&mut self, frame: Vec<u8>) -> Result<(), String> {
        let request_id = self.next_request_id();
        let message = self.shared_command_message(request_id, &frame)?;
        let promise = self.register_pending(request_id);
        if let Err(error) = self.worker.post_message(&message) {
            self.pending.borrow_mut().remove(&request_id);
            self.release_shared_inflight(request_id);
            return Err(format!("failed to post to server worker: {error:?}"));
        }
        self.record_runner_request(request_id, frame.len());
        let response = JsFuture::from(promise).await.map_err(|error| {
            format!("server worker request failed: {}", js_error_string(&error))
        })?;
        ensure_worker_response_ok(&response)
    }

    fn post_command_frame_fire_and_forget(&mut self, frame: Vec<u8>) -> Result<(), String> {
        match self.transport_kind {
            WorkerFrameTransportKind::SharedMemory => {
                self.post_shared_command_frame_fire_and_forget(frame)
            }
            _ => self.post_transferred_command_frame_fire_and_forget(frame),
        }
    }

    fn post_transferred_command_frame_fire_and_forget(
        &mut self,
        frame: Vec<u8>,
    ) -> Result<(), String> {
        let request_id = self.next_request_id();
        let bytes = Uint8Array::from(frame.as_slice());
        let transfer = Array::new();
        transfer.push(&bytes.buffer());
        let message = Object::new();
        set_string(&message, "kind", "command")?;
        set_number(&message, "requestId", f64::from(request_id))?;
        Reflect::set(&message, &JsValue::from_str("frame"), &bytes)
            .map_err(|error| format!("failed to attach command frame: {error:?}"))?;
        self.worker
            .post_message_with_transfer(&message, &transfer)
            .map_err(|error| format!("failed to post command to server worker: {error:?}"))?;
        self.record_runner_request(request_id, frame.len());
        Ok(())
    }

    fn post_shared_command_frame_fire_and_forget(&mut self, frame: Vec<u8>) -> Result<(), String> {
        let request_id = self.next_request_id();
        let message = self.shared_command_message(request_id, &frame)?;
        if let Err(error) = self.worker.post_message(&message) {
            self.release_shared_inflight(request_id);
            return Err(format!(
                "failed to post command to server worker: {error:?}"
            ));
        }
        self.record_runner_request(request_id, frame.len());
        Ok(())
    }

    fn post_shutdown_with_request_id(&self, request_id: u32) -> Result<(), String> {
        let message = Object::new();
        set_string(&message, "kind", "shutdown")?;
        set_number(&message, "requestId", f64::from(request_id))?;
        self.worker
            .post_message(&message)
            .map_err(|error| format!("failed to post shutdown to server worker: {error:?}"))
    }

    fn shared_command_message(&mut self, request_id: u32, frame: &[u8]) -> Result<Object, String> {
        let request_bytes = u32::try_from(frame.len()).map_err(|_| {
            format!(
                "runner command frame is too large for shared transport: {} bytes",
                frame.len()
            )
        })?;
        let request_bytes_i32 = i32::try_from(request_bytes).map_err(|_| {
            format!(
                "runner command frame is too large for shared byte counters: {} bytes",
                frame.len()
            )
        })?;
        let slot = self.acquire_runner_shared_slot(request_bytes);
        let build_result = (|| {
            let control = Int32Array::new(slot.control_buffer.as_ref());
            Atomics::store(
                &control,
                RUNNER_SHARED_STATUS_INDEX,
                RUNNER_SHARED_STATUS_PENDING,
            )
            .map_err(|error| {
                format!(
                    "failed to initialize shared runner status: {}",
                    js_error_string(&error)
                )
            })?;
            Atomics::store(
                &control,
                RUNNER_SHARED_REQUEST_BYTES_INDEX,
                request_bytes_i32,
            )
            .map_err(|error| {
                format!(
                    "failed to initialize shared runner request byte count: {}",
                    js_error_string(&error)
                )
            })?;
            Atomics::store(&control, RUNNER_SHARED_RESPONSE_BYTES_INDEX, 0).map_err(|error| {
                format!(
                    "failed to clear shared runner response byte count: {}",
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
            set_string(&message, "kind", "command")?;
            set_string(
                &message,
                "transportKind",
                WorkerFrameTransportKind::SharedMemory.label(),
            )?;
            set_number(&message, "requestId", f64::from(request_id))?;
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
            .map_err(|error| format!("failed to attach shared runner control buffer: {error:?}"))?;
            Reflect::set(
                &message,
                &JsValue::from_str("requestBuffer"),
                slot.request_buffer.as_ref(),
            )
            .map_err(|error| format!("failed to attach shared runner request buffer: {error:?}"))?;
            Reflect::set(
                &message,
                &JsValue::from_str("responseBuffer"),
                slot.response_buffer.as_ref(),
            )
            .map_err(|error| {
                format!("failed to attach shared runner response buffer: {error:?}")
            })?;
            Ok(message)
        })();
        match build_result {
            Ok(message) => {
                self.shared_inflight.borrow_mut().insert(request_id, slot);
                Ok(message)
            }
            Err(error) => {
                release_runner_shared_slot(slot, &self.shared_pool, &self.runner_frame_metrics);
                Err(error)
            }
        }
    }

    fn acquire_runner_shared_slot(&mut self, request_bytes: u32) -> RunnerSharedSlot {
        let mut slot = if let Some(slot) = self.shared_pool.borrow_mut().pop() {
            self.runner_frame_metrics
                .borrow_mut()
                .record_shared_buffer_pool_hit();
            slot
        } else {
            self.runner_frame_metrics
                .borrow_mut()
                .record_shared_buffer_pool_miss();
            let slot = RunnerSharedSlot::new(
                runner_shared_capacity_for_len(request_bytes, MIN_RUNNER_SHARED_REQUEST_BYTES),
                runner_shared_capacity_for_len(
                    self.runner_initial_response_bytes,
                    RUNNER_SHARED_CONTROL_BYTES,
                ),
            );
            self.runner_frame_metrics
                .borrow_mut()
                .record_shared_buffer_capacity_delta(slot.capacity_bytes());
            slot
        };
        ensure_runner_slot_request_capacity(&mut slot, request_bytes, &self.runner_frame_metrics);
        slot
    }

    fn release_shared_inflight(&mut self, request_id: u32) {
        if let Some(slot) = self.shared_inflight.borrow_mut().remove(&request_id) {
            release_runner_shared_slot(slot, &self.shared_pool, &self.runner_frame_metrics);
        }
    }

    fn record_runner_request(&self, request_id: u32, bytes: usize) {
        self.request_start_ms_by_id
            .borrow_mut()
            .insert(request_id, js_sys::Date::now());
        let pending_frames = self.request_start_ms_by_id.borrow().len();
        let mut metrics = self.runner_frame_metrics.borrow_mut();
        metrics.record_request(bytes);
        metrics.observe_pending_frames(pending_frames);
    }

    async fn post_request(
        &mut self,
        request_id: u32,
        message: &Object,
        transfer: Option<&Array>,
    ) -> Result<JsValue, String> {
        let promise = self.register_pending(request_id);
        let post_result = if let Some(transfer) = transfer {
            self.worker.post_message_with_transfer(message, transfer)
        } else {
            self.worker.post_message(message)
        };
        if let Err(error) = post_result {
            self.pending.borrow_mut().remove(&request_id);
            return Err(format!("failed to post to server worker: {error:?}"));
        }
        JsFuture::from(promise)
            .await
            .map_err(|error| format!("server worker request failed: {}", js_error_string(&error)))
    }

    fn register_pending(&self, request_id: u32) -> Promise {
        let pending = Rc::clone(&self.pending);
        Promise::new(&mut move |resolve: Function, reject: Function| {
            pending
                .borrow_mut()
                .insert(request_id, PendingRequest { resolve, reject });
        })
    }

    fn drain_update_frames(&mut self) -> Vec<Vec<u8>> {
        self.drain_update_frames_budgeted(usize::MAX)
    }

    fn drain_update_frames_budgeted(&mut self, max_frames: usize) -> Vec<Vec<u8>> {
        let mut frames = self.update_frames.borrow_mut();
        let drained = if max_frames >= frames.len() {
            std::mem::take(&mut *frames)
        } else {
            frames.drain(0..max_frames).collect()
        };
        let mut diagnostics = self.diagnostics.borrow_mut();
        diagnostics.update_queue_depth = frames.len();
        diagnostics.update_queue_bytes = queued_frame_bytes(&frames);
        drained
    }
}

impl Drop for WebIntegratedServerRunner {
    fn drop(&mut self) {
        self.request_shutdown();
        let _ = &self.message_closure;
        let _ = &self.error_closure;
    }
}

fn queued_frame_bytes(frames: &[Vec<u8>]) -> usize {
    frames.iter().map(Vec::len).sum()
}

impl IntegratedServerRunner for WebIntegratedServerRunner {
    fn kind(&self) -> ServerRunnerKind {
        ServerRunnerKind::WebWorker
    }

    fn send_command(&mut self, command: ClientCommand) -> ServerRunnerResult<()> {
        if self.shutdown_requested {
            return Err(ServerRunnerError::CommandChannelClosed);
        }
        let frame = encode_client_command(&command)?;
        self.post_command_frame_fire_and_forget(frame)
            .map_err(ServerRunnerError::ThreadStart)
    }

    fn try_recv_update(&mut self) -> ServerRunnerResult<Option<ServerUpdateEnvelope>> {
        let Some(frame) = self.drain_update_frames_budgeted(1).into_iter().next() else {
            return Ok(None);
        };
        let encoded_len = frame.len();
        Ok(Some(ServerUpdateEnvelope {
            update: decode_server_update(&frame)?,
            encoded_len,
            queued_age: std::time::Duration::ZERO,
        }))
    }

    fn drain_updates(&mut self) -> ServerRunnerResult<Vec<ServerUpdate>> {
        let mut updates = Vec::new();
        for frame in self.drain_update_frames() {
            updates.push(decode_server_update(&frame)?);
        }
        Ok(updates)
    }

    fn poll_diagnostics(&self) -> ServerRunnerResult<ServerRunnerDiagnostics> {
        Ok(self.diagnostics())
    }

    fn request_shutdown(&mut self) {
        Self::request_shutdown(self);
    }

    fn join_shutdown(&mut self) -> ServerRunnerResult<()> {
        self.request_shutdown();
        Ok(())
    }
}

/// 069 Stage 1: the server-job worker's resident worldgen session. The worker
/// instantiates one of these on its first worldgen job and reuses it across jobs
/// (exactly as the render worker holds a `WebRenderCompilerSession` resident), so
/// the `OverworldFeatureDependencyCache` persists and each job applies only its
/// request delta to it before generating. This replaces the former stateless
/// `mclone_web_compute_worldgen_job_frame` free function, which rebuilt the cache
/// every call and so re-decoded the whole 529-chunk dependency neighbourhood
/// shipped in by the scheduler. Desktop keeps the stateless per-job cache (it
/// moves the dependency `Vec` over `mpsc` for free).
#[wasm_bindgen]
pub struct WebWorldgenJobSession {
    session: WorldgenJobSession,
}

impl Default for WebWorldgenJobSession {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WebWorldgenJobSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            session: WorldgenJobSession::new(),
        }
    }

    /// Apply a worldgen request delta to the resident dependency mirror and
    /// generate the job from it, returning the standard worldgen response frame
    /// (Stage 1 keeps the full response). A generation mismatch is rejected loudly
    /// (the 067 Stage 4 desync tripwire) rather than generated against a partial
    /// mirror.
    #[wasm_bindgen(js_name = computeWorldgenJobFrame)]
    pub fn compute_worldgen_job_frame(&mut self, frame: Uint8Array) -> Result<Uint8Array, JsValue> {
        let response = self
            .session
            .compute_delta_job_frame(&frame.to_vec())
            .map_err(JsValue::from)?;
        Ok(Uint8Array::from(response.as_slice()))
    }

    /// Dependency columns currently resident in the worker mirror (bounded to the
    /// last job's plan by the cache's own retain step). Observability for the
    /// resident-mirror behaviour; the perf fence rides on the SAB request bytes.
    #[wasm_bindgen(js_name = mirrorChunkCount)]
    pub fn mirror_chunk_count(&self) -> usize {
        self.session.mirror_chunk_count()
    }

    /// Dependency columns shipped as upserts on the most recent delta.
    #[wasm_bindgen(js_name = lastDeltaUpsertCount)]
    pub fn last_delta_upsert_count(&self) -> usize {
        self.session.last_delta_upsert_count()
    }
}

#[wasm_bindgen]
pub fn mclone_web_compute_light_status_job_frame(frame: Uint8Array) -> Result<Uint8Array, JsValue> {
    let response = compute_light_status_job_frame(&frame.to_vec()).map_err(JsValue::from)?;
    Ok(Uint8Array::from(response.as_slice()))
}

#[wasm_bindgen]
pub fn mclone_web_shared_topology_stress_report(
    server_worker_url: String,
    server_job_worker_url: String,
    bindgen_js_url: String,
    bindgen_wasm_url: String,
) -> Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        let report = shared_topology_stress_report(
            server_worker_url,
            server_job_worker_url,
            bindgen_js_url,
            bindgen_wasm_url,
        )
        .await
        .map_err(JsValue::from)?;
        Ok(report)
    })
}

async fn shared_topology_stress_report(
    server_worker_url: String,
    server_job_worker_url: String,
    bindgen_js_url: String,
    bindgen_wasm_url: String,
) -> Result<JsValue, String> {
    let shared_runner = run_shared_runner_stress(
        WebIntegratedServerRunnerConfig::new(
            424242,
            server_worker_url.clone(),
            server_job_worker_url.clone(),
            bindgen_js_url.clone(),
            bindgen_wasm_url.clone(),
        )
        .with_runner_transport_kind(WorkerFrameTransportKind::SharedMemory)
        .with_runner_initial_response_bytes(4 * 1024),
    )
    .await?;
    let fallback_runner = run_message_transfer_runner_probe(
        WebIntegratedServerRunnerConfig::new(
            424243,
            server_worker_url,
            server_job_worker_url,
            bindgen_js_url,
            bindgen_wasm_url,
        )
        .with_runner_transport_kind(WorkerFrameTransportKind::MessageTransfer),
    )
    .await?;

    let object = Object::new();
    set_bool(&object, "ok", shared_runner.ok && fallback_runner.ok)?;
    Reflect::set(
        &object,
        &JsValue::from_str("sharedRunner"),
        &shared_runner.to_js_value()?,
    )
    .map_err(|error| format!("failed to attach shared runner stress report: {error:?}"))?;
    Reflect::set(
        &object,
        &JsValue::from_str("fallbackRunner"),
        &fallback_runner.to_js_value()?,
    )
    .map_err(|error| format!("failed to attach fallback runner stress report: {error:?}"))?;
    Ok(object.into())
}

struct RunnerStressReport {
    ok: bool,
    command_count: usize,
    update_count: usize,
    poll_count: usize,
    diagnostics: ServerRunnerDiagnostics,
    shutdown: ServerRunnerDiagnostics,
}

impl RunnerStressReport {
    fn to_js_value(&self) -> Result<JsValue, String> {
        let object = Object::new();
        set_bool(&object, "ok", self.ok)?;
        set_number(&object, "commandCount", self.command_count as f64)?;
        set_number(&object, "updateCount", self.update_count as f64)?;
        set_number(&object, "pollCount", self.poll_count as f64)?;
        Reflect::set(
            &object,
            &JsValue::from_str("diagnostics"),
            &diagnostics_to_js(&self.diagnostics)?,
        )
        .map_err(|error| format!("failed to attach runner diagnostics: {error:?}"))?;
        Reflect::set(
            &object,
            &JsValue::from_str("shutdown"),
            &diagnostics_to_js(&self.shutdown)?,
        )
        .map_err(|error| format!("failed to attach runner shutdown diagnostics: {error:?}"))?;
        Reflect::set(
            &object,
            &JsValue::from_str("runnerFrameMetrics"),
            &frame_metrics_to_js(self.diagnostics.runner_frame_metrics)?,
        )
        .map_err(|error| format!("failed to attach runner frame metrics: {error:?}"))?;
        Reflect::set(
            &object,
            &JsValue::from_str("worldgenJobFrameMetrics"),
            &frame_metrics_to_js(self.diagnostics.worldgen_job_frame_metrics)?,
        )
        .map_err(|error| format!("failed to attach worldgen job frame metrics: {error:?}"))?;
        Reflect::set(
            &object,
            &JsValue::from_str("lightStatusJobFrameMetrics"),
            &frame_metrics_to_js(self.diagnostics.light_status_job_frame_metrics)?,
        )
        .map_err(|error| format!("failed to attach light-status job frame metrics: {error:?}"))?;
        Ok(object.into())
    }
}

async fn run_shared_runner_stress(
    config: WebIntegratedServerRunnerConfig,
) -> Result<RunnerStressReport, String> {
    const FIRE_AND_FORGET_COMMANDS: i32 = 4;
    const REUSE_COMMANDS: i32 = 2;

    let mut runner = WebIntegratedServerRunner::new(config).await?;
    let mut command_count = 0usize;
    let mut update_count = 0usize;
    let mut poll_count = 0usize;

    for x in 0..FIRE_AND_FORGET_COMMANDS {
        runner
            .send_command(chunk_view_command(x, 0))
            .map_err(|error| error.to_string())?;
        command_count += 1;
    }
    wait_for_runner_condition(
        &mut runner,
        &mut update_count,
        &mut poll_count,
        |diagnostics| {
            let metrics = diagnostics.runner_frame_metrics;
            metrics.request_frames >= FIRE_AND_FORGET_COMMANDS as usize
                && metrics.shared_buffer_pool_misses >= FIRE_AND_FORGET_COMMANDS as usize
                && metrics.shared_buffer_pool_drops > 0
                && metrics.shared_buffer_fallback_response_frames > 0
                && runner_diagnostics_settled(diagnostics)
        },
        "shared runner pool overflow and fallback",
    )
    .await?;

    for x in FIRE_AND_FORGET_COMMANDS..(FIRE_AND_FORGET_COMMANDS + REUSE_COMMANDS) {
        let exchange = runner.exchange_command(chunk_view_command(x, 0)).await?;
        command_count += 1;
        update_count = update_count.saturating_add(exchange.updates.len());
        update_count = update_count.saturating_add(runner.drain_decoded_updates()?.len());
    }
    let diagnostics = wait_for_runner_condition(
        &mut runner,
        &mut update_count,
        &mut poll_count,
        |diagnostics| {
            let metrics = diagnostics.runner_frame_metrics;
            metrics.request_frames >= (FIRE_AND_FORGET_COMMANDS + REUSE_COMMANDS) as usize
                && metrics.shared_buffer_pool_hits >= REUSE_COMMANDS as usize
                && metrics.shared_buffer_pooled_response_frames > 0
                && frame_metrics_shared_worker_active(metrics)
                && frame_metrics_shared_worker_active(diagnostics.worldgen_job_frame_metrics)
                && frame_metrics_shared_worker_active(diagnostics.light_status_job_frame_metrics)
                && runner_diagnostics_settled(diagnostics)
        },
        "shared runner reuse after overflow",
    )
    .await?;
    let ok = diagnostics.runner_frame_metrics.transport_kind
        == WorkerFrameTransportKind::SharedMemory
        && diagnostics.runner_frame_metrics.shared_buffer_pool_misses
            >= FIRE_AND_FORGET_COMMANDS as usize
        && diagnostics.runner_frame_metrics.shared_buffer_pool_drops > 0
        && diagnostics.runner_frame_metrics.shared_buffer_pool_hits >= REUSE_COMMANDS as usize
        && diagnostics
            .runner_frame_metrics
            .shared_buffer_fallback_response_frames
            > 0
        && diagnostics
            .runner_frame_metrics
            .shared_buffer_pooled_response_frames
            > 0
        && update_count > 0
        && runner_diagnostics_settled(&diagnostics);
    runner.request_shutdown();
    let shutdown = runner.diagnostics();
    Ok(RunnerStressReport {
        ok,
        command_count,
        update_count,
        poll_count,
        diagnostics,
        shutdown,
    })
}

async fn run_message_transfer_runner_probe(
    config: WebIntegratedServerRunnerConfig,
) -> Result<RunnerStressReport, String> {
    let mut runner = WebIntegratedServerRunner::new(config).await?;
    let exchange = runner.exchange_command(chunk_view_command(0, 0)).await?;
    let mut update_count = exchange.updates.len();
    let mut poll_count = 0usize;
    let diagnostics = wait_for_runner_condition(
        &mut runner,
        &mut update_count,
        &mut poll_count,
        |diagnostics| {
            let metrics = diagnostics.runner_frame_metrics;
            frame_metrics_transfer_worker_active(metrics) && runner_diagnostics_settled(diagnostics)
        },
        "message-transfer runner fallback",
    )
    .await?;
    let ok = diagnostics.runner_frame_metrics.transport_kind
        == WorkerFrameTransportKind::MessageTransfer
        && diagnostics.runner_frame_metrics.request_frames > 0
        && diagnostics.runner_frame_metrics.response_frames > 0
        && diagnostics.runner_frame_metrics.shared_buffer_pool_hits == 0
        && diagnostics.runner_frame_metrics.shared_buffer_pool_misses == 0
        && diagnostics.runner_frame_metrics.shared_buffer_pool_drops == 0
        && update_count > 0
        && runner_diagnostics_settled(&diagnostics);
    runner.request_shutdown();
    let shutdown = runner.diagnostics();
    Ok(RunnerStressReport {
        ok,
        command_count: 1,
        update_count,
        poll_count,
        diagnostics,
        shutdown,
    })
}

async fn wait_for_runner_condition(
    runner: &mut WebIntegratedServerRunner,
    update_count: &mut usize,
    poll_count: &mut usize,
    condition: impl Fn(&ServerRunnerDiagnostics) -> bool,
    label: &str,
) -> Result<ServerRunnerDiagnostics, String> {
    for _ in 0..2400 {
        *update_count = update_count.saturating_add(runner.drain_decoded_updates()?.len());
        let diagnostics = runner.diagnostics();
        if condition(&diagnostics) {
            return Ok(diagnostics);
        }
        *poll_count = poll_count.saturating_add(1);
        wait_for_browser_turn().await?;
    }
    Err(format!("timed out waiting for {label}"))
}

async fn wait_for_browser_turn() -> Result<(), String> {
    let promise = Promise::new(&mut |resolve: Function, reject: Function| {
        let Some(window) = web_sys::window() else {
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str("window is unavailable"));
            return;
        };
        let callback = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::NULL);
        });
        if let Err(error) = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(), 0)
        {
            let _ = reject.call1(&JsValue::NULL, &error);
        }
    });
    JsFuture::from(promise)
        .await
        .map(|_| ())
        .map_err(|error| js_error_string(&error))
}

fn chunk_view_command(x: i32, z: i32) -> ClientCommand {
    ClientCommand::SetChunkView(ChunkView {
        center: ChunkPos { x, z },
        render_distance: 0,
        chunk_tracking_radius: 0,
    })
}

fn runner_diagnostics_settled(diagnostics: &ServerRunnerDiagnostics) -> bool {
    diagnostics.command_queue_depth == 0
        && diagnostics.update_queue_depth == 0
        && diagnostics.pending_jobs == 0
        && diagnostics.pending_publications == 0
        && diagnostics.pending_persistence_loads == 0
        && diagnostics.pending_persistence_saves == 0
        && diagnostics.worldgen_mailbox_pending_jobs == 0
        && diagnostics.light_status_mailbox_pending_statuses == 0
}

fn frame_metrics_shared_worker_active(metrics: WorkerFrameMetrics) -> bool {
    metrics.transport_kind == WorkerFrameTransportKind::SharedMemory
        && metrics.request_frames > 0
        && metrics.request_bytes > 0
        && metrics.response_frames > 0
        && metrics.response_bytes > 0
}

fn frame_metrics_transfer_worker_active(metrics: WorkerFrameMetrics) -> bool {
    metrics.transport_kind == WorkerFrameTransportKind::MessageTransfer
        && metrics.request_frames > 0
        && metrics.request_bytes > 0
        && metrics.response_frames > 0
        && metrics.response_bytes > 0
}

#[derive(Clone)]
struct PendingRequest {
    resolve: Function,
    reject: Function,
}

fn handle_runner_message(
    data: JsValue,
    worker: &Worker,
    pending: &Rc<RefCell<BTreeMap<u32, PendingRequest>>>,
    request_start_ms_by_id: &Rc<RefCell<BTreeMap<u32, f64>>>,
    runner_frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
    shared_pool: &Rc<RefCell<Vec<RunnerSharedSlot>>>,
    shared_inflight: &Rc<RefCell<BTreeMap<u32, RunnerSharedSlot>>>,
    update_frames: &Rc<RefCell<Vec<Vec<u8>>>>,
    diagnostics: &Rc<RefCell<ServerRunnerDiagnostics>>,
) {
    let request_id = number_prop(&data, "requestId")
        .map(|value| value as u32)
        .unwrap_or(0);
    if let Some(diagnostic_value) = reflect_get(&data, "diagnostics") {
        let fallback = diagnostics.borrow().clone();
        if let Some(parsed) = parse_diagnostics(&diagnostic_value, &fallback) {
            *diagnostics.borrow_mut() = parsed;
        }
    }

    if bool_prop(&data, "ok") == Some(false) {
        if request_id != 0 {
            if let Some(slot) = shared_inflight.borrow_mut().remove(&request_id) {
                release_runner_shared_slot(slot, shared_pool, runner_frame_metrics);
            }
            request_start_ms_by_id.borrow_mut().remove(&request_id);
        }
        diagnostics.borrow_mut().runner_frame_metrics = *runner_frame_metrics.borrow();
        reject_pending_failure(&data, pending, request_id);
        return;
    }

    let shutdown_complete = string_prop(&data, "kind").as_deref() == Some("shutdown-complete");

    let frames_result = if string_prop(&data, "transportKind").as_deref() == Some("shared-memory") {
        let mut slot = if request_id != 0 {
            shared_inflight.borrow_mut().remove(&request_id)
        } else {
            None
        };
        match shared_runner_response_frames(&data) {
            Ok(response) => {
                if let Some(mut slot) = slot.take() {
                    record_runner_shared_response_metrics(runner_frame_metrics, &response);
                    if !response.pooled_response {
                        grow_runner_slot_response_buffer(
                            &mut slot,
                            response.packed_bytes,
                            runner_frame_metrics,
                        );
                    }
                    release_runner_shared_slot(slot, shared_pool, runner_frame_metrics);
                } else if response.pooled_response {
                    runner_frame_metrics
                        .borrow_mut()
                        .record_shared_buffer_pooled_response();
                } else {
                    runner_frame_metrics
                        .borrow_mut()
                        .record_shared_buffer_fallback_response();
                }
                if let Some(buffer_id) = number_prop(&data, "runnerSharedBufferId") {
                    let _ = post_runner_shared_buffer_release(worker, buffer_id as u32);
                }
                Ok(response.frames)
            }
            Err(error) => {
                if let Some(slot) = slot {
                    release_runner_shared_slot(slot, shared_pool, runner_frame_metrics);
                }
                Err(error)
            }
        }
    } else if let Some(updates_value) = reflect_get(&data, "updates") {
        Ok(frames_from_js(&updates_value))
    } else {
        Ok(Vec::new())
    };

    match frames_result {
        Ok(frames) => {
            if !frames.is_empty() {
                let mut metrics = runner_frame_metrics.borrow_mut();
                for frame in &frames {
                    metrics.record_response(frame.len());
                }
                drop(metrics);
                let mut queued = update_frames.borrow_mut();
                queued.extend(frames);
                let mut diagnostics = diagnostics.borrow_mut();
                diagnostics.update_queue_depth = queued.len();
                diagnostics.update_queue_bytes = queued_frame_bytes(&queued);
            }
        }
        Err(error) => {
            diagnostics.borrow_mut().last_error = Some(error.clone());
            if request_id != 0 {
                if let Some(pending_request) = pending.borrow_mut().remove(&request_id) {
                    let _ = pending_request
                        .reject
                        .call1(&JsValue::NULL, &JsValue::from_str(&error));
                }
            }
            diagnostics.borrow_mut().runner_frame_metrics = *runner_frame_metrics.borrow();
            return;
        }
    }

    if request_id != 0 {
        if let Some(start_ms) = request_start_ms_by_id.borrow_mut().remove(&request_id) {
            let request_us = ((js_sys::Date::now() - start_ms).max(0.0_f64) * 1000.0) as u128;
            runner_frame_metrics
                .borrow_mut()
                .record_request_time_us(request_us);
        }
    }
    diagnostics.borrow_mut().runner_frame_metrics = *runner_frame_metrics.borrow();

    if shutdown_complete {
        worker.terminate();
    }

    if request_id == 0 {
        return;
    }
    let Some(pending_request) = pending.borrow_mut().remove(&request_id) else {
        return;
    };
    let _ = pending_request.resolve.call1(&JsValue::NULL, &data);
}

fn reject_pending_failure(
    data: &JsValue,
    pending: &Rc<RefCell<BTreeMap<u32, PendingRequest>>>,
    request_id: u32,
) {
    if request_id == 0 {
        return;
    }
    let Some(pending_request) = pending.borrow_mut().remove(&request_id) else {
        return;
    };
    let reason = string_prop(data, "reason")
        .unwrap_or_else(|| "integrated server worker returned an unknown failure".to_owned());
    let _ = pending_request
        .reject
        .call1(&JsValue::NULL, &JsValue::from_str(&reason));
}

fn reject_all_pending(pending: &Rc<RefCell<BTreeMap<u32, PendingRequest>>>, message: &str) {
    for (_, pending_request) in std::mem::take(&mut *pending.borrow_mut()) {
        let _ = pending_request
            .reject
            .call1(&JsValue::NULL, &JsValue::from_str(message));
    }
}

fn parse_diagnostics(
    value: &JsValue,
    fallback: &ServerRunnerDiagnostics,
) -> Option<ServerRunnerDiagnostics> {
    let mut diagnostics = fallback.clone();
    diagnostics.kind = ServerRunnerKind::WebWorker;
    diagnostics.running = bool_prop(value, "running")?;
    diagnostics.seed = number_prop(value, "seed")? as i64;
    diagnostics.day_time = number_prop(value, "dayTime")? as u64;
    diagnostics.awaiting_tick = bool_prop(value, "awaitingTick").unwrap_or(false);
    diagnostics.command_queue_depth =
        number_prop(value, "commandQueueDepth").unwrap_or(0.0) as usize;
    diagnostics.update_queue_depth = number_prop(value, "updateQueueDepth").unwrap_or(0.0) as usize;
    diagnostics.pending_jobs = number_prop(value, "pendingJobs").unwrap_or(0.0) as usize;
    diagnostics.pending_publications =
        number_prop(value, "pendingPublications").unwrap_or(0.0) as usize;
    diagnostics.pending_persistence_loads =
        number_prop(value, "pendingPersistenceLoads").unwrap_or(0.0) as usize;
    diagnostics.pending_persistence_saves =
        number_prop(value, "pendingPersistenceSaves").unwrap_or(0.0) as usize;
    diagnostics.worldgen_mailbox_kind = worldgen_mailbox_kind_prop(
        value,
        "worldgenMailboxKind",
        diagnostics.worldgen_mailbox_kind,
    );
    diagnostics.light_status_mailbox_kind = light_status_mailbox_kind_prop(
        value,
        "lightStatusMailboxKind",
        diagnostics.light_status_mailbox_kind,
    );
    diagnostics.worldgen_mailbox_pending_jobs =
        number_prop(value, "worldgenMailboxPendingJobs").unwrap_or(0.0) as usize;
    diagnostics.light_status_mailbox_pending_statuses =
        number_prop(value, "lightStatusMailboxPendingStatuses").unwrap_or(0.0) as usize;
    diagnostics.runner_frame_metrics = frame_metrics_prop(
        value,
        "runnerFrameMetrics",
        diagnostics.runner_frame_metrics,
    );
    diagnostics.worldgen_job_frame_metrics = frame_metrics_prop(
        value,
        "worldgenJobFrameMetrics",
        diagnostics.worldgen_job_frame_metrics,
    );
    diagnostics.light_status_job_frame_metrics = frame_metrics_prop(
        value,
        "lightStatusJobFrameMetrics",
        diagnostics.light_status_job_frame_metrics,
    );
    diagnostics.last_tick.simulation_tick =
        number_prop(value, "lastSimulationTick").unwrap_or(0.0) as u64;
    diagnostics.last_tick.chunk_tick = number_prop(value, "lastChunkTick").unwrap_or(0.0) as u64;
    diagnostics.last_tick.wall_us = number_prop(value, "lastTickWallUs").unwrap_or(0.0) as u128;
    diagnostics.last_error = string_prop(value, "lastError").filter(|message| !message.is_empty());
    Some(diagnostics)
}

fn frames_from_js(value: &JsValue) -> Vec<Vec<u8>> {
    if !Array::is_array(value) {
        return Vec::new();
    }
    Array::from(value)
        .iter()
        .filter(|value| value.is_instance_of::<Uint8Array>())
        .map(|value| Uint8Array::new(&value).to_vec())
        .collect()
}

struct SharedRunnerResponse {
    frames: Vec<Vec<u8>>,
    packed_bytes: usize,
    pooled_response: bool,
}

fn shared_runner_response_frames(value: &JsValue) -> Result<SharedRunnerResponse, String> {
    let Some(control_buffer) = reflect_get(value, "controlBuffer") else {
        return Err("shared runner response returned no control buffer".to_owned());
    };
    let control = Int32Array::new(&control_buffer);
    let status = Atomics::load(&control, RUNNER_SHARED_STATUS_INDEX).map_err(|error| {
        format!(
            "failed to read shared runner status: {}",
            js_error_string(&error)
        )
    })?;
    if status == RUNNER_SHARED_STATUS_FAILED {
        return Err("shared runner worker reported a failure status".to_owned());
    }
    if status != RUNNER_SHARED_STATUS_COMPLETE {
        return Err(format!(
            "shared runner response completed with unexpected status {status}"
        ));
    }
    let response_bytes =
        Atomics::load(&control, RUNNER_SHARED_RESPONSE_BYTES_INDEX).map_err(|error| {
            format!(
                "failed to read shared runner response byte count: {}",
                js_error_string(&error)
            )
        })?;
    if response_bytes < 0 {
        return Err(format!(
            "shared runner response returned negative byte count {response_bytes}"
        ));
    }
    let Some(update_buffer) = reflect_get(value, "updateBuffer") else {
        return Err("shared runner response returned no update buffer".to_owned());
    };
    if !update_buffer.is_instance_of::<SharedArrayBuffer>() {
        return Err("shared runner update buffer was not a SharedArrayBuffer".to_owned());
    }
    let response_bytes = response_bytes as u32;
    let packed = Uint8Array::new_with_byte_offset_and_length(&update_buffer, 0, response_bytes);
    Ok(SharedRunnerResponse {
        frames: unpack_runner_update_frames(&packed.to_vec())?,
        packed_bytes: response_bytes as usize,
        pooled_response: bool_prop(value, "pooledResponse").unwrap_or(false),
    })
}

/// Packed runner update frame codec — encode side. Single-sourced in Rust so the
/// `SharedArrayBuffer` packing format has one owner; the integrated-server worker
/// calls this via wasm-bindgen instead of hand-rolling a little-endian writer
/// (the former `writeU32Le` / `writePackedUpdates` in
/// `mclone-integrated-server-worker.js`). The decode side is
/// `unpack_runner_update_frames` just below. Format: a u32 LE frame count, then
/// per frame a u32 LE byte length followed by the frame bytes.
#[wasm_bindgen(js_name = mcloneWebPackedRunnerUpdateByteLength)]
pub fn mclone_web_packed_runner_update_byte_length(updates: &Array) -> u32 {
    let mut byte_length: u32 = 4; // leading u32 frame count
    for value in updates.iter() {
        byte_length = byte_length
            .saturating_add(4)
            .saturating_add(runner_update_frame_len(&value));
    }
    byte_length
}

/// Pack `updates` (an array of `Uint8Array` server-update frames) into `buffer` at
/// offset 0, writing exactly `packed_bytes` bytes (as returned by
/// `mclone_web_packed_runner_update_byte_length`). The caller arms the SAB doorbell
/// (response byte count + status word) after this returns.
#[wasm_bindgen(js_name = mcloneWebWritePackedRunnerUpdates)]
pub fn mclone_web_write_packed_runner_updates(
    updates: &Array,
    buffer: &SharedArrayBuffer,
    packed_bytes: u32,
) -> Result<(), JsValue> {
    let capacity = buffer.byte_length();
    if packed_bytes > capacity {
        return Err(JsValue::from_str(&format!(
            "packed shared runner updates need {packed_bytes} bytes but buffer has {capacity}"
        )));
    }
    let view = Uint8Array::new_with_byte_offset_and_length(buffer.as_ref(), 0, packed_bytes);
    let mut offset = write_packed_runner_u32(&view, 0, updates.length());
    for value in updates.iter() {
        let frame_len = runner_update_frame_len(&value);
        offset = write_packed_runner_u32(&view, offset, frame_len);
        if frame_len > 0 {
            // `value` is the Uint8Array frame itself; `TypedArray.set` copies it in one shot.
            view.set(&value, offset);
        }
        offset = offset.saturating_add(frame_len);
    }
    Ok(())
}

fn runner_update_frame_len(value: &JsValue) -> u32 {
    value.dyn_ref::<Uint8Array>().map_or(0, Uint8Array::length)
}

fn write_packed_runner_u32(view: &Uint8Array, offset: u32, value: u32) -> u32 {
    let bytes = value.to_le_bytes();
    view.set_index(offset, bytes[0]);
    view.set_index(offset + 1, bytes[1]);
    view.set_index(offset + 2, bytes[2]);
    view.set_index(offset + 3, bytes[3]);
    offset + 4
}

fn unpack_runner_update_frames(packed: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut cursor = 0usize;
    let frame_count = read_le_u32(packed, &mut cursor)? as usize;
    let mut frames = Vec::with_capacity(frame_count);
    for _ in 0..frame_count {
        let frame_len = read_le_u32(packed, &mut cursor)? as usize;
        let end = cursor
            .checked_add(frame_len)
            .ok_or_else(|| "packed runner update frame length overflowed".to_owned())?;
        if end > packed.len() {
            return Err(format!(
                "packed runner update frame length {frame_len} exceeds remaining {} bytes",
                packed.len().saturating_sub(cursor)
            ));
        }
        frames.push(packed[cursor..end].to_vec());
        cursor = end;
    }
    if cursor != packed.len() {
        return Err(format!(
            "packed runner updates had {} trailing bytes",
            packed.len() - cursor
        ));
    }
    Ok(frames)
}

fn read_le_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, String> {
    let end = cursor
        .checked_add(4)
        .ok_or_else(|| "packed runner update cursor overflowed".to_owned())?;
    let Some(word) = bytes.get(*cursor..end) else {
        return Err("packed runner updates ended before u32 field".to_owned());
    };
    *cursor = end;
    Ok(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
}

fn post_runner_shared_buffer_release(worker: &Worker, buffer_id: u32) -> Result<(), String> {
    let message = Object::new();
    set_string(&message, "kind", "release-shared-buffer")?;
    set_number(&message, "requestId", 0.0)?;
    set_number(&message, "runnerSharedBufferId", f64::from(buffer_id))?;
    worker
        .post_message(&message)
        .map_err(|error| format!("failed to release shared runner buffer: {error:?}"))
}

fn ensure_worker_response_ok(value: &JsValue) -> Result<(), String> {
    if bool_prop(value, "ok") == Some(false) {
        return Err(string_prop(value, "reason")
            .unwrap_or_else(|| "integrated server worker request failed".to_owned()));
    }
    Ok(())
}

#[wasm_bindgen]
pub struct McloneWebIntegratedServerWorker {
    server: IntegratedServer,
    diagnostics: ServerRunnerDiagnostics,
    indexed_db_state: Option<Rc<RefCell<WebIndexedDbWorldStoreState>>>,
    command_queue_depth: usize,
    running: bool,
}

#[derive(Debug, Default)]
struct WebIndexedDbWorldStoreState {
    chunks: BTreeMap<ChunkPos, ChunkRecord>,
    entity_chunks: BTreeMap<ChunkPos, EntityChunkRecord>,
    dirty_chunks: BTreeMap<ChunkPos, ChunkRecord>,
    dirty_entity_chunks: BTreeMap<ChunkPos, EntityChunkRecord>,
}

impl WebIndexedDbWorldStoreState {
    fn from_js_records(
        chunk_records: JsValue,
        entity_chunk_records: JsValue,
    ) -> Result<Rc<RefCell<Self>>, String> {
        let mut state = Self::default();
        for record in decode_chunk_records_from_js(&chunk_records)? {
            state.chunks.insert(record.pos(), record);
        }
        for record in decode_entity_chunk_records_from_js(&entity_chunk_records)? {
            state.entity_chunks.insert(record.pos, record);
        }
        Ok(Rc::new(RefCell::new(state)))
    }
}

#[derive(Clone, Debug)]
struct WebIndexedDbWorldStore {
    state: Rc<RefCell<WebIndexedDbWorldStoreState>>,
}

impl WebIndexedDbWorldStore {
    fn new(state: Rc<RefCell<WebIndexedDbWorldStoreState>>) -> Self {
        Self { state }
    }
}

impl WorldStore for WebIndexedDbWorldStore {
    fn supports_entity_chunks(&self) -> bool {
        true
    }

    fn load_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<ChunkRecord>> {
        Ok(self.state.borrow().chunks.get(&pos).cloned())
    }

    fn save_chunk(&mut self, record: &ChunkRecord) -> ChunkStoreResult<()> {
        let mut state = self.state.borrow_mut();
        state.chunks.insert(record.pos(), record.clone());
        state.dirty_chunks.insert(record.pos(), record.clone());
        Ok(())
    }

    fn load_entity_chunk(&mut self, pos: ChunkPos) -> ChunkStoreResult<Option<EntityChunkRecord>> {
        Ok(self.state.borrow().entity_chunks.get(&pos).cloned())
    }

    fn save_entity_chunk(&mut self, record: &EntityChunkRecord) -> ChunkStoreResult<()> {
        let mut state = self.state.borrow_mut();
        state.entity_chunks.insert(record.pos, record.clone());
        state.dirty_entity_chunks.insert(record.pos, record.clone());
        Ok(())
    }
}

#[wasm_bindgen]
impl McloneWebIntegratedServerWorker {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: i64) -> Self {
        let server = IntegratedServer::local_integrated(seed);
        Self::from_server(seed, server, None)
    }

    #[wasm_bindgen(js_name = withJobWorkers)]
    pub fn with_job_workers(
        seed: i64,
        job_worker_url: String,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
    ) -> Self {
        let server = IntegratedServer::local_integrated_with_wasm_job_workers(
            seed,
            WasmServerJobWorkerConfig::new(job_worker_url, bindgen_js_url, bindgen_wasm_url),
        );
        Self::from_server(seed, server, None)
    }

    #[wasm_bindgen(js_name = withIndexedDbRecords)]
    pub fn with_indexed_db_records(
        seed: i64,
        chunk_records: JsValue,
        entity_chunk_records: JsValue,
    ) -> Result<Self, JsValue> {
        let state =
            WebIndexedDbWorldStoreState::from_js_records(chunk_records, entity_chunk_records)
                .map_err(|error| JsValue::from_str(&error))?;
        let store = Box::new(WebIndexedDbWorldStore::new(Rc::clone(&state)));
        let server = IntegratedServer::local_integrated_with_world_store(seed, store);
        Ok(Self::from_server(seed, server, Some(state)))
    }

    #[wasm_bindgen(js_name = withIndexedDbExternalLoads)]
    pub fn with_indexed_db_external_loads(seed: i64) -> Self {
        let state = Rc::new(RefCell::new(WebIndexedDbWorldStoreState::default()));
        let store = Box::new(WebIndexedDbWorldStore::new(Rc::clone(&state)));
        let server = IntegratedServer::local_integrated_with_external_load_world_store(seed, store);
        Self::from_server(seed, server, Some(state))
    }

    #[wasm_bindgen(js_name = withJobWorkersAndIndexedDbRecords)]
    pub fn with_job_workers_and_indexed_db_records(
        seed: i64,
        job_worker_url: String,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
        chunk_records: JsValue,
        entity_chunk_records: JsValue,
    ) -> Result<Self, JsValue> {
        let state =
            WebIndexedDbWorldStoreState::from_js_records(chunk_records, entity_chunk_records)
                .map_err(|error| JsValue::from_str(&error))?;
        let store = Box::new(WebIndexedDbWorldStore::new(Rc::clone(&state)));
        let server = IntegratedServer::local_integrated_with_world_store_and_wasm_job_workers(
            seed,
            store,
            WasmServerJobWorkerConfig::new(job_worker_url, bindgen_js_url, bindgen_wasm_url),
        );
        Ok(Self::from_server(seed, server, Some(state)))
    }

    #[wasm_bindgen(js_name = withJobWorkersAndIndexedDbExternalLoads)]
    pub fn with_job_workers_and_indexed_db_external_loads(
        seed: i64,
        job_worker_url: String,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
    ) -> Self {
        let state = Rc::new(RefCell::new(WebIndexedDbWorldStoreState::default()));
        let store = Box::new(WebIndexedDbWorldStore::new(Rc::clone(&state)));
        let server =
            IntegratedServer::local_integrated_with_external_load_world_store_and_wasm_job_workers(
                seed,
                store,
                WasmServerJobWorkerConfig::new(job_worker_url, bindgen_js_url, bindgen_wasm_url),
            );
        Self::from_server(seed, server, Some(state))
    }

    #[wasm_bindgen(js_name = completeIndexedDbLoadRecords)]
    pub fn complete_indexed_db_load_records(
        &mut self,
        completions: JsValue,
    ) -> Result<JsValue, JsValue> {
        let completions = decode_indexed_db_load_completions_from_js(&completions)
            .map_err(|error| JsValue::from_str(&error))?;
        for completion in completions {
            self.server
                .scheduler_mut()
                .complete_external_persistence_request(completion)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
        }
        match self.server.try_poll() {
            Ok(updates) => {
                self.refresh_diagnostics(None, false, None);
                self.worker_response(updates).map_err(JsValue::from)
            }
            Err(error) => {
                let error = error.to_string();
                self.refresh_diagnostics(None, false, Some(error.clone()));
                Err(JsValue::from_str(&error))
            }
        }
    }

    #[wasm_bindgen(js_name = handleCommandFrame)]
    pub fn handle_command_frame(&mut self, frame: Uint8Array) -> Result<JsValue, JsValue> {
        let request_start = js_sys::Date::now();
        self.diagnostics
            .runner_frame_metrics
            .record_request(frame.byte_length() as usize);
        self.command_queue_depth = self.command_queue_depth.saturating_add(1);
        self.diagnostics
            .runner_frame_metrics
            .observe_pending_frames(self.command_queue_depth);
        self.refresh_diagnostics(None, true, None);
        let result = decode_client_command(&frame.to_vec())
            .map_err(|error| error.to_string())
            .and_then(|command| {
                let mut updates = self
                    .server
                    .try_handle_command(command)
                    .map_err(|error| error.to_string())?;
                updates.extend(self.server.try_poll().map_err(|error| error.to_string())?);
                self.autosave_indexed_db_dirty_chunks(&mut updates)?;
                Ok(updates)
            });
        self.command_queue_depth = self.command_queue_depth.saturating_sub(1);
        match result {
            Ok(updates) => {
                self.refresh_diagnostics(None, false, None);
                self.diagnostics
                    .runner_frame_metrics
                    .record_request_time_us(elapsed_us_since(request_start));
                self.worker_response(updates).map_err(JsValue::from)
            }
            Err(error) => {
                self.diagnostics
                    .runner_frame_metrics
                    .record_request_time_us(elapsed_us_since(request_start));
                self.refresh_diagnostics(None, false, Some(error.clone()));
                Err(JsValue::from_str(&error))
            }
        }
    }

    #[wasm_bindgen(js_name = poll)]
    pub fn poll(&mut self) -> Result<JsValue, JsValue> {
        match self.server.try_poll() {
            Ok(updates) => {
                self.refresh_diagnostics(None, false, None);
                self.worker_response(updates).map_err(JsValue::from)
            }
            Err(error) => {
                let error = error.to_string();
                self.refresh_diagnostics(None, false, Some(error.clone()));
                Err(JsValue::from_str(&error))
            }
        }
    }

    #[wasm_bindgen(js_name = tick)]
    pub fn tick(&mut self) -> Result<JsValue, JsValue> {
        if !self.running {
            return self.worker_response(Vec::new()).map_err(JsValue::from);
        }
        let wall_start = js_sys::Date::now();
        match self.server.try_simulation_tick_report() {
            Ok(report) => {
                let wall_us = ((js_sys::Date::now() - wall_start).max(0.0) * 1000.0) as u128;
                let mut updates = report.updates.clone();
                if let Err(error) = self.autosave_indexed_db_dirty_chunks(&mut updates) {
                    self.refresh_diagnostics(None, false, Some(error.clone()));
                    return Err(JsValue::from_str(&error));
                }
                self.refresh_diagnostics(
                    Some(ServerRunnerTickDiagnostics::from_report(&report, wall_us)),
                    false,
                    None,
                );
                self.worker_response(updates).map_err(JsValue::from)
            }
            Err(error) => {
                let error = error.to_string();
                self.refresh_diagnostics(None, false, Some(error.clone()));
                Err(JsValue::from_str(&error))
            }
        }
    }

    #[wasm_bindgen(js_name = diagnostics)]
    pub fn diagnostics(&self) -> Result<JsValue, JsValue> {
        diagnostics_to_js(&self.diagnostics).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = shutdown)]
    pub fn shutdown(&mut self) -> Result<JsValue, JsValue> {
        self.running = false;
        if let Err(error) = self.server.shutdown_persistence() {
            let error = error.to_string();
            self.refresh_diagnostics(None, false, Some(error.clone()));
            return Err(JsValue::from_str(&error));
        }
        self.refresh_diagnostics(None, false, None);
        self.worker_response(Vec::new()).map_err(JsValue::from)
    }
}

impl McloneWebIntegratedServerWorker {
    fn from_server(
        seed: i64,
        server: IntegratedServer,
        indexed_db_state: Option<Rc<RefCell<WebIndexedDbWorldStoreState>>>,
    ) -> Self {
        let mut diagnostics =
            ServerRunnerDiagnostics::initial(ServerRunnerKind::WebWorker, seed, server.day_time());
        diagnostics.running = true;
        diagnostics.runner_frame_metrics = WorkerFrameMetrics::message_transfer();
        let mut worker = Self {
            server,
            diagnostics,
            indexed_db_state,
            command_queue_depth: 0,
            running: true,
        };
        worker.refresh_diagnostics(None, false, None);
        worker
    }

    fn worker_response(&mut self, updates: Vec<ServerUpdate>) -> Result<JsValue, String> {
        let response = worker_response(updates, &mut self.diagnostics)?;
        if let Some(state) = &self.indexed_db_state {
            attach_indexed_db_dirty_records(&response, &mut state.borrow_mut())?;
            attach_indexed_db_load_requests(
                &response,
                self.server
                    .scheduler_mut()
                    .drain_external_persistence_requests(),
            )?;
        }
        Ok(response)
    }

    fn autosave_indexed_db_dirty_chunks(
        &mut self,
        updates: &mut Vec<ServerUpdate>,
    ) -> Result<(), String> {
        if self.indexed_db_state.is_none() {
            return Ok(());
        }
        self.server
            .save_dirty_chunks()
            .map_err(|error| error.to_string())?;
        for _ in 0..256 {
            if self.server.scheduler().pending_persistence_save_count() == 0 {
                return Ok(());
            }
            updates.extend(self.server.try_poll().map_err(|error| error.to_string())?);
        }
        Err("timed out waiting for IndexedDB autosave persistence writes".to_owned())
    }

    fn refresh_diagnostics(
        &mut self,
        tick: Option<ServerRunnerTickDiagnostics>,
        awaiting_tick: bool,
        last_error: Option<String>,
    ) {
        self.diagnostics.running = self.running;
        self.diagnostics.day_time = self.server.day_time();
        self.diagnostics.command_queue_depth = self.command_queue_depth;
        self.diagnostics.update_queue_depth = 0;
        self.diagnostics.pending_jobs = self.server.pending_job_count();
        self.diagnostics.pending_publications = self.server.pending_publication_count();
        self.diagnostics.pending_persistence_loads =
            self.server.scheduler().pending_persistence_load_count();
        self.diagnostics.pending_persistence_saves =
            self.server.scheduler().pending_persistence_save_count();
        self.diagnostics.worldgen_mailbox_kind = self.server.scheduler().worldgen_mailbox_kind();
        self.diagnostics.light_status_mailbox_kind =
            self.server.scheduler().light_status_mailbox_kind();
        self.diagnostics.worldgen_mailbox_pending_jobs =
            self.server.scheduler().worldgen_mailbox_pending_count();
        self.diagnostics.light_status_mailbox_pending_statuses =
            self.server.scheduler().light_status_mailbox_pending_count();
        self.diagnostics.worldgen_job_frame_metrics =
            self.server.scheduler().worldgen_mailbox_frame_metrics();
        self.diagnostics.light_status_job_frame_metrics =
            self.server.scheduler().light_status_mailbox_frame_metrics();
        self.diagnostics.scheduler_metrics = self.server.scheduler().metrics();
        self.diagnostics.chunk_tracking = self.server.chunk_tracking_diagnostics();
        self.diagnostics.loading_progress = self.server.loading_progress_stats();
        self.diagnostics.loading_progress_snapshot = self.server.loading_progress_snapshot();
        self.diagnostics.view_readiness_snapshot = self.server.view_readiness_snapshot();
        self.diagnostics.awaiting_tick = awaiting_tick;
        if let Some(tick) = tick {
            self.diagnostics.last_tick = tick;
        }
        if let Some(last_error) = last_error {
            self.diagnostics.last_error = Some(last_error);
        }
    }
}

fn worker_response(
    updates: Vec<ServerUpdate>,
    diagnostics: &mut ServerRunnerDiagnostics,
) -> Result<JsValue, String> {
    let object = Object::new();
    let packed_updates = Array::new();
    for update in updates {
        let frame = encode_server_update(&update).map_err(|error| error.to_string())?;
        diagnostics
            .runner_frame_metrics
            .record_response(frame.len());
        packed_updates.push(&Uint8Array::from(frame.as_slice()));
    }
    set_bool(&object, "ok", true)?;
    set_number(&object, "updateCount", packed_updates.length() as f64)?;
    Reflect::set(&object, &JsValue::from_str("updates"), &packed_updates)
        .map_err(|error| format!("failed to attach worker updates: {error:?}"))?;
    Reflect::set(
        &object,
        &JsValue::from_str("diagnostics"),
        &diagnostics_to_js(diagnostics)?,
    )
    .map_err(|error| format!("failed to attach worker diagnostics: {error:?}"))?;
    Ok(object.into())
}

fn decode_chunk_records_from_js(value: &JsValue) -> Result<Vec<ChunkRecord>, String> {
    if value.is_null() || value.is_undefined() {
        return Ok(Vec::new());
    }
    Array::from(value)
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let bytes = js_record_bytes(&entry)
                .map_err(|error| format!("indexedDB chunk record {index}: {error}"))?;
            decode_chunk_record(&bytes)
                .map_err(|error| format!("decode indexedDB chunk record {index}: {error}"))
        })
        .collect()
}

fn decode_entity_chunk_records_from_js(value: &JsValue) -> Result<Vec<EntityChunkRecord>, String> {
    if value.is_null() || value.is_undefined() {
        return Ok(Vec::new());
    }
    Array::from(value)
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let bytes = js_record_bytes(&entry)
                .map_err(|error| format!("indexedDB entity chunk record {index}: {error}"))?;
            decode_entity_chunk_record(&bytes)
                .map_err(|error| format!("decode indexedDB entity chunk record {index}: {error}"))
        })
        .collect()
}

fn js_record_bytes(value: &JsValue) -> Result<Vec<u8>, String> {
    if let Some(bytes) = value.dyn_ref::<Uint8Array>() {
        return Ok(bytes.to_vec());
    }
    let record = Reflect::get(value, &JsValue::from_str("record"))
        .map_err(|error| format!("failed to read record bytes: {error:?}"))?;
    record
        .dyn_ref::<Uint8Array>()
        .map(Uint8Array::to_vec)
        .ok_or_else(|| "record bytes must be a Uint8Array".to_owned())
}

fn attach_indexed_db_dirty_records(
    response: &JsValue,
    state: &mut WebIndexedDbWorldStoreState,
) -> Result<(), String> {
    let chunks = chunk_records_to_js(&state.dirty_chunks)?;
    let entity_chunks = entity_chunk_records_to_js(&state.dirty_entity_chunks)?;
    Reflect::set(response, &JsValue::from_str("indexedDbChunks"), &chunks)
        .map_err(|error| format!("failed to attach indexedDB chunks: {error:?}"))?;
    Reflect::set(
        response,
        &JsValue::from_str("indexedDbEntityChunks"),
        &entity_chunks,
    )
    .map_err(|error| format!("failed to attach indexedDB entity chunks: {error:?}"))?;
    state.dirty_chunks.clear();
    state.dirty_entity_chunks.clear();
    Ok(())
}

fn attach_indexed_db_load_requests(
    response: &JsValue,
    requests: Vec<WorldStoreRequest>,
) -> Result<(), String> {
    let requests = indexed_db_load_requests_to_js(requests)?;
    Reflect::set(
        response,
        &JsValue::from_str("indexedDbLoadRequests"),
        &requests,
    )
    .map_err(|error| format!("failed to attach indexedDB load requests: {error:?}"))?;
    Ok(())
}

fn indexed_db_load_requests_to_js(requests: Vec<WorldStoreRequest>) -> Result<Array, String> {
    let array = Array::new();
    for request in requests {
        let object = Object::new();
        match request {
            WorldStoreRequest::LoadChunk { request_id, pos } => {
                set_string(&object, "kind", "chunk")?;
                set_number(&object, "requestId", request_id as f64)?;
                set_number(&object, "x", f64::from(pos.x))?;
                set_number(&object, "z", f64::from(pos.z))?;
            }
            WorldStoreRequest::LoadEntityChunk { request_id, pos } => {
                set_string(&object, "kind", "entityChunk")?;
                set_number(&object, "requestId", request_id as f64)?;
                set_number(&object, "x", f64::from(pos.x))?;
                set_number(&object, "z", f64::from(pos.z))?;
            }
            other => {
                return Err(format!(
                    "IndexedDB external persistence emitted unsupported request {other:?}"
                ));
            }
        }
        array.push(&object);
    }
    Ok(array)
}

fn decode_indexed_db_load_completions_from_js(
    value: &JsValue,
) -> Result<Vec<WorldStoreCompletion>, String> {
    let array = value
        .dyn_ref::<Array>()
        .ok_or_else(|| "indexedDB load completions must be an array".to_owned())?;
    let mut completions = Vec::with_capacity(array.length() as usize);
    for index in 0..array.length() {
        let value = array.get(index);
        let kind = string_prop(&value, "kind")
            .ok_or_else(|| format!("indexedDB load completion {index} is missing kind"))?;
        let request_id = number_prop(&value, "requestId")
            .ok_or_else(|| format!("indexedDB load completion {index} is missing requestId"))?
            as u64;
        let pos = ChunkPos::new(
            number_prop(&value, "x")
                .ok_or_else(|| format!("indexedDB load completion {index} is missing x"))?
                as i32,
            number_prop(&value, "z")
                .ok_or_else(|| format!("indexedDB load completion {index} is missing z"))?
                as i32,
        );
        match kind.as_str() {
            "chunk" => {
                let result = decode_indexed_db_chunk_load_result(&value, pos).map_err(|error| {
                    ChunkStoreError::InvalidData(format!(
                        "indexedDB chunk load completion {index}: {error}"
                    ))
                });
                completions.push(WorldStoreCompletion::ChunkLoaded {
                    request_id,
                    pos,
                    result,
                });
            }
            "entityChunk" => {
                let result =
                    decode_indexed_db_entity_chunk_load_result(&value, pos).map_err(|error| {
                        ChunkStoreError::InvalidData(format!(
                            "indexedDB entity chunk load completion {index}: {error}"
                        ))
                    });
                completions.push(WorldStoreCompletion::EntityChunkLoaded {
                    request_id,
                    pos,
                    result,
                });
            }
            _ => {
                return Err(format!(
                    "indexedDB load completion {index} has unsupported kind {kind}"
                ));
            }
        }
    }
    Ok(completions)
}

fn decode_indexed_db_chunk_load_result(
    value: &JsValue,
    pos: ChunkPos,
) -> ChunkStoreResult<Option<ChunkRecord>> {
    if let Some(error) = string_prop(value, "error")
        && !error.is_empty()
    {
        return Err(ChunkStoreError::InvalidData(error));
    }
    if bool_prop(value, "found") == Some(false) {
        return Ok(None);
    }
    let bytes = js_record_bytes(value).map_err(ChunkStoreError::InvalidData)?;
    let record = decode_chunk_record(&bytes)?;
    if record.pos() != pos {
        return Err(ChunkStoreError::InvalidData(format!(
            "record position ({}, {}) does not match requested ({}, {})",
            record.pos().x,
            record.pos().z,
            pos.x,
            pos.z
        )));
    }
    Ok(Some(record))
}

fn decode_indexed_db_entity_chunk_load_result(
    value: &JsValue,
    pos: ChunkPos,
) -> ChunkStoreResult<Option<EntityChunkRecord>> {
    if let Some(error) = string_prop(value, "error")
        && !error.is_empty()
    {
        return Err(ChunkStoreError::InvalidData(error));
    }
    if bool_prop(value, "found") == Some(false) {
        return Ok(None);
    }
    let bytes = js_record_bytes(value).map_err(ChunkStoreError::InvalidData)?;
    let record = decode_entity_chunk_record(&bytes)?;
    if record.pos != pos {
        return Err(ChunkStoreError::InvalidData(format!(
            "record position ({}, {}) does not match requested ({}, {})",
            record.pos.x, record.pos.z, pos.x, pos.z
        )));
    }
    Ok(Some(record))
}

fn chunk_records_to_js(records: &BTreeMap<ChunkPos, ChunkRecord>) -> Result<Array, String> {
    let array = Array::new();
    for (pos, record) in records {
        let bytes = encode_chunk_record(record).map_err(|error| {
            format!(
                "encode indexedDB chunk record ({}, {}): {error}",
                pos.x, pos.z
            )
        })?;
        let object: JsValue = indexed_db_record_to_js(*pos, bytes)?.into();
        array.push(&object);
    }
    Ok(array)
}

fn entity_chunk_records_to_js(
    records: &BTreeMap<ChunkPos, EntityChunkRecord>,
) -> Result<Array, String> {
    let array = Array::new();
    for (pos, record) in records {
        let bytes = encode_entity_chunk_record(record).map_err(|error| {
            format!(
                "encode indexedDB entity chunk record ({}, {}): {error}",
                pos.x, pos.z
            )
        })?;
        let object: JsValue = indexed_db_record_to_js(*pos, bytes)?.into();
        array.push(&object);
    }
    Ok(array)
}

fn indexed_db_record_to_js(pos: ChunkPos, bytes: Vec<u8>) -> Result<Object, String> {
    let object = Object::new();
    set_number(&object, "x", f64::from(pos.x))?;
    set_number(&object, "z", f64::from(pos.z))?;
    let bytes = Uint8Array::from(bytes.as_slice());
    Reflect::set(&object, &JsValue::from_str("record"), &bytes)
        .map_err(|error| format!("failed to attach indexedDB record bytes: {error:?}"))?;
    Ok(object)
}

fn diagnostics_to_js(diagnostics: &ServerRunnerDiagnostics) -> Result<JsValue, String> {
    let object = Object::new();
    set_string(&object, "kind", diagnostics.kind.label())?;
    set_bool(&object, "running", diagnostics.running)?;
    set_number(&object, "seed", diagnostics.seed as f64)?;
    set_number(&object, "dayTime", diagnostics.day_time as f64)?;
    set_bool(&object, "awaitingTick", diagnostics.awaiting_tick)?;
    set_number(
        &object,
        "commandQueueDepth",
        diagnostics.command_queue_depth as f64,
    )?;
    set_number(
        &object,
        "updateQueueDepth",
        diagnostics.update_queue_depth as f64,
    )?;
    set_number(&object, "pendingJobs", diagnostics.pending_jobs as f64)?;
    set_number(
        &object,
        "pendingPublications",
        diagnostics.pending_publications as f64,
    )?;
    set_number(
        &object,
        "pendingPersistenceLoads",
        diagnostics.pending_persistence_loads as f64,
    )?;
    set_number(
        &object,
        "pendingPersistenceSaves",
        diagnostics.pending_persistence_saves as f64,
    )?;
    set_string(
        &object,
        "worldgenMailboxKind",
        diagnostics.worldgen_mailbox_kind.label(),
    )?;
    set_string(
        &object,
        "lightStatusMailboxKind",
        diagnostics.light_status_mailbox_kind.label(),
    )?;
    set_number(
        &object,
        "worldgenMailboxPendingJobs",
        diagnostics.worldgen_mailbox_pending_jobs as f64,
    )?;
    set_number(
        &object,
        "lightStatusMailboxPendingStatuses",
        diagnostics.light_status_mailbox_pending_statuses as f64,
    )?;
    Reflect::set(
        &object,
        &JsValue::from_str("runnerFrameMetrics"),
        &frame_metrics_to_js(diagnostics.runner_frame_metrics)?,
    )
    .map_err(|error| format!("failed to attach runner frame metrics: {error:?}"))?;
    Reflect::set(
        &object,
        &JsValue::from_str("worldgenJobFrameMetrics"),
        &frame_metrics_to_js(diagnostics.worldgen_job_frame_metrics)?,
    )
    .map_err(|error| format!("failed to attach worldgen job frame metrics: {error:?}"))?;
    Reflect::set(
        &object,
        &JsValue::from_str("lightStatusJobFrameMetrics"),
        &frame_metrics_to_js(diagnostics.light_status_job_frame_metrics)?,
    )
    .map_err(|error| format!("failed to attach light-status job frame metrics: {error:?}"))?;
    set_number(
        &object,
        "lastSimulationTick",
        diagnostics.last_tick.simulation_tick as f64,
    )?;
    set_number(
        &object,
        "lastChunkTick",
        diagnostics.last_tick.chunk_tick as f64,
    )?;
    set_number(
        &object,
        "lastTickWallUs",
        diagnostics.last_tick.wall_us as f64,
    )?;
    set_string(
        &object,
        "lastError",
        diagnostics.last_error.as_deref().unwrap_or(""),
    )?;
    Ok(object.into())
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

fn worldgen_mailbox_kind_prop(
    value: &JsValue,
    name: &str,
    fallback: WorldgenMailboxKind,
) -> WorldgenMailboxKind {
    match string_prop(value, name).as_deref() {
        Some("inline") => WorldgenMailboxKind::Inline,
        Some("native-thread") => WorldgenMailboxKind::NativeThread,
        Some("web-worker") => WorldgenMailboxKind::WebWorker,
        _ => fallback,
    }
}

fn light_status_mailbox_kind_prop(
    value: &JsValue,
    name: &str,
    fallback: LightStatusMailboxKind,
) -> LightStatusMailboxKind {
    match string_prop(value, name).as_deref() {
        Some("inline") => LightStatusMailboxKind::Inline,
        Some("native-thread") => LightStatusMailboxKind::NativeThread,
        Some("web-worker") => LightStatusMailboxKind::WebWorker,
        _ => fallback,
    }
}

fn frame_metrics_prop(
    value: &JsValue,
    name: &str,
    fallback: WorkerFrameMetrics,
) -> WorkerFrameMetrics {
    let Some(value) = reflect_get(value, name) else {
        return fallback;
    };
    WorkerFrameMetrics {
        transport_kind: frame_transport_kind_prop(&value, "transportKind", fallback.transport_kind),
        request_frames: number_prop(&value, "requestFrames")
            .unwrap_or(fallback.request_frames as f64) as usize,
        request_bytes: number_prop(&value, "requestBytes").unwrap_or(fallback.request_bytes as f64)
            as usize,
        response_frames: number_prop(&value, "responseFrames")
            .unwrap_or(fallback.response_frames as f64) as usize,
        response_bytes: number_prop(&value, "responseBytes")
            .unwrap_or(fallback.response_bytes as f64) as usize,
        max_pending_frames: number_prop(&value, "maxPendingFrames")
            .unwrap_or(fallback.max_pending_frames as f64) as usize,
        last_request_us: number_prop(&value, "lastRequestUs")
            .unwrap_or(fallback.last_request_us as f64) as u128,
        total_request_us: number_prop(&value, "totalRequestUs")
            .unwrap_or(fallback.total_request_us as f64) as u128,
        max_request_us: number_prop(&value, "maxRequestUs")
            .unwrap_or(fallback.max_request_us as f64) as u128,
        shared_buffer_pool_hits: number_prop(&value, "sharedBufferPoolHits")
            .unwrap_or(fallback.shared_buffer_pool_hits as f64)
            as usize,
        shared_buffer_pool_misses: number_prop(&value, "sharedBufferPoolMisses")
            .unwrap_or(fallback.shared_buffer_pool_misses as f64)
            as usize,
        shared_buffer_pool_drops: number_prop(&value, "sharedBufferPoolDrops")
            .unwrap_or(fallback.shared_buffer_pool_drops as f64)
            as usize,
        shared_buffer_capacity_bytes: number_prop(&value, "sharedBufferCapacityBytes")
            .unwrap_or(fallback.shared_buffer_capacity_bytes as f64)
            as usize,
        max_shared_buffer_capacity_bytes: number_prop(&value, "maxSharedBufferCapacityBytes")
            .unwrap_or(fallback.max_shared_buffer_capacity_bytes as f64)
            as usize,
        shared_buffer_pooled_response_frames: number_prop(
            &value,
            "sharedBufferPooledResponseFrames",
        )
        .unwrap_or(fallback.shared_buffer_pooled_response_frames as f64)
            as usize,
        shared_buffer_fallback_response_frames: number_prop(
            &value,
            "sharedBufferFallbackResponseFrames",
        )
        .unwrap_or(fallback.shared_buffer_fallback_response_frames as f64)
            as usize,
    }
}

fn frame_transport_kind_prop(
    value: &JsValue,
    name: &str,
    fallback: WorkerFrameTransportKind,
) -> WorkerFrameTransportKind {
    match string_prop(value, name).as_deref() {
        Some("none") => WorkerFrameTransportKind::None,
        Some("message-transfer") => WorkerFrameTransportKind::MessageTransfer,
        Some("shared-memory") => WorkerFrameTransportKind::SharedMemory,
        Some("websocket") => WorkerFrameTransportKind::WebSocket,
        _ => fallback,
    }
}

fn frame_metrics_to_js(metrics: WorkerFrameMetrics) -> Result<JsValue, String> {
    let object = Object::new();
    set_string(&object, "transportKind", metrics.transport_kind.label())?;
    set_number(&object, "requestFrames", metrics.request_frames as f64)?;
    set_number(&object, "requestBytes", metrics.request_bytes as f64)?;
    set_number(&object, "responseFrames", metrics.response_frames as f64)?;
    set_number(&object, "responseBytes", metrics.response_bytes as f64)?;
    set_number(
        &object,
        "maxPendingFrames",
        metrics.max_pending_frames as f64,
    )?;
    set_number(&object, "lastRequestUs", metrics.last_request_us as f64)?;
    set_number(&object, "totalRequestUs", metrics.total_request_us as f64)?;
    set_number(&object, "maxRequestUs", metrics.max_request_us as f64)?;
    set_number(
        &object,
        "sharedBufferPoolHits",
        metrics.shared_buffer_pool_hits as f64,
    )?;
    set_number(
        &object,
        "sharedBufferPoolMisses",
        metrics.shared_buffer_pool_misses as f64,
    )?;
    set_number(
        &object,
        "sharedBufferPoolDrops",
        metrics.shared_buffer_pool_drops as f64,
    )?;
    set_number(
        &object,
        "sharedBufferCapacityBytes",
        metrics.shared_buffer_capacity_bytes as f64,
    )?;
    set_number(
        &object,
        "maxSharedBufferCapacityBytes",
        metrics.max_shared_buffer_capacity_bytes as f64,
    )?;
    set_number(
        &object,
        "sharedBufferPooledResponseFrames",
        metrics.shared_buffer_pooled_response_frames as f64,
    )?;
    set_number(
        &object,
        "sharedBufferFallbackResponseFrames",
        metrics.shared_buffer_fallback_response_frames as f64,
    )?;
    Ok(object.into())
}

fn runner_shared_capacity_for_len(len: u32, minimum: u32) -> u32 {
    let wanted = len.max(minimum);
    wanted.checked_next_power_of_two().unwrap_or(wanted)
}

fn ensure_runner_slot_request_capacity(
    slot: &mut RunnerSharedSlot,
    request_bytes: u32,
    frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
) {
    if request_bytes <= slot.request_capacity {
        return;
    }
    let previous_capacity = slot.request_capacity;
    let request_capacity =
        runner_shared_capacity_for_len(request_bytes, MIN_RUNNER_SHARED_REQUEST_BYTES);
    slot.request_buffer = SharedArrayBuffer::new(request_capacity);
    slot.request_capacity = request_capacity;
    frame_metrics
        .borrow_mut()
        .record_shared_buffer_capacity_delta((request_capacity - previous_capacity) as usize);
}

fn grow_runner_slot_response_buffer(
    slot: &mut RunnerSharedSlot,
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
    let response_capacity =
        runner_shared_capacity_for_len(response_bytes, DEFAULT_RUNNER_SHARED_RESPONSE_BYTES);
    slot.response_buffer = SharedArrayBuffer::new(response_capacity);
    slot.response_capacity = response_capacity;
    frame_metrics
        .borrow_mut()
        .record_shared_buffer_capacity_delta((response_capacity - previous_capacity) as usize);
}

fn release_runner_shared_slot(
    slot: RunnerSharedSlot,
    shared_pool: &Rc<RefCell<Vec<RunnerSharedSlot>>>,
    frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
) {
    let mut shared_pool = shared_pool.borrow_mut();
    if shared_pool.len() < MAX_RUNNER_SHARED_POOL_SLOTS {
        shared_pool.push(slot);
    } else {
        frame_metrics
            .borrow_mut()
            .record_shared_buffer_pool_drop(slot.capacity_bytes());
    }
}

fn record_runner_shared_response_metrics(
    frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
    response: &SharedRunnerResponse,
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

fn runner_shared_memory_transport_available() -> bool {
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

fn set_bool(object: &Object, name: &str, value: bool) -> Result<(), String> {
    Reflect::set(object, &JsValue::from_str(name), &JsValue::from_bool(value))
        .map_err(|error| format!("failed to set {name}: {error:?}"))?;
    Ok(())
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

fn elapsed_us_since(start_ms: f64) -> u128 {
    ((js_sys::Date::now() - start_ms).max(0.0) * 1000.0) as u128
}
