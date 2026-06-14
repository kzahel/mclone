# 003: Native Path To TypeScript Parity

Status: active parent checklist.

This is the parent ordering plan for the native Rust workstream from the current native state to roughly the capability level of the TypeScript engine. It is not a single implementation slice. Use it to choose the next high-impact tactical and to avoid app-local cleanup that does not move ownership, parity, validation, or platform reach forward.

## Current Native State

Landed:

- native `winit`/`wgpu` window and headless clear capture
- headless generated terrain PNG validation
- one generated chunk and small multi-chunk flat-color render path
- minimal camera controls
- Rust crate skeletons for client, server, protocol, net, worldgen, mesh, render, assets, light

Still missing compared with the TypeScript engine:

- native client/server/protocol ownership is not real yet
- `mclone-native-client` still generates chunks directly and meshes them immediately
- chunk snapshots are not yet canonical packed runtime facts
- no native chunk-status scheduler, persistence, local/remote transport, or web runtime smoke
- no committed WASM/browser build gate beyond the current scaffold check
- no vanilla asset/model/texture pipeline in the Rust renderer
- no native lighting, liquids, movement, entities, or broad decorated-world parity

## Target Horizon

"Around where the TypeScript version got to" means this native track should reach:

- singleplayer through the same client/server/protocol shape intended for multiplayer
- native headless and windowed validation rendering from a client replica, not direct worldgen
- generated overworld chunks with status-aware scheduling and persistence hooks
- vanilla-shaped block-state ids, packed sections, chunk snapshots, light facts, and deltas
- textured vanilla chunk rendering through assets, blockstates, model baking, atlas, render layers, and frustum/camera ownership
- broad recognizable overworld worldgen: terrain, surface, classic carvers, major decoration families, ores/underground features, and first structures
- initial lighting, live light deltas, liquid ticks, basic movement/prediction, and entity/passive-mob baseline
- early web/WASM compatibility gate kept alive from the same crates
- browser client target that boots through the same runtime/protocol shape instead of a separate demo path
- native presentation shape that does not block a desktop OpenXR path

This is a parity horizon, not a pledge to port every TypeScript tactical one-for-one. The TypeScript implementation is the oracle/scaffold/reference target; Rust should group work by durable engine boundaries.

## Ordering Rules

- Runtime ownership before renderer polish: the renderer should consume client-replica facts, not generate world data.
- Data/protocol contracts before transports: local, dedicated, and web transports should move the same logical messages.
- Native-first, web-kept-alive: keep web compiling/smoking early at subsystem boundaries, but do not develop a second full product in lockstep.
- Web/WASM has two gates: an early build/boot smoke so constraints shape APIs, then a later real browser runtime once transport/render facts stabilize.
- Validation is part of the slice: every render/runtime slice needs a headless or app smoke that exercises the new path.
- Avoid throwaway cache/refactor work unless it removes a bypass or lands inside a real ownership boundary.
- Prefer porting from current TypeScript and Java source where parity matters; drop TypeScript-specific browser orchestration when Rust/native gives us a cleaner runtime shape.

## Planned Tactical Sequence

Expect about 19 implementation tacticals after this parent roadmap before native is broadly comparable to the current TypeScript engine. Some worldgen parity follow-through may split further only when fixtures prove a concrete miss.

