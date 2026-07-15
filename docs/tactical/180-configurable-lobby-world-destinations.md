# Tactical 180 — Configurable Lobby World Destinations

Status: complete — 2026-07-15; Slices 0-5 landed

Topic: `embedded-worlds`

## Objective

Replace the fixed one-chunk authored-island product destination with a shared,
configurable bounded preview of the most recently actively played compatible
local world. If no compatible catalog world exists, retain the same lobby
experience by warming a persistent managed full-overworld fallback seed.

The tabletop crop and the activated destination are separate contracts. The
preview may expose a configured rectangular 2x2, 4x4, or other validated chunk
region and a configured vertical section range, while activation selects the
already-running complete destination world. Previewing a catalog world must
not update recency. A successful activation into it must.

This tactical also closes the unsafe arrival reported in production browser
WebGPU: the old authored island accepted the first scan-order surface column
near its rim, and the browser activation smoke accepted nonblank pixels without
proving that the player remained supported.

## Architecture

- `mclone-render` owns a finite section-aligned rectangular source region. Its
  legacy center-plus-radius constructor remains a convenience, not the data
  model.
- `mclone-app-runtime` owns path-free scenario preview bounds, catalog recency
  selection, catalog play-record semantics, and the versioned managed
  full-overworld fallback recipe.
- `mclone-scene` owns destination-source selection, accepted-entry-relative
  preview resolution, complete-slot activation, arrival admission, and the one
  active plus optional standby lifecycle.
- Native adapters resolve catalog ids or managed keys to filesystem/SQLite
  storage. Browser adapters resolve the same neutral ids to IndexedDB and
  Workers. Neither adapter chooses the destination, bounds, spawn, or recency
  policy.
- The direct single-world path does not allocate scenario, preview, second
  runtime, placed renderer, or catalog-play state.

The selected destination source is one of:

1. the most recent compatible catalog world, ordered by explicit
   `last_played_unix_millis` and then existing deterministic catalog ordering;
2. a versioned app-private generated-overworld fallback when no compatible
   catalog world is available.

The old authored island remains a deterministic renderer/actor fixture and
historical compatibility recipe. It is no longer the default product
destination.

## Product Contracts

1. `Enter Lobby` remains immediately useful even while destination selection,
   provisioning, generation, compilation, and GPU admission continue.
2. The preview source bounds are data, validated independently of placement.
   At least 2x2 and 4x4 configurations must execute through the same path.
3. Source X/Z placement is centered on the configured bounds. Source Y and the
   entry region resolve from the destination's accepted safe startup pose after
   authoritative camera reconciliation.
4. The preview stays bounded after startup. The selected destination runtime is
   not bounded and continues ordinary chunk tracking/generation when active.
5. Opening or warming a catalog destination does not mutate its catalog row.
   A successful complete-slot activation records active play exactly once per
   outbound activation.
6. Arrival is admitted only with loaded body clearance and solid support. The
   first uncovered frame and a later stability sample must both remain on
   terrain.
7. The product uses one shared Rust feature implementation. Browser WebGPU is
   an acceptance lane for every rendered slice, not a parity follow-up.

## Slices

### Slice 0 — Baseline, plan, and failure receipt

- Record the current clean worktree and direct-path baseline.
- Preserve and inspect the existing browser under-chunk activation capture.
- Lock the unsafe authored spawn, radius-zero product region, and nonblank-only
  browser acceptance with focused tests.
- Land this tactical and index entry.

### Slice 1 — Rectangular configurable preview bounds

- Normalize `EmbeddedChunkRegion` around inclusive chunk min/max bounds plus
  inclusive section min/max bounds.
- Keep center/radius construction as compatibility sugar.
- Add path-free scenario bounds configuration and resolve it around a runtime
  entry chunk/section.
- Exercise exact 2x2 and 4x4 regions, source-bounds conversion, culling,
  priority clamping, UI diagnostics, and activation-volume derivation.
- Prove the ordinary direct renderer remains byte/pipeline unchanged.

### Slice 2 — Destination source and recency policy

- Retain all catalog summaries in shared policy even when the UI displays only
  its capped first page.
- Select the most recent compatible world without invoking `OpenWorld`.
- Add an explicit catalog record-play operation whose successful response
  updates cached metadata without starting another session.
- Keep preview-open/warmup read-only with respect to recency and record play
  only after successful outbound activation.
- Cover empty, incompatible-only, tied, pending-list, cancellation, and stale
  completion cases.

