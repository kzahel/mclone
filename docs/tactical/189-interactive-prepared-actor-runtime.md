# 189: Interactive Prepared Actor Runtime

Status: active 2026-07-17. Slices 0-4 are complete; Slice 5 browser,
persistence, and scale closeout is next.

Topic: `compiled-figure-rendering`

Workstream: shared native Rust assets, protocol, authoritative simulation,
client presentation, scene, and rendering; Asset Lab remains a semantic review
tool. Desktop/offscreen validation first, then production browser WebGPU and
shared stereo contracts.

## Goal

Take the approved prepared player animation architecture into ordinary
interactive gameplay without requiring exact rounded tessellation:

```text
canonical semantic figure JSON
  -> shared startup PreparedFigure with explicit cuboid proxies
  -> immutable geometry and atlas shared by drawable worlds

debug-hotbar chicken/mannequin tool
  -> ordinary authoritative UseItemOn validation
  -> stable entity and presentation identity
  -> interpolated world transform + continuous travel phase
  -> world-local actor record + continuously evaluated part palette
  -> production prepared draw
```

The user can place chickens and player-shaped mannequins. Chickens remain
ordinary authoritative chickens. A mannequin is an explicit non-vanilla debug
entity with player dimensions and appearance; it reuses the same supported
passive goal family as cows so its locomotion and animation are observable
without a second client. Chicken and mannequin rendering use the prepared path
while unsupported actor kinds retain `ActorMeshCache`.

Success is an end-to-end, interactively testable coexistence path with stable
immutable resources and small per-actor mutable state. It is not instancing,
exact curved parity, LOD, or a thousand-chicken optimization yet.

## Binding Decisions

### Asset representation

- Semantic JSON remains the only persisted figure representation. No geometry,
  clip, palette, or compiled sidecar is added.
- `box` preparation remains exact. A solid-color `sphere` becomes a cuboid of
  size `[2r, 2r, 2r]`; `capsule` becomes `[2r, length + 2r, 2r]`; and
  `cylinder` becomes
  `[2 * max(radiusTop, radiusBottom), length,
  2 * max(radiusTop, radiusBottom)]`.
- These bounds match the existing accepted native approximation. Prepared
  diagnostics retain the source primitive kind and count proxy substitutions.
  Review labels say "cuboid proxy" and do not claim Three.js silhouette parity.
- Any texture application on a non-box primitive is rejected. The current
  promoted non-box primitives are solid-colored, so this does not create UV
  authoring work. Exact curved topology remains an optional later quality tier.
- Preparation remains deterministic and budgeted. Player, chicken, and upright
  bear are the promoted proof set even though only chicken and mannequin enter
  production selection in this tactical.

### Interactive debug tools and authority

- The existing debug hotbar becomes a tagged content contract rather than a
  block-ID-only array. Its initial contents include block entries plus chicken
  and mannequin spawn tools. This is a debug fixture, not a substitute for the
  later survival item/inventory registry.
- Spawn-tool assignment and use require negotiated `DEBUG_ACTIONS`. Placement
  travels through ordinary `UseItemOn` targeting and the server's reach,
  world-policy, and collision/bounds checks; the client never creates an
  authoritative actor directly.
- A chicken tool creates the existing `EntityKind::Chicken`, including its
  current passive goals, species state, observation, replication, and
  persistence.
- `EntityKind::Mannequin` is a custom debug entity with player dimensions
  `0.6 x 1.8`, eye height `1.62`, movement speed `0.2`, and no natural spawn-
  table entry. It registers the same supported passive goal family used by the
  cow. It persists under an explicit mclone entity identifier so a placed
  movement fixture survives an integrated-world restart.
- The UI gives actor tools distinct usable labels/icons and allows a replaced
  actor tool to be restored without restarting the session.

### Presentation and rendering

- Stable server/client presentation identity reaches prepared actor records;
  vector position or list index is never used as render identity.
- Presentation interpolation derives travel distance continuously from actual
  smoothed horizontal displacement. It does not copy an authoritative distance
  step onto an interpolated position. Clip evaluation samples the current
  presentation time and phase with no display-frequency ceiling.
