# 005: Local Integrated Client/Server

Status: completed.

Wire the first real native singleplayer runtime loop so the native client renders from client-replica chunk facts, not by calling worldgen directly. This is the ownership boundary the later scheduler, persistence, dedicated server, web target, movement, lighting, and XR work should build on.

## Standing

Depends on:

- [`003-native-ts-parity-roadmap.md`](003-native-ts-parity-roadmap.md)
- [`004-canonical-chunk-snapshot-protocol.md`](004-canonical-chunk-snapshot-protocol.md)
- [`../architecture.md`](../architecture.md)
- [`../protocol.md`](../protocol.md)
- [`../runtime-data-model.md`](../runtime-data-model.md)

Native implementation references:

- `native/crates/mclone-server/src/integrated.rs`
- `native/crates/mclone-server/src/scheduler.rs`
- `native/crates/mclone-client/src/lib.rs`
- `native/crates/mclone-net/src/lib.rs`

Reference Minecraft source to read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/server/IntegratedServer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java`

## Core Decision

Native singleplayer is a local authoritative server plus a client world replica. It is not a renderer shortcut.

For this slice the local transport can be an in-process queue and generation can stay synchronous. The important behavior is ownership:

- `IntegratedServer` owns seed/worldgen and handles `ClientCommand`.
- `ClientRuntime` owns chunk interest and hydrated client-replica snapshots.
- `mclone-native-client` sends interest, drains updates, then meshes client-owned snapshots.
- the renderer and native app do not call `generate_overworld_surface_chunk(...)`.

## Scope

1. Add `mclone_server::IntegratedServer`.
2. Add `mclone_client::ClientRuntime` with a minimal client chunk replica.
3. Add `mclone_net::LocalTransport` for in-process `ClientCommand` / `ServerUpdate` queues.
4. Replace native app/headless chunk scene build with:
   - client sends `SetChunkInterest`
   - local transport drains into integrated server
   - server publishes chunk snapshots
   - client applies snapshots
   - native app meshes client snapshots
5. Keep unload handling in the client replica, even if the first app path only loads once.
6. Add tests for command/update flow and native scene construction.

## Out Of Scope

- async generation
- chunk-status futures
- persistence
- remote sockets
- browser boot
- lighting
- entities
- movement/prediction
- texture/model renderer work
- dynamic chunk streaming while the camera moves

## Implementation Order

1. Add `LocalTransport` queues in `mclone-net`.
2. Add `IntegratedServer::handle_command(...)` in `mclone-server`.
3. Add `ClientRuntime::set_chunk_interest(...)` and `ClientRuntime::apply_update(...)` in `mclone-client`.
4. Add a snapshot-to-temporary-mesh-block adapter in the native app.
5. Update `build_scene_mesh(...)` to use the local client/server flow.
6. Remove `mclone-native-client`'s direct dependency on `mclone-worldgen` if nothing else uses it.

## Validation

Required:

```bash
cargo test --workspace
cargo check -p mclone-web-client --target wasm32-unknown-unknown
cargo run -p mclone-native-client -- --headless-chunk /tmp/mclone-native-client-runtime-chunk.png --width 960 --height 640 --chunk-radius 1
git diff --check
```

Inspect `/tmp/mclone-native-client-runtime-chunk.png`. It should remain a nonblank multi-chunk terrain capture, but it must now be fed by the client runtime path.

## Done When

- `mclone-native-client` no longer imports or calls `generate_overworld_surface_chunk`.
- `IntegratedServer` publishes `ServerUpdate::ChunkSnapshot`.
- `ClientRuntime` hydrates and owns chunk snapshots.
- Native window/headless rendering consumes snapshots from `ClientRuntime`.
- Tests prove duplicate local interest does not force a renderer-owned worldgen path.

## Next Slice

After this lands, implement `006-wasm-browser-build-smoke.md` as the first explicit web/WASM runtime gate.
