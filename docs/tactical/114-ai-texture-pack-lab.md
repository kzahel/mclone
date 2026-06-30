# 114 - AI Texture Pack Lab

Status: active; scaffold/export/sheet loop, software cube, rotation-grid panels,
first grass-block bundle, primary-tint terrain patch, source/tint split review
sheets, texture illustrator guide, DSL tint-role metadata, and metadata reports
plus seam diagnostics landed on 2026-06-30.

## Purpose

Create a disposable, fast-iteration texture lab for original Minecraft-style
block textures. The lab should help an agent or human author many related
textures, preview tiling and block appearance, evaluate 32x32 source art
against 16x16/mip readability, and export derived PNG/resource-pack payloads
without ever committing Mojang-owned textures.

Workstream: documentation and isolated tooling under `tools/texture-lab/`.
Runtime adoption, when justified, must go through shared asset/render owners
such as `mclone-assets`, `mclone-render-session`, and `mclone-render`.

## Context

The existing `tools/asset-lab` proved a useful pattern for AI-authored visual
assets:

- TypeScript DSL as the editable source.
- Three.js as a preview implementation detail.
- Playwright screenshots and sheets written under `/tmp`.
- Small generated JSON/PNG outputs rather than arbitrary renderer code.

Texture authoring has different needs than primitive figures. Natural block
textures need seamless tiling, mip/downsample readability, palette coherence,
biome tint review, rotation/mirror review, and repeated terrain-patch review.
The source format should therefore be hybrid rather than pure ASCII:

- indexed ASCII for compact masks and hand-authored pixel details
- palette ramps for material families
- deterministic procedural helpers for tileable natural variation
- explicit texture roles for per-block bundles

## Reference Facts

Minecraft Java 1.17.1 commonly uses 16x16 block textures. For grass block, the
blockstate has four deterministic Y rotations for `snowy=false`, and the model
uses separate dirt bottom, grass top, grass side, and tinted side overlay
textures. Grass, fern, leaves, and water use biome tint through block color
logic.

Many vanilla blocks hide repetition with deterministic rotation or mirroring
rather than several texture PNG variants. At 32x32, those rotations may expose
directional details, so the lab should explicitly compare:

- one texture plus auto-rotation/mirroring
- several authored variants with stable per-position selection
- no rotation for orientation-sensitive materials

Current native mesh catalog behavior chooses the first blockstate variant for
textured block models. Runtime-visible variant support is therefore a separate
shared asset/meshing task and is not required for the initial lab.

## Target Shape

Authoring files:

```text
tools/texture-lab/ILLUSTRATOR_GUIDE.md
tools/texture-lab/examples/dirt/texture.ts
tools/texture-lab/examples/grass-block/texture.ts
```

Generated review outputs:

```text
/tmp/mclone-texture-lab/dirt-sheet.png
/tmp/mclone-texture-lab/grass-block-sheet.png
/tmp/mclone-texture-lab/grass-block-side-sheet.png
/tmp/mclone-texture-lab/*-metadata.md
/tmp/mclone-texture-lab/*-metadata.json
/tmp/mclone-texture-lab/pack/assets/mclone/textures/block/*.png
```

Likely root commands after implementation:

```powershell
pnpm texture-lab:install
pnpm texture-lab:typecheck
pnpm texture-lab:export
pnpm texture-lab:sheet
pnpm texture-lab:batch
pnpm texture-lab:preview
```

Current landed commands:

```powershell
pnpm texture-lab:install
pnpm texture-lab:typecheck
pnpm texture-lab:export
pnpm texture-lab:sheet
```

## Source Contract

The first DSL should describe texture packs and block bundles, not raw Three.js
scenes.

Required concepts:

- pack metadata: name, author/license notes, default resolution
- pack-level tint roles with normal and alternate review colors
- palettes and named palette ramps
- indexed textures from ASCII rows
- generated tileable textures from deterministic helpers
- texture roles: `all`, `top`, `bottom`, `side`, `overlay`, `particle`
- texture source categories: `final-color` or `tintable`
- texture `tintRole` links for source art that receives a shared tint
- illustrator guidance for relationship graphs, palette discipline, tint roles,
  and agent briefs before drawing a block family
