# Tactical 274: All-Client Distant Terrain Control

Status: active 2026-07-27.

Topic: `procedural-horizon-clipmap`

## Originating Request

Expose the new procedural distant-terrain presentation in the shared Graphics
options so it can be enabled, tested, and checked in every first-class client:
desktop flat, browser/WASM, flat Android, desktop OpenXR, and Android XR /
Quest. The setting may remain experimental and imperfect while evidence is
collected. Record gaps here instead of hiding the feature behind startup-only
configuration.

The default XR product path is separate per-eye rendering. Full-frame
multiview is an opt-in Quest alternate that showed no decisive production
performance benefit and is unsupported by some runtimes. It does not block
this control. Composed terrain remains explicitly unavailable in that optional
path until the horizon renderer gains a real two-layer multiview pipeline.

## Objective

Turn the existing startup-only `exact-only | composed` scene choice into one
shared, persisted player setting:

```text
Graphics
  Distant Terrain: Off | Experimental
```

The shared setting must:

- appear in the same Graphics page on every flat and XR client;
- persist through the existing graphics-preference storage on native,
  Android, XR, and browser hosts;
- apply and tear down the shared `mclone-terrain-view` scene presentation at
  runtime without rebuilding the app or restarting the session;
- preserve the allocation-free exact-only path when off;
- retain startup CLI/query overrides as developer/test inputs;
- use the same clipmap-derived far projection in flat and per-eye XR composed
  views; and
- keep terrain semantics, residency, coverage, and renderer policy out of app
  adapters.

## Current Constraints

The live procedural source currently matches only local
`mclone-overworld-v1` worlds. Other generation profiles and remote sessions do
not publish a compatible procedural source identity. The control may retain a
global preference, but the scene must render exact-only when the active source
is incompatible and make that limitation observable.

The accepted PH-4 implementation has known limitations:

- live exact natural-tree meshes can overlap proxy-owned frontier trees;
- the full-quality browser path can settle slowly;
- flat Android performance has not yet been characterized;
- the first real Quest 3 session confirmed the per-eye horizon and full
  projection reach, but sustained high-speed persistent-world travel ended in
  an exact-terrain starvation symptom and a platform low-memory kill;
- full-frame multiview has no procedural-horizon pipeline; and
- remote hosts do not yet publish generator/profile facts suitable for local
  procedural reconstruction.

These are experiment follow-ups, not reasons to keep the setting hidden.

## Slices

### Slice 1: shared setting and persistence

- [x] Add a UI-neutral terrain-presentation value to `mclone-ui`.
- [x] Add the row to the shared Graphics category.
- [x] Route its action through `client_experience` and the exhaustive scene
  settings dispatcher.
- [x] Persist it in the schema-1 graphics document with backward-compatible
  missing-field defaults.
- [x] Restore it through the existing desktop, web, Android, desktop XR, and
  Android XR preference adapters.

### Slice 2: runtime scene effect

- [x] Apply Off/Experimental without restarting the active scene.
- [x] Reset procedural resources immediately when switched off.
- [x] Recreate them lazily when switched on.
- [x] Keep exact-only byte/pixel behavior and zero horizon scheduling.
- [x] Gate effective composition on a compatible local
  `mclone-overworld-v1` source while retaining the stored preference.

### Slice 3: per-eye XR reach

- [x] Derive XR composed far distance from the same resident clipmap contract
  used by flat views.
- [x] Rebuild both per-eye projections with that far distance.
- [x] Preserve the existing 700-block projection in exact-only mode.
- [x] Prove both eyes consume one committed residency/source generation with
  distinct view/projection data.

### Slice 4: all-target evidence

- [x] Shared UI/controller/preference/scene tests.
- [ ] Desktop exact-only and composed captures, including a runtime toggle.
  The matched startup captures pass; a one-process visual toggle receipt is
  still open.
- [ ] Headed browser composed capture and preference restoration. The composed
  capture passes; an automated reload receipt for this preference is still
  open.
- [ ] Flat Android APK plus an AVD screenshot when the host is available.
- [x] Synthetic stereo capture with horizon pixels in both eyes.
- [ ] Desktop OpenXR per-eye runtime validation as available.
- [x] Quest 3 per-eye interactive validation. The distant view was compelling
  and remained visible during high-altitude, 8x-speed travel; the same session
  exposed the exact-terrain starvation / low-memory failure recorded below.
- [x] Android XR APK validation through the scripted NDK lane.

## Implementation And Evidence

Implemented on 2026-07-27:

- `GameUiRenderState` and the shared V2 Graphics category now expose
  `Distant Terrain: Off | Experimental`. If the stored choice is active on an
  incompatible world, the value reads `Experimental (Unavailable)` without
  discarding the preference.
- The schema-1 graphics document gained a backward-compatible
  `terrainPresentation` field. The desktop, browser, flat Android, desktop XR,
  and Android XR adapters already consume this shared document.
- `McloneSceneHost` owns the desired value and derives an effective value from
  active source identity. Only local `mclone-overworld-v1` is composed today;
  remote and other-profile sessions remain exact-only.
- Explicit `--terrain-presentation` and `terrainPresentation` launch inputs
  retain precedence over a stored preference. This was caught by the first
  synthetic-stereo run, which initially restored a legacy exact-only document
  over the explicit composed request.
