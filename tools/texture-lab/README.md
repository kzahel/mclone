# Mclone Texture Lab

Disposable TypeScript lab for authoring an original Minecraft-style texture
pack. The first block preview is a deterministic software isometric renderer;
browser-based previews remain optional later tooling.

The goal is to make it cheap for an agent or human to generate, preview,
validate, and revise block textures without distributing Mojang-owned PNGs.
This lab is an authoring and review tool first. Runtime integration should come
later through shared asset contracts, not through direct dependency on preview
code.

## Intent

Mclone needs original block textures for any distributable build. The vanilla
1.17.1 assets remain useful as local, gitignored reference material for model
shape, tint behavior, atlas semantics, mipmaps, and renderer parity, but they
must not become source art for a public pack.

The texture lab should support a fast loop:

1. Author a small, reviewable source file for a block or block family.
2. Render 2D texture views, tiling views, and block/terrain previews.
3. Capture deterministic review sheets under `/tmp`.
4. Export derived PNG/resource-pack payloads only after the source passes
   visual review.

Current first loop:

```sh
pnpm texture-lab:install
pnpm texture-lab:typecheck
pnpm texture-lab:export
pnpm texture-lab:analyze
pnpm texture-lab:catalog
pnpm texture-lab:runtime-compat
pnpm texture-lab:pack-overlay
pnpm texture-lab:coverage
pnpm --dir tools/texture-lab web:test
```

`texture-lab:analyze` is the feedback step of the authoring loop: it reports
objective structural, color, and defect features for each candidate against its
local vanilla counterpart, and ends with a `biggest gaps vs vanilla` list to
drive the next revision. Beyond value/color statistics it measures palette
concentration (top-8 color share), structure vs speckle (color run length),
dominant grain direction and strength (catches diagonal streaks the row/column
banding test cannot see), internal motif repetition (stamped macro
noise/masks show as a high repetition peak), detail spread (clustering of local
contrast — bunched detail stamps when tiled), distribution shape (Earth Mover's
Distance between candidate and vanilla luminance/hue histograms), and
alpha/cutout discipline (semi-alpha share — vanilla cutout alpha is binary —
plus silhouette island structure and dark-fringe halo detection, all measured
at the tile's native resolution since downsampling manufactures semi-alpha
edges). The candidate is downsampled to the vanilla grid first, so a 32x32 tile
and a 16x16 vanilla texture are compared apples-to-apples. Hue rows report n/a
below the neutral-saturation floor (tint-driven grayscale like vanilla leaves
has no meaningful hue direction to chase). The numbers are directional
guidance, not a pass/fail gate. Scope it to one or a few textures with
`--texture <name>`, or add `--json` for tool consumption.

To choose between two candidate PNGs (for example two iterations or two
variants), use compare mode:

```sh
pnpm texture-lab:analyze --compare a.png b.png
```

It ranks both against the vanilla counterpart — inferred from the filename, or
set with `--reference-name <block>` or `--reference-png <path>` — and prints a
per-feature `closer` column plus a `verdict` for which one to keep iterating
from. This is the mechanical tiebreaker for the tournament step of the loop.

For authoring rules, palette discipline, tint roles, the iteration loop, and
AI-agent brief shape, see [`ILLUSTRATOR_GUIDE.md`](ILLUSTRATOR_GUIDE.md). For
local measurements of typical vanilla block texture palette sizes, see
[`VANILLA_COLOR_COUNTS.md`](VANILLA_COLOR_COUNTS.md).

Accepted pack source is committed under:

```text
tools/texture-lab/packs/mclone-default/texture.ts
tools/texture-lab/packs/mclone-default/block/dirt.ts
tools/texture-lab/packs/mclone-default/block/directional-cubes.ts
tools/texture-lab/packs/mclone-default/block/grass-block.ts
tools/texture-lab/packs/mclone-default/block/plants-and-flats.ts
tools/texture-lab/packs/mclone-default/block/stone.ts
```

These TypeScript files are the source of truth. They define palettes, tint
roles, seeded procedural layers, and ASCII masks. The generated PNGs and review
sheets are deterministic derived artifacts and stay out of git by default.

The default generated output root is gitignored:

```text
generated-assets/texture-lab/
```

Override it when needed:

```sh
MCLONE_TEXTURE_LAB_OUTPUT_ROOT=/some/path pnpm texture-lab:export
```

The export command writes the `mclone-default` overlay pack PNGs plus review
sheets to:

```text
generated-assets/texture-lab/pack/assets/mclone/textures/block/dirt.png
generated-assets/texture-lab/dirt-sheet.png
generated-assets/texture-lab/pack/assets/mclone/textures/block/grass_block_top.png
generated-assets/texture-lab/pack/assets/mclone/textures/block/grass_block_side.png
generated-assets/texture-lab/pack/assets/mclone/textures/block/grass_block_side_overlay.png
generated-assets/texture-lab/pack/assets/mclone/textures/block/grass_block_bottom.png
generated-assets/texture-lab/pack/assets/mclone/textures/block/stone.png
generated-assets/texture-lab/pack/assets/mclone/textures/block/coal_ore.png
generated-assets/texture-lab/pack/assets/mclone/textures/block/iron_ore.png
generated-assets/texture-lab/grass-block-sheet.png
generated-assets/texture-lab/grass-block-side-sheet.png
generated-assets/texture-lab/stone-sheet.png
generated-assets/texture-lab/stone-block-sheet.png
generated-assets/texture-lab/coal_ore-sheet.png
generated-assets/texture-lab/coal-ore-sheet.png
generated-assets/texture-lab/iron_ore-sheet.png
generated-assets/texture-lab/iron-ore-sheet.png
generated-assets/texture-lab/mclone-default-metadata.md
generated-assets/texture-lab/mclone-default-metadata.json
generated-assets/texture-lab/mclone-default-texture-catalog.md
generated-assets/texture-lab/mclone-default-texture-catalog.json
```

When a block name collides with a texture name, such as `stone`, the texture
sheet keeps `stone-sheet.png` and the block sheet uses `stone-block-sheet.png`.
If `reference/minecraft-1.17.1/extracted/` or `reference/minecraft-1.17.1/src/`
exists, individual texture sheets also draw the matching local vanilla texture
in a bordered lower-right reference panel. That panel is for private review
only and is never written into exported pack PNGs or overlay packs.

Reference panels can be disabled or pointed at another local extraction:

```sh
pnpm --dir tools/texture-lab export -- --no-reference
pnpm --dir tools/texture-lab export -- --reference-root /path/to/extracted
```

The runtime-compatible export also writes vanilla-path texture overrides:

```text
generated-assets/texture-lab/runtime-pack/assets/minecraft/textures/block/dirt.png
generated-assets/texture-lab/runtime-pack/assets/minecraft/textures/block/grass_block_top.png
generated-assets/texture-lab/runtime-pack/assets/minecraft/textures/block/grass_block_side.png
generated-assets/texture-lab/runtime-pack/assets/minecraft/textures/block/grass_block_side_overlay.png
generated-assets/texture-lab/runtime-pack/assets/minecraft/textures/block/grass_block_bottom.png
generated-assets/texture-lab/runtime-pack/assets/minecraft/textures/block/stone.png
generated-assets/texture-lab/runtime-pack/assets/minecraft/textures/block/coal_ore.png
generated-assets/texture-lab/runtime-pack/assets/minecraft/textures/block/iron_ore.png
```

This is a development bridge, not a standalone distributable pack. It overrides
PNG textures while the engine still reads local vanilla blockstate/model
JSON from `reference/minecraft-1.17.1/extracted/` or `extracted.zip`. To try it
with the native runtime:

```sh
MCLONE_FIRST_PARTY_ASSET_ROOT=generated-assets/texture-lab/runtime-pack pnpm native:timedemo:smoke
```

The browser UI uses one source-controlled lifecycle:

1. **Candidate** is an authoring recipe or generated image and is not used by
   the runtime.
2. **Provisional** is validated first-party art allowed in the default visual
   profile.
3. **Curated** is manually accepted first-party art that shadows provisional.

The main page shows Candidate, Provisional, Curated, and the real local
Minecraft Reference together. Minecraft Reference is read-only and local-only;
it is never a promotion source.

`Use as Provisional` and `Accept as Curated` write the pack-local lifecycle
manifest and promoted PNG:

