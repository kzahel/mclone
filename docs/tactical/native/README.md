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
