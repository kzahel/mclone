use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use js_sys::{ArrayBuffer, Function, Promise, Uint8Array};
use mclone_app_runtime::client_connection::{
    ClientConnectionDrainResult, ClientConnectionQueueMetrics, QueuedServerUpdate,
};
use mclone_net::{
    decode_websocket_client_command, decode_websocket_server_handshake,
    decode_websocket_server_update_batch, encode_current_websocket_client_handshake,
    encode_websocket_client_command,
};
use mclone_protocol::{ClientCommand, PROTOCOL_VERSION, ServerUpdate, encode_server_update};
use mclone_server::{
    ServerRunnerDiagnostics, ServerRunnerKind, WorkerFrameMetrics, WorkerFrameTransportKind,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{BinaryType, ErrorEvent, Event, MessageEvent, WebSocket};

pub struct WebSocketServerSession {
    url: String,
    socket: WebSocket,
    pending_raw_response: Rc<RefCell<Option<PendingWebSocketResponse>>>,
    // Current WebSocket transport is still response-paired at the wire level.
    // Normal frame polling drains decoded `queued_updates` through
    // `WebRuntimeHost: ClientConnection`; this only tracks outstanding
    // response bookkeeping.
    pending_response_batches: Rc<RefCell<usize>>,
    sent_sequence: Rc<RefCell<u64>>,
    received_sequence: Rc<RefCell<u64>>,
    response_waiters: Rc<RefCell<VecDeque<PendingWebSocketWaiter>>>,
    queued_updates: Rc<RefCell<VecDeque<QueuedWebSocketUpdate>>>,
    queued_update_bytes: Rc<RefCell<usize>>,
    request_start_times: Rc<RefCell<VecDeque<f64>>>,
    frame_metrics: Rc<RefCell<WorkerFrameMetrics>>,
    last_error: Rc<RefCell<Option<String>>>,
    day_time: Rc<RefCell<u64>>,
    message_closure: Closure<dyn FnMut(MessageEvent)>,
    error_closure: Closure<dyn FnMut(ErrorEvent)>,
    close_closure: Closure<dyn FnMut(Event)>,
}

impl std::fmt::Debug for WebSocketServerSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocketServerSession")
            .field("url", &self.url)
            .field("ready_state", &self.socket.ready_state())
            .field(
                "pending_raw_response",
                &self.pending_raw_response.borrow().is_some(),
            )
            .field(
                "pending_response_batches",
                &self.pending_response_batches.borrow(),
            )
            .field("queued_updates", &self.queued_updates.borrow().len())
            .field("queued_update_bytes", &self.queued_update_bytes.borrow())
            .field("frame_metrics", &self.frame_metrics.borrow())
            .field("last_error", &self.last_error.borrow())
            .finish_non_exhaustive()
    }
}