### Slice 3 — Managed full-overworld fallback and storage routing

- Add a new managed scenario content version whose fallback destination is an
  empty persistent store with `WorldGenerationProfile::Overworld` and a
  curated seed; do not pre-author a terrain chunk.
- Route scenario starts through a neutral catalog-or-managed storage source.
- Native resolves catalog ids to user-world directories and managed keys to
  app-private scenario directories. Browser uses the corresponding IndexedDB
  world id with the same Rust scene/runtime policy.
- Keep the v1/v2 authored fixture recipes readable and keep managed fallback
  worlds excluded from the user catalog.

### Slice 4 — Accepted-entry-relative preview and supported arrival

- Resolve configured horizontal and vertical preview bounds after the standby
  accepts its authoritative startup pose.
- Center tabletop placement on the rectangular crop while retaining the safe
  entry pose separately for activation.
- Give authored-only fixture startup a center-seeking safe-spawn policy so the
  retained island regression fixture no longer arrives at its rim.
- Require loaded clearance/support before switchability and record post-swap
  entry/stability facts in activation diagnostics.
- Keep the return preview non-invasive and relative to the destination entry;
  never mutate a catalog world to build a return plinth.

### Slice 5 — Product and platform acceptance

- Native: empty-catalog fallback, recent-world preference, no-recency warmup,
  activation recency update, 2x2 and 4x4 preview, A-to-B-to-A, persistence, and
  stable supported landing.
- Browser WebGPU: the same desktop and mobile product cases through IndexedDB
  and dual Workers, including a delayed stability assertion after activation.
- Capture and inspect new lobby, 2x2/4x4 preview, first destination, stable
  destination, and return pixels under `/tmp`.
- Run focused crate tests, native flat/stereo smokes, browser desktop/mobile
  smokes, format/lint checks, and the applicable Android packaging lanes.
- Measure the direct single-world feature-off path against the accepted
  Tactical 179 control and reject an unacceptable unattributed regression.
- Update `docs/topics/embedded-worlds.md` with current state and evidence.

## Validation Requirements

Each implementation slice receives focused native tests and, when it produces
or changes pixels, production browser WebGPU validation in the same slice.
Rendered captures live only under `/tmp` and are inspected before proceeding.
Receipts must report resolved chunk bounds, section bounds, selected source
kind/id, recency-before/warm/activation values, accepted entry, post-swap
camera position, `on_ground`, collision support, and first-uncovered work.

The direct feature-off batch remains the performance control. Scenario-on cost
is reported separately by preview size so a 4x4 proof cannot hide its work in a
1x1 or feature-off number.

## Stop Conditions

Pause only if:

- catalog-world preview requires `OpenWorld` and therefore changes recency
  before activation;
- supporting adjustable bounds would require duplicating terrain/actor feature
  implementation in native and web;
- a catalog or managed storage route risks destructive migration or overwrite
  of valid user world data;
- generated-world anchoring requires a platform-specific scene fork;
- arrival cannot prove loaded clearance and support before uncovering;
- the direct single-world path has an unacceptable unattributed regression;
- new pixels are genuinely ambiguous after capture and inspection; or
- a new unresolved architecture choice would materially change the product
  contract above.

Routine slice transitions do not require approval.

## Completion Records

### Slice 0 — 2026-07-15

The pre-change worktree was clean at `8f25920f`. The retained Tactical 179
direct-path control is 2.271/3.614 ms median average/P95 natively and
13.3/18.5 ms in production browser WebGPU, with the direct image fixed at
SHA-256 `8ba561d8ef4dc376ec535cb224387c0bf41b04411485dd98943bee3c7110bc3b`.
That remains the comparison control for this tactical rather than spending a
new timing batch on documentation-only Slice 0.

The existing accepted browser lifecycle images
`/tmp/mclone-native-web-lobby-lifecycle-island.png` and
`/tmp/mclone-native-web-lobby-lifecycle-reopened-island.png` were inspected.
Both show the camera beside/below the authored terrain edge instead of a
supported arrival. The fixture declares feet at `(1.5, 65.0, 8.5)`, the
generic safe-spawn scan accepts the first suitable column, only the center
chunk contains island terrain, and `AuthoredOnly` makes surrounding misses
void. The lifecycle smoke asserted a completed swap, drawable sections, one
eye, and nonblank pixels, but did not retain camera/support facts. This is the
locked failure receipt for Slice 4 and Slice 5.

Focused pre-change validation passed:

- `cargo test -p mclone-render placement`: 10 passed;
- `cargo test -p mclone-scene warm_world`: 15 relevant tests passed across the
  library and ownership contract;
- `git diff --check`: clean.

The tactical and index now lock rectangular 2x2/4x4 configuration, most-recent
compatible selection, managed full-overworld fallback, recency-on-activation,
supported arrival, per-slice browser WebGPU acceptance, and direct-path gates.

### Slice 1 — 2026-07-15

`EmbeddedChunkRegion` now stores inclusive minimum and maximum chunk and
section coordinates. Center-plus-radius construction remains compatibility
sugar, while exact rectangular construction, width/depth diagnostics, bounds
conversion, containment, source-priority clamping, and even-sized 2x2/4x4
behavior use the canonical min/max model. Even spans keep scenario entry and
streaming interest pinned to the explicit entry chunk instead of adopting an
arbitrary geometric midpoint.

`ScenarioPreviewBounds` is a path-free, serialized launch setting with
validated inclusive offsets and a maximum span of 16 chunks per axis. The
product default is 2x2; the same browser entry point accepted a runtime 4x4
configuration without a native/web feature fork. Stable default intent JSON
remains `{"id":"lobby-preview"}`. Web diagnostics now expose exact horizontal
and vertical bounds, and the checked-in browser bounds lane fails unless the
live retained preview reports 4x4.

Focused validation passed:

- `cargo test -p mclone-render placement`: 11 passed;
- `cargo test -p mclone-app-runtime scenario`: 20 relevant tests passed;
- `cargo test -p mclone-scene warm_world`: 14 library tests plus the focused
  ownership exchange passed;
- the native-client live-diorama placement CLI test passed;
- `pnpm native:web:typecheck` and `git diff --check` passed.

Native `pnpm native:lobby-scenario:smoke` completed six captures and two slot
switches using the 2x2 product default. The new native lobby/preview/return
pixels under `/tmp/mclone-lobby-scenario-smoke` were inspected. Production
browser WebGPU `pnpm native:web:lobby-scenario-bounds-smoke` completed with
bounds `(-2,-2)..(1,1)`, 4x4 chunks, sections `3..5`, 48 bounded sections,
two drawn authored sections, zero out-of-region submissions, live actors, and
complete Worker shutdown. Its new preview pixels and JSON receipt are under
`/tmp/mclone-native-web-lobby-scenario-bounds-4x4*` and were inspected.

The first native absolute feature-off batch drifted above the Tactical 179
historical comparison, so it was not accepted without attribution. Five exact
interleaved current/control pairs against detached commit `4717c5bd` measured
2.568/4.532 ms candidate median average/P95 versus 2.554/4.552 ms control,
+0.55%/-0.44%, with no over-budget frames or accounting violations. Five
production browser feature-off samples measured 13.6/18.9 ms median compile
average/P95 versus 13.3/18.5 ms in the accepted control, +2.26%/+2.16%, with
two of two direct actors, one compiler Worker, shared-result transport, and
zero overflow. The direct native pixel witness was inspected and remains
byte-identical at SHA-256
`8ba561d8ef4dc376ec535cb224387c0bf41b04411485dd98943bee3c7110bc3b`.
No performance or architecture stop condition fired.

### Slice 2 — 2026-07-15

Shared catalog policy now retains every returned world summary while preserving
the existing eight-row UI cap as presentation only. Destination selection uses
the canonical catalog ordering to choose the most recently played compatible
world, including deterministic ties, and returns no selection for empty or
incompatible-only catalogs. A pending list is distinguishable from unrelated
catalog work so scenario routing can wait for authoritative selection instead
of prematurely choosing the fallback.

`RecordWorldPlayed` is a typed catalog operation on both native and web. It
validates and updates only catalog metadata with a monotonic timestamp; it does
not invoke `OpenWorld`, open the native SQLite world store, or start a browser
Worker runtime. The shared controller folds its completion into the full
catalog cache without emitting a session start. Cancelled and stale
completions cannot change cached recency. The scene issues this operation only
after a successful complete-slot switch whose destination descriptor names a
catalog world, leaving preview warmup read-only. Actual catalog destination
routing remains the next slice, so that hook is not yet reached by the current
authored-fixture product route.

Focused validation passed:

- `cargo test -p mclone-app-runtime client_catalog_policy`: 15 passed;
- `cargo test -p mclone-app-runtime world_catalog`: 19 passed;
- `cargo test -p mclone-app-runtime catalog_executor`: 7 passed;
- `cargo test -p mclone-scene catalog_activation` completed with no matching
  focused tests, while the affected scene crate compiled in the web and native
  lanes;
