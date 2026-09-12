use std::collections::HashMap;

use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

const AUTHORED_PACK_REQUEST_ID: u32 = 2;
const FALLBACK_PACK_REQUEST_ID: u32 = 3;
const DIAGNOSTIC_PACK_REQUEST_ID: u32 = 4;

const RESOURCE_REQUESTS: [(u32, &str); 3] = [
    (
        AUTHORED_PACK_REQUEST_ID,
        "/first-party-packs/mclone-authored.pbp",
    ),
    (
        FALLBACK_PACK_REQUEST_ID,
        "/first-party-packs/mclone-generated-fallback.pbp",
    ),
    (
        DIAGNOSTIC_PACK_REQUEST_ID,
        "/first-party-packs/mclone-diagnostic-missing.pbp",
    ),
];

pub(crate) struct InitialAssetPacks {
    pub reference: Vec<u8>,
    pub authored: Vec<u8>,
    pub fallback: Vec<u8>,
    pub diagnostic: Vec<u8>,
}

/// Raw browser facts used by Rust-owned initial presentation policy.
///
/// The browser adapter probes its environment; it does not decide which
/// engine UI or diagnostic surface those facts should enable.
#[wasm_bindgen]
pub struct WebHostCapabilities {
    touch_input_available: bool,
    viewport_width_css_pixels: f64,
}

#[wasm_bindgen]
impl WebHostCapabilities {
    #[wasm_bindgen(constructor)]
    pub fn new(touch_input_available: bool, viewport_width_css_pixels: f64) -> Self {
        Self {
            touch_input_available,
            viewport_width_css_pixels: viewport_width_css_pixels.max(0.0),
        }
    }
}

impl WebHostCapabilities {
    pub(crate) fn touch_input_available(&self) -> bool {
        self.touch_input_available
    }

    pub(crate) fn initial_debug_overlay_visible(&self) -> bool {
        !self.touch_input_available && self.viewport_width_css_pixels >= 681.0
    }
}

/// Opaque response bag for Rust-authored browser bootstrap resource requests.
///
/// TypeScript creates this bag and fills it with fetched bytes by request ID;
/// only Rust resolves those IDs to engine resource roles.
#[wasm_bindgen]
pub struct WebBootstrapResources {
    responses: HashMap<u32, Vec<u8>>,
}

#[wasm_bindgen]
impl WebBootstrapResources {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    #[wasm_bindgen(js_name = add)]
    pub fn add(&mut self, request_id: u32, bytes: js_sys::Uint8Array) -> Result<(), JsValue> {
        if !RESOURCE_REQUESTS
            .iter()
            .any(|(expected, _)| *expected == request_id)
        {
            return Err(JsValue::from_str(
                "unknown browser bootstrap resource request",
            ));
        }
        if self.responses.insert(request_id, bytes.to_vec()).is_some() {
            return Err(JsValue::from_str(
                "duplicate browser bootstrap resource response",
            ));
        }
        Ok(())
    }
}

impl Default for WebBootstrapResources {
    fn default() -> Self {
        Self::new()
    }
}

impl WebBootstrapResources {
    pub(crate) fn into_initial_asset_packs(mut self) -> Result<InitialAssetPacks, JsValue> {
        let mut take = |request_id, label| {
            self.responses.remove(&request_id).ok_or_else(|| {
                JsValue::from_str(&format!("missing browser bootstrap resource for {label}"))
            })
        };
        Ok(InitialAssetPacks {
            // Empty bytes represent an absent optional pack at the browser ABI.
            reference: Vec::new(),
            authored: take(AUTHORED_PACK_REQUEST_ID, "authored asset pack")?,
            fallback: take(FALLBACK_PACK_REQUEST_ID, "fallback asset pack")?,
            diagnostic: take(DIAGNOSTIC_PACK_REQUEST_ID, "diagnostic asset pack")?,
        })
    }
}

pub(crate) fn browser_resource_plan() -> Result<JsValue, JsValue> {
    let plan = js_sys::Object::new();
    let resources = js_sys::Array::new();
    for (request_id, url) in RESOURCE_REQUESTS {
        let request = js_sys::Object::new();
        js_sys::Reflect::set(
            &request,
            &JsValue::from_str("requestId"),
            &JsValue::from_f64(f64::from(request_id)),
        )?;
        js_sys::Reflect::set(&request, &JsValue::from_str("url"), &JsValue::from_str(url))?;
        resources.push(&request);
    }
    js_sys::Reflect::set(&plan, &JsValue::from_str("resources"), &resources.into())?;
    Ok(plan.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_ids_are_unique_and_urls_are_root_relative() {
        let mut ids = RESOURCE_REQUESTS
            .iter()
            .map(|(request_id, _)| *request_id)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), RESOURCE_REQUESTS.len());
        assert!(
            RESOURCE_REQUESTS
                .iter()
                .all(|(_, url)| url.starts_with('/'))
        );
    }

    #[test]
    fn public_bootstrap_requests_only_first_party_packs() {
        assert!(
            RESOURCE_REQUESTS
                .iter()
                .all(|(_, url)| url.starts_with("/first-party-packs/"))
        );
    }
}