- Prepared immutable figure resources are asset/device-epoch scoped and may be
  shared across drawable worlds. Actor transforms, packed light, palettes,
  interpolation, and visibility remain world-local.
- Initial production selectors are chicken and mannequin. Remote/local players,
  cows, debug cubes, and items retain the legacy path in this tactical. A
  preparation or unsupported-feature failure selects the legacy actor fallback
  instead of dropping the actor.
- The per-actor record boundary remains separate from each final part palette
  so later instancing, LOD, and GPU pose expansion can be additive. They are not
  implemented here.
- The production path supports mono, distinct-slot per-eye, and full-frame
  multiview ownership. First-eye/second-eye draws reuse the same current actor
  state; no second-eye pose evaluation or palette upload is permitted.

## Explicit Non-Goals

- No persisted compiled representation or disk cache.
- No exact sphere/capsule/cylinder tessellation or curved-surface UV editor.
- No instanced draw, sampled phase palette, compute pose evaluation, GPU crowd
  path, LOD, billboard, or impostor.
- No full survival item registry, recipes, drops, eggs, spawn eggs, or creative
  inventory parity.
- No mannequin combat, breeding, loot, natural spawn, bespoke AI, skin network
  service, or claim of vanilla entity parity.
- No removal of `ActorMeshCache` or migration of every actor kind.

## Implementation Slices

### Slice 0: decisions and accepted-proof closeout

- Record Tactical 186's human approval and accepted lighting difference.
- Select and document cuboid-proxy policy, interactive entity fixtures,
  authority boundary, coexistence, and validation gates here and in the topic.

Gate: documentation states what is semantic authority, what is an explicit
runtime approximation, and what still requires later quality/performance work.

### Slice 1: deterministic cuboid-proxy preparation (complete 2026-07-17)

- Extend `mclone-assets` preparation to accept current solid-color spheres,
  capsules, and cylinders using the binding bounds policy.
- Retain semantic primitive-kind/proxy diagnostics and reject textured non-box
  inputs with actionable errors.
- Add deterministic counts, bounds, transforms, animation, malformed/budget,
  and promoted chicken/upright-bear fixtures.
- Extend the native comparison/review lane with visibly labeled cuboid-proxy
  animal output and inspect the first drawable pixels.

Gate: player output is unchanged; chicken and upright bear prepare repeatedly
with finite exact accounting; the inspected animal proxy is coherent and is
never mislabeled as Three.js parity.

Gate evidence:

- `PreparedFigure` now records each part as an exact box or a sphere/capsule/
  cylinder cuboid proxy. Its compiler identity is
  `mclone-prepared-figure-cuboid-proxy-v1`, and diagnostics report exact counts
  for all four categories without changing schema-v1 semantic JSON.
- One validated size function implements the selected bounds math for raw
  bounds and emitted cuboid topology. Missing, non-positive, non-finite, and
  unknown primitive parameters fail preparation. A texture or box-face
  override on a non-box primitive fails with an explicit solid-material-only
  diagnostic.
- Player remains 12 exact boxes, 288 vertices, 432 indices, and 72 ranges.
  Chicken prepares deterministically as 21 parts, 504 vertices, 756 indices,
  and 126 ranges: 6 exact boxes plus 11 sphere, 3 capsule, and 1 cylinder
  proxies. Upright bear remains 15 exact boxes. Both promoted animals retain
  their compiled walk clips.
- The generalized comparison command accepts any promoted figure path and
  emits the geometry variant and proxy count in its receipt and visible labels.
  `/tmp/mclone-prepared-chicken-comparison/comparison.png` shows corresponding
  semantic Three.js and prepared cuboid-proxy front/right/three-quarter panels;
  the sheet explicitly labels all 15 approximated chicken parts.
- The sheet and raw mono/stereo images were inspected immediately. The proxy
  chicken is grounded, recognizable, correctly oriented, hierarchically
  coherent, and consistent between stereo eyes. Its intentionally blockier
  breast, neck, head, comb, beak, wattle, and legs are the selected proxy style,
  not an unnoticed parity defect.
- All focused preparation tests, `mclone-figure-review` check, Asset Lab
  typecheck, native GPU review, labeled comparison lane, and portability
  capture pass. The final native receipt reports four immutable uploads and
  three view writes; preparation measured 0.706 ms in this diagnostic run.

