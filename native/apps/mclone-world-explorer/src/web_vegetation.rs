use std::collections::VecDeque;

use js_sys::{Array, Function, Int32Array, Object, Reflect, SharedArrayBuffer, Uint8Array};
use mclone_terrain_view::{
    TerrainVegetationExecutor, TerrainVegetationExecutorActor,
    TerrainVegetationExecutorDiagnostics, TerrainVegetationExecutorEvent,
    TerrainVegetationExecutorJob, TerrainVegetationExecutorKind, TerrainVegetationSubmitError,
};
use mclone_worldgen::terrain_vegetation::{
    MCHV_MAX_RESULT_CAPACITY, MCHV_RESIDENT_RESULT_CAPACITY, MchvActorIdentity, MchvFailureKind,
    MchvFrame, MchvJobIdentity, TerrainVegetationCompileError, TerrainVegetationCompilerSession,
    TerrainVegetationSourceIdentity, decode_mchv_frame, encode_mchv_frame,
};
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};

const RESULT_CONTROL_WORDS: u32 = 6;
const RESULT_STATUS_INDEX: u32 = 0;
const RESULT_BYTES_INDEX: u32 = 1;
const RESULT_CAPACITY_INDEX: u32 = 2;
const RESULT_EXECUTOR_GENERATION_INDEX: u32 = 3;
const RESULT_SOURCE_EPOCH_INDEX: u32 = 4;
const RESULT_REQUEST_ID_INDEX: u32 = 5;

const RESULT_PENDING: i32 = 1;
const RESULT_COMPLETE: i32 = 2;
const RESULT_OVERFLOW: i32 = 3;
const RESULT_FAILED: i32 = 4;

const FRAME_BYTES_PROPERTY: &str = "frame";
const CONTROL_BUFFER_PROPERTY: &str = "control";
const RESULT_BUFFER_PROPERTY: &str = "result";
const OVERFLOW_BUFFER_PROPERTY: &str = "buffer";

struct BrowserSharedResultArena {
    control_buffer: SharedArrayBuffer,
    control: Int32Array,
    result_buffer: SharedArrayBuffer,
    capacity: u32,
    high_water: u32,
    overflow_count: u64,
    copied_bytes: u64,
}

struct BrowserSharedResultRead {
    bytes: Vec<u8>,
}

impl BrowserSharedResultArena {
    fn new() -> Result<Self, String> {
        Self::with_capacity(MCHV_RESIDENT_RESULT_CAPACITY)
    }

    fn with_capacity(initial_capacity: usize) -> Result<Self, String> {
        if !shared_memory_supported() {
            return Err(
                "World Explorer vegetation requires cross-origin isolation, SharedArrayBuffer, \
                 and Atomics"
                    .to_owned(),
            );
        }
        if initial_capacity == 0
            || !initial_capacity.is_power_of_two()
            || initial_capacity > MCHV_MAX_RESULT_CAPACITY
        {
            return Err(format!(
                "MCHV initial result capacity {initial_capacity} must be a non-zero power of \
                 two no larger than {MCHV_MAX_RESULT_CAPACITY}"
            ));
        }
        let capacity = u32::try_from(initial_capacity)
            .map_err(|_| "MCHV initial result capacity exceeds u32")?;
        let control_buffer = SharedArrayBuffer::new(RESULT_CONTROL_WORDS * 4);
        let control = Int32Array::new(control_buffer.as_ref());
        let result_buffer = SharedArrayBuffer::new(capacity);
        let arena = Self {
            control_buffer,
            control,
            result_buffer,
            capacity,
            high_water: 0,
            overflow_count: 0,
            copied_bytes: 0,
        };
        atomic_store(&arena.control, RESULT_STATUS_INDEX, RESULT_COMPLETE)?;
        atomic_store(&arena.control, RESULT_BYTES_INDEX, 0)?;
        atomic_store(
            &arena.control,
            RESULT_CAPACITY_INDEX,
            i32::try_from(capacity).expect("MCHV resident capacity fits i32"),
        )?;
        atomic_store(&arena.control, RESULT_EXECUTOR_GENERATION_INDEX, 0)?;
        atomic_store(&arena.control, RESULT_SOURCE_EPOCH_INDEX, 0)?;
        atomic_store(&arena.control, RESULT_REQUEST_ID_INDEX, 0)?;
        Ok(arena)
    }

