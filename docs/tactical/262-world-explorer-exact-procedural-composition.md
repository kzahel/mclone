# Tactical 262: World Explorer Exact/Procedural Composition

Status: Slice 3B native correction and screenshot gate complete 2026-07-27
after Human Review 1A was retracted. Slices 0 through 3A proved shared
target/depth plumbing, exact-painted coverage, and whole-tree exact/proxy
ownership, but the orbit host still placed its focus-centered exact square
behind procedural foreground terrain at low pitch. Slice 3B now gives the
exact patch, procedural rings, vegetation, and camera one viewer-forward
composition anchor while retaining the accepted collar as the explicit seam
treatment. Browser composition is next; Terrain Lab and game-scene promotion
remain gated.

Topics:

- `procedural-horizon-clipmap`
- `lod-native-vegetation`

Parent:

- [`261`](261-procedural-horizon-product-integration-roadmap.md) is the global
  procedural-horizon parent. This tactical implements PH-1 and PH-2 before
  game-scene adoption.

Related directions:

- [`procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  owns exact-painted coverage, masking, frontier treatment, and horizon
  residency;
- [`world-view-navigation.md`](../topics/world-view-navigation.md) already
  sequences a reusable exact-view source and exact/procedural Explorer
  composition before the player-facing map-to-world view;
- [`gpu-procedural-terrain.md`](../topics/gpu-procedural-terrain.md) owns the
  Terrain Lab comparison evidence and source-quality work; and
- [`lod-native-vegetation.md`](../topics/lod-native-vegetation.md) owns stable
  natural-tree identity, conservative bounds, exact realization, proxy
  presentation, and the exact/proxy XOR contract; and
- [`244`](244-lod-native-vegetation-presentation.md) is now the completed
  footprint-summary and vegetation-representation proof rather than an open
  composition owner.

## Objective

Prove the missing reusable compositor before introducing full game lifecycle:

```text
locally compiled exact near field
              |
              v
renderer-neutral exact-painted snapshot
              |
              +------> fixed exact coverage mask
              |                 |
procedural clipmap -------------+
              |
              v
one color target + one reversed-Z depth target
              |
              v
Horizon | Exact | Composed | Coverage diagnostic
```

World Explorer must render a bounded exact near field and the existing
procedural horizon through one device, encoder, color target, depth target,
camera, and source identity. `Composed` is the acceptance mode. A side-by-side
comparison is useful diagnostics but cannot substitute for real masking,
shared depth, and frontier geometry.

The proof must produce shared contracts that the game can consume later. It
must not import `mclone-scene`, the client/server runtime, React, Terrain Lab
UI policy, or an app-local copy of exact/procedural arbitration.

## Why World Explorer First

World Explorer already owns the newest proven horizon session:

- fixed ten-level toroidal residency;
- requested/staged/committed terrain and vegetation presentations;
- native-thread and browser-Worker vegetation execution;
- deterministic native/offscreen/browser movement receipts;
- shared view controls; and
- a deliberately small dependency firewall.

Terrain Lab already owns the mature canonical comparison path, but its exact
renderer and coordinator are currently browser-app-local and use a separate
surface, device, depth target, and canvas from each procedural pane. A full
Lab renovation would mix reusable extraction with React layout and
browser-only lifecycle.

The useful Terrain Lab work in this tactical is narrow: extract its canonical
mesh compiler/session and exact-painted readiness facts into a reusable Rust
boundary. World Explorer then proves the compositor. A later slice may feed
the same runtime-horizon/composed view back into Terrain Lab without replacing
its CPU/GPU/reference research panes.

The ordinary game already has the production exact renderer, but adopting the
horizon there first would combine the unproven compositor with world
lifecycle, settings, render-section scheduling, browser/Android construction,
and XR admission. The game follows only after the smaller proof is accepted.

## Binding Contracts

### Exact-painted means drawable now

An exact chunk enters `ExactPaintedCoverageSnapshot` only after every packed
opaque/cutout resource required by the proof renderer has been accepted for
the current source and generation. Requested, generated, compiled, resident,
or queued chunks do not mask procedural terrain.

The snapshot carries:

- terrain source identity;
- monotonically changing coverage generation;
- the exact painted chunk set; and
- bounded mask dimensions or an equivalent validated coverage encoding.

Late results from an older source, desired footprint, or generation are
rejected before upload or mask publication.

### One caller-owned target

The horizon renderer must support a caller-owned color and reversed-Z depth
target with explicit clear/load/store behavior. Its current convenience path
may retain an internal depth target for existing callers, but the composed
path cannot clear or replace depth between procedural and exact draws.

The first order is:

1. clear and draw masked procedural terrain plus eligible tree proxies;
2. load color/depth and draw exact opaque, cutout, and translucent sections;
3. retain enough depth for the existing capture proof; and
4. expose one coherent frame report.

Later game composition may split actors and translucent water into more
precise phases. This proof must not make that later ordering harder.

### Coverage mask, not depth arbitration

The procedural fragment path maps world X/Z to exact chunk coordinates and
consults a bounded GPU mask. In `Composed` mode it discards painted chunks. In
coverage diagnostics it visualizes the same ownership without changing which
chunks the CPU considers painted.

Depth testing is still required, but it is not ownership. Approximate terrain
may otherwise cover exact geometry or z-fight.

### Explicit frontier treatment

The first exact near field retains a narrow boundary wall, skirt, or collar
where exact and approximate heights can disagree. The implementation must
name and diagnose that treatment rather than hiding cracks with accidental
overdraw.

### Bounded representation ownership, with trees first

The reusable substrate is a small bounded-representation ownership envelope,
not a universal natural-feature payload or renderer. An ownership unit
carries:

- composition source identity;
- a feature-family-specific stable unit ID;
- complete conservative working bounds;
- exact and approximate drawable readiness;
- the selected owner and ownership generation; and
- enough diagnostics to prove exactly one visible representation.

Each feature family chooses its useful ownership unit and retains its own
planner, payload, compiler, and renderer. A tree uses one stable tree record.
A later route may use a segment, while a large structure may use a bounded
piece. Aggregate fields and dynamic entities do not become fake bounded
features merely to reuse this contract.

Trees are the first adapter because they already have stable IDs, exact
realization, proxy presentation, and conservative bounds. Terrain coverage
and tree ownership are related but are not the same mask. A tree may cross
chunk edges and may intersect the procedural collar even when its base chunk
is exact-painted.

An exact natural tree is eligible only when:

- its stable source and occurrence ID match the current composition source;
- its complete conservative working bounds lie inside the exact-safe terrain
  interior, excluding every procedural collar fragment;
- every exact tree draw resource for the record is drawable now; and
- the ownership generation participates in the same immutable frame decision
  as terrain coverage.

Otherwise the complete record remains proxy-owned. Exact and proxy tree
rendering switch atomically by stable record ID. Neither base-chunk tests nor
fragment-by-fragment proxy discard satisfy this contract.

Exact natural-tree draw admission must therefore be separable from exact
terrain admission. The correction must not hide the defect by shrinking
crowns, expanding the terrain mask blindly, deleting all boundary trees, or
allowing depth to choose between representations.

### Shared source, host-specific production

The proof exact source may compile canonical chunks locally. The later game
produces exact sections from `ClientRuntime`. Both publish the same
renderer-neutral exact-painted snapshot and consume the same coverage/mask
contract.

Canonical generation and mesh preparation may be reused from Terrain Lab.
React scheduling, Lab URLs, browser canvas ownership, and the Lab-specific
coordinator are not reusable engine policy.

## Diagnostic Modes

World Explorer exposes these Rust-owned modes through native hotkeys and
query/raw-key browser mechanics:

| Mode | Required result |
|---|---|
| `Horizon` | Existing procedural-only result, byte/pixel compatible except for intentional target-contract refactoring. |
| `Exact` | Bounded canonical near field over the normal background with no procedural draw. |
| `Composed` | Procedural horizon outside the exact-painted mask and exact chunks inside it on one shared target. |
| `Coverage` | A conspicuous visualization of the same painted mask/frontier used by `Composed`. |

Suggested native bindings are `1` through `4`. The visible product remains
UI-less; title/diagnostics may name the mode, and browser smoke may select it
through an explicit query parameter.

## Slice Plan

### Slice 0: parent correction and execution contract

- [x] Record the proof-first sequence in Tactical 261.
- [x] Close Tactical 244's stale representation-decision ledger.
- [x] Define the shared target, exact-painted, mask, frontier, and diagnostic
  contracts here.
- [x] Preserve game-scene and XR adoption as later parent phases.

Gate: implementation has one small proof owner and no host-local arbitration.

### Slice 1: reusable target and coverage substrate

- [x] Add a platform-neutral exact-painted snapshot with source/generation
  checks and negative-coordinate mask packing tests.
- [x] Let the horizon encode into caller-owned color/depth attachments with
  explicit clear/load/store behavior.
- [x] Add a bounded exact-coverage GPU resource and procedural terrain/tree
  mask path.
- [x] Preserve the existing Horizon convenience path, allocation report, and
  native/browser builds.

Gate: unit/offscreen tests prove the mask identity and one shared depth
attachment without exact generation.

### Slice 2: reusable canonical exact near field

- [x] Move the canonical mesh compiler/session and packed batch codec out of
  Terrain Lab app-local ownership without changing its behavior.
- [x] Add a bounded native threaded exact producer for World Explorer.
- [x] Admit no more than one exact result per frame and retain valid overlap
  while the desired footprint moves.
- [x] Publish exact-painted chunks only after accepted mesh upload.
- [x] Retain an explicit procedural collar at the composed frontier.

Gate: `Exact` mode moves through a bounded near field without presentation-
thread generation or stale uploads.

### Slice 3: composed World Explorer and Human Review 1

- [x] Add `Horizon`, `Exact`, `Composed`, and `Coverage` modes.
- [x] Align exact and procedural cameras, color transfer, source identity, and
  reversed-Z target.
- [x] Exercise stationary, movement, negative coordinates, exact
  admission/eviction, deliberately delayed exact work, and teleport.
- [x] Capture and inspect native window/offscreen pixels at the first composed
  frame and after movement.
- [x] Record mask population, exact resident/painted chunks, stale results,
  frontier mode, exact draw counts, and fixed/transient bytes.

Gate: pause for Human Review 1. Do not begin game-scene integration.

## Review Candidate Evidence

The implementation series is:

| Commit | Result |
|---|---|
| `2edc9f57` | Added the renderer-neutral exact-painted snapshot, negative-coordinate mask packing, caller-owned target contract, and procedural terrain/tree mask. |
| `c80a45b9` | Moved the canonical compiler session and packed mesh codec from Terrain Lab into `mclone-terrain-view` without changing the Lab's Wasm build. |
| `39d210e7` | Added the bounded native exact producer and real World Explorer composition on one color/depth target, including the explicit procedural collar. |
| `116d3189` | Added exact/mask ownership assertions and detailed composition receipts to every native smoke checkpoint. |

The final delayed-work smoke was:

```text
pnpm native:world-explorer:smoke \
  /tmp/mclone-world-explorer-composed-review \
  --composition composed \
  --blocks-across 256 \
  --exact-radius 2 \
  --exact-delay-ms 20
```

Both native-window and offscreen lanes passed initial 3D, axial and diagonal
movement, negative coordinates, million-block teleport, zoom, map, and orbit
checkpoints. The final offscreen receipt reported:

- `25` desired and `25` painted exact chunks;
- exact coverage generation `146`, with the procedural mask checked against
  that same generation;
- zero queued chunks, pending admissions, or in-flight work;
- `175` admitted results and `16` deliberately stale movement results rejected;
- `50` drawn exact sections and `154,728` drawn exact indices;
- `6,383,392` bytes of resident exact mesh data; and
- frontier mode `procedural-collar-1.5-blocks`.

Focused validation also passed:

```text
cargo test --manifest-path native/Cargo.toml -p mclone-terrain-view
cargo test --manifest-path native/Cargo.toml \
  -p mclone-world-explorer --lib
cargo check --manifest-path native/Cargo.toml -p mclone-world-explorer
cargo check --manifest-path native/Cargo.toml \
  -p mclone-world-explorer --lib --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml \
  -p mclone-terrain-lab --lib --target wasm32-unknown-unknown
```

Inspected matched `Horizon`, `Exact`, `Composed`, and `Coverage` captures show
that the 5-by-5 exact footprint and mask agree. `Exact` no longer produces
full-height footprint walls. `Composed` suppresses procedural terrain inside
the painted footprint and keeps a 1.5-block procedural collar at its edge.
Dark stepped bands near some rivers are also visible in `Horizon`, so they
predate this compositor.

The review candidate is intentionally not visually final. Forest and surface
detail change conspicuously at the square exact footprint. Ocean and coast
anchors can show an especially strong blue/gray square because canonical
exact water/surface output and the procedural horizon do not yet summarize
the source identically. The ownership is stable—there is no old full-height
wall—but this semantic/appearance discontinuity is the main Human Review 1
question.

## Human Review 1 Result

Interactive review on 2026-07-26 accepted the terrain compositor and collar
overall. It found one repeatable blocking vegetation defect at the frontier.
Matched `Exact` and `Composed` views show the same logical natural tree as:

1. exact block geometry admitted with its exact chunk;
2. exact geometry partly hidden by retained procedural collar terrain; and
3. the remaining outside portion of its LOD proxy, because proxy fragments
   are discarded independently by their world-space chunk.

The result is a visually implausible exact/proxy chimera. This is not a depth
precision failure and not merely final forest quality. Depth is faithfully
showing two incompatible ownership decisions.

The review changes the sequence. Browser proof, Terrain Lab adoption, and
game-scene integration remain gated until Slice 3A proves whole-record
vegetation arbitration. The later PH-5 work still owns authoritative edits,
production-scene invalidation, and lifecycle; only the reusable untouched
natural-tree XOR primitive moves forward into this proof.

### Slice 3A: whole-tree frontier arbitration

- [x] Add a renderer-neutral bounded-representation ownership snapshot keyed
  by source, ownership generation, stable unit ID, complete working bounds,
  exact/approximate readiness, and selected owner.
- [x] Add the first tree adapter keyed by stable occurrence ID, terrain
  coverage generation, and `McloneTreeRecord` working bounds without putting
  tree payload or rendering policy in the neutral envelope.
- [x] Separate exact natural-tree draw admission from exact terrain
  admission without changing terrain, low-vegetation, or reference-profile
  semantics.
- [x] Keep a complete record proxy-owned whenever its bounds touch unpainted
  terrain or the procedural collar; make it exact-owned only after its full
  exact-safe footprint and exact draw resources are ready.
- [x] Suppress or admit the complete proxy instance by stable record ID; do
  not clip it fragment-by-fragment at chunk boundaries.
- [x] Switch exact/proxy ownership atomically on admission, eviction,
  movement, delayed work, and source reset.
- [x] Extend `Coverage` and smoke receipts with exact-owned, proxy-owned,
  frontier-crossing, dual-owned, and unowned record counts.
- [x] Add direct edge/corner, cross-chunk crown, negative-coordinate,
  delayed-admission, eviction, and teleport fixtures.
- [x] Capture and inspect the reported forest anchor plus additional dense
  forest boundaries before requesting Human Review 1A.

Gate: every eligible natural tree has exactly one complete visible
representation, procedural terrain never cuts an exact-owned tree, and
Human Review 1A accepts the same boundary views.

### Human Review 1A Candidate Evidence

Commit `8c44acaf` completes the native whole-record proof:

- canonical compilation can separate stable natural-tree meshes from exact
  terrain while leaving low vegetation and other feature families unchanged;
- one neutral ownership snapshot selects exact or approximate presentation
  for a complete stable occurrence and generation;
- exact ownership requires complete working bounds inside the painted
  terrain's true 1.5-block-collar-safe interior and drawable exact resources;
- every clipmap copy of an exact-owned ID is removed on the CPU; the tree
  shader no longer clips a proxy fragment-by-fragment against exact chunks;
  and
- exact-tree GPU sections rebuild from the same ownership snapshot used to
  filter proxies, including admission and eviction.

The delayed native-window and offscreen smoke passed:

```text
pnpm native:world-explorer:smoke \
  /tmp/mclone-world-explorer-tree-ownership-review \
  --composition composed \
  --blocks-across 256 \
  --exact-radius 2 \
  --exact-delay-ms 20
```

The initial forest checkpoint reported `17` owned records: `12` exact and
`5` frontier-crossing proxies. The clipmap contained `22` copies of those
`12` exact-owned IDs across its nested record tiles; every copy was
suppressed, with `0` missing exact IDs, `0` dual-owned records, and `0`
unowned records; all `5` proxy-owned IDs were also resident. After delayed
movement, the corresponding receipt was `16` records (`12` exact and `4`
proxy), `27` suppressed clipmap instances for all `12` exact IDs, and again
zero missing exact-owned or proxy-owned IDs, dual ownership, or unowned
records.

Both lanes completed axial/diagonal movement, negative coordinates,
admission/eviction, deliberately delayed work, and million-block teleport.
The final receipts retained `25/25` painted exact chunks, rejected `16` stale
results, and admitted `175` current results. Stationary, closer 128-block,
moved forest, and `Coverage` captures were inspected. They show complete
exact trees in the safe interior and complete proxies at the collar; the
reviewed exact/proxy chimera is not visible.

### Human Review 1A Retraction and Slice 3B

The initial Human Review 1A acceptance was retracted on 2026-07-27 after
lower camera perspectives exposed a broader apparent occlusion. Exact still
draws second into the same reversed-Z depth buffer. Render order was not the
defect: the orbit eye sat well outside the focus-centered 5-by-5 exact square,
so procedural ground was geographically between the eye and the patch and
correctly won depth.

A full exact-footprint procedural discard was tested at the same deterministic
camera. It made no material change to the broad occlusion and exposed open
boundary cracks when the collar was removed. That disproved the proposed
full-mask/transition rewrite as the fix for this report. The 1.5-block collar
remains the named seam treatment; whole-tree ownership continues to keep
collar-crossing trees proxy-owned.

Slice 3B instead:

- [x] established matched exact/composed/coverage captures at
  `blocksAcross=96`, `pitch=0.12`, and all four cardinal yaw directions;
- [x] proved with a temporary full-footprint discard that collar overlap was
  not the cause of the whole-patch behavior;
- [x] added one shared viewer-forward composition anchor for exact terrain,
  procedural ring residency, and vegetation records;
- [x] kept map and ordinary Horizon behavior focus-centered;
- [x] added a shared optional camera target height so composed low-angle
  views retain their requested pitch but gain 32 blocks of viewer-side
  terrain clearance when sea-level targeting would put the camera in ground;
- [x] inspected exact/composed/coverage captures from all four sides across
  land, water, slopes, and forest; and
- [x] reran delayed admission, movement, eviction, negative-coordinate,
  teleport, map, and orbit smoke in native-window and offscreen lanes.

The delayed smoke completed with `25/25` exact chunks, `0` missing exact or
proxy tree records at every checkpoint, `16` stale results rejected during
movement, and bounded `160`-slot procedural residency. The known failing
camera now places exact terrain in the foreground and procedural terrain
behind it in all four directions while both still use one depth buffer.

Gate: native screenshot evidence is now a credible Human Review 1B candidate.
Browser Worker composition is the next slice and must expose the same seed,
center, scale, yaw, pitch, composition, and exact-radius facts through query
parameters because interactive human validation is browser-only.

Focused validation passed:

```text
cargo test --manifest-path native/Cargo.toml -p mclone-mesh \
  merged_tree_sections_preserve_render_phase_ranges
cargo test --manifest-path native/Cargo.toml -p mclone-terrain-view
cargo check --manifest-path native/Cargo.toml -p mclone-world-explorer
cargo check --manifest-path native/Cargo.toml \
  -p mclone-world-explorer --lib --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml \
  -p mclone-terrain-lab --lib --target wasm32-unknown-unknown
```

### 2026-07-26 to 2026-07-27 browser shader compatibility hotfix

The shared coverage diagnostic initially declared a WGSL local named
`diagnostic`. A mobile browser rejected that reserved language word while
creating `mclone_terrain_viewport_render_shader`, so even the deployed
Horizon-only browser profile could not initialize after the composition
substrate landed. The local is now named `coverage_color`; this is a source
compatibility correction with no rendering or composition change.

The browser smoke's fixed-residency assertion now includes the composition
coverage resource and expects `128,838,264` bytes. Local desktop and Pixel 7
WebGPU smokes passed pipeline initialization, Worker recovery, continuous
input, negative coordinates, and teleport. The native offscreen capture
retained meaningful color and depth and was inspected. Chrome 150's local
headed screenshot path returned a transparent canvas despite successful
surface submissions, so that blank capture is not retained as pixel evidence.

The first corrected deployment was Cloudflare Worker version
`be63f9a9-acad-4a60-bfce-ea74e50a7ea6`. A later full-site deployment,
`11eb0c10-e778-4ae6-92a7-d0be5d05c4b3`, restored an older World Explorer
bundle and exposed the mobile shader failure again. The full current site was
rebuilt and restored as version `154246b9-6331-4d51-981a-a7d170e99cff`.
The hosted and local Explorer Wasm SHA-256 is
`432f26547f0cd88cbf30047d3eb9f8aa586c58cae455615df12760bf2eee5c80`.
Direct hosted desktop and Pixel 7 smokes then passed without page, shader, or
console errors, including Worker recovery, movement, negative coordinates,
teleport, and shutdown. This restores the existing Horizon browser product;
it does not complete Slice 4's composed exact-field Worker proof.

### Slice 4: browser proof

- [ ] Reuse the shared canonical compiler/session behind an isolated browser
  Worker and domain-blind transport.
- [ ] Keep exact compilation off `requestAnimationFrame`.
- [ ] Add desktop and Pixel 7 headed-Wayland `Composed`/`Coverage` receipts and
  inspected captures.
- [ ] Preserve the UI-less browser host and payload/dependency accounting.

Gate: native and browser agree on source, painted chunks, mask generation,
and composition behavior.

### Slice 5: Terrain Lab adoption

- [ ] Add the proven runtime horizon/composed presentation as a shared Lab
  consumer only if the comparison improves the research workflow.
- [ ] Preserve existing canonical, CPU LOD, fast CPU LOD, and GPU LOD panes.
- [ ] Do not move compositor, source identity, or mask policy into React.
- [ ] Close the remaining world-view-navigation exact-source extraction step.

Gate: Terrain Lab can inspect the runtime composition without becoming its
engine owner.

### Slice 6: closeout and game handoff

- [ ] Update Tactical 261 evidence and mark PH-1/PH-2 complete.
- [ ] Record the exact construction seam for `mclone-scene`.
- [ ] Preserve the exact-only game baseline and app-rim executor rule.
- [ ] Open a separate game-scene adoption tactical after human acceptance.

## Human Review 1A

Run the native Explorer with:

```text
pnpm native:world-explorer:run \
  --composition composed \
  --blocks-across 256 \
  --exact-radius 2
```

Use `1` for `Horizon`, `2` for `Exact`, `3` for `Composed`, and `4` for
`Coverage`. To make admission behavior easier to see, add
`--exact-delay-ms 100`.

Review the native World Explorer at fixed and moving views. The focused
question is whether the previously reported partial exact/partial proxy tree
has disappeared:

1. toggle `Horizon`, `Exact`, `Composed`, and `Coverage` without moving the
   camera;
2. confirm composed terrain is continuous at all four exact edges;
3. at each exact edge, confirm a tree is either a complete block tree or a
   complete proxy, never pieces of both and never an exact tree cut by the
   collar;
4. look for cracks, vertical curtains, z-fighting, duplicate water, tree
   duplication, color/lighting discontinuity, or camera-like jumps;
5. move slowly across a chunk boundary while exact work is delayed;
6. zoom so the exact footprint is small enough to inspect against multiple
   clipmap levels;
7. inspect coast, steep terrain, water, and dense forest anchors; and
8. compare negative-coordinate and post-teleport behavior.

Subjective acceptance does not require final game lighting or a perfect
frontier treatment. It does require that the ownership model is visually
credible and that any remaining artifact is stable, localized, and explained
by diagnostics.

The first review accepted the terrain behavior but failed the natural-tree
XOR criterion below. Repeat this review as Human Review 1A after Slice 3A,
using the originally reported forest boundary as a required anchor.

## Acceptance At The Review Checkpoint

- One native World Explorer frame contains exact and procedural terrain on one
  color/depth target.
- The procedural mask equals the published exact-painted snapshot.
- Incomplete exact chunks leave procedural coverage visible.
- Exact admission removes procedural terrain atomically for that chunk.
- Exact eviction restores already committed procedural coverage.
- Procedural and exact natural trees do not visibly co-render inside painted
  chunks in the reviewed footprint.
- Negative coordinates and teleport do not shift mask ownership.
- Horizon-only remains the default and preserves its fixed budget.
- Exact-only performs no procedural draw work.
- Composition policy is reusable without `mclone-world-explorer`,
  `mclone-terrain-lab`, `mclone-scene`, DOM, `winit`, Android, or OpenXR types.
- Captures and a concise manual test recipe are available under `/tmp`.

## Non-Goals Before Human Review 1

- Full game, server, persistence, or `mclone-scene` integration.
- Browser exact Worker promotion.
- Terrain Lab UI changes.
- Authoritative edit invalidation.
- Multiple active/standby game worlds.
- Device-loss recovery beyond preserving existing Explorer behavior.
- Synthetic stereo, OpenXR, Quest, or multiview.
- Final water, fog, lighting, or vegetation quality.
- Distant player-build summaries.