| Doc | Theme | Lands | Gate |
|---|---|---|---|
| `004-canonical-chunk-snapshot-protocol.md` | data/protocol | `BlockStateId`, packed section or section-ready snapshot facts, first chunk-interest and chunk-snapshot messages | unit tests over snapshot roundtrip and protocol data shape |
| `005-local-integrated-client-server.md` | runtime spine | `IntegratedServer`, `ClientRuntime`, in-process transport, client chunk replica, native app renders from client facts | native headless chunk PNG comes from `ClientRuntime`, not direct worldgen |
| `006-wasm-browser-build-smoke.md` | web compatibility | `mclone-web-client` compiles to WASM, boots in a browser shell, creates WebGPU or reports a stable fallback, and exercises a tiny protocol/client path | `cargo check --target wasm32-unknown-unknown` plus browser smoke where available |
| `007-chunk-interest-status-scheduler.md` | server runtime | interest-driven chunk requests, holder/status slots, async/coalesced generation, publishable chunk events | tests for duplicate request coalescing and status ordering |
| `008-native-persistence-and-residency.md` | storage/runtime | chunk residency, dirty/save queue shape, filesystem adapter scaffold, reload/resume hook | save/load roundtrip of generated chunk snapshots |
| `009-dedicated-server-and-remote-transport.md` | networking | `mclone-dedicated-server` serves the same protocol over a simple native transport; native client can join remotely | local two-process smoke or loopback integration |
| `010-browser-runtime-parity.md` | web runtime | browser client uses the same `ClientRuntime` and protocol messages against local or remote host adapters, with browser storage/transport constraints visible | browser runtime smoke from client replica facts |
| `011-block-registry-and-asset-source.md` | assets/data | vanilla block-state id registry, file/web asset sources, extracted asset loading boundaries | registry and native/web asset fixture tests |
| `012-model-baking-and-atlas.md` | renderer assets | blockstate/model parse, model baking, texture atlas, material/render-layer facts | baked model/atlas tests plus native and browser textured-block smoke where available |
| `013-vanilla-section-meshing.md` | mesh/render | client snapshot to section meshes using baked models, cutout/liquid layer separation, neighbor culling | headless rendered chunk from client snapshot with real textures |
| `014-streaming-renderer-and-camera.md` | renderer runtime | visible section upload cache, frustum, render invalidation, debug camera/headless scenario runner | multi-step native headless captures and browser/native window smoke |
| `015-decoration-framework-foundation.md` | worldgen parity | configured/decorated feature framework, first trees/plants/water features from Rust | oracle/unit coverage plus visible decorated terrain capture |
| `016-biome-feature-breadth.md` | worldgen parity | biome table coverage, tree family breadth, ores, underground extras, surface vegetation/water families | selected fixture matrix and visual probes |
| `017-structures-foundation.md` | worldgen parity | status-aware structure starts/references, first true structure, template/block-entity scaffolding | server-backed fixture diff for first structure slice |
| `018-lighting-pipeline.md` | lighting | `DataLayer`, light sections, solver/service boundary, initial light facts, renderer consumption | light fixture tests and non-fullbright terrain capture |
| `019-live-light-and-liquid-updates.md` | simulation/runtime | light deltas, block/liquid dirty updates, pending liquid ticks, water flow baseline | update/delta tests and runtime smoke |
| `020-player-movement-and-collision.md` | gameplay/runtime | shared body/collision view, sequenced input commands, native prediction/reconciliation baseline | local and remote movement integration tests |
| `021-entities-and-passive-mobs.md` | gameplay/render | entity sections, snapshots/deltas, passive spawn baseline, first render path | generated entity fixture and visible entity capture |
| `022-native-xr-and-parity-consolidation.md` | platform/perf | OpenXR presentation smoke shape, perf counters, fly-through benchmark, parity gap report | native smoke plus tracked perf/parity report |

## Immediate Focus

The next implementation tactical should be `004-canonical-chunk-snapshot-protocol.md`.

That slice should not build a local app cache. It should define the chunk facts and protocol messages that `005-local-integrated-client-server.md` will use immediately:

- chunk position, status, revision, and section facts live in shared crates
- chunk snapshots are suitable for client replicas, storage, meshing, and transport
- protocol messages distinguish chunk interest from chunk publication
- tests prove the data model is not renderer-owned and not worldgen-owned

Then `005` wires the first local integrated loop and removes the direct worldgen-to-render shortcut from the native client.

`006` is the first web/WASM compatibility gate. It should stay thin, but it must be real enough to catch API shapes that assume native threads, blocking filesystem access, or native-only `wgpu` setup.

## Deferral Notes

- Do not start broad renderer asset polish before the native app is rendering from a client replica.
- Do not start native lighting before snapshots can carry chunk facts through the client/server boundary.
- Do not start movement/prediction before `ClientRuntime` owns a client world and collision-relevant chunk facts.
- Do not build OpenXR on the current direct worldgen render path; wait until renderer presentation is separated from world ownership.
- Do not postpone all web work until the end; keep the WASM/browser target compiling and booting at the early runtime and renderer boundaries.
- Do not port every TypeScript worldgen follow-through in order. Port grouped families against oracle fixtures and split only when a concrete parity failure needs a focused tactical.
