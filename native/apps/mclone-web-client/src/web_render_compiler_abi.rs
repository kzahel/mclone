//! External shared-buffer ABI used by both browser render Wasm instances.
//!
//! Main Wasm owns and polls the resident arenas. Worker Wasm reads the input
//! arena and publishes packed results into the response arena. JavaScript has
//! one matching authored copy in `www/mclone-render-compiler-abi.js`; the host
//! integration test locks the two copies together.

use wasm_bindgen::JsValue;

pub(crate) const RENDER_COMPILER_SHARED_RESULT_CONTROL_WORDS: u32 = 4;
pub(crate) const RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX: u32 = 0;
pub(crate) const RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX: u32 = 1;
pub(crate) const RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX: u32 = 2;
pub(crate) const RENDER_COMPILER_SHARED_RESULT_PENDING: i32 = 1;
pub(crate) const RENDER_COMPILER_SHARED_RESULT_COMPLETE: i32 = 2;
pub(crate) const RENDER_COMPILER_SHARED_RESULT_OVERFLOW: i32 = 3;
pub(crate) const RENDER_COMPILER_SHARED_RESULT_FAILED: i32 = 4;
pub(crate) const RENDER_COMPILER_SHARED_INPUT_CONTROL_WORDS: u32 = 4;
pub(crate) const RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX: u32 = 0;
pub(crate) const RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX: u32 = 1;
pub(crate) const RENDER_COMPILER_SHARED_INPUT_CAPACITY_INDEX: u32 = 2;
pub(crate) const RENDER_COMPILER_SHARED_INPUT_READY: i32 = 2;
pub(crate) const RENDER_COMPILER_DEFAULT_SHARED_RESULT_CAPACITY: u32 = 16 * 1024 * 1024;
pub(crate) const RENDER_COMPILER_DEFAULT_SHARED_INPUT_CAPACITY: u32 = 1024 * 1024;
pub(crate) const RENDER_COMPILER_TRANSPORT_KIND: &str = "shared-result-buffer";

pub(crate) fn atomic_store(
    control: &js_sys::Int32Array,
    index: u32,
    value: i32,
) -> Result<(), String> {
    js_sys::Atomics::store(control, index, value)
        .map(|_| ())
        .map_err(|error| format!("failed to store render-compile control word {index}: {error:?}"))
}

pub(crate) fn atomic_load(control: &js_sys::Int32Array, index: u32) -> Result<i32, String> {
    js_sys::Atomics::load(control, index)
        .map_err(|error| format!("failed to load render-compile control word {index}: {error:?}"))
}

pub(crate) fn atomic_notify(control: &js_sys::Int32Array, index: u32) -> Result<(), String> {
    js_sys::Atomics::notify_with_count(control, index, 1)
        .map(|_| ())
        .map_err(|error| format!("failed to notify render-compile control word {index}: {error:?}"))
}

pub(crate) fn shared_memory_supported() -> bool {
    let global = js_sys::global();
    global_is_function(&global, "SharedArrayBuffer")
        && global_method_is_function(&global, "Atomics", "load")
        && global_method_is_function(&global, "Atomics", "store")
        && global_method_is_function(&global, "Atomics", "notify")
}

fn global_is_function(global: &JsValue, name: &str) -> bool {
    js_sys::Reflect::get(global, &JsValue::from_str(name))
        .ok()
        .is_some_and(|value| value.is_function())
}

fn global_method_is_function(global: &JsValue, object_name: &str, method_name: &str) -> bool {
    js_sys::Reflect::get(global, &JsValue::from_str(object_name))
        .ok()
        .and_then(|object| js_sys::Reflect::get(&object, &JsValue::from_str(method_name)).ok())
        .is_some_and(|value| value.is_function())
}
