# 200: Box-Only Figure Authoring

Status: active 2026-07-20. The authoring-policy, canonical Chicken, and first
quadruped re-authoring slices are implemented and test-green; three content
waves and final compatibility cleanup follow. The final native offscreen pixel
rerun remains pending because the current host's Metal review process stalled
before producing its receipt.

Topic: `compiled-figure-rendering`

## Goal

Make sparse Minecraft-style cuboids the only canonical and promotable figure
primitive while retaining the rounded sources as explicit legacy A/B evidence.
Finish the current Asset Lab catalog with box-only canonical sources, promote
the approved Chicken to production, and prevent later authors from spending
topology on curved whiskers, limbs, spots, or other details better expressed by
pixel textures or omission.

## Binding decisions

- `figure()` is the canonical authoring entry point and accepts boxes only.
- `legacyFigure()` is an explicitly deprecated compatibility entry point for
  retained rounded comparison sources. Its sphere, capsule, and cylinder
  helpers are deprecated as well.
- Canonical examples live under `tools/asset-lab/examples`; rounded sources
  live under `tools/asset-lab/legacy-examples` and are not part of the default
  batch.
- Every promoted first-party figure must be box-only. The drift gate enforces
  this before generated JSON can be written or accepted.
- Schema-v1 JSON continues to parse sphere, capsule, and cylinder records.
  The shared Rust compiler retains its labeled cuboid-proxy compatibility path
  for legacy or replacement input during this campaign. Removing persisted
  kinds would require a separate schema decision.
- No animal is converted by merely substituting equal-bounds boxes for every
  old primitive. Each canonical source is re-authored as a sparse rig whose
  silhouette, attachments, grounding, texture detail, and motion are reviewed.
- Existing clip timing and locomotion metadata remain stable unless a visual
  review finds a concrete animation defect.

## Performance interpretation

Asset Lab's exact Three.js topology shows the future cost avoided by dropping
curves. The native prepared path already compiles every current primitive to a
12-triangle cuboid, so its immediate gain comes from fewer rigid parts, draw
ranges, and per-instance palette matrices rather than the full preview ratio.

For the production Chicken, the accepted source changes from 21 mixed parts to
14 boxes. Three.js triangles change from 3,108 to 168. Current prepared runtime
triangles change from 252 to 168, and part matrices change from 21 to 14. The
campaign must report both comparisons rather than presenting the authoring
ratio as measured end-to-end frame-time evidence.

## Implementation slices

### Slice 1: policy boundary and canonical approved set

- Split `FigureApi` from deprecated `LegacyFigureApi` and add a runtime
  box-only assertion to `figure()`.
- Move every rounded source to `legacy-examples`, keeping canonical JSON
  round-trip coverage for both roots.
- Promote the approved Elephant, Tiger, Rabbit, Chicken, and Butterfly box
  sources to their ordinary names.
- Enforce box-only first-party generation, regenerate Chicken, update prepared
  and legacy-renderer expectations, and refresh normal pack evidence.
- Render and inspect canonical/legacy Chicken sheets plus semantic/prepared
  static and animation comparisons.

Gate: all ordinary examples and all promoted JSON contain only boxes; rounded
sources still load and render only through the explicit legacy API; production
Chicken is recognizable, grounded, attached, and animated in flat and shared
prepared review paths.

Implemented evidence:

- `figure()` exposes only `box` and rejects handcrafted non-box drafts at
  runtime; deprecated `legacyFigure()` owns the three historical helpers.
- The canonical root currently contains seven figures, 128 parts, and 128
  boxes with zero curved primitives. The legacy root contains 18 figures and
  all 205 retained curved parts.
- Elephant, Tiger, Rabbit, Chicken, and Butterfly now own their ordinary
  canonical names. Their rounded sources have `_rounded` identities in the
  legacy root, so cross-root round-trip tests reject duplicate identities.
- Generated Chicken is 14 exact boxes, 336 preview vertices, 168 preview
  triangles, and zero prepared proxies. Player and Upright Bear remain exact
  box-only promoted sources.