    fn arm_and_attach(
        &self,
        message: &Object,
        actor: TerrainVegetationExecutorActor,
        request_id: u32,
    ) -> Result<(), String> {
        atomic_store(&self.control, RESULT_BYTES_INDEX, 0)?;
        atomic_store(
            &self.control,
            RESULT_CAPACITY_INDEX,
            i32::try_from(self.capacity).expect("MCHV resident capacity fits i32"),
        )?;
        atomic_store(
            &self.control,
            RESULT_EXECUTOR_GENERATION_INDEX,
            actor.executor_generation as i32,
        )?;
        atomic_store(
            &self.control,
            RESULT_SOURCE_EPOCH_INDEX,
            actor.source_epoch as i32,
        )?;
        atomic_store(&self.control, RESULT_REQUEST_ID_INDEX, request_id as i32)?;
        atomic_store(&self.control, RESULT_STATUS_INDEX, RESULT_PENDING)?;
        set_value(
            message,
            CONTROL_BUFFER_PROPERTY,
            self.control_buffer.as_ref(),
        )?;
        set_value(message, RESULT_BUFFER_PROPERTY, self.result_buffer.as_ref())?;
        Ok(())
    }

    fn read_published(
        &mut self,
        response: &JsValue,
        actor: TerrainVegetationExecutorActor,
        request_id: u32,
    ) -> Result<BrowserSharedResultRead, String> {
        let published_actor = TerrainVegetationExecutorActor {
            executor_generation: atomic_load(&self.control, RESULT_EXECUTOR_GENERATION_INDEX)?
                as u32,
            source_epoch: atomic_load(&self.control, RESULT_SOURCE_EPOCH_INDEX)? as u32,
        };
        let published_request_id = atomic_load(&self.control, RESULT_REQUEST_ID_INDEX)? as u32;
        if published_actor != actor || published_request_id != request_id {
            return Err(format!(
                "browser Worker result identity ({published_actor:?}, {published_request_id}) \
                 does not match ({actor:?}, {request_id})"
            ));
        }

        let byte_length =
            nonnegative_word(&self.control, RESULT_BYTES_INDEX, "published byte length")?;
        let published_capacity =
            nonnegative_word(&self.control, RESULT_CAPACITY_INDEX, "published capacity")?;
        let status = atomic_load(&self.control, RESULT_STATUS_INDEX)?;
        let buffer = match status {
            RESULT_COMPLETE => self.result_buffer.clone(),
            RESULT_OVERFLOW => {
                self.overflow_count = self.overflow_count.saturating_add(1);
                let overflow = shared_buffer_property(response, OVERFLOW_BUFFER_PROPERTY)?;
                if overflow.byte_length() < byte_length {
                    return Err(format!(
                        "browser Worker overflow buffer has {} bytes for a {byte_length}-byte frame",
                        overflow.byte_length()
                    ));
                }
                self.grow_after_overflow(byte_length)?;
                overflow
            }
            RESULT_FAILED => {
                return Err("browser Worker failed before publishing an MCHV frame".to_owned());
            }
            other => {
                return Err(format!(
                    "browser Worker result status {other} is not complete"
                ));
            }
        };
        if byte_length > published_capacity {
            return Err(format!(
                "browser Worker published {byte_length} bytes with capacity \
                 {published_capacity}"
            ));
        }
        if byte_length > buffer.byte_length() {
            return Err(format!(
                "browser Worker published {byte_length} bytes into a {}-byte buffer",
                buffer.byte_length()
            ));
        }
        let bytes =
            Uint8Array::new_with_byte_offset_and_length(buffer.as_ref(), 0, byte_length).to_vec();
        self.high_water = self.high_water.max(byte_length);
        self.copied_bytes = self.copied_bytes.saturating_add(u64::from(byte_length));
        Ok(BrowserSharedResultRead { bytes })
    }

    fn grow_after_overflow(&mut self, required: u32) -> Result<(), String> {
        if required <= self.capacity {
            return Ok(());
        }
        let maximum = u32::try_from(MCHV_MAX_RESULT_CAPACITY)
            .map_err(|_| "MCHV maximum result capacity exceeds u32")?;
        let next_capacity = required
            .checked_next_power_of_two()
            .unwrap_or(maximum.saturating_add(1));
        if required > maximum || next_capacity > maximum {
            return Err(format!(
                "browser Worker result requires {required} bytes, above the \
                 {maximum}-byte MCHV bound"
            ));
        }
        self.result_buffer = SharedArrayBuffer::new(next_capacity);
        self.capacity = next_capacity;
        Ok(())
    }

