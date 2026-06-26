# Architecture

Runtime architecture for `mclone`.

For rough sequencing of the active runtime refactor work, see the Client Runtime / Integrated Server arc in [`tactical/README.md`](./tactical/README.md).

This document exists to answer a different question than [`strategy.md`](./strategy.md), [`worldgen-status.md`](./worldgen-status.md), and the more specific runtime contract docs:

- `strategy.md`: how we translate Minecraft 1.17.1 into native Rust
- `worldgen-status.md`: what parts of worldgen are landed today
- `worldgen-deterministic-order.md`: vanilla chunk-status order, decoration finality, lighting gates, and publication gates
- `runtime-data-model.md`: the shared chunk/block-state data model across simulation, storage, protocol, client workers, and meshing
- `protocol.md`: the logical host/client message model and transport-codec boundaries
- `loading-persistence.md`: world creation/open/join flow, chunk lifecycle, and save/eviction policy
- `authoritative-host-scheduling.md`: how player/session authority stays responsive while chunk jobs run
- `multiplayer-hosting.md`: local, dedicated, future P2P, transport, asset-hosting, and server-config shape
- `minecraft-client-replica-research.md`: vanilla integrated-server, client-world, lighting, fluid, entity-interpolation, and networking source review
- `gui.md`: WebGPU-only, vanilla-shaped 2D GUI architecture for menus, loading status, HUD, options, debug settings, and touch UI
- `player-movement-netcode.md`: paused high-rate player movement and netcode constraint notes
- `structures.md`: vanilla overworld structure starts, references, placement, and implementation order
- `worker-ownership.md`: concrete worker/cache ownership and the no-hangs baseline for UI/GPU and host ticks
- `platforms.md`: supported desktop flat, desktop XR, Android XR, flat Android, and web/WASM lanes plus validation policy
- this document: how the engine should be split across simulation, rendering, storage, workers, multiplayer hosts, and platform adapters

The central decision is simple:

**Keep Minecraft parity in the simulation where it matters; diverge deliberately in runtime architecture where native, web, Android, OpenXR, storage, workers, and multiplayer needs require it.**

## Architectural divergence standard

Even when this document recommends an engine-native runtime architecture, architectural choices should still be weighed against the reference Minecraft source.

That means:

- start by understanding how vanilla 1.17.1 splits the responsibility in question
- prefer the reference shape when it is still a good fit
- diverge only for a clear reason, not convenience or guesswork
- make the divergence intentional, narrow, and documented

Before committing to an architectural divergence, answer these questions:

1. What does the reference source do here?
2. Why is that shape a poor fit for native/web/Android/OpenXR/worker constraints?
3. What exact layer or boundary is diverging?
4. Does the divergence make future parity work easier, neutral, or harder?
5. What constraint keeps the divergence from leaking into parity-critical simulation logic?

Good divergences are ones where we can say all of the following clearly:

- the vanilla shape is understood
- the platform reason for divergence is concrete
- the scope of divergence is bounded
- the parity cost is known and acceptable
- there is a mitigation if parity work later needs tighter alignment

## Goals

- Support desktop flat, desktop OpenXR, Android XR / Quest standalone, flat Android, and web/WASM clients over shared engine contracts.
- Support local singleplayer without render-thread worldgen stalls.
- Support browser and native multiplayer clients against an authoritative server.
- Support a headless dedicated server.
- Preserve a path to vanilla 1.17.1 overworld parity.
- Preserve a path to non-vanilla gameplay later, including alternate physics systems such as PhysX-backed simulation.

## Non-goals

- Do not reproduce Minecraft Java's thread model class-for-class.
- Do not make IndexedDB the canonical world-storage model.
- Do not make any one platform app shell the long-term engine architecture.
- Do not tie all future gameplay to strict vanilla movement/collision rules.

## Core decisions

### 1. Singleplayer is an integrated server plus client world, not a renderer shortcut

Browser singleplayer should run an authoritative local server in a worker and talk to it through a client-facing protocol boundary. It should also hydrate a client world replica from that boundary, like vanilla's `IntegratedServer` plus `ClientLevel` shape. It should not let the renderer call `NoiseBasedChunkGenerator` directly or read host internals.

