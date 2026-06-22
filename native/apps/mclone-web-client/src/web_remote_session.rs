use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use js_sys::{ArrayBuffer, Function, Promise, Uint8Array};
use mclone_net::{
    decode_websocket_client_command, decode_websocket_server_handshake,
    decode_websocket_server_update_batch, encode_current_websocket_client_handshake,
    encode_websocket_client_command,
};
use mclone_protocol::{ClientCommand, PROTOCOL_VERSION, ServerUpdate};
use mclone_server::{
    ServerRunnerDiagnostics, ServerRunnerKind, WorkerFrameMetrics, WorkerFrameTransportKind,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{BinaryType, ErrorEvent, Event, MessageEvent, WebSocket};

#[derive(Clone, Debug, PartialEq)]
pub struct WebSocketExchange {
    pub updates: Vec<ServerUpdate>,
    pub protocol_codec_roundtrip: bool,
    pub transport_drained: bool,
}

pub struct WebSocketServerSession {
    url: String,
    socket: WebSocket,
    pending: Rc<RefCell<Option<PendingWebSocketResponse>>>,
    queued_messages: Rc<RefCell<VecDeque<Vec<u8>>>>,
    frame_metrics: Rc<RefCell<WorkerFrameMetrics>>,
    last_error: Rc<RefCell<Option<String>>>,
    day_time: u64,
    message_closure: Closure<dyn FnMut(MessageEvent)>,
    error_closure: Closure<dyn FnMut(ErrorEvent)>,
    close_closure: Closure<dyn FnMut(Event)>,
}

impl std::fmt::Debug for WebSocketServerSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocketServerSession")
            .field("url", &self.url)
            .field("ready_state", &self.socket.ready_state())
            .field("pending", &self.pending.borrow().is_some())
            .field("queued_messages", &self.queued_messages.borrow().len())
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

        let pending: Rc<RefCell<Option<PendingWebSocketResponse>>> = Rc::new(RefCell::new(None));
        let queued_messages = Rc::new(RefCell::new(VecDeque::new()));
        let frame_metrics = Rc::new(RefCell::new(WorkerFrameMetrics::websocket()));
        let last_error = Rc::new(RefCell::new(None));

        let message_closure = {
            let pending = Rc::clone(&pending);
            let queued_messages = Rc::clone(&queued_messages);
            let last_error = Rc::clone(&last_error);
            Closure::wrap(Box::new(move |event: MessageEvent| {
                match websocket_message_bytes(event.data()) {
                    Ok(bytes) => {
                        if let Some(pending_response) = pending.borrow_mut().take() {
                            let value: JsValue = Uint8Array::from(bytes.as_slice()).into();
                            let _ = pending_response.resolve.call1(&JsValue::NULL, &value);
                        } else {
                            queued_messages.borrow_mut().push_back(bytes);
                        }
                    }
                    Err(error) => {
                        *last_error.borrow_mut() = Some(error.clone());
                        if let Some(pending_response) = pending.borrow_mut().take() {
                            let _ = pending_response
                                .reject
                                .call1(&JsValue::NULL, &JsValue::from_str(&error));
                        }
                    }
                }
            }) as Box<dyn FnMut(_)>)
        };
        socket.set_onmessage(Some(message_closure.as_ref().unchecked_ref()));

        let error_closure = {
            let pending = Rc::clone(&pending);
            let last_error = Rc::clone(&last_error);
            Closure::wrap(Box::new(move |event: ErrorEvent| {
                let message = if event.message().is_empty() {
                    "websocket transport failed".to_owned()
                } else {
                    event.message()
                };
                *last_error.borrow_mut() = Some(message.clone());
                if let Some(pending_response) = pending.borrow_mut().take() {
                    let _ = pending_response
                        .reject
                        .call1(&JsValue::NULL, &JsValue::from_str(&message));
                }
            }) as Box<dyn FnMut(_)>)
        };
        socket.set_onerror(Some(error_closure.as_ref().unchecked_ref()));

        let close_closure = {
            let pending = Rc::clone(&pending);
            let last_error = Rc::clone(&last_error);
            Closure::wrap(Box::new(move |_event: Event| {
                let message = "websocket transport closed".to_owned();
                if last_error.borrow().is_none() {
                    *last_error.borrow_mut() = Some(message.clone());
                }
                if let Some(pending_response) = pending.borrow_mut().take() {
                    let _ = pending_response
                        .reject
                        .call1(&JsValue::NULL, &JsValue::from_str(&message));
                }
            }) as Box<dyn FnMut(_)>)
        };
        socket.set_onclose(Some(close_closure.as_ref().unchecked_ref()));

        let mut session = Self {
            url,
            socket,
            pending,
            queued_messages,
            frame_metrics,
            last_error,
            day_time: 0,
            message_closure,
            error_closure,
            close_closure,
        };
        session.complete_protocol_handshake().await?;
        Ok(session)
    }

    pub async fn exchange_command(
        &mut self,
        command: ClientCommand,
    ) -> Result<WebSocketExchange, String> {
        let frame = encode_websocket_client_command(&command)
            .map_err(|error| format!("encode websocket command: {error}"))?;
        let protocol_codec_roundtrip = decode_websocket_client_command(&frame)
            .map(|decoded| decoded == command)
            .unwrap_or(false);
        let response = self.exchange_frame(frame).await?;
        let updates = decode_websocket_server_update_batch(&response)
            .map_err(|error| format!("decode websocket server updates: {error}"))?;
        for update in &updates {
            if let ServerUpdate::TimeUpdate { day_time } = update {
                self.day_time = *day_time;
            }
        }
        Ok(WebSocketExchange {
            updates,
            protocol_codec_roundtrip,
            transport_drained: self.transport_drained(),
        })
    }

    pub fn diagnostics(&self) -> ServerRunnerDiagnostics {
        let mut diagnostics =
            ServerRunnerDiagnostics::initial(ServerRunnerKind::RemoteWebSocket, 0, self.day_time);
        diagnostics.running = self.socket.ready_state() == WebSocket::OPEN;
        diagnostics.command_queue_depth = usize::from(self.pending.borrow().is_some());
        diagnostics.update_queue_depth = self.queued_messages.borrow().len();
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
        let promise = self.register_pending_response()?;
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

    fn register_pending_response(&self) -> Result<Promise, String> {
        if let Some(bytes) = self.queued_messages.borrow_mut().pop_front() {
            let value: JsValue = Uint8Array::from(bytes.as_slice()).into();
            return Ok(Promise::resolve(&value));
        }
        if self.pending.borrow().is_some() {
            return Err("websocket transport already has an in-flight request".to_owned());
        }
        let pending = Rc::clone(&self.pending);
        Ok(Promise::new(
            &mut move |resolve: Function, reject: Function| {
                *pending.borrow_mut() = Some(PendingWebSocketResponse { resolve, reject });
            },
        ))
    }

    fn transport_drained(&self) -> bool {
        self.pending.borrow().is_none() && self.queued_messages.borrow().is_empty()
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