    fn apply_diagnostics(&self, diagnostics: &mut TerrainVegetationExecutorDiagnostics) {
        diagnostics.result_capacity_bytes = u64::from(self.capacity);
        diagnostics.result_high_water_bytes = u64::from(self.high_water);
        diagnostics.result_overflows = self.overflow_count;
        diagnostics.copied_result_bytes = self.copied_bytes;
    }
}

#[derive(Clone, Copy)]
enum BrowserPending {
    Initialize {
        actor: TerrainVegetationExecutorActor,
        source: TerrainVegetationSourceIdentity,
    },
    Compile(TerrainVegetationExecutorJob),
    Shutdown {
        actor: TerrainVegetationExecutorActor,
    },
}

impl BrowserPending {
    const fn actor(self) -> TerrainVegetationExecutorActor {
        match self {
            Self::Initialize { actor, .. } | Self::Shutdown { actor } => actor,
            Self::Compile(job) => job.identity.actor,
        }
    }

    const fn request_id(self) -> u32 {
        match self {
            Self::Initialize { .. } | Self::Shutdown { .. } => 0,
            Self::Compile(job) => job.identity.request_id,
        }
    }
}

pub struct WebTerrainVegetationExecutor {
    transport_factory: Function,
    transport: Option<JsValue>,
    arena: BrowserSharedResultArena,
    actor: Option<TerrainVegetationExecutorActor>,
    pending: Option<BrowserPending>,
    shutdown_requested: Option<TerrainVegetationExecutorActor>,
    queued_events: VecDeque<TerrainVegetationExecutorEvent>,
    diagnostics: TerrainVegetationExecutorDiagnostics,
    terminated: bool,
}

impl WebTerrainVegetationExecutor {
    pub fn new(transport_factory: JsValue) -> Result<Self, String> {
        Self::with_initial_capacity(transport_factory, MCHV_RESIDENT_RESULT_CAPACITY)
    }

    pub fn with_initial_capacity(
        transport_factory: JsValue,
        initial_capacity: usize,
    ) -> Result<Self, String> {
        let transport_factory = transport_factory.dyn_into::<Function>().map_err(|_| {
            "World Explorer browser Worker transport factory is not callable".to_owned()
        })?;
        Ok(Self {
            transport_factory,
            transport: None,
            arena: if initial_capacity == MCHV_RESIDENT_RESULT_CAPACITY {
                BrowserSharedResultArena::new()?
            } else {
                BrowserSharedResultArena::with_capacity(initial_capacity)?
            },
            actor: None,
            pending: None,
            shutdown_requested: None,
            queued_events: VecDeque::new(),
            diagnostics: TerrainVegetationExecutorDiagnostics::default(),
            terminated: false,
        })
    }

    fn create_transport(&self) -> Result<JsValue, String> {
        let transport = self
            .transport_factory
            .call0(&JsValue::UNDEFINED)
            .map_err(js_message)?;
        for name in ["post", "poll", "terminate"] {
            method(&transport, name)?;
        }
        Ok(transport)
    }

    fn post(&mut self, pending: BrowserPending, frame: MchvFrame) -> Result<(), String> {
        if self.pending.is_some() {
            return Err("browser Worker already has an in-flight frame".to_owned());
        }
        let transport = self
            .transport
            .as_ref()
            .ok_or("browser Worker transport is not active")?;
        let encoded = encode_mchv_frame(&frame)?;
        let frame_bytes = Uint8Array::from(encoded.as_slice());
        let message = Object::new();
        set_value(&message, FRAME_BYTES_PROPERTY, frame_bytes.as_ref())?;
        self.arena
            .arm_and_attach(&message, pending.actor(), pending.request_id())?;
        let transfer = Array::new();
        transfer.push(&frame_bytes.buffer());
        method(transport, "post")?
            .call2(transport, message.as_ref(), transfer.as_ref())
            .map_err(js_message)?;
        self.pending = Some(pending);
        Ok(())
    }

