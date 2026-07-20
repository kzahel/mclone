# 200: Box-Only Figure Authoring

Status: complete 2026-07-20. The authoring policy, canonical Chicken, four
content re-authoring waves, compatibility cleanup, and broad non-GPU validation
are implemented and test-green. Canonical and legacy Asset Lab pixels were
inspected throughout the campaign. A fresh Linux follow-up approved the native
prepared Chicken comparisons and desktop offscreen receipt, replacing the old
Metal blocker. Browser asset-pack selection also passes at the shared-host and
provenance boundary, but Chrome 147/150 on this displayless host presents only
transparent or monochrome capture pixels, so no browser screenshot is accepted.

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
- At the end of Slice 1, the canonical root contained seven figures, 128
  parts, and 128 boxes with zero curved primitives. The legacy root contained
  18 figures and all 205 retained curved parts.
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
- The 2026-07-20 Linux follow-up rendered and inspected all three semantic and
  native prepared views. Chicken has matching textures, silhouette, framing,
  phase, and grounding; its three-cycle animation comparison also remains
  synchronized across exact, interpolated, and wrapped samples.

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

Implemented evidence:

- Dog is 17 boxes rather than 18 mixed parts. Its broad short barrel, hanging
  ears, collar, wide paws, and raised tail deliberately read as domestic.
- Fox is 18 boxes rather than 32 mixed parts. The vanilla-derived low narrow
  torso, oversized ears, texture-painted black stockings, and large attached
  two-box white-tipped tail keep it distinct from the other canids.
- Wolf is 16 boxes rather than 30 mixed parts. A separate vanilla-derived
  shoulder mass, tall legs, upright ears, narrow head, and heavy straight tail
  make it visibly larger and more angular than the dog.
- All three keep their archived trot duration, cycle distance, stance, body
  and head motion, and secondary ear/tail tracks.
- Canonical and rounded clean sheets plus six three-cycle movies were rendered
  under `/tmp/mclone-asset-lab` and inspected. Sampled cycles remain grounded,
  every attachment remains connected, and the three silhouettes separate at
  thumbnail scale.
- The three canonical sources total 51 boxes versus 80 mixed legacy parts.
  Exact Three.js preview topology is 612 triangles versus 4,748, a 7.8x
  reduction; canonical part count is 36% lower.

### Slice 4: Piglet, Sheep, and Horse

Retain the piglet snout, sheep wool mass, and horse long-leg/mane silhouette
with box-only rigs. Favor stepped cuboids and textures over decorative parts.

Implemented evidence:

- Piglet remains 14 parts but all are boxes. Its vanilla-style large head,
  projecting textured snout, compact body, floppy ears, and raised block tail
  retain the juvenile pig silhouette without a round tail or capsule legs.
- Sheep is 15 boxes rather than 17 mixed parts. One oversized fleece box plus
  a texture-painted irregular lower edge replaces ornamental side spheres;
  the dark face, wool forelock, short legs, and tail remain explicit.
- Horse is 22 boxes rather than 30 mixed parts. The vanilla-proportioned long
  barrel, angled neck, narrow head and muzzle, tall legs, four-part animated
  mane ridge, and hanging two-box tail keep its distinctive posture.
- All three keep their archived walk duration, cycle distance, stance, body
  and head motion, and secondary ear/mane/tail tracks.
- Canonical and rounded clean sheets plus six three-cycle movies were rendered
  under `/tmp/mclone-asset-lab` and inspected. Sampled cycles remain grounded
  and every snout, ear, neck, mane, hoof, and tail attachment stays connected.
- The three canonical sources total 51 boxes versus 61 mixed legacy parts.
  Exact Three.js preview topology is 612 triangles versus 4,144, a 6.8x
  reduction; canonical part count is 16% lower.

### Slice 5: Bear, Lion, Bearfolk, and Lionfolk

Finish the heavy quadrupeds and the two player-derived anthropomorphic rigs.
The folk figures should reuse the box-only biped vocabulary rather than carry
rounded animal limbs into an otherwise cuboid body plan.

Implemented evidence:

- Bear is 17 boxes rather than 41 mixed parts. Its vanilla-polar-bear-derived
  two-mass torso, shoulder hump, wide short legs, textured claw faces, tiny
  ears, and projecting muzzle preserve the heavy brown-bear silhouette.
- Lion is 23 boxes rather than 48 mixed parts. It keeps a lighter feline body,
  explicit chest/head mane frame, textured paws, and attached two-box tufted
  tail while omitting separate cheek, brow, haunch, and claw geometry.
- Bearfolk is a 15-box player-derived biped rather than 17 rounded parts. The
  broad head/muzzle, tiny ears, vest, belly patch, and paw textures now sit on
  the same cuboid torso and top-pivoted limb grammar as the canonical player.