impl WebSocketServerSession {
    pub async fn connect(url: impl Into<String>) -> Result<Self, String> {
        let url = url.into();
        let socket = WebSocket::new(&url)
            .map_err(|error| format!("failed to create websocket {url}: {error:?}"))?;
        socket.set_binary_type(BinaryType::Arraybuffer);
        wait_for_websocket_open(&socket)
            .await
            .map_err(|error| format!("failed to open websocket {url}: {error}"))?;

        let pending_raw_response: Rc<RefCell<Option<PendingWebSocketResponse>>> =
            Rc::new(RefCell::new(None));
        let pending_response_batches = Rc::new(RefCell::new(0usize));
        let sent_sequence = Rc::new(RefCell::new(0u64));
        let received_sequence = Rc::new(RefCell::new(0u64));
        let response_waiters = Rc::new(RefCell::new(VecDeque::new()));
        let queued_updates = Rc::new(RefCell::new(VecDeque::new()));
        let queued_update_bytes = Rc::new(RefCell::new(0usize));
        let request_start_times = Rc::new(RefCell::new(VecDeque::new()));
        let frame_metrics = Rc::new(RefCell::new(WorkerFrameMetrics::websocket()));
        let last_error = Rc::new(RefCell::new(None));
        let day_time = Rc::new(RefCell::new(0u64));

        let message_closure = {
            let pending_raw_response = Rc::clone(&pending_raw_response);
            let pending_response_batches = Rc::clone(&pending_response_batches);
            let received_sequence = Rc::clone(&received_sequence);
            let response_waiters = Rc::clone(&response_waiters);
            let queued_updates = Rc::clone(&queued_updates);
            let queued_update_bytes = Rc::clone(&queued_update_bytes);
            let request_start_times = Rc::clone(&request_start_times);
            let frame_metrics = Rc::clone(&frame_metrics);
            let last_error = Rc::clone(&last_error);
            let day_time = Rc::clone(&day_time);
            Closure::wrap(Box::new(move |event: MessageEvent| {
                match websocket_message_bytes(event.data()) {
                    Ok(bytes) => {
                        if let Some(pending_response) = pending_raw_response.borrow_mut().take() {
                            let value: JsValue = Uint8Array::from(bytes.as_slice()).into();
                            let _ = pending_response.resolve.call1(&JsValue::NULL, &value);
                        } else if let Err(error) = queue_websocket_update_batch(
                            bytes,
                            &pending_response_batches,
                            &received_sequence,
                            &response_waiters,
                            &queued_updates,
                            &queued_update_bytes,
                            &request_start_times,
                            &frame_metrics,
                            &day_time,
                        ) {
                            record_websocket_error(&last_error, &response_waiters, error);
                        }
                    }
                    Err(error) => {
                        if let Some(pending_response) = pending_raw_response.borrow_mut().take() {
                            let _ = pending_response
                                .reject
                                .call1(&JsValue::NULL, &JsValue::from_str(&error));
                        }
                        record_websocket_error(&last_error, &response_waiters, error);
                    }
                }
            }) as Box<dyn FnMut(_)>)
        };
        socket.set_onmessage(Some(message_closure.as_ref().unchecked_ref()));

        let error_closure = {
            let pending_raw_response = Rc::clone(&pending_raw_response);
            let response_waiters = Rc::clone(&response_waiters);
            let last_error = Rc::clone(&last_error);
            Closure::wrap(Box::new(move |event: ErrorEvent| {
                let message = if event.message().is_empty() {
                    "websocket transport failed".to_owned()
                } else {
                    event.message()
                };
                if let Some(pending_response) = pending_raw_response.borrow_mut().take() {
                    let _ = pending_response
                        .reject
                        .call1(&JsValue::NULL, &JsValue::from_str(&message));
                }
                record_websocket_error(&last_error, &response_waiters, message);
            }) as Box<dyn FnMut(_)>)
        };
        socket.set_onerror(Some(error_closure.as_ref().unchecked_ref()));

        let close_closure = {
            let pending_raw_response = Rc::clone(&pending_raw_response);
            let response_waiters = Rc::clone(&response_waiters);
            let last_error = Rc::clone(&last_error);
            Closure::wrap(Box::new(move |_event: Event| {
                let message = "websocket transport closed".to_owned();
                if let Some(pending_response) = pending_raw_response.borrow_mut().take() {
                    let _ = pending_response
                        .reject
                        .call1(&JsValue::NULL, &JsValue::from_str(&message));
                }
                record_websocket_error(&last_error, &response_waiters, message);
            }) as Box<dyn FnMut(_)>)
        };
        socket.set_onclose(Some(close_closure.as_ref().unchecked_ref()));

        let mut session = Self {
            url,
            socket,
            pending_raw_response,
            pending_response_batches,
            sent_sequence,
            received_sequence,
            response_waiters,
            queued_updates,
            queued_update_bytes,
            request_start_times,
            frame_metrics,
            last_error,
            day_time,
            message_closure,
            error_closure,
            close_closure,
        };
        session.complete_protocol_handshake().await?;
        Ok(session)
    }

