# Tactical 261: Procedural Horizon Product Integration Roadmap

Status: active coordinating parent, opened 2026-07-26. The research,
standalone Explorer, cross-host parity, transition hardening, reusable
vegetation-service, and shared composition-substrate phases are complete.
Child Tactical
[`262`](262-world-explorer-exact-procedural-composition.md) reached Human
Review 1 with native exact/procedural World Explorer pixels. Review accepted
the terrain compositor and collar overall but found a blocking whole-tree
ownership defect. A focused exact/proxy frontier correction now precedes
browser and full-game rendering.

Topics:

- `procedural-horizon-clipmap`
- `gpu-procedural-terrain`
- `lod-native-vegetation`
- `world-view-navigation`
- `platform-parity`

## Why This Parent Exists

The procedural horizon is no longer one experimental renderer. It is a large
product feature assembled from terrain semantics, bounded multiscale
residency, exact-chunk arbitration, vegetation, scene lifecycle, platform
execution, and mono/stereo/multiview rendering.

The proof campaign intentionally landed those concerns in small tacticals.
That made each change reviewable, but the complete feature is now difficult to
see from any one execution record. This parent is the global answer to:

1. Why was the old in-game LOD system removed?
2. Which replacement foundations are already proven?
3. What still separates World Explorer from the real game?
4. Which tactical owns the next code change?
5. When may desktop, browser, Android, desktop XR, or Quest claim support?
6. Which attractive follow-ups are outside the first product boundary?

Child tacticals own detailed design, implementation, commits, and evidence.
This parent owns feature-wide ordering, status, review gates, and final
cross-platform closure. It does not become a second implementation record.

## Product Promise

The completed feature should let an ordinary game scene render:

- exact block terrain, edits, and exact vegetation near the observer;
- a continuous, bounded procedural natural horizon outside exact coverage;
- atomic transitions between those representations without holes, duplicate
  terrain, forest-free frames, cracks, or visible one-frame LOD fallback;
- the same world/source identity through desktop, browser, flat Android,
  desktop OpenXR, and Android XR / Quest;
- one scene-owned frame decision shared by mono, per-eye, synthetic stereo,
  and full-frame multiview rendering; and
- a protected exact-only mode whose behavior and cost do not regress when the
  procedural horizon is disabled.

This is real game-engine integration. World Explorer remains a small proof,
profiling, and acceptance host; it is not the final gameplay owner.

## One-Page Feature State

```text
semantic natural world
  mclone-worldgen terrain fields + forest intent + stable tree records
                              |
                              v
bounded procedural products and execution
  mclone-terrain-view clipmap + admission + vegetation coordinator
                              |
                              v
shared composition                    platform execution
  exact-painted snapshot, mask,       native thread | browser Worker
  target, frontier, draw order                  |
                \                              /
                 v                            v
render lifecycle and pipelines
  mclone-render-session + mclone-render
  mono | synthetic stereo | per-eye | multiview
                              |
                              v
desktop | web game | flat Android | desktop XR | Quest
```

The top, bottom, and right-hand proof boundaries exist. Tactical 262 now
proves the missing reusable center with a canonical exact near field and the
procedural horizon on one World Explorer target. After that proof,
`mclone-scene` will produce the same exact-painted snapshot from real client
render sections and own product lifecycle.

## Source-Of-Truth Rule