    fn poll_transport(&mut self) {
        for _ in 0..64 {
            let Some(transport) = self.transport.clone() else {
                break;
            };
            let event = match method(&transport, "poll")
                .and_then(|poll| poll.call0(&transport).map_err(js_message))
            {
                Ok(event) => event,
                Err(error) => {
                    self.transport_failed(error);
                    break;
                }
            };
            if event.is_null() || event.is_undefined() {
                break;
            }
            let kind = match string_property(&event, "kind") {
                Ok(kind) => kind,
                Err(error) => {
                    self.transport_failed(error);
                    break;
                }
            };
            match kind.as_str() {
                "message" => {
                    let data = match property(&event, "data") {
                        Ok(data) => data,
                        Err(error) => {
                            self.transport_failed(error);
                            break;
                        }
                    };
                    let decode_started = js_sys::Date::now();
                    let result = self.handle_doorbell(data);
                    self.diagnostics.main_decode_micros = self
                        .diagnostics
                        .main_decode_micros
                        .saturating_add(millis_to_micros(js_sys::Date::now() - decode_started));
                    if let Err(error) = result {
                        self.transport_failed(error);
                        break;
                    }
                }
                "error" => {
                    let error = string_property(&event, "message")
                        .unwrap_or_else(|_| "browser Worker failed".to_owned());
                    self.transport_failed(error);
                    break;
                }
                other => {
                    self.transport_failed(format!(
                        "browser Worker transport returned unsupported event {other:?}"
                    ));
                    break;
                }
            }
        }
        self.maybe_send_shutdown();
        self.arena.apply_diagnostics(&mut self.diagnostics);
    }

    fn handle_doorbell(&mut self, response: JsValue) -> Result<(), String> {
        let pending = self
            .pending
            .take()
            .ok_or("browser Worker rang a doorbell with no in-flight frame")?;
        let published =
            self.arena
                .read_published(&response, pending.actor(), pending.request_id())?;
        let frame = decode_mchv_frame(&published.bytes)?;
        match (pending, frame) {
            (
                BrowserPending::Initialize { actor, source },
                MchvFrame::Ready {
                    actor: response_actor,
                    source: response_source,
                },
            ) if response_actor == mchv_actor(actor) && response_source == source => {
                self.queued_events
                    .push_back(TerrainVegetationExecutorEvent::Ready { actor, source });
            }
            (
                BrowserPending::Compile(job),
                MchvFrame::Completed {
                    job: response_job,
                    source,
                    product,
                    compile_micros,
                },
            ) if response_job == mchv_job(&job)
                && source == job.source
                && product.request().request() == job.request =>
            {
                self.diagnostics.completed_jobs = self.diagnostics.completed_jobs.saturating_add(1);
                self.queued_events
                    .push_back(TerrainVegetationExecutorEvent::Completed {
                        identity: job.identity,
                        source,
                        product,
                        compile_micros,
                    });
            }
            (
                BrowserPending::Shutdown { actor },
                MchvFrame::ShutdownComplete {
                    actor: response_actor,
                },
            ) if response_actor == mchv_actor(actor) => {
                self.terminated = true;
                self.terminate_transport();
                self.queued_events
                    .push_back(TerrainVegetationExecutorEvent::ShutdownComplete { actor });
            }
            (
                pending,
                MchvFrame::Failed {
                    actor,
                    request_id,
                    kind,
                    message,
                },
            ) if actor == mchv_actor(pending.actor()) && request_id == pending.request_id() => {
                let error = format!("browser Worker {kind:?} failure: {message}");
                match pending {
                    BrowserPending::Compile(job) => {
                        self.queued_events
                            .push_back(TerrainVegetationExecutorEvent::JobFailed {
                                identity: job.identity,
                                error,
                            });
                    }
                    BrowserPending::Initialize { .. } | BrowserPending::Shutdown { .. } => {
                        return Err(error);
                    }
                }
            }
            (pending, response) => {
                return Err(format!(
                    "browser Worker MCHV response {response:?} does not match pending \
                     actor {:?} request {}",
                    pending.actor(),
                    pending.request_id()
                ));
            }
        }
        Ok(())
    }

    fn maybe_send_shutdown(&mut self) {
        let Some(actor) = self.shutdown_requested else {
            return;
        };
        if self.pending.is_some() || self.terminated {
            return;
        }
        if let Err(error) = self.post(
            BrowserPending::Shutdown { actor },
            MchvFrame::Shutdown {
                actor: mchv_actor(actor),
            },
        ) {
            self.diagnostics.transport_failures =
                self.diagnostics.transport_failures.saturating_add(1);
            self.force_shutdown(actor, error);
        }
    }