- Dynamic and frozen XR views derive both eye far planes from the same clipmap
  reach helper as mono composed views. Exact-only remains `700` blocks.
- The synthetic stereo capture now waits for non-empty, target-ready horizon
  diagnostics before saving, so it cannot silently pass with exact-only
  pixels.

Validation:

- `cargo check --workspace`
- `cargo check -p mclone-native-client --features xr`
- focused UI, controller, persistence, scene-dispatch, projection, and fixed
  stereo-view tests
- `pnpm native:web:typecheck`
- `pnpm native:android:apk`
- `pnpm native:android-xr:apk`
- headed-Wayland browser composition: 10 levels, 160 tiles, target ready,
  809 tree instances, and 48/48 vegetation jobs
- inspected native exact/composed captures:
  `/tmp/mclone-t274-desktop-exact.png` and
  `/tmp/mclone-t274-desktop-composed.png`
- inspected per-eye composed capture:
  `/tmp/mclone-t274-xr-composed.png`; the two eyes retain distinct pixels and
  both show the procedural background
- inspected headed browser canvas:
  `/tmp/mclone-native-web-app-canvas.png`

The full `mclone-scene` unit suite passes. Its separate
`composable_world_presentation_contract` integration target currently has one
stale source-shape assertion: it expects
`render_full_frame_for_view_inner` to call actor preparation directly, while
the committed renderer now delegates through
`render_full_frame_for_view_inner_with_backdrop`. This change does not touch
that renderer file.

### Quest 3 sustained-travel failure

Interactive acceptance on Quest 3 `2G0YC1ZF93041Z` provided the first physical
per-eye product evidence. The viewer could fly high enough to see an unusually
large Minecraft-scale landscape and travel for an extended period at roughly
8x flight speed. After returning to ground level, exact terrain stopped
appearing under the current position. Continued travel ended when Horizon OS
closed the app.

The retained device log makes the terminal event exact. At
`2026-07-27 13:45:49`, `lowmemorykiller` reported a `6,621,968 kB` process
footprint above its `6,577,152 kB` threshold, then killed
`com.kzahel.mclone.xr` to free `3,863,244 kB` RSS plus `2,015,228 kB` swap.
`ApplicationExitInfo` records `reason=LOW_MEMORY`; there is no Rust panic,
fatal signal, or graphics-driver exception.

The horizon itself has fixed residency: the default ten levels by four-by-four
tiles allocate 160 clipmap slots, with bounded transition and vegetation
admission. It can raise Quest's baseline memory and therefore reduce available
headroom, but it has no distance-travel-proportional tile store.

At the incident revision, the strongest source-level causal candidate was the
persistent exact-world mailbox rather than the clipmap:

- native persistence uses unbounded request and completion channels;
- a moving view may admit up to 128 new chunk-load requests per reconcile;
- unloading a position clears scheduler bookkeeping but does not cancel the
  already-enqueued persistence request;
- every generated exact chunk queues a full `ChunkRecord` cache save; and
- while pending writes exist, the persistence thread accepts incoming requests
  first and advances one background write only after a one-millisecond receive
  timeout.

Sustained movement can therefore leave obsolete loads and full-record cache
writes ahead of the current view. This matches both observed symptoms: the
current-position persistence misses needed to admit generation stop catching
up, while retained queued records grow until the platform kills the process.
This remains a strong diagnosis rather than a closed attribution until a
reproduction records request counts/owned bytes by operation family.

Tactical
[`275`](275-bounded-persistence-streaming.md) implemented the immediate shared
remediation on 2026-07-28: bounded foreground/durable/cache lanes, lossless
durable backpressure, discardable cache-pressure skips, cancellation on
interest loss, one-write-per-request fairness, browser record-cache eviction,
and shared current/high-water owned-byte diagnostics. This removes the known
unbounded mailbox mechanisms but does not by itself close the device incident;
the same persistent-world Quest flight soak must still show exact-center
reacquisition plus flat queue and process-memory plateaus.

## Explicit Follow-Ups

Do not silently close these gaps:

1. close Tactical 275's physical acceptance: run a persistent-world
   8x-flight/churn soak that proves current-center exact terrain reacquires and
   Quest RSS, queue counts, and owned-byte high-water marks plateau;
2. decide the independent generated-clean storage policy or storage-optimized
   mode so long-distance travel does not consume device storage merely because
   a chunk was generated;
3. prune or separately bound completed scheduler job-history metadata during
   the same long-travel audit;
4. add a true full-frame multiview horizon pipeline if that optional Quest
   path becomes valuable;
5. continue collecting in-headset quality, comfort, thermal, and frame-pacing
   evidence on desktop OpenXR and Quest;
6. choose device-specific work budgets only from measured evidence, without
   changing terrain semantics;
7. resolve live exact/proxy tree overlap;
8. publish safe source identity for remote sessions or keep distant terrain
   unavailable there; and
9. decide after testing whether Experimental should become the default for
   compatible worlds.

## Acceptance

The tactical's implementation slice is acceptable when every first-class
client receives the same Graphics row and setting effect, compatible flat and
default per-eye XR paths draw the composed horizon with full projection reach,
incompatible sources remain safely exact-only, exact-only remains protected,
and the available build/pixel evidence is recorded here.

Full-frame multiview support and perfect headset tuning are not acceptance
requirements for exposing this experimental control.
