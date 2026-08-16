use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::rc::Rc;

use js_sys::{
    Array, Atomics, Function, Int32Array, Object, Promise, Reflect, SharedArrayBuffer, Uint8Array,
};
use mclone_app_runtime::client_connection::{
    ClientConnectionDrainResult, ClientConnectionQueueMetrics, QueuedServerUpdate,
};
use mclone_app_runtime::host_mode::diagnostics_worker_exchange_drained;
use mclone_app_runtime::startup_args::parse_world_topology_arg;
use mclone_core::{ChunkPos, ChunkStatus, HorizontalTopology};
use mclone_protocol::{
    ChunkView, ClientCommand, ClientIdentity, DimensionKey, PlayerProfileId, ServerUpdate,
    decode_client_command, decode_server_update, encode_client_command, encode_server_update,
};
use mclone_server::{
    AuthoredWorldFixtureKind, ChunkLoadingProgressCell, ChunkLoadingProgressSnapshot,
    ChunkLoadingProgressStats, ChunkStoreError, ChunkStoreResult, DimensionRecord,
    INITIAL_DAY_TIME, IntegratedServerRunner, LOCAL_REALM_BOOTSTRAP_SAVED_DATA_KEYS,
    LightStatusMailboxKind, LocalRealmSession, ObserverSimulationInterest, PersistenceErrorKind,
    PersistenceExecutorFailureLatch, PersistenceRecordAddress, PersistenceRecordBatch,
    PersistenceRecordExecutor, PersistenceRecordKeyPart, PersistenceRecordMutation,
    PersistenceRecordNamespace, PersistenceRecordPayload, PersistenceRecordRequest,
    PersistenceRecordResponse, RecordExecutorWorldStore, ServerJobActor, ServerRunnerDiagnostics,
    ServerRunnerError, ServerRunnerKind, ServerRunnerResult, ServerRunnerTickDiagnostics,
    ServerUpdateEnvelope, WasmServerJobWorkerConfig, WorkerFrameMetrics, WorkerFrameTransportKind,
    WorldGenerationProfile, WorldMetadata, WorldStore, WorldStoreRequest, WorldgenMailboxKind,
    dimension_record_address, record_read_for_world_store_request, saved_data_record_address,
    world_metadata_record_address, world_store_completion_from_record_read,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

use crate::web_catalog_execution::web_world_writer_lease_name;
use crate::web_integrated_server_startup::WebIntegratedServerStartupConfig;

const WEB_WORKER_TICK_INTERVAL_MS: u32 = 50;
const WEB_WORKER_BACKGROUND_POLL_INTERVAL_MS: u32 = 8;
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
const RETIRED_WORKER_SHUTDOWN_TIMEOUT_MS: f64 = 30_000.0;
const RETIRED_WORKER_POLL_INTERVAL_MS: i32 = 10;

type RetiredWorkerOutcome = Rc<RefCell<Option<Result<(), String>>>>;

struct RetiredWorldWriter {
    token: u64,
    outcome: RetiredWorkerOutcome,
}

thread_local! {
    static RETIRED_WORLD_WRITERS: RefCell<BTreeMap<String, RetiredWorldWriter>> =
        const { RefCell::new(BTreeMap::new()) };
    static NEXT_RETIRED_WORLD_WRITER_TOKEN: Cell<u64> = const { Cell::new(1) };
}

pub(crate) async fn await_retired_world_writer(lease_name: &str) -> Result<(), String> {
    let outcome = RETIRED_WORLD_WRITERS.with(|retired| {
        retired
            .borrow()
            .get(lease_name)
            .map(|entry| Rc::clone(&entry.outcome))
    });
    let Some(outcome) = outcome else {
        return Ok(());
    };
    let deadline_ms = js_sys::Date::now() + RETIRED_WORKER_SHUTDOWN_TIMEOUT_MS;
    loop {
        if let Some(result) = outcome.borrow().clone() {
            return result.map_err(|error| {
                format!("previous browser world worker did not retire cleanly: {error}")
            });
        }
        if js_sys::Date::now() >= deadline_ms {
            return Err("timed out waiting for the previous browser world worker to retire".into());
        }
        wait_for_browser_delay(RETIRED_WORKER_POLL_INTERVAL_MS).await?;
    }
}

fn retain_worker_until_shutdown(
    worker: Worker,
    message_closure: Closure<dyn FnMut(MessageEvent)>,
    error_closure: Closure<dyn FnMut(ErrorEvent)>,
    lease_name: Option<String>,
    outcome: RetiredWorkerOutcome,
) {
    let token = NEXT_RETIRED_WORLD_WRITER_TOKEN.with(|next| {
        let token = next.get();
        next.set(token.wrapping_add(1).max(1));
        token
    });
    if let Some(lease_name) = lease_name.as_ref() {
        RETIRED_WORLD_WRITERS.with(|retired| {
            retired.borrow_mut().insert(
                lease_name.clone(),
                RetiredWorldWriter {
                    token,
                    outcome: Rc::clone(&outcome),
                },
            );
        });
    }
    wasm_bindgen_futures::spawn_local(async move {
        let deadline_ms = js_sys::Date::now() + RETIRED_WORKER_SHUTDOWN_TIMEOUT_MS;
        loop {
            if outcome.borrow().is_some() {
                break;
            }
            if js_sys::Date::now() >= deadline_ms {
                worker.terminate();
                *outcome.borrow_mut() = Some(Err(
                    "timed out while gracefully shutting down browser world worker".to_owned(),
                ));
                break;
            }
            if let Err(error) = wait_for_browser_delay(RETIRED_WORKER_POLL_INTERVAL_MS).await {
                worker.terminate();
                *outcome.borrow_mut() = Some(Err(error));
                break;
            }
        }
        worker.set_onmessage(None);
        worker.set_onerror(None);
        drop(message_closure);
        drop(error_closure);
        if let Some(lease_name) = lease_name {
            RETIRED_WORLD_WRITERS.with(|retired| {
                let mut retired = retired.borrow_mut();
                if retired
                    .get(&lease_name)
                    .is_some_and(|entry| entry.token == token)
                {
                    retired.remove(&lease_name);
                }
            });
        }
    });
}

async fn wait_for_browser_delay(delay_ms: i32) -> Result<(), String> {
    let promise = Promise::new(&mut |resolve: Function, reject: Function| {
        let Some(window) = web_sys::window() else {
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str("window is unavailable"));
            return;
        };
        let callback = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::NULL);
        });
        if let Err(error) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.unchecked_ref(),
            delay_ms,
        ) {
            let _ = reject.call1(&JsValue::NULL, &error);
        }
    });
    JsFuture::from(promise)
        .await
        .map(|_| ())
        .map_err(|error| js_error_string(&error))
}

