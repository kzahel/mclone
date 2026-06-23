// 070 Stage 1: single source for the server-worker SharedArrayBuffer ring ABI on the
// JavaScript side. The integrated-server worker (mclone-integrated-server-worker.js) and the
// stateless worldgen/light job worker (mclone-server-job-worker.js) both import these
// control-word constants instead of hand-duplicating them — collapsing the two former
// hand-synced JS copies (which had diverging `RUNNER_SHARED_*` vs `SHARED_*` prefixes and an
// asymmetric status enum) into one. This mirrors the render lane's
// mclone-render-compiler-abi.js (067 Stage 5).
//
// The Rust main-wasm reader keeps its own copy of the numeric constants below in
// native/apps/mclone-web-client/src/web_server_worker.rs (it cannot import this JS file at
// compile time). The two authored copies are locked by the host test
// native/apps/mclone-web-client/tests/runner_shared_abi_lock.rs, which parses both files and
// fails on any drift — so a mismatched status code or word index becomes a failing test
// instead of a silent SAB corruption (a server job that silently never completes or decodes
// garbage, in a worker, in the browser).
//
// Deploy note: each importer's own URL is cache-busted with `?v=<version>`, but a static
// `import` of this module cannot carry that query string, so a deploy that bumps the asset
// version must rely on HTTP cache revalidation of this bare URL (smoke/dev leave the version
// unset, so they are unaffected).

// 4 i32 control words (status, request bytes, response bytes, reserved) × 4 bytes each.
export const RUNNER_SHARED_CONTROL_BYTES = 16;
export const RUNNER_SHARED_STATUS_INDEX = 0;
export const RUNNER_SHARED_REQUEST_BYTES_INDEX = 1;
export const RUNNER_SHARED_RESPONSE_BYTES_INDEX = 2;
// Status enum written into the status word. PENDING is armed by the Rust runner before posting
// (only meaningful to a poller); the worker overwrites it with COMPLETE on success or FAILED on
// error. All three exist on both sides so the lock test covers the full enum.
export const RUNNER_SHARED_STATUS_PENDING = 1;
export const RUNNER_SHARED_STATUS_COMPLETE = 2;
export const RUNNER_SHARED_STATUS_FAILED = -1;
