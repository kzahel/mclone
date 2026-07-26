# Tactical 256: Shared Horizon Vegetation Worker Topology

Status: active; architecture checkpoint and Slices 0–4 completed 2026-07-26.
Slice 5 is next.

Topics:

- `procedural-horizon-clipmap`
- `lod-native-vegetation`
- `web-worker-runtime-ownership`
- `platform-host-boundary`

Parent:

- [`253`](253-world-explorer-cross-host-parity.md) sequences the complete
  cross-host parity campaign.

Existing patterns to audit:

- [`062`](062-shared-threading-topology.md) owns backend-neutral mailbox and
  native-thread/browser-Worker convergence;
- [`197`](197-domain-blind-web-worker-broker.md) established isolated Rust
  actors behind domain-blind browser transport;
- [`248`](248-terrain-lab-navigation-and-worker-modernization.md) implemented
  the Terrain Lab exact-terrain Rust coordinator and external-SAB mailbox; and
- the full web game already uses a Rust `WebRenderWorkerCoordinator` and
  worker-resident render actor over the shared `PolledWorkerTransport`
  mechanics.

## Objective

Render the same stable procedural tree proxies in native and browser World
Explorer without compiling vegetation synchronously on either presentation
thread.

Both targets must use one shared Rust coordinator and the same logical worker
topology. Platform adapters may use native channels/moves or browser
Worker/SAB mechanics, but they must not own different job policy, cache
behavior, budgets, or acceptance rules.

World Explorer is the first product host and parity proof, not the final owner.
The implementation produced by this tactical must be a reusable procedural
terrain-vegetation streaming service that a later `mclone-scene` integration
can consume without moving coordinator policy out of shared crates or
reimplementing it for the game.

This tactical stops before actual game-scene adoption. Exact/procedural
coverage arbitration, edited-tree suppression, multiple live game worlds,
scene lifecycle, and XR draw integration require a later focused tactical.
They must not be hidden inside the Explorer parity change.

## Product And Engine Role

The relevant Terrain Lab work has two distinct parts:

- the procedural CPU/GPU LOD source, viewport planning, products, and
  rendering that already live in `mclone-worldgen` and
  `mclone-terrain-view`; and
- `CanonicalTerrainWorkerCoordinator`, which serves the Lab's exact
  generated-chunk comparison pane.

The first part is the system intended for eventual in-game use. The second is
an architectural precedent for Worker ownership, bounded admission, cache
lifetime, and external-SAB publication; it is not the terrain-LOD coordinator
to import into the game.

Similarly, the full web game's `WebRenderWorkerCoordinator` is a
domain-specific exact render-section coordinator. It owns multiple world
identities, active/standby priority, asset epochs, compiler-session release,
timeouts, and render-section requests. It is not a universal engine worker
framework. This tactical reuses its domain-blind `PolledWorkerTransport`
mechanics and its tested lifecycle patterns, but it must not make procedural
vegetation pretend to be an exact render-section job.

The reusable layering is:

```text
Terrain Lab / World Explorer / later mclone-scene
                         |
       shared terrain vegetation coordinator
                  mclone-terrain-view
                         |
       shared compiler session and semantic cache
                    mclone-worldgen
                         |
             platform execution adapter
             native thread | Web Worker
```

The same service type, compiler, job identity, budgets, stale rules, result
meaning, and diagnostics must be reusable by all consumers. Reusing the same
logical service does not require exact chunk meshing and vegetation planning
to share one physical Worker.

## Existing Data Shape

The default horizon is a ten-level clipmap with `4 x 4` fixed physical tiles
per level. Every tile evaluates a `64 x 64` terrain-cell grid, while sample
spacing doubles by level:

| Level | Spacing | Tile footprint | Vegetation representation |
|---:|---:|---:|---|
| 0 | 1 block | `64 x 64` blocks | all admitted stable tree records |
| 1 | 2 blocks | `128 x 128` blocks | landmark ranks `2/3` |
| 2 | 4 blocks | `256 x 256` blocks | landmark rank `3` |
| 3+ | 8–512 blocks | `512 x 512` through `32,768 x 32,768` blocks | forest summary only |

The terrain clipmap therefore owns `160` fixed slots, while the current
individual-tree working set contains at most `48` tiles: three near levels
times sixteen tiles. This is a default configuration fact, not a coordinator
ABI. The shared coordinator accepts a desired set of semantic
`TerrainViewportTileId` values and derives its bounds from the caller's
validated clipmap configuration.