```text
tools/texture-lab/packs/mclone-default/lifecycle.v1.json
tools/texture-lab/packs/mclone-default/provisional/**/*.png
tools/texture-lab/packs/mclone-default/curation.v1.json
tools/texture-lab/packs/mclone-default/frozen/**/*.png
```

Every promoted entry records an explicit canonical engine material id. Thus
`grass_block_top` deliberately binds to `mclone:block/grass_block` instead of
relying on its filename. Exact inventory matches are offered as visible
suggestions; ambiguous authoring textures remain unpromotable until a binding
is chosen. Tintable promotions use the same `sourceNeutrality` policy as
normal exports.

`curation.v1.json` remains the detailed provenance record for frozen curated
PNGs. The older selection/apply/freeze-request APIs and CLI commands remain
available for provenance and compatibility work, but they are no longer the
primary browser workflow. The matching low-level promotion command is:

```sh
pnpm texture-lab:promote-frozen -- --texture grass_block_top
```

The packed overlay command builds the same overrides into a first-party-only
`.pbp` file without reading the local Mojang extraction:

```text
generated-assets/texture-lab/mclone-default-overlay.pbp
generated-assets/texture-lab/mclone-default-overlay.pbp.json
```

To try the packed overlay:

```sh
MCLONE_ASSET_OVERLAY_PACK=generated-assets/texture-lab/mclone-default-overlay.pbp pnpm native:timedemo:smoke
```

The coverage command compares that overlay pack against the texture materials
the native terrain atlas currently requests:

```text
generated-assets/texture-lab/mclone-default-overlay-coverage.md
```

The browser UI integration tests are Playwright-backed:

```sh
pnpm --dir tools/texture-lab web:test
```

The test harness builds a deterministic fixture under the gitignored
`generated-assets/texture-lab-playwright/` root, starts the local texture-lab
server, verifies authored texture indexing, lifecycle metadata, committed
frozen overlay metadata, generated candidate discovery, allowlisted image
serving, Candidate/Provisional/Curated promotion, the read-only Minecraft
comparison, lifecycle filtering, reindex preservation, empty-candidate
behavior, system-default dark mode, and manual light/dark toggling. Validation
screenshots are written to
`/tmp/mclone-texture-lab-playwright-a2.png`,
`/tmp/mclone-texture-lab-playwright-lifecycle.png`, and
`/tmp/mclone-texture-lab-playwright-dark-mode.png`.

## Source Format

Use a constrained TypeScript DSL, following the same broad pattern as
`tools/asset-lab`:

- TypeScript gives editor completion and type checking.
- The DSL gives agents a limited vocabulary instead of free-form renderer code.
- Preview/render code is only tooling implementation.
- Exported files are derived artifacts; the editable source remains compact and
  reviewable.
- Accepted generated textures can be committed as small frozen PNGs referenced
  by `curation.v1.json`; temporary exports and diffusion work products remain
  under gitignored `generated-assets/`.

## Reference Counterparts

Most authored textures compare against one vanilla block texture with the same
name. Some vanilla blocks are model-part sets instead of one PNG. The reference
layer encodes those as composite counterparts so the UI and sheets show the real
vanilla texture set rather than a false `Missing` state.

Current special mappings:

- `pointed_dripstone`: a 2x5 composite, with `down` and `up` columns and
  `base`, `frustum`, `middle`, `tip`, and `tip_merge` rows, matching the
  `vertical_direction` and `thickness` blockstate/model fanout in Minecraft
  1.17.1.

The first source format is hybrid:

- Indexed ASCII textures for small pixel-art masks, icons, overlays, and
  simple 16x16 or 32x32 surfaces.
- Palette ramps for related colors such as grass greens, dirt browns, stone
  grays, sand yellows, and bark rings.
- Small procedural texture helpers for natural materials: wraparound noise,
  speckles, pebbles, scratches, cracks, roots, moss, and edge blending.
- Pack-level tint roles, such as `grass`, with normal and alternate review
  colors plus a `sourceNeutrality` policy for tintable raw source art.
- Texture-level source categories: `final-color` or `tintable`.
- Texture-level `tintRole` links for neutral source art that needs a shared
  tint relationship. Tintable textures must use a tint role with an explicit
  source-neutrality threshold.
