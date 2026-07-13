# 175: Live Hosted World Diorama

Status: active 2026-07-13. Slices 0–2 landed: contract/baseline,
authored-only server/content foundation, and the static placed-terrain
renderer. Tactical 174's software lifecycle/invalidation closeout is green;
Slice 3 live local scene composition is next. Capable-device multiview remains
a shared named validation gap.

Topic: `embedded-worlds`

Workstream: shared native Rust server/content, scene ownership, and renderer
work. Desktop flat is the first interactive lane. Offscreen flat
and synthetic stereo are the first automated visual lanes; full-frame
multiview follows on a capable desktop-XR or Quest device. This is not a
desktop-app rendering feature.

## Decision

Build the first simultaneous-world experience around one bounded live terrain
preview, not a staged transition:

> While world A remains the ordinary active world, render a bounded region of
> a separately hosted world B as scaled Minecraft geometry on a block-built
> table in A.

World B may be supplied by a local integrated server or an ordinary joined
remote dedicated-server connection. It continues receiving live chunk updates,
compiling dirty terrain, and admitting GPU uploads, but it has no physical
player authority while it is a preview. Both worlds render into the same color
and depth targets with the same physical mono/stereo views. This is geometry
composition, not render-to-texture.

Activation is deliberately simple. A shared blink/fade may cover the existing
atomic whole-slot selection from Tactical 174. Continuous scale-up, falling,
portal apertures, split physics, or two simultaneous player authorities are not
part of this tactical.

The deterministic sample uses persistence-backed authored worlds and an
explicit no-overworld-generation policy. It does not require a complete
Minecraft Superflat port before the first diorama. A reusable vanilla-shaped
flat generator may follow separately if product world creation needs it.

## Milestone

The milestone is complete when all of the following are true:

- An arbitrary active `DrawableWorldSlot` renders through the unchanged direct
  one-world path when no preview is configured.
- One separately hosted drawable slot retains a bounded chunk/section region
  around an explicit source anchor, independently of the physical player's
  chunk-interest center.
- That region appears at a table placement in world A at a uniform miniature
  scale, with correct mono, per-eye, and full-frame multiview projection.
- The physical table and world-A terrain occlude the miniature, and the
  miniature occludes world-A geometry behind it, through the shared depth
  target.
- A server-side block mutation in B becomes a bounded compile/upload update and
  visibly changes the miniature without changing A.
- Opaque, cutout, and the sample's water/translucent terrain have an explicit
  composition order; the sample contains no known cross-world blend-order
  artifact.
- The user can activate B under a short blink, see B as the normal full-scale
  active world on the first uncovered frame, and return to A without rebuilding
  either runtime.
- The same scene-owned preview contract works with a local integrated B and an
  ordinary joined remote dedicated B. Remote observer/subscription protocol is
  not required for this milestone.
- Five-run interleaved no-preview measurements show no significant regression
  from the pre-tactical direct path.

The bounded product shape is:

```text
McloneSceneHost
  physical camera/input/UI
  active DrawableWorldSlot A
    normal authority
    unchanged direct rendering when no preview exists
  optional DrawableWorldSlot B
    local-integrated or remote-dedicated runtime
    fixed preview interest anchor
    background preparation budgets
    no movement/interaction/camera authority
  optional EmbeddedWorldPreview
    B source region
    B-local -> A-composition placement
    placed-render readiness
    activation target
```

This tactical keeps exactly one active slot and one optional preview slot. It
does not introduce an N-world registry.

## Experience Contract

The first checked-in fixture is intentionally small and deterministic:

- **World A:** a persistence-backed authored grass platform with a recognizable
  block-built table near its accepted safe spawn.
- **World B:** a separate persistence-backed authored grass/stone island. Slice
  5 adds the water/ocean presentation after opaque composition is correct.
- **Missing chunks:** deterministic void/air through an `AuthoredOnly` world
  generation profile. Storage misses must not invoke overworld generation.
- **Preview region:** one center chunk plus a small section-aligned horizontal
  radius and explicit vertical section range. A void neighbor ring makes the
  fixture boundary naturally closed before arbitrary-world boundary treatment
  lands.
- **Placement:** source surface anchor mapped to the table-top anchor at a
  uniform positive scale, provisionally `1/16`. Source coordinates are rebased
  around the source anchor before scaling so a distant source is not translated
  through one large absolute `f32` matrix.
- **Sky:** only A's sky is drawn. B terrain keeps its own packed block/sky light
  and live sky-darken value; it does not draw a second sky dome.
- **Authority:** only A receives locomotion, interactions, camera correction,
  HUD targeting, actors, particles, audio, Far LOD, or underwater state. B is
  terrain-only until activation.
- **Activation:** use/ray activation of a scene-owned diorama volume begins one
  shared blink/fade and invokes the already-proven complete-slot exchange. The
  destination resumes its authored active cadence; a local source may receive
  the configured standby cadence. Remote cadence remains server-owned.
- **Return:** the fixture supplies a paired placement/activation point so the
  old active can be shown as the new preview and selected back. Inventories,
  health, entities, and persistence remain world-local.

The fixture is a sample and validation artifact, not a requirement that every
world be a lobby or flat world. Once the placement path is proven, A and B may
both be ordinary overworlds.

## Explicit Non-Goals

- No render-to-texture preview.
- No continuous shrink, expansion, falling, or scale-sweep transition.
- No portal aperture, stencil target, arbitrary clip plane, or `x=0` split.
- No sealed/capped cut through arbitrary B terrain. The first fixture uses a
  void neighbor ring and opaque table rim; general boundary-aware preview
  meshing remains a follow-up.
- No simultaneous collision, block interaction, or entity authority in B.
- No inventory/health/velocity transfer between unrelated servers.
- No preview actors, particles, selection outlines, world UI, audio, Far LOD,
  dynamic shadows, or cross-world light spill.
