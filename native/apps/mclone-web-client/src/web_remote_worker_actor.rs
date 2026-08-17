//! Worker-resident Rust authority for the browser remote WebSocket transport.
//!
//! Main Rust authors opaque actor frames. The browser Worker owns only module
//! loading, `WebSocket`, event forwarding, and execution of open/send/post/close
//! actions. This actor owns protocol state, canonicalization, bounded batch
//! accounting, backpressure decisions, and shutdown.

use std::collections::{BTreeMap, VecDeque};

use mclone_protocol::{ClientCommand, ClientIdentity, ServerUpdate, encode_server_update};

const ACTOR_MAGIC: [u8; 4] = *b"MCRW";
const ACTOR_VERSION: u8 = 1;
const FRAME_START: u8 = 1;
const FRAME_COMMAND: u8 = 2;
const FRAME_RELEASE_BATCH: u8 = 3;
const FRAME_SHUTDOWN: u8 = 4;
const MAX_ACTOR_STRING_BYTES: usize = 1024 * 1024;
const MAX_SOCKET_BUFFERED_COMMAND_BYTES: usize = 8 * 1024 * 1024;
const MAX_UNCONSUMED_UPDATE_BYTES: usize = 64 * 1024 * 1024;

pub(crate) fn encode_remote_actor_start_frame(
    url: &str,
    identity: &ClientIdentity,
) -> Result<Vec<u8>, String> {
    let mut frame = actor_frame(FRAME_START);
    write_string(&mut frame, url)?;
    frame.extend_from_slice(&identity.profile_id.bytes());
    write_string(&mut frame, &identity.display_name)?;
    Ok(frame)
}

pub(crate) fn encode_remote_actor_command_frame(command: &[u8]) -> Result<Vec<u8>, String> {
    let mut frame = actor_frame(FRAME_COMMAND);
    write_bytes(&mut frame, command)?;
    Ok(frame)
}

pub(crate) fn encode_remote_actor_release_frame(batch_sequence: u64) -> Vec<u8> {
    let mut frame = actor_frame(FRAME_RELEASE_BATCH);
    frame.extend_from_slice(&batch_sequence.to_le_bytes());
    frame
}

pub(crate) fn encode_remote_actor_shutdown_frame() -> Vec<u8> {
    actor_frame(FRAME_SHUTDOWN)
}

fn actor_frame(kind: u8) -> Vec<u8> {
    let mut frame = Vec::with_capacity(6);
    frame.extend_from_slice(&ACTOR_MAGIC);
    frame.push(ACTOR_VERSION);
    frame.push(kind);
    frame
}

fn write_string(frame: &mut Vec<u8>, value: &str) -> Result<(), String> {
    write_bytes(frame, value.as_bytes())
}

