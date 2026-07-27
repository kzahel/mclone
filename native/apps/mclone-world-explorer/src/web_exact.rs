use std::collections::VecDeque;

use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use mclone_terrain_view::{
    CanonicalEncodedAdmission, CanonicalEncodedBatch, CanonicalEncodedNaturalTree,
    CanonicalExactExecutor, CanonicalExactRequest, CanonicalExactResult, CanonicalMeshBatch,
    CanonicalMeshCoordinate, CanonicalMeshFrontier, CanonicalMeshRequestReceipt,
    CanonicalMeshSession, CanonicalNaturalTreePresentation, CanonicalPackedAdmission,
    CanonicalPackedNaturalTree, CanonicalTerrainStage, CanonicalTerrainVisibility,
    decode_canonical_batch, encode_canonical_batch,
};
use mclone_worldgen::terrain_preview::TerrainPreviewProfile;
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};

use crate::web::load_web_assets;

const FRAME_INIT: &str = "exact-init";
const FRAME_COMPILE: &str = "exact-compile";
const RESPONSE_READY: &str = "exact-ready";
const RESPONSE_BATCH: &str = "exact-batch";
const RESPONSE_ERROR: &str = "exact-error";

#[wasm_bindgen(js_name = WorldExplorerExactWorkerActor)]
#[derive(Default)]
pub struct WorldExplorerExactWorkerActor {
    session: Option<CanonicalMeshSession>,
}

#[wasm_bindgen(js_class = WorldExplorerExactWorkerActor)]
impl WorldExplorerExactWorkerActor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[wasm_bindgen(js_name = handleMessage)]
    pub fn handle_message(&mut self, frame: JsValue) -> WorldExplorerExactWorkerDispatch {
        match self.handle_message_inner(&frame) {
            Ok(message) => WorldExplorerExactWorkerDispatch { message },
            Err(reason) => WorldExplorerExactWorkerDispatch {
                message: error_message(reason),
            },
        }
    }
}

impl WorldExplorerExactWorkerActor {
    fn handle_message_inner(&mut self, frame: &JsValue) -> Result<Object, String> {
        match string_property(frame, "kind")?.as_str() {
            FRAME_INIT => {
                let seed_text = string_property(frame, "seed")?;
                let seed = seed_text
                    .parse::<i64>()
                    .map_err(|error| format!("invalid exact Worker seed {seed_text:?}: {error}"))?;
                let assets = load_web_assets(
                    bytes_property(frame, "authoredBytes")?,
                    bytes_property(frame, "provisionalBytes")?,
                    bytes_property(frame, "diagnosticBytes")?,
                )?;
                let mut session = CanonicalMeshSession::new(
                    TerrainPreviewProfile::McloneOverworldV1,
                    seed,
                    CanonicalTerrainStage::FinalFeatures,
                    assets.catalog,
                );
                session.set_frontier(CanonicalMeshFrontier::SuppressFootprintWalls);
                session.set_natural_tree_presentation(CanonicalNaturalTreePresentation::Separated);
                self.session = Some(session);
                response_message(RESPONSE_READY)
            }
            FRAME_COMPILE => {
                let generation = string_property(frame, "generation")?
                    .parse::<u64>()
                    .map_err(|error| format!("invalid exact generation: {error}"))?;
                let desired = coordinates_property(frame, "desired")?;
                let requested = coordinates_property(frame, "requested")?;
                let session = self
                    .session
                    .as_mut()
                    .ok_or("exact Worker actor is not initialized")?;
                session.begin(desired, CanonicalTerrainVisibility::default(), true);
                let batch = session.compile_batch(&requested)?;
                let encoded = encode_canonical_batch(&encoded_batch(batch))?;
                let message = response_message(RESPONSE_BATCH)?;
                set_string(&message, "generation", &generation.to_string())?;
                set_value(
                    &message,
                    "bytes",
                    Uint8Array::from(encoded.as_slice()).as_ref(),
                )?;
                Ok(message)
            }
            other => Err(format!("unsupported exact Worker frame {other:?}")),
        }
    }
}

#[wasm_bindgen(js_name = WorldExplorerExactWorkerDispatch)]
pub struct WorldExplorerExactWorkerDispatch {
    message: Object,
}

#[wasm_bindgen(js_class = WorldExplorerExactWorkerDispatch)]
impl WorldExplorerExactWorkerDispatch {
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> JsValue {
        self.message.clone().into()
    }
}

pub(crate) struct WebCanonicalExactExecutor {
    transport: JsValue,
    ready: bool,
    in_flight: bool,
    queued: VecDeque<CanonicalExactResult>,
}