- No federated remote authority or new remote observer protocol.
- No product world-browser UI, configurable table editor, saved portal block,
  or general N-world registry.
- No complete Superflat structures/features/lakes parity as a prerequisite.
- No sharing of the duplicate atlas/pipeline shell until measured memory makes
  that refactor independently worthwhile.

## Current Evidence And Gaps

### Already Landed

- Tactical 174 gives `McloneSceneHost` one direct active
  `DrawableWorldSlot`, one optional detached slot, stable
  `WorldInstanceId`s, independent runtime/camera/draw/upload/traversal state,
  budgeted standby preparation, exact GPU/topology readiness, and atomic
  A-to-B-to-A selection.
- The detached slot remains live and applies ordinary section-delta updates;
  only its drawing and player authority are suppressed.
- `SceneSessionRuntime` and `NativeSceneServices` already abstract local
  integrated versus remote dedicated runtime mechanics for one slot.
- `TexturedSectionDrawResources` already separates opaque/cutout and
  translucent phases and implements mono/per-eye plus full-frame multiview.
- Terrain vertices carry packed block/sky light. Drawing B in the same depth
  target provides cross-world opaque occlusion without a new depth system.
- The current launch gate proves a host-scoped visual can drive complete-slot
  activation without reconstructing either runtime.

### Missing Today

- Persistence misses in `ChunkScheduler` flow into the overworld feature
  mailbox. There is no world-generation/content profile and no stored-only or
  authored-only miss policy.
- `LocalWorldCreateOptions`, `LocalWorldSummary`, integrated-runner config, and
  dedicated-server startup carry a seed but no generation profile.
- The current worldgen mailbox request is overworld-specific. An authored-only
  profile must bypass that work without weakening the normal overworld status,
  lighting, publication, and persistence contracts.
- Terrain shaders multiply absolute mesh positions by view-projection only.
  They have no placed-terrain uniform or composition-space fog position.
- Terrain culling, graph traversal, translucent sorting, and distance facts are
  expressed in the slot's local world coordinates. A preview needs an
  inverse-placed local camera and composition-space ordering.
- Mono, per-eye, and multiview frame orchestration submits only
  `active_world.draw`. There is no composition submission that can retain the
  existing active-only fast path.
- The warm standby launch request constructs only a second local integrated
  runtime from a seed and deliberately clears `world_dir`. Scene-owned detached
  startup has no persistent-local or target-neutral local/open/remote
  request/completion seam.
- The current warm-world launch presentation couples a retained slot to an
  opaque `WorldGate`. A diorama must select a mutually exclusive presentation
  without constructing or drawing the blue gate.
- Selecting an arbitrary section-aligned subset can expose faces that the
  canonical mesher omitted against neighbors outside the preview. The authored
  fixture avoids this with a void ring/table rim. This tactical does not claim
  a clean hard-edged cut through arbitrary hosted terrain.
- Translucent section sorting is per draw store. A water diorama needs
  composition-space ordering across A and B or a narrowly documented fixture
  constraint.

## Architecture Contracts

### World Generation Profile

Add a shared, serializable server-owned contract, provisionally:

```rust
enum WorldGenerationProfile {
    Overworld,
    AuthoredOnly {
        missing_chunk: AuthoredMissingChunk,
    },
}

enum AuthoredMissingChunk {
    Void,
}
```

`Overworld` is the backward-compatible default for every existing world,
platform, CLI, catalog entry, and dedicated server. `AuthoredOnly` still uses
the normal `WorldStore`: a present record wins exactly as today. On a miss, the
scheduler publishes a deterministic empty feature chunk through its normal
status/light/client-visibility path instead of enqueueing an overworld job.
Runtime mutations and saves remain persistence-backed.

Do not implement authored content by making a `WorldStore` pretend every miss
was a stored record. Persistence owns saved facts; the generation profile owns
the missing-chunk rule.

The additive catalog field must decode old metadata as `Overworld`. Decide and
test whether the existing catalog schema version remains compatible or needs a
single migration; do not silently strand existing worlds. Integrated runner,
dedicated server, browser worker startup, and catalog/session startup must all
receive the same shared profile.

If a general flat-world product option opens later, start from Java 1.17.1
`FlatLevelSource` and `FlatLevelGeneratorSettings`: fixed biome source,
layer-list fill, base height/column, and optional decoration/lake/structure
settings. Do not label a one-off fixture fill as vanilla Superflat.

### Authored Fixture Builder

Add a shared test/diagnostic builder that emits valid `ChunkRecord`s into a
chosen `WorldStore`. It must:

- build deterministic section/block data for the grass platform, table,
  island, void neighbor ring, safe spawn, and later water;
- preserve the normal chunk revision/status/light contract rather than
  injecting renderer-only meshes;
- write through the same SQLite store used by persistent local/dedicated
  worlds;
- be idempotent and refuse to overwrite an unrelated non-fixture world;
- record a fixture schema/id so reruns can rebuild deliberately;
- expose exact authored block positions used by the mutation and visual tests.

Keep the builder below app crates. A small native diagnostic command may call
it to create `/tmp` fixture roots for interactive runs.

### Placement

Add a target-neutral placement contract in `mclone-render`,
provisionally:

```rust
struct WorldPlacement {
    source_anchor: Vec3d,
    composition_anchor: Vec3d,
    uniform_scale: f64,
}

struct EmbeddedChunkRegion {
    center: ChunkPos,
    horizontal_radius: u32,
    min_section_y: i32,
    max_section_y: i32,
}
```

The first tactical permits only a finite positive uniform scale and
translation. Rotation and non-uniform scale wait until a concrete experience
requires them. The mapping is:

```text
composition_position = composition_anchor
                     + uniform_scale * (source_position - source_anchor)
```