### Slice 2: authoritative interactive tools and mannequin (complete 2026-07-17)

- Generalize protocol/server/client/UI debug-hotbar entries to tagged block or
  actor-spawn content while keeping capability gates and serialization bounds.
- Add assignable chicken/mannequin entries with usable UI labels/icons.
- Spawn through server-validated ordinary use-item targeting.
- Add persistent `EntityKind::Mannequin`, player dimensions/appearance,
  observation and replication, and the existing passive-animal goal family.
- Cover unauthorized, invalid target, bounds/collision, persistence, restart,
  and no-natural-spawn behavior.

Gate: an integrated native session can assign/select/place both tools; the
server produces ordinary moving observed actors; restart restores them; a
client without `DEBUG_ACTIONS` can do neither.

Gate evidence:

- Protocol version 27 replaces the block-only debug hotbar slot with a bounded
  tagged `Block`/`SpawnActor` item. Default zero-based slots 7 and 8 contain
  chicken and mannequin tools, and the command codec rejects unknown item,
  actor, and entity tags.
- The shared debug palette exposes `Spawn Chicken` and `Spawn Mannequin` as
  assignable entries with distinct representative icons. The shared UI action,
  experience effect, scene routing, local command, and Web canvas label all
  preserve actor identity rather than converting the tool back to a block.
- `UseItemOn` remains the only placement request. The authoritative realm
  requires negotiated `DEBUG_ACTIONS`, ordinary reach and hit validation, a
  mutable world policy, loaded supporting terrain, clear feet/head blocks, and
  a collision-free entity bounding box before spawning at the adjacent cell
  center. The client does not synthesize an entity.
- Chicken placement uses the existing persistent chicken runtime.
  `EntityKind::Mannequin` uses dimensions `0.6 x 1.8`, eye height `1.62`, speed
  `0.2`, neutral cow species state, and exactly the same supported random
  stroll/look-at-player/random-look goal registration as cow. It has no farm
  animal spawn-table or showcase entry.
- Mannequin entity payloads use the explicit `mclone:mannequin` persistence
  identity. Entity-record packing/hydration and binary persistence tests cover
  chicken, item, and mannequin records, including the restored mannequin's
  three registered passive goals.
- Integrated tests place both default tools and observe authoritative entity
  snapshots with expected positions and dimensions. Dedicated negative tests
  prove missing capability and occupied target rejection; existing shared
  reach, world-policy, and bounds validation remains on the same command path.
- Client, protocol, UI, app-runtime, render-session, and the complete 452-test
  server suite pass after updating the old slot-8 block assumption. The native
  `/tmp/mclone-actor-tools-palette.png` capture was inspected after playable
  startup: the first two assignable cells visibly carry the distinct chicken
  and mannequin representative icons without a loading overlay.
- Until Slice 4, both new actors deliberately use the legacy production mesh
  path. The mannequin already selects the player figure approximation so the
  interactive fixture remains visible while prepared coexistence lands.

### Slice 3: stable presentation and continuous travel phase (complete 2026-07-17)

- Carry stable actor identity into the renderer-neutral actor instance.
- Make movement-derived travel phase follow smoothed presentation displacement
  and remain continuous across target updates, pauses, and correction.
- Keep wing/head/other independent channels composable with walk pose.
- Add exact arbitrary-cadence, reorder, despawn/respawn, drawable-world
  isolation, and no-list-index-identity tests.

Gate: a wandering mannequin and chicken animate continuously between network
updates; no authored-key-rate or display-rate hold exists.

Gate evidence:

- Renderer-neutral `ActorInstanceId` distinguishes local player, remote player,
  and entity identities. The presentation-to-render boundary copies protocol
  IDs explicitly; a prepared actor record never needs a vector index or world
  position as identity.
- Each `DrawableWorldSlot` owns its own `ActorInterpolationState` and last
  presentation instant. Active and retained-preview worlds reconcile and step
  independently, replacement/install resets the state, and one actor list is
  prepared before both eyes consume it.
