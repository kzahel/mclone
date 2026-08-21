# Tactical 327: Atomic Distant Terrain Settings

Status: active 2026-08-21. Physical Quest failure evidence and the corrective
architecture are accepted; implementation is authorized end to end.

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
- [ ] Correct the living Quest default prose after implementation evidence.
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

- Run the affected shared package suites and workspace formatting/checks.
- Exercise every direct preset pair, especially Off-to-Low/Medium/High with
  exact terrain already ready, plus rapid staged changes before one Apply.
- Build native, Wasm, flat Android, and Android XR boundaries.
- Inspect native and synthetic-stereo pixels after cold enable.
- On physical Quest, select and apply Off, Low, Medium, High, and Low in one
  process; require no failure notification or new abnormal exit, complete
  frontier receipts, and normal continued head/controller presentation.
- Confirm Low remains the unset Android XR default and reaches the established
  72-submission behavior; retain the p95 qualification rather than claiming a
  stricter lock than the evidence supports.
- Commit evidence and close the tactical.

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