Forest intent and coarse forest summaries are continuous terrain-source facts.
They remain available at coarse levels without enumerating individual trees.
A vegetation worker job compiles the bounded stable tree occurrences for one
near LOD tile. It does not compile terrain pixels, exact chunks, GPU geometry,
or an entire forest region.

The existing planner uses `32 x 32`-block planning cells with `32` fixed
candidate slots. After the product's base-in-tile filter, the largest current
`256 x 256` tile has a hard ceiling of:

```text
8 x 8 planning cells x 32 candidates = 2,048 occurrences
```

Real results are smaller because terrain suitability, water, density,
conflict, and landmark-rank rules reject candidates. Current native smoke
evidence observed `1,726` trees and `165,696` GPU bytes across the initial
48-tile working set, then `5,267` trees and `505,632` GPU bytes in the denser
movement checkpoint. Current shared rendering reserves `96` GPU bytes per
tree; that is a renderer packing fact, not the browser worker ABI.

## Current Problem

`TerrainHorizonRenderer::encode` currently owns a vegetation queue and
compiles one `TerrainPreviewVegetationProduct` synchronously during frame
encoding. Native enables this path. Browser hardcodes vegetation disabled to
avoid performing the same CPU work on the Wasm animation thread.

The result is a visible feature split:

- native progressively shows stable tree proxies at near sample spacings; and
- browser reports zero vegetation work and zero tree instances at every zoom.

Simply enabling the flag in browser would remove the visual omission by
introducing the wrong frame topology. Keeping native synchronous would retain
another privileged path that the full game and browser cannot share.

## Required Logical Topology

```text
TerrainHorizonVegetationCoordinator
  owns source identity, epochs, desired work, priority, budgets,
  in-flight bounds, cache lifecycle, stale rejection, diagnostics
                         |
             VegetationWorkerMailbox
                         |
            +------------+------------+
            |                         |
   native threaded executor   browser Worker executor
            |                         |
      shared Rust compiler      worker-resident Rust actor
            |                         |
            +------------+------------+
                         |
        revisioned vegetation products/completions
                         |
        renderer uploads admitted instance records
```

“Same topology” means:

- the same job identity and source revision;
- the same desired-work and priority calculation;
- the same bounded in-flight and completion-admission policy;
- the same semantic record/cache ownership;
- the same stale and cancellation behavior;
- the same readiness and failure states; and
- the same diagnostics and acceptance hashes.

It does not require native to serialize Rust values through a browser-shaped
ABI. Native may use typed channels and moved products. Browser may use encoded
opaque frames and external `SharedArrayBuffer` mailboxes between isolated Wasm
instances.

## Shared Ownership

`mclone-terrain-view` should own the platform-neutral, WGPU-independent
coordinator, request and completion identity, desired-work ordering, admission
rules, and prepared renderer handoff. The coordinator consumes semantic tile
identity and slot-admission tokens; it must not require a concrete WGPU device
or queue.

`mclone-worldgen` continues to own deterministic vegetation planning,
`McloneOverworldVegetationPlanCache`, summaries, records, and source
revisions. Add one stateful vegetation compiler session around the existing
product compiler and cache so native and browser actors call the same pure
job boundary.

Native and browser app/platform adapters own only executor construction,
wakeup/poll mechanics, failure envelopes, and shutdown:

- native creates and joins the worker thread or bounded pool;
- browser creates the generic Worker transport, supplies external SAB
  mechanics, and forwards opaque actor frames; and
- neither adapter understands tree families, tile priority, cache keys,
  landmark ranks, or accepted record meaning.

Worker count may be a device/capability budget. The logical lanes and
coordinator behavior must remain identical. Capability absence is explicit;
target identity must not silently change the product configuration.

For this tactical, lock both default executors to one worker. A later measured
device policy may increase physical parallelism behind the same coordinator,
but it must not change result identity, cache correctness, priority, or
admission semantics.

World Explorer app code constructs the executor and passes it into the shared
session/service. It must not own the desired tile queue, source epoch, retries,
or record acceptance. The later game integration should be able to compose the
same service under `mclone-scene`; it must not copy an Explorer coordinator
into either the native or browser game app.

## Selected Coordinator Contract

The shared coordinator owns:

- one explicit source identity and source epoch;
- one monotonically increasing request ID and executor generation;
- the desired semantic tile set and deterministic priority;
- a bounded queued set, one in-flight job, and one admission per pump;
- physical-slot generation tokens supplied by the clipmap consumer;
- stale and superseded completion rejection;
- compiler/cache, queue, transport, failure, and admission diagnostics;
- restart policy; and
- graceful and forced shutdown state.

The first priority order preserves the current progressive presentation:

1. eligible coarser tree level before its finer level (`4`, then `2`, then
   `1` sample spacing);
2. tile-center distance from the current focus within one spacing;
3. semantic tile Z and X as stable tie-breakers.

Physical slot number is never semantic priority or cache identity.

A job identity contains at least:

```text
executor generation
source epoch
request ID
semantic TerrainViewportTileId
physical slot index and slot generation
```

Changing seed, profile, topology, content stage, or a relevant compiler source
revision increments the source epoch and resets worker-resident cache state.
Ordinary movement advances desired coverage without invalidating a retained
tile. A completion from an older coverage revision may still be admitted only
when its semantic tile remains desired and the exact physical slot generation
still matches. A reassigned toroidal slot always rejects the old completion.

The source identity is authored by shared Rust and includes:

- terrain profile and signed seed;
- sampling topology;
- preview content stage and surface-quality contract;
- terrain/structured-source revision fingerprint;
- vegetation-plan revision; and
- vegetation product/codec revision.

Camera focus, pane visibility, map/orbit mode, diagnostics, and physical slot
number are not source identity.

The executor boundary should be a narrow typed Rust contract resembling:

```text
kind()
try_submit(job)
drain_events()
restart(generation)
request_shutdown()
is_terminated()
diagnostics()
```

`try_submit` and `drain_events` must never block the presentation thread.
Inline execution is permitted only for explicit unit tests or a separately
named diagnostic fallback; it is not an ordinary native or browser executor.

## Selected Native Mailbox

Native uses one named OS thread. The worker owns the shared worldgen compiler
session and its vegetation plan cache.

- request channel capacity: `1`;
- completion channel capacity: `1`;
- coordinator in-flight bound: `1`;
- request submission: nonblocking `try_send`;
- result transfer: moved typed Rust product, with no browser codec;
- normal shutdown: disconnect request ingress, let active work finish, and
  join; and
- forced failure: report thread disconnect/panic through the executor event
  envelope rather than compiling inline.

The server worldgen mailbox is the lifecycle precedent, not a type dependency.
Its unbounded production queue and server-specific job/result values are not
copied into this service.

## Selected Browser Actor And Frame ABI

The browser keeps isolated main and Worker Wasm heaps. JavaScript reuses the
existing domain-blind `PolledWorkerTransport` source and a small specialized
Worker shell that loads bindgen, retains one Rust actor, forwards opaque
frames/buffer handles, and posts opaque completion doorbells.

Worker Rust owns frame decoding, source validation, the compiler session,
cache reset, product encoding, typed errors, metrics, and graceful shutdown.
JavaScript must not name vegetation, tiles, ranks, profiles, cache policy,
epochs, or result fields.

The dependency direction remains `mclone-terrain-view -> mclone-worldgen`.
`TerrainViewportTileId`, physical-slot generations, desired coverage, and
admission stay terrain-view concepts. The worldgen compiler/codec accepts a
validated `TerrainPreviewRequest`, a worldgen-owned semantic source identity,
and fixed-width opaque correlation scalars. It must not import
`mclone-terrain-view` merely to encode a coordinator job.

Use one versioned binary `MCHV` envelope with:

- version `1`;
- commands `initialize`, `compile`, and `shutdown`;
- results `ready`, `completed`, `failed`, and `shutdown-complete`;
- executor generation, source epoch, and request ID;
- complete semantic tile/source identity;
- compile and cache diagnostics; and
- strict length, enum, count, reserved-byte, and trailing-byte validation.

The version-1 occurrence payload is a fixed `80` bytes:

```text
stable planning-cell ID and candidate slot
vegetation revision
canonical base position
family and archetype
trunk/crown dimensions
orientation and landmark rank
variant seed
canonical conservative bounds
periodic X lift
reserved zero bytes
```

Working bounds are derived from canonical bounds and X lift rather than
encoded twice. Products remain stably ordered, and the result includes a
stable semantic record hash plus family and total counts independent of draw
order.

