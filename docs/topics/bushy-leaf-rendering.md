# Bushy Leaf Rendering

Topic: `bushy-leaf-rendering`

Status: research complete; implementation not started.

## Scope

This topic records how Motschen's Better Leaves produces its bushy canopy
silhouette, what changed in its performance-focused 9.x line, and how a
similar optional presentation feature could fit mclone. It also coordinates
the feature with [`lush-grass-rendering.md`](lush-grass-rendering.md),
[`graphics-video-settings.md`](graphics-video-settings.md), and the active
asset-pack/provenance contract.

This is not a vanilla Minecraft 1.17.1 parity requirement. Bushy leaves would
be a client-side mclone presentation option: authoritative blocks, collision,
lighting opacity, decay, persistence, protocol state, and world generation
would remain unchanged.

No runtime or asset implementation is present. Future work should begin with a
bounded tactical and fresh performance evidence.

## Bottom Line

Mclone does **not** need hand-painted bushy textures before it can implement
this effect.

There are three viable asset approaches:

1. Reuse the active leaf texture on several extra cutout cards. This is closest
   to Better Leaves 8.1 and needs no new texture, but uses much more geometry
   for a convincing rounded silhouette.
2. Derive one larger bushy sprite from each active leaf texture during asset
   preparation. This is closest to the performance-focused 9.x technique and
   is the recommended direction. It needs an original mclone mask/generator,
   but not bespoke art for every leaf species.
3. Author dedicated bushy sprites by hand. This may eventually give the best
   art direction, but it is an optional polish path rather than an engine
   prerequisite.

Mclone Original still needs authored source leaf textures before public-release
visual acceptance: its current oak, birch, spruce, dark-oak, acacia, and jungle
leaf materials come from the generated fallback unless another selected pack
supplies them. That distribution requirement is separate from the rendering
technique. A derived sprite must inherit the provenance of its source texture;
deriving Minecraft-reference pixels does not make them first-party content.

## Sources And Pinned Evidence

Primary sources inspected on 2026-07-24:

- [CurseForge project page](https://www.curseforge.com/minecraft/texture-packs/motschens-better-leaves)
  describes the visible effect as additional layers around leaf blocks,
  distinguishes dynamic-texture-compatible 8.1 from precompiled 9.x, and
  recommends Cull Leaves for more FPS.
- [Better Leaves 9.5 file page](https://www.curseforge.com/minecraft/texture-packs/motschens-better-leaves/files/7556678)
  is the current release, uploaded 2026-01-31 for Minecraft 1.20 and later.
- [Better Leaves 8.1 file page](https://www.curseforge.com/minecraft/texture-packs/motschens-better-leaves/files/5732199)
  is the final pre-9.x resource-pack-compatible shape inspected here.
- [Better Leaves 6.0 file page](https://www.curseforge.com/minecraft/texture-packs/motschens-better-leaves/files/3441981)
  is the release explicitly covering Minecraft 1.17.1.
- [TeamMidnightDust/BetterLeavesLite](https://github.com/TeamMidnightDust/BetterLeavesLite)
  contains the 9.x generator, masks, base models, compatibility data, and
  generated output. Research pinned commit
  [`19fd18512b414a7d4c37ea9320af7b10aac817ff`](https://github.com/TeamMidnightDust/BetterLeavesLite/commit/19fd18512b414a7d4c37ea9320af7b10aac817ff).

The CurseForge project and source repository declare the MIT license. Mclone
could reuse covered material with the required notice, but the recommended
product direction is an original mask, geometry layout, and generator that fit
mclone's asset pipeline and art direction. Do not copy the inspected Minecraft
leaf pixels into tracked or distributable mclone content.

Temporary downloaded artifact hashes:

```text
5480640cb2f7674da4f2897ebc48b29a436706614160f4d4d8ad0ead5150fea6  Better-Leaves-9.5.zip
d0d5ab55ef5458fb0eccefbb79e7d0526d6188cb924cbb232385e506738880ea  Better-Leaves-8.1-1.20+.zip
391150cf84707f8f5800945693441c9c4d8040a7f2f9d3fffed10dc167a73de7  Better-Leaves-6.0-1.13+.zip
```

These artifacts were inspected under `/tmp`; they were not added to the
repository.

## How Better Leaves Works

Better Leaves is a static resource-pack model effect. It does not add blocks,
run a simulation, require a server component, or animate the canopy by itself.
Shader packs may add their own movement, but wind is not required for the
bushy shape.

### Performance-focused 9.x path

The 9.5 generator does the following for each square leaf texture:

1. creates a transparent image at twice the source width and height;
2. pastes the source in a centered 3x3 repetition, with the outer copies
   naturally cropped by the 2x canvas;
3. selects a deterministic alpha mask from the source filename;
4. applies that mask to make an irregular rounded cluster; and
5. writes the derived texture over the leaf material used by the model.

A normal 16x16 leaf therefore becomes a 32x32 derived sprite. The middle
16x16 region still looks like the original block face. The surrounding pixels
provide a larger irregular cluster for geometry that protrudes beyond the
block.

Each in-world 9.5 model contains:

- one ordinary full cube whose UVs select the middle half of the derived
  sprite; and
- two intersecting, double-sided cutout planes extending roughly half a block
  beyond the cube horizontally and about three-eighths of a block vertically.

There are four plane-layout models and four Y rotations of each, for sixteen
blockstate choices. Minecraft's stable position-seeded model selection keeps
nearby leaf blocks from repeating one identical cross.

The common model has three JSON elements and ten quads before neighbor
culling: six cube faces plus four outer-card faces. The outer planes reuse the
same atlas sprite as the cube; there is no second bushy-leaf material lookup.

Animated/non-square textures cannot use this preprocessing rule. The generator
detects them and falls back to the legacy geometry.

### Texture-compatible 8.1 and 1.17.1-era path

Better Leaves 8.1 ships no Minecraft leaf PNGs. Its leaf models refer to the
ordinary active leaf material, so a different resource pack can replace the
texture without recompiling Better Leaves.

The rounded shape is instead assembled from many small, double-sided cutout
planes that sample pieces of the ordinary leaf texture. The inspected base
models have seventeen elements and thirty-eight quads: the six-face cube plus
thirty-two outer-card faces. Eight layouts and four Y rotations produce
thirty-two choices in 8.1. The 1.17.1-compatible 6.0 artifact has the same
seventeen-element/thirty-eight-quad model shape, with eight listed model
choices and no bundled Minecraft leaf textures.

The project description calls this texture adjustment “dynamic.” It means the
model follows whichever leaf texture is active through ordinary resource-pack
composition; it is not per-frame procedural texture generation.

### Why 9.x is faster

The derived rounded sprite moves silhouette complexity from geometry into an
alpha mask:

| Shape | Elements | Quads before culling | Added quads over a cube |
|---|---:|---:|---:|
| ordinary leaf cube | 1 | 6 | 0 |
| Better Leaves 9.5 | 3 | 10 | 4 |
| Better Leaves 8.1 / 6.0 | 17 | 38 | 32 |

Relative to 8.1, 9.5 removes 28 of 38 total model quads and 28 of 32 added
outer quads. The tradeoff is texture preparation and a 2x-by-2x derived sprite,
which has four times the source texel count.

The author also avoids a separate outer-leaf atlas sprite. That can reduce
material/sprite bookkeeping, but it does not make the feature free. Atlas
packing, mip gutters, transparent-fragment coverage, and leaf density matter
more than the compressed PNG size. The resulting atlas dimensions must be
measured because one larger sprite can push a power-of-two atlas over a size
step.

## Expected Mclone Performance Shape

Mclone's current leaf blocks are cutout full cubes. Their Java-shaped render
facts deliberately report `occludes == false`, so leaf neighbors do not hide
one another's cube faces in
[`mclone-mesh/src/builder.rs`](../../native/crates/mclone-mesh/src/builder.rs).
The current performance baseline already includes that interior face pressure.

Adding a 9.x-like pair of double-sided planes to the existing baked terrain
mesh would add four quads per admitted leaf. For an exposed leaf retaining all
six cube faces, leaf-local geometry rises from six to ten quads, about 67%.
With the current 40-byte textured vertex and 32-bit indices, four independently
baked quads are approximately 736 raw vertex/index bytes per leaf before
allocator and GPU-buffer capacity overhead.

That is not a prediction of a 67% frame-time regression. Leaves are only part
of a scene, the cards remain in the existing cutout phase, and no additional
draw is required if they are appended to the section mesh. Conversely, the
outer cards cover a much larger screen area than their quad count suggests and
contain transparent texels, so fragment discard, overdraw, mip shimmer, and
stereo pixel work may dominate in dense forests.

Useful mclone-specific conservation:

- do not emit outer cards for a leaf completely enclosed by leaf neighbors;
- consider a stricter, measured canopy-surface admission rule only if the
  fully-enclosed rule is insufficient;
- leave Far LOD unchanged rather than preserving tiny bush cards at distances
  where they cannot contribute useful silhouette;
- retain a true Blocky mode that emits no extra vertices or indices; and
- measure desktop, Steam Deck, browser WebGPU, Android, and physical Quest
  independently before choosing defaults.

The inspected pack recommends Cull Leaves and includes options that force leaf
culling and hide inner leaves. Mclone should not import that mod's policy
implicitly or change vanilla-target base-leaf semantics as a side effect.
Skipping only optional bush cards for provably enclosed leaves is a narrower
optimization.

## Existing Mclone Fit And Gaps

Useful foundations:

- [`mclone-mesh/src/tint.rs`](../../native/crates/mclone-mesh/src/tint.rs)
  already supplies Java-shaped foliage tint, including biome blend plus fixed
  birch and spruce colors.
- [`mclone-mesh/src/catalog.rs`](../../native/crates/mclone-mesh/src/catalog.rs)
  identifies current leaf families, render layers, sprites, and face geometry.
- [`mclone-assets/src/atlas.rs`](../../native/crates/mclone-assets/src/atlas.rs)
  accepts variable-size sprites and reserves mip-safe gutters, so 32x32
  derivatives are representable.
- The ordinary section cutout path already renders through mono, per-eye, and
  full-frame multiview terrain pipelines.
- Asset epochs and transactional replacement already bind one selected source
  chain, prepared atlas/catalog, compiler, and resident scene lifetime.

Important gaps mean the Better Leaves ZIP is not a drop-in mclone feature:

- the Minecraft JSON adapter currently keeps only the first weighted blockstate
  variant instead of selecting a stable position-dependent variant;
- the model parser supports blockstate rotations but not per-element model
  rotations, which the outer Better Leaves planes use;
- the first-party visual catalog classifies leaves as ordinary solid cubes and
  has no optional leaf-detail contract; and
- a graphics-quality change that alters compiled leaf geometry needs a shared
  background remesh/commit path rather than an app-local toggle.

The feature should therefore be expressed as native shared mesh/asset policy,
not by copying the external resource pack into one client target.

## Recommended Mclone Direction

### Asset preparation

During shared asset preparation, derive one bushy sprite for each resolved leaf
material from:

- the active source texture;
- an original mclone-owned alpha-mask family;
- a stable generator/mask version; and
- the source asset fingerprint and provenance.

Cache keys should include all four facts. The derivative must be prepared and
committed in the same asset epoch as the source pack. Switching between
Mclone Original, Vanilla Reference, Hybrid Authoring, or another future pack
must never leave derivatives from the retired source in the active atlas.

For a first visual proof, applying the ordinary leaf sprite directly to two
large cards is acceptable. It will show whether the silhouette and render path
are valuable before mask art is finalized. It should not be mistaken for the
production-quality/performance result.

### Shared mesh compilation

The simplest first production shape is:

1. identify supported leaf states in `mclone-mesh`;
2. use the section halo to reject fully enclosed leaves;
3. choose an original outer-card layout and rotation from a stable hash of
   world position and block/material identity;
4. reuse the leaf's existing foliage tint and packed light;
5. append four outer cutout quads to the ordinary section cutout mesh; and
6. keep collision, block bounds, selection outline, occlusion, and lighting
   facts on the original full leaf block.

This path adds no new draw, shader, per-frame allocation, or platform-specific
renderer. It automatically follows the existing terrain mono/stereo/multiview
contract. The section culling bounds must account for the maximum protrusion,
especially for cards emitted by blocks on a section edge.

If measurement shows baked-card memory, remesh latency, or distance control is
unacceptable, the next option is a compact section-keyed
`BushyLeafInstance` artifact and shared instanced template. That would make a
distance cutoff cheaper and reduce bytes per leaf, but it adds a new resident
resource and draw path. Do not pay that complexity before the baked path is
measured.

### Animation

Static bushy leaves are sufficient for the intended first effect. A later
world-space wind field may be shared conceptually with lush grass and ordinary
plant sway, but the features must not share geometry caches, eligibility, or
quality controls. Stereo views must observe identical world-space deformation;
never billboard or deform a card independently per eye.

## Settings Coordination With Lush Grass

Bushy leaves and lush grass should be independently selectable because their
cost and platform fit differ substantially:

- **Leaf Detail:** `Blocky` / `Bushy`
- **Grass Detail:** `Off` / `Sparse` / `Lush` / `Ultra`

Do not require expensive grass to obtain bushy leaves, and do not disable a
measured-cheap leaf silhouette merely because a platform defaults grass Off.
A future overall Graphics Quality preset may project both settings after the
individual runtime effects and preference fields exist:

| Overall preset | Leaf Detail | Grass Detail |
|---|---|---|
| Low | Blocky | Off |
| Medium | Bushy if measured safe | Sparse |
| High | Bushy | Lush |
| Ultra | Bushy | Ultra |

This table is a suggested projection, not a current product default. Manual
edits should eventually make an overall preset report `Custom`, consistent
with [`graphics-video-settings.md`](graphics-video-settings.md).

Leaf Detail belongs in the planned machine-local
`ClientGraphicsPreferences`, not world persistence or asset-pack selection.
Platform profiles may supply different defaults while preserving an explicit
player choice. A conservative initial policy is Blocky on Quest/mobile and no
non-Blocky default anywhere until A/B evidence exists. Desktop, desktop XR,
Steam Deck, browser, and mobile defaults should then be decided from their own
measurements.

Changing Leaf Detail may require derived-asset preparation and section
recompilation. Apply it transactionally in the background: keep the old
drawable scene until the new compiler/atlas/sections are ready, commit at a
shared frame boundary, persist only the accepted result, and retain the old
setting/resources on failure.

## Suggested Implementation Sequence

### Slice 0: visual and accounting proof

- Create an original two-card layout and temporary derived-mask experiment
  outside tracked runtime behavior.
- Capture close, middle, and canopy-interior images using oak, birch, spruce,
  dark oak, acacia, and jungle leaves.
- Establish counters for total leaves, surface-admitted leaves, rejected
  enclosed leaves, added quads/bytes, cutout draws, atlas dimensions, and
  preparation/compile/upload timings.

### Slice 1: shared static geometry

- Add Blocky/Bushy compilation policy to the shared owner.
- Emit deterministic surface-only cards in the existing cutout mesh.
- Preserve a zero-extra-geometry Blocky path.
- Validate section-edge culling, lighting, tint, asset replacement, mono,
  per-eye, and full-frame multiview at the first drawable milestone.

### Slice 2: derived assets and setting

- Generate and provenance-track bushy sprites for the active asset epoch.
- Add the shared Leaf Detail effect and eventual graphics-preference field.
- Make live apply transactional and prove Blocky/Bushy/Blocky restoration in
  one session without changing world state.

### Slice 3: measured optimization only if needed

- Compare broader surface admission, baked geometry, compact instancing, and a
  near-distance cutoff using actual bottleneck evidence.
- Consider shared vegetation wind only after the static effect and lush-grass
  base paths are independently accepted.

## Validation Expectations

An eventual implementation needs:

- deterministic world-position layout selection;
- eligibility tests across air, leaves, logs, solids, section boundaries, and
  fully enclosed canopy cells;
- exact Off/Blocky conservation and zero added-geometry counters;
- foliage tint checks for ordinary, birch, spruce, swamp, and biome-boundary
  cases;
- asset-epoch tests proving derivatives follow selected source and provenance;
- atlas dimension/mip inspection and close/far shimmer review;
- section compile, upload, resident-byte, cutout GPU, overdraw, and movement
  frame A/B evidence;
- rendered mono, stereo, and full-frame multiview captures with expanded
  section bounds; and
- physical Quest evidence before any Bushy mobile/XR default.

Screenshots and temporary artifacts belong under `/tmp`, following the
repository rendered-output guardrail.

## Open Decisions

- The original mclone mask family and whether all leaf species share one shape.
- Exact card count, dimensions, angles, and deterministic variation.
- Whether source-pack preparation or a dedicated derived-material overlay is
  the cleanest asset-epoch boundary.
- Whether fully enclosed rejection is enough for dense-canopy overdraw.
- Whether the first measured implementation should bake quads or begin with
  compact instances.
- The final player-facing name: `Leaf Detail`, `Leaf Shape`, or another clear
  Blocky/Bushy label.
- Platform defaults after representative A/B captures and frame probes.