- Explicit masks and overlays for blocks with multiple layers, such as grass
  side base plus tinted overlay.
- Cube block bundles for reviewing a full block from named texture roles.
- Directional cube block bundles with distinct `north`, `east`, `south`, and
  `west` faces. Their block review sheets show multiple isometric yaw previews
  plus a flat face panel for `top`, `bottom`, and all four lateral faces.
- Cross-plant and flat ground-sprite block bundles for cutout geometry such as
  grass, ferns, flowers, redstone dust, and similar non-cube block models.
  Their review sheets show the cutout texture, model-shaped preview, and small
  patch context instead of a misleading cube.
- Preview-only metadata for showing alpha on a checkerboard, controlling tiling
  mode, or disabling cube/rotation panels where they are misleading.

Tintable textures are source-first in review sheets. The enlarged pixel panel
shows the raw, tintless source texture; repeat, mip, rotation, and block-context
panels use the preview tint when one is defined. Grass side textures should be
judged in block context because the final side is dirt base plus a tinted
transparent overlay, not either source texture alone.

Tintable raw source is validated before export. `renderAllTextures(pack)` checks
each rendered `source: "tintable"` texture against its tint role's
`sourceNeutrality` policy using alpha-weighted saturation. A colored raw grass
top or transparent grass overlay fails export instead of becoming an active
double-tinted texture.

Each export also writes a metadata report next to the sheets. The Markdown
report is for quick human review; the JSON report is for future agent/tool
checks. Both list tint roles, texture source categories, preview tiling modes,
seam diagnostic modes, sheet paths, and block face composition.

`texture-lab:catalog` writes a wider generated matrix for planning the full
Minecraft block texture atlas. It reads the local vanilla 1.17.1 extraction,
blockstate JSON, block model JSON, PNG alpha/animation metadata, and the Java
`ItemBlockRenderTypes` render-layer map when the decompiled source is present.
It then joins those facts to the current mclone pack by runtime-compatible
texture path. The editable source remains the TypeScript pack plus optional
per-texture `catalog` metadata; the large Markdown/JSON matrix is derived and
stays under `/tmp`.

Catalog fields are intentionally split by ownership:

- vanilla-derived facts: texture path, size, alpha mode, animation, model
  families, alpha visual bounds, block/model/face uses, tint indexes,
  blockstate rotations, face UV rotations, and render layers
- inferred authoring constraints: tiling, rotation safety, alpha/render-layer
  class, and tint role hints
- mclone status: authored replacement path, source category, tint role,
  optional status, tags, and notes

This is the place to answer questions such as "which cutout decorations are
still missing?", "which textures must survive 90-degree blockstate rotation?",
and "which tint-index textures need grass or foliage review?"

The browser curation index also carries a compact version of that usage
semantics data per authored texture. It resolves the vanilla counterpart's
blockstate/model usage, model families, normalized geometry kinds such as
`cross-sprite`, `crop-cross`, `flat-ground`, `cube`, `pane`, `rail`, `door`,
and `torch`, render layers, texture slots, tint indexes/roles, preview hints,
and concrete vanilla block examples. This metadata is the preferred source for
choosing review previews and authoring constraints. The browser UI defaults to
an `Auto` preview mode that uses the vanilla preview hint to show focused
rendered uses for cube, cross-sprite, flat-ground, and partial-model textures
when authored block usage is available; alpha-shape diagnostics are secondary
validation for the model families where centering or cutout coverage matters.
The block review-sheet renderer has explicit preview kinds for `pane`, `rail`,
`torch`, `door`, and `trapdoor` families, so partial and thin vanilla model
families can be judged in shape context instead of as cube or raw-texture-only
art. When a texture has vanilla usage metadata but no authored pack block, the
index builder derives a review block from the sampled vanilla block/model usage
and writes a generated block sheet into `generated-assets`. These derived review
blocks are labeled separately from authored pack blocks in the browser Blocks
and Auto previews, so generated vanilla-context cards do not read as source
definitions. Pane/door/rail/torch review no longer needs hand-authored
`block(...)` fixture entries.