    pub fn queue_command(&mut self, command: ClientCommand) -> Result<bool, String> {
        let frame = encode_websocket_client_command(&command)
            .map_err(|error| format!("encode websocket command: {error}"))?;
        let protocol_codec_roundtrip = decode_websocket_client_command(&frame)
            .map(|decoded| decoded == command)
            .unwrap_or(false);
        self.send_command_frame(frame)?;
        Ok(protocol_codec_roundtrip)
    }

    pub async fn send_command_acknowledged(
        &mut self,
        command: ClientCommand,
    ) -> Result<bool, String> {
        let frame = encode_websocket_client_command(&command)
            .map_err(|error| format!("encode websocket command: {error}"))?;
        let protocol_codec_roundtrip = decode_websocket_client_command(&frame)
            .map(|decoded| decoded == command)
            .unwrap_or(false);
        let target_sequence = self.send_command_frame(frame)?;
        self.wait_for_response_sequence(target_sequence).await?;
        Ok(protocol_codec_roundtrip)
    }

    pub fn drain_next_queued_update(&mut self) -> Result<ClientConnectionDrainResult, String> {
        let Some(queued_update) = self.queued_updates.borrow_mut().pop_front() else {
            let pending = *self.pending_response_batches.borrow();
            if pending == 0 {
                return Ok(ClientConnectionDrainResult::default());
            }
            return Ok(ClientConnectionDrainResult::pending(
                pending,
                *self.queued_update_bytes.borrow(),
            ));
        };
        {
            let mut queued_update_bytes = self.queued_update_bytes.borrow_mut();
            *queued_update_bytes = queued_update_bytes.saturating_sub(queued_update.encoded_len);
        }
        let remaining_depth =
            *self.pending_response_batches.borrow() + self.queued_updates.borrow().len();
        Ok(ClientConnectionDrainResult::with_update(
            queued_update.into_queued_server_update(),
            remaining_depth,
            *self.queued_update_bytes.borrow(),
        ))
    }

    pub fn queued_update_metrics(&self) -> ClientConnectionQueueMetrics {
        ClientConnectionQueueMetrics::new(
            *self.pending_response_batches.borrow() + self.queued_updates.borrow().len(),
            *self.queued_update_bytes.borrow(),
        )
    }

    pub async fn reconnect(&mut self) -> Result<(), String> {
        let replacement = Self::connect(self.url.clone()).await?;
        *self = replacement;
        Ok(())
    }

    pub fn diagnostics(&self) -> ServerRunnerDiagnostics {
        let mut diagnostics = ServerRunnerDiagnostics::initial(
            ServerRunnerKind::RemoteWebSocket,
            0,
            *self.day_time.borrow(),
        );
        diagnostics.running = self.socket.ready_state() == WebSocket::OPEN;
        diagnostics.command_queue_depth = *self.pending_response_batches.borrow();
        diagnostics.update_queue_depth = self.queued_updates.borrow().len();
        diagnostics.update_queue_bytes = *self.queued_update_bytes.borrow();
        diagnostics.runner_frame_metrics = *self.frame_metrics.borrow();
        diagnostics
            .runner_frame_metrics
            .observe_pending_frames(diagnostics.command_queue_depth);
        diagnostics.last_error = self.last_error.borrow().clone();
        diagnostics
    }

    pub fn request_shutdown(&mut self) {
        self.socket.set_onmessage(None);
        self.socket.set_onerror(None);
        self.socket.set_onclose(None);
        let _ = self.socket.close();
    }

    async fn complete_protocol_handshake(&mut self) -> Result<(), String> {
        let request = encode_current_websocket_client_handshake()
            .map_err(|error| format!("encode websocket handshake: {error}"))?;
        let response = self.exchange_frame(request).await?;
        decode_websocket_server_handshake(&response, PROTOCOL_VERSION)
            .map_err(|error| format!("websocket protocol handshake failed: {error}"))
    }