Use this parent for macro order and feature completion. Use
[`procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md) for
the living terrain-composition contract and the focused topic documents for
their own concerns.

When an older completed tactical says “next”:

- this parent controls current ordering;
- the older tactical remains a historical execution record;
- a completed tactical is not reopened for adjacent integration work;
- each implementation child receives its own numbered tactical when it
  becomes active; and
- child numbers need not be contiguous because unrelated workstreams may
  advance between them.

## Historical Ledger

### 1. Evidence before the replacement

Tacticals
[`227`](227-web-terrain-lab-vertical-slice.md) through
[`242`](242-terrain-lab-worker-canonical-meshing.md) established the Terrain
Lab evidence base: production procedural fields, CPU/GPU comparison,
viewport-scale sampling, exact canonical chunks, broad footprints, shared
navigation, cache behavior, and Worker-backed canonical meshing.

Tacticals
[`243`](243-lod-native-vegetation-exact.md) and
[`244`](244-lod-native-vegetation-presentation.md) established forest intent,
stable natural tree records, exact realization, and the initial multiscale
vegetation presentation. Tactical 244's Terrain Lab hierarchy-quality proof is
now complete. These semantics are shared source facts rather than a
renderer-owned forest approximation.

### 2. Rejected architecture removed

Tactical
[`245`](245-retire-chunk-far-lod-runtime.md) removed the previous in-game
chunk-granular Far LOD. That system reduced samples within a chunk but still
created area-proportional chunk identities, work, arbitration, and residency.
It could not become the intended multi-kilometer or XR architecture.

The removal boundary remains binding. Do not restore its tile identities,
coverage coordinator, renderer, settings, workers, or compatibility types
while integrating the replacement.

### 3. Shared proof hosts and controls

Tactical
[`247`](247-standalone-world-explorer-foundation.md) created the small native
World Explorer and shared view controller. Tactical
[`248`](248-terrain-lab-navigation-and-worker-modernization.md) moved Terrain
Lab navigation and browser-worker ownership onto reusable Rust boundaries.

These hosts proved product scope and platform are independent: a small
Explorer and the full game can both run on desktop or the web without either
platform owning terrain policy.

### 4. Fixed-budget toroidal horizon

Tactical
[`249`](249-cross-platform-procedural-horizon-proof.md) implemented the shared
ten-level toroidal clipmap and rendered it through native and browser World
Explorer. Tactical
[`250`](250-continuous-explorer-presentation-and-cadence.md) separated
continuous camera presentation from snapped residency and unified
frame-rate-independent held navigation.

Tactical
[`251`](251-lod-surface-appearance-quality.md) established shared material
classification and appearance quality without making Terrain Lab or one
platform the runtime owner.

### 5. Transition and seam hardening

Tactical
[`252`](252-procedural-horizon-seams-and-transition-admission.md) fixed the
desktop-and-browser forest flicker reported during interactive review. It
added requested/staged/committed terrain and vegetation presentations,
retained complete same-source coverage during asynchronous replacement, and
welded tile and fine/coarse lighting footprints with bounded scalar normal
halos.

The proof now retains complete terrain and forest through ordinary movement
without increasing the per-frame dispatch budget.

### 6. Cross-host parity and reusable execution

Parent Tactical
[`253`](253-world-explorer-cross-host-parity.md) sequenced three focused
children:

| Tactical | Completed result |
|---|---|
| [`254`](254-ui-less-world-explorer-host.md) | Browser host contains platform mechanics rather than product diagnostics or terrain policy. |
| [`255`](255-world-explorer-color-output-parity.md) | One explicit display-space contract preserves the selected appearance across UNORM and sRGB targets. |
| [`256`](256-shared-horizon-vegetation-worker-topology.md) | One terrain-vegetation coordinator uses a native thread or isolated browser Worker with matching semantic receipts. |

The resulting coordinator and compiler session live in shared crates. World
Explorer is their first consumer, not their owner. This is the construction
seam the real scene must now consume.

### 7. Parallel semantic-source improvement

Later world-generation tacticals, including the active Mclone macro-landscape
and coast work, improve the shared semantic source used by exact chunks and
the horizon. They are parallel input-quality work, not prerequisites for
scene composition unless a child tactical identifies a concrete source
contract gap.

Scene integration must not freeze current terrain aesthetics, and terrain
quality work must not invent a second LOD or platform execution policy.

## What Is Proven Today

- Fixed-capacity toroidal residency across ten nested levels.
- Negative coordinates, axial and diagonal movement, long walks, and
  teleports.
- Continuous presentation independent of snapped update origins.
- Bounded requested/staged/committed terrain admission.
- Complete same-source vegetation retention during asynchronous replacement.
- Source-reset and stale-completion rejection.
- Shared scalar normal halos and fine/coarse transition footprints.
- Stable tree-record identity and bounded near-level vegetation compilation.
- Native-thread and browser-Worker execution behind one shared coordinator.
- Matching native/offscreen/desktop-browser/phone-browser semantic receipts.
- Caller-owned render targets, reversed-Z depth, and explicit display-space
  color behavior.
- A renderer-neutral exact-painted snapshot with source/generation identity,
  negative-coordinate mask packing, and stale-result rejection.
- One shared canonical compiler session and packed mesh codec consumed by
  Terrain Lab and the native World Explorer.
- A bounded threaded exact producer with one-result-per-frame admission and
  overlap retention during movement.
- Native `Horizon`, `Exact`, `Composed`, and `Coverage` modes on one
  color/depth target with a checked GPU mask and explicit procedural collar.
- A deliberately small World Explorer dependency boundary.

## What Is Not Yet Proven

- The procedural horizon has not rendered inside the ordinary
  `mclone-scene::McloneSceneHost`.
- Production exact chunk draws and procedural coverage do not yet come from
  one immutable game-scene frame snapshot.
- The ordinary game does not yet publish or consume the proven exact-painted
  coverage mask.
- The proof's explicit procedural collar is not yet a production game
  frontier, and visible exact/horizon surface and water differences remain.
- Exact trees and procedural proxies are not yet atomically XORed across
  cross-chunk crowns.
- Authoritative edits do not yet invalidate nearby natural proxy ownership.
- Full-game world replacement, active/standby worlds, device rebuild, and
  feature budgets do not yet own the horizon lifecycle.
- The full browser game and flat Android game have not consumed the service.
- Synthetic stereo, desktop OpenXR, per-eye Quest, and full-frame multiview
  have not rendered the procedural horizon through the game scene.
- Device-specific budgets and sustained locomotion costs are not yet accepted
  on phone or Quest.

## Binding Ownership

- `mclone-worldgen` owns terrain semantics, source identity,
  footprint-aware evaluation, forest intent, stable tree records, compiler
  sessions, and bounded semantic caches.
- `mclone-terrain-view` owns clipmap geometry, toroidal addressing, procedural
  products, requested/staged/committed admission, stale policy, vegetation
  coordination, the renderer-neutral exact-painted snapshot/mask contract,
  and prepared procedural draws.
- `mclone-render` owns procedural terrain, material, water, fog, vegetation,
  depth, mono/per-eye, and multiview pipelines.
- `mclone-render-session` owns GPU resources, masks, pending and committed
  origins, uploads, device rebuild, and render admission.
- proof hosts may publish a bounded exact-painted snapshot from a local
  canonical source without owning product policy;
- `mclone-scene` ultimately owns exact/procedural arbitration, the immutable
  product frame snapshot, proxy/exact XOR, world switching, locomotion
  anchors, cross-feature budgets, and feature enablement.
- Platform adapters own only window/canvas/activity/OpenXR cadence, physical
  targets, native threads or browser Workers, raw input translation, and
  presentation.

No desktop, browser, Android, or XR app may own a private horizon policy.
`mclone-scene` must not depend on `winit`, browser APIs, Android activity
types, or OpenXR.

## Target Frame Contract

One composition frame snapshot must decide all of:

1. the exact chunks whose complete opaque/cutout resources are drawable now;
2. the exact-painted coverage mask derived from that same set;
3. the committed procedural terrain presentation and source identity;
4. the committed procedural vegetation presentation;
5. exact/proxy vegetation ownership for every intersecting footprint;
6. render order, views, targets, and feature budgets; and
7. whether the exact-only protected path is selected.

“Loaded,” “generated,” “resident,” or “compiling” does not mean
`exact-painted`. Procedural coverage may be removed only when the replacing
exact resources participate in the current snapshot.

The intended opaque order is:

1. sky and background;
2. masked procedural terrain;
3. exact opaque and cutout terrain under the same depth convention;
4. actors plus exactly one exact-or-proxy vegetation representation; and
5. translucent terrain and water.

Depth testing alone is not arbitration. Approximate terrain can sit above
exact terrain, hide edits, or z-fight. The coverage mask owns visibility; a
narrow collar or skirt owns geometric disagreement at the frontier.

## Ordered Child Campaign

The child identifiers below are stable roadmap labels, not reserved tactical
numbers. Create one numbered tactical at a time when its entry contract is
ready.

| Child | State | Scope | Exit gate |
|---|---|---|---|
| **PH-1 Shared composition substrate** | **complete in Tactical 262** | Add the immutable exact-painted snapshot, source identity, bounded GPU mask, caller-owned color/depth target, explicit load/store behavior, and renderer-neutral draw ordering without importing a product scene. | Unit/offscreen tests prove coherent coverage, negative-coordinate packing, stale rejection, shared depth, and unchanged Horizon mode. |
| **PH-2 World Explorer composition proof** | **Human Review 1 correction active in Tactical 262** | Extract the useful canonical exact-view source from Terrain Lab, then add `Horizon`, `Exact`, `Composed`, and `Coverage` modes over one World Explorer device/target. Correct the reviewed exact/proxy tree chimera through complete-record ownership before promotion. | Native movement, delayed admission, eviction, negative, and teleport receipts plus inspected pixels reach Human Review 1A with exactly one complete representation per natural tree; browser and Terrain Lab promotion follow after review. |
| **PH-3 Terrain Lab adoption** | waiting on PH-2 review | Add the runtime horizon/composed presentation as another consumer while preserving canonical, CPU LOD, fast CPU LOD, and GPU LOD research panes. | Fixed-anchor Lab comparison exercises the same compositor without moving policy into React or browser canvas code. |
| **PH-4 Full-game scene adoption** | waiting on PH-2 acceptance | Have `mclone-scene` publish the same exact-painted facts from real client render sections, construct platform executors at the app rim, and render the ordinary native game with exact terrain near and procedural terrain beyond. | Native game window/offscreen movement proves the accepted compositor under real scene lifecycle while exact-only stays equivalent. |
| **PH-5 Vegetation and edit arbitration** | waiting on PH-4; reusable untouched-tree primitive starts in PH-2 | Consume the proven whole-record XOR in the game scene, then reject stale products and invalidate nearby natural proxy ownership when authoritative exact edits change the replacement facts. | No duplicate or disappearing trees at the frontier, including cross-chunk crowns, delayed exact compilation, edits, movement, and source switches. |
| **PH-6 Lifecycle, recovery, and budgets** | waiting on PH-4/PH-5 | Integrate world/session replacement, active and bounded standby worlds, device loss/rebuild, dynamic exact-radius changes, cross-feature admission, diagnostics, and stable locomotion budgets. | Rebuild and world-switch smokes recover coarse-first without stale draws or unbounded work; exact-only remains protected. |
| **PH-7 Flat-platform promotion** | waiting on stable native scene | Wire the same scene contracts through the full browser game and flat Android. Construct native-thread or browser-Worker executors only at platform rims; do not copy Explorer policy. Establish device-specific memory/work defaults from evidence. | Desktop web, representative phone browser, and flat Android render matching source/coverage identities and inspected pixels under movement and replacement. |
| **PH-8 Stereo and XR promotion** | waiting on PH-4 and stable lifecycle | Feed the same scene snapshot and committed horizon through synthetic stereo, desktop OpenXR, Quest per-eye rendering, and full-frame multiview. Share residency, compilation, masks, and admission across views; only view/projection/targets vary. | Headset-free stereo, desktop XR, Android XR, and capable-device multiview receipts show correct per-view geometry, no one-eye omissions, bounded work independent of view count where appropriate, and accepted headset pixels/performance. |
| **PH-9 Product view and handoff** | waiting on credible game composition | Reuse the same composition for the player-facing Explorer/map/tabletop direction, accessible navigation, URLs or source recipes, and validated “Enter Here” authority handoff. Preserve the option to load/navigate before attempting seamless GPU residency transfer. | A player can inspect a world broadly and enter an authoritative safe location without the preview claiming authority it does not own. |
| **PH-10 Quality and scale closeout** | parallel after PH-2 measurements | Tune footprint summaries, coast/river/water transitions, fog, lighting handoff, forest silhouettes, horizon distance, and device tiers. Add an adaptive comparator only for a measured question. | Review fixtures across coast, mountain, water, forest, and large-coordinate cases meet selected desktop/phone/Quest visual and performance budgets. |

PH-1 through PH-8 constitute the first all-target in-game procedural-horizon
campaign. PH-9 is product use of the capability. PH-10 may supply bounded
quality children before or after a platform promotion when evidence identifies
a specific defect, but it must not block composition on speculative polish.

## Immediate Correction

Tactical
[`262`](262-world-explorer-exact-procedural-composition.md) has completed
**PH-1: Shared Composition Substrate** and implemented **PH-2: World Explorer
Composition Proof** through its first native review.

The terrain compositor and collar passed that review overall. The next work
is Tactical 262 Slice 3A: make exact natural-tree geometry and its stable
record-derived proxy one whole-record XOR decision against the actual
exact-safe terrain interior. The current exact tree is admitted wholesale,
the procedural collar can depth-occlude it, and the proxy is clipped by
fragment position. That combination can show pieces of both representations
for one logical tree.

After the same forest anchors pass Human Review 1A, Tactical 262 may continue
with the browser proof and optional Terrain Lab adoption. A separate child
then owns `mclone-scene` integration; the proof boundary prevents that child
from combining scene lifecycle, GPU masking, and first-pixel discovery in one
cut.

## XR And Multiview Invariants

The procedural horizon is view-independent world geometry. For one scene
frame:

- clipmap residency and vegetation desired coverage are computed once;
- compilation, result polling, admission, and mask publication run once;
- every eye or multiview layer reads the same committed source and coverage
  generation;
- each view retains its own view/projection and target facts;
- no mutable per-eye uniform is overwritten before one submission completes;
- full-frame multiview must receive every horizon draw visible in per-eye
  rendering; and
- foveation or view-local culling may reduce drawing only after the shared
  coverage decision, never by creating a different world representation per
  eye.

Synthetic stereo is the first topology gate. It does not replace real desktop
OpenXR and Quest validation.

## Human Review Gates

Automation owns deterministic identity, coverage, bounded work, stale
rejection, and platform mechanics. Human review is required where pixels,
comfort, or product meaning cannot be reduced to those receipts.

### Review 1: first true World Explorer composition

After PH-2 first renders:

- toggle Horizon, Exact, Composed, and Coverage at the same camera;
- move and zoom across the exact/procedural frontier;
- inspect coast, steep terrain, water, and forest cases;
- look for cracks, z-fighting, duplicate surfaces, material popping, and
  camera-like one-frame transitions; and
- compare horizon enabled and exact-only behavior.

Do not begin game-scene adoption until this review.

Result on 2026-07-26: terrain and collar accepted overall; vegetation
ownership correction required. One stable natural tree can be shown as exact
geometry partly depth-occluded by procedural collar terrain plus the surviving
outside fragments of its LOD proxy. Tactical 262 Slice 3A must replace that
base-chunk/fragment behavior with complete-bound, stable-ID ownership and
repeat the forest cases as Human Review 1A.

### Review 2: first real-game composition

After PH-4, repeat the accepted Explorer scenarios under the ordinary game
runtime, real exact render-section readiness, movement, and view-distance
changes.

### Review 3: vegetation and lifecycle

After PH-5/PH-6:

- cross the frontier through dense forest and clearings;
- edit blocks and natural trees near the frontier;
- teleport, change worlds or seeds, and trigger delayed replacement;
- inspect device/surface rebuild recovery; and
- confirm old-source terrain or vegetation never survives a source switch.

### Review 4: flat-platform parity

After PH-7, compare equivalent native, desktop-browser, phone-browser, and
flat-Android views. Pixel identity is not required across physical displays,
but geometry, source identity, coverage, tree ownership, and display-space
intent must agree.

### Review 5: XR acceptance

After PH-8 synthetic and automated gates:

- inspect desktop OpenXR and Quest in-headset;
- turn and translate through the frontier at ordinary locomotion speeds;
- inspect near trees against the distant horizon in both eyes;
- look for eye-specific omissions, stereo depth disagreement, shimmer, or
  transition discomfort; and
- evaluate sustained frame pacing and thermal behavior on Quest.

This is the gate for claiming the feature is usable in VR, not merely that the
XR crates compile.

### Review 6: product completion

After PH-9/PH-10, judge horizon scale, recognizable geography, navigation,
enter-world expectations, and whether the selected device tiers feel like one
coherent feature rather than a debug renderer.

## Automated Acceptance Matrix

| Surface | Required evidence before support is claimed |
|---|---|
| Native offscreen | Deterministic color/depth capture, exact/procedural mask receipt, movement, admission/eviction, negative, teleport, source reset, exact-only comparison. |
| Native desktop game | Interactive pixels plus bounded frame/update diagnostics during sustained locomotion. |
| Full browser game | Wasm build, domain-blind Worker transport, headed Wayland desktop/phone WebGPU captures, matching semantic receipts, lifecycle and overflow recovery. |
| Flat Android | Scripted APK build/validation, lifecycle recovery, representative device or AVD pixels where the lane can prove them, and bounded memory/work receipt. |
| Synthetic stereo | Both views contain the horizon and exact frontier with distinct projections over one shared committed presentation. |
| Desktop OpenXR | Real frame-loop rendering, per-eye correctness, session stop/restart behavior, and headset inspection. |
| Android XR / Quest | Scripted package validation, per-eye and full-frame multiview where supported, controller/head motion, headset pixels, frame pacing, memory, and thermal evidence. |

Every rendered child captures and inspects pixels at its first drawable
milestone. A ready queue, successful build, or color-only screenshot is not
coverage evidence.

## Performance And Resource Questions

Each child must report the resources and work it adds instead of treating the
current Explorer budget as universally free:

- fixed and transient GPU bytes by terrain, normal, mask, and vegetation
  resource;
- CPU and Worker cache bounds;
- dispatch, upload, poll, and draw admission per frame;
- exact render traversal added by composition;
- mask update area and frequency;
- cold start, movement, teleport, world switch, and device rebuild latency;
- view-count sensitivity for mono, stereo, per-eye, and multiview;
- browser/phone memory and long-frame behavior; and
- Quest frame pacing, memory pressure, and thermal behavior.

Device tiers may select different level counts, distances, or work budgets.
They may not select different terrain semantics, ownership, stale rules, or
exact/procedural truth.

## First Product Boundary

The first in-game horizon represents untouched natural terrain outside exact
coverage.

- Exact drawable chunks show their authoritative blocks and edits.
- When edited chunks leave exact range, the natural procedural base may
  return.
- Nearby authoritative edits must not leave duplicate natural proxies while
  exact coverage is active.
- Distant player-built structure summaries, persistent edit pyramids, and
  arbitrary volumetric LOD are later systems.

This boundary delivers broad natural-world scale without making PH-4 depend on
a persistent planet-wide edit database.

## Explicitly Later Or Separate

- Persistent distant representations of arbitrary player edits.
- Distant structure, settlement, actor, or vehicle proxies.
- Hidden-seed remote-server descriptors and curated preview protocols.
- Seamless GPU/device/residency transfer from standalone Explorer to game.
- Adaptive quadtree or hybrid residency without a measured clipmap problem.
- Volumetric caves, overhangs, or cubic distant residency.
- A final universal 100–200 km configuration across every device.
- Advanced water reflection, atmosphere, cloud, or lighting systems that have
  their own focused topics.
- Player-facing Explorer menus, tabletop authority, and remote “Enter Here”
  policy before PH-9.

These may become child campaigns later, but they do not invalidate the first
natural-horizon completion.

## Parent Completion Criteria

This parent closes only when:

- PH-1 through PH-8 are complete or an explicit newer product decision removes
  a target from scope;
- the ordinary game uses one `mclone-scene` exact/procedural frame contract;
- exact terrain, procedural terrain, and vegetation have exclusive,
  flicker-free ownership during movement and asynchronous replacement;
- world/source switches and device rebuild reject stale work;
- exact-only mode remains behaviorally and operationally protected;
- native, browser, flat Android, synthetic stereo, desktop OpenXR, Quest
  per-eye, and full-frame multiview evidence is recorded where supported;
- real headset review accepts stereo correctness and frame behavior;
- fixed/transient resource and work budgets are documented by device tier;
- living topic, architecture, platform, and validation docs describe the
  landed ownership rather than the proof-host architecture; and
- every deferred product or representation gap is named without reopening
  completed children.

## Related Living Documents

- [`procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  — current composition and residency contract.
- [`gpu-procedural-terrain.md`](../topics/gpu-procedural-terrain.md) —
  evaluator, Terrain Lab evidence, and terrain quality direction.
- [`lod-native-vegetation.md`](../topics/lod-native-vegetation.md) — forest
  intent, records, summaries, compiler service, and exact/proxy ownership.
- [`world-view-navigation.md`](../topics/world-view-navigation.md) — Explorer,
  map, tabletop, and enter-world navigation.
- [`far-lod.md`](../topics/far-lod.md) — rejected architecture and removal
  boundary.
- [`platform-parity.md`](../topics/platform-parity.md) — target-equivalent
  ownership and validation.
- [`performance.md`](../topics/performance.md) — profiling and regression
  budgets.
- [`../native-engine-architecture.md`](../native-engine-architecture.md) —
  shared scene, renderer, platform, and XR ownership.
- [`../platforms.md`](../platforms.md) — current target posture and executable
  validation commands.