The browser UI also has a separate **Minecraft Reference** atlas view for full
vanilla block-texture
coverage. The normal `Atlas` tab shows only textures authored by this pack; the
reference tab enumerates every local vanilla `assets/minecraft/textures/block/*.png`
reference and marks whether mclone has no matching texture, only placeholder
coverage, candidate coverage, or frozen/accepted coverage. This is the view for
finding zero-coverage vanilla textures before deciding what to generate next.

The `mclone-default` grass block module encodes that relationship directly:

```ts
tint("grass", {
  normal: "#79b34e",
  alternates: ["#5fa343", "#98b85e", "#6fa35b"],
  sourceNeutrality: {
    maxMeanSaturation: 0.005,
    maxPixelSaturation: 0.01,
  },
});

texture("grass_block_top", {
  source: "tintable",
  tintRole: "grass",
  // ...
});
```

Pure ASCII is useful, but it should not be the only tool. A 32x32 natural block
is 1024 cells per face; asking an agent to hand-paint every cell is slow and
usually less coherent than combining a small palette, a few masks, and seeded
tileable noise.

The starter DSL now includes broader authored structure beyond speckles:

- `macroNoise(...)` for tileable low-frequency noise: clods, stone clouds,
  bands, and patches
- virtual-resolution ASCII masks authored at 8x8, 16x16, or 32x32
- `mask(...)` upscale modes: nearest, linear, bicubic, and smooth
- masks that drive opacity and overlay color; palette-ramp and procedural
  placement drivers are future extensions
- compositional textures, such as base stone plus ore/mineral masks

The web index classifies the current art source for each texture. Textures that
are only seeded macro-noise/speckle coverage are labeled `noise placeholder`
in the sidebar, detail view, atlas cards, inspector, and search. They are
useful for coverage and layout review, but should not be mistaken for
curated/frozen texture art.
The sidebar Queue filter turns those labels into replacement queues: `Noise
placeholders`, `Authored structure`, `Frozen assets`, `Has candidates`, and
`Needs candidates`. `Needs candidates` means the texture is not frozen and has
no local generated candidates linked yet.

All procedural helpers that target fully tiled textures should be periodic by
construction. Rotation-safe materials should avoid one-way lighting or streaks;
orientation-aware materials should declare that directionality explicitly.
The first dirt/grass/stone pass proves the mechanism but is not final art;
generated repeat panels still need human review for visible macro-pattern
stamping.

## Resolution

Default original source resolution should start at 32x32 per vanilla block
face. Vanilla 1.17.1 block textures such as dirt, stone, and grass are commonly
16x16, so every 32x32 texture must also be reviewed at 16x16 and at generated
mip levels. A texture that looks good only at 32x32 is not good enough for the
game view.

For the open design problem of preserving a successful 16x16 material read
while adding real native 32x32 detail, see
[`UPSCALING_PROBLEM_STATEMENT.md`](UPSCALING_PROBLEM_STATEMENT.md). The
active experiment plan for it — diffusion-proposed detail projected back into
deterministic, palette-quantized mask source — is
[`DIFFUSION_UPSCALE_PLAN.md`](DIFFUSION_UPSCALE_PLAN.md).

The lab should make resolution explicit per texture:

```ts
texture("dirt", {
  size: 32,
  palette: "dirt",
  layers: [
    macroNoise({
      seed: "dirt-broad-clods",
      frequency: 4,
      colors: ["shadow", "base", "warm"],
      opacity: 0.3,
    }),
    mask({
      pixels: ["..c.....", ".....w..", "...s....", "........"],
      colors: { c: "cool", w: "warm", s: "shadow" },
      upscale: "smooth",
      opacity: 0.25,
    }),
    speckles({ density: 0.16, colors: ["dark", "light"] }),
  ],
});
```

## Block Bundles

Author per block or small block family, not as one huge texture file.

Useful first bundles:

- `dirt`: single tile, rotation/mirror preview.
- `grass_block`: top, side, bottom, tinted side overlay, biome tint previews.
- `stone`: tile plus mirrored/rotated variants.
- `coal_ore` / `iron_ore`: base stone material plus authored ore masks.
- `oak_log`: side, top rings, axis rotation previews.
- `sand`: subtle tile, mip/noise review.
- `leaves`: alpha/cutout preview and foliage tint.

Texture families should be treated differently:

- natural full tiles: dirt, stone, gravel, and sand need wrap-safe macro and
  micro structure
