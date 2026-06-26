# 085: Web Host-Mode Convergence

Status: active. Slice 1 landed shared command/update exchange accounting
without forcing native desktop or Android TCP paths into async. Slice 2 landed
an enum/free-function remote exchange resolver consumed by the browser
WebSocket path. Remaining work is naming cleanup and fuller web remote wiring.

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
- [ ] Keep worker/WebSocket connection setup, JS promises, and browser
  diagnostics in the web app crate.
- [ ] Update smoke/app entrypoints so remote WebSocket play is reachable from
  the playable browser app, not only smoke exports.

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