fn write_bytes(frame: &mut Vec<u8>, value: &[u8]) -> Result<(), String> {
    let len = u32::try_from(value.len())
        .map_err(|_| "remote actor frame value exceeds u32 length".to_owned())?;
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(value);
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RemoteActorFrame {
    Start {
        url: String,
        identity: ClientIdentity,
    },
    Command(Vec<u8>),
    ReleaseBatch(u64),
    Shutdown,
}

fn decode_remote_actor_frame(frame: &[u8]) -> Result<RemoteActorFrame, String> {
    let mut cursor = FrameCursor::new(frame);
    if cursor.take_exact(ACTOR_MAGIC.len())? != ACTOR_MAGIC {
        return Err("remote actor frame has invalid magic".to_owned());
    }
    let version = cursor.take_u8()?;
    if version != ACTOR_VERSION {
        return Err(format!(
            "remote actor frame version {version} is unsupported"
        ));
    }
    let kind = cursor.take_u8()?;
    let decoded = match kind {
        FRAME_START => {
            let url = cursor.take_string()?;
            if url.trim().is_empty() {
                return Err("remote actor start frame has an empty URL".to_owned());
            }
            let profile_id: [u8; 16] = cursor
                .take_exact(16)?
                .try_into()
                .expect("profile UUID length was checked");
            let display_name = cursor.take_string()?;
            let identity = ClientIdentity::new(
                mclone_protocol::PlayerProfileId::new(profile_id),
                display_name,
            )
            .map_err(|error| error.to_string())?;
            RemoteActorFrame::Start { url, identity }
        }
        FRAME_COMMAND => RemoteActorFrame::Command(cursor.take_bytes()?),
        FRAME_RELEASE_BATCH => RemoteActorFrame::ReleaseBatch(cursor.take_u64()?),
        FRAME_SHUTDOWN => RemoteActorFrame::Shutdown,
        other => return Err(format!("remote actor frame kind {other} is unsupported")),
    };
    cursor.finish()?;
    Ok(decoded)
}

struct FrameCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> FrameCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take_exact(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| "remote actor frame offset overflow".to_owned())?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| "remote actor frame is truncated".to_owned())?;
        self.offset = end;
        Ok(value)
    }

    fn take_u8(&mut self) -> Result<u8, String> {
        Ok(self.take_exact(1)?[0])
    }

    fn take_u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.take_exact(4)?
                .try_into()
                .expect("u32 frame length was checked"),
        ))
    }

    fn take_u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(
            self.take_exact(8)?
                .try_into()
                .expect("u64 frame length was checked"),
        ))
    }

    fn take_bytes(&mut self) -> Result<Vec<u8>, String> {
        let len = self.take_u32()? as usize;
        if len > MAX_ACTOR_STRING_BYTES.max(MAX_UNCONSUMED_UPDATE_BYTES) {
            return Err("remote actor frame value exceeds its limit".to_owned());
        }
        Ok(self.take_exact(len)?.to_vec())
    }

    fn take_string(&mut self) -> Result<String, String> {
        let bytes = self.take_bytes()?;
        if bytes.len() > MAX_ACTOR_STRING_BYTES {
            return Err("remote actor frame string exceeds its limit".to_owned());
        }
        String::from_utf8(bytes).map_err(|_| "remote actor frame string is not UTF-8".to_owned())
    }

    fn finish(self) -> Result<(), String> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err("remote actor frame has trailing bytes".to_owned())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RemoteSocketState {
    Connecting,
    AwaitingHandshake,
    Ready,
    Failed,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SendBound {
    None,
    Command,
    Control,
}

#[derive(Clone, Debug, PartialEq)]
enum RemoteBrowserAction {
    Open(String),
    Send { bytes: Vec<u8>, bound: SendBound },
    Post(RemoteMainReport),
    CloseSocket,
    CloseWorker,
}

#[derive(Clone, Debug, PartialEq)]
enum RemoteMainReport {
    Ready,
    CommandSent {
        bytes: usize,
    },
    Updates {
        batch_sequence: u64,
        received_bytes: usize,
        decode_ms: f64,
        frames: Vec<Vec<u8>>,
    },
    Error {
        message: String,
    },
    Closed {
        message: String,
    },
}

#[derive(Debug)]
struct RemoteSocketActorCore {
    identity: ClientIdentity,
    state: RemoteSocketState,
    next_batch_sequence: u64,
    unconsumed_update_bytes: usize,
    unconsumed_batches: BTreeMap<u64, usize>,
    actions: VecDeque<RemoteBrowserAction>,
}

impl RemoteSocketActorCore {
    fn new(frame: &[u8]) -> Result<Self, String> {
        let RemoteActorFrame::Start { url, identity } = decode_remote_actor_frame(frame)? else {
            return Err("remote actor must be initialized with a start frame".to_owned());
        };
        let mut actions = VecDeque::new();
        actions.push_back(RemoteBrowserAction::Open(url));
        Ok(Self {
            identity,
            state: RemoteSocketState::Connecting,
            next_batch_sequence: 1,
            unconsumed_update_bytes: 0,
            unconsumed_batches: BTreeMap::new(),
            actions,
        })
    }

    fn handle_main_frame(&mut self, frame: &[u8]) {
        let decoded = match decode_remote_actor_frame(frame) {
            Ok(decoded) => decoded,
            Err(error) => {
                self.fail(error);
                return;
            }
        };
        match decoded {
            RemoteActorFrame::Start { .. } => {
                self.fail("remote websocket actor was already started".to_owned())
            }
            RemoteActorFrame::Command(frame) => self.handle_command(frame),
            RemoteActorFrame::ReleaseBatch(sequence) => self.release_batch(sequence),
            RemoteActorFrame::Shutdown => self.shutdown(),
        }
    }

    fn socket_opened(&mut self) {
        if matches!(
            self.state,
            RemoteSocketState::Failed | RemoteSocketState::Closed
        ) {
            return;
        }
        if self.state != RemoteSocketState::Connecting {
            self.fail("remote websocket opened in an invalid actor state".to_owned());
            return;
        }
        match mclone_net::encode_websocket_client_handshake_with_identity(
            mclone_protocol::PROTOCOL_VERSION,
            &self.identity,
        ) {
            Ok(bytes) => {
                self.state = RemoteSocketState::AwaitingHandshake;
                self.actions.push_back(RemoteBrowserAction::Send {
                    bytes,
                    bound: SendBound::None,
                });
            }
            Err(error) => self.fail(error.to_string()),
        }
    }

    fn socket_frame(&mut self, frame: &[u8], decode_ms: f64) {
        match self.state {
            RemoteSocketState::AwaitingHandshake => {
                match mclone_net::decode_websocket_server_handshake(
                    frame,
                    mclone_protocol::PROTOCOL_VERSION,
                ) {
                    Ok(_) => {
                        self.state = RemoteSocketState::Ready;
                        self.actions
                            .push_back(RemoteBrowserAction::Post(RemoteMainReport::Ready));
                    }
                    Err(error) => self.fail(error.to_string()),
                }
            }
            RemoteSocketState::Ready => self.handle_update_batch(frame, decode_ms),
            RemoteSocketState::Failed | RemoteSocketState::Closed => {}
            _ => self.fail("remote websocket received data before it was ready".to_owned()),
        }
    }

    fn set_latest_decode_ms(&mut self, decode_ms: f64) {
        let decode_ms = decode_ms.max(0.0);
        for action in self.actions.iter_mut().rev() {
            if let RemoteBrowserAction::Post(RemoteMainReport::Updates {
                decode_ms: current, ..
            }) = action
            {
                *current = decode_ms;
                break;
            }
        }
    }

    fn socket_failed(&mut self, message: String) {
        if matches!(
            self.state,
            RemoteSocketState::Failed | RemoteSocketState::Closed
        ) {
            return;
        }
        self.fail(message);
    }

    fn socket_closed(&mut self) {
        if matches!(
            self.state,
            RemoteSocketState::Failed | RemoteSocketState::Closed
        ) {
            return;
        }
        self.state = RemoteSocketState::Closed;
        self.actions
            .push_back(RemoteBrowserAction::Post(RemoteMainReport::Closed {
                message: "remote websocket transport closed".to_owned(),
            }));
    }

    fn take_action(&mut self, buffered_amount: usize) -> Option<RemoteBrowserAction> {
        let action = self.actions.pop_front()?;
        let RemoteBrowserAction::Send { bound, .. } = &action else {
            return Some(action);
        };
        if *bound == SendBound::None || buffered_amount <= MAX_SOCKET_BUFFERED_COMMAND_BYTES {
            return Some(action);
        }
        let message = match bound {
            SendBound::Command => format!(
                "remote command socket buffer exceeded {MAX_SOCKET_BUFFERED_COMMAND_BYTES} bytes"
            ),
            SendBound::Control => format!(
                "remote control socket buffer exceeded {MAX_SOCKET_BUFFERED_COMMAND_BYTES} bytes"
            ),
            SendBound::None => unreachable!("unbounded sends returned above"),
        };
        self.fail(message);
        self.actions.pop_front()
    }

    fn handle_command(&mut self, frame: Vec<u8>) {
        if self.state != RemoteSocketState::Ready {
            self.fail("remote websocket is not ready for commands".to_owned());
            return;
        }
        let command = match mclone_net::decode_websocket_client_command(&frame) {
            Ok(command) => command,
            Err(error) => {
                self.fail(error.to_string());
                return;
            }
        };
        let canonical = match mclone_net::encode_websocket_client_command(&command) {
            Ok(frame) => frame,
            Err(error) => {
                self.fail(error.to_string());
                return;
            }
        };
        let bytes = canonical.len();
        self.actions.push_back(RemoteBrowserAction::Send {
            bytes: canonical,
            bound: SendBound::Command,
        });
        self.actions
            .push_back(RemoteBrowserAction::Post(RemoteMainReport::CommandSent {
                bytes,
            }));
    }

    fn handle_update_batch(&mut self, frame: &[u8], decode_ms: f64) {
        let updates = match mclone_net::decode_websocket_server_update_batch(frame) {
            Ok(updates) => updates,
            Err(error) => {
                self.fail(error.to_string());
                return;
            }
        };
        let mut canonical_frames = Vec::with_capacity(updates.len());
        for update in updates {
            if let ServerUpdate::KeepAlive { id } = &update {
                match mclone_net::encode_websocket_client_command(&ClientCommand::KeepAlive {
                    id: *id,
                }) {
                    Ok(response) => self.actions.push_back(RemoteBrowserAction::Send {
                        bytes: response,
                        bound: SendBound::Control,
                    }),
                    Err(error) => {
                        self.fail(error.to_string());
                        return;
                    }
                }
            }
            match encode_server_update(&update) {
                Ok(canonical) => canonical_frames.push(canonical),
                Err(error) => {
                    self.fail(error.to_string());
                    return;
                }
            }
        }

        let next_bytes = self.unconsumed_update_bytes.saturating_add(frame.len());
        if next_bytes > MAX_UNCONSUMED_UPDATE_BYTES {
            self.fail(format!(
                "remote update queue exceeded {MAX_UNCONSUMED_UPDATE_BYTES} bytes"
            ));
            return;
        }
        let batch_sequence = self.next_batch_sequence;
        self.next_batch_sequence = self.next_batch_sequence.saturating_add(1);
        self.unconsumed_update_bytes = next_bytes;
        self.unconsumed_batches.insert(batch_sequence, frame.len());
        self.actions
            .push_back(RemoteBrowserAction::Post(RemoteMainReport::Updates {
                batch_sequence,
                received_bytes: frame.len(),
                decode_ms: decode_ms.max(0.0),
                frames: canonical_frames,
            }));
    }

    fn release_batch(&mut self, sequence: u64) {
        let Some(bytes) = self.unconsumed_batches.remove(&sequence) else {
            return;
        };
        self.unconsumed_update_bytes = self.unconsumed_update_bytes.saturating_sub(bytes);
    }

    fn shutdown(&mut self) {
        if self.state == RemoteSocketState::Closed {
            if !self
                .actions
                .iter()
                .any(|action| matches!(action, RemoteBrowserAction::CloseWorker))
            {
                self.actions.push_back(RemoteBrowserAction::CloseWorker);
            }
            return;
        }
        self.state = RemoteSocketState::Closed;
        self.actions.clear();
        self.actions.push_back(RemoteBrowserAction::CloseSocket);
        self.actions.push_back(RemoteBrowserAction::CloseWorker);
    }

    fn fail(&mut self, message: String) {
        self.state = RemoteSocketState::Failed;
        self.actions.clear();
        self.actions
            .push_back(RemoteBrowserAction::Post(RemoteMainReport::Error {
                message,
            }));
        self.actions.push_back(RemoteBrowserAction::CloseSocket);
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use js_sys::{Array, Function, Object, Reflect, Uint8Array};
    use wasm_bindgen::prelude::*;

    use super::*;

    #[wasm_bindgen]
    pub struct WebRemoteSocketWorkerActor {
        core: RemoteSocketActorCore,
    }

    #[wasm_bindgen]
    impl WebRemoteSocketWorkerActor {
        #[wasm_bindgen(constructor)]
        pub fn new(frame: Uint8Array) -> Result<Self, JsValue> {
            Ok(Self {
                core: RemoteSocketActorCore::new(&frame.to_vec())
                    .map_err(|error| JsValue::from_str(&error))?,
            })
        }

        #[wasm_bindgen(js_name = handleMainFrame)]
        pub fn handle_main_frame(&mut self, frame: Uint8Array) {
            self.core.handle_main_frame(&frame.to_vec());
        }

        #[wasm_bindgen(js_name = socketOpened)]
        pub fn socket_opened(&mut self) {
            self.core.socket_opened();
        }

        #[wasm_bindgen(js_name = socketFrame)]
        pub fn socket_frame(&mut self, frame: Uint8Array) {
            let start_ms = monotonic_now_ms();
            self.core.socket_frame(&frame.to_vec(), 0.0);
            self.core
                .set_latest_decode_ms((monotonic_now_ms() - start_ms).max(0.0));
        }

        #[wasm_bindgen(js_name = socketFailed)]
        pub fn socket_failed(&mut self, message: String) {
            self.core.socket_failed(message);
        }

        #[wasm_bindgen(js_name = socketClosed)]
        pub fn socket_closed(&mut self) {
            self.core.socket_closed();
        }

        #[wasm_bindgen(js_name = takeAction)]
        pub fn take_action(
            &mut self,
            buffered_amount: f64,
        ) -> Result<Option<WebRemoteSocketWorkerAction>, JsValue> {
            let buffered_amount = if buffered_amount.is_finite() && buffered_amount > 0.0 {
                buffered_amount as usize
            } else {
                0
            };
            self.core
                .take_action(buffered_amount)
                .map(WebRemoteSocketWorkerAction::from_core)
                .transpose()
                .map_err(|error| JsValue::from_str(&error))
        }
    }

    #[wasm_bindgen]
    pub struct WebRemoteSocketWorkerAction {
        kind: String,
        url: Option<String>,
        bytes: Option<Vec<u8>>,
        message: JsValue,
        transfers: Array,
    }

    impl WebRemoteSocketWorkerAction {
        fn from_core(action: RemoteBrowserAction) -> Result<Self, String> {
            match action {
                RemoteBrowserAction::Open(url) => Ok(Self::simple("open", Some(url))),
                RemoteBrowserAction::Send { bytes, .. } => Ok(Self {
                    kind: "send".to_owned(),
                    url: None,
                    bytes: Some(bytes),
                    message: JsValue::UNDEFINED,
                    transfers: Array::new(),
                }),
                RemoteBrowserAction::Post(report) => {
                    let (message, transfers) = build_report(report)?;
                    Ok(Self {
                        kind: "post".to_owned(),
                        url: None,
                        bytes: None,
                        message,
                        transfers,
                    })
                }
                RemoteBrowserAction::CloseSocket => Ok(Self::simple("close-socket", None)),
                RemoteBrowserAction::CloseWorker => Ok(Self::simple("close-worker", None)),
            }
        }

        fn simple(kind: &str, url: Option<String>) -> Self {
            Self {
                kind: kind.to_owned(),
                url,
                bytes: None,
                message: JsValue::UNDEFINED,
                transfers: Array::new(),
            }
        }
    }

    #[wasm_bindgen]
    impl WebRemoteSocketWorkerAction {
        #[wasm_bindgen(getter)]
        pub fn kind(&self) -> String {
            self.kind.clone()
        }

        #[wasm_bindgen(getter)]
        pub fn url(&self) -> Option<String> {
            self.url.clone()
        }

        #[wasm_bindgen(js_name = takeBytes)]
        pub fn take_bytes(&mut self) -> Uint8Array {
            let bytes = self.bytes.take().unwrap_or_default();
            Uint8Array::from(bytes.as_slice())
        }

        #[wasm_bindgen(js_name = message)]
        pub fn message(&self) -> JsValue {
            self.message.clone()
        }

        #[wasm_bindgen(js_name = transfers)]
        pub fn transfers(&self) -> Array {
            self.transfers.clone()
        }
    }

    fn build_report(report: RemoteMainReport) -> Result<(JsValue, Array), String> {
        let object = Object::new();
        let transfers = Array::new();
        match report {
            RemoteMainReport::Ready => set_string(&object, "kind", "ready")?,
            RemoteMainReport::CommandSent { bytes } => {
                set_string(&object, "kind", "command-sent")?;
                set_number(&object, "bytes", bytes as f64)?;
            }
            RemoteMainReport::Updates {
                batch_sequence,
                received_bytes,
                decode_ms,
                frames,
            } => {
                set_string(&object, "kind", "updates")?;
                set_number(&object, "batchSequence", batch_sequence as f64)?;
                set_number(&object, "receivedBytes", received_bytes as f64)?;
                set_number(&object, "decodeMs", decode_ms)?;
                let frame_array = Array::new();
                for frame in frames {
                    let bytes = Uint8Array::from(frame.as_slice());
                    let buffer = bytes.buffer();
                    frame_array.push(&buffer);
                    transfers.push(&buffer);
                }
                set_value(&object, "frames", &frame_array)?;
            }
            RemoteMainReport::Error { message } => {
                set_string(&object, "kind", "error")?;
                set_string(&object, "message", &message)?;
            }
            RemoteMainReport::Closed { message } => {
                set_string(&object, "kind", "closed")?;
                set_string(&object, "message", &message)?;
            }
        }
        Ok((object.into(), transfers))
    }

    fn monotonic_now_ms() -> f64 {
        let global = js_sys::global();
        let Ok(performance) = Reflect::get(&global, &JsValue::from_str("performance")) else {
            return js_sys::Date::now();
        };
        let Ok(now) = Reflect::get(&performance, &JsValue::from_str("now")) else {
            return js_sys::Date::now();
        };
        let Ok(now) = now.dyn_into::<Function>() else {
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
            .map_err(|error| format!("set remote actor report field {name}: {error:?}"))
    }

    fn set_string(object: &Object, name: &str, value: &str) -> Result<(), String> {
        set_value(object, name, &JsValue::from_str(value))
    }

    fn set_number(object: &Object, name: &str, value: f64) -> Result<(), String> {
        set_value(object, name, &JsValue::from_f64(value))
    }

    pub use WebRemoteSocketWorkerAction as ExportedWebRemoteSocketWorkerAction;
    pub use WebRemoteSocketWorkerActor as ExportedWebRemoteSocketWorkerActor;
}

#[cfg(target_arch = "wasm32")]
pub use wasm::{
    ExportedWebRemoteSocketWorkerAction as WebRemoteSocketWorkerAction,
    ExportedWebRemoteSocketWorkerActor as WebRemoteSocketWorkerActor,
};

#[cfg(test)]
mod tests {
    use mclone_protocol::{PlayerProfileId, decode_client_command, decode_server_update};

    use super::*;

    fn identity() -> ClientIdentity {
        ClientIdentity::new(PlayerProfileId::new([7; 16]), "Remote Actor").unwrap()
    }

    fn actor() -> RemoteSocketActorCore {
        RemoteSocketActorCore::new(
            &encode_remote_actor_start_frame("ws://example.test", &identity()).unwrap(),
        )
        .unwrap()
    }

    fn ready_actor() -> RemoteSocketActorCore {
        let mut actor = actor();
        assert_eq!(
            actor.take_action(0),
            Some(RemoteBrowserAction::Open("ws://example.test".to_owned()))
        );
        actor.socket_opened();
        let Some(RemoteBrowserAction::Send { bytes, bound }) = actor.take_action(0) else {
            panic!("socket open should send a handshake");
        };
        assert_eq!(bound, SendBound::None);
        let handshake = mclone_net::decode_websocket_client_handshake(&bytes).unwrap();
        assert_eq!(handshake.identity, identity());
        actor.socket_frame(
            &mclone_net::encode_websocket_server_handshake_accept(
                mclone_protocol::PROTOCOL_VERSION,
            )
            .unwrap(),
            0.25,
        );
        assert_eq!(
            actor.take_action(0),
            Some(RemoteBrowserAction::Post(RemoteMainReport::Ready))
        );
        actor
    }

    #[test]
    fn actor_frame_codec_is_strict() {
        let start = encode_remote_actor_start_frame("ws://example.test", &identity()).unwrap();
        assert_eq!(
            decode_remote_actor_frame(&start).unwrap(),
            RemoteActorFrame::Start {
                url: "ws://example.test".to_owned(),
                identity: identity(),
            }
        );
        assert_eq!(
            decode_remote_actor_frame(&encode_remote_actor_release_frame(42)).unwrap(),
            RemoteActorFrame::ReleaseBatch(42)
        );
        assert_eq!(
            decode_remote_actor_frame(&encode_remote_actor_shutdown_frame()).unwrap(),
            RemoteActorFrame::Shutdown
        );
        let mut trailing = start;
        trailing.push(0);
        assert!(decode_remote_actor_frame(&trailing).is_err());
    }

    #[test]
    fn command_before_handshake_fails_and_closes() {
        let mut actor = actor();
        actor.take_action(0);
        let command = mclone_protocol::encode_client_command(&ClientCommand::Respawn).unwrap();
        actor.handle_main_frame(&encode_remote_actor_command_frame(&command).unwrap());
        assert!(matches!(
            actor.take_action(0),
            Some(RemoteBrowserAction::Post(RemoteMainReport::Error { message }))
                if message == "remote websocket is not ready for commands"
        ));
        assert_eq!(actor.take_action(0), Some(RemoteBrowserAction::CloseSocket));
    }

    #[test]
    fn ready_command_is_canonical_and_acknowledged() {
        let mut actor = ready_actor();
        let command = mclone_protocol::encode_client_command(&ClientCommand::Respawn).unwrap();
        actor.handle_main_frame(&encode_remote_actor_command_frame(&command).unwrap());
        let Some(RemoteBrowserAction::Send { bytes, bound }) = actor.take_action(0) else {
            panic!("ready command should be sent");
        };
        assert_eq!(bound, SendBound::Command);
        assert_eq!(
            decode_client_command(&bytes).unwrap(),
            ClientCommand::Respawn
        );
        assert_eq!(
            actor.take_action(0),
            Some(RemoteBrowserAction::Post(RemoteMainReport::CommandSent {
                bytes: command.len(),
            }))
        );
    }

    #[test]
    fn keepalive_batch_sends_control_and_tracks_release() {
        let mut actor = ready_actor();
        let frame = mclone_net::encode_websocket_server_update_batch(&[
            ServerUpdate::KeepAlive { id: 99 },
            ServerUpdate::TimeUpdate {
                game_time: 12,
                day_time: 34,
                daylight_cycle_running: true,
                calendar_policy: Default::default(),
            },
        ])
        .unwrap();
        actor.socket_frame(&frame, 1.5);
        let Some(RemoteBrowserAction::Send { bytes, bound }) = actor.take_action(0) else {
            panic!("keepalive should produce a control response");
        };
        assert_eq!(bound, SendBound::Control);
        assert_eq!(
            mclone_net::decode_websocket_client_command(&bytes).unwrap(),
            ClientCommand::KeepAlive { id: 99 }
        );
        let Some(RemoteBrowserAction::Post(RemoteMainReport::Updates {
            batch_sequence,
            received_bytes,
            decode_ms,
            frames,
        })) = actor.take_action(0)
        else {
            panic!("update batch should be posted");
        };
        assert_eq!(batch_sequence, 1);
        assert_eq!(received_bytes, frame.len());
        assert_eq!(decode_ms, 1.5);
        assert_eq!(frames.len(), 2);
        assert_eq!(
            decode_server_update(&frames[0]).unwrap(),
            ServerUpdate::KeepAlive { id: 99 }
        );
        assert_eq!(actor.unconsumed_update_bytes, frame.len());
        actor.handle_main_frame(&encode_remote_actor_release_frame(batch_sequence));
        assert_eq!(actor.unconsumed_update_bytes, 0);
    }

    #[test]
    fn command_backpressure_is_actor_owned() {
        let mut actor = ready_actor();
        let command = mclone_protocol::encode_client_command(&ClientCommand::Respawn).unwrap();
        actor.handle_main_frame(&encode_remote_actor_command_frame(&command).unwrap());
        assert!(matches!(
            actor.take_action((MAX_SOCKET_BUFFERED_COMMAND_BYTES + 1) as usize),
            Some(RemoteBrowserAction::Post(RemoteMainReport::Error { message }))
                if message.contains("remote command socket buffer exceeded")
        ));
        assert_eq!(actor.take_action(0), Some(RemoteBrowserAction::CloseSocket));
    }

    #[test]
    fn shutdown_is_idempotent() {
        let mut actor = ready_actor();
        actor.handle_main_frame(&encode_remote_actor_shutdown_frame());
        assert_eq!(actor.take_action(0), Some(RemoteBrowserAction::CloseSocket));
        assert_eq!(actor.take_action(0), Some(RemoteBrowserAction::CloseWorker));
        actor.handle_main_frame(&encode_remote_actor_shutdown_frame());
        assert_eq!(actor.take_action(0), Some(RemoteBrowserAction::CloseWorker));
        assert_eq!(actor.take_action(0), None);
    }

    #[test]
    fn invalid_handshake_is_reported_by_rust() {
        let mut actor = actor();
        actor.take_action(0);
        actor.socket_opened();
        actor.take_action(0);
        actor.socket_frame(b"not a handshake", 0.0);
        assert!(matches!(
            actor.take_action(0),
            Some(RemoteBrowserAction::Post(RemoteMainReport::Error { .. }))
        ));
        assert_eq!(actor.take_action(0), Some(RemoteBrowserAction::CloseSocket));
    }

    #[test]
    fn browser_failure_and_close_reports_are_actor_owned() {
        let mut failed = ready_actor();
        failed.socket_failed("browser websocket failure".to_owned());
        assert_eq!(
            failed.take_action(0),
            Some(RemoteBrowserAction::Post(RemoteMainReport::Error {
                message: "browser websocket failure".to_owned(),
            }))
        );
        assert_eq!(
            failed.take_action(0),
            Some(RemoteBrowserAction::CloseSocket)
        );

        let mut closed = ready_actor();
        closed.socket_closed();
        assert_eq!(
            closed.take_action(0),
            Some(RemoteBrowserAction::Post(RemoteMainReport::Closed {
                message: "remote websocket transport closed".to_owned(),
            }))
        );
    }

    #[test]
    fn measured_decode_time_updates_the_pending_report() {
        let mut actor = ready_actor();
        let frame = mclone_net::encode_websocket_server_update_batch(&[ServerUpdate::TimeUpdate {
            game_time: 12,
            day_time: 34,
            daylight_cycle_running: true,
            calendar_policy: Default::default(),
        }])
        .unwrap();
        actor.socket_frame(&frame, 0.0);
        actor.set_latest_decode_ms(2.5);
        assert!(matches!(
            actor.take_action(0),
            Some(RemoteBrowserAction::Post(RemoteMainReport::Updates {
                decode_ms,
                ..
            })) if decode_ms == 2.5
        ));
    }

    #[test]
    fn stale_socket_events_after_shutdown_are_ignored() {
        let mut actor = ready_actor();
        actor.handle_main_frame(&encode_remote_actor_shutdown_frame());
        assert_eq!(actor.take_action(0), Some(RemoteBrowserAction::CloseSocket));
        assert_eq!(actor.take_action(0), Some(RemoteBrowserAction::CloseWorker));

        actor.socket_opened();
        actor.socket_frame(b"stale", 0.0);
        actor.socket_failed("stale failure".to_owned());
        actor.socket_closed();
        assert_eq!(actor.take_action(0), None);
    }
}