- layered blocks: grass, podzol, mycelium, and snow sides need face
  relationships plus overlays
- ore blocks: base stone/deepslate material plus authored ore masks
- directional blocks: logs and planks can be directional, but must declare face
  and axis expectations
- cutout plants: grass, ferns, and flowers should be mask-heavy
- flat ground sprites: redstone dust, rails, pressure-plate-like overlays, and
  other mostly-horizontal cutouts need ground-plane previews
- fluids/emissive textures: water, lava, and glow lichen need tint/animation or
  light-related rules later

A grass block bundle should model the real rendering problem:

- bottom: dirt-like tile
- top: biome-tinted grass surface
- side: dirt/grass transition
- overlay: biome-tinted transparent grass fringe
- snowy variant placeholder later

## Preview Modes

The review sheet should include:

- 1x and enlarged pixel view with optional grid.
- 3x3 or 5x5 seamless tile view.
- seam view that highlights left/right and top/bottom discontinuities.
- 16x16 downsample and mip chain preview.
- isolated software-rendered cube in three-quarter view.
- model-specific block previews for cross plants and flat ground sprites where
  cube previews would hide the actual silhouette problem.
- small terrain patch preview, because repetition problems often appear only
  across many blocks.
- rotation and mirror preview for blocks that rely on deterministic model
  variants.
- optional biome tint swatches for grass, foliage, and water-tinted textures.
- local-only vanilla reference panel when the gitignored reference assets
  exist; never export or commit that panel as pack source.

Review sheets and texture-lab generated outputs should be written under
`generated-assets/texture-lab/` by default. Ad hoc validation screenshots that
are not part of the lab output can still go under `/tmp`.

Biome tint previews should stay secondary. The grass block sheet uses one
primary tint for texture review and keeps alternate tint swatches compact so
they do not distract from source texture quality.

The individual texture sheet seam panel shows the texture repeated across the
wrap boundary. Red/orange marks left-right edge mismatch; blue marks top-bottom
edge mismatch. Textures marked with horizontal-only tiling show only the
left-right diagnostic.

## Variants And Rotation

Vanilla commonly hides repetition by deterministic model rotation or mirroring,
not by shipping many texture tiles for every block. That works well at 16x16
because the art is coarse and rotational artifacts are less obvious. At 32x32,
auto-rotation can reveal directional details, broken shadows, or inconsistent
material grain.

The lab should test both:

- auto-rotation/mirroring of a single texture
- multiple authored variants selected with stable per-block-position randomness

Do not assume one approach wins globally. Dirt may work with rotation only.
Stone may need mirror-safe texture construction. Grass top may need a few
variants. Logs and planks may need orientation-aware authoring instead of
rotation.

If variants are exported for runtime use, they should use stable block-position
selection so the world does not shimmer frame to frame and so multiplayer
clients agree on visible results.

## Export Targets

Initial exports should be derived and disposable:

```text
generated-assets/texture-lab/pack/
generated-assets/texture-lab/runtime-pack/
generated-assets/texture-lab/mclone-default-overlay.pbp
generated-assets/texture-lab/mclone-default-overlay-coverage.md
generated-assets/texture-lab/sheets/
```

The checked-in original pack source tree lives under
`tools/texture-lab/packs/mclone-default/`. Generated PNG export still goes to
`generated-assets/texture-lab/` by default, which is ignored by git. Do not
commit generated PNGs, review sheets, overlay packs, or local candidate
archives.

Runtime adoption should go through shared owners such as `mclone-assets`,
`mclone-render-session`, and `mclone-render`. The desktop app should only gain
platform glue if needed for choosing or staging a pack.

## Legal Policy

- Do not commit Mojang texture PNGs.
- Do not trace, recolor, or mechanically transform Mojang textures into source
  art.
- Local vanilla assets may be used for private visual reference and renderer
  parity checks only.
- Checked-in source art must be original to mclone or explicitly licensed for
  redistribution.

## Non-Goals

- No runtime integration in the first slice.
- No full resource-pack manager.
- No arbitrary image-generation prompt log as the source of truth.
- No Photoshop-style binary source files as the primary editable format.
- No dependency from native runtime crates on Three.js or texture-lab preview
  code.