- The canonical and rounded Chicken clean sheets were rendered and inspected.
  Both preserve the accepted walk timing, grounding, attachments, and readable
  anatomy; the canonical source is the previously approved blocky capture.
- Asset Lab's ten tests, three first-party drift checks, all 62 `mclone-assets`
  tests, all 158 non-ignored `mclone-render` tests, Rust formatting, the normal
  asset lock, standalone first-party pack rebuild, and strict provenance audit
  pass.
- The semantic side of the shared comparison wrote all three views. Two Metal
  review attempts then stalled inside the host GPU process before an engine
  receipt or actionable adapter error; no comparison pixel was accepted from
  those attempts. Retry the native review before final closeout rather than
  treating the host stall as a figure defect.

### Slice 2: Cat, Cow, and Goat

Re-author the highest-priority remaining quadrupeds. Preserve the cat tail,
cow horns/udder, and goat horns/beard as sparse cuboid silhouettes or pixel
detail. Render clean A/B sheets and three-cycle movies before committing.

Implemented evidence:

- Cat is 16 boxes rather than 20 mixed parts. Whiskers are omitted, tabby
  stripes move to textures, and an attached two-box vanilla-ocelot-style tail
  replaces the capsule.
- Cow is 22 boxes rather than 38 mixed parts. Its Holstein hide, lower-leg
  socks, face, and nostrils are textures; stepped horn pairs, a box udder, and
  tail tuft keep the identifying silhouette.
- Goat is 24 boxes rather than 32 mixed parts. Two-box swept horns, the
  articulated beard, slim legs, and an upturned two-box tail retain the billy
  goat read; a hoof-face texture carries the cloven split.
- All three keep their original `quadrupedWalk` duration, cycle distance,
  stance, body/head motion, and secondary ear/tail/beard tracks.
- Canonical and rounded clean sheets plus six three-cycle movies were rendered
  under `/tmp/mclone-asset-lab` and inspected. Sampled cycles stay grounded and
  all tail, horn, ear, muzzle, hoof, and beard attachments remain connected.
- The three canonical sources total 62 boxes versus 90 mixed legacy parts.
  Their exact Three.js preview total is 744 triangles versus 5,488, a 7.4x
  reduction; canonical part count is 31% lower.

### Slice 3: Dog, Fox, and Wolf

Use a consistent small-canid vocabulary without collapsing their proportions:
dog breadth and floppy ears, fox narrow muzzle/large ears/tail tip, and wolf
longer legs/chest/tail. Keep ordinary quadruped locomotion contracts.

### Slice 4: Piglet, Sheep, and Horse

Retain the piglet snout, sheep wool mass, and horse long-leg/mane silhouette
with box-only rigs. Favor stepped cuboids and textures over decorative parts.

### Slice 5: Bear, Lion, Bearfolk, and Lionfolk

Finish the heavy quadrupeds and the two player-derived anthropomorphic rigs.
The folk figures should reuse the box-only biped vocabulary rather than carry
rounded animal limbs into an otherwise cuboid body plan.

### Slice 6: closeout

- Inventory canonical and legacy figures separately and prove zero non-box
  canonical or promoted parts.
- Update the renderer topic to close exact curved topology, curved-texture,
  and segment-count LOD directions for production figures. Figure LOD becomes
  part omission/merge and texture policy.
- Keep proxy parsing/tests labeled as legacy compatibility; do not silently
  claim that the schema has become box-only.
- Run Asset Lab tests, focused shared asset/render tests, first-party pack
  validation, static/animation pixel review, default native gates, and web
  asset/render boundaries affected by the promoted Chicken change.

Gate: every canonical source is box-only, every retained rounded source is
explicitly legacy, all promoted assets and packs are current, screenshots and
movies have been inspected, and the working tree contains a coherent commit
series under `Topic: compiled-figure-rendering`.

## Deferred

- Removing legacy primitive fields from schema-v1 JSON.
- A schema-v2 migration or support promise for third-party replacement packs.
- Instancing, GPU pose evaluation, part-merging LOD generation, or texture LOD.
- Weighted skinning, morph targets, cloth, or other non-rigid figure systems.
