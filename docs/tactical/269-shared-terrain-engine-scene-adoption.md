# Tactical 269: Shared Terrain Engine Scene Adoption

## Status

Active 2026-07-27. This is PH-4 from parent Tactical
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
- The procedural horizon renders after sky and before live exact opaque
  terrain on the same reversed-Z depth target. Exact terrain, actors, and
  translucent passes retain the ordinary frame order.
- The game retains an exact-only mode with unchanged pixels and zero horizon
  scheduling. Composed mode is explicit during validation and becomes a
  product default only after the review gate accepts it.
- Scene source identity includes generator/profile, seed when legitimately
  known, topology, authority/freshness role, and a non-zero generation.
- Platform adapters construct native-thread or browser-Worker executors only.
  They do not copy terrain policy.
- Mono, independent flat views, per-eye stereo, and full-frame multiview
  consume one committed terrain presentation. View matrices differ; residency
  and source generations do not.

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

- Derive exact-painted columns from drawable, traversal-ready client render
  sections rather than requested or merely loaded chunks.
- Qualify the snapshot with active world identity, source generation,
  topology, and authoritative role.
- Reset atomically on session/world/source replacement and reject stale
  products.
- Preserve edits and propagated exact lighting by retaining the live draw
  store.

### Slice 3: ordinary scene composition

- Insert the shared procedural terrain submission into the ordinary frame
  after sky and before exact opaque terrain.
- Anchor residency to the primary player/view focus while allowing auxiliary
  and stereo views to reuse the committed presentation.
- Preserve actors, translucent terrain, water, effects, selection, and UI
  ordering.
- Add explicit exact-only/composed configuration and diagnostics.

### Slice 4: ownership, lifecycle, and topology

- Feed authoritative readiness into complete-record natural-feature XOR.
- Invalidate or reselect ownership when edits, section readiness, source
  generation, topology, world, or device resources change.
- Exercise movement, teleport, view-distance changes, session replacement,
  and device rebuild without stale horizon or proxy facts.

### Slice 5: native and stereo evidence

- Run focused unit/workspace gates and native Wasm compilation.
- Capture and inspect low-angle tree and steep-hill silhouettes at the first
  drawable game milestone.
- Compare exact-only and composed mode under matched inputs.
- Prove independent flat views and synthetic stereo share residency while
  retaining per-view projection/depth correctness.

### Slice 6: full web game review surface

- Carry the shared scene configuration and source adapter through the browser
  game.
- Expose deterministic URL inputs for exact-only/composed mode and the review
  location without moving terrain policy into TypeScript.
- Validate headed desktop/phone WebGPU pixels and semantic receipts.
- Deploy the full web game and stop at the first subjective review checkpoint.

## Human review checkpoint

The first required review is the hosted full web game after Slice 6. The fixed
URL must make it possible to compare exact-only and composed presentation at
one low-angle site. Review:

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
