# 085: Web Host-Mode Convergence

Status: active. Slices 1-2 landed shared command/update exchange accounting
and remote-dedicated reconnect/resync policy without forcing native desktop or
Android TCP paths into async. Slice 3 wired the playable browser app to the
dedicated WebSocket path. Remaining work is naming cleanup plus deeper web
runtime/render convergence.

## Purpose

Make web/WASM consume the same host-mode policy as native desktop and flat
Android where the policy is truly shared, while preserving browser-only async
worker and WebSocket mechanics.

Host mode means the authoritative runtime shape:

- local integrated
- remote dedicated
- future P2P/session host

It is not the display/input platform. Desktop flat, flat Android, web, and XR
should differ by adapter, not by private command/update semantics.

## Current State

- Native desktop and flat Android use `mclone-app-runtime::host_mode` for
  remote dedicated dispatch/resync policy.
- Native desktop and flat Android compose
  `NativeSingleViewSceneRuntime<S>` for local/remote single-view scene state.
- Web uses the shared `SingleViewRuntime` and `RuntimeExchange`, but keeps a
  private `WebRuntimeHost` enum for inline, worker, and remote WebSocket modes.
- Browser worker startup and command exchange are async because they use
  `postMessage`, promises, and callback-driven response delivery.
- Browser WebSocket connect/exchange is async because `web_sys::WebSocket`
  opens and receives messages through browser events.

## Target Shape

Do not make native paths async just to match the browser. Instead:

- shared app-runtime owns host-mode-neutral command/update accounting,
  diagnostics-derived transport-drained facts, chunk-view resync prep, and
  remote-dedicated reconnect/resync policy where transport shape permits it
- native TCP/Android TCP implement the blocking session adapter
- web worker/WebSocket implement an async adapter that returns the same shared
  `RuntimeExchange` facts
- web-specific deferred update draining remains browser-local until there is a
  shared nonblocking host-driver contract

## Implementation Slices

### Slice 1 - Shared Exchange Accounting

- [x] Add shared helpers in `mclone-app-runtime::host_mode` for command
  exchanges, deferred command sends, pending-update drains, and diagnostics
  queue-drained checks.
- [x] Rewire `WebRuntimeHost` to construct `RuntimeExchange` through those
  helpers instead of app-local struct literals.
- [x] Keep browser async mechanics in `mclone-web-client`.
- [x] Validate app-runtime, web-client, native-client, and wasm build gates.

Recorded Slice 1 result:

- Added shared `command_exchange`, `deferred_command_exchange`,
  `update_drain_exchange`, `diagnostics_command_update_queues_drained`, and
  `diagnostics_worker_exchange_drained` helpers to
  `mclone-app-runtime::host_mode`.
- Rewired native remote dedicated update application to use the shared command
  exchange helper.
- Rewired `WebRuntimeHost` so worker/WebSocket command exchange, worker deferred
  send, and pending-update drains construct `RuntimeExchange` through shared
  helpers.
- Rewired `WebIntegratedServerRunner::exchange_command(...)` to use the shared
  worker-drained diagnostic predicate.
- Kept `WebRuntimeHost`, browser worker startup, JS promises, `postMessage`,
  and `web_sys::WebSocket` mechanics in `mclone-web-client`.

Validation after Slice 1:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
pnpm native:web:build
cargo fmt --manifest-path native/Cargo.toml --all --check
git diff --check
```

### Slice 2 - Async Adapter Shape

- [x] Define a web-friendly host adapter boundary that can represent async
  command exchange without requiring async native callers.
- [x] Decide whether this is a trait, enum+free functions, or a small adapter
  object. Avoid `async_trait` unless the ergonomics clearly justify allocation
  and feature-gating costs.
- [x] Move shared remote dedicated command apply/resync policy behind that
  boundary where web can consume it.

Recorded Slice 2 result:

- Added `RemoteDedicatedExchangeReport<E>` and
  `resolve_remote_dedicated_exchange_report(...)` to
  `mclone-app-runtime::host_mode`.
- Refactored native blocking `dispatch_remote_dedicated_command(...)` to use
  the same resolver before doing its TCP reconnect/resync send.
- Added browser `WebSocketServerSession::reconnect()` while keeping WebSocket
  construction, event callbacks, and promises in `mclone-web-client`.
- Routed web remote WebSocket async chunk-view/gameplay commands through the
  shared resolver. Successful exchanges apply through shared app-runtime
  command exchange policy; failed exchanges use shared resync prep and then
  reconnect/resync with awaited WebSocket mechanics.
- Kept worker-integrated web on the existing async worker path; it is local
  integrated, not remote dedicated.

Validation after Slice 2:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:web:build
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
```

### Slice 3 - Web Remote Wiring

- [ ] Replace remaining web-only host-mode naming with shared names where behavior is
  aligned.
- [x] Keep worker/WebSocket connection setup, JS promises, and browser
  diagnostics in the web app crate.
- [x] Update smoke/app entrypoints so remote WebSocket play is reachable from
  the playable browser app, not only smoke exports.

Recorded Slice 3 result:

- Added `mclone_web_create_remote_chunk_render_session(...)` so the browser app
  can construct the normal `WebChunkRenderSession` against a dedicated
  WebSocket server.
- Added `?remoteWsUrl=ws://...` app selection in `mclone-web-app.ts`. Without
  the parameter the app still uses the worker-integrated local host.
- Made the live camera streaming frame path remote-aware: WebSocket hosts await
  `SetChunkView` and camera-pose exchanges, while worker-integrated mode keeps
  the existing deferred command path. The common render/doorbell tail remains
  shared.
- Extended app/smoke diagnostics with the runtime kind, client host mode,
  requested remote URL, and render dirty/in-flight counts. Remote boot treats a
  visible loaded view with no runner queues or compile jobs as playable even if
  strict section-level render quiescence still has dirty sections to revisit.
- Updated `native:web:remote-smoke` so `--remote-websocket` launches the
  playable app with a dedicated WebSocket URL, then exercises walking,
  targeting, break/place, no-clip movement, actor rendering, native UI, and
  canvas screenshot validation.
- Fixed the web glue TypeScript emit launcher on Windows by invoking `pnpm`
  through `cmd.exe` when Node's direct spawn cannot resolve the shim.

Validation after Slice 3:

```bash
pnpm native:web:typecheck
pnpm native:web:remote-smoke
pnpm native:web:app-smoke
cargo fmt --manifest-path native/Cargo.toml --all
```

Follow-up:

- Investigate why render-section dirty counts can remain nonzero after the
  visible web view is playable and `renderPendingWork` is false. Keep the new
  diagnostics until the strict section-level idle story is clearer.
- Rename the web host-mode surface where it now aligns with shared
  `host_mode` concepts, without moving browser promises/WebSocket setup into
  shared native crates.

## Validation

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
pnpm native:web:build
git diff --check
```

## Guardrails

- Do not force native TCP, Android TCP, or local integrated runner paths into
  async.
- Do not hide browser worker deferred-drain behavior behind a blocking API.
- Do not make `mclone-app-runtime` depend on `web_sys`, `wasm_bindgen`, or JS
  promise types.
- Keep concrete browser worker/WebSocket setup in `mclone-web-client`.
