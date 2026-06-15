# 010: Browser Runtime Parity

Status: completed.

## Purpose

Move the Rust browser target beyond a one-command compatibility smoke. The browser app still stays thin, but it now exercises a browser-shaped runtime path through the same client replica, protocol messages, server runtime, and transport boundary used by native.

This is not the full web client. It is the web compatibility gate for runtime ownership before renderer assets and streaming work start.

## Scope

Landed:

- `mclone-web-client` owns a small `WebRuntime` harness around `ClientRuntime`
- browser loopback host adapter uses `LocalTransport` plus protocol byte encode/decode for every command and update
- browser smoke performs two chunk-interest steps:
  - load chunk `(0, 0)`
  - move interest to `(1, 0)`
  - verify `(0, 0)` unloads and `(1, 0)` loads
- packed WASM report now includes movement/unload and protocol-codec roundtrip facts
- Playwright smoke asserts the richer runtime report and still probes WebGPU

Kept out:

- WebSocket/WebTransport adapter
- long-lived browser remote session
- browser storage persistence
- WASM worker threading or `SharedArrayBuffer`
- WebGPU canvas rendering from Rust

## Architecture

The browser target now has this local runtime shape:

```text
browser smoke page
  -> raw WASM export
  -> WebRuntime
  -> ClientRuntime
  -> WebLoopbackHost
  -> protocol byte codec
  -> LocalTransport
  -> IntegratedServer
```

The loopback host intentionally encodes and decodes the protocol messages even though the server is still in-process. That keeps the browser path honest about the same serialized command/update boundary a future WebSocket or WebTransport adapter will use, while avoiding premature browser networking decisions.

## Validation

Required checks:

- `cargo test --workspace`
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown`
- `pnpm native:web:smoke`

The Playwright smoke saves its screenshot to `/tmp/mclone-native-web-smoke.png`.

## Follow-Up

Proceed to `011-block-registry-and-asset-source.md`: establish the vanilla block-state/asset source boundary before textured meshing and atlas work.