    async fn exchange_frame(&mut self, frame: Vec<u8>) -> Result<Vec<u8>, String> {
        if self.socket.ready_state() != WebSocket::OPEN {
            return Err(format!(
                "websocket {} is not open; readyState={}",
                self.url,
                self.socket.ready_state()
            ));
        }
        let request_start = js_sys::Date::now();
        let promise = self.register_pending_raw_response()?;
        self.socket
            .send_with_u8_array(&frame)
            .map_err(|error| format!("failed to send websocket frame: {error:?}"))?;
        {
            let mut metrics = self.frame_metrics.borrow_mut();
            metrics.transport_kind = WorkerFrameTransportKind::WebSocket;
            metrics.record_request(frame.len());
            metrics.observe_pending_frames(1);
        }
        let response = JsFuture::from(promise)
            .await
            .map_err(|error| format!("websocket response failed: {}", js_error_string(&error)))?;
        let bytes = websocket_message_bytes(response)?;
        let request_us = ((js_sys::Date::now() - request_start).max(0.0) * 1000.0) as u128;
        let mut metrics = self.frame_metrics.borrow_mut();
        metrics.record_response(bytes.len());
        metrics.record_request_time_us(request_us);
        metrics.observe_pending_frames(0);
        Ok(bytes)
    }

    fn send_command_frame(&mut self, frame: Vec<u8>) -> Result<u64, String> {
        if self.socket.ready_state() != WebSocket::OPEN {
            return Err(format!(
                "websocket {} is not open; readyState={}",
                self.url,
                self.socket.ready_state()
            ));
        }
        let request_start = js_sys::Date::now();
        self.socket
            .send_with_u8_array(&frame)
            .map_err(|error| format!("failed to send websocket frame: {error:?}"))?;
        let target_sequence = {
            let mut sent_sequence = self.sent_sequence.borrow_mut();
            *sent_sequence = sent_sequence.saturating_add(1);
            *sent_sequence
        };
        {
            let mut pending = self.pending_response_batches.borrow_mut();
            *pending = pending.saturating_add(1);
        }
        self.request_start_times
            .borrow_mut()
            .push_back(request_start);
        {
            let pending = *self.pending_response_batches.borrow();
            let mut metrics = self.frame_metrics.borrow_mut();
            metrics.transport_kind = WorkerFrameTransportKind::WebSocket;
            metrics.record_request(frame.len());
            metrics.observe_pending_frames(pending);
        }
        Ok(target_sequence)
    }

    async fn wait_for_response_sequence(&self, target_sequence: u64) -> Result<(), String> {
        if *self.received_sequence.borrow() >= target_sequence {
            return Ok(());
        }
        let promise = Promise::new(&mut {
            let response_waiters = Rc::clone(&self.response_waiters);
            move |resolve: Function, reject: Function| {
                response_waiters
                    .borrow_mut()
                    .push_back(PendingWebSocketWaiter {
                        target_sequence,
                        response: PendingWebSocketResponse { resolve, reject },
                    });
            }
        });
        JsFuture::from(promise)
            .await
            .map(|_| ())
            .map_err(|error| format!("websocket response failed: {}", js_error_string(&error)))
    }

    fn register_pending_raw_response(&self) -> Result<Promise, String> {
        if self.pending_raw_response.borrow().is_some() {
            return Err("websocket transport already has an in-flight request".to_owned());
        }
        let pending_raw_response = Rc::clone(&self.pending_raw_response);
        Ok(Promise::new(
            &mut move |resolve: Function, reject: Function| {
                *pending_raw_response.borrow_mut() =
                    Some(PendingWebSocketResponse { resolve, reject });
            },
        ))
    }
}

