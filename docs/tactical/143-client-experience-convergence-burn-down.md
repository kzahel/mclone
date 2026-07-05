# 143: Client Experience Convergence Burn-Down

Status: open, ready to implement. Opened 2026-07-05. This tactical is the
executable checklist for
[`../client-experience-architecture.md`](../client-experience-architecture.md)
(revised 2026-07-05). That document is law for this work; this tactical is the
work order and log. If they ever disagree, stop and reconcile the documents
before writing more code.

Workstream: native Rust shared architecture. Adopted stance: convergence and
enforcement take priority over new user-facing feature work until this
burn-down is closed or every remaining slice has an owner.

## Contract For Implementing Agents

Read this section, your slice section, and the architecture doc's
[Guardrails](../client-experience-architecture.md#guardrails) before writing
code. If your context is compressed mid-task, re-read this section first.

The invariants, in priority order:

1. **The goal is deletion, not wrapping.** Every slice must reduce app-local
   policy. If your diff adds matching `GameUiAction` logic to two or more app
   crates, or adds a wrapper that leaves the old duplicate alive, stop — the
   slice is being done wrong.
2. **Sans-I/O core.** Shared policy code never does I/O, never blocks, never
   spawns, is never `async`, and never reads the clock or entropy. If shared
   code needs any of those, it emits an effect and later accepts a completion.
3. **One noun, one owner.** Session, catalog, status projection, settings —
   each policy noun has exactly one shared implementation. Adapters hold
   executors and handles, never a second implementation.
4. **No silent inert arms.** A profile that cannot support a visible shared
   action projects shared unsupported/pending/disabled state. `=> {}` for a
   visible action is a defect.
5. **TypeScript executes, never decides.** No validation, id generation,
   ordering, or user-facing message text in browser glue.
6. **Capabilities before platform branches.** No `if desktop / if web / if xr`
   in policy code; profile facts only.

Process rules:

- **One slice per session.** Do not start the next slice in the same run
  unless the user asks. Between slices the user reviews.
- **Do the slices in order** unless a recorded note in Open Questions explains
  the deviation. The order front-loads the forks that compound fastest.
- **A slice is done only when** its exit criteria are individually verified,
  its validation block has been run and results recorded in the slice
  section (the tactical 141 pattern), the tripwire greps below have been run
  and show no new hits, the slice's row in the status table is updated, and
  [`../topics/platform-parity.md`](../topics/platform-parity.md) is updated if
  user-visible support or contract adoption changed.
- **If a slice cannot comply with the architecture doc,** stop and record the
  conflict under Open Questions instead of improvising a local exception.
- **Do not** take on adjacent workstreams from the architecture doc's
  out-of-scope list (web inline render fork, worker/job lifecycle, web audio,
  text entry) even when they brush against a slice.

Tripwire greps (run before declaring any slice done; baselines recorded in
Slice 0):

```bash
rg -n "fn apply_ui_action|fn apply_web_ui_action|fn apply_xr_ui_action" \
  native/apps/mclone-native-client/src native/apps/mclone-web-client/src \
  native/apps/mclone-android-client/src native/crates/mclone-xr-scene/src

rg -n "GameUiAction::[A-Za-z]+(\(_\))? => \{\}" \
  native/apps native/crates/mclone-xr-scene/src

rg -n "already exists|was not found|cannot delete" \
  native/apps/mclone-web-client/www
```

## Slice Status

| Slice | Closes | Status |
|---|---|---|
| 0: enforcement baseline | gates | open |
| 1: facade + `GameUiAction` classification | V1 (shared side), V2 (policy) | open |
| 2: desktop and web adopt the facade | V1/V2 on flat lanes | open |
| 3: TypeScript catalog demotion | V3 | open |
| 4: XR session-machine merge | V4 | open |
| 5: emulated-XR profile test | display-neutrality gate | open |
| 6: flat Android adopts the facade | V1/V2 on Android | open |
| 7: XR catalog CRUD via shared controller | last V2 no-ops | open |

V1-V4 are defined in
[`../client-experience-architecture.md`](../client-experience-architecture.md#current-violations-burn-down).

## Slice 0: Enforcement Baseline

Why: every later slice self-checks against gates that must exist first.

Deliverables:

- add a `pnpm` script (suggested name `native:policy:wasm-check`) running
  `cargo check --manifest-path native/Cargo.toml -p mclone-app-runtime --lib
  --target wasm32-unknown-unknown` (verified passing 2026-07-05; `--lib` is
  required because the crate ships native-only diagnostic binaries);
- run the three tripwire greps and record their full output counts in this
  section as the baseline (dispatch sites, inert-arm count per crate, TS
  policy-string hits);
- no production code changes.

Non-goals: do not fix any hits found; this slice only measures and gates.

Exit criteria:

- the script exists and passes from a clean checkout;
- baseline counts are recorded below.

Validation:

```bash
pnpm native:policy:wasm-check
git diff --check
```

Recorded result: (pending)

## Slice 1: Facade Plus Full `GameUiAction` Classification

Why: V1's duplicated settings/toggle policy has no shared owner, and V2's
inert arms exist because no capability projection exists to replace them.

Deliverables:

- audit every `GameUiAction` variant and classify it: core action,
  host-effect action, capability-gated, or projection-specific; record the
  table in the architecture doc (its Decisions section reserves the slot);
- add the display-neutral facade in `mclone-app-runtime` (working name
  `ClientExperienceController`), composing the existing `flat_client_catalog`
  and `flat_client_session` helpers unchanged;
- add a shared settings/toggle controller behind the facade owning the
  duplicated bodies (`SetMovementMode`, `SetFlySpeed`, `SetMovementSpeed`,
  `ToggleFullbright`, `ToggleCrosshair`, `ToggleFirstPersonPlayer`,
  far-LOD, render-distance, and the rest the audit classifies as core),
  emitting effects for anything touching runtime/render resources;
- add a shared capability-projection type so unsupported actions surface
  visible shared state instead of silent no-ops;
- conformance tests per the architecture doc's Enforcement list, fake-adapter
  style, no `winit`/DOM/OpenXR/Android types in the tests.

Non-goals / drift tripwires:

- do not modify any app crate in this slice (tactical 141 Slice 1 pattern:
  shared crate only, apps adopt in the next slice);
- do not rename or restructure `flat_client_catalog`/`flat_client_session`
  internals beyond what composition requires — renames ride along later;
- if the facade starts owning input or widget vocabulary rather than routing
  it, that vocabulary belongs in `mclone-input`/`mclone-ui` — stop and put it
  there.

Exit criteria:

- classification table exists in the architecture doc with zero unclassified
  variants;
- settings controller + capability projection exist with conformance tests;
- app crates untouched (`git diff --stat` shows only `mclone-app-runtime`,
  `mclone-ui` if the projection type lands there, and docs);
- `pnpm native:policy:wasm-check` still passes.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
pnpm native:policy:wasm-check
git diff --check
```

Recorded result: (pending)

## Slice 2: Desktop And Web Adopt The Facade

Why: closes V1 and V2 on the flat lanes; both already consume the constituent
controllers, so this is adoption, not invention.

Deliverables:

- `flat_client_driver.rs` and `web_canvas.rs` route all classified core
  actions through the facade; their dispatch matches shrink to host-effect
  execution and completion feeding (the shape `app.rs:534` already has);
- inert arms on desktop/web become capability projections (far LOD on web
  either gains support via the shared controller effects or projects visible
  unsupported state — decide from the classification, do not leave `{}`);
- delete the app-local duplicated toggle bodies.

Non-goals / drift tripwires:

- no behavior changes beyond de-duplication and projection: if a toggle
  behaves differently after adoption, that is a bug, not an improvement;
- do not touch XR, Android, or TypeScript in this slice.

Exit criteria:

- inert-arm tripwire grep returns zero hits for visible shared actions in
  `mclone-native-client` and `mclone-web-client` (remaining XR/Android hits
  are Slices 4/6/7 territory — record the counts);
- desktop and web dispatch sites contain only host-action arms and
  completion feeding;
- all existing desktop/web smokes stay green.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client ui_action_routing
cargo test --manifest-path native/Cargo.toml -p mclone-native-client catalog_
pnpm native:desktop-offscreen:smoke
pnpm native:web:typecheck
pnpm native:web:smoke
pnpm native:web:catalog-smoke
git diff --check
```

Recorded result: (pending)

## Slice 3: Demote TypeScript Catalog Glue To A Dumb Executor

Why: closes V3, the only cross-language policy fork, where drift is hardest
to notice.

Deliverables:

- id/name validation, id generation (`available_from_display_name`), world
  sort order, active-delete guard, and all user-facing message text route
  through the Rust `world_catalog` module from wasm, before/after the raw
  IndexedDB operation;
- `mclone-web-world-catalog.ts` keeps only IndexedDB mechanics: open, get,
  put, delete, cursor, transaction, error passthrough as raw data;
- delete the TS copies: `LOCAL_WORLD_ID_MAX_LEN`, the
  `already exists` / `was not found` / `cannot delete active local world`
  throws, `availableLocalWorldIdFromDisplayName`, and the sort comparator.

Non-goals / drift tripwires:

- do not redesign the IndexedDB schema or the worker startup path;
- do not weaken `mclone-web-smoke.js` assertions — the smoke may keep
  asserting on displayed text, since after this slice that text has exactly
  one producer (Rust).

Exit criteria:

- the policy-string tripwire grep returns zero hits in
  `mclone-web-world-catalog.ts` (smoke-harness assertion strings are
  acceptable and recorded);
- create/open/delete/delete-active/duplicate-name flows behave identically in
  the catalog smoke.

Validation:

```bash
pnpm native:web:typecheck
pnpm native:web:build
pnpm native:web:catalog-smoke
pnpm native:web:smoke
git diff --check
```

Recorded result: (pending)

## Slice 4: XR Adopts The Shared Session Machine

Why: closes V4, the largest and fastest-compounding fork. This is the
flagship slice and the proof the core is not flat-shaped.

Deliverables:

- `mclone-xr-scene` replaces `replace_session_for_request` and its private
  `session_status` transition rules with `GameSessionCoordinator`, the shared
  session effects, and the shared session projection, consumed through the
  facade;
- XR keeps: runtime factory payloads (`SceneOptions` mapping, TCP/Android
  session adapters), world-panel projection, pose/ray plumbing, comfort
  presentation;
- XR's dispatch shrinks the session/status arms to facade routing plus XR
  host effects; `SetXrTurnMode` becomes a capability-gated core action
  (functional in XR, visibly unsupported elsewhere) per the Slice 1
  classification.

Non-goals / drift tripwires:

- **if you are adding a trait so the private XR machine and the shared
  coordinator can coexist long-term, you have drifted — the private machine
  gets deleted, not abstracted over;**
- do not move OpenXR session/swapchain/action types into shared crates;
- do not wire catalog CRUD here (Slice 7);
- do not tune comfort behavior in this slice.

Exit criteria:

- `rg -n "replace_session_for_request" native/crates/mclone-xr-scene/src`
  returns nothing;
- `rg -n "GameSessionCoordinator" native/crates/mclone-xr-scene/src` returns
  hits;
- no XR-private session status transition rules remain (status text and
  transitions come from shared projection);
- `cargo test -p mclone-xr-scene` passes, desktop and Android XR app crates
  still compile.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
pnpm native:policy:wasm-check
git diff --check
```

Plus the device lanes per
[`../platforms.md`](../platforms.md#validation-policy): one desktop-XR or
headset smoke is required before this slice is called fully validated, since
it changes session replacement behavior on XR.

Recorded result: (pending)

## Slice 5: Emulated-XR Desktop Profile Test

Why: makes display-neutrality a standing regression gate instead of a
device-lane afterthought. Lands with or immediately after Slice 4.

Deliverables:

- a desktop/offscreen test profile with emulated XR facts: controller-ray
  pointer, world-panel placement, no headset;
- an automated flow driving title → world list → create/open through the
  facade under that profile, in the standard test/smoke set;
- record which profile facts were needed (feeds the architecture doc's open
  question on emulated-XR facts).

Non-goals: this does not replace headset validation and must not be sold as
such; it gates policy display-neutrality only.

Exit criteria: the test exists, runs without a headset or display, and fails
if a menu/session policy change breaks the XR projection path.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
pnpm native:desktop-offscreen:smoke
git diff --check
```

Recorded result: (pending)

## Slice 6: Flat Android Adopts The Facade

Why: closes V1/V2 on Android; catalog/session contract rows for Android are
currently ✗ in parity Matrix 2.

Deliverables:

- `mclone-android-client` routes core actions through the facade; its
  dispatch shrinks to host arms (activity, touch, storage roots, packaging
  stay app-local);
- Android catalog warn no-op becomes either a wired app-private catalog
  adapter or a visible capability-gated state — decide with the user before
  implementing storage;
- parity Matrix 2 `flat_client_*` rows for Android flip from ✗.

Exit criteria: inert-arm grep returns zero visible-action hits in
`mclone-android-client`; AVD session smoke stays green.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
pnpm native:android:apk
pnpm native:android:avd-smoke
git diff --check
```

Plus the AVD touch/session smokes listed in
[`../platforms.md`](../platforms.md#validation-policy).

Recorded result: (pending)

## Slice 7: XR Catalog CRUD Through The Shared Controller

Why: closes the last V2 no-ops; persistent worlds reach XR through the shared
owner, never as an XR-local wiring pass.

Deliverables:

- XR world-list/create/open/delete routes through
  `FlatClientCatalogController` (by then display-neutrally named) via the
  facade, projected onto world-space panels;
- the `log::warn!("persistent world catalog action is not wired to XR scene
  yet")` arms are gone;
- storage adapter per platform: desktop XR uses the native catalog backend,
  Android XR an app-private root adapter.

Exit criteria: no catalog warn no-ops remain in `mclone-xr-scene`; the
emulated-XR profile test covers a catalog action end to end; headset
validation of the panel flow recorded.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
pnpm native:policy:wasm-check
git diff --check
```

Plus one headset lane per [`../platforms.md`](../platforms.md#validation-policy).

Recorded result: (pending)

## Open Questions

- Slice 6: does flat Android get a real app-private catalog backend in this
  workstream, or a capability-gated "no persistent worlds yet" state first?
  (Storage work may deserve its own tactical; the facade adoption does not
  depend on the answer.)
- Recorded deviations from slice order, if any, go here with reasons.