fn web_dimension_definition(
    seed: i64,
    generation_profile: &str,
    world_topology: &str,
) -> Result<mclone_server::DimensionDefinition, String> {
    let generation_profile = WorldGenerationProfile::parse_label(generation_profile)?;
    let topology = parse_world_topology_arg("worldTopology", world_topology)
        .map_err(|error| error.to_string())?;
    let mut definition = mclone_server::DimensionDefinition::overworld(seed, generation_profile);
    definition.topology = topology;
    Ok(definition)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebIntegratedServerRunnerConfig {
    pub seed: i64,
    pub world_generation_profile: WorldGenerationProfile,
    pub starter_content: mclone_server::StarterContentDescriptor,
    pub world_topology: HorizontalTopology,
    pub world_behavior_profile: mclone_server::WorldBehaviorProfile,
    pub freeze_scheduled_fluid_ticks: bool,
    pub debug_passive_showcase: bool,
    pub debug_auxiliary_player_script: bool,
    pub light_status_batch_size: usize,
    pub worker_url: String,
    pub job_worker_url: String,
    pub bindgen_js_url: String,
    pub bindgen_wasm_url: String,
    pub world_storage: WebIntegratedServerWorldStorage,
    pub transient_authored_fixture: Option<AuthoredWorldFixtureKind>,
    pub transient_playable_showcase: Option<mclone_server::PlayableShowcaseId>,
    pub day_time: Option<u64>,
    pub day_time_frozen: bool,
    pub runner_transport_kind: Option<WorkerFrameTransportKind>,
    pub runner_initial_inbound_bytes: u32,
    pub local_player_identity: ClientIdentity,
    pub observer_only: bool,
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
            world_generation_profile: WorldGenerationProfile::default(),
            starter_content: mclone_server::StarterContentDescriptor::Wild,
            world_topology: HorizontalTopology::UNBOUNDED,
            world_behavior_profile: mclone_server::WorldBehaviorProfile::default(),
            freeze_scheduled_fluid_ticks: false,
            debug_passive_showcase: true,
            debug_auxiliary_player_script: false,
            light_status_batch_size: mclone_server::DEFAULT_LIGHT_STATUS_BATCH_SIZE,
            worker_url: worker_url.into(),
            job_worker_url: job_worker_url.into(),
            bindgen_js_url: bindgen_js_url.into(),
            bindgen_wasm_url: bindgen_wasm_url.into(),
            world_storage: WebIntegratedServerWorldStorage::Transient,
            transient_authored_fixture: None,
            transient_playable_showcase: None,
            day_time: None,
            day_time_frozen: false,
            runner_transport_kind: None,
            runner_initial_inbound_bytes: DEFAULT_RUNNER_SHARED_RESPONSE_BYTES,
            local_player_identity: ClientIdentity::test_default(),
            observer_only: false,
        }
    }

    pub const fn with_observer_only(mut self, observer_only: bool) -> Self {
        self.observer_only = observer_only;
        self
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

    pub fn with_transient_authored_fixture(mut self, fixture: AuthoredWorldFixtureKind) -> Self {
        self.world_storage = WebIntegratedServerWorldStorage::Transient;
        self.transient_authored_fixture = Some(fixture);
        self
    }

    pub fn with_transient_playable_showcase(
        mut self,
        showcase: mclone_server::PlayableShowcaseId,
    ) -> Self {
        self.world_storage = WebIntegratedServerWorldStorage::Transient;
        self.transient_authored_fixture = None;
        self.transient_playable_showcase = Some(showcase);
        self
    }

    pub const fn with_day_time(mut self, day_time: Option<u64>) -> Self {
        self.day_time = day_time;
        self
    }

    pub const fn with_day_time_frozen(mut self, frozen: bool) -> Self {
        self.day_time_frozen = frozen;
        self
    }

    pub const fn with_world_generation_profile(mut self, profile: WorldGenerationProfile) -> Self {
        self.world_generation_profile = profile;
        self
    }

    pub const fn with_starter_content(
        mut self,
        starter_content: mclone_server::StarterContentDescriptor,
    ) -> Self {
        self.starter_content = starter_content;
        self
    }

    pub const fn with_world_topology(mut self, topology: HorizontalTopology) -> Self {
        self.world_topology = topology;
        self
    }

    pub const fn with_world_behavior_profile(
        mut self,
        profile: mclone_server::WorldBehaviorProfile,
    ) -> Self {
        self.world_behavior_profile = profile;
        self
    }

    pub const fn with_freeze_scheduled_fluid_ticks(mut self, freeze: bool) -> Self {
        self.freeze_scheduled_fluid_ticks = freeze;
        self
    }

    pub const fn with_debug_passive_showcase(mut self, enabled: bool) -> Self {
        self.debug_passive_showcase = enabled;
        self
    }

    pub const fn with_debug_auxiliary_player_script(mut self, enabled: bool) -> Self {
        self.debug_auxiliary_player_script = enabled;
        self
    }

    pub const fn with_light_status_batch_size(mut self, batch_size: usize) -> Self {
        self.light_status_batch_size = if batch_size == 0 { 1 } else { batch_size };
        self
    }

    pub const fn with_runner_transport_kind(
        mut self,
        runner_transport_kind: WorkerFrameTransportKind,
    ) -> Self {
        self.runner_transport_kind = Some(runner_transport_kind);
        self
    }

    pub const fn with_runner_initial_inbound_bytes(
        mut self,
        runner_initial_inbound_bytes: u32,
    ) -> Self {
        self.runner_initial_inbound_bytes = runner_initial_inbound_bytes;
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
    outstanding_command_request_ids: Rc<RefCell<BTreeSet<u32>>>,
    runner_frame_metrics: Rc<RefCell<WorkerFrameMetrics>>,
    shared_pool: Rc<RefCell<Vec<RunnerSharedSlot>>>,
    shared_inflight: Rc<RefCell<BTreeMap<u32, RunnerSharedSlot>>>,
    runner_initial_inbound_bytes: u32,
    update_frames: Rc<RefCell<Vec<Vec<u8>>>>,
    diagnostics: Rc<RefCell<ServerRunnerDiagnostics>>,
    message_closure: Option<Closure<dyn FnMut(MessageEvent)>>,
    error_closure: Option<Closure<dyn FnMut(ErrorEvent)>>,
    world_writer_lease_name: Option<String>,
    shutdown_outcome: RetiredWorkerOutcome,
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
        let mut config = config;
        config.local_player_identity =
            mclone_app_runtime::local_profile::load_or_create_web_local_player_profile()
                .map_err(|error| format!("load browser player profile: {error:#}"))?
                .client_identity();
        let startup_world_writer_lease_name = match &config.world_storage {
            WebIntegratedServerWorldStorage::Transient => None,
            WebIntegratedServerWorldStorage::IndexedDb { world_id, .. } => {
                Some(web_world_writer_lease_name(world_id))
            }
        };
        if let Some(lease_name) = startup_world_writer_lease_name.as_deref() {
            await_retired_world_writer(lease_name).await?;
        }
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
        let runner_initial_inbound_bytes = config
            .runner_initial_inbound_bytes
            .max(RUNNER_SHARED_CONTROL_BYTES);
        let pending = Rc::new(RefCell::new(BTreeMap::new()));
        let request_start_ms_by_id: Rc<RefCell<BTreeMap<u32, f64>>> =
            Rc::new(RefCell::new(BTreeMap::new()));
        let outstanding_command_request_ids = Rc::new(RefCell::new(BTreeSet::new()));
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
        let shutdown_outcome: RetiredWorkerOutcome = Rc::new(RefCell::new(None));

        let message_closure = {
            let worker = worker.clone();
            let pending = Rc::clone(&pending);
            let request_start_ms_by_id = Rc::clone(&request_start_ms_by_id);
            let outstanding_command_request_ids = Rc::clone(&outstanding_command_request_ids);
            let runner_frame_metrics = Rc::clone(&runner_frame_metrics);
            let shared_pool = Rc::clone(&shared_pool);
            let shared_inflight = Rc::clone(&shared_inflight);
            let update_frames = Rc::clone(&update_frames);
            let diagnostics = Rc::clone(&diagnostics);
            let shutdown_outcome = Rc::clone(&shutdown_outcome);
            Closure::wrap(Box::new(move |event: MessageEvent| {
                handle_runner_message(
                    event.data(),
                    &worker,
                    &pending,
                    &request_start_ms_by_id,
                    &outstanding_command_request_ids,
                    &runner_frame_metrics,
                    &shared_pool,
                    &shared_inflight,
                    &update_frames,
                    &diagnostics,
                    &shutdown_outcome,
                );
            }) as Box<dyn FnMut(_)>)
        };
        worker.set_onmessage(Some(message_closure.as_ref().unchecked_ref()));

        let error_closure = {
            let worker = worker.clone();
            let pending = Rc::clone(&pending);
            let diagnostics = Rc::clone(&diagnostics);
            let outstanding_command_request_ids = Rc::clone(&outstanding_command_request_ids);
            let shutdown_outcome = Rc::clone(&shutdown_outcome);
            Closure::wrap(Box::new(move |event: ErrorEvent| {
                let message = if event.message().is_empty() {
                    "integrated server worker failed".to_owned()
                } else {
                    event.message()
                };
                diagnostics.borrow_mut().last_error = Some(message.clone());
                outstanding_command_request_ids.borrow_mut().clear();
                reject_all_pending(&pending, &message);
                worker.terminate();
                *shutdown_outcome.borrow_mut() = Some(Err(message));
            }) as Box<dyn FnMut(_)>)
        };
        worker.set_onerror(Some(error_closure.as_ref().unchecked_ref()));

        let mut runner = Self {
            worker,
            transport_kind,
            next_request_id: 1,
            pending,
            request_start_ms_by_id,
            outstanding_command_request_ids,
            runner_frame_metrics,
            shared_pool,
            shared_inflight,
            runner_initial_inbound_bytes,
            update_frames,
            diagnostics,
            message_closure: Some(message_closure),
            error_closure: Some(error_closure),
            world_writer_lease_name: None,
            shutdown_outcome,
            shutdown_requested: false,
        };
        if let Err(error) = runner.start(config).await {
            runner.worker.terminate();
            *runner.shutdown_outcome.borrow_mut() = Some(Err(error.clone()));
            return Err(error);
        }
        runner.world_writer_lease_name = startup_world_writer_lease_name;
        Ok(runner)
    }

    pub const fn kind(&self) -> ServerRunnerKind {
        ServerRunnerKind::WebWorker
    }

    pub fn diagnostics(&self) -> ServerRunnerDiagnostics {
        let mut diagnostics = self.diagnostics.borrow().clone();
        diagnostics.command_queue_depth = self.outstanding_command_request_ids.borrow().len();
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

    /// Compatibility/probe helper for tests and browser smoke diagnostics.
    ///
    /// Normal web runtime frames call `queue_command` and
    /// `drain_next_queued_update` through `WebRuntimeHost`'s
    /// `ClientConnection` implementation instead of command exchange.
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
        if self.post_shutdown_with_request_id(0).is_err() {
            self.worker.terminate();
            *self.shutdown_outcome.borrow_mut() = Some(Err(
                "failed to post shutdown to integrated server worker".to_owned(),
            ));
        }
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
        let startup_frame = WebIntegratedServerStartupConfig {
            seed: config.seed,
            world_generation_profile: config.world_generation_profile,
            starter_content: config.starter_content,
            world_topology: config.world_topology,
            world_behavior_profile: config.world_behavior_profile,
            transient_authored_fixture: config.transient_authored_fixture,
            transient_playable_showcase: config.transient_playable_showcase,
            day_time: config.day_time,
            day_time_frozen: config.day_time_frozen,
            freeze_scheduled_fluid_ticks: config.freeze_scheduled_fluid_ticks,
            debug_passive_showcase: config.debug_passive_showcase,
            debug_auxiliary_player_script: config.debug_auxiliary_player_script,
            light_status_batch_size: config.light_status_batch_size,
            local_player_identity: config.local_player_identity.clone(),
            observer_only: config.observer_only,
        }
        .encode()?;
        let startup_frame = Uint8Array::from(startup_frame.as_slice());
        Reflect::set(&message, &JsValue::from_str("startupFrame"), &startup_frame).map_err(
            |error| format!("failed to attach integrated-server startup frame: {error:?}"),
        )?;
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
        set_number(
            &message,
            "backgroundPollIntervalMs",
            f64::from(WEB_WORKER_BACKGROUND_POLL_INTERVAL_MS),
        )?;
        let transfer = Array::new();
        transfer.push(&startup_frame.buffer());
        let response = self
            .post_request(request_id, &message, Some(&transfer))
            .await?;
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
        self.record_command_request(request_id, frame.len());
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
        self.record_command_request(request_id, frame.len());
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
        self.record_command_request(request_id, frame.len());
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
        self.record_command_request(request_id, frame.len());
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

    fn post_flush_persistence(&self) -> Result<(), String> {
        let message = Object::new();
        set_string(&message, "kind", "flush-persistence")?;
        set_number(&message, "requestId", 0.0)?;
        self.worker.post_message(&message).map_err(|error| {
            format!("failed to post persistence flush to server worker: {error:?}")
        })
    }

    fn post_promote_observer(&mut self) -> Result<(), String> {
        let request_id = self.next_request_id();
        let message = Object::new();
        set_string(&message, "kind", "promote-observer")?;
        set_number(&message, "requestId", f64::from(request_id))?;
        self.worker
            .post_message(&message)
            .map_err(|error| format!("failed to post observer promotion: {error:?}"))?;
        self.record_runner_request(request_id, 0);
        Ok(())
    }

    fn post_demote_player(&mut self) -> Result<(), String> {
        let request_id = self.next_request_id();
        let message = Object::new();
        set_string(&message, "kind", "demote-player")?;
        set_number(&message, "requestId", f64::from(request_id))?;
        self.worker
            .post_message(&message)
            .map_err(|error| format!("failed to post player demotion: {error:?}"))?;
        self.record_runner_request(request_id, 0);
        Ok(())
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
                    self.runner_initial_inbound_bytes,
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

    fn record_command_request(&self, request_id: u32, bytes: usize) {
        self.outstanding_command_request_ids
            .borrow_mut()
            .insert(request_id);
        self.record_runner_request(request_id, bytes);
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
        let Some(message_closure) = self.message_closure.take() else {
            return;
        };
        let Some(error_closure) = self.error_closure.take() else {
            self.worker.set_onmessage(None);
            return;
        };
        retain_worker_until_shutdown(
            self.worker.clone(),
            message_closure,
            error_closure,
            self.world_writer_lease_name.clone(),
            Rc::clone(&self.shutdown_outcome),
        );
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

    // Compatibility surface for the generic integrated-runner trait. The web
    // runtime's normal frame path drains through WebRuntimeHost's
    // ClientConnection implementation.
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

    // Unlimited compatibility drain for startup/probe callers, not the normal
    // web frame ingress path.
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

    fn flush_persistence(&mut self) -> ServerRunnerResult<usize> {
        if self.shutdown_requested {
            return Err(ServerRunnerError::CommandChannelClosed);
        }
        self.post_flush_persistence()
            .map_err(ServerRunnerError::ThreadStart)?;
        Ok(0)
    }

    fn promote_observer_to_player(&mut self) -> ServerRunnerResult<()> {
        if self.shutdown_requested {
            return Err(ServerRunnerError::CommandChannelClosed);
        }
        self.post_promote_observer()
            .map_err(ServerRunnerError::ThreadStart)
    }

    fn demote_player_to_observer(&mut self) -> ServerRunnerResult<()> {
        if self.shutdown_requested {
            return Err(ServerRunnerError::CommandChannelClosed);
        }
        self.post_demote_player()
            .map_err(ServerRunnerError::ThreadStart)
    }

    fn request_shutdown(&mut self) {
        Self::request_shutdown(self);
    }

    fn join_shutdown(&mut self) -> ServerRunnerResult<()> {
        self.request_shutdown();
        Ok(())
    }
}

/// Browser binding for the Rust-owned server-job actor. Main Rust supplies an
/// opaque initialization frame; this worker-side Rust object decodes its lane,
/// owns the resident worldgen mirror when applicable, and dispatches all later
/// opaque request frames. The TypeScript Worker broker never selects an
/// algorithm or constructs a domain session.
#[wasm_bindgen]
pub struct WebServerJobActor {
    actor: ServerJobActor,
}

#[wasm_bindgen]
impl WebServerJobActor {
    #[wasm_bindgen(constructor)]
    pub fn new(init_frame: Uint8Array) -> Result<Self, JsValue> {
        let actor = ServerJobActor::from_init_frame(&init_frame.to_vec()).map_err(JsValue::from)?;
        Ok(Self { actor })
    }

    #[wasm_bindgen(js_name = computeFrame)]
    pub fn compute_frame(&mut self, frame: Uint8Array) -> Result<Uint8Array, JsValue> {
        let response = self
            .actor
            .compute_frame(&frame.to_vec())
            .map_err(JsValue::from)?;
        Ok(Uint8Array::from(response.as_slice()))
    }

    #[wasm_bindgen(js_name = diagnosticsFrame)]
    pub fn diagnostics_frame(&self) -> Uint8Array {
        let frame = self.actor.diagnostics_frame();
        Uint8Array::from(frame.as_slice())
    }

    pub fn shutdown(&mut self) {
        self.actor.shutdown();
    }
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
        .with_runner_initial_inbound_bytes(4 * 1024),
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
                && metrics.shared_buffer_pooled_inbound_frames > 0
                && runner_diagnostics_settled(diagnostics)
        },
        "shared runner pool overflow",
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
                && metrics.shared_buffer_pooled_inbound_frames > 0
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
            .shared_buffer_pooled_inbound_frames
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
        && diagnostics.runner_frame_metrics.inbound_frames > 0
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
    let mut last_diagnostics = runner.diagnostics();
    for _ in 0..2400 {
        *update_count = update_count.saturating_add(runner.drain_decoded_updates()?.len());
        let diagnostics = runner.diagnostics();
        if condition(&diagnostics) {
            return Ok(diagnostics);
        }
        last_diagnostics = diagnostics;
        *poll_count = poll_count.saturating_add(1);
        wait_for_browser_turn().await?;
    }
    Err(format!(
        "timed out waiting for {label}: {last_diagnostics:?}"
    ))
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
        && metrics.inbound_frames > 0
        && metrics.inbound_bytes > 0
}

fn frame_metrics_transfer_worker_active(metrics: WorkerFrameMetrics) -> bool {
    metrics.transport_kind == WorkerFrameTransportKind::MessageTransfer
        && metrics.request_frames > 0
        && metrics.request_bytes > 0
        && metrics.inbound_frames > 0
        && metrics.inbound_bytes > 0
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
    outstanding_command_request_ids: &Rc<RefCell<BTreeSet<u32>>>,
    runner_frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
    shared_pool: &Rc<RefCell<Vec<RunnerSharedSlot>>>,
    shared_inflight: &Rc<RefCell<BTreeMap<u32, RunnerSharedSlot>>>,
    update_frames: &Rc<RefCell<Vec<Vec<u8>>>>,
    diagnostics: &Rc<RefCell<ServerRunnerDiagnostics>>,
    shutdown_outcome: &RetiredWorkerOutcome,
) {
    let request_id = number_prop(&data, "requestId")
        .map(|value| value as u32)
        .unwrap_or(0);
    if request_id != 0 {
        outstanding_command_request_ids
            .borrow_mut()
            .remove(&request_id);
    }
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
        match shared_runner_inbound_frames(&data) {
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
                    metrics.record_inbound(frame.len());
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
        *shutdown_outcome.borrow_mut() = Some(Ok(()));
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
    diagnostics.persistence_queue_metrics.foreground_requests =
        number_prop(value, "persistenceForegroundRequests").unwrap_or(0.0) as usize;
    diagnostics.persistence_queue_metrics.durable_write_requests =
        number_prop(value, "persistenceDurableWriteRequests").unwrap_or(0.0) as usize;
    diagnostics.persistence_queue_metrics.durable_write_bytes =
        number_prop(value, "persistenceDurableWriteBytes").unwrap_or(0.0) as usize;
    diagnostics.persistence_queue_metrics.cache_write_requests =
        number_prop(value, "persistenceCacheWriteRequests").unwrap_or(0.0) as usize;
    diagnostics.persistence_queue_metrics.cache_write_bytes =
        number_prop(value, "persistenceCacheWriteBytes").unwrap_or(0.0) as usize;
    diagnostics
        .persistence_queue_metrics
        .retained_record_cache_entries =
        number_prop(value, "persistenceRetainedRecordCacheEntries").unwrap_or(0.0) as usize;
    diagnostics
        .persistence_queue_metrics
        .retained_record_cache_bytes =
        number_prop(value, "persistenceRetainedRecordCacheBytes").unwrap_or(0.0) as usize;
    diagnostics
        .persistence_queue_metrics
        .high_water_foreground_requests =
        number_prop(value, "persistenceHighWaterForegroundRequests").unwrap_or(0.0) as usize;
    diagnostics
        .persistence_queue_metrics
        .high_water_durable_write_requests =
        number_prop(value, "persistenceHighWaterDurableWriteRequests").unwrap_or(0.0) as usize;
    diagnostics
        .persistence_queue_metrics
        .high_water_durable_write_bytes =
        number_prop(value, "persistenceHighWaterDurableWriteBytes").unwrap_or(0.0) as usize;
    diagnostics
        .persistence_queue_metrics
        .high_water_cache_write_requests =
        number_prop(value, "persistenceHighWaterCacheWriteRequests").unwrap_or(0.0) as usize;
    diagnostics
        .persistence_queue_metrics
        .high_water_cache_write_bytes =
        number_prop(value, "persistenceHighWaterCacheWriteBytes").unwrap_or(0.0) as usize;
    diagnostics.persistence_queue_metrics.cancelled_requests =
        number_prop(value, "persistenceCancelledRequests").unwrap_or(0.0) as u64;
    diagnostics.persistence_queue_metrics.skipped_cache_writes =
        number_prop(value, "persistenceSkippedCacheWrites").unwrap_or(0.0) as u64;
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
    diagnostics.scheduler_metrics.client_visible_chunks =
        number_prop(value, "schedulerClientVisibleChunks").unwrap_or(0.0) as usize;
    diagnostics.scheduler_metrics.loaded_snapshot_chunks =
        number_prop(value, "schedulerLoadedSnapshotChunks").unwrap_or(0.0) as usize;
    diagnostics.scheduler_metrics.active_ticket_chunks =
        number_prop(value, "schedulerActiveTicketChunks").unwrap_or(0.0) as usize;
    diagnostics.chunk_tracking.total_player_visible_chunks =
        number_prop(value, "trackingPlayerVisibleChunks").unwrap_or(0.0) as usize;
    diagnostics.chunk_tracking.total_player_published_chunks =
        number_prop(value, "trackingPlayerPublishedChunks").unwrap_or(0.0) as usize;
    diagnostics
        .chunk_tracking
        .total_player_published_visible_chunks =
        number_prop(value, "trackingPlayerPublishedVisibleChunks").unwrap_or(0.0) as usize;
    diagnostics
        .chunk_tracking
        .total_player_missing_published_chunks =
        number_prop(value, "trackingPlayerMissingPublishedChunks").unwrap_or(0.0) as usize;
    diagnostics
        .chunk_tracking
        .total_player_published_outside_visible_chunks =
        number_prop(value, "trackingPlayerPublishedOutsideVisibleChunks").unwrap_or(0.0) as usize;
    diagnostics
        .chunk_tracking
        .total_player_queued_snapshot_updates =
        number_prop(value, "trackingPlayerQueuedSnapshotUpdates").unwrap_or(0.0) as u64;
    diagnostics
        .chunk_tracking
        .total_player_queued_unload_updates =
        number_prop(value, "trackingPlayerQueuedUnloadUpdates").unwrap_or(0.0) as u64;
    diagnostics
        .chunk_tracking
        .total_player_drained_snapshot_updates =
        number_prop(value, "trackingPlayerDrainedSnapshotUpdates").unwrap_or(0.0) as u64;
    diagnostics
        .chunk_tracking
        .total_player_drained_unload_updates =
        number_prop(value, "trackingPlayerDrainedUnloadUpdates").unwrap_or(0.0) as u64;
    diagnostics.chunk_tracking.aggregate_player_ticket_chunks =
        number_prop(value, "trackingAggregatePlayerTicketChunks").unwrap_or(0.0) as usize;
    diagnostics.chunk_tracking.total_outbound_queue_depth =
        number_prop(value, "trackingPlayerOutboundQueueDepth").unwrap_or(0.0) as usize;
    diagnostics.runner_emitted_snapshot_updates =
        number_prop(value, "runnerEmittedSnapshotUpdates").unwrap_or(0.0) as u64;
    diagnostics.runner_emitted_unload_updates =
        number_prop(value, "runnerEmittedUnloadUpdates").unwrap_or(0.0) as u64;
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
    diagnostics.loading_progress_snapshot = reflect_get(value, "loadingProgressSnapshot")
        .and_then(|value| loading_progress_snapshot_from_js(&value));
    diagnostics.view_readiness_snapshot = reflect_get(value, "viewReadinessSnapshot")
        .and_then(|value| loading_progress_snapshot_from_js(&value));
    diagnostics.accepted_local_chunk_view =
        if bool_prop(value, "acceptedLocalViewAvailable").unwrap_or(false) {
            Some(ChunkView {
                center: ChunkPos::new(
                    number_prop(value, "acceptedLocalCenterX")? as i32,
                    number_prop(value, "acceptedLocalCenterZ")? as i32,
                ),
                render_distance: number_prop(value, "acceptedLocalRenderDistance")? as u32,
                chunk_tracking_radius: number_prop(value, "acceptedLocalTrackingRadius")? as u32,
            })
        } else {
            None
        };
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

fn shared_runner_inbound_frames(value: &JsValue) -> Result<SharedRunnerResponse, String> {
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
    let inbound_bytes =
        Atomics::load(&control, RUNNER_SHARED_RESPONSE_BYTES_INDEX).map_err(|error| {
            format!(
                "failed to read shared runner response byte count: {}",
                js_error_string(&error)
            )
        })?;
    if inbound_bytes < 0 {
        return Err(format!(
            "shared runner response returned negative byte count {inbound_bytes}"
        ));
    }
    let Some(update_buffer) = reflect_get(value, "updateBuffer") else {
        return Err("shared runner response returned no update buffer".to_owned());
    };
    if !update_buffer.is_instance_of::<SharedArrayBuffer>() {
        return Err("shared runner update buffer was not a SharedArrayBuffer".to_owned());
    }
    let inbound_bytes = inbound_bytes as u32;
    let packed = Uint8Array::new_with_byte_offset_and_length(&update_buffer, 0, inbound_bytes);
    Ok(SharedRunnerResponse {
        frames: unpack_runner_update_frames(&packed.to_vec())?,
        packed_bytes: inbound_bytes as usize,
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
    server: LocalRealmSession,
    diagnostics: ServerRunnerDiagnostics,
    indexed_db_state: Option<Rc<RefCell<WebPersistenceRecordState>>>,
    pending_indexed_db_reads: BTreeMap<u64, WorldStoreRequest>,
    command_queue_depth: usize,
    running: bool,
}

/// Decoded, versioned authority configuration for one browser integrated
/// server. TypeScript retains browser storage and cadence mechanics, but never
/// projects these domain fields or chooses their defaults.
#[wasm_bindgen]
pub struct WebIntegratedServerStartup {
    config: WebIntegratedServerStartupConfig,
}

/// Resident domain actor for one browser integrated-server Worker.
///
/// Browser code retains event-loop serialization, IndexedDB transactions and
/// external SAB views. This actor admits one domain operation at a time,
/// selects the authoritative session transition, and authors
/// completion/failure envelopes.
#[wasm_bindgen]
pub struct WebIntegratedServerActor {
    server: McloneWebIntegratedServerWorker,
    operation: Option<WebIntegratedServerOperation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WebIntegratedServerOperationKind {
    Command,
    Tick,
    Poll,
    PersistenceCompletion,
    FlushPersistence,
    PromoteObserver,
    DemotePlayer,
    Shutdown,
}

impl WebIntegratedServerOperationKind {
    const fn response_kind(self) -> &'static str {
        match self {
            Self::Command => "command-result",
            Self::Tick | Self::Poll | Self::PersistenceCompletion => "updates",
            Self::FlushPersistence => "flush-complete",
            Self::PromoteObserver => "observer-promoted",
            Self::DemotePlayer => "player-demoted",
            Self::Shutdown => "shutdown-complete",
        }
    }

    const fn posts_empty_response(self) -> bool {
        !matches!(self, Self::Tick | Self::Poll | Self::PersistenceCompletion)
    }

    const fn closes_worker(self) -> bool {
        matches!(self, Self::Shutdown)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WebIntegratedServerOperation {
    kind: WebIntegratedServerOperationKind,
    request_id: u32,
}

const WEB_EXECUTOR_REQUEST_ID_BASE: u64 = 1_u64 << 63;
const WEB_BOOTSTRAP_SAVED_DATA_REQUEST_ID_BASE: u64 = 3;
const WEB_BOOTSTRAP_PROBE_REQUEST_ID: u64 =
    WEB_BOOTSTRAP_SAVED_DATA_REQUEST_ID_BASE + LOCAL_REALM_BOOTSTRAP_SAVED_DATA_KEYS.len() as u64;

#[derive(Debug)]
struct WebPersistenceRecordState {
    records: BTreeMap<PersistenceRecordAddress, Option<PersistenceRecordPayload>>,
    legacy_records_present: bool,
    outgoing: VecDeque<PersistenceRecordRequest>,
    pending_executor_requests: BTreeSet<u64>,
    next_request_id: u64,
    failure_latch: PersistenceExecutorFailureLatch,
    closed: bool,
}

impl WebPersistenceRecordState {
    fn new(
        records: BTreeMap<PersistenceRecordAddress, Option<PersistenceRecordPayload>>,
        legacy_records_present: bool,
    ) -> Self {
        Self {
            records,
            legacy_records_present,
            outgoing: VecDeque::new(),
            pending_executor_requests: BTreeSet::new(),
            next_request_id: WEB_EXECUTOR_REQUEST_ID_BASE,
            failure_latch: PersistenceExecutorFailureLatch::default(),
            closed: false,
        }
    }

    fn check_healthy(&self) -> ChunkStoreResult<()> {
        self.failure_latch.check_healthy()?;
        if self.closed {
            return Err(ChunkStoreError::classified(
                PersistenceErrorKind::Closed,
                "browser persistence record executor is closed",
            ));
        }
        Ok(())
    }

    fn queue_executor_request(&mut self, build: impl FnOnce(u64) -> PersistenceRecordRequest) {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.pending_executor_requests.insert(request_id);
        self.outgoing.push_back(build(request_id));
    }

    fn apply_external_read(
        &mut self,
        address: PersistenceRecordAddress,
        result: &ChunkStoreResult<Option<PersistenceRecordPayload>>,
    ) {
        if let Ok(payload) = result {
            self.records.insert(address, payload.clone());
        }
    }

    fn complete_executor_response(
        &mut self,
        response: &PersistenceRecordResponse,
    ) -> ChunkStoreResult<()> {
        let request_id = response.request_id();
        if !self.pending_executor_requests.remove(&request_id) {
            return Err(ChunkStoreError::InvalidData(format!(
                "browser record executor completion {request_id} was not pending"
            )));
        }
        let result = match response {
            PersistenceRecordResponse::Read { result, .. } => result.as_ref().map(|_| ()),
            PersistenceRecordResponse::ProbeAny { result, .. } => result.as_ref().map(|_| ()),
            PersistenceRecordResponse::Commit { result, .. }
            | PersistenceRecordResponse::Flush { result, .. }
            | PersistenceRecordResponse::Close { result, .. } => result.as_ref().map(|_| ()),
        };
        if let Err(error) = result {
            return Err(self.failure_latch.poison(error));
        }
        Ok(())
    }

    fn drain_outgoing(&mut self) -> Vec<PersistenceRecordRequest> {
        self.outgoing.drain(..).collect()
    }
}

#[derive(Clone, Debug)]
struct WebPersistenceRecordExecutor {
    state: Rc<RefCell<WebPersistenceRecordState>>,
}

impl WebPersistenceRecordExecutor {
    fn new(state: Rc<RefCell<WebPersistenceRecordState>>) -> Self {
        Self { state }
    }
}

impl PersistenceRecordExecutor for WebPersistenceRecordExecutor {
    fn read(
        &mut self,
        address: &PersistenceRecordAddress,
    ) -> ChunkStoreResult<Option<PersistenceRecordPayload>> {
        let state = self.state.borrow();
        state.check_healthy()?;
        Ok(state.records.get(address).cloned().flatten())
    }

    fn probe_any(&mut self, namespaces: &[PersistenceRecordNamespace]) -> ChunkStoreResult<bool> {
        let state = self.state.borrow();
        state.check_healthy()?;
        Ok(state.legacy_records_present
            || state.records.iter().any(|(address, payload)| {
                payload.is_some() && namespaces.contains(&address.namespace)
            }))
    }

    fn commit(&mut self, batch: &PersistenceRecordBatch) -> ChunkStoreResult<()> {
        let mut state = self.state.borrow_mut();
        state.check_healthy()?;
        for mutation in &batch.mutations {
            let address = match mutation {
                PersistenceRecordMutation::Put { address, .. }
                | PersistenceRecordMutation::Delete { address } => address,
            };
            if !state.records.contains_key(address) {
                return Err(ChunkStoreError::classified(
                    PersistenceErrorKind::Unavailable,
                    format!(
                        "browser record {address:?} must be read before revision-safe mutation"
                    ),
                ));
            }
        }
        for mutation in &batch.mutations {
            match mutation {
                PersistenceRecordMutation::Put { address, payload } => {
                    state.records.insert(address.clone(), Some(payload.clone()));
                }
                PersistenceRecordMutation::Delete { address } => {
                    state.records.insert(address.clone(), None);
                }
            }
        }
        let batch = batch.clone();
        state.queue_executor_request(|request_id| PersistenceRecordRequest::Commit {
            request_id,
            batch,
        });
        Ok(())
    }

    fn release_cached_record(&mut self, address: &PersistenceRecordAddress) {
        self.state.borrow_mut().records.remove(address);
    }

    fn flush(&mut self) -> ChunkStoreResult<()> {
        let mut state = self.state.borrow_mut();
        state.check_healthy()?;
        state.queue_executor_request(|request_id| PersistenceRecordRequest::Flush { request_id });
        Ok(())
    }

    fn close(&mut self) -> ChunkStoreResult<()> {
        let mut state = self.state.borrow_mut();
        state.check_healthy()?;
        state.queue_executor_request(|request_id| PersistenceRecordRequest::Close { request_id });
        state.closed = true;
        Ok(())
    }
}

fn web_persistence_state_from_bootstrap(
    responses: Vec<PersistenceRecordResponse>,
) -> ChunkStoreResult<(
    Rc<RefCell<WebPersistenceRecordState>>,
    Option<WorldMetadata>,
    Option<DimensionRecord>,
)> {
    let mut records = BTreeMap::new();
    let mut legacy_records_present = None;
    for response in responses {
        match response {
            PersistenceRecordResponse::Read {
                request_id,
                address,
                result,
            } if request_id == 1
                || request_id == 2
                || (WEB_BOOTSTRAP_SAVED_DATA_REQUEST_ID_BASE..WEB_BOOTSTRAP_PROBE_REQUEST_ID)
                    .contains(&request_id) =>
            {
                records.insert(address, result?);
            }
            PersistenceRecordResponse::ProbeAny {
                request_id: WEB_BOOTSTRAP_PROBE_REQUEST_ID,
                result,
            } => {
                legacy_records_present = Some(result?);
            }
            other => {
                return Err(ChunkStoreError::InvalidData(format!(
                    "unexpected IndexedDB bootstrap completion {other:?}"
                )));
            }
        }
    }
    for address in [
        world_metadata_record_address(),
        dimension_record_address(&DimensionKey::overworld()),
    ]
    .into_iter()
    .chain(
        LOCAL_REALM_BOOTSTRAP_SAVED_DATA_KEYS
            .iter()
            .map(|key| saved_data_record_address(key)),
    ) {
        if !records.contains_key(&address) {
            return Err(ChunkStoreError::InvalidData(format!(
                "IndexedDB bootstrap omitted record address {address:?}"
            )));
        }
    }
    let legacy_records_present = legacy_records_present.ok_or_else(|| {
        ChunkStoreError::InvalidData("IndexedDB bootstrap omitted record probe".to_owned())
    })?;
    let state = Rc::new(RefCell::new(WebPersistenceRecordState::new(
        records,
        legacy_records_present,
    )));
    let mut store =
        RecordExecutorWorldStore::new(WebPersistenceRecordExecutor::new(Rc::clone(&state)), true);
    let metadata = store.load_world_metadata()?.record;
    let dimension = store.load_dimension(&DimensionKey::overworld())?;
    Ok((state, metadata, dimension))
}

fn web_dimension_definition_from_startup(
    config: &WebIntegratedServerStartupConfig,
) -> mclone_server::DimensionDefinition {
    let mut definition =
        mclone_server::DimensionDefinition::overworld(config.seed, config.world_generation_profile);
    definition.topology = config.world_topology;
    definition
}

fn web_dimension_definition_for_startup_state(
    config: &WebIntegratedServerStartupConfig,
    dimension: Option<&DimensionRecord>,
) -> mclone_server::DimensionDefinition {
    dimension.map_or_else(
        || web_dimension_definition_from_startup(config),
        |record| record.definition.clone(),
    )
}

fn apply_stored_world_metadata_profiles(
    server: &mut LocalRealmSession,
    metadata: Option<&WorldMetadata>,
) -> Result<(), String> {
    let Some(metadata) = metadata else {
        return Ok(());
    };
    server
        .set_world_generation_profile(metadata.world_generation_profile)
        .map_err(|error| error.to_string())?;
    server.set_world_behavior_profile(metadata.world_behavior_profile);
    server.set_starter_content(metadata.starter_content);
    Ok(())
}

#[wasm_bindgen]
impl WebIntegratedServerStartup {
    #[wasm_bindgen(constructor)]
    pub fn new(frame: Uint8Array) -> Result<Self, JsValue> {
        let config = WebIntegratedServerStartupConfig::decode(&frame.to_vec())
            .map_err(|error| JsValue::from_str(&error))?;
        Ok(Self { config })
    }

    #[wasm_bindgen(js_name = indexedDbBootstrapRequests)]
    pub fn indexed_db_bootstrap_requests(&self) -> Result<Array, JsValue> {
        let mut requests = vec![
            PersistenceRecordRequest::Read {
                request_id: 1,
                address: world_metadata_record_address(),
            },
            PersistenceRecordRequest::Read {
                request_id: 2,
                address: dimension_record_address(&DimensionKey::overworld()),
            },
        ];
        requests.extend(
            LOCAL_REALM_BOOTSTRAP_SAVED_DATA_KEYS
                .iter()
                .enumerate()
                .map(|(index, key)| PersistenceRecordRequest::Read {
                    request_id: WEB_BOOTSTRAP_SAVED_DATA_REQUEST_ID_BASE + index as u64,
                    address: saved_data_record_address(key),
                }),
        );
        requests.push(PersistenceRecordRequest::ProbeAny {
            request_id: WEB_BOOTSTRAP_PROBE_REQUEST_ID,
            namespaces: vec![
                PersistenceRecordNamespace::Dimension,
                PersistenceRecordNamespace::Chunk,
                PersistenceRecordNamespace::EntityChunk,
                PersistenceRecordNamespace::Player,
            ],
        });
        persistence_record_requests_to_js(requests).map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen(js_name = indexedDbWriterLeaseName)]
    pub fn indexed_db_writer_lease_name(&self, world_id: String) -> Result<String, JsValue> {
        let world_id = world_id.trim();
        if world_id.is_empty() {
            return Err(JsValue::from_str(
                "IndexedDB writer lease requires a non-empty world id",
            ));
        }
        Ok(web_world_writer_lease_name(world_id))
    }

    #[wasm_bindgen(js_name = createTransient)]
    pub fn create_transient(
        &self,
        job_worker_url: String,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
    ) -> Result<WebIntegratedServerActor, JsValue> {
        let definition = web_dimension_definition_from_startup(&self.config);
        let fixture_store = if let Some(showcase) = self.config.transient_playable_showcase {
            Some(
                mclone_server::playable_showcase_memory_store(
                    showcase,
                    &self.config.local_player_identity,
                )
                .map_err(|error| JsValue::from_str(&error.to_string()))?
                .1,
            )
        } else {
            self.config
                .transient_authored_fixture
                .map(mclone_server::authored_world_fixture_memory_store)
                .transpose()
                .map_err(|error| JsValue::from_str(&error.to_string()))?
                .map(|(_, store)| store)
        };
        let server = match (fixture_store, job_worker_url.trim().is_empty()) {
            (Some(store), true) => {
                LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
                    definition,
                    Box::new(store),
                )
            }
            (Some(store), false) => LocalRealmSession::
                local_integrated_with_world_store_dimension_definition_and_wasm_job_workers(
                    definition,
                    Box::new(store),
                    WasmServerJobWorkerConfig::new(
                        job_worker_url,
                        bindgen_js_url,
                        bindgen_wasm_url,
                    ),
                ),
            (None, true) => {
                LocalRealmSession::local_integrated_with_dimension_definition(definition)
            }
            (None, false) => {
                LocalRealmSession::local_integrated_with_dimension_definition_and_wasm_job_workers(
                    definition,
                    WasmServerJobWorkerConfig::new(
                        job_worker_url,
                        bindgen_js_url,
                        bindgen_wasm_url,
                    ),
                )
            }
        };
        self.finish_startup(
            McloneWebIntegratedServerWorker::from_server(self.config.seed, server, None),
            self.config.transient_authored_fixture.is_some()
                || self.config.transient_playable_showcase.is_some(),
            false,
        )
        .map_err(|error| JsValue::from_str(&error))
    }

    #[wasm_bindgen(js_name = createIndexedDb)]
    pub fn create_indexed_db(
        &self,
        bootstrap_completions: JsValue,
        job_worker_url: String,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
    ) -> Result<WebIntegratedServerActor, JsValue> {
        let responses = decode_persistence_record_responses_from_js(&bootstrap_completions)
            .map_err(|error| JsValue::from_str(&error))?;
        let (state, metadata, dimension) = web_persistence_state_from_bootstrap(responses)
            .map_err(|error| {
                JsValue::from_str(&format!("initialize IndexedDB record executor: {error}"))
            })?;
        let stored_world_metadata_present = metadata.is_some();
        let definition =
            web_dimension_definition_for_startup_state(&self.config, dimension.as_ref());
        let store = Box::new(RecordExecutorWorldStore::new(
            WebPersistenceRecordExecutor::new(Rc::clone(&state)),
            true,
        ));
        let mut server = if job_worker_url.trim().is_empty() {
            LocalRealmSession::
                local_integrated_with_external_load_world_store_and_dimension_definition(
                    definition, store,
                )
        } else {
            LocalRealmSession::
                local_integrated_with_external_load_world_store_dimension_definition_and_wasm_job_workers(
                    definition,
                    store,
                    WasmServerJobWorkerConfig::new(
                        job_worker_url,
                        bindgen_js_url,
                        bindgen_wasm_url,
                    ),
                )
        };
        apply_stored_world_metadata_profiles(&mut server, metadata.as_ref())
            .map_err(|error| JsValue::from_str(&error))?;
        self.finish_startup(
            McloneWebIntegratedServerWorker::from_server(self.config.seed, server, Some(state)),
            true,
            stored_world_metadata_present,
        )
        .map_err(|error| JsValue::from_str(&error))
    }
}

impl WebIntegratedServerStartup {
    fn finish_startup(
        &self,
        mut worker: McloneWebIntegratedServerWorker,
        initialize_world_metadata: bool,
        stored_world_metadata_present: bool,
    ) -> Result<WebIntegratedServerActor, String> {
        if !stored_world_metadata_present {
            worker
                .server
                .set_world_generation_profile(self.config.world_generation_profile)
                .map_err(|error| error.to_string())?;
            worker
                .server
                .set_world_behavior_profile(self.config.world_behavior_profile);
            worker
                .server
                .set_starter_content(self.config.starter_content);
        }
        if initialize_world_metadata {
            worker
                .server
                .initialize_world_metadata_blocking()
                .map_err(|error| error.to_string())?;
        }
        if let Some(day_time) = self.config.day_time {
            worker.server.server_mut().set_day_time(day_time);
        }
        worker
            .server
            .server_mut()
            .set_day_time_frozen(self.config.day_time_frozen);
        if self.config.observer_only {
            worker
                .server
                .begin_observing(
                    DimensionKey::overworld(),
                    ChunkView {
                        center: ChunkPos::new(0, 0),
                        render_distance: 0,
                        chunk_tracking_radius: 0,
                    },
                    ObserverSimulationInterest::BlockAndEntityTicking,
                )
                .map_err(|error| error.to_string())?;
        }
        worker
            .server
            .configure_local_player_identity(self.config.local_player_identity.clone())
            .map_err(|error| error.to_string())?;
        worker
            .server
            .set_scheduled_fluid_ticks_frozen(self.config.freeze_scheduled_fluid_ticks);
        worker
            .server
            .set_debug_passive_showcase_enabled(self.config.debug_passive_showcase);
        worker
            .server
            .set_debug_auxiliary_player_script_enabled(self.config.debug_auxiliary_player_script);
        worker
            .server
            .set_light_status_batch_size(self.config.light_status_batch_size);
        worker.refresh_diagnostics(None, false, None);
        Ok(WebIntegratedServerActor {
            server: worker,
            operation: None,
        })
    }
}

#[wasm_bindgen]
impl WebIntegratedServerActor {
    #[wasm_bindgen(js_name = readyReport)]
    pub fn ready_report(&self, request_id: u32) -> Result<JsValue, JsValue> {
        let response = Object::new();
        set_bool(&response, "ok", true).map_err(|error| JsValue::from_str(&error))?;
        set_string(&response, "kind", "ready").map_err(|error| JsValue::from_str(&error))?;
        set_number(&response, "requestId", f64::from(request_id))
            .map_err(|error| JsValue::from_str(&error))?;
        let updates = Array::new();
        Reflect::set(&response, &JsValue::from_str("updates"), &updates)
            .map_err(|error| JsValue::from_str(&format!("attach ready updates: {error:?}")))?;
        Reflect::set(
            &response,
            &JsValue::from_str("diagnostics"),
            &diagnostics_to_js(&self.server.diagnostics).map_err(JsValue::from)?,
        )
        .map_err(|error| JsValue::from_str(&format!("attach ready diagnostics: {error:?}")))?;
        Ok(response.into())
    }

    #[wasm_bindgen(js_name = beginMessage)]
    pub fn begin_message(
        &mut self,
        message: JsValue,
        frame: Uint8Array,
    ) -> Result<JsValue, JsValue> {
        let kind = string_prop(&message, "kind")
            .ok_or_else(|| JsValue::from_str("integrated-server actor message is missing kind"))?;
        let request_id = number_prop(&message, "requestId").unwrap_or(0.0) as u32;
        let operation_kind = match kind.as_str() {
            "command" => WebIntegratedServerOperationKind::Command,
            "flush-persistence" => WebIntegratedServerOperationKind::FlushPersistence,
            "promote-observer" => WebIntegratedServerOperationKind::PromoteObserver,
            "demote-player" => WebIntegratedServerOperationKind::DemotePlayer,
            "shutdown" => WebIntegratedServerOperationKind::Shutdown,
            other => {
                return Err(JsValue::from_str(&format!(
                    "unexpected integrated server worker message kind {other}"
                )));
            }
        };
        self.begin_operation(operation_kind, request_id)
            .map_err(|error| JsValue::from_str(&error))?;
        let result = match operation_kind {
            WebIntegratedServerOperationKind::Command => self.server.handle_command_frame(frame),
            WebIntegratedServerOperationKind::FlushPersistence => self.server.flush_persistence(),
            WebIntegratedServerOperationKind::PromoteObserver => {
                self.server.promote_observer_to_player()
            }
            WebIntegratedServerOperationKind::DemotePlayer => {
                self.server.demote_player_to_observer()
            }
            WebIntegratedServerOperationKind::Shutdown => self.server.shutdown(),
            WebIntegratedServerOperationKind::Tick => unreachable!("ticks have a dedicated entry"),
            WebIntegratedServerOperationKind::Poll => {
                unreachable!("polls have a dedicated entry")
            }
            WebIntegratedServerOperationKind::PersistenceCompletion => {
                unreachable!("persistence completions have a dedicated entry")
            }
        };
        if result.is_err() {
            self.operation = None;
        }
        result
    }

    #[wasm_bindgen(js_name = beginTick)]
    pub fn begin_tick(&mut self) -> Result<JsValue, JsValue> {
        self.begin_operation(WebIntegratedServerOperationKind::Tick, 0)
            .map_err(|error| JsValue::from_str(&error))?;
        let result = self.server.tick();
        if result.is_err() {
            self.operation = None;
        }
        result
    }

    #[wasm_bindgen(js_name = beginPoll)]
    pub fn begin_poll(&mut self) -> Result<JsValue, JsValue> {
        self.begin_operation(WebIntegratedServerOperationKind::Poll, 0)
            .map_err(|error| JsValue::from_str(&error))?;
        let result = self.server.poll();
        if result.is_err() {
            self.operation = None;
        }
        result
    }

    #[wasm_bindgen(js_name = beginPersistenceCompletion)]
    pub fn begin_persistence_completion(
        &mut self,
        completions: JsValue,
    ) -> Result<JsValue, JsValue> {
        self.begin_operation(WebIntegratedServerOperationKind::PersistenceCompletion, 0)
            .map_err(|error| JsValue::from_str(&error))?;
        let result = self.server.complete_indexed_db_record_requests(completions);
        if result.is_err() {
            self.operation = None;
        }
        result
    }

    #[wasm_bindgen(js_name = continuePersistenceFence)]
    pub fn continue_persistence_fence(&mut self, completions: JsValue) -> Result<JsValue, JsValue> {
        let Some(operation) = self.operation else {
            return Err(JsValue::from_str(
                "integrated-server actor has no active persistence fence",
            ));
        };
        if !matches!(
            operation.kind,
            WebIntegratedServerOperationKind::FlushPersistence
                | WebIntegratedServerOperationKind::Shutdown
        ) {
            return Err(JsValue::from_str(
                "integrated-server persistence continuation requires a flush or shutdown fence",
            ));
        }
        self.server.complete_indexed_db_record_requests(completions)
    }

    #[wasm_bindgen(js_name = finishOperation)]
    pub fn finish_operation(
        &mut self,
        result: JsValue,
        updates: Array,
    ) -> Result<JsValue, JsValue> {
        let operation = self
            .operation
            .take()
            .ok_or_else(|| JsValue::from_str("integrated-server actor has no active operation"))?;
        set_string(
            &Object::from(result.clone()),
            "kind",
            operation.kind.response_kind(),
        )
        .map_err(|error| JsValue::from_str(&error))?;
        Reflect::set(&result, &JsValue::from_str("updates"), &updates)
            .map_err(|error| JsValue::from_str(&format!("attach actor updates: {error:?}")))?;
        Reflect::set(
            &result,
            &JsValue::from_str("updateCount"),
            &JsValue::from_f64(f64::from(updates.length())),
        )
        .map_err(|error| JsValue::from_str(&format!("attach actor update count: {error:?}")))?;
        Reflect::set(
            &result,
            &JsValue::from_str("requestId"),
            &JsValue::from_f64(f64::from(operation.request_id)),
        )
        .map_err(|error| JsValue::from_str(&format!("attach actor request id: {error:?}")))?;

        let report = Object::new();
        Reflect::set(&report, &JsValue::from_str("message"), &result)
            .map_err(|error| JsValue::from_str(&format!("attach actor message: {error:?}")))?;
        set_bool(
            &report,
            "postMessage",
            operation.kind.posts_empty_response() || updates.length() > 0,
        )
        .map_err(|error| JsValue::from_str(&error))?;
        set_bool(&report, "closeWorker", operation.kind.closes_worker())
            .map_err(|error| JsValue::from_str(&error))?;
        set_bool(
            &report,
            "backgroundPollRequested",
            integrated_server_background_poll_requested(&self.server.diagnostics),
        )
        .map_err(|error| JsValue::from_str(&error))?;
        Ok(report.into())
    }

    #[wasm_bindgen(js_name = failOperation)]
    pub fn fail_operation(
        &mut self,
        fallback_request_id: u32,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let request_id = self
            .operation
            .take()
            .map_or(fallback_request_id, |operation| operation.request_id);
        let response = Object::new();
        set_bool(&response, "ok", false).map_err(|error| JsValue::from_str(&error))?;
        set_string(&response, "kind", "error").map_err(|error| JsValue::from_str(&error))?;
        set_number(&response, "requestId", f64::from(request_id))
            .map_err(|error| JsValue::from_str(&error))?;
        set_string(&response, "reason", &reason).map_err(|error| JsValue::from_str(&error))?;
        Ok(response.into())
    }
}

impl WebIntegratedServerActor {
    fn begin_operation(
        &mut self,
        kind: WebIntegratedServerOperationKind,
        request_id: u32,
    ) -> Result<(), String> {
        if self.operation.is_some() {
            return Err("integrated-server actor already has an active operation".to_owned());
        }
        if !self.server.running {
            return Err("integrated-server actor is shut down".to_owned());
        }
        self.operation = Some(WebIntegratedServerOperation { kind, request_id });
        Ok(())
    }
}

fn integrated_server_background_poll_requested(diagnostics: &ServerRunnerDiagnostics) -> bool {
    diagnostics.pending_jobs > 0
        || diagnostics.pending_publications > 0
        || diagnostics.worldgen_mailbox_pending_jobs > 0
        || diagnostics.light_status_mailbox_pending_statuses > 0
}

#[wasm_bindgen]
impl McloneWebIntegratedServerWorker {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: i64) -> Self {
        let server = LocalRealmSession::local_integrated(seed);
        Self::from_server(seed, server, None)
    }

    #[wasm_bindgen(js_name = withDefinition)]
    pub fn with_definition(
        seed: i64,
        generation_profile: String,
        world_topology: String,
    ) -> Result<Self, JsValue> {
        let definition = web_dimension_definition(seed, &generation_profile, &world_topology)
            .map_err(|error| JsValue::from_str(&error))?;
        let server = LocalRealmSession::local_integrated_with_dimension_definition(definition);
        Ok(Self::from_server(seed, server, None))
    }

    #[wasm_bindgen(js_name = withJobWorkers)]
    pub fn with_job_workers(
        seed: i64,
        job_worker_url: String,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
    ) -> Self {
        let server = LocalRealmSession::local_integrated_with_wasm_job_workers(
            seed,
            WasmServerJobWorkerConfig::new(job_worker_url, bindgen_js_url, bindgen_wasm_url),
        );
        Self::from_server(seed, server, None)
    }

    #[wasm_bindgen(js_name = withDefinitionAndJobWorkers)]
    pub fn with_definition_and_job_workers(
        seed: i64,
        generation_profile: String,
        world_topology: String,
        job_worker_url: String,
        bindgen_js_url: String,
        bindgen_wasm_url: String,
    ) -> Result<Self, JsValue> {
        let definition = web_dimension_definition(seed, &generation_profile, &world_topology)
            .map_err(|error| JsValue::from_str(&error))?;
        let server =
            LocalRealmSession::local_integrated_with_dimension_definition_and_wasm_job_workers(
                definition,
                WasmServerJobWorkerConfig::new(job_worker_url, bindgen_js_url, bindgen_wasm_url),
            );
        Ok(Self::from_server(seed, server, None))
    }

    #[wasm_bindgen(js_name = setLightStatusBatchSize)]
    pub fn set_light_status_batch_size(&mut self, batch_size: usize) {
        self.server.set_light_status_batch_size(batch_size);
    }

    #[wasm_bindgen(js_name = setWorldGenerationProfile)]
    pub fn set_world_generation_profile(&mut self, profile: String) -> Result<(), JsValue> {
        let profile = WorldGenerationProfile::parse_label(&profile)
            .map_err(|error| JsValue::from_str(&error))?;
        self.server
            .set_world_generation_profile(profile)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    #[wasm_bindgen(js_name = setLocalPlayerIdentity)]
    pub fn set_local_player_identity(
        &mut self,
        profile_id: Uint8Array,
        display_name: String,
    ) -> Result<(), JsValue> {
        let bytes: [u8; 16] = profile_id
            .to_vec()
            .try_into()
            .map_err(|_| JsValue::from_str("local profile UUID must contain 16 bytes"))?;
        let identity = ClientIdentity::new(PlayerProfileId::new(bytes), display_name)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        self.server
            .configure_local_player_identity(identity)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    #[wasm_bindgen(js_name = enableObserverMode)]
    pub fn enable_observer_mode(&mut self) -> Result<(), JsValue> {
        self.server
            .begin_observing(
                DimensionKey::overworld(),
                ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                },
                ObserverSimulationInterest::BlockAndEntityTicking,
            )
            .map(|_| ())
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    #[wasm_bindgen(js_name = promoteObserverToPlayer)]
    pub fn promote_observer_to_player(&mut self) -> Result<JsValue, JsValue> {
        self.server
            .promote_observer_to_player()
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let updates = self
            .server
            .try_drain_updates()
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        self.refresh_diagnostics(None, false, None);
        self.worker_response(updates).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = demotePlayerToObserver)]
    pub fn demote_player_to_observer(&mut self) -> Result<JsValue, JsValue> {
        self.server
            .demote_player_to_observer()
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let updates = self
            .server
            .try_drain_updates()
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        self.refresh_diagnostics(None, false, None);
        self.worker_response(updates).map_err(JsValue::from)
    }

    #[wasm_bindgen(js_name = setWorldBehaviorProfile)]
    pub fn set_world_behavior_profile(&mut self, profile: String) -> Result<(), JsValue> {
        let profile = mclone_server::WorldBehaviorProfile::parse_label(&profile)
            .map_err(|error| JsValue::from_str(&error))?;
        self.server.set_world_behavior_profile(profile);
        Ok(())
    }

    #[wasm_bindgen(js_name = initializeWorldMetadata)]
    pub fn initialize_world_metadata(&mut self) -> Result<(), JsValue> {
        self.server
            .initialize_world_metadata_blocking()
            .map(|_| ())
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    #[wasm_bindgen(js_name = setScheduledFluidTicksFrozen)]
    pub fn set_scheduled_fluid_ticks_frozen(&mut self, frozen: bool) {
        self.server.set_scheduled_fluid_ticks_frozen(frozen);
    }

    #[wasm_bindgen(js_name = setDebugPassiveShowcaseEnabled)]
    pub fn set_debug_passive_showcase_enabled(&mut self, enabled: bool) {
        self.server.set_debug_passive_showcase_enabled(enabled);
    }

    #[wasm_bindgen(js_name = setDebugAuxiliaryPlayerScriptEnabled)]
    pub fn set_debug_auxiliary_player_script_enabled(&mut self, enabled: bool) {
        self.server
            .set_debug_auxiliary_player_script_enabled(enabled);
    }

    #[wasm_bindgen(js_name = completeIndexedDbRecordRequests)]
    pub fn complete_indexed_db_record_requests(
        &mut self,
        completions: JsValue,
    ) -> Result<JsValue, JsValue> {
        let completions = decode_persistence_record_responses_from_js(&completions)
            .map_err(|error| JsValue::from_str(&error))?;
        for response in completions {
            let request_id = response.request_id();
            if let Some(request) = self.pending_indexed_db_reads.remove(&request_id) {
                let PersistenceRecordResponse::Read {
                    address, result, ..
                } = &response
                else {
                    return Err(JsValue::from_str(&format!(
                        "IndexedDB record read {request_id} completed with {response:?}"
                    )));
                };
                let state = self.indexed_db_state.as_ref().ok_or_else(|| {
                    JsValue::from_str("IndexedDB record completion has no active executor")
                })?;
                state
                    .borrow_mut()
                    .apply_external_read(address.clone(), result);
                let completion = world_store_completion_from_record_read(request, response)
                    .map_err(|error| JsValue::from_str(&error.to_string()))?;
                self.server
                    .scheduler_mut()
                    .complete_external_persistence_request(completion)
                    .map_err(|error| JsValue::from_str(&error.to_string()))?;
            } else {
                self.indexed_db_state
                    .as_ref()
                    .ok_or_else(|| {
                        JsValue::from_str("IndexedDB record completion has no active executor")
                    })?
                    .borrow_mut()
                    .complete_executor_response(&response)
                    .map_err(|error| JsValue::from_str(&error.to_string()))?;
            }
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
                self.server
                    .try_handle_command(command)
                    .map_err(|error| error.to_string())
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
                let updates = report.updates.clone();
                let autosave_error = self.autosave_indexed_db_dirty_chunks().err();
                self.refresh_diagnostics(
                    Some(ServerRunnerTickDiagnostics::from_report(&report, wall_us)),
                    false,
                    autosave_error,
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

    #[wasm_bindgen(js_name = flushPersistence)]
    pub fn flush_persistence(&mut self) -> Result<JsValue, JsValue> {
        if let Err(error) = self.queue_indexed_db_persistence(true) {
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
        server: LocalRealmSession,
        indexed_db_state: Option<Rc<RefCell<WebPersistenceRecordState>>>,
    ) -> Self {
        let mut diagnostics =
            ServerRunnerDiagnostics::initial(ServerRunnerKind::WebWorker, seed, server.day_time());
        diagnostics.running = true;
        diagnostics.runner_frame_metrics = WorkerFrameMetrics::message_transfer();
        let mut worker = Self {
            server,
            diagnostics,
            indexed_db_state,
            pending_indexed_db_reads: BTreeMap::new(),
            command_queue_depth: 0,
            running: true,
        };
        worker.refresh_diagnostics(None, false, None);
        worker
    }

    fn worker_response(&mut self, updates: Vec<ServerUpdate>) -> Result<JsValue, String> {
        let response = worker_response(updates, &mut self.diagnostics)?;
        if self.indexed_db_state.is_some() {
            let external = self
                .server
                .scheduler_mut()
                .drain_external_persistence_requests();
            let mut requests = self
                .indexed_db_state
                .as_ref()
                .expect("checked above")
                .borrow_mut()
                .drain_outgoing();
            for request in external {
                let record_request = record_read_for_world_store_request(&request)
                    .map_err(|error| error.to_string())?;
                if self
                    .pending_indexed_db_reads
                    .insert(request.request_id(), request)
                    .is_some()
                {
                    return Err("duplicate IndexedDB persistence request id".to_owned());
                }
                requests.push(record_request);
            }
            attach_persistence_record_requests(&response, requests)?;
        }
        Ok(response)
    }

    fn autosave_indexed_db_dirty_chunks(&mut self) -> Result<(), String> {
        self.queue_indexed_db_persistence(self.server.game_time().is_multiple_of(6_000))
    }

    fn queue_indexed_db_persistence(&mut self, save_world_metadata: bool) -> Result<(), String> {
        if self.indexed_db_state.is_none() {
            return Ok(());
        }
        self.server
            .save_dirty_chunks()
            .map_err(|error| error.to_string())?;
        self.server
            .save_all_player_records()
            .map_err(|error| error.to_string())?;
        if save_world_metadata {
            self.server
                .save_world_metadata_blocking()
                .map_err(|error| error.to_string())?;
        }
        Ok(())
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
        self.diagnostics.persistence_queue_metrics =
            self.server.scheduler().persistence_queue_metrics();
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
        self.diagnostics.accepted_local_chunk_view =
            self.server.accepted_local_chunk_view().cloned();
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
    diagnostics.runner_emitted_snapshot_updates =
        diagnostics.runner_emitted_snapshot_updates.saturating_add(
            updates
                .iter()
                .filter(|update| matches!(update, ServerUpdate::ChunkSnapshot(_)))
                .count() as u64,
        );
    diagnostics.runner_emitted_unload_updates =
        diagnostics.runner_emitted_unload_updates.saturating_add(
            updates
                .iter()
                .filter(|update| matches!(update, ServerUpdate::ChunkUnload { .. }))
                .count() as u64,
        );
    let object = Object::new();
    let packed_updates = Array::new();
    for update in updates {
        let frame = encode_server_update(&update).map_err(|error| error.to_string())?;
        diagnostics.runner_frame_metrics.record_inbound(frame.len());
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

fn attach_persistence_record_requests(
    response: &JsValue,
    requests: Vec<PersistenceRecordRequest>,
) -> Result<(), String> {
    let requests = persistence_record_requests_to_js(requests)?;
    Reflect::set(
        response,
        &JsValue::from_str("persistenceRecordRequests"),
        &requests,
    )
    .map_err(|error| format!("attach persistence record requests: {error:?}"))?;
    Ok(())
}

fn persistence_record_requests_to_js(
    requests: Vec<PersistenceRecordRequest>,
) -> Result<Array, String> {
    let array = Array::new();
    for request in requests {
        let object = Object::new();
        set_string(&object, "requestId", &request.request_id().to_string())?;
        match request {
            PersistenceRecordRequest::Read { address, .. } => {
                set_string(&object, "kind", "read")?;
                encode_persistence_record_address(&object, &address)?;
            }
            PersistenceRecordRequest::ProbeAny { namespaces, .. } => {
                set_string(&object, "kind", "probeAny")?;
                let encoded = Array::new();
                for namespace in namespaces {
                    encoded.push(&JsValue::from_f64(f64::from(namespace.stable_id())));
                }
                Reflect::set(&object, &JsValue::from_str("namespaces"), &encoded)
                    .map_err(|error| format!("attach record probe namespaces: {error:?}"))?;
            }
            PersistenceRecordRequest::Commit { batch, .. } => {
                set_string(&object, "kind", "commit")?;
                let mutations = Array::new();
                for mutation in batch.mutations {
                    let encoded = Object::new();
                    match mutation {
                        PersistenceRecordMutation::Put { address, payload } => {
                            set_string(&encoded, "kind", "put")?;
                            encode_persistence_record_address(&encoded, &address)?;
                            set_number(&encoded, "codecVersion", f64::from(payload.codec_version))?;
                            set_string(&encoded, "revision", &payload.revision.to_string())?;
                            Reflect::set(
                                &encoded,
                                &JsValue::from_str("record"),
                                &Uint8Array::from(payload.bytes.as_slice()),
                            )
                            .map_err(|error| format!("attach record mutation bytes: {error:?}"))?;
                        }
                        PersistenceRecordMutation::Delete { address } => {
                            set_string(&encoded, "kind", "delete")?;
                            encode_persistence_record_address(&encoded, &address)?;
                        }
                    }
                    mutations.push(&encoded);
                }
                Reflect::set(&object, &JsValue::from_str("mutations"), &mutations)
                    .map_err(|error| format!("attach record mutations: {error:?}"))?;
            }
            PersistenceRecordRequest::Flush { .. } => {
                set_string(&object, "kind", "flush")?;
            }
            PersistenceRecordRequest::Close { .. } => {
                set_string(&object, "kind", "close")?;
            }
        }
        array.push(&object);
    }
    Ok(array)
}

fn encode_persistence_record_address(
    object: &Object,
    address: &PersistenceRecordAddress,
) -> Result<(), String> {
    set_number(
        object,
        "namespace",
        f64::from(address.namespace.stable_id()),
    )?;
    let key = Array::new();
    for part in &address.key {
        let encoded = Object::new();
        match part {
            PersistenceRecordKeyPart::Text(value) => {
                set_string(&encoded, "kind", "text")?;
                set_string(&encoded, "value", value)?;
            }
            PersistenceRecordKeyPart::I32(value) => {
                set_string(&encoded, "kind", "i32")?;
                set_number(&encoded, "value", f64::from(*value))?;
            }
        }
        key.push(&encoded);
    }
    Reflect::set(object, &JsValue::from_str("key"), &key)
        .map_err(|error| format!("attach persistence record key: {error:?}"))?;
    Ok(())
}

fn decode_persistence_record_responses_from_js(
    value: &JsValue,
) -> Result<Vec<PersistenceRecordResponse>, String> {
    let array = value
        .dyn_ref::<Array>()
        .ok_or_else(|| "persistence record completions must be an array".to_owned())?;
    let mut responses = Vec::with_capacity(array.length() as usize);
    for index in 0..array.length() {
        let value = array.get(index);
        let kind = string_prop(&value, "kind")
            .ok_or_else(|| format!("record completion {index} is missing kind"))?;
        let request_id = u64_prop(&value, "requestId")
            .map_err(|error| format!("record completion {index}: {error}"))?;
        let error = persistence_record_error_from_js(&value, index)?;
        let response = match kind.as_str() {
            "read" => {
                let address = decode_persistence_record_address(&value, index)?;
                let result = if let Some(error) = error {
                    Err(error)
                } else if bool_prop(&value, "found") == Some(false) {
                    Ok(None)
                } else {
                    Ok(Some(PersistenceRecordPayload::new(
                        number_prop(&value, "codecVersion").unwrap_or(0.0) as u32,
                        optional_u64_prop(&value, "revision")?.unwrap_or(0),
                        js_record_bytes(&value)?,
                    )))
                };
                PersistenceRecordResponse::Read {
                    request_id,
                    address,
                    result,
                }
            }
            "probeAny" => PersistenceRecordResponse::ProbeAny {
                request_id,
                result: match error {
                    Some(error) => Err(error),
                    None => Ok(bool_prop(&value, "value").unwrap_or(false)),
                },
            },
            "commit" => PersistenceRecordResponse::Commit {
                request_id,
                result: error.map_or(Ok(()), Err),
            },
            "flush" => PersistenceRecordResponse::Flush {
                request_id,
                result: error.map_or(Ok(()), Err),
            },
            "close" => PersistenceRecordResponse::Close {
                request_id,
                result: error.map_or(Ok(()), Err),
            },
            other => {
                return Err(format!(
                    "record completion {index} has unsupported kind {other}"
                ));
            }
        };
        responses.push(response);
    }
    Ok(responses)
}

fn decode_persistence_record_address(
    value: &JsValue,
    index: u32,
) -> Result<PersistenceRecordAddress, String> {
    let namespace_id = number_prop(value, "namespace")
        .ok_or_else(|| format!("record completion {index} is missing namespace"))?;
    if namespace_id.fract() != 0.0 || !(0.0..=f64::from(u8::MAX)).contains(&namespace_id) {
        return Err(format!(
            "record completion {index} has invalid namespace {namespace_id}"
        ));
    }
    let namespace = PersistenceRecordNamespace::from_stable_id(namespace_id as u8)
        .ok_or_else(|| format!("record completion {index} has unknown namespace {namespace_id}"))?;
    let key_value = Reflect::get(value, &JsValue::from_str("key"))
        .map_err(|error| format!("read record completion key: {error:?}"))?;
    let key_array = key_value
        .dyn_ref::<Array>()
        .ok_or_else(|| format!("record completion {index} key must be an array"))?;
    let mut key = Vec::with_capacity(key_array.length() as usize);
    for key_index in 0..key_array.length() {
        let part = key_array.get(key_index);
        match string_prop(&part, "kind").as_deref() {
            Some("text") => key.push(PersistenceRecordKeyPart::Text(
                string_prop(&part, "value").ok_or_else(|| {
                    format!("record completion {index} text key {key_index} is missing value")
                })?,
            )),
            Some("i32") => {
                let value = number_prop(&part, "value").ok_or_else(|| {
                    format!("record completion {index} i32 key {key_index} is missing value")
                })?;
                if value.fract() != 0.0
                    || value < f64::from(i32::MIN)
                    || value > f64::from(i32::MAX)
                {
                    return Err(format!(
                        "record completion {index} i32 key {key_index} is out of range"
                    ));
                }
                key.push(PersistenceRecordKeyPart::I32(value as i32));
            }
            other => {
                return Err(format!(
                    "record completion {index} key {key_index} has invalid kind {other:?}"
                ));
            }
        }
    }
    Ok(PersistenceRecordAddress::new(namespace, key))
}

fn persistence_record_error_from_js(
    value: &JsValue,
    index: u32,
) -> Result<Option<ChunkStoreError>, String> {
    if bool_prop(value, "ok") != Some(false) {
        return Ok(None);
    }
    let kind_label = string_prop(value, "errorKind").unwrap_or_else(|| "backend".to_owned());
    let kind = PersistenceErrorKind::parse_label(&kind_label).ok_or_else(|| {
        format!("record completion {index} has unknown error kind {kind_label:?}")
    })?;
    let message = string_prop(value, "error")
        .unwrap_or_else(|| "browser persistence request failed".to_owned());
    Ok(Some(ChunkStoreError::classified(kind, message)))
}

fn u64_prop(value: &JsValue, name: &str) -> Result<u64, String> {
    optional_u64_prop(value, name)?.ok_or_else(|| format!("missing {name}"))
}

fn optional_u64_prop(value: &JsValue, name: &str) -> Result<Option<u64>, String> {
    let raw = Reflect::get(value, &JsValue::from_str(name))
        .map_err(|error| format!("read {name}: {error:?}"))?;
    if raw.is_null() || raw.is_undefined() {
        return Ok(None);
    }
    if let Some(text) = raw.as_string() {
        return text
            .parse::<u64>()
            .map(Some)
            .map_err(|error| format!("invalid {name} {text:?}: {error}"));
    }
    let number = raw
        .as_f64()
        .ok_or_else(|| format!("{name} must be a decimal string"))?;
    if number.fract() != 0.0 || number < 0.0 || number > 9_007_199_254_740_991.0 {
        return Err(format!("{name} number is not an exact nonnegative integer"));
    }
    Ok(Some(number as u64))
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
    set_number(
        &object,
        "persistenceForegroundRequests",
        diagnostics.persistence_queue_metrics.foreground_requests as f64,
    )?;
    set_number(
        &object,
        "persistenceDurableWriteRequests",
        diagnostics.persistence_queue_metrics.durable_write_requests as f64,
    )?;
    set_number(
        &object,
        "persistenceDurableWriteBytes",
        diagnostics.persistence_queue_metrics.durable_write_bytes as f64,
    )?;
    set_number(
        &object,
        "persistenceCacheWriteRequests",
        diagnostics.persistence_queue_metrics.cache_write_requests as f64,
    )?;
    set_number(
        &object,
        "persistenceCacheWriteBytes",
        diagnostics.persistence_queue_metrics.cache_write_bytes as f64,
    )?;
    set_number(
        &object,
        "persistenceRetainedRecordCacheEntries",
        diagnostics
            .persistence_queue_metrics
            .retained_record_cache_entries as f64,
    )?;
    set_number(
        &object,
        "persistenceRetainedRecordCacheBytes",
        diagnostics
            .persistence_queue_metrics
            .retained_record_cache_bytes as f64,
    )?;
    set_number(
        &object,
        "persistenceHighWaterForegroundRequests",
        diagnostics
            .persistence_queue_metrics
            .high_water_foreground_requests as f64,
    )?;
    set_number(
        &object,
        "persistenceHighWaterDurableWriteRequests",
        diagnostics
            .persistence_queue_metrics
            .high_water_durable_write_requests as f64,
    )?;
    set_number(
        &object,
        "persistenceHighWaterDurableWriteBytes",
        diagnostics
            .persistence_queue_metrics
            .high_water_durable_write_bytes as f64,
    )?;
    set_number(
        &object,
        "persistenceHighWaterCacheWriteRequests",
        diagnostics
            .persistence_queue_metrics
            .high_water_cache_write_requests as f64,
    )?;
    set_number(
        &object,
        "persistenceHighWaterCacheWriteBytes",
        diagnostics
            .persistence_queue_metrics
            .high_water_cache_write_bytes as f64,
    )?;
    set_number(
        &object,
        "persistenceCancelledRequests",
        diagnostics.persistence_queue_metrics.cancelled_requests as f64,
    )?;
    set_number(
        &object,
        "persistenceSkippedCacheWrites",
        diagnostics.persistence_queue_metrics.skipped_cache_writes as f64,
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
        "schedulerClientVisibleChunks",
        diagnostics.scheduler_metrics.client_visible_chunks as f64,
    )?;
    set_number(
        &object,
        "schedulerLoadedSnapshotChunks",
        diagnostics.scheduler_metrics.loaded_snapshot_chunks as f64,
    )?;
    set_number(
        &object,
        "schedulerActiveTicketChunks",
        diagnostics.scheduler_metrics.active_ticket_chunks as f64,
    )?;
    set_number(
        &object,
        "trackingPlayerVisibleChunks",
        diagnostics.chunk_tracking.total_player_visible_chunks as f64,
    )?;
    set_number(
        &object,
        "trackingPlayerPublishedChunks",
        diagnostics.chunk_tracking.total_player_published_chunks as f64,
    )?;
    set_number(
        &object,
        "trackingPlayerPublishedVisibleChunks",
        diagnostics
            .chunk_tracking
            .total_player_published_visible_chunks as f64,
    )?;
    set_number(
        &object,
        "trackingPlayerMissingPublishedChunks",
        diagnostics
            .chunk_tracking
            .total_player_missing_published_chunks as f64,
    )?;
    set_number(
        &object,
        "trackingPlayerPublishedOutsideVisibleChunks",
        diagnostics
            .chunk_tracking
            .total_player_published_outside_visible_chunks as f64,
    )?;
    set_number(
        &object,
        "trackingPlayerQueuedSnapshotUpdates",
        diagnostics
            .chunk_tracking
            .total_player_queued_snapshot_updates as f64,
    )?;
    set_number(
        &object,
        "trackingPlayerQueuedUnloadUpdates",
        diagnostics
            .chunk_tracking
            .total_player_queued_unload_updates as f64,
    )?;
    set_number(
        &object,
        "trackingPlayerDrainedSnapshotUpdates",
        diagnostics
            .chunk_tracking
            .total_player_drained_snapshot_updates as f64,
    )?;
    set_number(
        &object,
        "trackingPlayerDrainedUnloadUpdates",
        diagnostics
            .chunk_tracking
            .total_player_drained_unload_updates as f64,
    )?;
    set_number(
        &object,
        "trackingAggregatePlayerTicketChunks",
        diagnostics.chunk_tracking.aggregate_player_ticket_chunks as f64,
    )?;
    set_number(
        &object,
        "trackingPlayerOutboundQueueDepth",
        diagnostics.chunk_tracking.total_outbound_queue_depth as f64,
    )?;
    set_number(
        &object,
        "runnerEmittedSnapshotUpdates",
        diagnostics.runner_emitted_snapshot_updates as f64,
    )?;
    set_number(
        &object,
        "runnerEmittedUnloadUpdates",
        diagnostics.runner_emitted_unload_updates as f64,
    )?;
    if let Some(view) = diagnostics.accepted_local_chunk_view.as_ref() {
        set_bool(&object, "acceptedLocalViewAvailable", true)?;
        set_number(&object, "acceptedLocalCenterX", f64::from(view.center.x))?;
        set_number(&object, "acceptedLocalCenterZ", f64::from(view.center.z))?;
        set_number(
            &object,
            "acceptedLocalRenderDistance",
            f64::from(view.render_distance),
        )?;
        set_number(
            &object,
            "acceptedLocalTrackingRadius",
            f64::from(view.chunk_tracking_radius),
        )?;
    } else {
        set_bool(&object, "acceptedLocalViewAvailable", false)?;
    }
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
    Reflect::set(
        &object,
        &JsValue::from_str("loadingProgressSnapshot"),
        &loading_progress_snapshot_to_js(diagnostics.loading_progress_snapshot.as_ref())?,
    )
    .map_err(|error| format!("failed to attach loading progress snapshot: {error:?}"))?;
    Reflect::set(
        &object,
        &JsValue::from_str("viewReadinessSnapshot"),
        &loading_progress_snapshot_to_js(diagnostics.view_readiness_snapshot.as_ref())?,
    )
    .map_err(|error| format!("failed to attach view readiness snapshot: {error:?}"))?;
    Ok(object.into())
}

fn loading_progress_snapshot_to_js(
    snapshot: Option<&ChunkLoadingProgressSnapshot>,
) -> Result<JsValue, String> {
    let Some(snapshot) = snapshot else {
        return Ok(JsValue::NULL);
    };
    let object = Object::new();
    let stats = snapshot.stats;
    set_number(&object, "centerX", f64::from(stats.center.x))?;
    set_number(&object, "centerZ", f64::from(stats.center.z))?;
    set_number(&object, "targetRadius", f64::from(stats.target_radius))?;
    set_number(
        &object,
        "targetStatus",
        f64::from(chunk_status_code(stats.target_status)),
    )?;
    set_number(&object, "targetChunkCount", stats.target_chunk_count as f64)?;
    set_number(
        &object,
        "targetReadyChunks",
        stats.target_ready_chunks as f64,
    )?;
    set_number(&object, "playableX", f64::from(stats.playable_chunk.x))?;
    set_number(&object, "playableZ", f64::from(stats.playable_chunk.z))?;
    set_number(
        &object,
        "playableGateRadius",
        f64::from(stats.playable_gate_radius),
    )?;
    set_number(
        &object,
        "playableGateChunkCount",
        stats.playable_gate_chunk_count as f64,
    )?;
    set_number(
        &object,
        "playableGateReadyChunks",
        stats.playable_gate_ready_chunks as f64,
    )?;
    set_bool(&object, "playableChunkReady", stats.playable_chunk_ready)?;
    let cells = Array::new();
    for cell in &snapshot.cells {
        let encoded = Array::new();
        encoded.push(&JsValue::from_f64(f64::from(cell.relative_x)));
        encoded.push(&JsValue::from_f64(f64::from(cell.relative_z)));
        encoded.push(&JsValue::from_f64(
            cell.status
                .map_or(-1.0, |status| f64::from(chunk_status_code(status))),
        ));
        encoded.push(&JsValue::from_bool(cell.target_ready));
        encoded.push(&JsValue::from_bool(cell.playable));
        cells.push(&encoded);
    }
    Reflect::set(&object, &JsValue::from_str("cells"), &cells)
        .map_err(|error| format!("failed to attach loading progress cells: {error:?}"))?;
    Ok(object.into())
}

fn loading_progress_snapshot_from_js(value: &JsValue) -> Option<ChunkLoadingProgressSnapshot> {
    let cells_value = reflect_get(value, "cells")?;
    if !Array::is_array(&cells_value) {
        return None;
    }
    let cells = Array::from(&cells_value)
        .iter()
        .filter_map(|value| {
            if !Array::is_array(&value) {
                return None;
            }
            let values = Array::from(&value);
            Some(ChunkLoadingProgressCell {
                relative_x: values.get(0).as_f64()? as i32,
                relative_z: values.get(1).as_f64()? as i32,
                status: chunk_status_from_code(values.get(2).as_f64()? as i32),
                target_ready: values.get(3).as_bool()?,
                playable: values.get(4).as_bool()?,
            })
        })
        .collect();
    Some(ChunkLoadingProgressSnapshot {
        stats: ChunkLoadingProgressStats {
            center: ChunkPos::new(
                number_prop(value, "centerX")? as i32,
                number_prop(value, "centerZ")? as i32,
            ),
            target_radius: number_prop(value, "targetRadius")? as u32,
            target_status: chunk_status_from_code(number_prop(value, "targetStatus")? as i32)?,
            target_chunk_count: number_prop(value, "targetChunkCount")? as usize,
            target_ready_chunks: number_prop(value, "targetReadyChunks")? as usize,
            playable_chunk: ChunkPos::new(
                number_prop(value, "playableX")? as i32,
                number_prop(value, "playableZ")? as i32,
            ),
            playable_gate_radius: number_prop(value, "playableGateRadius")? as u32,
            playable_gate_chunk_count: number_prop(value, "playableGateChunkCount")? as usize,
            playable_gate_ready_chunks: number_prop(value, "playableGateReadyChunks")? as usize,
            playable_chunk_ready: bool_prop(value, "playableChunkReady")?,
        },
        cells,
    })
}

const fn chunk_status_code(status: ChunkStatus) -> u8 {
    match status {
        ChunkStatus::Terrain => 0,
        ChunkStatus::Surface => 1,
        ChunkStatus::Features => 2,
        ChunkStatus::Light => 3,
        ChunkStatus::Full => 4,
        ChunkStatus::StructureStarts => 5,
        ChunkStatus::StructureReferences => 6,
    }
}

const fn chunk_status_from_code(code: i32) -> Option<ChunkStatus> {
    match code {
        0 => Some(ChunkStatus::Terrain),
        1 => Some(ChunkStatus::Surface),
        2 => Some(ChunkStatus::Features),
        3 => Some(ChunkStatus::Light),
        4 => Some(ChunkStatus::Full),
        5 => Some(ChunkStatus::StructureStarts),
        6 => Some(ChunkStatus::StructureReferences),
        _ => None,
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
        inbound_frames: number_prop(&value, "inboundFrames")
            .unwrap_or(fallback.inbound_frames as f64) as usize,
        inbound_bytes: number_prop(&value, "inboundBytes").unwrap_or(fallback.inbound_bytes as f64)
            as usize,
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
        shared_buffer_pooled_inbound_frames: number_prop(&value, "sharedBufferPooledInboundFrames")
            .unwrap_or(fallback.shared_buffer_pooled_inbound_frames as f64)
            as usize,
        shared_buffer_fallback_inbound_frames: number_prop(
            &value,
            "sharedBufferFallbackInboundFrames",
        )
        .unwrap_or(fallback.shared_buffer_fallback_inbound_frames as f64)
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
    set_number(&object, "inboundFrames", metrics.inbound_frames as f64)?;
    set_number(&object, "inboundBytes", metrics.inbound_bytes as f64)?;
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
        "sharedBufferPooledInboundFrames",
        metrics.shared_buffer_pooled_inbound_frames as f64,
    )?;
    set_number(
        &object,
        "sharedBufferFallbackInboundFrames",
        metrics.shared_buffer_fallback_inbound_frames as f64,
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
    inbound_bytes: usize,
    frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
) {
    let Ok(inbound_bytes) = u32::try_from(inbound_bytes) else {
        return;
    };
    if inbound_bytes <= slot.response_capacity {
        return;
    }
    let previous_capacity = slot.response_capacity;
    let response_capacity =
        runner_shared_capacity_for_len(inbound_bytes, DEFAULT_RUNNER_SHARED_RESPONSE_BYTES);
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