Canonical generation from seed is host-only. `ClientWorld` may interpret received block-state ids, light facts, biomes, model data, and collision views, but it must not fill missing chunks by running worldgen or decoration locally. Placeholder/loading visuals are allowed only when they are clearly non-authoritative and never become client-world truth.

### 2. The simulation core is shared across all hosts

The same simulation core should be usable by:

- desktop flat and desktop XR
- flat Android and Android XR / Quest
- web/WASM singleplayer
- browser/native multiplayer client-facing local prediction systems
- dedicated server
- future test/oracle harnesses

This core must not depend on:

- DOM APIs
- `Worker`
- WebGPU
- IndexedDB
- desktop windowing
- Android activity/JNI APIs
- OpenXR sessions, actions, or swapchains
- platform-specific filesystem APIs

### 3. The renderer is a consumer, not the owner of world state

The renderer should consume chunk snapshots, block updates, light data, and other authoritative outputs. It should not own generation, persistence, or gameplay simulation.

### 4. Persistence and transport are adapters

World state and chunk records should have an engine-defined logical shape. Browser storage and dedicated-server storage should be adapter choices behind that shape.

Likewise, local singleplayer and remote multiplayer should share the same message model, with different transports:

- local: `postMessage` / `MessagePort`
- remote dedicated: WebSocket by default, with HTTP retained only as non-default compatibility coverage
- future P2P or high-rate snapshot lanes: WebRTC only if the protocol needs it

### 5. Parity and custom gameplay are policies, not architectural forks

We should be able to support both:

- a `vanilla17` profile that keeps translated worldgen and gameplay rules as strict as practical
- custom profiles that diverge in physics, collision, entity behavior, or other systems

The architecture should not assume only one of those exists.

### 6. High-rate player movement is command-driven runtime gameplay, but paused behind client runtime architecture

Player movement is a deliberate runtime/gameplay divergence from vanilla 1.17.1, not a reason to fork the world or entity architecture. The lower shared movement ideas in `Entity.move(...)` and `LivingEntity.travel(...)` are still the reference to study before implementation, but the vanilla 20 TPS player packet loop is not the target protocol shape.

The durable constraints are:

- local player and host simulation consume the same sequenced movement commands
- render frame delta is never authoritative movement delta
- commands use fixed integer movement quanta; long frames are split or capped before simulation
- the host drains queued commands in order, rather than applying one mutable "latest input" per tick
- prediction snaps to authoritative state and replays unacknowledged commands; smoothing is presentation-only
- lower-rate NPC AI can produce movement intent for the shared body simulation without forcing player physics down to AI tick rate
- collision and physics revisions should be explicit once dynamic collision can affect prediction

The movement tactical arc is paused until the client runtime arc establishes `IntegratedServer`, `ClientRuntime`, `ClientWorld`, prediction-service, and presentation ownership. See [`player-movement-netcode.md`](./player-movement-netcode.md) for retained constraints and the Client Runtime / Integrated Server arc in [`tactical/README.md`](./tactical/README.md) for active sequencing.

### 7. Simulation clocks are separate API boundaries

The client runtime architecture must not bake in vanilla's 20 TPS rate or a browser render-frame rate as the single simulation clock. Keep these as separate concepts at API boundaries:

- host world/block/entity tick
- player command clock
- player physics step or fixed command quantum
- host snapshot publication cadence
- transport send/poll/push cadence
- render frame and presentation interpolation cadence

`ClientWorld` stores replicated facts and revisions. It should not decide high-rate movement timing. `PredictionService` may later run fixed quanta such as `1/120` or `1/128` over a bounded `ClientWorld` collision/entity view. The host may drain multiple movement commands inside one lower-rate world or network tick. Presentation may smooth or interpolate, but it must not become simulation truth.

## Layer model

| Layer | Responsibility | Parity expectation |
|---|---|---|
| Simulation core | worldgen, block/state rules, chunk contents, gameplay systems, authoritative world state | strict where we target vanilla parity |
| Server runtime | task scheduling, chunk lifecycle, ticking, authority, persistence orchestration, multiplayer session state | engine-native divergence |
| Client runtime | protocol application, client-world replica, input/session ownership, prediction/interpolation services, presentation-state publication | engine-native divergence shaped by vanilla `ClientLevel` ownership |
| Meshing/build pipeline | convert chunk/block state into renderer-ready geometry | renderer-native divergence, while consuming parity-correct chunk contents |
| Renderer | WebGPU resources, uploads, passes, shaders, frame submission | engine-native divergence |
| UI | shared Rust/WebGPU GUI model and draw list for menus, HUD, loading, options, and debug surfaces | engine-native divergence with vanilla-inspired behavior where useful |
| App/platform adapters | desktop `winit`, Android activity/JNI, browser canvas/workers, OpenXR runtime/session/swapchain, packaging, validation scripts | platform divergence |
| Persistence adapters | IndexedDB, filesystem, future alternate backends | engine-native divergence |
| Transport adapters | local worker transport, WebSocket, future transports | engine-native divergence |

