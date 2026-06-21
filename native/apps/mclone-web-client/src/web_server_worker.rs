use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
use mclone_protocol::{
    ClientCommand, ServerUpdate, decode_client_command, decode_server_update,
    encode_client_command, encode_server_update,
};
use mclone_server::{
    INITIAL_DAY_TIME, IntegratedServer, IntegratedServerRunner, LightStatusMailboxKind,
    ServerRunnerDiagnostics, ServerRunnerError, ServerRunnerKind, ServerRunnerResult,
    ServerRunnerTickDiagnostics, WasmServerJobWorkerConfig, WorldgenMailboxKind,
    compute_light_status_job_frame, compute_worldgen_job_frame,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

const WEB_WORKER_TICK_INTERVAL_MS: u32 = 50;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebIntegratedServerRunnerConfig {
    pub seed: i64,
    pub worker_url: String,
    pub job_worker_url: String,
    pub bindgen_js_url: String,
    pub bindgen_wasm_url: String,
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
        }
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
    next_request_id: u32,
    pending: Rc<RefCell<BTreeMap<u32, PendingRequest>>>,
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
            .field("pending_requests", &self.pending.borrow().len())
            .field("update_frames", &self.update_frames.borrow().len())
            .field("diagnostics", &self.diagnostics.borrow())
            .field("shutdown_requested", &self.shutdown_requested)
            .finish_non_exhaustive()
    }
}

