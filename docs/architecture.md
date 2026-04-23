# Architecture

Runtime architecture for `mclone`.

This document exists to answer a different question than [`strategy.md`](./strategy.md) and [`worldgen-status.md`](./worldgen-status.md):

- `strategy.md`: how we translate Minecraft 1.17.1 into TypeScript
- `worldgen-status.md`: what parts of worldgen are landed today
- this document: how the engine should be split across simulation, rendering, storage, workers, and multiplayer hosts

The central decision is simple:

**Keep Minecraft parity in the simulation where it matters; diverge deliberately in runtime architecture where the browser, WebGPU, workers, storage, and multiplayer needs require it.**

## Architectural divergence standard

Even when this document recommends an engine-native runtime architecture, architectural choices should still be weighed against the reference Minecraft source.

That means:

- start by understanding how vanilla 1.17.1 splits the responsibility in question
- prefer the reference shape when it is still a good fit
- diverge only for a clear reason, not convenience or guesswork
- make the divergence intentional, narrow, and documented

Before committing to an architectural divergence, answer these questions:

1. What does the reference source do here?
2. Why is that shape a poor fit for browser/WebGPU/worker/Node constraints?
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

- Support browser singleplayer without render-thread worldgen stalls.
- Support browser multiplayer clients against an authoritative server.
- Support a headless dedicated server in Node.
- Preserve a path to vanilla 1.17.1 overworld parity.
- Preserve a path to non-vanilla gameplay later, including alternate physics systems such as PhysX-backed simulation.

## Non-goals

- Do not reproduce Minecraft Java's thread model class-for-class.
- Do not make IndexedDB the canonical world-storage model.
- Do not make the current browser smoke harness architecture the long-term engine architecture.
- Do not tie all future gameplay to strict vanilla movement/collision rules.

## Core decisions

### 1. Singleplayer is a local server, not a renderer shortcut

Browser singleplayer should run an authoritative local world host in a worker and talk to it through a transport boundary. It should not let the renderer call `NoiseBasedChunkGenerator` directly.

### 2. The simulation core is shared across all hosts

The same simulation core should be usable by:

- browser singleplayer
- browser multiplayer client-facing local prediction systems
- dedicated server in Node
- future test/oracle harnesses

This core must not depend on:

- DOM APIs
- `Worker`
- WebGPU
- IndexedDB
- Node-specific filesystem APIs

### 3. The renderer is a consumer, not the owner of world state

The renderer should consume chunk snapshots, block updates, light data, and other authoritative outputs. It should not own generation, persistence, or gameplay simulation.

### 4. Persistence and transport are adapters

World state and chunk records should have an engine-defined logical shape. Browser storage and dedicated-server storage should be adapter choices behind that shape.

Likewise, local singleplayer and remote multiplayer should share the same message model, with different transports:

- local: `postMessage` / `MessagePort`
- remote: WebSocket or later another network transport

### 5. Parity and custom gameplay are policies, not architectural forks

We should be able to support both:

- a `vanilla17` profile that keeps translated worldgen and gameplay rules as strict as practical
- custom profiles that diverge in physics, collision, entity behavior, or other systems

The architecture should not assume only one of those exists.

## Layer model

| Layer | Responsibility | Parity expectation |
|---|---|---|
| Simulation core | worldgen, block/state rules, chunk contents, gameplay systems, authoritative world state | strict where we target vanilla parity |
| Server runtime | task scheduling, chunk lifecycle, ticking, authority, persistence orchestration, multiplayer session state | engine-native divergence |
| Client runtime | input, camera, UI, chunk subscription, prediction/interpolation where needed | engine-native divergence |
| Meshing/build pipeline | convert chunk/block state into renderer-ready geometry | renderer-native divergence, while consuming parity-correct chunk contents |
| Renderer | WebGPU resources, uploads, passes, shaders, frame submission | engine-native divergence |
| Persistence adapters | IndexedDB, filesystem, future alternate backends | engine-native divergence |
| Transport adapters | local worker transport, WebSocket, future transports | engine-native divergence |

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
- Node
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

- browser singleplayer host: worker-backed
- dedicated host: Node-backed

The client should not bypass this layer even in singleplayer.

### Client runtime

This is the owner of rendering, input, and presentation.

Responsibilities:

- camera and controls
- subscribing to chunk/state data from the server runtime
- handing chunk contents to client meshing workers
- GPU upload and frame submission
- future UI, HUD, inventory, and presentation-only effects

The client runtime should never call worldgen directly.

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

## Host modes

### Browser singleplayer

Recommended shape:

- main thread: input, UI, renderer, GPU submission
- world worker: authoritative local server runtime
- one or more mesh workers: chunk meshing
- storage adapter: IndexedDB initially, with OPFS still open as an implementation option later

This should feel like local singleplayer, but architecturally it should behave like a local client talking to a local server.

### Browser multiplayer client

Recommended shape:

- main thread: input, UI, renderer
- one or more mesh workers: chunk meshing
- transport adapter: WebSocket or equivalent remote transport

This mode does not need local worldgen for authority, though it may still use local systems for prediction, interpolation, or temporary placeholder chunk handling.

### Dedicated server

Recommended shape:

- Node process as authoritative server runtime
- file-backed or database-backed persistence adapter
- remote transport adapter for clients
- optional worker-thread job pools later for chunk generation, meshing-independent preprocessing, or heavy simulation tasks

The dedicated server should be headless and should not depend on renderer code, browser globals, or IndexedDB assumptions.

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

These should be serializable without depending on live class instances.

## Persistence model

The canonical model should be engine-defined chunk/world records, not raw IndexedDB layout and not whatever Node filesystem structure we choose first.

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

That means:

- browser singleplayer uses the same command/update protocol shape over `postMessage`
- remote multiplayer uses the same protocol shape over WebSocket

This avoids building two engines:

- a shortcut local one
- a real remote one

Only the transport changes.

## Translation policy inside this architecture

The direct-translation rule from [`AGENTS.md`](../AGENTS.md) remains correct for the simulation/content side:

- worldgen
- block/state systems
- content logic where vanilla parity is the goal

But the runtime shell should intentionally diverge where Minecraft's JVM architecture is not the right fit for browser and Node hosts:

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

Today, the codebase does not yet match this target architecture.

Current gaps:

- generated chunks are created synchronously from the browser path
- the renderer can trigger chunk generation directly
- chunk meshing is still effectively main-thread work
- there is no real storage abstraction yet
- there is no transport abstraction yet
- there is no authoritative world host boundary yet

That is why performance and future multiplayer support are now architectural priorities, not just implementation details.

## Immediate implications

The next major refactor direction should be:

1. Introduce an authoritative chunk/world service boundary.
2. Stop letting renderer-owned code call generation directly.
3. Move browser singleplayer world ownership into a worker.
4. Move client chunk meshing into one or more workers.
5. Define persistence interfaces before implementing browser and Node adapters.
6. Define a shared local/remote message protocol before building multiplayer-specific shortcuts.

## Decision checklist

When making architectural changes, prefer the option that satisfies all of these:

- Can the same simulation core run in browser worker, main-thread tests, and Node?
- Can browser singleplayer and remote multiplayer share the same command/update protocol shape?
- Can the renderer remain a consumer rather than an owner of world state?
- Can persistence be swapped without touching simulation logic?
- Can a future `vanilla17` mode and a future custom-physics mode both fit without a rewrite?

If the answer is no, the boundary is probably in the wrong place.