## Current Platform Architecture Status

The repo is materially past the original browser-first split. Current validated
platform lanes are tracked in [`platforms.md`](./platforms.md):

- desktop flat: `mclone-native-client`
- desktop OpenXR: `mclone-native-client --features xr`
- Android XR / Quest: `mclone-android-xr-client` plus `android-xr/`
- flat Android: `mclone-android-client` plus `android/`
- web/WASM: `mclone-web-client`

The shared boundaries that matter most today are:

- `mclone_server`, `mclone_client`, and `mclone_protocol` for host/client
  authority and replicated state
- `mclone_app_runtime` for shared single-view runtime/render helpers
- `mclone_render_session` for render-section dirty/cache/compile policy and
  camera-controller contracts
- `mclone_render` for drawing from explicit view/target facts
- `mclone_ui` for shared Rust/WebGPU UI data
- `mclone_xr_host`, `mclone_xr_graphics`, and `mclone_xr_scene` for shared
  desktop/Quest XR session, graphics, terrain, and controller-locomotion
  contracts

The architectural goal is no longer "can another platform boot?" The goal is
"can features land once, behind shared contracts, with targeted platform
sentinel smokes catching adapter regressions?"

## Recommended runtime shape

### Shared simulation core

This layer should contain the deterministic content and rules that need to behave like Minecraft when parity matters:

- PRNG and noise
- biome source
- terrain generation
- carvers
- surface rules
- feature placement
- structures when they land
- block/state/property logic
- future authoritative gameplay systems

This layer should expose pure or mostly pure APIs around:

- chunk generation
- chunk updates
- block and fluid state queries
- world tick entry points
- entity/system integration points

It should not know whether it is running in:

- a browser worker
- the main thread
- a native dedicated-server process
- a test harness

### Authoritative server runtime

This is the owner of the world.

Responsibilities:

- receive client intents/commands
- decide which chunks should be loaded, generated, decorated, meshed, saved, or evicted
- run authoritative gameplay and world ticks
- own persistence policy
- publish chunk snapshots and world updates

There should be two host forms of the same conceptual server:

- `IntegratedServer`: local singleplayer, runner/worker-backed, created by a local game session
- dedicated/headless host: native server process

The client should not bypass this layer even in singleplayer.

### Client runtime

This is the owner of the client session and client-side replica.

Responsibilities:

- apply client-facing protocol messages
- own `ClientWorld`: visible/interested chunks, block/fluid states, block entities, entities, light/render facts, revisions, and speculative overlays
- own prediction and interpolation services when they exist
- publish compact presentation state to UI/render
- hand chunk contents or mesh jobs to client meshing workers

The presentation/UI thread owns input sampling, pointer lock, UI, GPU resources, uploads, and frame submission. It should consume client-runtime outputs rather than owning raw world facts.

The client runtime should never call worldgen directly.

The first facade layer lives under `native/crates/mclone-client/src/`:

- `ClientRuntime` applies protocol updates, owns the local replica, and exposes ownership-oriented methods such as `set_chunk_view(...)`.
- `LocalPlayerController` and related player modules build movement and interaction commands against the client replica.
- `ActorPresentation` and interpolation state convert authoritative remote-player/entity facts into render-facing presentation data.

These facades are a naming and contract step over current code. They do not make `ClientChunkCache` final, and they do not add new movement physics, NPC AI, transport semantics, or fluid prediction.

### Client meshing workers

Chunk meshing is renderer-adjacent CPU work, not authoritative gameplay.

Move section rebuild work off the render thread into worker-backed jobs that consume:

- chunk block data
- light data
- baked model/material references

and produce:

- CPU-side vertex/index payloads or mesh records ready for GPU upload

