# Bushy Leaf Rendering

Topic: `bushy-leaf-rendering`

Status: implemented 2026-07-24; `Blocky` remains the default while physical
Quest acceptance and broader hardware profiling remain open.

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

The implementation record is
[`tactical/231-bushy-leaf-rendering.md`](../tactical/231-bushy-leaf-rendering.md).

## Bottom Line

Mclone does **not** need hand-painted bushy textures for this effect. The
implemented path derives an original doubled sprite from each active square
leaf texture in memory, then uses that sprite on two double-sided cards.
Whichever pack supplied the leaf remains the art authority.

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

## Implemented Mclone Contract

The shipped shared contract is:

- `LeafDetail::{Blocky, Bushy}` is catalog policy in `mclone-mesh`.
- Every resolved square `_leaves` material receives an in-memory 2x derivative
  made from the source pixels and an original analytic mask. No derived PNG is
  checked in.
- The ordinary cube keeps the source sprite. `Bushy` appends two double-sided
  vertical cards, exactly four quads, using the derivative.
- A stable world-position hash selects one of four original layouts.
- A leaf with leaf neighbors on all six sides emits no decorative cards.
- The cards use the ordinary foliage tint, packed light, cutout phase, section
  mesh, and draw submission.
- Ordinary and placed section culling bounds include the exact 0.25-block
  overhang. Mono, flat multi-view, stereo/per-eye, and full-frame multiview all
  consume that same resident mesh.

`Leaf Detail: Blocky / Bushy` is a shared Graphics row. A change clones the
active prepared bundle, changes catalog policy, rebuilds through the existing
asset-epoch replacement, and commits only after replacement succeeds. Native
preparation is background work; browser preparation uses the portable
synchronous replacement implementation behind the same scene contract.

Live choices are serialized with every other asset-epoch replacement. If a
replacement is already active, the scene records the latest requested leaf
detail and reports successful handling to the input router. The pending choice
survives the active commit and begins on the next replacement poll. Repeated
clicks coalesce, while reversing from `Bushy` to `Blocky` during a Bushy
compile queues the reversal instead of losing it.

This serialization closes a 2026-07-24 Steam Deck/macOS regression. A Leaf
Detail click could previously reach `begin_prepared_asset_replacement` while
another replacement was active, propagate `an asset replacement is already in
progress` through desktop pointer routing, and trigger the desktop host's
intentional exit-on-unexpected-input-error policy. The host policy remains
strict; Leaf Detail contention is now an expected shared scene state rather
than an input error.

Schema-1 `ClientGraphicsPreferences` currently owns only leaf detail. Native
hosts use the atomic `graphics-preferences.v1.json` document, browser uses
`mclone.graphics.preferences.v1` in `localStorage`, and Factory Reset removes
both forms. Desktop flat/XR, flat Android, Android XR, and web restore the
choice. Direct-to-world startup may replace its compile catalog before the
first poll, so a stored Bushy choice affects the first compiled epoch rather
than triggering a throwaway Blocky epoch.

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
- let any future distant-terrain system derive its own canopy summary rather
  than preserving tiny bush cards where they cannot contribute useful
  silhouette;
- retain a true Blocky mode that emits no extra vertices or indices; and
- measure desktop, Steam Deck, browser WebGPU, Android, and physical Quest
  independently before choosing defaults.

The inspected pack recommends Cull Leaves and includes options that force leaf
culling and hide inner leaves. Mclone should not import that mod's policy
implicitly or change vanilla-target base-leaf semantics as a side effect.
Skipping only optional bush cards for provably enclosed leaves is a narrower
optimization.

## Existing Mclone Fit And Resolved Gaps

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

The research also established why the Better Leaves ZIP was not a drop-in
mclone feature:

- the Minecraft JSON adapter currently keeps only the first weighted blockstate
  variant instead of selecting a stable position-dependent variant;
- the model parser supports blockstate rotations but not per-element model
  rotations, which the outer Better Leaves planes use;
- the first-party visual catalog originally classified leaves as ordinary
  solid cubes and had no optional leaf-detail contract; and
- a graphics-quality change that alters compiled leaf geometry needed a shared
  remesh/commit path rather than an app-local toggle.

The implementation deliberately bypasses those resource-model gaps: it is
native shared mesh/asset policy, not a parser expansion or a copied external
pack.

## Implemented Design

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

The current analytic mask and four layout variants are the accepted first
implementation. Further art-direction work may revise them, but bespoke
species textures are not an engine prerequisite.

### Shared mesh compilation

The implemented production shape is:

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

If broader measurement later shows baked-card memory, remesh latency, or
distance control is unacceptable, the next option is a compact section-keyed
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
grass runtime effect and preference field exist:

| Overall preset | Leaf Detail | Grass Detail |
|---|---|---|
| Low | Blocky | Off |
| Medium | Bushy if measured safe | Sparse |
| High | Bushy | Lush |
| Ultra | Bushy | Ultra |

This table is a suggested projection, not a current product default. Manual
edits should eventually make an overall preset report `Custom`, consistent
with [`graphics-video-settings.md`](graphics-video-settings.md).

