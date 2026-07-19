//! External shared-buffer ABI used by both browser render Wasm instances.
//!
//! Main Wasm owns and polls the resident arenas. Worker Wasm reads the input
//! arena and publishes packed results into the response arena. JavaScript has
//! one matching authored copy in `www/mclone-render-compiler-abi.js`; the host
//! integration test locks the two copies together.

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
