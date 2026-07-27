# Tactical 262: World Explorer Exact/Procedural Composition

Status: Human Review 1B accepted the corrected composition on 2026-07-27.
Slice 4B commit `91fe9302` makes procedural terrain and proxy vegetation use
the same chunk view-projection matrix and nonlinear reversed-Z encoding as
exact chunks. The hosted normal and magenta source-color views preserve exact
tree and terrain silhouettes while still allowing nearer procedural geometry
to win depth. PH-1 and PH-2 are complete. Minor z-fighting limited to the
outermost exact blocks is accepted as a known frontier-overlap issue for later
collar/skirt refinement; it does not reopen the shared-depth correction.
Post-review commit `c75b488b` makes the orbit focus the default composition
anchor so exact terrain remains beneath the screen's point of interest.
The prior camera-relative placement remains available explicitly as
`viewer-forward`; hosted review of the new default is the next checkpoint.

Topics:

- `procedural-horizon-clipmap`
- `lod-native-vegetation`
- `world-view-navigation`

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

- [x] Reuse the shared canonical compiler/session behind an isolated browser
  Worker and domain-blind transport.
- [x] Keep exact compilation off `requestAnimationFrame`.
- [x] Add desktop and Pixel 7 headed-Wayland `Composed`/`Coverage` semantic
  receipts.
- [x] Preserve the UI-less browser host and payload/dependency accounting.
- [x] Content-version the browser module graph from the Explorer Wasm and
  pass hosted desktop `Composed`/`Coverage` plus Pixel 7 `Composed` gates.
- [x] Complete hosted interactive pixel review. Local Chrome presents WebGPU
  and reports coherent draw work, but its page screenshot API returns a
  solid-white canvas for this app; that image remains rejected as pixel
  evidence. The hosted view passed direct human review on 2026-07-27.

Gate: native and browser agree on source, painted chunks, mask generation,
whole-tree ownership, and composition draw behavior. Human review of hosted
pixels passed on 2026-07-27.

### Slice 4 browser evidence

Commits `4ecf414c` and `d816681e` extend the versioned canonical batch with
complete natural-tree records, move the exact renderer behind a native-thread
or browser-Worker executor, and expose `composition` plus `exactRadius` query
parameters. Ordinary Horizon still constructs neither the exact renderer nor
the exact Worker.

The fixed low-angle review camera uses seed `12345`, center `(0, 0)`,
`blocksAcross=96`, yaw `3.1415927`, pitch `0.12`, and radius `2`. Desktop and
Pixel 7 emulation passed both `Composed` and `Coverage` with:

- `25/25` desired and painted exact chunks;
- canonical and procedural coverage generation `27`;
- `15` complete natural-tree records split into `8` exact-owned and `7`
  proxy-owned records;
- zero missing exact or proxy tree representations;
- `6,958,200` resident exact mesh bytes; and
- a ready fixed `160`-slot procedural allocation.

The ordinary Horizon browser smoke also passed initialization, Worker
overflow/recovery, raw input, negative coordinates, teleport, and shutdown
with zero exact chunks or exact mesh allocation. The composition receipts are
under `/tmp/mclone-world-explorer-web-{desktop,phone}-{composed,coverage}-receipt.json`.
The four-sided native captures remain the accepted deterministic pixel
evidence. Hosted browser review must now confirm those same pixels
interactively; the rejected all-white local Chrome images are not evidence.

### Slice 4 hosted review deployment

The first production composition probe loaded an older browser module graph
even though direct fetches returned the current Explorer Wasm. Commit
`586c4607` closes that deployment boundary: the build derives one content
version from the Wasm and stamps it through the HTML, main module, shared
transport, smoke observer, both Workers, and explicit Wasm initialization
URLs. A browser can no longer mix those pieces across deployment generations.

Cloudflare production version `132e1eff-e324-4b7d-9878-fe6620f29629`
serves Explorer asset version `8a9712a54cd0b9df`. The hosted and local
Explorer Wasm SHA-256 is
`8a9712a54cd0b9df2100a3562c944b04f45dc0414caaa2a4de23ff0584dad988`.
Direct hosted validation passed:

- desktop `Composed`: `25/25` exact chunks, coverage generation `27`,
  `8` exact-owned plus `7` proxy-owned trees, and zero missing records;
- desktop `Coverage`: the same coherent ownership and generation with
  `visualize-painted` mask mode; and
- Pixel 7 `Composed`: the same `25/25`, `8/7`, and zero-missing receipt.

These gates prove that the public URL is executing the intended renderer,
Worker, source, ownership, and coverage state. They do not replace subjective
pixel review; Human Review 1B remains open until the hosted composed view is
inspected interactively.

### Human Review 1B rejection and Slice 4B

Hosted portrait review rejected the composition on 2026-07-27. Exact-only
frames contain complete foreground trees and hilltops, while matched composed
frames let the procedural surface cut through those silhouettes. This is not
limited to tree proxy ownership or the retained collar: ordinary exact terrain
is affected as well.

Source inspection and depth captures identify one renderer-contract defect:

- exact chunks and trees multiply world positions by
  `ChunkRenderView::view_projection`, which uses the engine's finite
  perspective reversed-Z matrix;
- procedural terrain manually projects X/Y, writes a linear
  `1 - (depth - near) / (far - near)` value with clip `w = 1`, and therefore
  does not produce the same depth distribution; and
- both pipelines then use `GreaterEqual` against one `Depth32Float` target.

The reviewed exact-only captures wrote maximum depths around `0.001` to
`0.003`; composed captures included procedural values around `0.94`. Those
numbers are not comparable even though the target, camera pose, clear value,
and compare function are shared. Changing draw order cannot correct an
incompatible depth coordinate.

Slice 4B must:

- [x] make procedural terrain use the same shared view-projection and
  reversed-Z encoding as exact chunks;
- [x] add a source-color diagnostic with exact geometry in bright magenta and
  procedural geometry in normal colors;
- [x] retain a tall silhouette or synthetic tower plus steep hill as matched
  exact/composed regression captures;
- [x] prove nearer exact geometry wins while genuinely nearer procedural
  geometry still occludes it; and
- [x] repeat native and hosted browser pixel review before treating semantic
  receipts as acceptance evidence.

Implementation commit `91fe9302` extends the shared terrain uniform from
`160` to `224` bytes with `ChunkRenderView::view_projection`. Both procedural
terrain and proxy-tree vertex shaders now output that matrix's complete clip
position; neither contains the retired linear `clip_z` expression. The fixed
horizon budget rises by `29,440` bytes across the `230` allocation and staging
resources, from `128,838,264` to `128,867,704` bytes including exact-coverage
resources.

The source-color diagnostic preserves texture alpha/cutouts and replaces
non-transparent exact atlas RGB with magenta. It is available as
`--source-colors` natively and `sourceColors=1` in the browser. Normal
procedural shading is deliberately unchanged.

Native inspected evidence:

- `/tmp/mclone-depth-shared-silhouette-exact.png`
- `/tmp/mclone-depth-shared-silhouette-composed.png`
- `/tmp/mclone-depth-shared-silhouette-source.png`
- `/tmp/mclone-depth-shared-hill-exact-portrait.png`
- `/tmp/mclone-depth-shared-hill-composed-portrait.png`
- `/tmp/mclone-depth-shared-hill-source-portrait.png`

The source-colored silhouette shows magenta exact canopies and ground
surviving in front of farther procedural hills. It also shows a nearer green
procedural proxy correctly covering magenta exact ground at the projected
frontier. The composed capture retains the exact-only foreground instead of
the rejected rectangular LOD slab. Its depth capture is entirely in the
shared nonlinear range (`0.0..0.002440` at the reviewed close view), rather
than combining exact values near `0.002` with procedural values near `0.94`.

Validation at this checkpoint:

- `cargo test -p mclone-terrain-view --lib`
- `cargo test -p mclone-world-explorer`
- `pnpm native:world-explorer:smoke`
- `pnpm host:check -- --probe-browser-webgpu`
- desktop and Pixel 7
  `native:world-explorer:web:composition-smoke --source-colors`