    fn transport_failed(&mut self, error: String) {
        let actor = self
            .pending
            .take()
            .map(BrowserPending::actor)
            .or(self.actor)
            .unwrap_or(TerrainVegetationExecutorActor {
                executor_generation: 0,
                source_epoch: 0,
            });
        self.diagnostics.transport_failures = self.diagnostics.transport_failures.saturating_add(1);
        self.terminate_transport();
        self.queued_events
            .push_back(TerrainVegetationExecutorEvent::TransportFailed { actor, error });
    }

    fn force_shutdown(&mut self, actor: TerrainVegetationExecutorActor, _error: String) {
        self.pending = None;
        self.terminate_transport();
        self.terminated = true;
        self.queued_events
            .push_back(TerrainVegetationExecutorEvent::ShutdownComplete { actor });
    }

    fn terminate_transport(&mut self) {
        if let Some(transport) = self.transport.take()
            && let Ok(terminate) = method(&transport, "terminate")
        {
            let _ = terminate.call0(&transport);
        }
    }
}

impl TerrainVegetationExecutor for WebTerrainVegetationExecutor {
    fn kind(&self) -> TerrainVegetationExecutorKind {
        TerrainVegetationExecutorKind::BrowserWorker
    }

    fn try_submit(
        &mut self,
        job: &TerrainVegetationExecutorJob,
    ) -> Result<(), TerrainVegetationSubmitError> {
        if self.pending.is_some() {
            return Err(TerrainVegetationSubmitError::Full);
        }
        if self.actor != Some(job.identity.actor) {
            return Err(TerrainVegetationSubmitError::Failed(
                "browser Worker job targets an inactive actor".to_owned(),
            ));
        }
        self.post(
            BrowserPending::Compile(*job),
            MchvFrame::Compile {
                job: mchv_job(job),
                source: job.source,
                request: job.request,
            },
        )
        .map_err(TerrainVegetationSubmitError::Failed)?;
        self.diagnostics.submitted_jobs = self.diagnostics.submitted_jobs.saturating_add(1);
        Ok(())
    }

    fn drain_events(&mut self) -> Vec<TerrainVegetationExecutorEvent> {
        self.poll_transport();
        self.queued_events.drain(..).collect()
    }

    fn restart(
        &mut self,
        actor: TerrainVegetationExecutorActor,
        source: TerrainVegetationSourceIdentity,
    ) -> Result<(), String> {
        source.validate()?;
        self.terminate_transport();
        self.pending = None;
        self.shutdown_requested = None;
        self.terminated = false;
        self.transport = Some(self.create_transport()?);
        self.actor = Some(actor);
        self.diagnostics.restarts = self.diagnostics.restarts.saturating_add(1);
        self.post(
            BrowserPending::Initialize { actor, source },
            MchvFrame::Initialize {
                actor: mchv_actor(actor),
                source,
            },
        )
    }

    fn request_shutdown(&mut self, actor: TerrainVegetationExecutorActor) {
        if self.shutdown_requested.is_some() || self.terminated {
            return;
        }
        self.shutdown_requested = Some(actor);
        self.maybe_send_shutdown();
    }

    fn is_terminated(&self) -> bool {
        self.terminated
    }

    fn diagnostics(&self) -> TerrainVegetationExecutorDiagnostics {
        self.diagnostics
    }
}

impl Drop for WebTerrainVegetationExecutor {
    fn drop(&mut self) {
        self.terminate_transport();
    }
}

#[wasm_bindgen(js_name = WorldExplorerWorkerActor)]
#[derive(Default)]
pub struct WorldExplorerWorkerActor {
    actor: Option<MchvActorIdentity>,
    session: Option<TerrainVegetationCompilerSession>,
    terminated: bool,
}

#[wasm_bindgen(js_class = WorldExplorerWorkerActor)]
impl WorldExplorerWorkerActor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[wasm_bindgen(js_name = handleMessage)]
    pub fn handle_message(&mut self, message: JsValue) -> WorldExplorerWorkerDispatch {
        let response = self
            .handle_message_inner(&message)
            .or_else(|error| failure_from_message(&message, error));
        match response.and_then(|frame| publish_frame(&message, &frame)) {
            Ok(dispatch) => dispatch,
            Err(error) => {
                mark_publication_failed(&message);
                bootstrap_error_dispatch(error)
            }
        }
    }
}