- Lionfolk is a 20-box player-derived biped rather than 18 rounded parts. The
  slightly higher part count forms a real cuboid mane frame and two-box tail;
  exact topology still falls from 2,540 to 240 triangles.
- All four keep their archived clip duration, cycle distance, stance, body and
  head motion, and secondary neck/ear/mane/tail tracks.
- Canonical and rounded clean sheets plus eight three-cycle movies were
  rendered under `/tmp/mclone-asset-lab` and inspected. Sampled cycles remain
  grounded, every attachment remains connected, and the quadruped/folk pairs
  remain distinct at thumbnail scale.
- The four canonical sources total 75 boxes versus 124 mixed legacy parts.
  Exact Three.js preview topology is 900 triangles versus 9,256, a 10.3x
  reduction; canonical part count is 40% lower.

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

Implemented inventory evidence:

- The final canonical root contains 20 figures, 367 parts, and 367 boxes with
  zero spheres, capsules, or cylinders. The legacy root remains 18 figures,
  542 parts, 337 boxes, and 205 curved parts.
- Each of the 18 legacy identities has an ordinary canonical counterpart.
  Across those pairs, canonical sources use 340 boxes, 8,160 exact preview
  vertices, and 4,080 triangles versus 542 mixed parts, 32,959 vertices, and
  41,512 triangles. That is 37% fewer rigid parts and 10.2x fewer preview
  triangles.
- The combined source inventory has 38 figures, 909 parts, 80 ASCII textures,
  and 186 explicit box-face texture applications. No curved primitive carries
  a texture reference.

Implemented validation evidence:

- `pnpm asset-lab:typecheck` and `pnpm asset-lab:test` pass. The latter includes
  semantic source round trips, animation checks, generated first-party drift,
  and the promoted box-only gate.
- `cargo fmt --all --manifest-path native/Cargo.toml --check` and the full
  `cargo test --manifest-path native/Cargo.toml` workspace pass.
- `pnpm assets:pack:check`, `pnpm assets:pack:first-party:test`, a clean
  `pnpm assets:pack:first-party` rebuild, and strict
  `pnpm --silent assets:validate:first-party` provenance validation pass. The
  prepared inventory resolves all three promoted actor figures from the
  authored first-party pack and remains proprietary-free.
- `pnpm native:thin-adapters:purity` and `pnpm native:web:build` pass, proving
  that the shared ownership and browser/WASM compile boundaries remain intact.
- Canonical/legacy clean sheets and three-cycle movies for every content wave
  were rendered under `/tmp/mclone-asset-lab` and inspected incrementally. All
  sampled poses remain grounded, connected, legible, and animated.
- The 2026-07-20 review regenerated all 20 canonical sheets and movies plus
  explicit canonical/rounded sheets and three-cycle movies for all 18 paired
  identities. Every important PNG and temporal contact sheet was inspected.
  The sparse rigs remain recognizable, grounded, connected, and more charming;
  no skating, clipping, floating attachment, or detached Tiger tail was found.
- Both Chicken prepared comparisons pass. The static receipt reports 336
  vertices and 504 indices, and the animated receipt derives 896 palette bytes
  per write from Chicken's 14 prepared parts rather than assuming Player's 12.
- `pnpm native:desktop-offscreen:smoke` produced and passed a fresh 2560x1600
  full-frame receipt with 11 drawn terrain sections and both authored actors.
  The inspected Cow and Chicken have correct textures, silhouettes, framing,
  and terrain contact. The previous Metal queue timeout is therefore not a
  figure-renderer defect.
- `pnpm native:web:asset-pack-smoke` completes the authored selection at epoch
  1, retains the local-world session, restores `mclone-authored`, resolves 265
  authored-plus-fallback files, and reports zero Minecraft-reference or unknown
  provenance. The review exposed and fixed a browser promise race that could
  reborrow `WebSceneHost` while asset replacement owned it.
- Chrome 147 and 150 on this displayless Linux host still return a one-color
  transparent or white WebGPU presentation capture. Headless, headed Xvfb,
  explicit Vulkan, bundled Chromium, and SwiftShader runs agree; direct X11
  capture shows only the bootstrap presentation. The smoke now rejects these
  false-positive receipts after six forced overview submissions. This is a
  browser presentation/compositor limitation on this host, not an adapter
  discovery failure, and no browser PNG is claimed as visual evidence.

## Deferred

- Removing legacy primitive fields from schema-v1 JSON.
- A schema-v2 migration or support promise for third-party replacement packs.
- Instancing, GPU pose evaluation, part-merging LOD generation, or texture LOD.
- Weighted skinning, morph targets, cloth, or other non-rigid figure systems.