impl Drop for WebSocketServerSession {
    fn drop(&mut self) {
        self.request_shutdown();
        let _ = &self.message_closure;
        let _ = &self.error_closure;
        let _ = &self.close_closure;
    }
}

#[derive(Clone)]
struct PendingWebSocketResponse {
    resolve: Function,
    reject: Function,
}

struct PendingWebSocketWaiter {
    target_sequence: u64,
    response: PendingWebSocketResponse,
}

#[derive(Debug)]
struct QueuedWebSocketUpdate {
    update: Option<ServerUpdate>,
    encoded_len: usize,
    queued_at_ms: f64,
    transport_drained: bool,
    response_sequence: u64,
    producer_decode_ms: f64,
}

impl QueuedWebSocketUpdate {
    fn single(
        update: ServerUpdate,
        encoded_len: usize,
        transport_drained: bool,
        response_sequence: u64,
        producer_decode_ms: f64,
    ) -> Self {
        Self {
            update: Some(update),
            encoded_len,
            queued_at_ms: js_sys::Date::now(),
            transport_drained,
            response_sequence,
            producer_decode_ms,
        }
    }

    fn empty(transport_drained: bool, response_sequence: u64, producer_decode_ms: f64) -> Self {
        Self {
            update: None,
            encoded_len: 0,
            queued_at_ms: js_sys::Date::now(),
            transport_drained,
            response_sequence,
            producer_decode_ms,
        }
    }

    fn queued_age(&self) -> Duration {
        let elapsed_ms = (js_sys::Date::now() - self.queued_at_ms).max(0.0);
        if elapsed_ms.is_finite() {
            Duration::from_secs_f64(elapsed_ms / 1000.0)
        } else {
            Duration::ZERO
        }
    }

    fn into_queued_server_update(self) -> QueuedServerUpdate {
        let queued_age = self.queued_age();
        let update = match self.update {
            Some(update) => QueuedServerUpdate::single(
                update,
                self.encoded_len,
                queued_age,
                self.transport_drained,
            ),
            None => QueuedServerUpdate::empty(self.transport_drained),
        };
        update.with_remote_metadata(Some(self.response_sequence), 0.0, self.producer_decode_ms)
    }
}

fn queue_websocket_update_batch(
    bytes: Vec<u8>,
    pending_response_batches: &Rc<RefCell<usize>>,
    received_sequence: &Rc<RefCell<u64>>,
    response_waiters: &Rc<RefCell<VecDeque<PendingWebSocketWaiter>>>,
    queued_updates: &Rc<RefCell<VecDeque<QueuedWebSocketUpdate>>>,
    queued_update_bytes: &Rc<RefCell<usize>>,
    request_start_times: &Rc<RefCell<VecDeque<f64>>>,
    frame_metrics: &Rc<RefCell<WorkerFrameMetrics>>,
    day_time: &Rc<RefCell<u64>>,
) -> Result<(), String> {
    let decode_start = js_sys::Date::now();
    let updates = decode_websocket_server_update_batch(&bytes)
        .map_err(|error| format!("decode websocket server updates: {error}"))?;
    let producer_decode_ms = (js_sys::Date::now() - decode_start).max(0.0);
    let response_sequence = {
        let mut received_sequence = received_sequence.borrow_mut();
        *received_sequence = received_sequence.saturating_add(1);
        *received_sequence
    };
    let pending_after_batch = {
        let mut pending = pending_response_batches.borrow_mut();
        *pending = pending.saturating_sub(1);
        *pending
    };
    let request_start = request_start_times.borrow_mut().pop_front();
    {
        let mut metrics = frame_metrics.borrow_mut();
        metrics.record_response(bytes.len());
        if let Some(request_start) = request_start {
            let request_us = ((js_sys::Date::now() - request_start).max(0.0) * 1000.0) as u128;
            metrics.record_request_time_us(request_us);
        }
        metrics.observe_pending_frames(pending_after_batch);
    }

    let update_count = updates.len();
    if update_count == 0 {
        queued_updates
            .borrow_mut()
            .push_back(QueuedWebSocketUpdate::empty(
                pending_after_batch == 0,
                response_sequence,
                producer_decode_ms,
            ));
        resolve_response_waiters(response_waiters, response_sequence);
        return Ok(());
    }

    for (index, update) in updates.into_iter().enumerate() {
        if let ServerUpdate::TimeUpdate {
            day_time: update_day_time,
        } = &update
        {
            *day_time.borrow_mut() = *update_day_time;
        }
        let encoded_len = encode_server_update(&update)
            .map_err(|error| format!("measure websocket server update: {error}"))?
            .len();
        {
            let mut queued_update_bytes = queued_update_bytes.borrow_mut();
            *queued_update_bytes = queued_update_bytes.saturating_add(encoded_len);
        }
        queued_updates
            .borrow_mut()
            .push_back(QueuedWebSocketUpdate::single(
                update,
                encoded_len,
                pending_after_batch == 0 && index + 1 == update_count,
                response_sequence,
                if index == 0 { producer_decode_ms } else { 0.0 },
            ));
    }
    resolve_response_waiters(response_waiters, response_sequence);
    Ok(())
}