CPU placement math and source/composition anchors remain `f64`; upload only the
rebased values the shader needs. This must remain compatible with Tactical
156's future section-local/camera-relative rendering. Do not make far-coordinate
precision worse or bake a second incompatible origin system into meshes.

For culling, derive a B-local camera by applying the inverse placement to the
physical composition camera. The clip transform used to test B-local section
bounds must match the exact shader transform. Chunk interest remains fixed on
`EmbeddedChunkRegion.center`; walking around the table must not move B's server
view.

### Separate Placed Terrain Pipeline

Preserve the direct active shader and uniform layout. Add a placed-terrain
variant used only when a preview is visible:

- normal single-view/per-eye and full-frame multiview variants land together;
- vertex position and exported fog position are transformed into composition
  space;
- each eye retains its own physical view/projection and camera position;
- packed B lighting remains B-local; B fog distance is composition-space;
- identity placement is characterized but is not inserted into the ordinary
  active draw path;
- placed pipelines are materialized before preview readiness so the first
  visible frame cannot compile lazily;
- the first slice may retain B's duplicate renderer shell and atlas. Sharing
  immutable assets is a later measured optimization.

Do not add a model multiplication to every ordinary-world vertex merely to
support the opt-in preview.

### Scene Composition

`mclone-scene` owns the optional preview because it owns physical views, world
slot lifecycle/readiness, frame phases, background budgets, and authority.
`mclone-render` owns placement-aware terrain preparation and encoding.
Platform apps own only diagnostic CLI/query projection and target/session glue.

The no-preview branch must continue calling the existing full-frame functions
with `active_world.draw` directly. The preview branch may build a small
composition submission over exactly A and B. Do not add world-id hashing to
the active draw store or make every section lookup aggregate across slots.

Preview state is separate from slot identity:

```rust
struct EmbeddedWorldPreview {
    source_world: WorldInstanceId,
    region: EmbeddedChunkRegion,
    placement: WorldPlacement,
    phase: EmbeddedPreviewPhase,
    activation: EmbeddedActivation,
}
```

`source_world` must name the retained non-active slot. Asset epochs must match
before publication. Failure, cancellation, asset replacement, device rebuild,
or runtime correction closes/hides the preview instead of drawing mixed or
stale state.

Retained-world ownership and its presentation must be separate. The launch
diagnostic chooses exactly one of the existing opaque gate or the new diorama;
the diorama path allocates no `OpaqueWorldGateRenderer` and the gate path
allocates no placed-terrain presentation.

### Authority And Activation

While B is a preview:

- only A receives physical movement, interaction commands, or camera
  reconciliation;
- B drains initial position correction and keeps its own accepted pose;
- B receives a fixed `SetChunkView` around the preview source center and pumps
  updates under background budgets;
- a remote B is an ordinary joined client for this milestone, so its remote
  server owns cadence and persistence;
- preview ray/volume activation is a scene action, not a block action forwarded
  into B.

Activation reuses Tactical 174's complete-slot exchange. A short shared
screen-effect blink may cover the selection frame, but the destination must
already be drawable. The first uncovered frame must contain full-scale B with
no runtime construction or bulk upload. Paired fixture placements retarget the
old A as B's preview for the return proof.

## Critical Invariants

1. **No-preview stays direct.** No collection, placed uniform write, preview
   cull, extra render pass, second runtime, or per-section world-id lookup runs
   without an explicit preview request.
2. **One physical authority.** The preview never reconciles the physical
   camera or consumes gameplay interaction.
3. **One coordinate contract per draw.** CPU culling and GPU placement use the
   same source anchor, scale, composition anchor, and physical view.
4. **Stereo world selection is atomic.** Both eyes draw the same A/B set and
   activate on the same scene frame.
5. **Shared depth is preserved.** Do not allocate a preview color/depth texture
   or sample it back.
6. **Bounded source work.** Preview interest, compilation, upload, and draw
   counts remain bounded by its explicit region and budgets.
7. **Persistence precedes generation.** Stored authored chunks load normally;
   only true misses consult `WorldGenerationProfile`.
8. **No mixed assets.** A visible preview and active world use the same asset
   epoch/catalog until multi-pack composition is designed separately.
9. **Activation is already warm.** Blink is presentation, not permission to
   hide runtime construction or an upload storm.
10. **Lifecycle owns threads and GPU state.** Cancelling or replacing B joins
    its runtime/compiler work and drops its placed resources through the slot.

## Dependency On Tactical 174

Server generation-profile and fixture work can begin immediately. Before Slice
3 makes B visible, Tactical 174 must have green tests for:

- local retained-world persistence flush on app pause/exit;
- cancellation, replacement, and shutdown without leaked runner/compiler
  threads;
- asset replacement while standby warms;
- surface/resource rebuild and device-loss policy;
- activation flag isolation and no-request performance.

The capable-device full-frame multiview receipt may be shared with this
tactical if both retained-world terrain and placed-terrain pipelines can be
validated in one device session.

Prerequisite closeout record — 2026-07-13:

- app background flushes both retained slot runtimes, while remote persistence
  remains a service-owned no-op;
- cancellation takes and drops the complete standby before updating
  diagnostics, so its runner/compiler/GPU store cannot outlive the slot;
- asset replacement cancels the old-epoch standby and closes its gate before
  committing a new active epoch, proven by a combined rendered smoke;
- render-resource/device rebuild cancels the standby before constructing
  replacement-device resources; live migration is not implied;
- the persistent dual-host test now edits, flushes, reopens, and isolates both
  roots; and
- the no-request one-world performance and pixel receipts remain those already
  accepted in Slices 0–2. The Mac's missing `MULTIVIEW` feature is a device
  receipt gap, not an unresolved lifetime policy.

## Slice 0: Contract Locks And Baselines

Characterize before changing world generation or rendering.

Deliverables:

- Add source/behavior locks proving the current no-preview scene has one active
  draw submission and no placement/composition state.
