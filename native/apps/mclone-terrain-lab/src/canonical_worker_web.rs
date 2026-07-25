use js_sys::{Array, Object, Reflect, Uint8Array};
use mclone_terrain_view::CanonicalTerrainVisibility;
use mclone_worldgen::terrain_preview::TerrainPreviewProfile;
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};

use crate::{
    canonical_batch_codec::{
        CanonicalEncodedAdmission, CanonicalEncodedBatch, encode_canonical_batch,
    },
    canonical_mailbox_web::{
        CANONICAL_SHARED_RESULT_TRANSPORT_KIND, mark_canonical_shared_result_failed,
        publish_canonical_shared_result,
    },
    canonical_mesh::{CanonicalMeshBatch, CanonicalMeshCoordinate, CanonicalMeshSession},
    canonical_terrain_stage,
    visual_assets::load_terrain_lab_visual_assets,
};

const FRAME_INIT: &str = "init";
const FRAME_BEGIN: &str = "begin";
const FRAME_COMPILE: &str = "compile";
const RESPONSE_READY: &str = "ready";
const RESPONSE_BEGAN: &str = "began";
const RESPONSE_BATCH: &str = "batch";
const RESPONSE_STALE: &str = "stale";
const RESPONSE_ERROR: &str = "error";

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = canonicalTerrainWorkerInitFrame)]
pub fn canonical_terrain_worker_init_frame(
    epoch: u32,
    authored_bytes: Uint8Array,
    reference_bytes: Uint8Array,
    provisional_bytes: Uint8Array,
    diagnostic_bytes: Uint8Array,
    visual_profile: String,
    texture_presentation: String,
    seed: String,
    profile: String,
    stage: String,
) -> Result<JsValue, JsValue> {
    let frame = Object::new();
    set(&frame, "kind", &JsValue::from_str(FRAME_INIT))?;
    set_u32(&frame, "epoch", epoch)?;
    set(&frame, "authoredBytes", authored_bytes.as_ref())?;
    set(&frame, "referenceBytes", reference_bytes.as_ref())?;
    set(&frame, "provisionalBytes", provisional_bytes.as_ref())?;
    set(&frame, "diagnosticBytes", diagnostic_bytes.as_ref())?;
    set_string(&frame, "visualProfile", &visual_profile)?;
    set_string(&frame, "texturePresentation", &texture_presentation)?;
    set_string(&frame, "seed", &seed)?;
    set_string(&frame, "profile", &profile)?;
    set_string(&frame, "stage", &stage)?;
    Ok(frame.into())
}

#[wasm_bindgen(js_name = canonicalTerrainWorkerBeginFrame)]
pub fn canonical_terrain_worker_begin_frame(
    epoch: u32,
    coordinates_json: String,
    water_visible: bool,
    vegetation_visible: bool,
    cache_enabled: bool,
) -> Result<JsValue, JsValue> {
    let frame = Object::new();
    set(&frame, "kind", &JsValue::from_str(FRAME_BEGIN))?;
    set_u32(&frame, "epoch", epoch)?;
    set_string(&frame, "coordinatesJson", &coordinates_json)?;
    set_bool(&frame, "waterVisible", water_visible)?;
    set_bool(&frame, "vegetationVisible", vegetation_visible)?;
    set_bool(&frame, "cacheEnabled", cache_enabled)?;
    Ok(frame.into())
}

#[wasm_bindgen(js_name = canonicalTerrainWorkerCompileFrame)]
pub fn canonical_terrain_worker_compile_frame(
    epoch: u32,
    coordinates_json: String,
) -> Result<JsValue, JsValue> {
    let frame = Object::new();
    set(&frame, "kind", &JsValue::from_str(FRAME_COMPILE))?;
    set_u32(&frame, "epoch", epoch)?;
    set_string(&frame, "coordinatesJson", &coordinates_json)?;
    Ok(frame.into())
}

#[wasm_bindgen(js_name = CanonicalTerrainWorkerActor)]
#[derive(Default)]
pub struct CanonicalTerrainWorkerActor {
    session: Option<CanonicalMeshSession>,
    active_epoch: u32,
}

