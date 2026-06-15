# 009: Dedicated Server And Remote Transport

Status: completed.

## Purpose

Turn the native dedicated server from a placeholder into a process that can serve the same client/server/protocol path used by the local integrated runtime.

This slice proves the native multiplayer boundary before richer scheduling, renderer streaming, browser runtime parity, movement, or prediction. It intentionally keeps the transport simple: one native TCP connection carries one `ClientCommand`, and the server replies with the resulting `ServerUpdate` batch.

## Scope

Landed:

- protocol-owned binary encoding/decoding for `ClientCommand` and `ServerUpdate`
- native-only TCP framing helpers in `mclone-net`
- `mclone-dedicated-server` listen loop backed by `IntegratedServer`
- native client `--remote-addr HOST:PORT` option for headless/window chunk loading
- loopback coverage for command/request/update flow
- native headless chunk capture from a remote dedicated server path

Kept out:

- long-lived client sessions
- auth/login/version negotiation beyond the current protocol version constant
- async networking or QUIC/WebTransport
- browser remote transport
- chunk delta compression, snapshot streaming, or backpressure

## Architecture

The crate boundaries remain:

- `mclone_protocol`: logical messages plus byte codec. No sockets.
- `mclone_net`: transport framing and native TCP request/response helper. No game rules.
- `mclone_server`: authoritative runtime that turns `ClientCommand` into `ServerUpdate`.
- `mclone_client`: client replica and host identity.
- apps: CLI parsing, process ownership, and validation entrypoints.

For this slice, the native socket protocol is:

```text
client connection:
  u32 little-endian command byte length
  encoded ClientCommand

server response:
  u32 little-endian update count
  repeated:
    u32 little-endian update byte length
    encoded ServerUpdate
```

This is intentionally a milestone wire shape, not a frozen public protocol. It is still useful because it forces snapshots to cross a real process/transport boundary instead of relying on in-process ownership.

## Validation

Required checks:

- `cargo test --workspace`
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown`
- native remote headless capture saved under `/tmp`
- `pnpm native:web:smoke`

The remote headless capture is important because it verifies the renderer still consumes a `ClientRuntime` populated by protocol updates, not direct worldgen.

## Follow-Up

The next native tactical remains `010-browser-runtime-parity.md`: keep the browser target compiling and move it toward the same runtime/protocol shape, while leaving production web transport for a later adapter slice.
