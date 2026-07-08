# 159 — Multipart blockstates and huge-mushroom cube rendering

Status: landed

## Problem

Huge-mushroom blocks (`red_mushroom_block`, `brown_mushroom_block`, `mushroom_stem`)
rendered as a single flat quad instead of a cube. In vanilla these are
**multipart** blockstates: `red_mushroom_block.json` etc. are a list of
`{ when, apply }` cases where each matching case contributes one single-face
model (`red_mushroom_block` skin for exposed faces, `mushroom_block_inside` for
interior faces), rotated per direction. The engine had no multipart evaluator —
the mesher's `multipart_*` shim picked one "primary" model per state, and for a
mushroom that primary model is `template_single_face`, i.e. one quad.

## Change

1. **Real multipart parsing** (`mclone-assets/src/block_registry.rs`):
   `BlockStateAsset` now parses the `multipart` array into `MultipartCase`
   (`when` + `apply`) with `MultipartWhen::{And, Or, Match}`. `multipart_selections`
   evaluates a state's properties against every case and returns the first
   `apply` variant of each match (weighted-random `apply` arrays take entry 0).
   A property that is **absent** from a state is treated as `"false"`, so blocks
   without full directional data still resolve their "no active side" fallback
   part instead of matching nothing.

2. **Mesher evaluation** (`mclone-mesh/src/catalog.rs`): the single-primary-model
   shim is gone. Multipart states now bake every matching part, honouring each
   part's `x`/`y` rotation through the existing `BlockStateModelRotation`
   pipeline. This is general — it also drives vine and bamboo (previously handled
   by block-specific special-casing in the shim).

3. **Mushroom directional state**: the three mushroom blocks are registered with
   fixed directional properties — caps `up=true, down=false, N/S/E/W=true`
   (skin on top and all sides, interior on the bottom), stem
   `up=false, down=false, N/S/E/W=true`.

## Parity note: fixed mushroom state vs. per-position state

Vanilla's huge-mushroom features set `up/down/north/...` per block based on the
block's position in the cap (see `HugeRedMushroomFeature`/`HugeBrownMushroomFeature`),
producing up to ~28 distinct states across the two caps. Our worldgen stores one
`RawBlockId` per block (the `RawBlockId → BlockStateId` map is 1:1 over a `u8`),
so representing every per-position state would mean allocating and threading
~28 new block ids through worldgen classifiers, persistence, and the registry.

We use a **single fixed cap state** instead, which is *visually identical* to
vanilla for these features: vanilla only marks a cap face `inside` when it is the
underside (we also set `down=false` → interior) or when an adjacent same-block
neighbour culls it anyway. Every face a cap actually leaves exposed is an outward
rim face (skin) or the underside (interior), so once neighbour culling runs the
rendered pixels match. The divergence is only observable if a player exposes a
formerly-interior face at runtime (e.g. by mining an adjacent cap block), where
vanilla would show the pale inside texture and we show skin. The path back to
full parity is unchanged: give the blocks real per-face state in worldgen.

## Follow-up: glow lichen

`glow_lichen` is also multipart but is placed by worldgen as a single id with no
attachment direction, so it resolves to the vanilla "disconnected" fallback part
(a single face) regardless of which surface it grew on. Rendering is correct for
the data it has; the missing piece is the worldgen feature recording the attached
face (the same per-face-state gap the mushrooms had). Tracked as a future slice.

## Validation

`cargo test -p mclone-assets -p mclone-mesh` (multipart cap/vine/bamboo/fallback
unit tests), plus headless full-frame captures of naturally-generated huge red
and brown mushrooms (dark-forest seeds) showing full cubes on stems.