Keep three revisions distinct:

- the semantic compiler/source revision covers every terrain, structured
  source, vegetation-planning, and admission rule that can change records;
- the semantic product revision covers the canonical occurrence meaning and
  stable-hash input; and
- the `MCHV` wire version covers only browser transport compatibility.

A wire-layout-only change does not invalidate native semantic products or the
worker planning cache. Version 1 hashes the concatenated canonical 80-byte
occurrence payloads in stable product order with 64-bit FNV-1a. The hash
includes periodic X lift, excludes reserved bytes, and is independent of GPU
instance packing and draw order.

Compile requests are small versioned byte frames and may use one transferred
`Uint8Array`. The steady-state bulk result uses one persistent external
`SharedArrayBuffer` owned by main Rust plus a six-word atomic control block:

```text
status
published byte length
resident capacity
executor generation
source epoch
request ID
```

Lock the initial result arena at `256 KiB`. At `2,048 x 80` bytes, the current
hard occurrence ceiling occupies `163,840` bytes before its bounded header.
The normal path therefore fits with margin.

An oversized result publishes through an explicitly sized one-off SAB, is
validated and admitted from that buffer, and grows the resident arena to the
next power of two. The absolute version-1 result limit is `1 MiB`; larger
results are protocol failures. Never truncate and never silently switch the
bulk result to a transferable array. Capacity, high-water, overflow, and
copied-byte counts remain observable.

Cross-origin isolation, `SharedArrayBuffer`, and required Atomics are explicit
browser capabilities. When vegetation is enabled, their absence fails
executor/session initialization with an actionable error; it must not
silently select the browser-disabled product configuration.

## Failure Recovery And Shutdown

Transport construction, browser error, malformed response, identity mismatch,
and native disconnect are executor failures. A deterministic compiler error
is a typed job/source failure.

The coordinator permits one automatic executor reconstruction after a
transport failure:

1. increment executor generation;
2. clear in-flight identity and worker-resident cache assumptions;
3. recreate the platform executor;
4. requeue only currently desired, nonresident tiles; and
5. reject every completion from the retired generation.

One successful completion clears the consecutive transport-failure count. A
second consecutive transport failure enters an explicit failed state and
keeps already drawable terrain/vegetation resident without a synchronous
fallback. Deterministic compile or protocol errors fail the affected source
rather than restart forever.

Graceful shutdown stops new admission, clears desired work, requests actor
shutdown, observes `shutdown-complete`, and releases the executor. Native then
joins its thread; browser then terminates the Worker. Drop/host teardown may
force termination as the final backstop. Shutdown is idempotent.

## Architecture Checkpoint

Before writing a new actor or mailbox:

1. Compare the Terrain Lab canonical coordinator, full-game render
   coordinator, server-job mailbox, and their browser transports.
2. Identify the reusable domain-blind construction/poll/termination mechanics
   and the coordinator state patterns that can be shared without creating a
   universal Worker framework.
3. Measure representative packed vegetation result sizes and decide whether
   the existing external-SAB mailbox shape is appropriate as-is.
4. Decide the native mailbox shape and prove it consumes the same coordinator
   contract without an inline ordinary-frame fallback.
5. Record the selected actor/frame ABI, source identity, capacities, overflow
   behavior, failure recovery, and shutdown contract before implementation.

Do not introduce shared Wasm linear memory in this tactical. The accepted
default remains isolated Rust actors plus domain-blind browser mechanics and
explicit external buffers.

Architecture checkpoint completed 2026-07-26. The sections above record the
selected service boundary, one-worker/one-in-flight policy, native mailbox,
`MCHV` version-1 ABI, `256 KiB`/`1 MiB` SAB capacities, stale identity,
single-restart policy, and shutdown contract.

## Implementation Order

### Slice 0: durable contract

Status: complete 2026-07-26.

1. Land the architecture checkpoint in this tactical and the relevant living
   topic documents.
2. Add source/ownership locks that prevent an Explorer-local coordinator or
   browser vegetation policy.
3. Pin the one current Horizon synchronous call and native/browser feature
   split as named pre-cutover debt; later slices must reduce both to zero.
4. Keep the Terrain Lab CPU/reference comparison renderer outside this
   tactical's cutover scope.

### Slice 1: shared compiler session and codec