The render thread should remain responsible for:

- GPU resource creation
- buffer uploads
- pass submission

For the current worker/cache ownership baseline, including the dedicated lighting worker and the decision to defer worldgen/decor pools until measurement, see [`worker-ownership.md`](./worker-ownership.md).

## Host modes

### Web/WASM singleplayer

Recommended shape:

- main thread: input, UI, renderer, GPU submission
- world worker: authoritative local server runtime
- one or more mesh workers: chunk meshing
- storage adapter: IndexedDB initially, with OPFS still open as an implementation option later

This should feel like local singleplayer, but architecturally it should behave like a local client talking to a local server.

### Web/WASM multiplayer client

Recommended shape:

- main thread: input, UI, renderer
- one or more mesh workers: chunk meshing
- transport adapter: WebSocket or equivalent remote transport

This mode does not need local worldgen for authority, though it may still use local systems for prediction, interpolation, or temporary placeholder chunk handling.

### Dedicated server

Recommended shape:

- native process as authoritative server runtime
- file-backed or database-backed persistence adapter
- remote transport adapter for clients
- optional worker-thread job pools later for chunk generation, meshing-independent preprocessing, or heavy simulation tasks

The dedicated server should be headless and should not depend on renderer code, browser globals, or IndexedDB assumptions.

The first dedicated-host slice is now landed in that shape:

- `FilesystemChunkSnapshotStore` provides the initial file-backed persistence adapter behind `ChunkSnapshotStore`.
- `native/apps/mclone-dedicated-server/src/main.rs` provides the native headless server bootstrap.
- the native desktop integrated path, native dedicated server, and native web integrated server all use the same protocol/client/server crate boundaries.

What is still missing is remote client connectivity, not a separate server runtime core.

## Data boundaries

The architecture should revolve around stable engine-level data contracts, not around direct object sharing across unrelated layers.

Important boundaries:

- client intent -> server command
- server authoritative state -> client updates
- server chunk state -> client meshing input
- meshing output -> renderer upload input
- logical chunk record -> persistence adapter record

At minimum, the engine should converge on explicit shapes for:

- chunk snapshot
- chunk delta / block update batch
- light snapshot or light delta
- player input command
- authoritative player state snapshot
- world metadata and save metadata

The durable shape of those data records lives in [`runtime-data-model.md`](./runtime-data-model.md). The message model that carries them lives in [`protocol.md`](./protocol.md).
Lighting has additional parity-sensitive solver rules and browser/worker ownership constraints; keep the detailed design in [`lighting.md`](./lighting.md).

These should be serializable without depending on live class instances.

## Persistence model

The canonical model should be engine-defined chunk/world records, not raw IndexedDB layout and not whatever native filesystem structure we choose first.

Detailed loading, dirty-state, lazy-save, and eviction policy lives in [`loading-persistence.md`](./loading-persistence.md).
Vanilla chunk-status order, generation finality, lighting gates, and publication gates live in [`worldgen-deterministic-order.md`](./worldgen-deterministic-order.md).
Scheduling rules that keep input, player ticks, and polling from blocking behind chunk jobs live in [`authoritative-host-scheduling.md`](./authoritative-host-scheduling.md).

Recommended rule:

- define logical persistence interfaces first
- implement browser and server adapters second

Suggested browser adapter:

- IndexedDB first, because it is the most practical baseline for browser singleplayer

Suggested dedicated-server adapter:

- file-backed chunk store first, with the option to move to a region-style layout or SQLite later if needed

Important constraint:

Do not let browser persistence choices leak into the simulation core.

## Networking / transport model

The message model should be shared between local singleplayer and multiplayer.

The durable logical protocol and wire-codec split lives in [`protocol.md`](./protocol.md).
The hosting/product shape lives in [`multiplayer-hosting.md`](./multiplayer-hosting.md).

That means:

- browser singleplayer uses the same command/update protocol shape over `postMessage`
- remote dedicated play uses the same serialized message shapes over a persistent WebSocket channel by default
- HTTP request/response remains temporary compatibility coverage, not the user-facing remote mode
- future WebRTC or other transports must carry the same logical messages instead of redefining authority

`R6` landed the first hardening pass on that rule:

- protocol-versioned remote envelopes
- stable remote error codes
- resumable remote sessions
- baseline session/player state snapshots
- shared dedicated-host chunk-interest management per save

