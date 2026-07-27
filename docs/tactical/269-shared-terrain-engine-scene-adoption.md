# Tactical 269: Shared Terrain Engine Scene Adoption

## Status

Human Review checkpoint ready 2026-07-27. This is PH-4 from parent Tactical
[`261`](261-procedural-horizon-product-integration-roadmap.md), following the
accepted World Explorer composition proof in Tactical
[`262`](262-world-explorer-exact-procedural-composition.md) and Terrain Lab
runtime consumer in Tactical
[`266`](266-terrain-lab-runtime-composition-adoption.md).

## Objective

Converge World Explorer, Terrain Lab runtime composition, and the live game
scene on one source-qualified terrain representation/composition engine.
Preserve Explorer as a cheap detached host and the game as the owner of live
authority, lighting, edits, actors, and lifecycle.

The end state is not two renderers sharing a coverage DTO:

```text
detached canonical source       live authoritative source
            \                         /
             `--> mclone-terrain-view
                  residency + coverage + frontier
                  representation ownership
                  prepared terrain frame
                             |
          .------------------+------------------.
          v                  v                  v
     World Explorer    Terrain Lab runtime   mclone-scene
```

## Binding decisions

- `mclone-terrain-view` owns the logical terrain representation engine,
  source qualification, procedural residency, exact-painted coverage,
  frontier policy, bounded natural-feature ownership, and prepared terrain
  submissions.
- Detached canonical generation and the live client replica are narrow source
  adapters. The live path never regenerates authoritative exact chunks from a
  seed, and the detached path never claims edits or authority.
- Exact section drawing continues through shared `mclone-render` terrain
  machinery. The game reuses its existing draw store and propagated lighting;
  Explorer may retain inferred/fullbright preview lighting.
- `TerrainRuntimeExactRenderer` is a transitional compatibility name. Split
  its canonical production/residency from reusable frame preparation, then
  migrate Explorer and Terrain Lab before removing or narrowing the aggregate.
- Live exact opaque/cutout terrain establishes the ordinary reversed-Z depth,
  then the procedural horizon loads that same color/depth target before
  actors and exact translucent terrain. This avoids a browser failure mode
  where exact terrain recreated rather than loaded the procedural depth.
- The game retains an exact-only mode with unchanged pixels and zero horizon
  scheduling. Composed mode is explicit during validation and becomes a
  product default only after the review gate accepts it.
- Scene source identity includes generator/profile, seed when legitimately
  known, topology, authority/freshness role, and a non-zero generation.
- Platform adapters construct native-thread or browser-Worker executors only.
  They do not copy terrain policy.
- Ordinary mono and per-eye stereo consume one committed terrain
  presentation. Independent multi-flat and full-frame multiview remain PH-7
  and PH-8 promotion work; view matrices may differ there, but residency and
  source generations must not.

## Slices

### Slice 0: source-qualified shared contract

- Add explicit detached, authoritative, and bounded-observer truth roles.
- Add a validated prepared terrain-source/frame envelope.
- Lock source, generation, topology, and coverage agreement with tests.
- Record the child tactical and shared ownership before renderer changes.

### Slice 1: detached source and shared engine split

- Separate canonical exact production/admission from exact draw preparation.
- Put the horizon compositor behind an external-view-capable shared engine.
- Migrate World Explorer and Terrain Lab runtime composition.
- Prove their current diagnostics, Worker contracts, and accepted pixels do
  not regress.

### Slice 2: live authoritative source adapter

- [x] Derive exact-painted columns from drawable, traversal-ready client render
  sections rather than requested or merely loaded chunks.
- [x] Qualify the snapshot with active world identity, source generation,
  topology, and authoritative role.
- [x] Reset atomically on session/world/source replacement and reject stale
  products.
- [x] Preserve edits and propagated exact lighting by retaining the live draw
  store.

### Slice 3: ordinary scene composition

- [x] Insert the shared procedural terrain submission into the ordinary mono
  frame between exact opaque/cutout and actors/translucent terrain.
- [x] Anchor residency to the authoritative player focus rather than a
  diagnostic camera target.
- [x] Allow per-eye stereo views to reuse the committed presentation.
- [ ] Promote the committed presentation through browser multi-flat and
  full-frame multiview after their explicit capacity/pipeline work.
- [x] Preserve actors, translucent terrain, water, effects, selection, and UI
  ordering.
- [x] Add explicit exact-only/composed configuration and diagnostics.

First drawable evidence on 2026-07-27 uses
`--generation-profile mclone-overworld-v1 --terrain-presentation composed`.
The ordinary scene now draws exact opaque terrain first, then lets procedural
terrain load and extend the same reversed-Z depth. An inspected `1024x576`
low-angle native
capture preserves the exact foreground hill silhouette over the procedural
horizon; an elevated capture exposes the bounded exact footprint surrounded by
the fixed-budget horizon. `exact-only` remains the default and never creates
the terrain-view engine. Procedural vegetation was intentionally disabled at
this first pixel milestone; Slice 4 subsequently supplied platform executors.

### Slice 4: ownership, lifecycle, and topology

- [x] Feed authoritative readiness into complete-record natural-feature XOR.
- [x] Invalidate or reselect ownership when edits, section readiness, source
  generation, topology, world, or device resources change.
- [x] Exercise ordinary movement and source resets without stale products.
- [ ] Carry the same recovery through the dedicated PH-6 teleport,
  view-distance, session-replacement, and device-rebuild campaign.

The live engine now treats exact-ready coverage as authoritative for a whole
natural tree record: it owns both a present exact tree and an edited exact
absence, while resident procedural products own complete proxies outside that
coverage. Native uses the shared named-thread executor moved out of World
Explorer. Browser construction supplies the same coordinator with a dedicated
Wasm Worker at the app rim.

The browser initially lost its WebGPU device with `A valid external Instance
reference no longer exists.` Retaining the `wgpu::Instance` for the surface
lifetime is correct, but was not the root capacity failure.
`TerrainViewportRenderer` was compiling both detached viewport and horizon
pipeline families even though a live scene uses only the latter. On top of the
game shader set this crossed Chrome's current WebGPU pipeline capacity and
lost the device. Construction now selects exactly one pipeline family.

The browser live-game tier uses six clipmap levels and an eight-sample
presentation stride. It retains the same source, computed products, exact
coverage, ownership, and roughly eight-kilometre horizon while drawing 96
coarser mesh tiles. Native retains ten levels and stride one. This is an
explicit device presentation budget, not a second terrain implementation.

### Slice 5: native and stereo evidence

- [x] Run focused unit gates and native Wasm compilation.
- [x] Capture and inspect low-angle tree and steep-hill silhouettes at the first
  drawable game milestone.
- [ ] Compare exact-only and composed mode under matched inputs at the hosted
  review site.
- [x] Prove synthetic per-eye stereo shares residency while retaining
  per-view projection/depth correctness.

The preliminary `1280x640` synthetic per-eye capture (two `640x640` eyes)
draws the exact foreground and procedural horizon in both eyes with 226,475
differing pixels. It advances PH-8's first topology gate but does not complete
PH-8: desktop OpenXR, Quest, and full-frame multiview remain their own later
promotion. The current horizon render pipeline is single-view; composed mode
is intentionally unavailable in full-frame multiview until that pipeline
consumes `view_index` and a two-layer target. Exact-only multiview remains
unchanged.

The browser auxiliary-split probe was also run as a falsification check. Two
horizon pane submissions currently lose Chrome's WebGPU device during the UI
switch, so multi-flat composition is deliberately not claimed by PH-4. PH-7
must batch or otherwise budget the repeated horizon submission before
enabling it; the existing exact-only auxiliary path remains unchanged.

### Slice 6: full web game review surface

- [x] Carry the shared scene configuration and source adapter through the browser
  game.
- [x] Expose deterministic URL inputs for exact-only/composed mode and the review
  location without moving terrain policy into TypeScript.
- [x] Validate headed desktop WebGPU pixels and semantic receipts.
- [ ] Validate the representative phone browser after desktop review.
- [x] Deploy the full web game and stop at the first subjective review checkpoint.

Production deployment `f0cdc1e4-5d1d-4389-a3ff-2926e04ae69f` serves bundle
asset version `cdf56dceed78-20260727132018`. The public app and new vegetation
Worker return the required COOP/COEP/CORP headers.

## Human review checkpoint

The first required review is the hosted full web game after Slice 6. The fixed
URL must make it possible to compare exact-only and composed presentation at
one low-angle site. Review:

- composed:
  `https://mclone.kzahel.com/app?startInWorld=1&generationProfile=mclone-overworld-v1&terrainPresentation=composed`
- exact-only control:
  `https://mclone.kzahel.com/app?startInWorld=1&generationProfile=mclone-overworld-v1&terrainPresentation=exact-only`

1. exact trees and steep hills correctly occlude farther procedural terrain;
2. nearer procedural terrain correctly occludes exact geometry;
3. the exact/procedural frontier has no holes, full-height wall, or
   representation chimera;
4. movement does not flash a missing forest or coarse fallback;
5. exact lighting, water, actors, effects, and UI still look like the game;
6. an authoritative edit near the frontier remains exact while covered; and
7. only the already-recorded outermost-block z-fighting remains.

No earlier human review is planned unless implementation evidence exposes a
new authority or product decision.

## Exit condition

PH-4 is complete when Explorer, Terrain Lab runtime composition, and the live
scene consume one shared terrain representation/composition owner through
different qualified sources; exact-only remains equivalent; native, web, and
synthetic-stereo evidence passes; and the hosted full-game checkpoint is ready
for subjective acceptance.
