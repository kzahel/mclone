# Terrain Lab Canonical Pan Responsiveness

Status: active 2026-07-25.

Topic: `gpu-procedural-terrain`

## Objective

Keep Terrain Lab navigation responsive while a large canonical footprint is
resident, loading, or returning from cache.

The completed first slice must:

- move the canonical camera immediately without rebuilding exact coverage for
  every block-coordinate change;
- preserve resident chunks and GPU sections shared by the old and new desired
  footprints;
- change desired exact coverage only when the view center crosses a chunk
  boundary or another generation identity changes;
- pace cached and newly generated chunk admission across animation frames;
- retain center-first progressive publication, bounded residency, cache-off
  truth, and stale-request rejection;
- render and report at most once for one admission frame; and
- prove that a loaded `9x9` footprint can move by one chunk without clearing
  to zero or synchronously replaying 81 cached chunks.

## Originating Observation

Human review found that panning a `9x9` final/canonical footprint locks the UI.
World generation already runs in a Web Worker, but the rest of the path does
not:

1. every block-level center update terminates the current Worker epoch;
2. the main thread clears all retained exact chunks and GPU sections;
3. cache hits are synchronously reaccepted in one tight loop;
4. each acceptance copies blocks and biomes into Wasm, filters presentation
   blocks, meshes the new chunk and immediate neighbors, uploads sections,
   renders, and publishes React status; and
5. the next Worker chunk is requested immediately after that synchronous work.

The nine-chunk hosted smoke already accumulated roughly 212--264 ms of
main-thread mesh-plus-upload work. Expanding the footprint to 81 chunks makes a
single synchronous cached replay visibly hostile to pointer and UI events.
Completion may remain expensive; event-loop starvation is not acceptable.

## Product Contract

### View And Coverage Are Separate

The shared integer center remains the visible camera target and URL identity.
Canonical drawing follows it immediately.

Exact generation is keyed by:

- seed;
- canonical stage;
- canonical radius;
- cache epoch and enabled state; and
- center chunk, using Euclidean division by 16.

Moving within the same center chunk must not restart generation, clear
residency, or change the canonical request epoch. Crossing into a neighboring
center chunk creates a new desired footprint, but does not invalidate chunks
that belong to both footprints.

### Resident Desired-Set Diff

The canonical renderer retains raw chunk facts and uploaded section meshes by
absolute chunk coordinate. On a desired-footprint change it:

1. keeps the old/new intersection drawable;
2. removes chunks outside the new bounded desired set;
3. schedules cached or missing desired chunks center-first; and
4. admits new chunks progressively.

A one-chunk shift of a complete `9x9` footprint therefore begins with 72 of 81
desired chunks already published and requests only the nine entering chunks.
Resident rendering remains bounded by the selected exact footprint in this
slice. A predictive halo or LRU may be added later, but is not required to
remove the current freeze.

Raw-result caching and GPU residency remain distinct. Returning to an evicted
coordinate may use the raw cache, but cached results enter the same paced
admission queue as Worker results; cache is never a synchronous replay path.

### Admission And Backpressure

The browser owns a small pending-result queue. One animation-frame callback
admits at most one chunk, then performs one render and one report publication.
The generation Worker keeps at most one chunk in flight and does not produce
another result while the main-thread pending queue is above its small
high-water mark.

This is a responsiveness boundary, not a claim that one current main-thread
mesh/upload fits within a 60 Hz frame. A single chunk may still create a hitch,
but no admission task may concatenate an entire footprint. Diagnostics record
resident reuse and maximum admissions per frame.

### Invalidation

Seed, stage, explicit cache clearing, and cache-off reruns remain hard
invalidation boundaries and may clear exact residency. Radius changes retain
only chunks inside the new desired set and schedule the rest. Center changes
replace obsolete queued work while preserving useful resident overlap.

Worker epoch checks remain authoritative. A stale result can never enter the
new desired set even if it arrives after cancellation.

## Ownership

- `tools/terrain-lab` owns view-to-center-chunk identity, Worker
  backpressure, animation-frame admission, status coalescing, and browser
  responsiveness tests.
- `mclone-terrain-lab` owns absolute canonical raw/GPU residency and bounded
  coordinate retention.
- `mclone-terrain-view`, `mclone-mesh`, and `mclone-render` retain generation,
  mesh, and section-upload semantics.

The React application does not reconstruct terrain or mesh facts. The
canonical renderer does not invent a second scheduler.

## Implementation Slices

1. Land this contract and baseline the current synchronous path.
2. Add renderer-side desired-coordinate retention without full reset.
3. Key coverage scheduling by center chunk while camera rendering follows the
   exact block center.
4. Queue cache and Worker results behind one-per-animation-frame admission and
   bounded Worker backpressure.
5. Remove duplicate renders and coalesce per-admission reports.
6. Add desktop/phone regression coverage for sub-chunk motion, one-chunk
   `9x9` overlap, cached return, progressive admission, and zero browser
   errors.
7. Capture and inspect the moving large canonical footprint, run local and
   hosted validation, deploy only `/terrain/`, and record the receipt.

Commit each coherent slice with `Topic: gpu-procedural-terrain`.

## Acceptance

- Sub-chunk camera movement changes the URL and pixels without changing the
  exact request epoch.
- A complete radius-four footprint shifted one chunk reports 72 resident hits
  before the nine entering chunks publish.
- Returning one chunk reports the same overlap and admits cached chunks over
  multiple animation frames, never in one tight loop.
- Maximum admissions observed in one animation frame is one.
- The old footprint is never cleared to zero during an ordinary pan.
- Cache-off still regenerates missing entering chunks.
- Final canonical fingerprints and rendered materials remain unchanged.
- Desktop and phone headed-WebGPU captures show a correct complete footprint
  after movement.

## Explicit Follow-Ups

If one paced admission still produces unacceptable long tasks, move
presentation filtering and border-aware textured section meshing into the
canonical Worker and transfer section vertex/index payloads to the main
thread. GPU upload would remain frame-budgeted. Moving the entire renderer to
a Worker-owned `OffscreenCanvas` is a later platform-dependent option, not a
prerequisite for responsive navigation.

