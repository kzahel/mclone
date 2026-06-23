// 067 Stage 5: single source for the render-compile SharedArrayBuffer ring ABI on the
// JavaScript side. The render-compile worker (the producer, mclone-render-compiler-worker.js)
// and the app/smoke glue (the consumers, via mclone-render-compiler-shared.js) all import
// these constants instead of hand-duplicating them — collapsing the three former hand-synced
// JS copies into one.
//
// The Rust main-wasm reader keeps its own copy of the numeric constants below in
// native/apps/mclone-web-client/src/web_canvas.rs (it cannot import this JS file at compile
// time). The two authored copies are locked by the host test
// native/apps/mclone-web-client/tests/render_compiler_abi_lock.rs, which parses both files and
// fails on any drift — so a mismatched status-word index becomes a failing test instead of a
// silent SAB corruption.
//
// Deploy note: each importer's own URL is cache-busted with `?v=<version>`, but a static
// `import` of this module cannot carry that query string, so a deploy that bumps the asset
// version must rely on HTTP cache revalidation of this bare URL (smoke/dev leave the version
// unset, so they are unaffected).
export const RENDER_COMPILER_TRANSPORT_KIND = "shared-result-buffer";
export const RENDER_COMPILER_SHARED_RESULT_CONTROL_WORDS = 4;
export const RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX = 0;
export const RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX = 1;
export const RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX = 2;
export const RENDER_COMPILER_SHARED_RESULT_PENDING = 1;
export const RENDER_COMPILER_SHARED_RESULT_COMPLETE = 2;
export const RENDER_COMPILER_SHARED_RESULT_OVERFLOW = 3;
export const RENDER_COMPILER_SHARED_RESULT_FAILED = 4;
export const RENDER_COMPILER_SHARED_INPUT_CONTROL_WORDS = 4;
export const RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX = 0;
export const RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX = 1;
export const RENDER_COMPILER_SHARED_INPUT_CAPACITY_INDEX = 2;
export const RENDER_COMPILER_SHARED_INPUT_READY = 2;
export const RENDER_COMPILER_DEFAULT_SHARED_RESULT_CAPACITY = 16 * 1024 * 1024;
export const RENDER_COMPILER_DEFAULT_SHARED_INPUT_CAPACITY = 1 * 1024 * 1024;
