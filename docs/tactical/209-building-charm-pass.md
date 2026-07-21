# Tactical 209: Building Charm Pass

Topic: `starter-farmstead-settlement`

Status: **complete 2026-07-21. The seven-state shared detail vocabulary,
version-two cottage/barn/lean-to, ordinary persisted gallery, two visual
iteration loops, and four final inspected production captures are complete.**

## Objective

Respond to the first human visual review of Tactical 208—useful but not yet
especially charming—before broadening into settlement planning. Improve the
standalone cottage first, then carry the same visual language into a restrained
barn refinement, while keeping every new visual piece in the shared block,
template, collision, asset, and persistence paths.

## Visual Diagnosis

The first gallery proved the authoring loop, but its coarse full-block roofs,
empty window apertures, dense dark facade framing, rigid symmetry, and elevated
entries make the buildings read more like diagrammatic prototypes than an
affectionate farmstead. The cottage is the first acceptance target because it
sets the settlement's emotional tone; the barn should remain a simpler working
building rather than compete through equal ornament.

## Scope

- Add only the canonical glass, straight bottom spruce-stair facings, and
  bottom/top spruce-slab states needed by this pass.
- Preserve extracted Minecraft model semantics and give the repo-owned
  first-party visual path honest partial-block geometry/material aliases.
- Add stair/slab outline and collision shapes and transform stair facings with
  template mirror/rotation.
- Reauthor the cottage around a steeper continuous stair roof, glazed
  asymmetric facade, shallower offset porch, flower box, lower working floor,
  restrained timber, and off-center chimney.
- Refine the barn roof edge, glazing, and entry details only after the cottage
  has been rendered and inspected.
- Rebuild the ordinary persisted gallery, capture production screenshots, and
  iterate on visible problems before closing the slice.

## Non-Goals

This pass does not add doors, trapdoors, fences, gates, furniture, structure
starts, site survey, settlement layout, animals, or procedural facade synthesis.
It also does not claim that one pleasing template establishes the final house
style. Human-provided build references may still redirect proportions and
material language later.

## Acceptance Gates

1. New partial and transparent states validate against extracted 1.17.1
   blockstates/models and through the first-party asset inventory.
2. Stair facing transforms and slab/stair collision shapes have focused tests.
3. The cottage is inspected at human scale before the barn refinement begins.
4. Final overview, arrival, cottage, and barn captures are inspected through the
   production renderer.
5. The gallery still persists and reopens as ordinary lit world chunks.

## Validation Ledger

| Date | Evidence | Result |
|---|---|---|
| 2026-07-21 | Minecraft 1.17.1 `StairBlock`, `SlabBlock`, `spruce_stairs` and `spruce_slab` model/blockstate review | straight bottom stair/slab state vocabulary, shapes, and transform direction confirmed |
| 2026-07-21 | `cargo test --manifest-path native/Cargo.toml -p mclone-assets --lib` | passed, 63 tests including extracted blockstate/model coverage for all 221 registered states |
| 2026-07-21 | `cargo test --manifest-path native/Cargo.toml -p mclone-blocks --lib` | passed, 16 tests including exact slab and two-box straight-stair collision/outline shapes |
| 2026-07-21 | `cargo test --manifest-path native/Cargo.toml -p mclone-worldgen --lib structure_template` | passed, 6 focused tests including mirror-plus-rotation stair facing |
| 2026-07-21 | `cargo test --manifest-path native/Cargo.toml -p mclone-mesh --lib` | passed, 86 tests including repo-owned first-party slab/stair partial geometry |
| 2026-07-21 | `cargo test --manifest-path native/Cargo.toml -p mclone-server --lib building_lab` | passed, 3 gallery/template/SQLite reopen tests |
| 2026-07-21 | `cargo check --manifest-path native/Cargo.toml -p mclone-native-client` | passed |
| 2026-07-21 | `cargo check --manifest-path native/Cargo.toml --target wasm32-unknown-unknown --lib -p mclone-server -p mclone-mesh -p mclone-assets` | passed; five pre-existing server cfg/dead-code warnings remain |
| 2026-07-21 | `building_lab_world --root /tmp/mclone-building-lab-v2` | wrote the version-two gallery: cottage 1,320 blocks, barn 2,520, lean-to 321; each crosses four chunks |
| 2026-07-21 | production `mclone-native-client --screenshot ...` captures | overview, arrival, cottage, and barn views captured at 1280x720 and inspected |

## Visual Review Receipt

Final captures live outside the repository:

- `/tmp/mclone-building-charm-overview.png`;
- `/tmp/mclone-building-charm-arrival.png`;
- `/tmp/mclone-building-charm-cottage.png`; and
- `/tmp/mclone-building-charm-barn.png`.

The first cottage milestone replaced the coarse roof and empty windows, but its
three-block entry, broad awning, and blank log-dominated gable still read too
heavily. The accepted second pass narrowed the doorway and porch, exposed the
small right window, and added a split glazed loft window. The first revised
barn placed each stair at the outside of a flat gambrel segment, producing
visible humps. Moving each stair to the inner rising edge produced the accepted
continuous profile; light trim and a narrower bay made the muted red facade
legible without giving the working barn more ornament than the cottage.

The final overview and arrival views establish a materially warmer building
language than Tactical 208. The Flat Grass gallery remains intentionally bare,
and door leaves, fences, props, interiors, and landscape composition remain
outside this pass. Human reference images can still redirect style; this is a
better original baseline, not a declaration of final art direction.