impl WebCanonicalExactExecutor {
    pub(crate) fn new(
        transport_factory: JsValue,
        seed: i64,
        authored_bytes: Vec<u8>,
        provisional_bytes: Vec<u8>,
        diagnostic_bytes: Vec<u8>,
    ) -> Result<Self, String> {
        let factory = transport_factory.dyn_into::<Function>().map_err(|_| {
            "World Explorer exact Worker transport factory is not callable".to_owned()
        })?;
        let transport = factory.call0(&JsValue::UNDEFINED).map_err(js_message)?;
        for name in ["post", "poll", "terminate"] {
            method(&transport, name)?;
        }
        let frame = Object::new();
        set_string(&frame, "kind", FRAME_INIT)?;
        set_string(&frame, "seed", &seed.to_string())?;
        let authored = Uint8Array::from(authored_bytes.as_slice());
        let provisional = Uint8Array::from(provisional_bytes.as_slice());
        let diagnostic = Uint8Array::from(diagnostic_bytes.as_slice());
        set_value(&frame, "authoredBytes", authored.as_ref())?;
        set_value(&frame, "provisionalBytes", provisional.as_ref())?;
        set_value(&frame, "diagnosticBytes", diagnostic.as_ref())?;
        let transfer = Array::new();
        transfer.push(&authored.buffer());
        transfer.push(&provisional.buffer());
        transfer.push(&diagnostic.buffer());
        method(&transport, "post")?
            .call2(&transport, frame.as_ref(), transfer.as_ref())
            .map_err(js_message)?;
        Ok(Self {
            transport,
            ready: false,
            in_flight: false,
            queued: VecDeque::new(),
        })
    }

    fn poll_transport(&mut self) -> Result<(), String> {
        for _ in 0..64 {
            let event = method(&self.transport, "poll")?
                .call0(&self.transport)
                .map_err(js_message)?;
            if event.is_null() || event.is_undefined() {
                break;
            }
            match string_property(&event, "kind")?.as_str() {
                "message" => {
                    let message =
                        Reflect::get(&event, &JsValue::from_str("data")).map_err(js_message)?;
                    match string_property(&message, "kind")?.as_str() {
                        RESPONSE_READY => self.ready = true,
                        RESPONSE_BATCH => {
                            self.in_flight = false;
                            let generation = string_property(&message, "generation")?
                                .parse::<u64>()
                                .map_err(|error| {
                                    format!("invalid exact Worker result generation: {error}")
                                })?;
                            let bytes = bytes_property(&message, "bytes")?;
                            self.queued.push_back(CanonicalExactResult {
                                generation,
                                batch: decode_canonical_batch(&bytes).map(decoded_batch),
                            });
                        }
                        RESPONSE_ERROR => {
                            self.in_flight = false;
                            self.queued.push_back(CanonicalExactResult {
                                generation: 0,
                                batch: Err(string_property(&message, "message")?),
                            });
                        }
                        other => {
                            return Err(format!("unsupported exact Worker response {other:?}"));
                        }
                    }
                }
                "error" => {
                    self.in_flight = false;
                    return Err(format!(
                        "World Explorer exact Worker failed: {}",
                        string_property(&event, "message")?
                    ));
                }
                other => return Err(format!("unsupported exact Worker event {other:?}")),
            }
        }
        Ok(())
    }
}

impl CanonicalExactExecutor for WebCanonicalExactExecutor {
    fn try_submit(&mut self, request: CanonicalExactRequest) -> anyhow::Result<bool> {
        self.poll_transport().map_err(anyhow::Error::msg)?;
        if !self.ready || self.in_flight {
            return Ok(false);
        }
        let frame = Object::new();
        set_string(&frame, "kind", FRAME_COMPILE).map_err(anyhow::Error::msg)?;
        set_string(&frame, "generation", &request.generation.to_string())
            .map_err(anyhow::Error::msg)?;
        set_string(
            &frame,
            "desired",
            &coordinates_json(&request.desired).map_err(anyhow::Error::msg)?,
        )
        .map_err(anyhow::Error::msg)?;
        set_string(
            &frame,
            "requested",
            &coordinates_json(&request.requested).map_err(anyhow::Error::msg)?,
        )
        .map_err(anyhow::Error::msg)?;
        method(&self.transport, "post")
            .and_then(|post| {
                post.call1(&self.transport, frame.as_ref())
                    .map(|_| ())
                    .map_err(js_message)
            })
            .map_err(anyhow::Error::msg)?;
        self.in_flight = true;
        Ok(true)
    }

    fn poll(&mut self) -> anyhow::Result<Option<CanonicalExactResult>> {
        self.poll_transport().map_err(anyhow::Error::msg)?;
        Ok(self.queued.pop_front())
    }
}

impl Drop for WebCanonicalExactExecutor {
    fn drop(&mut self) {
        if let Ok(terminate) = method(&self.transport, "terminate") {
            let _ = terminate.call0(&self.transport);
        }
    }
}

