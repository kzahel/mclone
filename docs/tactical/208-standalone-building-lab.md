# Tactical 208: Standalone Building Lab

Topic: `starter-farmstead-settlement`

Status: **complete 2026-07-21. The shared transform/material-role kernel,
minimal full-cube building palette, original cottage, barn and lean-to,
ordinary lit persisted gallery, SQLite reopen proof, and four inspected
production captures are complete.**

## Objective

Prove the farmstead's smallest useful authoring loop before site selection or
whole-settlement generation: author attractive standalone buildings as mostly
complete templates, apply only controlled rotation, mirror, material-theme,
and attachment variation, write the result through ordinary world
persistence, and inspect it through the production renderer.

This first visual pass is intentionally designed without external build-image
references. It establishes an honest baseline for what the native vocabulary
can produce. Later human-provided references may guide proportions, roof
language, material choices, and detail without replacing the reusable kernel.

## Scope

This tactical owns:

- a shared, pure structure-template kernel in `mclone-worldgen`;
- local block records, semantic material roles, material themes, markers,
  mirror/rotation transforms, directional-state transforms, bounds, and exact
  touched-chunk discovery;
- a deliberately small full-cube building kit: oak and spruce planks,
  cobblestone, stone bricks, and vertical hay bales;
- one authored cottage and one authored barn, with a small optional barn
  attachment demonstrating bounded modularity;
- a deterministic Flat Grass building gallery materialized into normal lit
  chunk records and native SQLite persistence; and
- inspected production screenshots from overview and human-scale views.

The template vocabulary is informed by Minecraft Java 1.17.1's separation of
template records, palettes, placement settings, transforms, markers, bounds,
and processors. The content itself is original and does not copy vanilla
village templates.

## Explicit Non-Goals

This slice does not yet implement:

- structure starts, references, pieces, disk template codecs, jigsaw pools, or
  placement at chunk statuses;
- arbitrary procedural building dimensions or unconstrained facade synthesis;
- site survey, grading, ponds, streams, roads, paddocks, or a full settlement;
- stairs, slabs, doors, trapdoors, fences, gates, glass panes, farmland, or
  growing crops;
- resident animals, gameplay markers, block entities, loot, or interiors with
  working furniture; or
- cross-profile placement claims.

Those remain later slices. This lab should make them easier to add without
prematurely binding art iteration to a large world-generation lifecycle.

## Accepted Authoring Model

Buildings begin as mostly complete authored stamps. Variation is controlled:

- quarter-turn rotation and optional axis mirror;
- semantic role substitution through a coherent material theme;
- transform-aware horizontal logs and wall-mounted directional states;
- authored markers for entrances and future attachments; and
- a small number of optional modules selected by a future composition recipe.

Dimensions are fixed per template in this slice. Future families may expose a
few bounded variants such as short/long barn or porch/no porch, but should not
stretch arbitrary dimensions until roof, facade, and interior constraints have
a convincing grammar.

## Slice Checklist

### Slice 0 — Kernel and palette

- [x] Add ordered local records with later-write override semantics.
- [x] Add semantic material roles and required-role validation.
- [x] Add mirror/rotation transforms and transformed bounds.
- [x] Transform horizontal log axes and wall-torch facings.
- [x] Transform markers and expose exact touched chunks.
- [x] Add only the five full-cube building states needed by the first gallery.
- [x] Run focused `mclone-worldgen` and full `mclone-assets` tests.

### Slice 1 — Original cottage and barn

- [x] Author a complete cottage template with a readable foundation, timber
  frame, gable roof, entrance, windows, chimney, and porch intent.
- [x] Author a complete barn template with a larger silhouette, broad entry,
  loft opening, timber frame, roof, hay, and one optional side attachment.
- [x] Keep template construction deterministic and test characteristic blocks,
  markers, bounds, and cross-chunk reach.

### Slice 2 — Persistent gallery