The browser build is asset version `e763583bf443efe7`. Both source-color
semantic receipts report `25/25` exact chunks, coverage generation `27`,
`8` exact-owned plus `7` proxy-owned trees, zero missing representations,
and fixed resident bytes `128,867,704`.

The review candidate was deployed from `2893c3dd` on 2026-07-27 as
Cloudflare production version `1f5b5861-1075-49cd-80d4-3f142574a199`.
Production `/explore/` serves the same `e763583bf443efe7` Explorer asset.
Direct hosted source-color composition smokes on desktop and the Pixel 7
profile reproduce the local `25/25` chunk, generation `27`, `8/7` ownership,
zero-missing, and `128,867,704`-byte receipts. These receipts establish
deployment and semantic parity; they do not replace the open interactive
pixel review.

Playwright's headed Wayland screenshot currently returns a one-color white
canvas for the Explorer. An isolated A/B build of the rejected pre-fix commit
`ff4363d2` returns the same white capture while the smaller host WebGPU probe
still captures valid pixels. Therefore that local screenshot is invalid
evidence for either candidate; it is not being counted as a pass or attributed
to this depth change.

Hosted Human Review 1B accepted the corrected composition on 2026-07-27:
exact trees and hill silhouettes remain correctly depth-ordered against the
procedural horizon. The reviewer observed minor z-fighting limited to the
outermost exact blocks. This is a localized near-coincident frontier overlap,
not a return of the incompatible projection-depth defect. It is accepted for
this proof and remains a known collar/skirt/ownership refinement under the
living `procedural-horizon-clipmap` topic.

### Post-review focus-anchor correction

The accepted depth and ownership proof exposed a separate Explorer usability
problem: Slice 3B's camera-relative anchor places the 5-by-5 exact footprint
near the orbit eye rather than beneath the orbit focus. At the 96-block review
scale and radius 2, the camera is about 150 blocks from the focus while the
exact center is only 48 blocks ahead of the eye. The patch can therefore sit
low in or outside the viewport, and changing yaw selects different exact
chunks even though the point of interest did not move.

Commit `c75b488b` introduces one explicit World Explorer exact-anchor policy:

- `focus` is the default for native and browser composition. It floors the
  shared `WorldViewState` focus and remains invariant across orbit yaw.
- `viewer-forward` preserves the prior camera-relative review placement.
  Native uses `--exact-anchor viewer-forward`; the browser uses
  `exactAnchor=viewer-forward`.
- map mode remains focus-centered under either policy because its eye has no
  horizontal offset.
- the selected anchor still drives exact compilation, procedural residency,
  coverage, and vegetation ownership together. This is a proof-host policy,
  not a new renderer or game-scene rule.

Inspected native evidence:

- `/tmp/mclone-exact-anchor-focus.png`
- `/tmp/mclone-exact-anchor-focus-magenta.png`
- `/tmp/mclone-exact-anchor-viewer-forward.png`
- `/tmp/mclone-exact-anchor-smoke/window/3d.png`
- `/tmp/mclone-exact-anchor-smoke/window/orbit.png`

The focus and magenta frames place the exact block patch around the
screen-center terrain. The viewer-forward frame reproduces the old foreground
placement. Native window/offscreen composed smoke passes movement, orbit,
map, negative coordinates, and teleport with `25/25` exact chunks. Local
desktop source-color and Pixel 7 browser receipts report `exactAnchor=focus`
at `(0, 0)`; the optional browser receipt reports `viewer-forward` at
`(-102, 0)`. All retain coherent coverage and zero missing tree
representations.

The candidate was deployed from `5cc1c9ea` as Cloudflare production version
`c89be118-a131-49c0-ac05-4b6cd3ab2b5d`. Production serves Explorer asset
version `507010324209c066`, whose Wasm SHA-256 is
`507010324209c066f4dad91fbc327eead578c5af69f0c550c31553c4b0c86b01`.
Hosted desktop source-color and Pixel 7 smokes reproduce the focus anchor at
`(0, 0)` with `25/25` exact chunks; a hosted desktop
`exactAnchor=viewer-forward` smoke reproduces the optional `(-102, 0)`
anchor. All three report coherent generation 27 coverage and zero missing
tree representations. Interactive hosted placement review remains open.

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