fn encoded_batch(batch: CanonicalMeshBatch) -> CanonicalEncodedBatch {
    CanonicalEncodedBatch {
        generation_ms: batch.generation_ms,
        presentation_ms: batch.presentation_ms,
        mesh_ms: batch.mesh_ms,
        pack_ms: batch.pack_ms,
        transfer_ms: 0.0,
        deduplicated_target_chunks: batch.deduplicated_target_chunks.min(u32::MAX as usize) as u32,
        raw_cache_chunks: batch.raw_cache_chunks.min(u32::MAX as usize) as u32,
        raw_cache_bytes: batch.raw_cache_bytes,
        admissions: batch
            .admissions
            .into_iter()
            .map(|admission| CanonicalEncodedAdmission {
                chunk_x: admission.requested.coordinate.chunk_x,
                chunk_z: admission.requested.coordinate.chunk_z,
                raw_cache_hit: admission.requested.raw_cache_hit,
                retained_dependency_chunks: admission
                    .requested
                    .retained_dependency_chunks
                    .min(u32::MAX as usize) as u32,
                packed_sections: admission.packed_sections,
                natural_trees: admission
                    .natural_trees
                    .into_iter()
                    .map(|tree| CanonicalEncodedNaturalTree {
                        occurrence: tree.occurrence,
                        packed_sections: tree.packed_sections,
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn decoded_batch(batch: CanonicalEncodedBatch) -> CanonicalMeshBatch {
    CanonicalMeshBatch {
        admissions: batch
            .admissions
            .into_iter()
            .map(|admission| CanonicalPackedAdmission {
                requested: CanonicalMeshRequestReceipt {
                    coordinate: CanonicalMeshCoordinate::new(admission.chunk_x, admission.chunk_z),
                    raw_cache_hit: admission.raw_cache_hit,
                    retained_dependency_chunks: admission.retained_dependency_chunks as usize,
                },
                packed_sections: admission.packed_sections,
                natural_trees: admission
                    .natural_trees
                    .into_iter()
                    .map(|tree| CanonicalPackedNaturalTree {
                        occurrence: tree.occurrence,
                        packed_sections: tree.packed_sections,
                    })
                    .collect(),
            })
            .collect(),
        generation_ms: batch.generation_ms,
        presentation_ms: batch.presentation_ms,
        mesh_ms: batch.mesh_ms,
        pack_ms: batch.pack_ms,
        deduplicated_target_chunks: batch.deduplicated_target_chunks as usize,
        raw_cache_chunks: batch.raw_cache_chunks as usize,
        raw_cache_bytes: batch.raw_cache_bytes,
    }
}

fn coordinates_json(coordinates: &[CanonicalMeshCoordinate]) -> Result<String, String> {
    serde_json::to_string(
        &coordinates
            .iter()
            .map(|coordinate| [coordinate.chunk_x, coordinate.chunk_z])
            .collect::<Vec<_>>(),
    )
    .map_err(|error| error.to_string())
}

fn coordinates_property(
    value: &JsValue,
    name: &str,
) -> Result<Vec<CanonicalMeshCoordinate>, String> {
    let coordinates = serde_json::from_str::<Vec<[i32; 2]>>(&string_property(value, name)?)
        .map_err(|error| format!("invalid exact Worker {name} coordinates: {error}"))?;
    Ok(coordinates
        .into_iter()
        .map(|[chunk_x, chunk_z]| CanonicalMeshCoordinate::new(chunk_x, chunk_z))
        .collect())
}

fn response_message(kind: &str) -> Result<Object, String> {
    let message = Object::new();
    set_string(&message, "kind", kind)?;
    Ok(message)
}

fn error_message(reason: String) -> Object {
    let message = response_message(RESPONSE_ERROR)
        .expect("creating a plain exact Worker error response cannot fail");
    set_string(&message, "message", &reason)
        .expect("setting a plain exact Worker error response cannot fail");
    message
}

fn method(target: &JsValue, name: &str) -> Result<Function, String> {
    Reflect::get(target, &JsValue::from_str(name))
        .map_err(js_message)?
        .dyn_into::<Function>()
        .map_err(|_| format!("exact Worker transport method {name:?} is not callable"))
}

fn string_property(value: &JsValue, name: &str) -> Result<String, String> {
    Reflect::get(value, &JsValue::from_str(name))
        .map_err(js_message)?
        .as_string()
        .ok_or_else(|| format!("exact Worker property {name:?} is not a string"))
}

fn bytes_property(value: &JsValue, name: &str) -> Result<Vec<u8>, String> {
    Reflect::get(value, &JsValue::from_str(name))
        .map_err(js_message)?
        .dyn_into::<Uint8Array>()
        .map(|bytes| bytes.to_vec())
        .map_err(|_| format!("exact Worker property {name:?} is not a Uint8Array"))
}

fn set_string(target: &Object, name: &str, value: &str) -> Result<(), String> {
    set_value(target, name, &JsValue::from_str(value))
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
        .unwrap_or_else(|| "exact Worker operation failed".to_owned())
}