- Capture desktop flat and synthetic-stereo active-only reference pixels for
  the deterministic seed/pose used by Tactical 174.
- Run five settled release frame-budget samples after confirming the host is
  idle; preserve work counts and use the same outlier/stability rule as
  Tactical 174.
- Record the current terrain uniform size, shader vertex transform, mono,
  per-eye, and multiview phase call sites, renderer-shell allocation, and
  translucent sort ownership.
- Lock persistence-first behavior: a stored chunk bypasses worldgen and a true
  miss enters the overworld feature mailbox under the default profile.
- Finalize the `WorldPlacement`, `EmbeddedChunkRegion`, preview phase, and
  authority contracts above before code moves.
- Record compatibility with Tactical 156: placement rebases around a source
  anchor and does not require remeshing absolute positions merely to transform
  them.

Exit criteria: exact existing behavior and performance anchors are available,
and every new field has an agreed shared owner.

No manual user review is expected yet.

### Slice 0 completion record — 2026-07-13

The pre-change contracts are now executable rather than prose-only:

- `mclone-scene`'s ownership characterization proves the ordinary mono,
  prepared per-eye, and full-frame multiview paths prepare and submit only
  `active_world.draw`; they contain no standby draw, embedded-preview, or
  placement state.
- `mclone-render` locks the direct terrain layout at 128 bytes per view and 256
  bytes for two multiview records. Both direct shaders still compute clip
  position as `view_projection * vec4(input.position, 1)` and export the
  unplaced input position for fog. The test rejects placement anchors, scale,
  or model-matrix fields in either ordinary shader.
- The mono phase call remains
  `render_full_frame_for_view_with_far_lod_and_opaque_gate`. Prepared per-eye
  rendering builds one active-store stereo union and calls the existing
  prepared full-frame entry once per eye. Full-frame multiview calls
  `render_prepared_multiview_stereo_draw_phase_with_options` on the active
  store, splitting opaque and translucent only around actors/the opaque gate.
- `TexturedSectionDrawResources::new` still allocates one
  `TexturedChunkRenderer`, its ordinary pipelines/uniforms, a complete uploaded
  `GpuChunkTextureAtlas`, and one slot-local section/cache store. Multiview
  pipeline materialization remains lazy and slot-local. Slice 2 therefore adds
  a separate placed variant without perturbing this shell.
- Translucent ownership is still per draw store: mono sorts that store's
  visible `RenderSectionKey`s against its local view; prepared stereo sorts one
  union using the eye-midpoint/averaged-forward view; multiview consumes that
  same prepared order. There is no cross-store composition order yet.
- A new scheduler behavior test preloads all nine required light-status chunks
  and proves interest publishes the center with zero status jobs and zero
  worldgen-mailbox work. The existing true-miss characterization still proves
  the default path submits one running feature job after persistence misses.

`WorldPlacement`, `EmbeddedChunkRegion`, preview ownership/authority, and the
Tactical 156 precision constraint are locked by this document: anchors and
placement math remain `f64`, source positions are rebased before scaling, and
ordinary meshes/uniforms do not acquire a second global-origin convention.

Fresh visually inspected active-only receipts are:

- `/tmp/mclone-desktop-offscreen.png`: 2560x1600, 64 resident sections, 11
  drawn sections, SHA-256
  `cdffd673bce6af5a14f2862f0667ca1f49aafa0fc8ada6df1f737b54db75e7e9`;
- `/tmp/mclone-xr-emulation.png`: 1280x640 side-by-side, 640x640 per eye,
  42 resident/8 drawn sections, 249,679 differing eye pixels, SHA-256
  `8061f25548a64ac1df64a937cf12b38dd5420290c35d0116b0890ba17406650d`.

The performance run followed Tactical 174's 10% within-batch stability rule.
Before the accepted attempt, two `top` samples reported 85.6% and 89.8% CPU
idle and no Cargo/Rust/C/C++ build or client process was active. Two preliminary
five-run batches remain under `/tmp/mclone-live-diorama-slice0-perf`; they were
rejected rather than curated because average/P95 spreads were 14.9%/12.5% and
29.6%/27.2%. The latter also contained two repeatable-contention runs with
over-budget frames.

The final direct-binary series retained six runs under the `settled` directory.
Its first lead-in sample (2.802 ms average, 4.648 ms P95) is recorded but was
excluded before inspecting run 6 because the following four P95 values formed
a separate 4.089–4.267 ms cluster. Accepted runs 2–6 measured averages of
2.668, 2.659, 2.563, 2.537, and 2.779 ms: 2.659 ms median and 9.1% range. P95s
were 4.252, 4.267, 4.137, 4.089, and 4.328 ms: 4.252 ms median and 5.6% range.
All five retained 1,936 initial sections and 240 frames with zero over-budget
frames and zero accounting violations. Those medians are 5.1% and 3.5% faster
than Tactical 174's contemporaneous no-standby 2.803/4.404 ms checkpoint, so
Slice 0 establishes no regression but claims no optimization.

Focused validation passes the direct shader-layout test, both stored-hit and
default-miss scheduler tests, the active-only frame ownership contract,
`native:desktop-offscreen:smoke`, `native:xr-emulation:smoke`, formatting, and
diff whitespace checks. Slice 0 changes no production behavior and requires no
manual review.

## Slice 1: Authored-Only Worlds And Persistent Fixtures

Give the sample honest server worlds without falling into overworld generation.

Deliverables:

- Add `WorldGenerationProfile::Overworld` and
  `WorldGenerationProfile::AuthoredOnly { missing_chunk: Void }` in
  `mclone-server`.
- Thread the profile through scheduler, integrated server, native runner,
  dedicated-server construction, shared startup options, local-world creation,
  catalog metadata, and browser worker job/startup codecs where applicable.
- Decode every existing world/config with `Overworld`; prove no default or
  generated seed behavior changes.