- Chicken and mannequin walk distance now accumulates only the finite
  horizontal displacement actually presented by the exponential interpolation
  step. Reconciliation does not copy an authoritative distance jump onto a
  partially interpolated position. Paused actors retain phase, while despawn
  followed by a new actor track resets it.
- Chicken wing animation remains an independent derived channel and composes
  with travel phase. Remote-player interpolation uses the same movement-derived
  rule while retaining its existing source-distance field as the initial phase
  seed for compatibility.
- Exact tests cover target reconciliation before a frame step, 60 Hz versus
  500 Hz over the same elapsed second, actor input reordering, unequal per-ID
  travel, pause stability, and despawn/respawn reset. The 500 Hz path produces
  the same presented position and phase within floating-point tolerance; there
  is no authored sampling or display-frequency ceiling.
- All 118 client tests, all 113 render-session tests, the 14-test composable
  world presentation contract, focused legacy actor-mesh tests, and workspace
  check pass. The rebuilt native production actor smoke captured and inspected
  `/tmp/mclone-stable-actor-presentation.png`; all three actors draw coherently,
  including the moving upright-bear remote-player figure at walk distance
  `0.08`.

### Slice 4: production prepared coexistence (complete 2026-07-17)

- Add asset/device-scoped prepared chicken/player resources and world-local
  mutable actor rows/palettes to shared renderer ownership.
- Route chicken and mannequin through the prepared production draw while
  preserving legacy actors in the same frame and graceful per-kind fallback.
- Support mono, distinct-slot stereo, multiview, drawable-world composition,
  asset replacement, and ordinary packed-light/world-transform updates.
- Prove immutable geometry/index/atlas identities and uploads do not change
  when actors move, animate, spawn, despawn, or another actor changes.

Gate: direct and retained-preview worlds show moving prepared and legacy actors
together; only bounded actor/palette data changes per presented frame, and the
second eye performs no duplicate update.

Gate evidence:

- The first-party figure registry now retains startup-prepared player and
  chicken data beside the legacy compiler result. Semantic JSON is read once
  and remains the only persisted input; a preparation failure logs an
  actionable warning and leaves that figure on the legacy path.
- Asset/device-scoped prepared actor resources own one immutable vertex,
  index, and atlas upload per promoted figure. Each drawable world owns only
  stable-ID records with a model/light uniform, a bounded 64-part palette,
  pose scratch, and per-view uniforms. Resource snapshots price immutable and
  world-local allocations independently.
- Stable chicken and mannequin entity IDs select the prepared path. Local and
  remote players, cows, debug cubes, items, anonymous figures, unsupported
  figures, and evaluation failures remain explicit `ActorMeshCache` fallback.
  Both paths draw in the same direct or composed frame.
- Continuous walk distance drives exact clip evaluation on every presented
  update. Chicken wing rotation is an additive actor-local channel composed
  before hierarchy evaluation; focused tests cover valid composition,
  malformed overrides, and finite actor-sized bounds across 241 samples of a
  complete walk/flap cycle.
- Direct, placed, clipped, distinct-slot stereo, and full-frame multiview
  shaders share packed light, fog, world placement, and half-space semantics.
  The second per-eye draw reuses the first eye's actor/model/palette state and
  writes only its distinct view slot.
- The renderer-owned cross-platform composition fixture now contains a
  prepared chicken, prepared mannequin, and legacy local player on one side,
  plus legacy cow/item/player actors on the other. Its native GPU proof reports
  3,070 actor/control pixel differences and 1,901 left/right-eye differences;
  immutable prepared uploads remain six across two drawable worlds, the right
  legacy mesh contains only one actor, and two prepared records draw alongside
  it. `/tmp/mclone-179-slice4-actor-composition-mono.png` and
  `/tmp/mclone-179-slice4-actor-composition-stereo.png` were inspected and are
  coherent. The ordinary production smoke
  `/tmp/mclone-production-prepared-coexistence.png` was also inspected with
  prepared chicken/mannequin-compatible resources and legacy actors together.
- The production browser WebGPU probe runs the identical Rust fixture and
  reports two prepared records, one legacy record, six immutable uploads, two
  pose evaluations/palette writes, two prepared draws, and no page errors.
  `/tmp/mclone-native-web-actor-composition-probe-canvas.png` was inspected;
  its prepared chicken/mannequin and legacy actors agree with native placement,
  orientation, clipping, and cuboid-proxy appearance.
