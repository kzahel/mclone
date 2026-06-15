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
- canonical packed chunk snapshots and first chunk-interest / chunk-snapshot protocol messages
- local integrated server, client runtime replica, in-process transport, and native rendering from client snapshots
- thin browser/WASM smoke that instantiates the Rust web shell, probes WebGPU, and runs a one-chunk protocol path
- server-side chunk holders, status slots, duplicate request coalescing, and unload publication
- chunk holder residency, dirty save queue, native filesystem snapshot store, and reload through the server path
- native TCP request/response transport, a sequential dedicated server loop, and native client remote chunk loading
- browser runtime harness that exercises serialized client/server protocol updates, interest movement, unloads, and WebGPU smoke
- resource locations, native/web asset sources, blockstate asset indexing, and the current terrain block-state registry
- blockstate variant parsing, block model parent/texture resolution, baked model face facts, and deterministic texture atlas planning
- textured native section mesh generation from client snapshots, stitched atlas upload, and textured headless/window rendering
- section-keyed textured render meshes, keyed GPU upload ownership, and repeatable native headless camera scenarios
- Java-shaped configured/decorated feature execution, starter oak trees, grass/flower patches, and default `Features` chunk snapshots
- biome-keyed native feature profiles with visible oak/birch/spruce tree families, ferns, dead bushes, and `COUNT_EXTRA` placement
- full decorated chunk parity gauntlet for seed `12345`, chunk `0,0`, carvers wired into the native `Features` generation path, first region-backed 3x3 feature spillover pass, batched feature dependency reuse for interest updates, durable scheduler job records, native worker-mailbox execution, split enqueue/poll job publication, worker-local lower-status dependency reuse across adjacent feature jobs, Java-order decorated feature random interleaving, the default underground variety ore blobs through tuff/deepslate, active default ore block families with normal/deepslate targets, Java-shaped taiga spruce/pine tree configs for the target biome, world-backed tree heightmap/water-depth decorators, nested configured-feature decorators for vanilla pine, lower/upper large-fern block support for future double-plant features, Java-shaped taiga `FLOWER_DEFAULT` / `PATCH_GRASS_TAIGA_2`, Java-shaped `OreFeature` `OCEAN_FLOOR_WG` precheck behavior, `CAVE_AIR` preservation for lake cavities, glow-lichen face state, Java-shaped positive worldgen skylight lake restoration, and exact clean scheduler `FEATURES` parity for chunk `(0,0)`
- first Java-shaped native chunk ticket layer: `PLAYER`/`FORCED`/timeout ticket types, numeric holder ticket levels, holder full-status facts, forced chunk retention, stale `UNKNOWN` ticket expiry, and ticket-driven holder load/unload reconciliation

Still missing compared with the TypeScript engine:

- no long-lived remote sessions or browser remote network transport
- no full Java-style distance propagation or scheduler-owned dependency protochunk holder graph; dependency reuse is currently worker-local clean lower-status buffers
- persistence is a temporary snapshot format, not Anvil/NBT or browser storage
- web/WASM has a runtime smoke gate, not browser storage, network transport, or render path
- no true frustum rejection, dirty-only section upload diffs, true liquid renderer, or AO/lighted textured path
- no exact tree placer parity, liquid-visible features, structures, lighting, movement, entities, or broad decorated-world parity

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

Expect about 20 implementation tacticals after this parent roadmap before native is broadly comparable to the current TypeScript engine. Some worldgen parity follow-through may split further only when fixtures prove a concrete miss.