- `pnpm native:web:typecheck` and `git diff --check` passed.

The native catalog test removes the world's SQLite database before recording
play, proving that the metadata-only operation does not reopen or recreate the
world store. Production browser WebGPU `pnpm native:web:smoke` directly passed
the IndexedDB create/open/record-play/delete contract, including a strictly
newer record-play timestamp; `pnpm native:web:catalog-smoke` also passed the
menu-driven create/open/delete flow. New native direct, browser world, and
browser catalog UI pixels at `/tmp/mclone-180-slice2-native-direct.png`,
`/tmp/mclone-native-web-smoke.png`, and
`/tmp/mclone-native-web-catalog-ui-probe.png` were inspected.

Five native feature-off samples measured 2.582/4.514 ms median average/P95,
versus the Slice 1 accepted 2.568/4.532 ms current control
(+0.55%/-0.40%), with no over-budget frames or accounting violations. Five
production browser feature-off samples measured 13.4/18.6 ms median compile
average/P95, versus 13.6/18.9 ms in Slice 1 (-1.47%/-1.59%). Shared-result
transport remained active with one compiler Worker, no asset-pack resend, and
zero result overflow. The direct native witness remains byte-identical at
SHA-256
`8ba561d8ef4dc376ec535cb224387c0bf41b04411485dd98943bee3c7110bc3b`.
No performance or architecture stop condition fired.

### Slice 3 — 2026-07-15

Managed lobby content is now v3 under a new `lobby-preview-v3` root. Its
fallback destination is a persistent empty store for seed `12345` with the
ordinary `Overworld` generation profile; it contains no authored chunk or
entity payload. The v1 and v2 authored recipes remain readable and untouched.
Native publication reuses the destination SQLite store without replacing
generated world data, browser publication accepts an intentionally empty
IndexedDB payload, and neither managed identity enters the user catalog.

Scenario starts now carry one neutral `ScenarioWorldStorageSource`. Shared
scene policy selects either a catalog id or a managed key, while native maps
those ids to catalog or app-private directories and browser maps both to the
existing IndexedDB `worldId` Worker route. Destination provisioning is lazy
and occurs only when catalog selection resolves to fallback. Independent
outbound and return regions preserve the same bounds configuration when the
two sources occupy unrelated chunk coordinates. The next slice replaces the
remaining recipe entry hint with each runtime's accepted entry pose.

Focused validation passed:

- `cargo test -p mclone-app-runtime scenario_content`: 18 passed;
- `cargo test -p mclone-scene --test one_world_ownership_contract`: 13 passed,
  one GPU characterization ignored;
- `cargo check -p mclone-native-client`, `pnpm native:web:typecheck`, and
  `git diff --check` passed;
- `pnpm native:web:managed-scenario-storage-smoke` proved zero destination
  records, reuse, primary repair/refusal behavior, catalog exclusion, and
  complete Worker shutdown.

Native `pnpm native:lobby-scenario:smoke` completed six captures, two slot
switches, Quit/relaunch persistence, protected lobby authority, mutable
destination authority, and the full generated-world A-to-B-to-A route. The
receipt names `managed-overworld-fallback`, reports 49 loaded destination
chunks, and retains separate 2x2 regions across both coordinate spaces.
Production browser WebGPU `pnpm native:web:lobby-scenario-smoke` passed through
the same Rust scene path with `storageSourceKind=managed`, an empty provisioned
destination payload, two integrated-server Workers, one compiler Worker, live
generated terrain/actors, and zero Workers after shutdown. New native pixels
under `/tmp/mclone-lobby-scenario-smoke`, browser pixels under
`/tmp/mclone-native-web-lobby-scenario-desktop*`, and the fresh direct witness
`/tmp/mclone-180-slice3-native-direct.png` were inspected. The arbitrary
generated crop visibly demonstrates the already-documented hard-edge boundary
limitation and reports its explicit boundary warning; it is not mistaken for
boundary-aware remeshing.

