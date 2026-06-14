# mclone native workspace

This workspace is the clean Rust track for the native-first engine with a web target kept alive from the beginning. It lives inside the existing repository so it can reuse the project oracle fixtures, reference notes, asset extraction scripts, and the TypeScript engine as prior art.

The durable direction is documented in [`../docs/native-rewrite-roadmap.md`](../docs/native-rewrite-roadmap.md). The short version: native desktop is the primary development target, web/WASM is an early compatibility gate, and the TypeScript implementation is now legacy/reference rather than the main engine direction.

The Rust workspace is quarantined from the TypeScript implementation:

- Rust crates may read shared docs, fixtures, and extracted assets.
- Rust crates should not import, execute, or depend on TypeScript runtime code.
- Minecraft Java `1.17.1` and the existing oracle fixtures remain the correctness target.
- The browser target should be kept alive early, but native desktop is the main development loop.

## Initial crate boundaries

- `mclone-core`: pure data/model/math primitives.
- `mclone-protocol`: versioned host/client messages and encodings, with no sockets.
- `mclone-net`: transport adapters and local/native/web channel boundaries, with no game rules.
- `mclone-server`: authoritative runtime, chunk scheduling, ticks, sessions, and persistence hooks.
- `mclone-client`: client replica, prediction/interpolation, and render-facing presentation state.
- `mclone-worldgen`: vanilla 1.17.1 terrain, biome, surface, carver, feature, and structure generation.
- `mclone-light`: packed sky/block lighting.
- `mclone-mesh`: chunk meshing into renderer-ready buffers.
- `mclone-assets`: blockstate/model/texture/NBT asset loading.
- `mclone-render`: `wgpu` renderer.

Applications:

- `mclone-native-client`: desktop client.
- `mclone-dedicated-server`: headless native server.
- `mclone-web-client`: browser/WASM client shell.

Start narrow: port one oracle-backed worldgen layer before adding heavyweight renderer, networking, async, or WASM dependencies.