fn resolve_response_waiters(
    response_waiters: &Rc<RefCell<VecDeque<PendingWebSocketWaiter>>>,
    received_sequence: u64,
) {
    let mut ready = Vec::new();
    {
        let mut waiters = response_waiters.borrow_mut();
        while waiters
            .front()
            .is_some_and(|waiter| waiter.target_sequence <= received_sequence)
        {
            if let Some(waiter) = waiters.pop_front() {
                ready.push(waiter.response);
            }
        }
    }
    for waiter in ready {
        let _ = waiter.resolve.call0(&JsValue::NULL);
    }
}

fn record_websocket_error(
    last_error: &Rc<RefCell<Option<String>>>,
    response_waiters: &Rc<RefCell<VecDeque<PendingWebSocketWaiter>>>,
    error: String,
) {
    *last_error.borrow_mut() = Some(error.clone());
    let waiters = response_waiters
        .borrow_mut()
        .drain(..)
        .map(|waiter| waiter.response)
        .collect::<Vec<_>>();
    for waiter in waiters {
        let _ = waiter
            .reject
            .call1(&JsValue::NULL, &JsValue::from_str(&error));
    }
}

async fn wait_for_websocket_open(socket: &WebSocket) -> Result<(), String> {
    let promise = Promise::new(&mut |resolve: Function, reject: Function| {
        let open = Closure::once_into_js(move |_event: Event| {
            let _ = resolve.call0(&JsValue::NULL);
        });
        let error = Closure::once_into_js(move |_event: Event| {
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str("websocket open failed"));
        });
        socket.set_onopen(Some(open.unchecked_ref()));
        socket.set_onerror(Some(error.unchecked_ref()));
    });
    JsFuture::from(promise)
        .await
        .map(|_| {
            socket.set_onopen(None);
            socket.set_onerror(None);
        })
        .map_err(|error| js_error_string(&error))
}

fn websocket_message_bytes(value: JsValue) -> Result<Vec<u8>, String> {
    if value.is_instance_of::<ArrayBuffer>() {
        return Ok(Uint8Array::new(&value).to_vec());
    }
    if value.is_instance_of::<Uint8Array>() {
        return Ok(Uint8Array::new(&value).to_vec());
    }
    Err(format!("websocket message was not binary data: {value:?}"))
}

fn js_error_string(value: &JsValue) -> String {
    if let Some(message) = value.as_string() {
        return message;
    }
    if let Ok(message) = js_sys::Reflect::get(value, &JsValue::from_str("message"))
        && let Some(message) = message.as_string()
    {
        return message;
    }
    format!("{value:?}")
}