Leaf Detail now lives in machine-local `ClientGraphicsPreferences`, not world
persistence or asset-pack selection. Grass should add its own independent
field to that existing document when implemented. `Blocky` remains the
default on every platform; the current measurements do not justify changing
that default on any profile.

Changing Leaf Detail uses derived-asset preparation and section recompilation.
The scene keeps the old drawable epoch until the replacement is ready, commits
at a shared frame boundary, persists only the accepted result, and retains the
old setting/resources on failure.

## Measured Closeout

The deterministic desktop comparison used seed 12345, RD2, frozen noon,
1600x1000 output, and the same eye/target. Passive actors were disabled.

| Exact mono mesh fact | Blocky | Bushy | Delta |
|---|---:|---:|---:|
| resident sections | 64 | 64 | 0 |
| drawn sections | 13 | 13 | 0 |
| total faces | 63,892 | 72,288 | +8,396 / +13.14% |
| total vertices | 255,568 | 289,152 | +33,584 |
| total indices | 383,352 | 433,728 | +50,376 |
| drawn faces | 15,454 | 20,638 | +5,184 / +33.55% |
| drawn indices | 92,724 | 123,828 | +31,104 / +33.55% |

The exact +8,396 faces are 2,099 admitted surface leaves at four quads each.
At the current 40-byte vertex and 32-bit index layout, the added raw section
mesh is 1,544,864 bytes, exactly 736 bytes per admitted leaf. The fixed camera
drew 1,296 of those admitted leaves.

Both policies produced a 1024x2048 atlas: 8,388,608 base bytes and 11,173,888
bytes across five mip levels. Derivatives are prepared in both modes so a live
change can be catalog-only; the six current leaf derivatives fit existing
packing without increasing atlas dimensions or allocated bytes.

The inspected Blocky and Bushy captures differ in 181,793 of 1,600,000 pixels
(11.36%, ImageMagick RMSE 0.0717617). Bushy visibly changes the spruce canopy
from regular cubes to a fuller irregular silhouette while retaining the
active source art. The stereo capture is 1600x800, with two 800x800 eyes and
392,174 differing eye pixels; both eyes show the cards with normal parallax.

A headed Wayland WebGPU app smoke restored a Bushy browser preference before
Web Worker compilation, completed ordinary streaming/movement/block-edit and
native-UI probes, and produced an inspected non-clear 1280x720 canvas with
bushy foliage. The smoke harness accepts `--leaf-detail blocky|bushy` for
repeatable captures.

### Performance samples

These Linux-host samples characterize this scene, not every GPU:

- Three warmed, frozen-runtime 960x600 RD2 timedemos had a median average frame
  time of 0.506 ms Blocky versus 0.535 ms Bushy: +0.029 ms / +5.7%.
- Average drawn indices in that lane were 284,233 versus 321,556: +13.13%.
- One 240-frame movement sample measured 1.087 ms average / 1.847 ms p95
  Blocky and 1.401 ms / 3.090 ms p95 Bushy. Render time rose from 0.173 to
  0.238 ms and drawn indices by 10.62%.
- A stationary-orbit sample was noisy enough to make Bushy appear faster.
  Neither policy exceeded the 16.67 ms budget, all 240 frames were accounted,
  and conservation violations were zero.

The increased geometry and the noisy sub-millisecond host results support
keeping `Blocky` as the default. They do not justify a more complex instanced
path or stricter surface admission yet.

### Validation record

Passing gates include:

- 92 `mclone-mesh` tests, including generator, policy, exact face, layout, and
  enclosed-leaf contracts;
- 166 `mclone-render` tests, with eight GPU characterization tests ignored by
  default;
- 323 `mclone-app-runtime`, 153 `mclone-scene`, and 100 `mclone-ui` tests plus
  scene integration suites;
- native workspace checks and the native client all-target check;
- exact mono and headset-free stereo captures inspected under `/tmp`;
- wasm build, generated-glue typecheck, headed WebGPU worker-backed app smoke,
  and inspected browser pixels;
- flat Android debug APK and Android XR release APK builds.

Post-closeout contention regression validation passed the full `mclone-scene`
and `mclone-native-client` cargo test suites, including the busy/same-detail
and busy/changed-detail request contracts. This was shared-state and
input-routing validation; no new pixels were introduced by the fix.

The ordinary cutout section mesh is shared by mono, per-eye, and full-frame
multiview; no Bushy renderer or per-eye branch exists. The Android XR release
build proves the full-frame multiview compile boundary. No Quest was attached,
so physical headset pixels and timing remain open and are required before any
mobile/XR default change.

## Remaining Follow-up

- Measure on representative Steam Deck, Android, desktop XR, and physical Quest
  hardware before considering a non-Blocky platform default.
- Revisit enclosed-only admission or compact instances only if those
  measurements identify leaf geometry, memory, or cutout overdraw as a real
  bottleneck.
- Treat mask/layout art-direction changes as versioned generator work and keep
  derivatives tied to source-pack provenance.
- Keep any future world-space vegetation wind shared only at the deformation
  fact level; leaf and grass settings, eligibility, and caches stay
  independent.
