# Tactical 256: Shared Horizon Vegetation Worker Topology

Status: proposed; architecture checkpoint required before implementation.

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

`mclone-terrain-view` should own the platform-neutral coordinator, request and
completion identity, admission rules, and renderer handoff.

`mclone-worldgen` continues to own deterministic vegetation planning,
`McloneOverworldVegetationPlanCache`, summaries, records, and source
revisions.

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

## Implementation Order

1. Extract synchronous vegetation compilation from renderer encoding into a
   pure job compiler callable by either executor.
2. Add the shared coordinator with deterministic desired-work, priority,
   revision, cancellation, and acceptance tests.
3. Implement the native threaded mailbox and prove no vegetation planning
   occurs on the render thread.
4. Reuse the generic browser Worker transport and add the worker-resident Rust
   vegetation actor plus bounded result mailbox.
5. Enable the same Explorer vegetation product configuration on both hosts.
6. Add tree identity/count/hash diagnostics independent of draw order.
7. Exercise movement, zoom, teleport, stale completion, Worker failure,
   overflow, shutdown, and source changes.
8. Capture and inspect matched native and headed-browser tree views.

## Acceptance

- Native and browser use the same shared coordinator type and vegetation job
  identity.
- No ordinary native or browser frame calls
  `TerrainPreviewVegetationProduct::compile_with_cache` synchronously.
- Browser TypeScript constructs/polls a domain-blind Worker transport and
  contains no vegetation vocabulary or scheduling policy.
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

## Non-Goals

- Exact/procedural vegetation masking or edit persistence.
- New tree families, proxy art, forest algorithms, or coarse summary quality.
- Shared Wasm heap/allocator ownership across Workers.
- Reusing the removed chunk Far LOD worker.
- Generalizing every project Worker behind one universal abstraction.