| Doc | Theme | Lands | Gate |
|---|---|---|---|
| [`004-canonical-chunk-snapshot-protocol.md`](004-canonical-chunk-snapshot-protocol.md) | data/protocol | **done** - `BlockStateId`, packed section snapshot facts, first chunk-interest and chunk-snapshot messages | unit tests over snapshot roundtrip and protocol data shape |
| [`005-local-integrated-client-server.md`](005-local-integrated-client-server.md) | runtime spine | **done** - `IntegratedServer`, `ClientRuntime`, in-process transport, client chunk replica, native app renders from client facts | native headless chunk PNG comes from `ClientRuntime`, not direct worldgen |
| [`006-wasm-browser-build-smoke.md`](006-wasm-browser-build-smoke.md) | web compatibility | **done** - `mclone-web-client` compiles/builds to WASM, boots in a browser shell, probes WebGPU, and exercises a one-chunk protocol/client/server path | `cargo check --target wasm32-unknown-unknown` plus `pnpm native:web:smoke` where Chrome is available |
| [`007-chunk-interest-status-scheduler.md`](007-chunk-interest-status-scheduler.md) | server runtime | **done** - interest-driven chunk requests, holder/status slots, coalesced synchronous generation, publishable chunk events | tests for duplicate request coalescing and status ordering |
| [`008-native-persistence-and-residency.md`](008-native-persistence-and-residency.md) | storage/runtime | **done** - chunk residency, dirty/save queue shape, filesystem adapter scaffold, reload/resume hook | save/load roundtrip of generated chunk snapshots |
| [`009-dedicated-server-and-remote-transport.md`](009-dedicated-server-and-remote-transport.md) | networking | **done** - `mclone-dedicated-server` serves the same protocol over a simple native transport; native client can join remotely | local two-process smoke or loopback integration |
| [`010-browser-runtime-parity.md`](010-browser-runtime-parity.md) | web runtime | **done** - browser client uses the same `ClientRuntime` and serialized protocol messages against a browser loopback host adapter, with browser storage/transport constraints visible | browser runtime smoke from client replica facts |
| [`011-block-registry-and-asset-source.md`](011-block-registry-and-asset-source.md) | assets/data | **done** - vanilla-shaped block-state registry scaffold, file/web asset sources, extracted asset loading boundaries | registry and native/web asset fixture tests |
| [`012-model-baking-and-atlas.md`](012-model-baking-and-atlas.md) | renderer assets | **done** - blockstate/model parse, parent texture resolution, baked face facts, and deterministic atlas planning | baked model/atlas tests over extracted vanilla assets |
| [`013-vanilla-section-meshing.md`](013-vanilla-section-meshing.md) | mesh/render | **done** - client snapshot to textured section meshes using baked models, stitched atlas upload, and neighbor culling | headless rendered chunk from client snapshot with real textures |
| [`014-streaming-renderer-and-camera.md`](014-streaming-renderer-and-camera.md) | renderer runtime | **done** - section-keyed textured render meshes, keyed GPU upload ownership, full-set render invalidation, and debug camera/headless scenario runner; true frustum/dirty-only diffs deferred | multi-step native headless captures and browser/native window smoke |
| [`015-decoration-framework-foundation.md`](015-decoration-framework-foundation.md) | worldgen parity | **done** - configured/decorated feature framework, starter trees/plants, feature snapshots, and visible decorated terrain capture; full water/lake parity deferred behind liquid rendering | oracle/unit coverage plus visible decorated terrain capture |
| [`016-biome-feature-breadth.md`](016-biome-feature-breadth.md) | worldgen parity | **done** - chunk-center biome feature profiles, oak/birch/spruce visible families, fern/dead-bush patches, and `COUNT_EXTRA`; exact trees/ores/water fixtures deferred | selected fixture matrix and visual probes |
| [`017-full-decorated-chunk-parity-gauntlet.md`](017-full-decorated-chunk-parity-gauntlet.md) | worldgen parity | **completed milestone** - exact clean `FEATURES` chunk parity for seed `12345`, chunk `(0,0)`; older full-runtime liquid/glow-lichen tails deferred | normal exact clean fixture test plus ignored full-runtime mismatch diagnostic |
| [`018-chunk-ticket-distance-manager.md`](018-chunk-ticket-distance-manager.md) | server runtime/perf | **active** - Java-shaped ticket types, numeric holder levels, full-status facts, forced/stale tickets, and ticket-driven load/unload reconciliation | server tests for player interest tickets, forced retention, stale ticket expiry, and existing interest movement |
| `019-scheduler-owned-dependency-holders.md` | server runtime/perf | distance propagation, lower-status holder halos, scheduler-owned dependency protochunks, and generation reuse counters | movement benchmark plus holder/dependency reuse tests |
| `020-structures-foundation.md` | worldgen parity | status-aware structure starts/references, first true structure, template/block-entity scaffolding | server-backed fixture diff for first structure slice |
| `021-lighting-pipeline.md` | lighting | `DataLayer`, light sections, solver/service boundary, initial light facts, renderer consumption | light fixture tests and non-fullbright terrain capture |
| `022-live-light-and-liquid-updates.md` | simulation/runtime | light deltas, block/liquid dirty updates, pending liquid ticks, water flow baseline | update/delta tests and runtime smoke |
| `023-player-movement-and-collision.md` | gameplay/runtime | shared body/collision view, sequenced input commands, native prediction/reconciliation baseline | local and remote movement integration tests |
| `024-entities-and-passive-mobs.md` | gameplay/render | entity sections, snapshots/deltas, passive spawn baseline, first render path | generated entity fixture and visible entity capture |
| `025-native-xr-and-parity-consolidation.md` | platform/perf | OpenXR presentation smoke shape, perf counters, fly-through benchmark, parity gap report | native smoke plus tracked perf/parity report |

## Immediate Focus

The next implementation tactical should stay on [`018-chunk-ticket-distance-manager.md`](018-chunk-ticket-distance-manager.md) and then move into scheduler-owned dependency holders.

Do not start structures until ticket-driven residency has full distance propagation or we explicitly accept a narrower MVP. The immediate focus is promoting worker-local dependency reuse into scheduler-owned lower-status holders, adding performance counters/benchmarks around interest movement, and keeping clean `FEATURES` parity stable while deferring runtime liquid simulation tails.

## Deferral Notes

- Do not start broad renderer asset polish before the native app is rendering from a client replica.
- Do not start native lighting before snapshots can carry chunk facts through the client/server boundary.
- Do not start movement/prediction before `ClientRuntime` owns a client world and collision-relevant chunk facts.
- Do not build OpenXR on the current direct worldgen render path; wait until renderer presentation is separated from world ownership.
- Do not postpone all web work until the end; keep the WASM/browser target compiling and booting at the early runtime and renderer boundaries.
- Do not port every TypeScript worldgen follow-through in order. Port grouped families against oracle fixtures and split only when a concrete parity failure needs a focused tactical.
