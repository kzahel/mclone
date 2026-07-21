# Tactical 215: Preliminary Couch Readiness

Status: active 2026-07-21.

Topics: [`local-couch-multiplayer`](../topics/local-couch-multiplayer.md) and
[`controller-input`](../topics/controller-input.md).

## Instruction Synthesis

Preserve a credible path to 1-4-player couch multiplayer without shipping local
multiplayer before ordinary gamepad support is ready. Implement the bounded
foundation end to end: make generic render-view identity independent of XR eye
identity, separate shared flat-frame preparation from per-view drawing, prove
two independently posed flat views for one participant, exercise authoritative
interest aggregation at cardinalities 1, 2, and 4, and prove deterministic
source identity/assignment with scripted gamepads.

Keep the work useful in the current product. The first visual proof is a
single-player auxiliary pane suitable for an overhead map, debug camera, or
building plan. Preserve the direct one-view path and do not introduce local
profiles, second logical client endpoints, split-screen menus, or a couch-only
authority.

## Current Seams

- `mclone-render::uniform::PerViewSlot` has generic naming but encodes two
  stereo eyes. Its frame ring uses two slots, and render preparation in terrain
  and actors uses `is_right_eye()` as a reuse signal.
- `McloneSceneHost::render_mono_frame_inner` combines runtime polling, startup,
  upload, Far LOD preparation, lifecycle/UI assembly, per-camera effects,
  drawing, submission, and accounting. Calling it once per pane would advance
  shared work more than once.
- The existing XR per-eye path already prepares once and draws twice, but its
  `XrView`, eye FOV, stereo culling, and compositor semantics must remain
  explicitly XR-specific.
- `RenderFrameTarget` describes a complete color/depth target and has no
  viewport rectangle. The preliminary proof can render separate textures and
  compose an evidence card without forcing viewport state through every pass.
- `mclone-input::GamepadInputAdapter` is one anonymous controller. No product
  host currently supplies ordinary gamepad snapshots.
- `RealmServer` already owns ordinary player and observer interest. The missing
  couch evidence is a focused cardinality/overlap/removal smoke, not a second
  local server.

## Architecture Invariants

1. Shared frame advancement, update drain, upload admission, actor preparation,
   and lifecycle accounting execute once for a multi-view flat frame.
2. Camera pose, projection, frustum/occlusion result, depth, effects, target,
   and view-local rendering remain independent for every admitted view.
3. Generic view indices never imply left/right-eye behavior. `StereoEye` owns
   the XR mapping and any eye-pair reuse decisions are explicit.
4. The one-view public entry point remains the direct fast path and produces no
   auxiliary targets or participant state.
5. An auxiliary camera neither creates a participant nor adds server interest.
6. Scripted input uses the same source/session reducer intended for future
   native, browser, and Android collectors. Platform device handles are not
   persisted or treated as participant identity.
7. One input source may be assigned to at most one local participant; neither
   participant nor assignment order depends on a physical device index.
8. The participant ceiling is four, but no `[T; 2]` flat-view or participant
   model is introduced. Genuine XR eye pairs remain fixed arrays of two.
9. Product policy stays in shared crates. Apps may host offscreen targets and
   future device collectors but may not own participant, layout, or join rules.

## Slice Plan

### Slice 0 — plan, inventory, and source locks

- Record the current two-slot and mono-frame seams in this tactical.
- Add source-contract checks that reject using XR stereo types as couch views,
  app-local participant/layout policy, or repeated mono-frame calls as a
  multi-view implementation.
- Capture the exact affected validation commands before changing behavior.

### Slice 1 — neutral presentation-view identity

- Add a bounded neutral presentation-view index supporting indices `0..4`.
- Keep a typed two-value `StereoEye` and explicit conversion to view indices.
- Allocate generic per-view uniform rings from the maximum presentation-view
  count while retaining three in-flight frame generations.