- On an authored-only storage miss, produce a deterministic empty feature chunk
  and continue through the ordinary lighting, publication, mutation, save,
  unload, and reload paths. Do not spawn an overworld feature job.
- Add the shared fixture builder and create persistent A/B roots under `/tmp`
  for smokes: grass/table A and grass/stone-island B with void padding.
- Prove safe spawn, chunk status/readiness, lighting, persistence, mutation,
  restart, and root isolation for both fixtures.
- Add a small fixture command/wrapper that is usable by automated and manual
  desktop runs without checking generated databases into the repo.
- Document the deliberate flat-world decision: authored layers satisfy this
  milestone; a general `FlatLevelSource` port remains separate.

Exit criteria: both fixture worlds start through ordinary local and dedicated
server paths, load authored chunks from SQLite, produce void on missing chunks,
and never enqueue overworld terrain work.

The user may inspect the two worlds independently here, but this is not yet the
important visual checkpoint.

### Slice 1 completion record — 2026-07-13

The server/content foundation is now production-shaped rather than a renderer
fixture:

- `WorldGenerationProfile` is a shared serializable `mclone-server` contract.
  `Overworld` remains the default; `AuthoredOnly { missingChunk: Void }` must be
  selected before any holder or job exists. Persistence remains first. A true
  authored miss creates a canonical empty `Features` snapshot, then uses the
  ordinary lighting, publication, visibility, mutation, cache-save, unload,
  and reload paths without creating an overworld feature job.
- Authored misses are admitted in bounded light-batch-sized groups. They count
  as pending work only for an authored scheduler, preventing an early idle
  report without changing the established Overworld `pending_jobs` metric.
- The profile is threaded through native runner and scene startup, dedicated
  server CLI, catalog create/summary metadata, session storage intent, shared
  argv/query parsing, and browser Worker/IndexedDB startup. Catalog schema 1 is
  still compatible: an absent field decodes as `Overworld` on native and web.
- Seed-derived spawn-center search is now explicitly an Overworld policy.
  Authored startup keeps its configured entry center, then lets the ordinary
  first-view safe-surface correction find the exact persisted pose. This fixed
  the first real offscreen launch, which otherwise followed seed `17501` to an
  unrelated biome-source chunk containing valid authored void.
- The shared non-WASM fixture builder emits valid SQLite `Light` records, not
  renderer meshes. It owns schema/id markers, refuses unrelated nonempty roots,
  and safely rebuilds only a matching fixture. Each root contains a center
  authored chunk plus a radius-three lit void ring (49 records), exact spawn,
  preview anchor, and mutation coordinates.
- Fixture `table-a` is a grass platform with a brick table; `island-b` is a
  separate grass/stone island. The diagnostic command writes them to
  `/tmp/mclone-live-diorama-fixtures/{table-a,island-b}` by default:

```bash
pnpm native:authored-world-fixtures
```

They can be inspected independently before composition exists:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --world-dir /tmp/mclone-live-diorama-fixtures/table-a \
  --generation-profile authored-only --seed 17501 --chunk-x 0 --chunk-z 0 \
  --render-distance 2 --debug-passive-showcase false --lighting false

cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --world-dir /tmp/mclone-live-diorama-fixtures/island-b \
  --generation-profile authored-only --seed 17502 --chunk-x 0 --chunk-z 0 \
  --render-distance 2 --debug-passive-showcase false --lighting false
