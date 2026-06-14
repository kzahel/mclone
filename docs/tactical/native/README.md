# Native Tactical Docs

Native-first Rust implementation tacticals live here, separated from the legacy TypeScript/browser tactical archive in `docs/tactical/`.

Use zero-padded numeric prefixes for new native tactical docs: `000-topic.md`, `001-next-topic.md`, and so on. Keep one active implementation slice per doc. Parent sequencing checklists are allowed when they keep the native workstream focused; mark them clearly as parent docs and add every new native tactical to this index.

| Doc | Status | Purpose |
|---|---|---|
| [`000-native-render-bringup.md`](000-native-render-bringup.md) | completed | `winit`/`wgpu` clear-frame bring-up, headless PNG capture, and the Minecraft coordinate contract. |
| [`001-one-generated-chunk-render.md`](001-one-generated-chunk-render.md) | completed | One generated chunk artifact, visible-face meshing, flat-color render path, and headless chunk capture. |
| [`002-multichunk-camera.md`](002-multichunk-camera.md) | completed | Small multi-chunk terrain area, neighbor-aware culling, fixed headless overview, and minimal camera controls. |
| [`003-native-ts-parity-roadmap.md`](003-native-ts-parity-roadmap.md) | active parent | Ordered native roadmap from current scaffolding to roughly the TypeScript engine capability horizon. |
| [`004-canonical-chunk-snapshot-protocol.md`](004-canonical-chunk-snapshot-protocol.md) | completed | Canonical native chunk snapshot data and first logical chunk-interest / chunk-publication protocol messages. |
| [`005-local-integrated-client-server.md`](005-local-integrated-client-server.md) | completed | Local integrated server, client runtime replica, in-process transport, and native rendering from client snapshots. |
| [`006-wasm-browser-build-smoke.md`](006-wasm-browser-build-smoke.md) | completed | Browser/WASM smoke for the Rust web shell, runtime protocol path, and WebGPU capability probe. |
| [`007-chunk-interest-status-scheduler.md`](007-chunk-interest-status-scheduler.md) | completed | Server-side chunk holders, status slots, coalesced interest scheduling, and unload publication. |
| [`008-native-persistence-and-residency.md`](008-native-persistence-and-residency.md) | completed | Chunk holder residency, dirty save queue, native filesystem snapshot store, and reload through the server path. |
| [`009-dedicated-server-and-remote-transport.md`](009-dedicated-server-and-remote-transport.md) | completed | Native TCP request/response transport, dedicated server loop, and native client remote chunk loading. |
| [`010-browser-runtime-parity.md`](010-browser-runtime-parity.md) | completed | Browser runtime harness, serialized loopback host adapter, movement/unload smoke, and WebGPU gate. |
| [`011-block-registry-and-asset-source.md`](011-block-registry-and-asset-source.md) | completed | Resource locations, native/web asset sources, blockstate asset index, and current terrain block-state registry. |
| [`012-model-baking-and-atlas.md`](012-model-baking-and-atlas.md) | completed | Blockstate variants, model parent/texture resolution, baked face facts, and deterministic texture atlas planning. |
| [`013-vanilla-section-meshing.md`](013-vanilla-section-meshing.md) | completed | Textured section meshes from client snapshots, baked model faces, stitched atlas upload, and native textured capture. |
| [`014-streaming-renderer-and-camera.md`](014-streaming-renderer-and-camera.md) | completed | Section-keyed textured render meshes, GPU section upload cache, and native headless camera scenarios. |