Five native feature-off samples measured 2.575/4.376 ms median average/P95,
versus Slice 2's 2.582/4.514 ms (-0.27%/-3.06%), with zero over-budget frames
or accounting violations. Five production browser feature-off samples measured
13.6/18.8 ms median compile average/P95, versus 13.4/18.6 ms in Slice 2
(+1.49%/+1.08%). Every browser control retained two of two direct actors, one
compiler Worker, shared-result transport, and zero overflow. The native direct
pixel remains byte-identical at SHA-256
`8ba561d8ef4dc376ec535cb224387c0bf41b04411485dd98943bee3c7110bc3b`.
No performance, migration, platform-fork, or architecture stop condition
fired.

### Slice 4 — 2026-07-15

Preview layout now resolves only after each standby runtime accepts its
authoritative entry pose. The configured horizontal and vertical offsets are
converted into exact entry-relative regions, including centered even 2x2 and
4x4 crops, while the safe entry remains a separate activation value. The
lobby keeps its fixed tabletop anchor. After exchange, the non-invasive return
preview is rotated from the destination entry's local right/up/forward offset
without writing a plinth or otherwise mutating either world.

One shared `StandingPoseFacts` collision contract now reports loaded body,
clear body, loaded support, and solid support using the existing teleport
probes. Standby switchability requires all four facts at the accepted entry.
Activation receipts retain the accepted pose, immediate post-swap sample,
first uncovered sample, and an eight-frame stability sample, and fail before
or after uncovering if support is lost. Authored-only fixtures use a new
center-first spawn-column order; ordinary generated Overworld scanning is
unchanged. The retained authored Table and Island fixtures now accept
`(6.5, 64.0, 7.5)` and `(7.5, 66.0, 7.5)` respectively.

The browser fall was traced to browser glue relocating the camera again on
every active world-instance change after the shared scene had already applied
the accepted pose. That browser-only relocation was removed. Startup now
reconciles the pending authoritative camera correction before readiness, using
the same Rust scene/runtime contract as native. No native/web feature
duplication or platform-specific generated-world anchoring was introduced.

Focused validation passed:

- `cargo test -p mclone-client teleport --lib`: 12 passed;
- `cargo test -p mclone-server spawn --lib`: 53 passed;
- `cargo test -p mclone-scene warm_world --lib`: 14 passed;
- the one-world ownership contract passed 13 tests with one GPU
  characterization ignored;
- `cargo check -p mclone-native-client`, `pnpm native:web:typecheck`,
  `cargo fmt --all -- --check`, and `git diff --check` passed.

Native flat and stereo lobby smokes each completed two world exchanges. The
flat receipt keeps the generated accepted entry `(64.5, 72.0, -109.5)` and
the lobby entry `(6.5, 64.0, 7.5)` unchanged through first-uncovered and
eight-frame stability samples, with loaded clearance and solid support in
both directions. The stereo capture contains 217,988 differing eye pixels.
Fresh lobby, destination, return, and stereo pixels under
`/tmp/mclone-lobby-scenario-smoke` and
`/tmp/mclone-lobby-scenario-stereo-smoke` were inspected.

Production browser WebGPU
`pnpm native:web:lobby-scenario-lifecycle-smoke` passed the full failure,
cancellation, launch, persistence, A-to-B-to-A, reopen, asset replacement,
and resource-rebuild lifecycle. Its live generated preview resolved the 2x2
region `(3,-8)..(4,-7)`, drew eight bounded sections and four actors including
one remote player, and submitted nothing out of region. Outbound and reopened
generated arrivals preserved `(64.5, 72.0, -109.5)`, support, and `on_ground`
through delayed stability; return preserved `(6.5, 64.0, 7.5)` with support
and no displacement. All Workers were gone after shutdown. The new browser
preview, outbound, return, live-actor, and reopened pixels under `/tmp` were
inspected and are unambiguous.

Five native direct-path samples measured 2.602/4.416 ms median average/P95,
versus Slice 3's 2.575/4.376 ms (+1.05%/+0.91%), with no over-budget frames
or accounting failures. The initial browser comparison was rejected because
candidate and control compiled different mesh workloads. Five replacement
interleaved production WebGPU pairs compiled the identical 16-mesh workload
in every run (SHA-256
`80d0760340eeb27b96ee34509b2669372c39a6b31add7837097702cb3d615c99`).
They measured 11.8/20.6 ms candidate median average/P95 versus 13.9/19.7 ms
control (-15.1%/+4.6%), with one compiler Worker, shared-result transport,
and zero overflow. The fresh native direct pixel
`/tmp/mclone-180-slice4-native-direct.png` was inspected and remains
byte-identical at SHA-256
`8ba561d8ef4dc376ec535cb224387c0bf41b04411485dd98943bee3c7110bc3b`.
No performance, support, visual, migration, platform-fork, or architecture
stop condition fired.