```

The proof covers both ordinary host shapes. A native local startup at render
distance two loads the table through the threaded SQLite runner and produces a
drawable startup seed even with runtime lighting disabled. A real loopback TCP
dedicated session opens the island root, receives its non-air center and exact
safe spawn, and reports zero worldgen publication. Server tests also prove both
fixtures' lit status, root isolation, runtime mutation/restart persistence, and
a far missing chunk becoming lit void with zero worldgen work.

Visually inspected 960x540 receipts are:

- `/tmp/mclone-authored-table-slice1.png`: two resident/drawn sections,
  SHA-256
  `1abe82cee0d6a4d10ef16671412a663b94eddc543eafe222e6e86fa22ebade67`;
- `/tmp/mclone-authored-island-slice1.png`: two resident/drawn sections,
  SHA-256
  `975662114fdf760868fb61ef2991b1f6d8ac033e7f414508664c8f26060bea9a`.

The first five-run no-preview release batch was rejected rather than curated:
an Android emulator began using about 70% of one core during it, producing
16.9% average and 31.2% P95 spread. Before the accepted retry, `top` reported
97.61% CPU idle with no Cargo, Rust, C/C++, or mclone process active and the
emulator had settled/exited. Accepted runs 6–10 under
`/tmp/mclone-live-diorama-slice1-perf` measured averages of 2.523, 2.538,
2.510, 2.509, and 2.436 ms: 2.510 ms median and 4.1% range. P95s were 4.436,
4.331, 4.412, 4.320, and 4.141 ms: 4.331 ms median and 6.8% range. All retained
1,936 initial sections and had zero over-budget frames and zero accounting
violations. Against Slice 0's 2.659/4.252 ms medians, average is 5.6% lower and
P95 is 1.9% higher; this establishes no significant no-preview regression and
claims no optimization.

Focused and full validation passes 385 server, 243 app-runtime, 111
render-session, 132 render (two device tests ignored), 107 scene plus nine
ownership-contract tests (one device characterization ignored), 154 native
client, and 20 dedicated-server tests. Browser WASM build plus TypeScript
typecheck, thin-adapter purity, formatting, and diff whitespace checks pass.
The current web build retains only pre-existing target-specific warnings.

Slice 1 requires no manual checkpoint. Slice 2 may now build static placed
terrain. Tactical 174's lifecycle/invalidation closeout remains required before
Slice 3 exposes the live retained slot.

## Slice 2: Placed Terrain Renderer

Prove transformed real terrain against one shared depth target without a live
second scene slot.

Deliverables:

- Add `WorldPlacement` and placement-derived local view/cull helpers in
  `mclone-render` with inverse/round-trip, frustum, scale, invalid-value,
  and source-rebase tests.
- Add separate placed solid and cutout terrain pipelines in `mclone-render`;
  reserve the matching translucent layout for Slice 5. The ordinary pipeline
  and uniform bytes remain unchanged.
- Implement placed mono/per-eye and full-frame multiview uniforms and shaders
  together. Export composition-space position for fog and preserve per-eye
  physical camera facts.
- Add a bounded region filter to prepared terrain records. Region tests cover
  negative chunks, vertical section limits, source-anchor offsets, and stable
  deterministic ordering.
- Add a renderer-only offscreen composition fixture with ordinary A geometry,
  placed B geometry, and one depth target. Prove A occludes B below the table
  surface and B occludes farther A geometry.
- Add synthetic-stereo assertions: both eyes draw the same B region, pixels
  differ by expected parallax, and no eye reuses the other's mutable uniform.
- Materialize the placed multiview pipeline explicitly and run it on a capable
  adapter when available. Lack of `MULTIVIEW` on the Mac remains a named device
  gap, not permission for a per-eye-only implementation.
- Capture and inspect the first drawable mono and stereo placed-terrain images
  under `/tmp` before proceeding.

Exit criteria: a static second terrain store appears as physical scaled
geometry with correct depth and stereo, while active-only pixels and release
timing remain within the Slice 0 gate.

No live runtime is composed yet.

### Slice 2 completion record — 2026-07-13

The static geometry-composition seam is now real while the active path remains
unchanged:

- `mclone-render::placement` owns validated `WorldPlacement` and
  `EmbeddedChunkRegion` contracts. Placement accepts only finite anchors and a
  finite positive uniform scale, keeps mapping/inverse math in `f64`, and
  exposes the inverse source-local camera facts used by traversal. Region
  bounds are inclusive, section-aligned, overflow-safe around negative chunk
  coordinates, and filter prepared records through their existing stable
  `BTreeMap` order.
- Placed culling applies the same rebased `f32` operation order as the shader:
  `composition_anchor + (source - source_anchor) * scale`. It does not rely on
  one large absolute model translation, including at 30-million-block source
  anchors. The physical projection and each eye's camera facts remain
  composition-local.
- `PlacedTexturedSectionRenderer` is an opt-in shell created from a particular
  terrain store. Ordinary `TexturedSectionDrawResources::new` allocates no
  placed renderer, uniforms, or pipelines. The direct terrain uniforms remain
  exactly 128 bytes per view / 256 bytes multiview and both ordinary shaders
  remain byte-for-byte on their unplaced vertex transform.
- Placed mono/per-eye records are 160 bytes and placed full-frame multiview is
  320 bytes. Separate solid and cutout pipelines transform clip and fog
  positions into composition space while keeping source packed-light facts.
  Per-eye slots retain distinct physical matrices/camera positions. The
  full-frame shader and pipeline path landed with an explicit eager
  materialization method for preview readiness; translucent placement remains
  reserved for Slice 5.
- The renderer-only GPU fixture builds independent A and B terrain stores,
  bounds B to one source chunk/section, and draws A followed by placed B into
  one reversed-Z depth target. It compares A-only, B-only, and composed pixels:
  6,004 pixels prove the table hides B below its surface, while 13,340 pixels
  prove B hides the farther A wall. The placed draw reports exactly one source
  section.
- The same fixture submits both eyes through distinct live uniform slots.
  Both eyes contain the same green B region; 10,383 pixels differ and the B
  centroids move from x=325.77 to x=313.24. The current Mac adapter does not
  expose `wgpu::Features::MULTIVIEW`, so capable-device execution remains a
  named receipt gap rather than a missing code path.

The first placed-terrain images were visually inspected:

- `/tmp/mclone-live-diorama-slice2-mono.png`: 960x640, SHA-256
  `d9cf8664ee4eb3484a55b9d147a38c759f038ae30a924be02aa502a3c558b80b`;
- `/tmp/mclone-live-diorama-slice2-stereo.png`: 1280x640 side-by-side,
  SHA-256
  `08b7a8ea2af086875d92bd5fa3773e99dcba2c450f7d74ad09757db8b87f0ae0`.

Active-only flat and synthetic-stereo smokes still produce the exact Slice 0
hashes `cdffd673...e7e9` and `8061f255...06650d`; the new placed state is not
constructed by either path. A preliminary performance batch was rejected when
its first two averages already spanned 2.650–3.167 ms. Before retry, `top`
reported 98.10% CPU idle and no Cargo, compiler, mclone, emulator, or QEMU
process. The accepted five-run direct-binary batch measured averages 2.548,
2.663, 2.547, 2.546, and 2.465 ms: 2.547 ms median and 7.8% range. P95s were
4.501, 4.445, 4.467, 4.380, and 4.180 ms: 4.445 ms median and 7.2% range. All
five retained 1,936 sections with zero over-budget frames and zero accounting
violations. Against Slice 0's 2.659/4.252 ms medians, average is 4.2% lower and
P95 is 4.5% higher; both remain within the 10% gate and no optimization is
claimed.

The full native workspace test/doc-test suite passes, including 136 ordinary
render tests plus the ignored-on-CI placed GPU proof. Browser WASM build,
TypeScript typecheck, thin-adapter and scene-host purity, formatting, and diff
whitespace checks pass; only pre-existing target-specific WASM warnings remain.
No live second slot is drawn yet, so this slice requires no manual review.

## Slice 3: Scene-Owned Live Local Diorama — First Review Checkpoint

Make Tactical 174's retained local slot visible as opaque/cutout terrain.

Deliverables:

- Add one optional `EmbeddedWorldPreview` owned by `McloneSceneHost`; it names
  the retained slot and cannot exist without it.
- Decouple retained-slot startup/readiness from launch presentation. The CLI
  fixture selects either `OpaqueGate` or `Diorama`, never both, and the diorama
  path constructs no blue gate model/renderer.
- Extend readiness so B becomes visible only after source-region coverage,
  traversal state, asset epoch, and placed mono/per-eye/multiview topology are
  ready.
- Keep the no-preview frame functions direct. Add separate composition branches
  for mono, per-eye stereo, and full-frame multiview.
- Submit frame phases as: A sky/Far LOD, A opaque/cutout, B placed
  opaque/cutout, A actors/gate/opaque visuals, then the existing later phases.
  B contributes no sky, actor, outline, overlay, UI, audio, or Far LOD work.
- Use B's fixed source region/interest anchor. Physical camera movement around
  the table changes only its placed render view and culling.
- Add launch-only diagnostic options for local persistent A/B fixture roots,
  source region, table placement, and preview scale. Absence of the option
  allocates no preview state or placed renderer.
- Extend the local retained-slot request to open B's persistent fixture root
  with its stored generation profile. Do not send that detached open through
  the primary session coordinator or replace A.
- Add `pnpm native:live-diorama:smoke` and a manually launchable desktop command.
- Capture A alone, A with B, left/right stereo, table-front, table-side, and
  table-behind occlusion views. Record pixel differences and drawn B
  section/index counts in a JSON report under `/tmp`.
- Keep B throttled when configured and prove preview drawing does not restore
  active simulation cadence.

Exit criteria: the user can launch the sample, walk around the table in A, and
see a live-hosted but initially static opaque/cutout B miniature with convincing
stereo and shared-depth occlusion.

This is the first required manual review checkpoint. Stop for feedback on
scale, table placement, source framing, culling, lighting, and XR presence
before adding mutation, water, or activation.

## Slice 4: Live Updates, Bounds, And Background Budgets

Prove the miniature is a live world rather than a retained snapshot.

Deliverables:

- Make preview update preparation explicit in frame accounting: runtime poll,
  compile submission/acceptance, upload, placed cull, and placed draw each
  receive separate B counters/timing.
- Keep B's server interest fixed on the source center and its compile priority
  based on the B-local inverse camera/source region, never A's raw coordinates.
- Apply hard background caps so B cannot consume A's last upload/acceptance
  grant or create an unbounded compile backlog.
- Add a deterministic authoritative mutation in B after the first preview
  capture. Prove the delta reaches only B, rebuilds/uploads only the affected
  preview sections, changes the expected table pixels, persists, and survives
  B restart.
- Prove the placed draw never submits a section outside `EmbeddedChunkRegion`
  and the server subscription remains within its configured view. The accepted
  authored fixture uses a void neighbor ring plus opaque table rim so canonical
  section meshes form a visually closed miniature.
- Add an explicit diagnostic when a non-authored source uses a hard region edge
  that may expose canonical neighbor-culled faces. General preview-only cap
  meshes remain named follow-up work rather than silently entering this
  milestone.
- Expose concise debug facts: source world/id/host mode, region, scale,
  readiness, B poll/compile/upload/draw counts, CPU/GPU retained bytes, cadence,
  last mutation, and failure reason.
- Add movement-around-table and ten-minute idle/mutation soak smokes. Queue
  depths and resident B section counts must remain bounded.

Exit criteria: a visible server mutation proves end-to-end live hosted terrain,
region boundaries meet the documented contract, and background work remains
bounded without stalling A.

## Slice 5: Water And Cross-World Phase Ordering

Upgrade the fixture from a dry opaque island to the intended grass/ocean
sample.

Deliverables:

- Author B's water/ocean blocks through the same persistent fixture builder;
  do not inject a renderer-only water plane.
- Define one composition-level terrain submission shape that can order A and B
  without turning either draw store into a world registry.
- Draw all opaque/cutout terrain before active actors and all translucent
  terrain afterward, preserving reversed-Z depth loads/stores across passes.
- Sort translucent sections back-to-front in composition space across A and B,
  qualifying aggregated keys at the scene boundary with `WorldInstanceId`.
  `mclone-render` may receive a neutral submission id/ordinal but must not
  depend on scene ownership types. Keep each slot's internal section maps keyed
  by plain `RenderSectionKey`.
- Implement equivalent mono, per-eye, and full-frame multiview ordering. Stereo
  sort uses the eye midpoint/union policy consistently, never one eye's mutable
  state for both.
- Add fixture views with A glass/water both in front of and behind B water.
  Capture and inspect them; record any deliberately unsupported intersecting
  translucent case explicitly.
- Keep underwater detection and overlay driven solely by active A; the camera
  cannot become underwater in miniature B while B lacks authority.

Exit criteria: the small island/ocean miniature renders with stable water and
no known ordering defect in the accepted fixture matrix.

## Slice 6: Blink Activation And Return

Use the existing warm selection as a simple product interaction.

Deliverables:

- Add a shared scene-owned diorama activation volume/ray target. Flat mouse/use
  and XR controller use resolve through neutral scene input; do not add a
  desktop-only gameplay key or send block interaction into B.
- Require B's full-scale entry coverage and canonical renderer topology in
  addition to preview readiness before activation is offered.
- Reuse the shared screen-effect fade/blink renderer for both eyes. Define a
  short close/select/open state machine; no geometry scale animation is added.
- Invoke Tactical 174's whole-slot exchange inside the fully covered interval.
  The first uncovered frame must draw full-scale B, with zero switch-boundary
  construction, compile submission/acceptance, or upload.
- Retarget the old A as B's preview using the paired fixture placement. Preserve
  B-local/A-local cameras and independent persistence; only the destination
  physical camera mapping and authority change.
- Restore authored active cadence on a selected local destination and apply the
  optional standby cadence to the demoted local source. Remote cadence changes
  remain unsupported and must report that honestly.
- Add deterministic flat and synthetic-stereo A-preview-B → blink → B-preview-A
  → blink → A smokes. Both eyes must select on the same frame and neither
  uncovered frame may be blank.
- Record blink duration, atomic switch cost, first uncovered draw counts, and
  boundary work in schema-versioned JSON.

Exit criteria: the user can inspect B on the table, activate it through a short
blink, arrive in ordinary full-scale B, inspect A on the paired table, and
return without a loading hitch.

## Slice 7: Remote Hosted Preview

Prove the drawable source is transport-neutral.

Deliverables:

- Generalize detached slot startup around a target-neutral source request:
  create/open local persistent world, local transient fixture, or join remote.
  Reuse `SessionStartRequest`/`SessionRuntimeKind` classification where
  possible, but do not route a detached start through the primary session
  coordinator and accidentally replace A.
- Add a slot-targeted asynchronous start/complete/fail seam usable by native
  runtime factories and later browser Worker/WebSocket adapters. The aggregate
  installed into B must match the same `DrawableWorldSlotInstall` contract as
  local startup.
- Join a native dedicated server hosting fixture B as an ordinary second
  client, acknowledge its spawn correction, set its fixed preview chunk view,
  and keep its inbound update pump alive without sending physical movement.
- Prove remote disconnect/reconnect/failure hides or closes the preview while A
  stays active and interactive. No remote runtime may correct A's camera.
- Prove a dedicated-server mutation streams to the visible miniature and uses
  the same placed compile/upload path as local B.
- Add `pnpm native:live-diorama:remote-smoke` that launches/owns a bounded
  dedicated fixture server, captures live mutation and activation evidence,
  and shuts every process down.
- Keep a real observer/subscription protocol out of scope. Record that an
  ordinary joined preview consumes a player slot and has a server-side player
  identity.

Exit criteria: local integrated and native remote dedicated B sources produce
the same scene/render behavior and failure isolation.

## Slice 8: Lifecycle, Performance, And Platform Closeout

Close the bounded milestone without hiding ordinary-play or XR costs.

Deliverables:

- Test pause/background/exit persistence flush for every retained local world;
  remote persistence remains server-owned.
- Test preview cancellation, source replacement, local/remote failure, asset
  replacement, surface/resource rebuild, device loss, resize/render-scale
  rebuild, and app exit. No thread, connection, compiler job, placed pipeline,
  or GPU buffer may outlive B.
- On asset-epoch change, close/cancel B before committing A's new epoch unless
  both slots can be rebuilt transactionally. Prove no mixed atlas/catalog draw.
- Measure no-preview versus pre-tactical Slice 0 with five correctly
  interleaved settled release pairs. Investigate a median average/P95 slowdown
  above 5%; do not accept above 10% without an explicit user decision.
- Measure preview-on frame CPU, GPU time where available, draw calls, section
  counts, placed cull cost, global translucent sort cost, uploads, compiler
  contention, RSS, GPU terrain/atlas bytes, and default/throttled idle CPU.
- Prove preview count/region/budgets cannot grow from one bounded source into an
  accidental N-world workload.
- Run desktop flat, synthetic stereo, a real per-eye desktop-XR lane, and a
  capable full-frame multiview device lane. Capture and inspect representative
  images under `/tmp`.
- Keep web/WASM compiling through shared contracts. If production browser
  adoption is deferred, add an exact reason-bearing feature-ledger entry rather
  than a silent unsupported path.
- Update `docs/topics/embedded-worlds.md`,
  `docs/native-engine-architecture.md`, platform parity records, tactical index,
  and durable performance records with landed evidence and remaining actor,
  interaction, arbitrary-boundary, registry, and transition gaps.

Exit criteria: the local and remote live-diorama milestone is lifecycle-safe,
measured, XR-correct, and has no significant no-preview regression.

## Validation Matrix

Every slice runs its focused crate tests plus:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
pnpm native:thin-adapters:purity
pnpm native:web:build
git diff --check
```