#[wasm_bindgen(js_class = CanonicalTerrainWorkerActor)]
impl CanonicalTerrainWorkerActor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[wasm_bindgen(js_name = handleMessage)]
    pub fn handle_message(&mut self, frame: JsValue) -> CanonicalTerrainWorkerDispatch {
        let epoch = u32_property(&frame, "epoch").unwrap_or_default();
        match self.handle_message_inner(&frame) {
            Ok(dispatch) => dispatch,
            Err(reason) => {
                mark_canonical_shared_result_failed(&frame);
                error_dispatch(epoch, reason)
            }
        }
    }
}

impl CanonicalTerrainWorkerActor {
    fn handle_message_inner(
        &mut self,
        frame: &JsValue,
    ) -> Result<CanonicalTerrainWorkerDispatch, String> {
        let kind = string_property(frame, "kind")?;
        let epoch = u32_property(frame, "epoch")?;
        match kind.as_str() {
            FRAME_INIT => {
                let seed_text = string_property(frame, "seed")?;
                let seed = seed_text.trim().parse::<i64>().map_err(|error| {
                    format!("invalid signed 64-bit seed {seed_text:?}: {error}")
                })?;
                let profile =
                    TerrainPreviewProfile::parse_label(&string_property(frame, "profile")?)?;
                let stage = canonical_terrain_stage(&string_property(frame, "stage")?)?;
                let assets = load_terrain_lab_visual_assets(
                    bytes_property(frame, "authoredBytes")?,
                    bytes_property(frame, "referenceBytes")?,
                    bytes_property(frame, "provisionalBytes")?,
                    bytes_property(frame, "diagnosticBytes")?,
                    &string_property(frame, "visualProfile")?,
                    &string_property(frame, "texturePresentation")?,
                )?;
                self.session = Some(CanonicalMeshSession::new(
                    profile,
                    seed,
                    stage,
                    assets.terrain.catalog,
                ));
                self.active_epoch = epoch;
                response_dispatch(RESPONSE_READY, epoch)
            }
            FRAME_BEGIN => {
                let coordinates = coordinates_property(frame)?;
                let session = self.session.as_mut().ok_or_else(|| {
                    "canonical terrain Worker actor is not initialized".to_owned()
                })?;
                self.active_epoch = epoch;
                session.begin(
                    coordinates,
                    CanonicalTerrainVisibility {
                        water: bool_property(frame, "waterVisible")?,
                        vegetation: bool_property(frame, "vegetationVisible")?,
                    },
                    bool_property(frame, "cacheEnabled")?,
                );
                let dispatch = response_dispatch(RESPONSE_BEGAN, epoch)?;
                set_u32(
                    &dispatch.message_object,
                    "rawCacheChunks",
                    session.raw_cache_chunks().min(u32::MAX as usize) as u32,
                )
                .map_err(js_message)?;
                set_f64(
                    &dispatch.message_object,
                    "rawCacheBytes",
                    session.raw_cache_bytes() as f64,
                )
                .map_err(js_message)?;
                Ok(dispatch)
            }
            FRAME_COMPILE => {
                if epoch != self.active_epoch {
                    return response_dispatch(RESPONSE_STALE, epoch);
                }
                let coordinates = coordinates_property(frame)?;
                let session = self.session.as_mut().ok_or_else(|| {
                    "canonical terrain Worker actor is not initialized".to_owned()
                })?;
                let batch = session.compile_batch(&coordinates)?;
                batch_dispatch(frame, epoch, batch)
            }
            other => Err(format!(
                "unsupported canonical terrain Worker frame {other:?}"
            )),
        }
    }
}

#[wasm_bindgen(js_name = CanonicalTerrainWorkerDispatch)]
pub struct CanonicalTerrainWorkerDispatch {
    message_object: Object,
    transferables: Array,
}

#[wasm_bindgen(js_class = CanonicalTerrainWorkerDispatch)]
impl CanonicalTerrainWorkerDispatch {
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> JsValue {
        self.message_object.clone().into()
    }

    #[wasm_bindgen(getter)]
    pub fn transferables(&self) -> Array {
        self.transferables.clone()
    }
}

#[wasm_bindgen(js_name = CanonicalTerrainWorkerResponse)]
pub struct CanonicalTerrainWorkerResponse {
    message: JsValue,
}

#[wasm_bindgen(js_class = CanonicalTerrainWorkerResponse)]
impl CanonicalTerrainWorkerResponse {
    pub(crate) fn raw_message(&self) -> &JsValue {
        &self.message
    }