### Slice 5 — 2026-07-15

The product acceptance matrix now has first-class catalog-backed native and
browser lanes rather than inferring recent-world behavior from policy tests.
The native 4x4 lane creates two ordinary catalog worlds, makes one uniquely
most recent, launches the shared lobby scenario, verifies that warmup leaves
both timestamps unchanged, activates the selected complete world, and proves
that only its play timestamp advances. Its receipt resolves chunks
`(2,-9)..(5,-6)`, sections `3..5`, records source
`catalog-recent-world`, and completes cancellation, persistence, relaunch, and
A-to-B-to-A with supported stable entry. The existing native lane remains the
empty-catalog 2x2 managed-overworld fallback proof.

Adding the integrated catalog lane exposed a real lifecycle race: destination
renderer-shell preparation could precede replacement of the initial primary
slot, whose transient-state reset then discarded that shell. Both native and
browser now prepare the shell at the shared post-lobby destination-start
boundary. The fix is entirely in `McloneSceneHost`; adapters still only resolve
native directories or browser IndexedDB/Worker starts, and no duplicate
native/web feature path was introduced.

Production browser WebGPU now runs actual outbound and return activation in
the desktop, mobile/touch, catalog desktop, catalog mobile/touch, and 4x4
bounds lanes. Both catalog lanes create two worlds through the native UI and
IndexedDB catalog path, choose the most recently actively played compatible
world, preserve both recencies through preview warmup, and advance only the
selected row after activation. Managed desktop/mobile accept
`(64.5,72.0,-109.5)`; catalog desktop/mobile accept
`(-111.5,64.0,-68.5)`; every outbound delayed-stability receipt retains loaded
clearance, solid support, and `on_ground=true`. Every return retains supported
`(6.5,64.0,7.5)` without displacement. The adversarial browser lifecycle lane
also passed after the shell-timing correction, including cancellation, failure,
relaunch, asset replacement, renderer rebuild, and complete Worker shutdown.

Fresh native lobby, 2x2/4x4 preview, destination, return, and synthetic-stereo
pixels under `/tmp/mclone-lobby-scenario-*` were inspected. Fresh production
browser desktop/mobile, managed/catalog, 2x2/4x4, destination, and return pixels
under `/tmp/mclone-native-web-lobby-scenario-*` were also inspected. They show
bounded tabletop worlds and fully rendered supported destinations; none repeat
the below-world Slice 0 failure.

Focused and platform validation passed:

- catalog policy, world-catalog, and catalog-executor tests: 15, 19, and 7
  passed;
- scene warm-world tests: 14 passed; the one-world ownership contract passed
  13 with one GPU characterization ignored;
- the three native lobby-smoke CLI parser tests passed;
- native flat fallback, native 4x4 catalog, and native synthetic-stereo smokes
  each completed two exchanges; the stereo image retained 218,058 differing
  eye pixels;
- production browser WebGPU desktop, mobile, catalog desktop, catalog mobile,
  4x4 bounds, and lifecycle smokes passed;
- `cargo check -p mclone-native-client`, `pnpm native:web:typecheck`, Node
  syntax checking, Rust formatting, and `git diff --check` passed;
- focused no-dependency Clippy passed for `mclone-scene` and the native client;
  the client invocation allows its two pre-existing denied
  `lod_settle_probe` extreme-comparison lints, while a broader all-targets
  audit remains blocked by unrelated pre-existing `mclone-mesh` and
  `mclone-worldgen` denied lints;
- flat Android debug and Android XR release APK packaging passed.

Five native direct-path controls measured 2.581/4.409 ms median average/P95,
versus Slice 4's 2.602/4.416 ms (-0.81%/-0.16%), with zero over-budget frames
and zero accounting violations. Five production browser controls measured
11.8/20.5 ms median compile average/nearest-rank P95, versus Slice 4's
11.8/20.6 ms. Every browser run retained 16 movement compile timings, two of
two direct actors, one compiler Worker, shared-result transport, one asset
payload, and zero overflow. Generated compile-byte sequences varied between
runs, so this closeout does not claim an exact byte-identical browser workload
pair. The native direct image was inspected and remains byte-identical at
SHA-256
`8ba561d8ef4dc376ec535cb224387c0bf41b04411485dd98943bee3c7110bc3b`.
No performance, support, visual, migration, platform-fork, or architecture
stop condition fired. Tactical 180 is complete.