- Replace `is_right_eye()` preparation shortcuts with explicit preparation
  policy or typed stereo-eye input.
- Prove four distinct per-view uniform values remain live in one submission and
  that every frame-ring generation remains disjoint.
- Audit non-uniform allocations that use the ring count and retain lazy or
  bounded behavior where expansion would create unnecessary product cost.

### Slice 2 — prepare once, render many

- Introduce a shared flat frame plan with one preparation phase, an ordered
  collection of independently posed views/targets, and one finish/accounting
  phase.
- Reuse the ordinary mono render semantics; do not project flat views through
  `XrView`, stereo culling, or OpenXR target contracts.
- Keep `render_mono_scene_frame` as the cardinality-one wrapper.
- Return a receipt that distinguishes shared preparation count from rendered
  view count and per-view summaries.
- Add deterministic tests proving one shared preparation for one, two, and four
  admitted views and rejecting zero or more than four views.

### Slice 3 — single-player auxiliary-view evidence

- Extend the native offscreen scene host with a two-flat-view smoke topology.
- Use the active first-person camera plus an independently posed overhead or
  oblique building-plan camera.
- Render independent color/depth textures, compose vertical and horizontal
  evidence cards, save them under `/tmp`, and inspect both.
- Assert distinct view/projection uniforms and culling summaries while shared
  update/upload preparation remains one.
- Re-run the mono smoke to ensure the auxiliary mode is opt-in and leaves the
  direct path intact.

### Slice 4 — authority and input cardinality smokes

- Add `RealmServer` tests for 1, 2, and 4 ordinary interest sources in one
  dimension: co-located overlap, separated union, source removal, and survival
  of the remaining sources.
- Introduce session-local `InputSourceId`, neutral source descriptors, and a
  normalized standard gamepad snapshot in `mclone-input`.
- Add a bounded source-assignment reducer with explicit join edges, reconnect
  behavior, disconnect clearing, and a four-participant ceiling.
- Drive it with four scripted gamepad sources. Prove stable assignment under
  reordered samples, no duplicate source assignment, no physical-index
  identity, and held-state clearing after disconnect.
- Do not add a product collector or enable couch joining in a shipping host.

### Slice 5 — closeout

- Run formatting, focused crate tests, shared source locks, native headless
  rendered-output validation, and affected wasm checks.
- Reconcile this tactical and both topic docs with landed contracts, receipts,
  inspected captures, and exact remaining product prerequisites.
- Leave physical gamepad collectors and controller-complete UI as the gate for
  beginning participant/profile/client implementation.

## Required Gates

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-input
cargo check --manifest-path native/Cargo.toml -p mclone-scene \
  --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml -p mclone-input \
  --target wasm32-unknown-unknown