impl WorldExplorerWorkerActor {
    fn handle_message_inner(&mut self, message: &JsValue) -> Result<MchvFrame, String> {
        let encoded = uint8_array_property(message, FRAME_BYTES_PROPERTY)?.to_vec();
        let frame = decode_mchv_frame(&encoded)?;
        match frame {
            MchvFrame::Initialize { actor, source } => {
                if self.terminated {
                    return Err("worker actor is already terminated".to_owned());
                }
                source.validate()?;
                self.actor = Some(actor);
                self.session = Some(TerrainVegetationCompilerSession::new(source));
                Ok(MchvFrame::Ready { actor, source })
            }
            MchvFrame::Compile {
                job,
                source,
                request,
            } => {
                if self.terminated {
                    return Ok(failed_frame(
                        job.actor,
                        job.request_id,
                        MchvFailureKind::Protocol,
                        "worker actor is terminated",
                    ));
                }
                if self.actor != Some(job.actor) {
                    return Ok(failed_frame(
                        job.actor,
                        job.request_id,
                        MchvFailureKind::Protocol,
                        "compile frame targets an inactive actor",
                    ));
                }
                let session = self.session.as_mut().ok_or_else(|| {
                    "worker actor received compile before initialization".to_owned()
                })?;
                if session.source() != Some(source) {
                    return Ok(failed_frame(
                        job.actor,
                        job.request_id,
                        MchvFailureKind::Source,
                        "compile source does not match the initialized session",
                    ));
                }
                let started = js_sys::Date::now();
                match session.compile(source, request) {
                    Ok(product) => Ok(MchvFrame::Completed {
                        job,
                        source,
                        product,
                        compile_micros: millis_to_micros(js_sys::Date::now() - started),
                    }),
                    Err(error) => {
                        let kind = match error {
                            TerrainVegetationCompileError::InvalidSource(_) => {
                                MchvFailureKind::Source
                            }
                            TerrainVegetationCompileError::Compile(_) => MchvFailureKind::Compile,
                        };
                        Ok(failed_frame(
                            job.actor,
                            job.request_id,
                            kind,
                            &error.to_string(),
                        ))
                    }
                }
            }
            MchvFrame::Shutdown { actor } => {
                if self.actor != Some(actor) {
                    return Ok(failed_frame(
                        actor,
                        0,
                        MchvFailureKind::Protocol,
                        "shutdown frame targets an inactive actor",
                    ));
                }
                self.session = None;
                self.terminated = true;
                Ok(MchvFrame::ShutdownComplete { actor })
            }
            other => Err(format!(
                "worker actor received unsupported MCHV command {other:?}"
            )),
        }
    }
}

#[wasm_bindgen(js_name = WorldExplorerWorkerDispatch)]
pub struct WorldExplorerWorkerDispatch {
    message: Object,
}

#[wasm_bindgen(js_class = WorldExplorerWorkerDispatch)]
impl WorldExplorerWorkerDispatch {
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> JsValue {
        self.message.clone().into()
    }
}