- Full-frame multiview execution remains a capability skip on the current Mac;
  both WGSL variants validate and the renderer fixture will execute and compare
  multiview layers to per-eye output when the adapter exposes `MULTIVIEW`.

### Slice 5: end-to-end and scale evidence

- Add deterministic integrated fixtures for interactive placement, autonomous
  movement, restart, replacement, and mixed prepared/legacy actors.
- Capture and inspect native gameplay plus synthetic stereo pixels, and run the
  production browser WebGPU path with both actor tools.
- Measure representative counts and a thousand-chicken stress fixture on the
  current non-instanced path: CPU pose, mutable bytes, draw count, immutable
  uploads, and memory. This records the baseline and later instancing/GPU-pose
  crossover; it is not required to make one thousand actors fast in this slice.
- Record unsupported full-frame multiview execution as a capability skip only
  after shader/layout/contract checks; execute it where a capable adapter is
  available.

Gate and human checkpoint: provide exact native/browser interaction steps,
screenshots/receipts under `/tmp`, test results, and the measured scale baseline
for review. Pause before starting instancing, LOD, exact curved topology, or GPU
crowd work.

## Validation Matrix

| Gate | Required evidence |
|---|---|
| preparation | deterministic promoted-figure counts/bounds, proxy accounting, textured-non-box rejection |
| authority | debug-capability assignment/use, reach/world/bounds validation, ordinary replication |
| simulation | chicken/mannequin passive goals, no mannequin natural spawn, persistence/restart |
| presentation | stable identity, interpolated displacement phase, arbitrary cadence and correction |
| residency | immutable upload/resource identity stable; bounded actor/palette writes only |
| coexistence | prepared chicken/mannequin plus legacy player/cow/item in direct and retained worlds |
| pixels | first native prepared animal, gameplay, stereo, and browser captures inspected under `/tmp` |
| platform | mono, per-eye, multiview contract, browser WebGPU, replacement/device epoch |
| scale | representative and 1,000-chicken counters recorded without claiming optimization complete |
| hygiene | focused tests/checks, source locks, `git diff --check`, clean intentional commit slices |

## Human Review Target

At the end, the reviewer should be able to:

1. open the debug palette/hotbar and assign chicken or mannequin tools;
2. place multiple actors through the same interaction used for blocks;
3. watch both kinds wander and animate smoothly at arbitrary display cadence;
4. leave and reopen an integrated world and find the placed fixtures restored;
5. compare the labeled prepared chicken/mannequin pixels with the accepted
   cuboid style; and
6. inspect receipts showing stable immutable resources and the current
   non-instanced thousand-chicken baseline.

The next human decision is whether this interactive visual/behavioral/runtime
baseline is good enough to begin instancing and LOD, or whether a figure,
movement, UI, or proxy-quality correction should land first.

## Code And Documentation Map

- Durable direction:
  [`../topics/compiled-figure-rendering.md`](../topics/compiled-figure-rendering.md)
- Approved animation predecessor:
  [`186`](186-prepared-figure-continuous-animation-proof.md)
- Figure preparation:
  [`../../native/crates/mclone-assets/src/prepared_figure.rs`](../../native/crates/mclone-assets/src/prepared_figure.rs)
- Legacy primitive bounds:
  [`../../native/crates/mclone-render/src/asset_lab_figure.rs`](../../native/crates/mclone-render/src/asset_lab_figure.rs)
- Prepared GPU resources:
  [`../../native/crates/mclone-render/src/prepared_figure.rs`](../../native/crates/mclone-render/src/prepared_figure.rs)
- Actor presentation:
  [`../../native/crates/mclone-client/src/actor.rs`](../../native/crates/mclone-client/src/actor.rs)
- Entity goals:
  [`../../native/crates/mclone-server/src/entity/mob/goals/passive.rs`](../../native/crates/mclone-server/src/entity/mob/goals/passive.rs)
- Production actor drawing:
  [`../../native/crates/mclone-render/src/entity.rs`](../../native/crates/mclone-render/src/entity.rs)