- preview-only texture metadata for checkerboard transparency, tiling mode, and
  disabling misleading cube/rotation panels
- block preview metadata: cube, cross-plane plant, log axis, cutout/alpha mode
- tint metadata for grass/foliage/water-style review
- variant metadata for preview: rotations, mirrors, authored variants, weights
- export metadata mapping generated textures to resource locations

Keep generated PNGs reproducible from source. Avoid source files that depend on
large binary image edits or unrecorded prompts.

## Preview Contract

The sheet renderer should produce enough evidence to judge a texture before it
enters the game:

- enlarged pixel view with optional grid
- 3x3 or 5x5 tile repeat
- seam/error panel for wrap discontinuities
- downsampled 16x16 view and mip strip
- isolated block in three-quarter view
- small terrain patch with repeated blocks
- rotation/mirror panel for variant candidates
- biome tint swatches where relevant
- optional local-only vanilla reference panel when gitignored reference assets
  exist

Screenshots and sheets must stay out of the repo and default to `/tmp`.

## Implementation Slices

### Slice 1 - Documentation And Scaffold

- [x] Add `tools/texture-lab/README.md` describing goals, source shape,
  preview expectations, legal policy, and non-goals.
- [x] Add `tools/texture-lab/package.json`, `tsconfig.json`, and minimal source
  layout.
- [x] Add root `pnpm texture-lab:*` wrappers.
- [x] Keep all generated outputs under `/tmp/mclone-texture-lab/` by default.

Validation:

```powershell
pnpm --dir tools/texture-lab install
pnpm texture-lab:typecheck
git diff --check
```

### Slice 2 - Minimal DSL And 2D Export

- [x] Define pack, palette, texture, and cube block-bundle DSL types.
- [x] Validate texture dimensions, palette symbols, resource names, alpha mode,
  and tileability flags.
- [x] Implement indexed ASCII texture rendering to PNG.
- [x] Add deterministic tileable speckle helpers for natural materials.
- [x] Export derived PNGs to `/tmp/mclone-texture-lab/pack/`.
- [x] Export Markdown and JSON metadata reports for texture roles, tint roles,
  preview modes, sheet paths, and block composition.
- [ ] Add `stone` example at 32x32. The first landed example is `dirt`.

Validation:

```powershell
pnpm texture-lab:typecheck
pnpm texture-lab:export
```

### Slice 3 - Review Sheet

- [ ] Build a browser preview backed by the DSL output.
- [x] Add 1x/enlarged pixel panels through the initial CLI sheet.
- [x] Add 3x3 tile-repeat panels through the initial CLI sheet.
- [x] Add seam/error panels with left-right and top-bottom mismatch overlays.
- [x] Add 16x16 downsample and mip-strip panels through the initial CLI sheet.
- [x] Add deterministic rotation-grid panel through the CLI sheet.
- [ ] Add Playwright sheet capture under `/tmp/mclone-texture-lab/`.

Validation:

```powershell
pnpm texture-lab:sheet
```

Review expectation: inspect the generated sheet before accepting an example
texture.

### Slice 4 - Block And Terrain Preview

- [x] Add software isometric cube preview for full-cube textures with nearest
  sampling and face shading.
- [ ] Add optional Three.js/browser preview later if interaction or non-cube
  models need it.
- [x] Add small terrain-patch preview for repeated grass/dirt block tops.
- [x] Add per-face block bundle for `grass_block`.
- [x] Add tint swatches for grass previews.
- [x] Add transparent overlay preview for grass side textures.
- [x] Add preview-only tint/checker controls so neutral grass and transparent
  overlay source textures render meaningfully in individual sheets.
- [x] Keep texture sheets source-first: enlarged source panel stays raw and
  tintless while repeat/mip/rotation/block-style panels can use preview tint.
- [x] Add a grass-side context sheet showing dirt side base, raw transparent
  overlay, and final composed tinted side together.
- [x] Add an illustrator guide covering final-color vs tintable source art,
  grass tint relationships, palette discipline, and agent brief shape.