fn publish_frame(
    message: &JsValue,
    frame: &MchvFrame,
) -> Result<WorldExplorerWorkerDispatch, String> {
    if !shared_memory_supported() {
        return Err("worker actor has no SharedArrayBuffer/Atomics capability".to_owned());
    }
    let control_buffer = shared_buffer_property(message, CONTROL_BUFFER_PROPERTY)?;
    if control_buffer.byte_length() < RESULT_CONTROL_WORDS * 4 {
        return Err("worker result control buffer is too small".to_owned());
    }
    let control = Int32Array::new(control_buffer.as_ref());
    if atomic_load(&control, RESULT_STATUS_INDEX)? != RESULT_PENDING {
        return Err("worker result mailbox was not armed as pending".to_owned());
    }
    let (actor, request_id) = frame_identity(frame)?;
    let control_actor = MchvActorIdentity {
        executor_generation: atomic_load(&control, RESULT_EXECUTOR_GENERATION_INDEX)? as u32,
        source_epoch: atomic_load(&control, RESULT_SOURCE_EPOCH_INDEX)? as u32,
    };
    let control_request_id = atomic_load(&control, RESULT_REQUEST_ID_INDEX)? as u32;
    if actor != control_actor || request_id != control_request_id {
        return Err(format!(
            "worker frame identity ({actor:?}, {request_id}) does not match control \
             ({control_actor:?}, {control_request_id})"
        ));
    }
    let encoded = encode_mchv_frame(frame)?;
    let byte_length =
        u32::try_from(encoded.len()).map_err(|_| "worker result length exceeds u32")?;
    let maximum = u32::try_from(MCHV_MAX_RESULT_CAPACITY)
        .map_err(|_| "MCHV maximum result capacity exceeds u32")?;
    if byte_length > maximum {
        return Err(format!(
            "worker result requires {byte_length} bytes, above the {maximum}-byte MCHV bound"
        ));
    }
    let resident = shared_buffer_property(message, RESULT_BUFFER_PROPERTY)?;
    let overflow = resident.byte_length() < byte_length;
    let buffer = if overflow {
        SharedArrayBuffer::new(byte_length)
    } else {
        resident
    };
    if byte_length > 0 {
        Uint8Array::new_with_byte_offset_and_length(buffer.as_ref(), 0, byte_length)
            .copy_from(&encoded);
    }
    atomic_store(&control, RESULT_BYTES_INDEX, byte_length as i32)?;
    atomic_store(
        &control,
        RESULT_CAPACITY_INDEX,
        i32::try_from(buffer.byte_length())
            .map_err(|_| "worker result buffer capacity exceeds i32")?,
    )?;
    atomic_store(
        &control,
        RESULT_STATUS_INDEX,
        if overflow {
            RESULT_OVERFLOW
        } else {
            RESULT_COMPLETE
        },
    )?;
    atomic_notify(&control, RESULT_STATUS_INDEX)?;
    let dispatch = Object::new();
    if overflow {
        set_value(&dispatch, OVERFLOW_BUFFER_PROPERTY, buffer.as_ref())?;
    }
    Ok(WorldExplorerWorkerDispatch { message: dispatch })
}

fn failure_from_message(message: &JsValue, error: String) -> Result<MchvFrame, String> {
    let control =
        Int32Array::new(shared_buffer_property(message, CONTROL_BUFFER_PROPERTY)?.as_ref());
    Ok(failed_frame(
        MchvActorIdentity {
            executor_generation: atomic_load(&control, RESULT_EXECUTOR_GENERATION_INDEX)? as u32,
            source_epoch: atomic_load(&control, RESULT_SOURCE_EPOCH_INDEX)? as u32,
        },
        atomic_load(&control, RESULT_REQUEST_ID_INDEX)? as u32,
        MchvFailureKind::Protocol,
        &error,
    ))
}

fn failed_frame(
    actor: MchvActorIdentity,
    request_id: u32,
    kind: MchvFailureKind,
    message: &str,
) -> MchvFrame {
    let mut message = message.to_owned();
    let mut maximum = mclone_worldgen::terrain_vegetation::MCHV_MAX_ERROR_BYTES.min(message.len());
    while !message.is_char_boundary(maximum) {
        maximum = maximum.saturating_sub(1);
    }
    message.truncate(maximum);
    MchvFrame::Failed {
        actor,
        request_id,
        kind,
        message,
    }
}

fn bootstrap_error_dispatch(error: String) -> WorldExplorerWorkerDispatch {
    let message = Object::new();
    let _ = set_value(
        &message,
        "error",
        &JsValue::from_str(&format!("worker bootstrap failed: {error}")),
    );
    WorldExplorerWorkerDispatch { message }
}

fn mark_publication_failed(message: &JsValue) {
    let Ok(buffer) = shared_buffer_property(message, CONTROL_BUFFER_PROPERTY) else {
        return;
    };
    if buffer.byte_length() < RESULT_CONTROL_WORDS * 4 {
        return;
    }
    let control = Int32Array::new(buffer.as_ref());
    let _ = atomic_store(&control, RESULT_STATUS_INDEX, RESULT_FAILED);
    let _ = atomic_notify(&control, RESULT_STATUS_INDEX);
}

