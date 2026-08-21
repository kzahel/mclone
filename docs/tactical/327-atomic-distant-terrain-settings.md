# Tactical 327: Atomic Distant Terrain Settings

Status: complete 2026-08-21. The shared implementation, cross-platform
validation, real Graphics UI pixels, and deterministic physical Quest 3
Off -> Low -> Medium -> High -> Low regression are accepted.

Topic: `procedural-horizon-clipmap`
Topic: `graphics-video-settings`

## Instruction Synthesis

Changing Distant Terrain in the live Quest Graphics menu can show an exact-
frontier error and close the app. Evaluate the retained device evidence, fix
the transition rather than merely hiding the error, replace the one-tap cycle
with a control that exposes Off, Low, Medium, and High directly, and use an
explicit apply boundary for this expensive setting. Implement and validate the
shared behavior across every client, committing coherent phases as they land.

The recent Quest recovery did reach 72 submissions per second at Low after the
constant-time frontier lookup. Preserve Android XR's Low unset default. Correct
the living documentation that later described Low as Off by default while
retaining the measured p95 qualification and the remaining performance work.

## Device Evidence And Root Cause

Two consecutive Android XR exits left one updated failure notification:

```text
render XR terrain left eye: render shared live terrain backdrop:
exact terrain generation 26 has no complete frontier certificate
```

The activity surfaced the native error, returned to Horizon Home, and used its
documented delayed `killProcess` fallback. `ApplicationExitInfo` consequently
records signal 9 rather than low memory. This is not the earlier persistence-
mailbox OOM incident.

The shared transition currently commits three facts too early:

1. the menu action immediately changes the desired preset;
2. Off-to-enabled constructs a cold terrain-view engine and immediately
   exposes already-ready exact coverage; and
3. the scene marks and persists the new preset before the clipmap has any
   committed procedural presentation.

The frontier correctly rejects that unsafe mixed frame. The bug is allowing a
temporary cold-start readiness state to escape as a fatal XR frame error.
The cycle control amplifies it: reaching a lower preset from High passes
through Off and tears down the reusable engine before rebuilding it.

## Product Contract

### Desired, staged, applied, and persisted are distinct

- The Graphics page owns one staged selection. Moving among the four visible
  values performs no engine work.
- Apply submits the staged value once. Cancel restores the current desired
  value without a scene effect.
- The scene retains its previous applied presentation while an enabled-to-
  enabled reconfiguration prepares. A cold Off-to-enabled transition keeps
  exact-only output until the first complete frontier certificate is ready.
- The applied value changes only after one drawable complete presentation.
- A live choice is persisted only after that same acceptance point. Off may
  tear down, apply, and persist immediately because it removes the optional
  representation without a warm-up phase.
- Rejected configuration or allocation leaves the previous applied and stored
  value active, returns the selector to it, and reports a bounded inline
  failure. A settings failure must never terminate a client frame loop.

### Direct stepped selection

Render `Off | Low | Medium | High` as one four-stop segmented/stepped control,
with the selected value and the currently applied value distinguishable while
work is pending. This is a discrete ordered choice, not a continuous quality
number. Mouse, keyboard, controller, touch, and tracked XR rays must dispatch
the same typed staging and Apply/Cancel actions.

## Ownership

- `mclone-terrain-view` owns safe cold frontier warm-up, complete-certificate
  admission, and renderer diagnostics.
- `mclone-scene` owns requested/applied state, rollback, accepted-effect
  persistence, and exact-only presentation while cold resources prepare.
- `mclone-app-runtime` owns staged settings reduction and typed effects.
- `mclone-ui` owns the four-stop selector, Apply/Cancel controls, labels, and
  pending/rejected projection.
- Platform apps remain storage, input, lifecycle, target, and presentation
  adapters. No Quest-only setting semantics are allowed.

## Implementation Phases

### Phase 0: Tactical And Default Reconciliation

- [x] Retain the physical failure notification and exit classification.
- [x] Trace the cold Off-to-enabled certificate gap through shared code.
- [x] Record the direct staged-selector and atomic-apply contract.
- [x] Correct the living Quest default prose after implementation evidence.
- Commit the tactical before changing behavior.

### Phase 1: Atomic Renderer And Scene Admission

- [x] Treat missing procedural coverage during bounded cold preparation as
  nonfatal warming, submit no procedural terrain/tree/connectors for that
  unsafe frame, and continue bounded clipmap work.
- [x] Keep genuinely invalid exact profiles or settled incomplete topology as
  errors.
- [x] Track applied separately from desired/configured. Do not persist an enabled
  request until diagnostics prove a complete drawable target.
- [x] Contain construction or reconfiguration rejection, roll back to the prior
  applied value, and expose the failure without ending the frame loop.
- [x] Add focused terrain-view and scene regressions for cold-frontier
  classification and drawable-target acceptance. The selector/reducer phase
  adds rollback and accepted-effect persistence coverage at its shared
  boundary.
- [x] Commit.

### Phase 2: Shared Staged Selector

- [x] Add typed Stage, Apply, and Cancel Distant Terrain actions.
- [x] Replace the cycle row with four directly addressable stops and explicit
  Apply/Cancel buttons using the existing shared menu/input machinery.
- [x] Coalesce arbitrary staging changes before Apply and reject Apply while a
  previous request is still preparing.
- [x] Update the browser menu check to select Low directly and apply it once.
  Its staged receipt proves that neither the engine nor storage changed before
  Apply; its accepted receipt proves Low was stored only once target-ready.
- [x] Add UI/reducer tests for pointer and focused controller/keyboard
  selection, pending labels, cancellation, coalescing, and rejected concurrent
  Apply.