`R7` keeps that same transport rule but adds the first gameplay-state loop on top of it:

- explicit `set_player_input` commands
- authoritative `player_state` snapshots
- queued `poll_world_updates` delivery for server-originated updates
- dedicated-host shared-session ticking without moving the renderer back into ownership

`R8` closes the first real browser-control ownership gap on top of that:

- the live browser debug/control path now derives camera state from authoritative `player_state`
- browser input is translated into `set_player_input` instead of mutating a renderer-owned camera directly
- chunk-interest updates now follow authoritative player position in the live browser loop
- the renderer remains a presentation consumer over host-owned player/world state

This avoids building two engines:

- a shortcut local one
- a real remote one

Only the transport changes.

## Translation policy inside this architecture

The direct-translation rule from [`AGENTS.md`](../AGENTS.md) remains correct for the simulation/content side:

- worldgen
- block/state systems
- content logic where vanilla parity is the goal

But the runtime shell should intentionally diverge where Minecraft's JVM architecture is not the right fit for native, browser, Android, or OpenXR hosts:

- worker boundaries
- scheduling
- persistence
- network transport
- render-thread ownership
- meshing pipeline organization

In short:

- translate gameplay logic
- design engine architecture

When making runtime-architecture decisions, use the reference Minecraft source as the baseline design input, not just as an implementation quarry. The question is not "can we do this differently?", but "what does vanilla do, why are we diverging, and what does that cost us for future parity work?"

## Extensibility for non-vanilla gameplay

This architecture is compatible with later divergence, including alternate physics systems such as PhysX, as long as those systems live in the authoritative simulation/runtime side and not in the renderer.

The clean way to think about this is in profiles or modes:

- `vanilla17`: translated worldgen and gameplay rules
- `custom`: translated worldgen with modified gameplay
- future additional profiles as needed

Examples of systems that may vary by profile:

- player movement and collision
- rigid-body simulation
- block destruction rules
- fluid behavior
- entity behavior
- game rules and interaction semantics

The important design rule is that these remain server-authoritative systems with stable client-facing state/update contracts.

## Current mismatch in the repo

The codebase is materially closer to this target architecture now that the five client/platform lanes have basic validation, but it still does not fully match the long-term shape.

Current gaps:

- flat Android still carries app-local scene/runtime glue that should be collapsed into `mclone-app-runtime` where it is not truly Android-specific
- desktop XR and Android XR share the new XR host/graphics/scene crates, but desktop XR still has richer app-local terrain/actor/session behavior that should converge before adding more XR-only features
- lighting has a strong first pass, but parity correctness and render integration are still a user-visible feature gap
- shared menu/HUD/options/loading UI is not yet complete enough to be the obvious feature path for every platform
- validation is still more script-list than contract matrix; contributors need clearer guidance on which shared boundary requires which platform sentinel
- richer gameplay still needs parity movement, entities, interactions, and server correctness work without moving ownership back into renderer/app shells

That is why boundary consolidation, lighting/UI feature parity, and richer authoritative gameplay are now the architectural priorities, not more platform bring-up.

## Immediate implications

The next major refactor direction should be:

1. Publish a platform contract matrix: crate boundary, consuming apps, required tests/smokes, and device/headset requirements.
2. Collapse reusable flat Android scene/runtime code into `mclone-app-runtime` so single-view hosts share the same contract.
3. Finish XR scene convergence so desktop XR and Android XR share terrain, actor, controller, startup-pose, and locomotion behavior behind `mclone-xr-scene`.
4. Advance lighting and shared UI as platform-neutral feature contracts.
5. Grow authoritative gameplay beyond baseline player/session motion state without moving ownership back into renderer or app shells.

## Decision checklist

When making architectural changes, prefer the option that satisfies all of these:

- Can the same simulation core run in browser workers, native runners, dedicated server processes, and tests?
- Can browser singleplayer and remote multiplayer share the same command/update protocol shape?
- Can the renderer remain a consumer rather than an owner of world state?
- Can desktop flat, desktop XR, Android XR, flat Android, and web drive the renderer from explicit view/target facts?
- Can persistence be swapped without touching simulation logic?
- Can a future `vanilla17` mode and a future custom-physics mode both fit without a rewrite?

If the answer is no, the boundary is probably in the wrong place.
