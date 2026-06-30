# Texture Illustrator Guide

This guide is for humans and AI agents authoring original Minecraft-style
textures in `tools/texture-lab`. It describes how to keep related textures
coherent before the renderer or DSL can enforce all of the relationships.

## Core Rule

Draw the source texture for its role, not just for how it looks in isolation.

Some textures are final-color source art. They should look close to their final
in-game color without tinting. Dirt, stone, sand, bark, and most ores are in
this category.

Some textures are tintable source art. They should be neutral enough that a
biome or material tint can multiply into the final color. Grass top, grass side
overlay, foliage, and water-style layers are in this category.

Do not make every texture gray or neutral. Only textures assigned a tint role
should be neutralized for tinting.

## Before Drawing A Block

Write down the relationship graph first:

- block name and resolution
- face roles: top, bottom, side, overlay, particle, or all
- which roles are final-color source art
- which roles are tintable source art
- tint roles used by the block, such as `grass`, `foliage`, or `water`
- normal review tint for each tint role
- tiling mode for each texture: full XY, horizontal strip, or no repeat
- whether rotation or mirroring is expected to work
- nearby blocks that should share material language

Then encode that graph in the DSL with pack-level `tint(...)` declarations and
texture-level `source` / `tintRole` fields. Do not put shared grass or foliage
tints in individual `preview` metadata.

For the current grass block starter, the graph is:

```text
block: grass-block
resolution: 32x32
normal grass review tint: #79b34e

roles:
- top: tintable grass source, full XY tile, rotation preview enabled
- side: final-color dirt base with a small neutral grass transition, horizontal review only
- overlay: tintable transparent grass fringe, horizontal review only
- bottom: final-color dirt tile

composition:
- final top = top multiplied by grass tint
- final side = side base plus overlay multiplied by the same grass tint
- final bottom = bottom as-authored
```

The same relationship appears in source as:

```ts
tint("grass", {
  normal: "#79b34e",
  alternates: ["#5fa343", "#98b85e", "#6fa35b"],
});

texture("grass_block_top", {
  source: "tintable",
  tintRole: "grass",
  // ...
});

texture("grass_block_side", {
  source: "final-color",
  // ...
});

texture("grass_block_side_overlay", {
  source: "tintable",
  tintRole: "grass",
  // ...
});
```

## Tint Roles

A tint role is a semantic promise. If two textures use the same tint role, they
must be authored so the same tint produces compatible final colors.

For `grass`, the current normal review tint is `#79b34e`. This is a lab review
color, not yet a full biome color map. Use it as the main target when judging
source art. Alternate biome swatches are useful, but they should stay secondary
until the base texture relationship is correct.

Tintable source art should preserve value and texture detail more than hue.
For grass, raw source colors can look pale yellow-green, gray-green, or almost
cream. That is expected. The final green appears after tint multiplication.

Final-color source art should not receive a tint. If a brown dirt texture is
multiplied by a grass tint it will become muddy green-brown, which is usually
wrong. Dirt remains brown because the block composition does not assign the
grass tint role to that face.

## Palette Discipline

Use palettes as shared material contracts, not just local color buckets.

- Keep dirt browns coherent across dirt, grass side, grass bottom, coarse dirt,
  paths, roots, and muddy transitions.
- Keep tintable grass source values coherent across grass top and grass side
  overlay, because they receive the same tint.
- Keep stone neutrals coherent across stone, ores, cobble, gravel, and cave
  transitions.
- Add new color names only when they express a real material distinction.

Avoid fixing one texture by inventing a standalone color that only works in one
sheet. If a grass side overlay needs to match the grass top, fix the shared
tintable source range or the tint role, not just one isolated swatch.

## Reviewing Sheets

Individual texture sheets are source-first:

- left enlarged panel: raw source texture
- repeat, mip, rotation, and cube-style panels: preview-tinted when a preview
  tint is defined
- checkerboard: transparent source or overlay review
- seam diagnostic: red/orange marks left-right wrap mismatch, blue marks
  top-bottom wrap mismatch

Block sheets are composition-first:

- cube: final face composition with the primary review tint
- top repeat/rotation: tinted grass top behavior
- side strip: final side after base plus tinted overlay
- terrain patch: repetition and rotation behavior across many blocks
- compact swatches: alternate tint sanity checks, not the main review target

The grass side context sheet shows the important three-part relationship:

- raw side base
- raw transparent overlay
- final composed side using the same grass tint as the top

Judge final grass-side color from that context sheet or the full block sheet,
not from the raw side texture alone.

## Tiling And Rotation

A texture may be intended for full XY tiling, horizontal strip tiling, or no
tiling review.

- Dirt, stone, sand, and grass top usually need full XY tiling.
- Grass side and grass side overlay usually need horizontal strip review.
- Icons, special masks, and one-off overlays may need no tiling panel.

At 32x32, rotation can expose directional brushwork that looked harmless at
16x16. Use rotation previews only for textures where deterministic model
rotation is expected, and inspect the result before relying on it.

## Resolution

Author natural block textures at 32x32 by default, then judge them at 16x16 and
mip levels. A good source texture must survive downsampling.

For 32x32 source art:

- prefer broad material noise plus a few intentional accents
- avoid one-pixel high-contrast details that sparkle at distance
- avoid directional highlights unless the block is orientation-aware
- keep seams quiet before adding decorative detail

## Legal Boundary

Do not trace, recolor, upscale, or mechanically transform Mojang textures into
source art. Local vanilla assets may be used only for private visual reference,
renderer behavior, model roles, tint semantics, and parity checks.

Checked-in source art must be original to mclone or explicitly licensed for
redistribution.

## Agent Brief Template

Use this shape when asking an AI agent to make or revise a texture:

```text
Block:
Resolution:
Texture role:
Source category: final-color or tintable
Tint role:
Normal review tint:
Tiling mode:
Rotation/mirror expectation:
Related textures that must match:
Known problem from the latest sheet:
Allowed edits:
Do not change:
```

For grass changes, always include both `grass_block_top` and
`grass_block_side_overlay` in context when the final green relationship matters.
