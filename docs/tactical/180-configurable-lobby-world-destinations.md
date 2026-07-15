# Tactical 180 — Configurable Lobby World Destinations

Status: active

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