```

Use the smallest native offscreen command that exercises the new flat-view
contract. Every generated capture goes to `/tmp` and must be inspected before
the visual slice proceeds.

## Completion Bar

- A neutral four-view uniform canary passes without reinterpreting flat views
  as stereo eyes.
- One flat frame can prepare once and draw two independently posed views.
- Inspected vertical and horizontal auxiliary-view captures are legible and
  materially different between panes.
- The ordinary mono capture and direct frame path remain green.
- Realm interest aggregation passes focused 1/2/4-source overlap, separation,
  and removal tests.
- Four scripted gamepads join deterministically through source IDs and clear on
  disconnect without any physical controller backend.
- Topic docs state exactly what is foundation-ready and what remains blocked on
  real gamepad and participant/client product work.

## Progress Evidence

### Slice 1 — neutral presentation-view identity

Implemented on 2026-07-21. `PresentationViewIndex` now admits four neutral
views, the three-generation uniform ring has twelve disjoint slots, and
`StereoEye` is the only left/right identity. Terrain stereo masks convert only
inside explicitly stereo draw code. Actor preparation reuse is an explicit
prepared-frame operation; generic `PerViewSlot` no longer exposes
`is_right_eye()`.

The ring-size audit found only bounded uniform buffers, small dynamic vertex
buffers, lazy world-panel texture entries, and renderer bookkeeping. No terrain
mesh, atlas, actor geometry, or depth/color target is multiplied by the ring
constant. The focused Rust suites passed, and the ignored GPU canary rendered
red, green, blue, and yellow from four distinct uniform slots in one
submission.

### Slice 2 — prepare once, render many

Implemented on 2026-07-21. `FlatPresentationView` supplies independent flat
targets, depth attachments, render views, and UI policies. The shared scene
entry admits one through four views, delegates cardinality one to the unchanged
mono inner path, and otherwise polls assets, advances startup, drains/uploads,
syncs lifecycle UI, prepares render records, and prepares Far LOD exactly once.
Each admitted view then owns its uniform slot, culling, target/depth, projection,
underwater effect, world overlays, and optional flat UI.

`FlatPresentationFrameSummary` reports one shared preparation and ordered
per-view render receipts. Explicit actor preparation refresh/reuse replaces a
slot-number convention. Source locks prove the multi-view path contains one
shared update sequence, does not use XR view/eye types, and leaves participant
or layout policy out of the desktop driver. Focused scene and app-runtime tests
pass for 1/2/4 admission, invalid cardinality, and explicit stereo actor reuse.

The multi-flat entry currently rejects a visible retained embedded-world
preview rather than silently drawing it incorrectly. Extending composition to
prepared multi-flat records is follow-up work; the direct mono and existing
stereo/multiview composition paths are unchanged.

### Slice 3 — single-player auxiliary-view evidence

Implemented and inspected on 2026-07-21. The existing
`--headless-dual-view` diagnostic now renders the active first-person camera
and a detached oblique plan camera into independent color/depth textures in one
frozen flat frame. It saves the two source views plus horizontal and vertical
composites; the compositor has a deterministic row-order test. The auxiliary
camera is derived from the live primary pose and does not move the player or
add interest.

The accepted 640x360-per-pane seed-12345 proof reported one shared preparation,
two rendered views, 209,870 differing paired pixels, four primary drawn
sections, and twelve auxiliary drawn sections. The visible boundary in the
plan pane is expected evidence that an auxiliary camera only sees resident
facts. Both composites and the ordinary mono control were inspected:

- `/tmp/mclone-couch-foundation-20260721/horizontal.png`
- `/tmp/mclone-couch-foundation-20260721/vertical.png`
- `/tmp/mclone-couch-foundation-20260721/mono-control.png`

The mono control remained a normal full-frame capture with eleven drawn
sections, two drawn actors, and no auxiliary allocation or presentation call.

### Slice 4a — authoritative interest cardinality

Implemented on 2026-07-21. A focused `RealmServer` test grows one ordinary
player source into two co-located sources and then four mixed co-located and
separated player/observer sources. Co-location retains one aggregate ticket;
the separated sources expand the union to three resident chunks. Removing each
source drops only its now-unreferenced ticket while the other centers survive,
and removing all four returns the aggregate and scheduler ticket counts to
zero. The focused server test passes without rendering or couch-only authority.

## Explicit Deferrals

- Native, web, Android, OpenXR, or Steam Input gamepad collectors.
- Multiple local profiles, guest persistence, or platform-account association.
- Multiple logical local client endpoints and owner-private stream aggregation.
- Product split-screen viewport layout, safe areas, reduced HUD/menu/inventory,
  pause ownership, or audio listener policy.
- Shared-cache optimization across multiple client replicas.
- Simultaneous retained embedded-world preview composition in a multi-flat
  frame; direct mono, stereo, and multiview composition remain supported.
- Helper authority, build-plan editing, mixed XR-plus-flat surface hosting, and
  four-player product performance tuning.
- Bedrock black-box measurement, which remains valuable hardware research but
  is not an implementation oracle or a blocker for this foundation.