impl WebIntegratedServerRunner {
    pub async fn new(config: WebIntegratedServerRunnerConfig) -> Result<Self, String> {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        options.set_name("mclone-integrated-server");
        let worker = Worker::new_with_options(&config.worker_url, &options)
            .map_err(|error| format!("failed to spawn integrated server worker: {error:?}"))?;

        let pending = Rc::new(RefCell::new(BTreeMap::new()));
        let update_frames = Rc::new(RefCell::new(Vec::new()));
        let diagnostics = Rc::new(RefCell::new(ServerRunnerDiagnostics::initial(
            ServerRunnerKind::WebWorker,
            config.seed,
            INITIAL_DAY_TIME,
        )));

        let message_closure = {
            let pending = Rc::clone(&pending);
            let update_frames = Rc::clone(&update_frames);
            let diagnostics = Rc::clone(&diagnostics);
            Closure::wrap(Box::new(move |event: MessageEvent| {
                handle_runner_message(event.data(), &pending, &update_frames, &diagnostics);
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
            next_request_id: 1,
            pending,
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
        diagnostics.update_queue_depth = self.update_frames.borrow().len();
        diagnostics
    }

    pub async fn exchange_command(
        &mut self,
        command: ClientCommand,
    ) -> Result<WebWorkerExchange, String> {
        if self.shutdown_requested {
            return Err("integrated server worker is shut down".to_owned());
        }
        let frame =
            encode_client_command(&command).map_err(|error| format!("encode command: {error}"))?;
        let protocol_codec_roundtrip = decode_client_command(&frame)
            .map(|decoded| decoded == command)
            .unwrap_or(false);
        self.post_command_frame(frame).await?;
        let updates = self.drain_decoded_updates()?;
        let diagnostics = self.diagnostics();
        Ok(WebWorkerExchange {
            updates,
            protocol_codec_roundtrip,
            transport_drained: diagnostics.command_queue_depth == 0
                && diagnostics.update_queue_depth == 0
                && diagnostics.pending_jobs == 0
                && diagnostics.pending_publications == 0,
        })
    }

    pub fn drain_decoded_updates(&mut self) -> Result<Vec<ServerUpdate>, String> {
        let frames = self.drain_update_frames();
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
        let _ = self.post_shutdown();
        self.worker.terminate();
        let mut diagnostics = self.diagnostics.borrow_mut();
        diagnostics.running = false;
        diagnostics.awaiting_tick = false;
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
        set_number(
            &message,
            "tickIntervalMs",
            f64::from(WEB_WORKER_TICK_INTERVAL_MS),
        )?;
        let response = self.post_request(request_id, &message, None).await?;
        ensure_worker_response_ok(&response)
    }

    async fn post_command_frame(&mut self, frame: Vec<u8>) -> Result<(), String> {
        let request_id = self.next_request_id();
        let bytes = Uint8Array::from(frame.as_slice());
        let transfer = Array::new();
        transfer.push(&bytes.buffer());
        let message = Object::new();
        set_string(&message, "kind", "command")?;
        set_number(&message, "requestId", f64::from(request_id))?;
        Reflect::set(&message, &JsValue::from_str("frame"), &bytes)
            .map_err(|error| format!("failed to attach command frame: {error:?}"))?;
        let response = self
            .post_request(request_id, &message, Some(&transfer))
            .await?;
        ensure_worker_response_ok(&response)
    }

    fn post_command_frame_fire_and_forget(&mut self, frame: Vec<u8>) -> Result<(), String> {
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
            .map_err(|error| format!("failed to post command to server worker: {error:?}"))
    }

    fn post_shutdown(&mut self) -> Result<(), String> {
        let message = Object::new();
        set_string(&message, "kind", "shutdown")?;
        set_number(&message, "requestId", 0.0)?;
        self.worker
            .post_message(&message)
            .map_err(|error| format!("failed to post shutdown to server worker: {error:?}"))
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
        let mut frames = self.update_frames.borrow_mut();
        let drained = std::mem::take(&mut *frames);
        self.diagnostics.borrow_mut().update_queue_depth = 0;
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

#[wasm_bindgen]
pub fn mclone_web_compute_worldgen_job_frame(frame: Uint8Array) -> Result<Uint8Array, JsValue> {
    let response = compute_worldgen_job_frame(&frame.to_vec()).map_err(JsValue::from)?;
    Ok(Uint8Array::from(response.as_slice()))
}

#[wasm_bindgen]
pub fn mclone_web_compute_light_status_job_frame(frame: Uint8Array) -> Result<Uint8Array, JsValue> {
    let response = compute_light_status_job_frame(&frame.to_vec()).map_err(JsValue::from)?;
    Ok(Uint8Array::from(response.as_slice()))
}

#[derive(Clone)]
struct PendingRequest {
    resolve: Function,
    reject: Function,
}

fn handle_runner_message(
    data: JsValue,
    pending: &Rc<RefCell<BTreeMap<u32, PendingRequest>>>,
    update_frames: &Rc<RefCell<Vec<Vec<u8>>>>,
    diagnostics: &Rc<RefCell<ServerRunnerDiagnostics>>,
) {
    if let Some(diagnostic_value) = reflect_get(&data, "diagnostics") {
        let fallback = diagnostics.borrow().clone();
        if let Some(parsed) = parse_diagnostics(&diagnostic_value, &fallback) {
            *diagnostics.borrow_mut() = parsed;
        }
    }
    if let Some(updates_value) = reflect_get(&data, "updates") {
        let frames = frames_from_js(&updates_value);
        if !frames.is_empty() {
            let mut queued = update_frames.borrow_mut();
            queued.extend(frames);
            diagnostics.borrow_mut().update_queue_depth = queued.len();
        }
    }

    let request_id = number_prop(&data, "requestId")
        .map(|value| value as u32)
        .unwrap_or(0);
    if request_id == 0 {
        return;
    }
    let Some(pending_request) = pending.borrow_mut().remove(&request_id) else {
        return;
    };
    if bool_prop(&data, "ok") == Some(false) {
        let reason = string_prop(&data, "reason")
            .unwrap_or_else(|| "integrated server worker returned an unknown failure".to_owned());
        let _ = pending_request
            .reject
            .call1(&JsValue::NULL, &JsValue::from_str(&reason));
    } else {
        let _ = pending_request.resolve.call1(&JsValue::NULL, &data);
    }
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
    command_queue_depth: usize,
    running: bool,
}

#[wasm_bindgen]
impl McloneWebIntegratedServerWorker {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: i64) -> Self {
        let server = IntegratedServer::new(seed);
        Self::from_server(seed, server)
    }

    #[wasm_bindgen(js_name = withJobWorkers)]
    pub fn with_job_workers(
        seed: i64,
        job_worker_url: String,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
    ) -> Self {
        let server = IntegratedServer::with_wasm_job_workers(
            seed,
            WasmServerJobWorkerConfig::new(job_worker_url, bindgen_js_url, bindgen_wasm_url),
        );
        Self::from_server(seed, server)
    }

    #[wasm_bindgen(js_name = handleCommandFrame)]
    pub fn handle_command_frame(&mut self, frame: Uint8Array) -> Result<JsValue, JsValue> {
        self.command_queue_depth = self.command_queue_depth.saturating_add(1);
        self.refresh_diagnostics(None, true, None);
        let result = decode_client_command(&frame.to_vec())
            .map_err(|error| error.to_string())
            .and_then(|command| {
                let mut updates = self
                    .server
                    .try_handle_command(command)
                    .map_err(|error| error.to_string())?;
                updates.extend(self.server.try_poll().map_err(|error| error.to_string())?);
                Ok(updates)
            });
        self.command_queue_depth = self.command_queue_depth.saturating_sub(1);
        match result {
            Ok(updates) => {
                self.refresh_diagnostics(None, false, None);
                worker_response(updates, &self.diagnostics).map_err(JsValue::from)
            }
            Err(error) => {
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
                worker_response(updates, &self.diagnostics).map_err(JsValue::from)
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
            return worker_response(Vec::new(), &self.diagnostics).map_err(JsValue::from);
        }
        let wall_start = js_sys::Date::now();
        match self.server.try_simulation_tick_report() {
            Ok(report) => {
                let wall_us = ((js_sys::Date::now() - wall_start).max(0.0) * 1000.0) as u128;
                let updates = report.updates.clone();
                self.refresh_diagnostics(
                    Some(ServerRunnerTickDiagnostics::from_report(&report, wall_us)),
                    false,
                    None,
                );
                worker_response(updates, &self.diagnostics).map_err(JsValue::from)
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
        self.refresh_diagnostics(None, false, None);
        worker_response(Vec::new(), &self.diagnostics).map_err(JsValue::from)
    }
}

impl McloneWebIntegratedServerWorker {
    fn from_server(seed: i64, server: IntegratedServer) -> Self {
        let mut diagnostics =
            ServerRunnerDiagnostics::initial(ServerRunnerKind::WebWorker, seed, server.day_time());
        diagnostics.running = true;
        let mut worker = Self {
            server,
            diagnostics,
            command_queue_depth: 0,
            running: true,
        };
        worker.refresh_diagnostics(None, false, None);
        worker
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
        self.diagnostics.worldgen_mailbox_kind = self.server.scheduler().worldgen_mailbox_kind();
        self.diagnostics.light_status_mailbox_kind =
            self.server.scheduler().light_status_mailbox_kind();
        self.diagnostics.worldgen_mailbox_pending_jobs =
            self.server.scheduler().worldgen_mailbox_pending_count();
        self.diagnostics.light_status_mailbox_pending_statuses =
            self.server.scheduler().light_status_mailbox_pending_count();
        self.diagnostics.scheduler_metrics = self.server.scheduler().metrics();
        self.diagnostics.chunk_tracking = self.server.chunk_tracking_diagnostics();
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
    diagnostics: &ServerRunnerDiagnostics,
) -> Result<JsValue, String> {
    let object = Object::new();
    let packed_updates = Array::new();
    for update in updates {
        let frame = encode_server_update(&update).map_err(|error| error.to_string())?;
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