fn frame_identity(frame: &MchvFrame) -> Result<(MchvActorIdentity, u32), String> {
    match frame {
        MchvFrame::Ready { actor, .. } | MchvFrame::ShutdownComplete { actor } => Ok((*actor, 0)),
        MchvFrame::Completed { job, .. } => Ok((job.actor, job.request_id)),
        MchvFrame::Failed {
            actor, request_id, ..
        } => Ok((*actor, *request_id)),
        other => Err(format!(
            "worker cannot publish command frame as a response: {other:?}"
        )),
    }
}

const fn mchv_actor(actor: TerrainVegetationExecutorActor) -> MchvActorIdentity {
    MchvActorIdentity {
        executor_generation: actor.executor_generation,
        source_epoch: actor.source_epoch,
    }
}

const fn mchv_job(job: &TerrainVegetationExecutorJob) -> MchvJobIdentity {
    MchvJobIdentity {
        actor: mchv_actor(job.identity.actor),
        request_id: job.identity.request_id,
        physical_slot: job.identity.slot.physical_slot,
        slot_generation: job.identity.slot.slot_generation,
    }
}

fn millis_to_micros(milliseconds: f64) -> u64 {
    if !milliseconds.is_finite() || milliseconds <= 0.0 {
        0
    } else if milliseconds >= u64::MAX as f64 / 1_000.0 {
        u64::MAX
    } else {
        (milliseconds * 1_000.0).round() as u64
    }
}

fn shared_memory_supported() -> bool {
    let global = js_sys::global();
    global_function(&global, "SharedArrayBuffer")
        && global_method(&global, "Atomics", "load")
        && global_method(&global, "Atomics", "store")
        && global_method(&global, "Atomics", "notify")
        && Reflect::get(&global, &JsValue::from_str("crossOriginIsolated"))
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
}

fn global_function(global: &JsValue, name: &str) -> bool {
    Reflect::get(global, &JsValue::from_str(name))
        .ok()
        .is_some_and(|value| value.is_function())
}

fn global_method(global: &JsValue, object: &str, name: &str) -> bool {
    Reflect::get(global, &JsValue::from_str(object))
        .ok()
        .and_then(|object| Reflect::get(&object, &JsValue::from_str(name)).ok())
        .is_some_and(|value| value.is_function())
}

fn atomic_store(control: &Int32Array, index: u32, value: i32) -> Result<(), String> {
    js_sys::Atomics::store(control, index, value)
        .map(|_| ())
        .map_err(|error| format!("failed to store worker result word {index}: {error:?}"))
}

fn atomic_load(control: &Int32Array, index: u32) -> Result<i32, String> {
    js_sys::Atomics::load(control, index)
        .map_err(|error| format!("failed to load worker result word {index}: {error:?}"))
}

fn atomic_notify(control: &Int32Array, index: u32) -> Result<(), String> {
    js_sys::Atomics::notify_with_count(control, index, 1)
        .map(|_| ())
        .map_err(|error| format!("failed to notify worker result word {index}: {error:?}"))
}

fn nonnegative_word(control: &Int32Array, index: u32, label: &str) -> Result<u32, String> {
    let value = atomic_load(control, index)?;
    u32::try_from(value).map_err(|_| format!("worker result {label} is negative ({value})"))
}

fn property(value: &JsValue, name: &str) -> Result<JsValue, String> {
    Reflect::get(value, &JsValue::from_str(name)).map_err(js_message)
}

fn string_property(value: &JsValue, name: &str) -> Result<String, String> {
    property(value, name)?
        .as_string()
        .ok_or_else(|| format!("worker property {name:?} is not a string"))
}

fn shared_buffer_property(value: &JsValue, name: &str) -> Result<SharedArrayBuffer, String> {
    property(value, name)?
        .dyn_into::<SharedArrayBuffer>()
        .map_err(|_| format!("worker property {name:?} is not a SharedArrayBuffer"))
}

fn uint8_array_property(value: &JsValue, name: &str) -> Result<Uint8Array, String> {
    property(value, name)?
        .dyn_into::<Uint8Array>()
        .map_err(|_| format!("worker property {name:?} is not a Uint8Array"))
}

fn method(value: &JsValue, name: &str) -> Result<Function, String> {
    property(value, name)?
        .dyn_into::<Function>()
        .map_err(|_| format!("worker transport property {name:?} is not callable"))
}

fn set_value(target: &Object, name: &str, value: &JsValue) -> Result<(), String> {
    Reflect::set(target, &JsValue::from_str(name), value)
        .map(|_| ())
        .map_err(js_message)
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