- [x] Move grass preview tinting out of individual textures and into pack-level
  tint roles plus texture-level `source` / `tintRole` metadata.
- [x] Keep alternate grass biome tint previews compact so source texture review
  remains primary.
- [ ] Add foliage/water tint swatches once those texture families exist.

Validation:

```powershell
pnpm texture-lab:sheet -- examples/grass-block/texture.ts
pnpm texture-lab:preview
```

### Slice 5 - Rotation And Variant Experiments

- [x] Add deterministic preview of vanilla-style model rotations.
- [ ] Add mirror preview where blocks use mirrored model variants.
- [ ] Add authored variant sets with weights.
- [ ] Render comparison panels for auto-rotation, authored variants, and no
  rotation.
- [ ] Use dirt, stone, grass top, and log/planks as the first decision matrix.
- [ ] Record guidance in the README for which block families tolerate
  rotation/mirroring at 32x32.

Acceptance: the lab can show whether 32x32 auto-rotation improves repetition or
creates visible artifacts before runtime support is attempted.

### Slice 6 - Pack Shape And Future Runtime Bridge

- [x] Export a local original resource-pack tree under
  `/tmp/mclone-texture-lab/runtime-pack`.
- [x] Record generated texture resource locations and source fingerprints in
  `/tmp/mclone-texture-lab/mclone-default-overlay.pbp.json`.
- [x] Decide checked-in original source location:
  `tools/texture-lab/packs/mclone-default/` is the accepted overlay source tree;
  `tools/texture-lab/examples/` remains for small wrapper entrypoints.
- [x] Add tactical 115 for shared runtime overlay-pack loading.

### Slice 7 - Authoring Vocabulary Quality Pass

The first dirt/grass textures prove the loop, but they read too much like
uniform speckle fields. Before drawing a large ground/stone batch, improve the
DSL so authored textures can carry broader shapes and more intentional human
structure.

- [x] Add tileable low-frequency value/perlin-style noise for broad clods,
  stone clouds, sand bands, and material patches.
- [x] Keep `macroNoise` procedural fields periodic by construction so full-XY
  tile textures wrap cleanly at left/right and top/bottom edges.
- [ ] Add explicit rotation-safety metadata for generators: avoid
  directional highlights for rotation-safe tiles, but allow directionality for
  orientation-aware faces such as logs and planks.
- [x] Add virtual-resolution ASCII masks: author `8x8`, `16x16`, or `32x32`
  masks and upscale them to texture size.
- [x] Support mask upscale modes: `nearest`, `linear`, `bicubic`, and `smooth`.
- [x] Let ASCII masks drive opacity and overlay color instead of only painting
  final pixels.
- [ ] Let ASCII masks drive palette-ramp selection and procedural placement.
- [ ] Add compositional texture definitions so ores and related blocks can
  reuse a base material plus an authored ore/mineral mask.
- [x] Add texture-class guidance for natural full tiles, layered blocks, ore
  overlays, directional blocks, cutout plants, and fluids/emissive materials.

Status 2026-06-30: `macroNoise(...)` and `mask(...)` landed in the DSL,
renderer, and metadata report. The dirt and grass-block starter textures now
use broad periodic noise, virtual-resolution masks, and lighter speckle detail.
The generated sheets show better macro structure, but the broad fields are
still visibly synthetic in repeat panels; future art passes should tune masks
and noise per material before producing a large block batch.

Acceptance: the next stone/ground textures show broad, intentional material
forms in sheets and terrain preview, not just high-frequency grain.

## Open Questions

- Whether 32x32 should remain the default for all natural blocks or only for
  blocks that still read clearly after downsample/mip review.
- Whether runtime should support authored texture variants independently of
  vanilla blockstate model variants.
- Whether original pack source should stay TypeScript-only or move to a data
  format once the helper vocabulary stabilizes.
- How much local vanilla side-by-side reference is useful without encouraging
  derivative art.

## Non-Goals

- No runtime texture-pack UI in this tactical.
- No native renderer dependency on Three.js.
- No committed Mojang assets.
- No AI image prompt transcript as the authoritative source.
- No broad block catalog in the first implementation slice; prove the loop on
  a few high-signal blocks first.
