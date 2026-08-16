use std::collections::VecDeque;

use crate::{
    CanonicalEncodedAdmission, CanonicalEncodedBatch, CanonicalEncodedNaturalTree,
    CanonicalExactExecutor, CanonicalExactRequest, CanonicalExactResult, CanonicalMeshBatch,
    CanonicalMeshCoordinate, CanonicalMeshRequestReceipt, CanonicalPackedAdmission,
    CanonicalPackedNaturalTree, decode_canonical_batch, encode_canonical_batch,
};
use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};

const FRAME_INIT: &str = "exact-init";
const FRAME_COMPILE: &str = "exact-compile";
const RESPONSE_READY: &str = "exact-ready";
const RESPONSE_BATCH: &str = "exact-batch";
const RESPONSE_ERROR: &str = "exact-error";

pub struct BrowserCanonicalExactExecutor {
    transport: JsValue,
    ready: bool,
    in_flight: bool,
    queued: VecDeque<CanonicalExactResult>,
}

impl BrowserCanonicalExactExecutor {
    pub fn new(
        transport_factory: JsValue,
        seed: i64,
        authored_bytes: Vec<u8>,
        provisional_bytes: Vec<u8>,
        diagnostic_bytes: Vec<u8>,
    ) -> Result<Self, String> {
        Self::new_with_visual_assets(
            transport_factory,
            seed,
            authored_bytes,
            Vec::new(),
            provisional_bytes,
            diagnostic_bytes,
            "mclone-original",
            "textured",
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_visual_assets(
        transport_factory: JsValue,
        seed: i64,
        authored_bytes: Vec<u8>,
        reference_bytes: Vec<u8>,
        provisional_bytes: Vec<u8>,
        diagnostic_bytes: Vec<u8>,
        visual_profile: &str,
        texture_presentation: &str,
    ) -> Result<Self, String> {
        let factory = transport_factory
            .dyn_into::<Function>()
            .map_err(|_| "runtime exact Worker transport factory is not callable".to_owned())?;
        let transport = factory.call0(&JsValue::UNDEFINED).map_err(js_message)?;
        for name in ["post", "poll", "terminate"] {
            method(&transport, name)?;
        }
        let frame = Object::new();
        set_string(&frame, "kind", FRAME_INIT)?;
        set_string(&frame, "seed", &seed.to_string())?;
        set_string(&frame, "visualProfile", visual_profile)?;
        set_string(&frame, "texturePresentation", texture_presentation)?;
        let authored = Uint8Array::from(authored_bytes.as_slice());
        let reference = Uint8Array::from(reference_bytes.as_slice());
        let provisional = Uint8Array::from(provisional_bytes.as_slice());
        let diagnostic = Uint8Array::from(diagnostic_bytes.as_slice());
        set_value(&frame, "authoredBytes", authored.as_ref())?;
        set_value(&frame, "referenceBytes", reference.as_ref())?;
        set_value(&frame, "provisionalBytes", provisional.as_ref())?;
        set_value(&frame, "diagnosticBytes", diagnostic.as_ref())?;
        let transfer = Array::new();
        transfer.push(&authored.buffer());
        transfer.push(&reference.buffer());
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

impl CanonicalExactExecutor for BrowserCanonicalExactExecutor {
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

impl Drop for BrowserCanonicalExactExecutor {
    fn drop(&mut self) {
        if let Ok(terminate) = method(&self.transport, "terminate") {
            let _ = terminate.call0(&self.transport);
        }
    }
}

pub fn canonical_mesh_batch_encoded_bytes(batch: CanonicalMeshBatch) -> Result<Vec<u8>, String> {
    encode_canonical_batch(&CanonicalEncodedBatch {
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
                surface_columns: admission.surface_columns,
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
    })
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
                surface_columns: admission.surface_columns,
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