    pub fn decode(message: JsValue) -> Result<CanonicalTerrainWorkerResponse, JsValue> {
        let kind = string_property(&message, "kind").map_err(js_error)?;
        if !matches!(
            kind.as_str(),
            RESPONSE_READY | RESPONSE_BEGAN | RESPONSE_BATCH | RESPONSE_STALE | RESPONSE_ERROR
        ) {
            return Err(js_error(format!(
                "unsupported canonical terrain Worker response {kind:?}"
            )));
        }
        u32_property(&message, "epoch").map_err(js_error)?;
        Ok(Self { message })
    }

    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> Result<String, JsValue> {
        string_property(&self.message, "kind").map_err(js_error)
    }

    #[wasm_bindgen(getter)]
    pub fn epoch(&self) -> Result<u32, JsValue> {
        u32_property(&self.message, "epoch").map_err(js_error)
    }

    #[wasm_bindgen(getter)]
    pub fn message(&self) -> Result<String, JsValue> {
        optional_string_property(&self.message, "message")
            .map(|message| message.unwrap_or_default())
            .map_err(js_error)
    }

    #[wasm_bindgen(getter, js_name = rawCacheChunks)]
    pub fn raw_cache_chunks(&self) -> Result<u32, JsValue> {
        optional_u32_property(&self.message, "rawCacheChunks")
            .map(|value| value.unwrap_or_default())
            .map_err(js_error)
    }

    #[wasm_bindgen(getter, js_name = rawCacheBytes)]
    pub fn raw_cache_bytes(&self) -> Result<f64, JsValue> {
        optional_f64_property(&self.message, "rawCacheBytes")
            .map(|value| value.unwrap_or_default())
            .map_err(js_error)
    }

    #[wasm_bindgen(getter, js_name = generationMs)]
    pub fn generation_ms(&self) -> Result<f64, JsValue> {
        f64_property(&self.message, "generationMs").map_err(js_error)
    }

    #[wasm_bindgen(getter, js_name = presentationMs)]
    pub fn presentation_ms(&self) -> Result<f64, JsValue> {
        f64_property(&self.message, "presentationMs").map_err(js_error)
    }

    #[wasm_bindgen(getter, js_name = meshMs)]
    pub fn mesh_ms(&self) -> Result<f64, JsValue> {
        f64_property(&self.message, "meshMs").map_err(js_error)
    }

    #[wasm_bindgen(getter, js_name = packMs)]
    pub fn pack_ms(&self) -> Result<f64, JsValue> {
        f64_property(&self.message, "packMs").map_err(js_error)
    }

    #[wasm_bindgen(getter, js_name = transferMs)]
    pub fn transfer_ms(&self) -> Result<f64, JsValue> {
        f64_property(&self.message, "transferMs").map_err(js_error)
    }

    #[wasm_bindgen(getter, js_name = deduplicatedTargetChunks)]
    pub fn deduplicated_target_chunks(&self) -> Result<u32, JsValue> {
        u32_property(&self.message, "deduplicatedTargetChunks").map_err(js_error)
    }

    #[wasm_bindgen(getter, js_name = admissionCount)]
    pub fn admission_count(&self) -> Result<u32, JsValue> {
        Ok(admissions_property(&self.message)?.length())
    }

    #[wasm_bindgen(js_name = admissionChunkX)]
    pub fn admission_chunk_x(&self, index: u32) -> Result<i32, JsValue> {
        i32_property(&admission(&self.message, index)?, "chunkX").map_err(js_error)
    }

    #[wasm_bindgen(js_name = admissionChunkZ)]
    pub fn admission_chunk_z(&self, index: u32) -> Result<i32, JsValue> {
        i32_property(&admission(&self.message, index)?, "chunkZ").map_err(js_error)
    }

    #[wasm_bindgen(js_name = admissionFingerprint)]
    pub fn admission_fingerprint(&self, index: u32) -> Result<String, JsValue> {
        string_property(&admission(&self.message, index)?, "fingerprint").map_err(js_error)
    }

    #[wasm_bindgen(js_name = admissionRawCacheHit)]
    pub fn admission_raw_cache_hit(&self, index: u32) -> Result<bool, JsValue> {
        bool_property(&admission(&self.message, index)?, "rawCacheHit").map_err(js_error)
    }

    #[wasm_bindgen(js_name = admissionRetainedDependencyChunks)]
    pub fn admission_retained_dependency_chunks(&self, index: u32) -> Result<u32, JsValue> {
        u32_property(
            &admission(&self.message, index)?,
            "retainedDependencyChunks",
        )
        .map_err(js_error)
    }

