# 006: WASM Browser Build Smoke

Status: completed.

## Purpose

Keep the web target alive now that native has real chunk facts, protocol messages, local transport, an integrated server, and a client runtime.

This is not the browser runtime port. It is a thin compatibility gate that proves the Rust web shell can:

- build to `wasm32-unknown-unknown`
- boot inside a browser-served page
- instantiate Rust WASM without a bundler
- exercise one local client/server/protocol path
- create a WebGPU device when available, or report a stable unsupported fallback when it is not

## Implemented

- `mclone-web-client` now builds a raw browser-loadable `.wasm`.
- The exported `mclone_web_runtime_smoke_report` runs:
  - `ClientRuntime::local_integrated`
  - `LocalTransport`
  - `IntegratedServer`
  - one `ChunkInterest` command at `(0, 0)` with radius `0`
  - one `ServerUpdate::ChunkSnapshot`
  - client replica hydration for the center chunk
- The browser smoke page in `native/apps/mclone-web-client/www/` loads the `.wasm`, calls the exported report, decodes its stable bit layout, and probes `navigator.gpu`.
- `pnpm native:web:smoke` builds the WASM target, serves the smoke page from localhost, runs Playwright Chrome, asserts the decoded runtime report, and saves a screenshot to `/tmp/mclone-native-web-smoke.png`.

## Validation

Required gates:

```text
cargo test --workspace
cargo check -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
```

If Chrome/WebGPU is unavailable on a host, the smoke page still treats `navigator.gpu` absence, adapter absence, and device request failure as stable fallback states. Failure to instantiate WASM or run the runtime report is still a hard failure.

## Scope Boundaries

In scope:

- raw WASM build/instantiate path
- one tiny client/protocol/server runtime path
- WebGPU capability detection from a real browser page
- browser smoke command suitable for manual and later CI use

Out of scope:

- renderer canvas integration
- `wgpu` surface creation in WASM
- browser storage
- WebSocket/WebTransport adapters
- WASM workers, shared memory, or `SharedArrayBuffer`
- chunk streaming beyond the one center chunk

## Design Notes

The crate uses `#![deny(unsafe_code)]` rather than `#![forbid(unsafe_code)]` because Rust 2024 treats `no_mangle` as an unsafe attribute. The only allowed unsafe surface in this crate is the exported C ABI symbol required for raw browser WebAssembly instantiation. Runtime logic remains safe Rust and lives in the shared native crates.

The Java reference has no direct counterpart for this browser platform shell. The runtime ownership shape remains aligned with the previously checked vanilla files:

- `IntegratedServer` owns authoritative world/chunk facts.
- `ClientChunkCache` / `ClientLevel` motivate the client-side replica shape.

## Next

Proceed to [`007-chunk-interest-status-scheduler.md`](007-chunk-interest-status-scheduler.md): replace direct generate-on-interest behavior with the first status-aware chunk scheduling boundary.
