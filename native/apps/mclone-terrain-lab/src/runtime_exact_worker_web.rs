use js_sys::{Object, Reflect, Uint8Array};
use mclone_terrain_view::{
    CanonicalMeshCoordinate, CanonicalMeshFrontier, CanonicalMeshSession,
    CanonicalNaturalTreePresentation, CanonicalTerrainStage, CanonicalTerrainVisibility,
    canonical_mesh_batch_encoded_bytes,
};
use mclone_worldgen::terrain_preview::TerrainPreviewProfile;
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};

use crate::visual_assets::load_terrain_lab_visual_assets;

const FRAME_INIT: &str = "exact-init";
const FRAME_COMPILE: &str = "exact-compile";
const RESPONSE_READY: &str = "exact-ready";
const RESPONSE_BATCH: &str = "exact-batch";
const RESPONSE_ERROR: &str = "exact-error";

#[wasm_bindgen(js_name = TerrainRuntimeExactWorkerActor)]
#[derive(Default)]
pub struct TerrainRuntimeExactWorkerActor {
    session: Option<CanonicalMeshSession>,
}

#[wasm_bindgen(js_class = TerrainRuntimeExactWorkerActor)]
impl TerrainRuntimeExactWorkerActor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[wasm_bindgen(js_name = handleMessage)]
    pub fn handle_message(&mut self, frame: JsValue) -> TerrainRuntimeExactWorkerDispatch {
        match self.handle_message_inner(&frame) {
            Ok(message) => TerrainRuntimeExactWorkerDispatch { message },
            Err(reason) => TerrainRuntimeExactWorkerDispatch {
                message: error_message(reason),
            },
        }
    }
}

impl TerrainRuntimeExactWorkerActor {
    fn handle_message_inner(&mut self, frame: &JsValue) -> Result<Object, String> {
        match string_property(frame, "kind")?.as_str() {
            FRAME_INIT => {
                let seed_text = string_property(frame, "seed")?;
                let seed = seed_text
                    .parse::<i64>()
                    .map_err(|error| format!("invalid exact Worker seed {seed_text:?}: {error}"))?;
                let profile =
                    TerrainPreviewProfile::parse_label(&string_property(frame, "profile")?)?;
                let assets = load_terrain_lab_visual_assets(
                    bytes_property(frame, "authoredBytes")?,
                    bytes_property(frame, "referenceBytes")?,
                    bytes_property(frame, "provisionalBytes")?,
                    bytes_property(frame, "diagnosticBytes")?,
                    &string_property(frame, "visualProfile")?,
                    &string_property(frame, "texturePresentation")?,
                )?;
                let mut session = CanonicalMeshSession::new(
                    profile,
                    seed,
                    CanonicalTerrainStage::FinalFeatures,
                    assets.terrain.catalog,
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
                let encoded =
                    canonical_mesh_batch_encoded_bytes(session.compile_batch(&requested)?)?;
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

#[wasm_bindgen(js_name = TerrainRuntimeExactWorkerDispatch)]
pub struct TerrainRuntimeExactWorkerDispatch {
    message: Object,
}

#[wasm_bindgen(js_class = TerrainRuntimeExactWorkerDispatch)]
impl TerrainRuntimeExactWorkerDispatch {
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> JsValue {
        self.message.clone().into()
    }
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