Status: complete 2026-07-26.

1. Extract synchronous vegetation compilation from renderer encoding into a
   stateful `mclone-worldgen` job compiler callable by either executor.
2. Preserve byte-identical cold/warm products and the existing bounded
   planning-cell cache behavior.
3. Round-trip and bounds-test the versioned `MCHV` codec, including empty
   products, maximum counts, malformed input, overflow, and stable hashes.

`TerrainVegetationCompilerSession` now owns the existing bounded planning-cell
cache and resets it on complete semantic source identity changes. The
worldgen-only MCHV codec carries validated preview requests and opaque numeric
correlation values without depending on terrain-view. Native/Wasm checks and
the complete worldgen suite pass, including cold/warm semantic receipts,
alternating topology/seed sources, every command/result kind, fixed 80-byte
occurrences, maximum overflow, and malformed magic/version/kind/enum/count/
reserved/receipt/trailing-byte rejection.

### Slice 2: shared coordinator

Status: complete 2026-07-26.

1. Add the WGPU-independent `mclone-terrain-view` coordinator.
2. Drive it with a fake executor and deterministic desired tile sets.
3. Prove priority, retained-tile acceptance, toroidal slot reassignment,
   source reset, stale generations, one-in-flight/one-admission bounds,
   restart, failure, and idempotent shutdown.

`TerrainVegetationCoordinator` now consumes bounded semantic desired sets and
explicit slot-generation tokens through the selected nonblocking executor
contract. Nine fake-executor tests prove coarse-to-fine/distance/stable
priority, retained coverage, slot reassignment, source/generation rejection,
one automatic restart with success reset, consecutive transport and
deterministic job failure, bounded-full retry, and idempotent shutdown. The
complete terrain-view suite and Wasm check pass.

### Slice 3: native threaded cutover

Status: complete 2026-07-26.

1. Add the native bounded-channel executor and named worker thread.
2. Pass it into World Explorer through the shared session boundary.
3. Remove the renderer-owned vegetation queue/cache and every ordinary
   synchronous `compile_with_cache` call from `TerrainHorizonRenderer`.
4. Prove no vegetation planning occurs on the render thread and inspect native
   movement, teleport, empty-region, and dense-forest pixels.

The native Explorer now constructs one named `mclone-terrain-vegetation`
thread with capacity-one typed request/completion channels. Source changes
retire an active generation without joining it on the presentation thread;
normal shutdown disconnects ingress, finishes active work, and joins on
executor drop. `TerrainHorizonRenderer` owns no vegetation queue/cache and has
zero synchronous `compile_with_cache` calls.

Native-window and offscreen smoke passed with the pre-cutover semantic counts:
the initial view admitted `1,726` trees / `165,696` bytes and the dense
movement checkpoint admitted `5,267` / `505,632`. Movement frame p95 was
`3.43 ms`; settle p95 was `3.21 ms`. Initial, movement, negative empty-region,
teleport, map, and orbit captures were inspected under
`/tmp/mclone-world-explorer-smoke`.

### Slice 4: browser Worker cutover

Status: complete 2026-07-26.

1. Reuse the generic browser Worker transport source.
2. Add the worker-resident Rust actor, specialized domain-blind shell, and
   persistent external result SAB.
3. Enable the same Explorer vegetation product configuration as native.
4. Prove cross-origin isolation, ordinary capacity, forced overflow/growth,
   worker failure/restart, and clean shutdown.

World Explorer now compiles the existing domain-blind
`PolledWorkerTransport` TypeScript source into its standalone web root. The
ordinary JavaScript host constructs only a generic transport factory; its
specialized Worker shell loads bindgen, retains `WorldExplorerWorkerActor`,
forwards opaque messages, and posts opaque doorbells. Rust on the main side
owns the six-word control block, persistent result arena, MCHV validation,
typed executor events, reconstruction, and shutdown. Worker Rust owns the
compiler session, source/cache lifetime, typed failure, result encoding, and
external-SAB publication. Both ordinary JavaScript files remain free of
vegetation scheduling vocabulary.

The production browser executor fails initialization without cross-origin
isolation, `SharedArrayBuffer`, and the required Atomics. Its resident result
arena starts at `256 KiB` and retains the `1 MiB` absolute MCHV bound. The
explicit smoke-only overflow probe starts at `1 KiB`; real results exercised
four one-off-SAB publications, grew the resident arena to `32 KiB`, reached a
`19,388`-byte high-water mark, and copied `217,804` result bytes by the
initial settled view. The same view admitted all 48 requested vegetation
tiles, `2,609` tree instances, and `250,464` GPU vegetation bytes.

