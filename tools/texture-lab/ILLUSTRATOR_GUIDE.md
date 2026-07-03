# Texture Illustrator Guide

This guide is for humans and AI agents authoring original Minecraft-style
textures in `tools/texture-lab`. It describes how to keep related textures
coherent before the renderer or DSL can enforce all of the relationships.

Accepted overlay-pack source lives under
`tools/texture-lab/packs/mclone-default/`. Use `tools/texture-lab/examples/`
only for small single-block wrapper entrypoints or experiments.

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
Vanilla terrain textures often use small exact palettes; see
[`VANILLA_COLOR_COUNTS.md`](VANILLA_COLOR_COUNTS.md) before assuming a weak
texture needs more colors. For stone and dirt, authored value placement matters
more than palette size.

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

## Authoring Structure

A natural texture should have macro, mid, and detail layers. Speckles alone are
not enough.

Use low-frequency periodic noise or virtual-resolution ASCII masks for broad
shapes, then add grain and sparse accents. Fully tiled textures need every
procedural field and mask to wrap cleanly. Rotation-safe textures should avoid
directional lighting or one-way streaks; orientation-aware textures should
declare their directionality explicitly.

ASCII masks may be authored below final texture resolution, such as `8x8` or
`16x16`, then upscaled. Use nearest upscaling for chunky pixel forms and
smoothed modes for soft organic masks. Masks can drive opacity and overlay
color now; palette-ramp selection and procedural placement are planned next.

In the current DSL, use `macroNoise(...)` before detail grain and `mask(...)`
for virtual-resolution authored forms. Treat both as structural layers. If the
repeat or rotation sheet shows an obvious stamped motif, reduce opacity, change
the mask scale, or revise the mask before adding more speckles.

Ore textures should be composed from the base stone or deepslate material plus
ore masks, so they inherit palette and noise from the host rock instead of
becoming unrelated standalone tiles.

For ore masks, keep the host rock layer identical to the base block, then add
only the mineral shape. Avoid placing ore pixels directly on tile edges unless
the cluster is intentionally wrapping; edge fragments are easy to spot in the
seam and repeat panels.

For stone and rock, do not rely on speckles as the main form. Start with
connected planes or chunks, add short dark/light edge pairs for shallow relief,
then use speckles only as final grain. If the enlarged source panel looks like a
flat gray field with dots, the texture is not structurally authored yet.

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

When local vanilla assets exist, individual texture sheets include a bordered
reference panel. Use it to judge density, value range, material scale, and the
kind of shapes the texture needs. Do not trace, recolor, or mechanically copy
the reference; the authored source must remain original.

## Texture Iteration Loop

The lab is meant to be run as a loop, not a one-shot. An agent that renders one
texture, looks at it, and decides "good enough" will almost always stop too
early: an LLM's absolute "is this good?" judgment is lenient and unreliable. The
same model is far more reliable at *comparison* — "which of these is closer to
vanilla?" and "did this change shrink the gap?" Drive the loop on measured gaps
and comparison, never on an absolute verdict.

Run the analyzer to get objective numbers for a candidate against its local
vanilla counterpart:

```sh
pnpm texture-lab:analyze --texture stone --texture dirt
```

The candidate is area-downsampled to the vanilla grid (usually 16x16) before
measuring, so every number is apples-to-apples — a 32x32 tile is neither
rewarded nor punished for simply having four times the pixels. Beyond value and
structure, the report now also measures **color** (dominant hue, hue spread,
saturation, warm-cool cast), **structure quality** (grain direction/strength
for streaks — including the diagonals the row/col banding test misses; color
run length for connected planes vs speckle; repetition peak for stamped
motifs; top-8 color share for palette concentration; mid/coarse retention and
value skew for scale and tone shape), and common **defects** (sparkle pixels,
one-way lighting bias). Each report ends with a `biggest gaps vs vanilla`
list: the features the candidate sits furthest from vanilla on, worst first.

Two of those numbers map directly to recurring authoring failures. A `color run
length` near 1 with low `grain strength` is the "flat field plus dots" texture
that has not been structurally authored yet — vanilla lays its few tones out in
connected planes and strokes. A high `repetition peak` is the stamped-motif
artifact that otherwise only shows up when a human stares at the repeat panel.

The loop:

1. Author or revise the source, export, and analyze.
2. Read the `biggest gaps` list. Change the source to shrink the **top** gap —
   not everything at once.
3. Re-export, re-analyze, and confirm that gap moved without opening new ones.
4. Compare against your **previous attempt**, not just vanilla: is this version
   closer? Keep the better one; discard regressions. Let the tool decide it
   mechanically rather than by eye:

   ```sh
   pnpm texture-lab:analyze --compare prev.png next.png
   ```

   It ranks both against the vanilla counterpart (inferred from the filename, or
   set with `--reference-name <block>` / `--reference-png <path>`) and prints a
   per-feature `closer` column and a `verdict` for which one to keep iterating
   from.
5. Repeat for at least a few rounds. Stop on **diminishing returns** (the gaps
   stop shrinking), not when the texture "looks fine".
6. Before calling a texture done, state the **top three remaining differences**
   from vanilla out loud. If you cannot, you have not read the numbers.

The numbers guide; they do not gate. The `vs` column and the gap list are
directional hints (`↑` higher than vanilla, `↓` lower, `•` close). A strong
texture can legitimately sit outside the vanilla range — an outlier is
information to look at, not a failure. There is deliberately no pass/fail
threshold. The goal is an informed decision, not a number you hit.

Defects are different from taste. A broken seam (`seam left-right` /
`seam top-bottom` well above 0), `sparkle pixels`, a one-way `lighting bias`,
an oversized `dark/light blob max %`, a `repetition peak` well above the
vanilla counterpart (stamped motif), or a strong `grain strength` on a tile
that must be rotation-safe are usually authoring mistakes, not style choices.
Treat those as must-fix-or-justify even when the aesthetic features look
right. Hue, saturation, contrast, and scale are taste — keep those directional
and use judgment.

### Presenting Options

When a block matters, or when the right direction is ambiguous, do not silently
commit one texture. Produce two or three distinct candidates, rank them with
`analyze --compare` (pairwise against the vanilla counterpart), and present them
to the user with their measured differences and a short recommendation, so the
user makes the final call or asks for "more of that one". Comparing a few
concrete options is a more reliable judgment than scoring one in isolation —
both for the agent and for the user.

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
Latest analyzer gaps (from texture-lab:analyze):
Allowed edits:
Do not change:
```

Do not treat one pass as the deliverable. Iterate against the analyzer's
`biggest gaps` list until the gaps stop shrinking, then present two or three
candidates with their measured differences for the user to choose. See
[Texture Iteration Loop](#texture-iteration-loop).

For grass changes, always include both `grass_block_top` and
`grass_block_side_overlay` in context when the final green relationship matters.