- [x] Capture and inspect the first native rendered Graphics page and the
  browser's staged mobile page. Both show the four-stop rail, selected label,
  current/applied status, and Apply/Cancel controls without clipping.
- [x] Commit.

Temporary native and mobile-browser pixel evidence was inspected under
`/tmp`; it remains outside the repository.

### Phase 3: Cross-Platform And XR Validation

- [x] Run the affected shared package suites and workspace formatting/checks.
- [x] Exercise every direct preset pair, especially Off-to-Low/Medium/High with
  exact terrain already ready, plus rapid staged changes before one Apply.
- [x] Build native, Wasm, flat Android, and Android XR boundaries.
- [x] Inspect native and synthetic-stereo pixels after cold enable.
- [x] On physical Quest, stage and apply Off, Low, Medium, High, and Low through
  the shared UI actions in one process; require no failure notification or new
  abnormal exit, complete frontier receipts, and normal continued
  head/controller presentation.
- [x] Confirm Low remains the unset Android XR default and reaches the established
  72-submission behavior; retain the p95 qualification rather than claiming a
  stricter lock than the evidence supports.
- [x] Commit final device evidence and close the tactical.

### Phase 3 Evidence

The shared suites pass with 151 terrain-view tests plus one adapter-dependent
ignore, five focused scene terrain-view tests, 301 app-runtime tests, and 115
UI tests. The app-runtime matrix covers all 12 distinct source/target pairs;
the UI matrix directly addresses all four stops from each current preset.
Workspace formatting and all-target checks pass.

Native/Wasm, flat Android, and release Android XR boundaries build. The headed
browser terrain-horizon smoke proves staged Low leaves both the engine and
`localStorage` at Off. It then applies Off -> Low -> Medium -> High -> Low in
one process, requires each direct target to become drawable before persistence,
moves after the High-to-Low downshift, captures Low pixels, and restores Low
after reload.

That sequence also covers two reconfiguration invariants found during
closeout. Accepted scene state must clear the reducer's applying state before
another value can be staged. The vegetation coordinator's desired-set bound
must grow with Medium/High, while a downshift must release coarse vegetation
presentations that no longer belong to the new preset; otherwise stale
reservations exhaust the bounded transition guard pool during later movement.

The native 960-by-540 Graphics capture, compact mobile-browser staged page,
and 640-by-640-per-eye Low synthetic-stereo capture were inspected under
`/tmp` without clipping or invalid pixels. The final stereo receipt reports
251,285 differing eye pixels.

The native `mclone-web-client` unit binary currently has three outside-scope
deterministic runtime-smoke expectation mismatches: observed update counts are
10/14 rather than 9/13, and the packed expected word follows that stale count.
No runtime-smoke implementation or expectation changed in this tactical; 41
other web-client unit tests, the Wasm build, and the real browser Distant
Terrain acceptance pass. Keep that existing suite discrepancy visible rather
than changing unrelated deterministic counts here.

After the headset was reattached, the canonical provider found an authorized,
fully charged Quest 3. The release APK built through
`pnpm native:android-xr:apk`; the device validator then passed this bounded
one-process lane while reusing the already staged device asset pack:

```text
pnpm native:android-xr:validate --skip-build --skip-assets \
  --terrain-lod-cycle --xr-debug-ui graphics \
  --log /tmp/mclone-t327-quest-lod-cycle-rerun.txt
```

The device mode stages and applies each value through the same typed shared UI
actions as the Graphics page, waits for the scene's applied value, and rejects
an enabled target without target-ready diagnostics plus a complete frontier
admission. The accepted receipts were:

| Step | Applied | Frontier | Exact generation | Drawn levels / tiles |
|---:|---|---|---:|---:|
| 1 | Off | disabled; target readiness not required | - | - |
| 2 | Low | preferred; target ready | 15 | 6 / 40 |
| 3 | Medium | preferred; target ready | 56 | 8 / 44 |
| 4 | High | preferred; target ready | 63 | 10 / 48 |
| 5 | Low | preferred; target ready | 63 | 6 / 40 |

Completion reported the exact sequence and final Low at submission 220. The
same log records the OpenXR current display refresh as 72.0 Hz and contains no
failure marker, fatal signal, panic, rejected apply, or transient incomplete-
certificate error. An earlier 180-second run on the fixed APK recorded two
active controllers and retained live stereo Graphics pixels while the process
continued at 72 Hz.

Post-run `ApplicationExitInfo` records only the Quest provider's intentional
`USER REQUESTED / FORCE STOP` cleanup for the new runs. The two original
signal-9 exits remain older history entries, and no active Mclone failure
notification exists. The provider restored proximity and sleep state after
each run.

## Acceptance

- No preset choice requires visiting an intermediate value.
- Merely moving the selector performs no terrain allocation or teardown.
- Off-to-enabled never draws uncertified procedural coverage and never returns
  the transient missing-certificate error.
- Enabled-to-enabled retains common levels and keeps the old applied state
  until the requested descriptor is drawable.
- Persistence records only accepted values; rejected work preserves the prior
  stored value.
- Settings errors remain local, visible, and recoverable on every client.
- Mono, per-eye, and multiview consume the same accepted frontier generation.
- Quest Low remains the unset default with accurate 72-Hz evidence wording.

## Non-Goals

- no automatic runtime quality controller;
- no change to preset geometry, reach, vegetation semantics, fog, exact render
  distance, or frontier topology;
- no second full terrain engine beside the active one;
- no Quest-local UI or renderer branch; and
- no claim that 72 submissions per second eliminates the recorded p95 tail.