    #[wasm_bindgen(js_name = admissionPackedSections)]
    pub fn admission_packed_sections(&self, index: u32) -> Result<Uint8Array, JsValue> {
        Reflect::get(
            &admission(&self.message, index)?,
            &JsValue::from_str("packedSections"),
        )?
        .dyn_into::<Uint8Array>()
        .map_err(|_| js_error("canonical Worker packedSections is not a Uint8Array"))
    }
}

fn response_dispatch(kind: &str, epoch: u32) -> Result<CanonicalTerrainWorkerDispatch, String> {
    let message_object = Object::new();
    set(&message_object, "kind", &JsValue::from_str(kind)).map_err(js_message)?;
    set_u32(&message_object, "epoch", epoch).map_err(js_message)?;
    Ok(CanonicalTerrainWorkerDispatch {
        message_object,
        transferables: Array::new(),
    })
}

fn error_dispatch(epoch: u32, reason: String) -> CanonicalTerrainWorkerDispatch {
    let dispatch = response_dispatch(RESPONSE_ERROR, epoch)
        .expect("creating a plain canonical Worker error object cannot fail");
    set_string(&dispatch.message_object, "message", &reason)
        .expect("setting a plain canonical Worker error message cannot fail");
    dispatch
}

fn batch_dispatch(
    frame: &JsValue,
    epoch: u32,
    batch: CanonicalMeshBatch,
) -> Result<CanonicalTerrainWorkerDispatch, String> {
    let transfer_started = js_sys::Date::now();
    let encoded_batch = CanonicalEncodedBatch {
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
                fingerprint: admission.requested.fingerprint,
                raw_cache_hit: admission.requested.raw_cache_hit,
                retained_dependency_chunks: admission
                    .requested
                    .retained_dependency_chunks
                    .min(u32::MAX as usize) as u32,
                packed_sections: admission.packed_sections,
            })
            .collect(),
    };
    let mut encoded = encode_canonical_batch(&encoded_batch)?;
    let publication =
        publish_canonical_shared_result(frame, epoch, &mut encoded, transfer_started)?;
    let dispatch = response_dispatch(RESPONSE_BATCH, epoch)?;
    set_string(
        &dispatch.message_object,
        "transportKind",
        CANONICAL_SHARED_RESULT_TRANSPORT_KIND,
    )
    .map_err(js_message)?;
    set_u32(
        &dispatch.message_object,
        "sharedResultByteLength",
        publication.byte_length,
    )
    .map_err(js_message)?;
    set_u32(
        &dispatch.message_object,
        "sharedResultBufferCapacityBytes",
        publication.capacity,
    )
    .map_err(js_message)?;
    set_bool(
        &dispatch.message_object,
        "sharedResultOverflow",
        publication.overflow,
    )
    .map_err(js_message)?;
    set_f64(
        &dispatch.message_object,
        "transferMs",
        publication.transfer_ms,
    )
    .map_err(js_message)?;
    set(
        &dispatch.message_object,
        "sharedResultBuffer",
        publication.buffer.as_ref(),
    )
    .map_err(js_message)?;
    Ok(dispatch)
}

fn coordinates_property(frame: &JsValue) -> Result<Vec<CanonicalMeshCoordinate>, String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Coordinate {
        chunk_x: i32,
        chunk_z: i32,
    }

    serde_json::from_str::<Vec<Coordinate>>(&string_property(frame, "coordinatesJson")?)
        .map(|coordinates| {
            coordinates
                .into_iter()
                .map(|coordinate| {
                    CanonicalMeshCoordinate::new(coordinate.chunk_x, coordinate.chunk_z)
                })
                .collect()
        })
        .map_err(|error| format!("invalid canonical Worker coordinates: {error}"))
}

fn admission(message: &JsValue, index: u32) -> Result<JsValue, JsValue> {
    let admissions = admissions_property(message)?;
    if index >= admissions.length() {
        return Err(js_error(format!(
            "canonical Worker admission index {index} is out of bounds"
        )));
    }
    Ok(admissions.get(index))
}

fn admissions_property(message: &JsValue) -> Result<Array, JsValue> {
    Reflect::get(message, &JsValue::from_str("admissions"))?
        .dyn_into::<Array>()
        .map_err(|_| js_error("canonical Worker admissions is not an Array"))
}