- [x] Place both templates through the shared kernel on a deterministic Flat
  Grass gallery with paths and restrained planting.
- [x] Produce ordinary lit `ChunkRecord`s and save them through
  `WorldStore`/SQLite under an independently marked rebuild-safe directory.
- [x] Reopen the world and verify persisted template landmarks.

### Slice 3 — Visual review and closeout

- [x] Capture and inspect an overview, arrival-height composition, cottage, and
  barn view through `mclone-native-client`.
- [x] Iterate obvious silhouette, proportion, occlusion, or camera problems.
- [x] Record screenshot paths, commands, limitations, and recommended next
  direction in this tactical and the living topic.
- [x] Commit only the tactical's files under the shared topic trailer.

## Acceptance Gates

The tactical closes only when:

1. the same template input produces byte-for-byte stable ordered placements;
2. rotation, mirror, semantic themes, markers, directional states, transformed
   bounds, and touched chunks have focused tests;
3. the gallery is normal persisted world data, not debug renderer geometry;
4. a saved gallery reopens with the expected cottage and barn landmarks;
5. production screenshots have been inspected and at least one visual
   refinement pass has been made when warranted; and
6. the living topic no longer treats mountains, valleys, periodic terrain, or
   hydrology as blockers for standalone building iteration.

## Validation Ledger

| Date | Evidence | Result |
|---|---|---|
| 2026-07-21 | `cargo test --manifest-path native/Cargo.toml -p mclone-worldgen --lib` | passed, 261 tests; 1 existing gauntlet ignored |
| 2026-07-21 | `cargo test --manifest-path native/Cargo.toml -p mclone-assets --lib` | passed, 63 tests including extracted Minecraft model/blockstate coverage |
| 2026-07-21 | `cargo test --manifest-path native/Cargo.toml -p mclone-server --lib` | passed, 531 tests including three building-lab persistence tests |
| 2026-07-21 | `cargo check --manifest-path native/Cargo.toml --target wasm32-unknown-unknown -p mclone-server --lib` | passed; five pre-existing cfg-specific warnings remain |
| 2026-07-21 | `cargo run --manifest-path native/Cargo.toml -p mclone-server --bin building_lab_world -- --root /tmp/mclone-building-lab` | wrote 49 ordinary lit chunks; cottage 1,245 blocks/4 chunks, barn 2,518 blocks/4 chunks, lean-to 347 blocks/4 chunks |
| 2026-07-21 | `mclone-native-client --screenshot ... --world-dir /tmp/mclone-building-lab --generation-profile authored-only ...` | production renderer captured and inspected overview, arrival, cottage, and barn at 1280x720 |

## Visual Review Receipt

Final inspected captures live outside the repository:

- `/tmp/mclone-building-lab-overview.png`;
- `/tmp/mclone-building-lab-arrival.png`;
- `/tmp/mclone-building-lab-cottage.png`; and
- `/tmp/mclone-building-lab-barn.png`.

The first overview and close cottage view showed that a five-block-deep flat
porch canopy visually compressed the facade. The accepted refinement replaced
it with a shallower stepped porch gable. The final views establish a useful
blind baseline: the cottage has a distinct warm timber/plaster silhouette and
chimney, while the much larger red working barn has an open central bay, loft,
hay, and clearly subordinate lean-to.

Known visual constraints are intentional tactical boundaries. Full-cube roofs
read as coarse steps; open apertures stand in for doors and glazing; the Flat
Grass gallery has no scenic horizon; and neither building has a gameplay-rich
interior. These are useful evidence for selecting the next coherent content
slice, not defects to hide with fixture-only renderer geometry.

## Recommended Next Direction

Obtain human visual feedback on this blind baseline before broadening the block
kit. After that review, prefer one small coherent detail slice—likely roof
shapes plus doors/windows, or the fence/gate family—over starting the whole site
planner at once. True structure starts/references and terrain work remain
independent upstream lanes rather than prerequisites for this art review.
