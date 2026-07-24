# Terrain Lab Canonical Pan Responsiveness

Status: completed 2026-07-25, including inspected desktop/phone pixels and
direct hosted `9x9` responsiveness acceptance.

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

## Implementation Receipt

Canonical view motion and exact desired coverage are now separate. Every
block-level center and camera update queues one coalesced animation-frame
render using the latest view. Exact coverage uses the Euclidean center chunk,
so moving from block `-304` through `-303` changes the URL and projection
without changing the canonical epoch.

The Wasm renderer now retains canonical chunks by absolute coordinate. A
desired-set update removes only departed raw chunks and render sections,
leaving the old/new intersection in the existing section arena. The mesher
also stopped producing presentation block arrays for every resident chunk on
each arrival: it converts only the changed target set and its cardinal
neighbor inputs.

The browser scheduler distinguishes raw-result cache entries from renderer
residency. It begins a one-chunk `9x9` move at 72 published resident chunks,
then schedules only the nine entering coordinates. Cache hits and Worker
results enter one shared queue. One animation-frame callback accepts at most
one chunk, performs one status publication, and requests one coalesced render.
The Worker has one in-flight chunk and pauses while the pending queue reaches
two results.

Hard seed, stage, cache-mode, and explicit cache-epoch changes still clear the
canonical renderer. With cache off, later center-chunk moves preserve their 72
resident overlap but regenerate the nine entering chunks. Radius changes use
the same bounded desired-set diff. Obsolete queued work and Worker results
remain guarded by the request epoch.

The diagnostics panel and browser DOM contract now expose resident reuse,
admission frames, and maximum admissions per frame. This makes a synchronous
cached replay observable as a regression rather than relying only on a
subjective pan impression.

Local validation passed:

- `cargo test --manifest-path native/Cargo.toml -p mclone-terrain-lab
  --lib`: 4 passed;
- `cargo check --manifest-path native/Cargo.toml -p mclone-terrain-lab
  --target wasm32-unknown-unknown`;
- `pnpm --dir tools/terrain-lab test`: 19 passed;
- `pnpm terrain-lab:typecheck`;
- the headed-Wayland Browser WebGPU probe;
- the complete browser suite: 10 passed and 2 platform-inapplicable cases
  skipped; and
- the final cache-off desktop/phone focused proof: 2 passed.

The regression loads 81 final-feature chunks, moves within the current chunk,
moves one chunk, returns through cache, changes to cache off, regenerates the
footprint, and moves once more. It proves:

- sub-chunk motion leaves the epoch unchanged;
- ordinary and cache-off one-chunk moves begin at 72/81;
- the generated entering edge uses nine admission frames and zero cache hits;
- the return edge uses nine admission frames and nine cache hits;
- maximum admission is one chunk per frame; and
- no observed publication count drops below 72 during an ordinary pan.

Local completed-footprint captures were inspected at:

- `/tmp/mclone-terrain-lab-desktop-chrome-canonical-responsive-pan.png`; and
- `/tmp/mclone-terrain-lab-phone-chrome-canonical-responsive-pan.png`.

Both show the complete moved `9x9` final/canonical footprint without a missing
row or column. The visible generated-fallback materials are the existing
first-party atlas state, not a scheduling artifact.

The production bundle was built from `23f2faf6` and uploaded only under
`/terrain/`. Immutable objects were fetched from the public route and
byte-compared before `index.html` was published last:

| Object | SHA-256 |
| --- | --- |
| `index.html` | `89a49724e3a90abd34fe3434c38c0d3136b7515d91b84e887a872e7ec3c7432a` |
| `assets/canonical-worker-CxOArGyk.js` | `f659f7b1567b73406b38437fdc0ae5d8a82df7b104c4e506c9b6243f788b2a6c` |
| `assets/index-CoWCOr4c.css` | `63ee48983f72ded10cfeb1b5891f1da019576678e7059e2f74dc7fb1c70ad343` |
| `assets/index-DbSysi2n.js` | `949686f47ef225984ac02ab215db2179aa9263411d0c3826bab40d070f786ccb` |
| `assets/mclone_terrain_lab_bg-CwsUIxR5.wasm` | `6a6ce0f36c0a4ecd9427a8b58b9012734d8e50db4844cb413f7abfa263901f3d` |

Hosted headed-Wayland desktop and phone standard smokes passed with zero
browser errors, exact CPU/GPU base and final agreement, and nine canonical
admission frames at a maximum of one chunk per frame. A separate direct
hosted `9x9` proof exercised the deployed bundle:

| Hosted lane | Sub-chunk epoch | Generated shift | Cached return | Total proof |
| --- | --- | --- | --- | --- |
| desktop | `1 -> 1` | 72 reused, 9 frames, 0 cache hits | 72 reused, 9 frames, 9 cache hits | 3,974.6 ms |
| phone | `1 -> 1` | 72 reused, 9 frames, 0 cache hits | 72 reused, 9 frames, 9 cache hits | 3,887.2 ms |

Those totals include route load, the initial 81-chunk footprint, the generated
one-chunk shift, and the cached return. Hosted completed-footprint captures
were inspected at
`/tmp/mclone-terrain-lab-hosted-desktop-canonical-responsive-pan.png` and
`/tmp/mclone-terrain-lab-hosted-phone-canonical-responsive-pan.png`.

Implementation commits are `cae35bcc` (contract), `badaeda6` (renderer
retention), `9388b30e` (chunk-keyed paced scheduler), and `23f2faf6`
(cache-off acceptance), followed by the documentation receipt.