fn bytes_property(value: &JsValue, name: &str) -> Result<Vec<u8>, String> {
    Reflect::get(value, &JsValue::from_str(name))
        .map_err(js_message)?
        .dyn_into::<Uint8Array>()
        .map(|bytes| bytes.to_vec())
        .map_err(|_| format!("canonical Worker property {name:?} is not a Uint8Array"))
}

fn string_property(value: &JsValue, name: &str) -> Result<String, String> {
    Reflect::get(value, &JsValue::from_str(name))
        .map_err(js_message)?
        .as_string()
        .ok_or_else(|| format!("canonical Worker property {name:?} is not a string"))
}

fn optional_string_property(value: &JsValue, name: &str) -> Result<Option<String>, String> {
    let property = Reflect::get(value, &JsValue::from_str(name)).map_err(js_message)?;
    if property.is_undefined() || property.is_null() {
        Ok(None)
    } else {
        property
            .as_string()
            .map(Some)
            .ok_or_else(|| format!("canonical Worker property {name:?} is not a string"))
    }
}

fn bool_property(value: &JsValue, name: &str) -> Result<bool, String> {
    Reflect::get(value, &JsValue::from_str(name))
        .map_err(js_message)?
        .as_bool()
        .ok_or_else(|| format!("canonical Worker property {name:?} is not a boolean"))
}

fn f64_property(value: &JsValue, name: &str) -> Result<f64, String> {
    Reflect::get(value, &JsValue::from_str(name))
        .map_err(js_message)?
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("canonical Worker property {name:?} is not a finite number"))
}

fn optional_f64_property(value: &JsValue, name: &str) -> Result<Option<f64>, String> {
    let property = Reflect::get(value, &JsValue::from_str(name)).map_err(js_message)?;
    if property.is_undefined() || property.is_null() {
        Ok(None)
    } else {
        property
            .as_f64()
            .filter(|value| value.is_finite())
            .map(Some)
            .ok_or_else(|| format!("canonical Worker property {name:?} is not a finite number"))
    }
}

fn u32_property(value: &JsValue, name: &str) -> Result<u32, String> {
    let value = f64_property(value, name)?;
    if value.fract() != 0.0 || !(0.0..=f64::from(u32::MAX)).contains(&value) {
        return Err(format!(
            "canonical Worker property {name:?} is not an unsigned 32-bit integer"
        ));
    }
    Ok(value as u32)
}

fn optional_u32_property(value: &JsValue, name: &str) -> Result<Option<u32>, String> {
    let property = Reflect::get(value, &JsValue::from_str(name)).map_err(js_message)?;
    if property.is_undefined() || property.is_null() {
        return Ok(None);
    }
    let number = property
        .as_f64()
        .ok_or_else(|| format!("canonical Worker property {name:?} is not a number"))?;
    if !number.is_finite()
        || number.fract() != 0.0
        || !(0.0..=f64::from(u32::MAX)).contains(&number)
    {
        return Err(format!(
            "canonical Worker property {name:?} is not an unsigned 32-bit integer"
        ));
    }
    Ok(Some(number as u32))
}

fn i32_property(value: &JsValue, name: &str) -> Result<i32, String> {
    let value = f64_property(value, name)?;
    if value.fract() != 0.0 || !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&value) {
        return Err(format!(
            "canonical Worker property {name:?} is not a signed 32-bit integer"
        ));
    }
    Ok(value as i32)
}

fn set(object: &Object, name: &str, value: &JsValue) -> Result<(), JsValue> {
    Reflect::set(object, &JsValue::from_str(name), value).map(|_| ())
}

fn set_string(object: &Object, name: &str, value: &str) -> Result<(), JsValue> {
    set(object, name, &JsValue::from_str(value))
}

fn set_bool(object: &Object, name: &str, value: bool) -> Result<(), JsValue> {
    set(object, name, &JsValue::from_bool(value))
}

fn set_f64(object: &Object, name: &str, value: f64) -> Result<(), JsValue> {
    set(object, name, &JsValue::from_f64(value))
}

fn set_u32(object: &Object, name: &str, value: u32) -> Result<(), JsValue> {
    set_f64(object, name, f64::from(value))
}

fn js_message(value: JsValue) -> String {
    value
        .as_string()
        .unwrap_or_else(|| "browser object operation failed".to_owned())
}

fn js_error(message: impl Into<String>) -> JsValue {
    js_sys::Error::new(&message.into()).into()
}
