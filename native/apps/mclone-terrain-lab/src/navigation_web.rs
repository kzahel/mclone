use mclone_view_control::{
    ContactEvent, ContactPurpose, ViewPoint, ViewportMetrics, WorldViewIntent, WorldViewMode,
    WorldViewProjection, WorldViewState,
};
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

use crate::navigation::{TerrainLabNavigationController, TerrainLabNavigationSnapshot};

#[wasm_bindgen(js_name = TerrainLabNavigationSession)]
#[derive(Default)]
pub struct TerrainLabNavigationSession {
    controller: TerrainLabNavigationController,
}

#[wasm_bindgen(js_class = TerrainLabNavigationSession)]
impl TerrainLabNavigationSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sync(
        &mut self,
        center_x: i32,
        center_z: i32,
        blocks_across: u32,
        view: String,
        projection: String,
        yaw_radians: f64,
        pitch_radians: f64,
    ) -> Result<(), JsValue> {
        self.controller.sync(WorldViewState {
            mode: view_mode(&view)?,
            focus_x: f64::from(center_x),
            focus_z: f64::from(center_z),
            blocks_across: f64::from(blocks_across),
            yaw_radians,
            pitch_radians,
            projection: projection_kind(&projection)?,
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = pointerDown)]
    pub fn pointer_down(
        &mut self,
        pointer_id: u32,
        x: f64,
        y: f64,
        purpose: String,
        time_seconds: f64,
        width: f64,
        height: f64,
    ) -> Result<TerrainLabNavigationUpdate, JsValue> {
        Ok(self
            .controller
            .contact(ContactEvent::Down {
                id: u64::from(pointer_id),
                position: ViewPoint::new(x, y),
                purpose: contact_purpose(&purpose)?,
                time_seconds,
                viewport: ViewportMetrics::new(width, height),
            })
            .into())
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = pointerMove)]
    pub fn pointer_move(
        &mut self,
        pointer_id: u32,
        x: f64,
        y: f64,
        time_seconds: f64,
        width: f64,
        height: f64,
    ) -> TerrainLabNavigationUpdate {
        self.controller
            .contact(ContactEvent::Moved {
                id: u64::from(pointer_id),
                position: ViewPoint::new(x, y),
                time_seconds,
                viewport: ViewportMetrics::new(width, height),
            })
            .into()
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = pointerUp)]
    pub fn pointer_up(
        &mut self,
        pointer_id: u32,
        x: f64,
        y: f64,
        time_seconds: f64,
        width: f64,
        height: f64,
    ) -> TerrainLabNavigationUpdate {
        self.controller
            .contact(ContactEvent::Up {
                id: u64::from(pointer_id),
                position: ViewPoint::new(x, y),
                time_seconds,
                viewport: ViewportMetrics::new(width, height),
            })
            .into()
    }

    pub fn cancel(&mut self) -> TerrainLabNavigationUpdate {
        self.controller.contact(ContactEvent::CancelAll).into()
    }

    pub fn wheel(
        &mut self,
        log_delta: f64,
        anchor_x: f64,
        anchor_y: f64,
        width: f64,
        height: f64,
    ) -> TerrainLabNavigationUpdate {
        self.controller
            .apply(WorldViewIntent::AnchoredZoom {
                log_delta,
                normalized_anchor: ViewPoint::new(anchor_x, anchor_y),
                viewport: ViewportMetrics::new(width, height),
            })
            .into()
    }

    pub fn arrow(&mut self, key: String) -> TerrainLabNavigationUpdate {
        self.controller.arrow(&key).into()
    }

    #[wasm_bindgen(js_name = panFraction)]
    pub fn pan_fraction(
        &mut self,
        horizontal: f64,
        depth: f64,
        fraction: f64,
    ) -> TerrainLabNavigationUpdate {
        self.controller
            .pan_fraction(horizontal, depth, fraction)
            .into()
    }

    #[wasm_bindgen(js_name = zoomFactor)]
    pub fn zoom_factor(&mut self, factor: f64) -> TerrainLabNavigationUpdate {
        self.controller
            .zoom_factor(factor, ViewPoint::default(), ViewportMetrics::new(1.0, 1.0))
            .into()
    }
}

#[wasm_bindgen(js_name = TerrainLabNavigationUpdate)]
pub struct TerrainLabNavigationUpdate {
    snapshot: TerrainLabNavigationSnapshot,
}

impl From<TerrainLabNavigationSnapshot> for TerrainLabNavigationUpdate {
    fn from(snapshot: TerrainLabNavigationSnapshot) -> Self {
        Self { snapshot }
    }
}

#[wasm_bindgen(js_class = TerrainLabNavigationUpdate)]
impl TerrainLabNavigationUpdate {
    #[wasm_bindgen(getter, js_name = centerX)]
    pub fn center_x(&self) -> i32 {
        self.snapshot.center_x
    }

    #[wasm_bindgen(getter, js_name = centerZ)]
    pub fn center_z(&self) -> i32 {
        self.snapshot.center_z
    }

    #[wasm_bindgen(getter, js_name = blocksAcross)]
    pub fn blocks_across(&self) -> u32 {
        self.snapshot.blocks_across
    }

    #[wasm_bindgen(getter, js_name = yawRadians)]
    pub fn yaw_radians(&self) -> f64 {
        self.snapshot.yaw_radians
    }

    #[wasm_bindgen(getter, js_name = pitchRadians)]
    pub fn pitch_radians(&self) -> f64 {
        self.snapshot.pitch_radians
    }

    #[wasm_bindgen(getter, js_name = viewChanged)]
    pub fn view_changed(&self) -> bool {
        self.snapshot.view_changed
    }

    #[wasm_bindgen(getter)]
    pub fn signal(&self) -> String {
        self.snapshot.signal.label().to_owned()
    }
}

fn view_mode(value: &str) -> Result<WorldViewMode, JsValue> {
    match value {
        "map" => Ok(WorldViewMode::Map),
        "3d" => Ok(WorldViewMode::Orbit),
        other => Err(js_error(format!(
            "unsupported Terrain Lab navigation view {other:?}"
        ))),
    }
}

fn projection_kind(value: &str) -> Result<WorldViewProjection, JsValue> {
    match value {
        "orthographic" => Ok(WorldViewProjection::Orthographic),
        "perspective" => Ok(WorldViewProjection::Perspective),
        other => Err(js_error(format!(
            "unsupported Terrain Lab navigation projection {other:?}"
        ))),
    }
}

fn contact_purpose(value: &str) -> Result<ContactPurpose, JsValue> {
    match value {
        "default" => Ok(ContactPurpose::ViewDefault),
        "pan" => Ok(ContactPurpose::Pan),
        "orbit" => Ok(ContactPurpose::Orbit),
        other => Err(js_error(format!(
            "unsupported Terrain Lab contact purpose {other:?}"
        ))),
    }
}

fn js_error(message: impl Into<String>) -> JsValue {
    js_sys::Error::new(&message.into()).into()
}