The headed Wayland desktop smoke then deliberately terminated the active
generic transport. The next movement request failed nonblockingly, the shared
coordinator incremented its executor generation, the factory constructed a
replacement Worker, and the replacement settled the desired set without an
inline fallback. Negative coordinates and a million-block teleport also
settled, and an explicit host/session shutdown observed the Rust actor's
`shutdown-complete` before termination. Browser single-Worker Wasm compilation
is materially slower than native—the complete 48-tile set needs a longer
acceptance window—so Slice 5 must retain timing diagnostics and record this as
measured follow-up rather than treating smoke success as a throughput claim.

### Slice 5: parity and closeout

1. Add shared source revision, stable record hash, family counts, tree count,
   proxy vertex count, queue, cache, timing, Worker, and SAB diagnostics.
2. Exercise movement, zoom, teleport, negative/large coordinates, stale
   completion, source changes, failure, overflow, and shutdown.
3. Capture and inspect matched native, offscreen, desktop-browser, and
   phone-browser tree views.
4. Record the exact service construction and prepared-product handoff that a
   later `mclone-scene` tactical will consume.
5. Leave the Explorer and full game app crates with platform mechanics only.

Each slice should land as an independently reviewable commit. Delete the
superseded synchronous path in the native cutover rather than retaining an
ordinary fallback until browser work completes.

## Later Game-Integration Tactical

After this tactical closes, a separate tactical should integrate the proven
service into the game through `mclone-scene`. It should own:

- scene/world lifecycle and source exposure;
- exact-painted frame snapshots and procedural coverage masks;
- exact/proxy tree XOR across cross-chunk crowns;
- natural-record invalidation from authoritative edits;
- multiple active/standby world budgets where required;
- render-session device rebuild and frame admission;
- mono, synthetic stereo, per-eye, and full-frame multiview;
- native, browser, Android, and XR validation; and
- a protected exact-only/feature-off path.

That tactical should consume the shared coordinator and compiler unchanged
unless game evidence exposes a missing host-neutral contract. It must not
route procedural vegetation through the exact render-section coordinator
merely because the browser game already owns that Worker.

## Acceptance

- Native and browser use the same shared coordinator type and vegetation job
  identity.
- The coordinator and compiler are reusable without a dependency on
  `mclone-world-explorer`, `mclone-web-client`, `winit`, browser APIs, or
  `mclone-scene`.
- No ordinary native or browser World Explorer frame calls
  `TerrainPreviewVegetationProduct::compile_with_cache` synchronously.
- Browser JavaScript/TypeScript constructs and polls a domain-blind Worker
  transport and contains no vegetation vocabulary or scheduling policy.
- Native default execution is threaded; browser default execution is a Web
  Worker. Inline compilation is limited to explicit tests or a documented
  diagnostic fallback.
- Cache ownership is bounded per worker/session and resets on every relevant
  source identity change.
- Stale completions cannot upload instances into reassigned toroidal slots.
- The same pinned view produces matching source revision, admitted stable
  record hash, family counts, tree instance count, and proxy vertex count.
- Movement and zoom retain stable trees; teleport and source changes cancel or
  reject old work deterministically.
- Frame and Worker diagnostics show bounded submission and completion
  admission with no presentation-thread vegetation spike.
- Native, browser, and Wasm validation preserve the standalone dependency
  boundary and the existing exact-tree identity fixtures.
- Documentation records the later `mclone-scene` construction and
  exact/proxy-arbitration boundary without implementing it in this tactical.

## Non-Goals

- Exact/procedural vegetation masking or edit persistence.
- New tree families, proxy art, forest algorithms, or coarse summary quality.
- Shared Wasm heap/allocator ownership across Workers.
- Reusing the removed chunk Far LOD worker.
- Generalizing every project Worker behind one universal abstraction.
- Routing vegetation through the full game's exact render-section
  coordinator.
- Integrating the service into `mclone-scene` or solving exact/procedural
  arbitration in this tactical.
- Migrating Terrain Lab's separate synchronous CPU/reference comparison
  pipeline; it may adopt the reusable service in a later focused cutover.