Pixel-producing slices add and inspect:

```bash
pnpm native:desktop-offscreen:smoke
pnpm native:xr-emulation:smoke
pnpm native:live-diorama:smoke
pnpm native:live-diorama:stereo-smoke
```

Activation/remote slices add:

```bash
pnpm native:live-diorama:activation-smoke
pnpm native:live-diorama:remote-smoke
```

Performance checkpoints use already-built release binaries, verify the host is
mostly idle with no concurrent compiler/emulator/test work, run at least five
samples, reject unstable batches, and compare interleaved base/candidate pairs:

```bash
pnpm native:frame-budget:perf
pnpm native:live-diorama:perf
```

All screenshots, JSON reports, fixture databases, and dedicated-server logs go
under `/tmp`, never into the repository or `test-results/`.

## First Manual Review

The first meaningful user checkpoint is the end of Slice 3, not the fixture or
isolated renderer foundations. The manual run must allow walking around the
table in ordinary desktop flat mode and should expose debug facts for:

- A/B ids, seeds/profile, host modes, and readiness;
- source region and anchors;
- table anchor and scale;
- placed solid/cutout sections and indices;
- B cadence and background queues;
- mono/stereo/multiview placed-pipeline readiness.

Review scale, source framing, table size, lighting, edge presentation,
occlusion, and stereo comfort before Slice 4 locks the live-update and boundary
contracts.

## Expected End State

By the end of Tactical 175, the engine can show a bounded region from a real
second local or remote hosted world as live Minecraft terrain sitting on a
table inside the active world. The preview updates when its server changes,
shares physical stereo/depth with the room, and can be selected under a simple
blink through the already-warm whole-slot handoff.

The engine still does not have preview actors/interactions, arbitrary portal
clipping, a general registry, cross-world shadows, a remote observer protocol,
or a staged shrink/fall transition. Those remain later experiences built on
the proven live geometry-composition primitive rather than requirements for it.
