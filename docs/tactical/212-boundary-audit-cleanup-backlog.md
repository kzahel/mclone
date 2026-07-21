# Tactical 212: Boundary Audit Cleanup Backlog

Status: complete 2026-07-21. All mandatory implementation Slices 0–6 are
complete; optional Slice 7 was deliberately skipped at closeout. The parent
concern remains open for the independent Phase 9 audit. This is the execution
record for the remaining work appended by the Phase 7 audit
([`211-platform-boundary-fixpoint-audit.md`](211-platform-boundary-fixpoint-audit.md)).
It is a bounded cleanup series, not a new campaign: every slice below has
a named target, a measured motivation from the audit, and its own exit
gate. Do not widen scope beyond these slices without appending the
justification here first.

Topic: `platform-boundary-convergence`

Parent concern:
[`../topics/platform-boundary-convergence.md`](../topics/platform-boundary-convergence.md).
This tactical may close itself; it must not declare the parent done. On
close it must append a pass-ledger row and a full scoreboard column
there. Closure of the parent afterwards requires the separate Phase 9
audit.

Commit trailers: use `Topic: platform-boundary-convergence` on every
commit in this series (already registered in `topics.md`). Add
`Topic: cross-platform-operation-execution` on slices that change the
operation machinery itself (Slices 2, 5, 6).

Line references are as of the 2026-07-21 audit. Re-locate by symbol name
if they have drifted; do not assume they are current.

## Ground Rules (from the parent topic)

- Report the full scoreboard (all rows, before/after) at closeout, using
  the measurement methods below. Explain any temporary growth.
- Relocation slices (notably Slice 3) are judged on ownership
  correctness, not net deletion. The combined boundary metric counts
  authored TypeScript plus `mclone-web-client/src`, so moving policy into
  shared crates *reduces* the combined boundary while growing the shared
  crate — report both honestly.
- Every slice that changes web behavior validates through the headed
  Wayland lane (`pnpm host:check` first; never accept headless-Chrome
  black/transparent captures as evidence — see `docs/native-web.md`).
- One slice active at a time; each lands as its own reviewable commit
  series with its own tests.

## Measurement Methods

Run from the repo root unless noted. These reproduce the scoreboard:

```bash
# authored web TypeScript lines
cd native/apps/mclone-web-client && \
  find www -name '*.ts' ! -name '*.d.ts' | xargs wc -l | tail -1

# web-only Rust lines
find src -name '*.rs' | xargs wc -l | tail -1   # in mclone-web-client

# combined boundary = the two numbers above summed

# shared crate lines
find native/crates/mclone-scene/src -name '*.rs' | xargs cat | wc -l
find native/crates/mclone-app-runtime/src -name '*.rs' | xargs cat | wc -l

# cfg forks (scoreboard method: negative gates only)
grep -rE 'cfg\(not\(target_arch = "wasm32"\)\)' \
  native/crates/mclone-scene/src --include='*.rs' | wc -l
grep -rE 'cfg\(not\(target_arch = "wasm32"\)\)' \
  native/crates/mclone-app-runtime/src --include='*.rs' | wc -l

# WebSceneHost export count: pinned by
# native/apps/mclone-web-client/tests/platform_boundary_convergence_debt.rs
```

## Slice 0: Resolve the DB-version decision (D1) — complete 2026-07-21

Tactical 207's trailing cleanup — deletion of the legacy IndexedDB
stores and the v5-to-v6 Overworld migration
(`www/mclone-web-world-catalog.ts`, `src/web_catalog_execution.rs`, the
two lock tests) — landed after the audit as commit `5bf2ffa2`
(`Topic: world-dimension-storage-layout`), together with the
headed-Wayland capture guidance. That commit did **not** resolve D1:

`WORLD_DB_VERSION` is still 6, so a browser profile that opened the old
internal schema may retain empty or disposable `chunks`/`entityChunks`
object stores. This is accepted deliberately: the product is unreleased,
there are no known web-world preservation consumers, and the compatibility
safety ledger classifies internal worlds as disposable unless a concrete
consumer is recorded. Commit `5bf2ffa2` intentionally deleted the runtime
migration and added source locks preventing the legacy store vocabulary from
returning. Bumping to 7 solely to delete stores from obsolete development
profiles would recreate a one-off legacy accommodation with no product
benefit. Developers with an old profile may clear site data.

This decision introduces no runtime code and leaves version 6 as the current
schema identifier, not as a promise to support any earlier schema.

Exit gate: D1 resolved explicitly here and in the Slice 0 commit message; the
existing lock tests continue to reject legacy migration/store policy. No code
or lock-test update is required.

## Slice 1: Deduplicate the catalog effect apply loop — complete 2026-07-21

Motivation: audit F2. The wasm branch of
`poll_external_catalog_operations`
(`native/crates/mclone-scene/src/session.rs:5754-5790`) re-implements
the native `apply_xr_catalog_effects` loop, and the two copies have
already drifted (`commit_render_state` present in one, absent in the
other). The only genuine platform fork is per-start scene construction:
native builds a persistent per-world scene under `world_root.join(id)`
(`:5668`); wasm clones the active scene and nulls `world_dir` (`:5677`).

Work:

1. Extract per-start scene construction into one helper whose *body* is
   the only cfg-forked code.
2. Make the apply loop arch-neutral and call it from both paths; delete
   the duplicated wasm block.
3. Resolve the `commit_render_state` drift deliberately: determine which
   behavior is correct, apply it to both arches, and note the decision
   in the commit message.

Exit gate: one shared loop; the duplicated block is deleted (~40 lines);
`cargo test -p mclone-scene` green; a web catalog smoke
(`pnpm native:web:catalog-smoke`) passes headed; scoreboard cfg delta
reported.

Completion evidence: catalog session scene construction is isolated in one
helper with cfg-forked bodies, while both deferred and immediate paths use the
same effect loop. The deferred path retains the immediate render-state commit
on both architectures because completions arrive between frames and input must
observe the new projection without waiting for presentation. Scene Rust is
24,731 → 24,714 lines and the scoreboard's negative-wasm cfg count is 71 → 70.
All 125 scene unit tests passed. The headed catalog lane passed with an
inspected world-list capture after its progress sample was made cadence-aware:
it now waits for an observed input-bearing frame instead of assuming two such
frames fit inside a fixed 50 ms window.

## Slice 2: Converge frame-timing accounting on MonotonicClock — complete 2026-07-21

Motivation: audit F3, and a live behavior fix — web frame-pipeline
accounting currently reads `0.0`.

Targets:

- `native/crates/mclone-app-runtime/src/lib.rs:2981-3005`
  (`RuntimeTimingSample`, `timing_start`, `timing_elapsed_ms` — wasm
  arms return `None`/`0.0`).
- `client_connection.rs:303-330` (`PumpTimingSample`),
  `frame_render.rs:33-57` (`CompositionTimingSample`),
  `far_lod.rs:67-77` — three independent Instant-vs-`js_sys::Date` shims.
- Replacement already exists: `monotonic.rs`
  (`MonotonicClock`/`MonotonicClockHandle`/`ProjectedMonotonicClock`),
  which the web client already injects via `web_scene_protocol.rs`.

Work: thread a `MonotonicClockHandle` (or equivalent) into these call
sites and delete the ad-hoc shims. Some are free functions; expect to
pass the handle through their callers rather than adding globals.

Exit gate: the ad-hoc timing shims are gone (~17 fewer cfg forks across
both methods of counting); web timing produces nonzero elapsed values,
verified through an existing diagnostic/smoke probe under the headed
lane; native timing values unchanged in kind; `cargo test` for both
crates green.

Completion evidence: `SingleViewRuntime`, connection pumping, far-LOD
aging, and timed frame composition now consume injected
`MonotonicClockHandle`s. The browser adapter supplies a live
`performance.now()` clock while retaining the rAF timestamp projection as
its monotonic floor; native defaults continue to use `Instant`. The four
ad-hoc timing families and their wasm zero/Date arms are gone. App-runtime's
negative-wasm cfg count is 110 → 98 and its full `target_arch = "wasm32"`
marker census is 133 → 112. The focused injected-clock test passed, as did
the full 279 app-runtime and 125 scene tests, and the wasm build passed. The
headed catalog lane reported `frameRenderViewsMs: 0.395` (and asserted it was
positive); its inspected capture contained the expected one-row world list
with no black or transparent pixels. This slice grows app-runtime by 26 lines,
scene by 3, and web-only Rust by 29 for explicit clock ownership and the
diagnostic pin; Slice 3 remains the planned boundary-mass reduction.

## Slice 3: Hoist CatalogExecutionCore into shared Rust — complete 2026-07-21

Motivation: audit F1 — the largest single mass of policy living in
web-only Rust (~800 lines), and the direct answer to the parent topic's
finding that campaign policy landed web-side.

Targets: `native/apps/mclone-web-client/src/web_catalog_execution.rs`
lines 24–868 as of the audit — `StorageStore`, `StorageMode`,
`StorageActionKind`, `StorageTransaction`, `CatalogFlow`,
`CatalogExecutionCore` (step sequencing, read-then-write separation,
duplicate-create rejection, active-world delete protection,
factory-reset ordering, `clear_world_transactions`) and their unit tests
(`:1232-1508`).

Work:

1. Move the storage-plan types and `CatalogExecutionCore` into
   `mclone-app-runtime` (suggested module: `catalog_storage_plan`,
   sibling to `catalog_executor.rs`). The moved code must stay free of
   `wasm-bindgen`/`js_sys`/`web_sys` types.
2. Keep only JsValue encode/decode web-side (`encode_storage_step`
   `:1064`, `encode_storage_action` `:1088`, and the decoders), plus the
   `WebCatalogExecution` wasm wrapper.
3. Move the unit tests with the code; keep a thin web-side test proving
   the JsValue encoding round-trips.
4. No behavior change of any kind. This is a relocation slice: its gate
   is ownership, and the combined boundary metric should drop by roughly
   the moved mass while `mclone-app-runtime` grows correspondingly.

Exit gate: `mclone-web-client/src` shrinks by ~700–800 lines; moved
tests green in the shared crate; wasm build and
`pnpm native:web:catalog-smoke` (headed) pass; scoreboard reports the
relocation honestly (web-only Rust down, app-runtime up, combined
boundary down).

Completion evidence: `CatalogExecutionCore`, its typed storage plans, and all
nine policy/state-machine tests now live in
`mclone-app-runtime::catalog_storage_plan`. The browser adapter supplies its
backend label and maps shared world identities to browser writer-lease names;
the remaining web module only encodes plans, decodes IndexedDB rows, and owns
the wasm wrapper/smoke constructor. The source lock now pins that ownership
split. `web_catalog_execution.rs` is 1,510 → 349 lines, total web-only Rust is
20,595 → 19,434 (-1,161), and app-runtime is 33,903 → 35,076 (+1,173); the
combined TypeScript-plus-web-Rust boundary is 24,150 → 22,989 (-1,161).
Native and wasm checks passed, the nine relocated tests and both web ownership
lock suites passed, and the headed catalog lane exercised the real
plan-encode/IndexedDB-read/decode continuation through create, open, record,
and delete. Its inspected final capture was the expected one-row world list
with no black or transparent pixels.

## Slice 4: Remove test scaffolding from the production ABI and widen the pins — implementation complete 2026-07-21

Motivation: audit F4 and G2.

Work:

1. Move the five smoke-only `WebSceneHost` exports off the production
   surface: `rebuildRenderResourcesForSmoke`,
   `renderHalfSpaceTerrainProof`, `renderPreparedFigureProof`,
   `renderActorCompositionProof` (`web_scene_host.rs:476-841`, ~365
   lines) and `beginLobbySmokeWithChunkSpan` (`:1779-1793`, which also
   bypasses the drain by calling `begin_lobby_launch` directly). Two
   acceptable shapes — pick one and record why:
   - a separate exported smoke type (e.g. `WebSceneSmokeHarness`)
     wrapping host access, constructed only by the smoke pages; or
   - a cargo feature enabled only for a distinct smoke bundle, if the
     smoke runner can build/serve its own bundle without forking the
     production build path.
   Constraint: all smoke lanes must keep passing; the production bundle
   must not regress.
2. Split the smoke constructor off `WebCatalogExecution`
   (`web_catalog_execution.rs:884, 900-903, 1007, 1018-1062`): make the
   production `token` non-optional and `token()` infallible; relocate
   `responseForSmoke` and `mclone_web_catalog_smoke_execution` to the
   smoke surface.
3. Widen the export pins in
   `tests/platform_boundary_convergence_debt.rs`: pin the export counts
   of `WebSceneOperation` and `WebCatalogExecution` (and the new smoke
   type, if any) alongside `WebSceneHost`, and update the `WebSceneHost`
   pin from 42 to the new count with the justification in the test.

Exit gate: production `WebSceneHost` ABI at ~37 mechanical exports;
sibling-class pins in place; no `Option` token or dead error branch in
production catalog execution; all headed smoke lanes green.

Implementation evidence: a separate `WebSceneSmokeHarness`, instantiated only
by the query-gated smoke observer, now owns the four render/resource probes and
the direct lobby launch probe. `WebSceneHost` fell from 42 to 37 mechanical
exports. `WebCatalogSmokeExecution` now owns tokenless smoke construction and
response encoding; production `WebCatalogExecution` has an always-present token
and six pinned exports. Exact pins cover `WebSceneOperation` (3),
`WebCatalogExecution` (6), `WebSceneSmokeHarness` (6), and
`WebCatalogSmokeExecution` (8). Native tests, wasm check, TypeScript checking,
adapter/purity locks, and headed catalog, prepared-figure, half-space,
actor-composition, and lobby-bounds lanes passed. Their captures were inspected
and contained the expected non-black, non-transparent output. The prepared
figure lane also exposed and corrected its stale `box-v0` compiler-ID pin to
the already-current `cuboid-proxy-v1` contract.

The aggregate lobby lifecycle lane was attempted twice. Both runs completed
the main lifecycle scenarios and then timed out in the asset-pack replacement
wait, before reaching the resource-rebuild subcase. Slice 5 traced that timeout
to stale disposable generated asset packs (209 visual entries against the
current 221-entry registry), regenerated and strictly validated those packs,
and reran the lane. Both the live asset-replacement and resource-rebuild
subcases now pass and their captures were inspected. The aggregate lane still
reports false solely through the unchanged actor-ID/age persistence fixture
already carried as separate baseline debt by Tactical 207; it reached and
passed both boundary subcases on two consecutive runs.

## Slice 5: Collapse rim in-flight guards onto the ledger — complete 2026-07-21

Motivation: audit F5 — the last identity-adjacent duplication.

Targets:

- `catalog_operation_in_flight: bool`
  (`web_scene_host.rs:463`; gate `:1681`, set `:1690`, clear `:1748`).
- `asset_pack_preparation_in_flight: Option<PlatformOperationToken>`
  (`:445`; gate `:1670, :1881`, set `:1909`, staleness compare `:1933`,
  clear `:1939`).
- Ledger capabilities that already model this:
  `PlatformOperationLedger::pending_len`
  (`mclone-app-runtime/src/platform_operation.rs:139`) and
  `PlatformOperationResolution::{Stale, Duplicate}` (`:216-217`).
- Also: drop or relocate the vestigial `render_resource_generation`
  counter (`web_scene_host.rs:464`, written only by the smoke rebuild at
  `:492`, read only as a diagnostic at `:2775`) — moves naturally with
  Slice 4's smoke surface.
- Optional, judgment call: relocate the descriptor-to-runner-config
  policy in `lower_runtime_start` (`:1817-1871`) into a shared builder,
  leaving web to inject only the four `*_url` fields. Skip if it nets
  combined-boundary growth.

Exit gate: both rim fields deleted; re-entry, duplicate, and stale
rejection covered by ledger resolutions with tests; no behavior change
observable from TypeScript.

Completion evidence: `catalog_operation_in_flight` and
`asset_pack_preparation_in_flight` are deleted. Shared scene services expose
their ledger `pending_len`, while their deferred platform handles provide a
one-shot take; browser Rust no longer mirrors operation identity or compares
tokens. A shared-operation test covers one-shot dispatch, re-entry while the
ledger remains pending, applied completion, duplicate completion, and stale
completion after epoch teardown. Source locks require the shared pending/take
calls and pin both deleted rim fields at zero occurrences.

The wasm build and the complete app-runtime, scene, and web-client test suites
passed. The focused headed asset-replacement probe advanced the content epoch
from 0 to 1, selected authored-only assets, cleared the active lobby launch,
and rendered non-black/non-transparent output. Two full headed lifecycle runs
then reached and passed both the asset-replacement and resource-rebuild
subcases; the inspected captures showed the active world before and after the
resource rebuild. Their aggregate verdict remained false only because the
known separate actor-persistence fixture regenerated entity IDs and ages on
world reopen, the same baseline debt recorded at Tactical 207 closeout. No
TypeScript product behavior changed; the only browser-script addition is a
focused smoke selector and failure-state diagnostic for this boundary.
Web-only Rust is 19,576 → 19,555 (-21), while shared scene Rust is
24,717 → 24,730 (+13) and app-runtime is 35,076 → 35,111 (+35) for the
shared queries and ledger proof. Authored TypeScript remains 3,593, so the
combined boundary is 23,169 → 23,148 (-21).

## Slice 6: Behavioral fixpoint demonstration — complete 2026-07-21

Motivation: audit G1 — the closure protocol requires the fixpoint
"demonstrated, not asserted", and the current proof is compile/grep
only.

Work: add a wasm test (the existing wasm test target that already
compiles `TestOnlyRemote`) that constructs a
`WebRuntimeStartEffect::TestOnlyRemote` operation, drives it through
`take_scene_operation -> start -> complete_scene_operation`, and asserts
the generic drain serviced it and the completion folded — with zero
TypeScript involvement by construction. Keep the existing source locks.

Exit gate: the runtime test exists and passes in the wasm test target;
`platform_boundary_convergence_debt.rs` still pins the source facts.
This is the evidence the Phase 9 audit will check first.

Completion evidence: the production host's take and complete exports now
delegate their operation-type-blind wrapping/consumption to
`WebSceneOperationDrain`. A wasm-bindgen runtime test constructs a
`TestOnlyRemote` runtime effect, takes it through that same core, awaits the
ordinary `WebSceneOperation::start` Promise capability, completes it through
the same core, and folds its deterministic result through the shared
`PlatformOperationLedger`. The test asserts the expected failed resolution and
an empty ledger. It passed under the pinned wasm-bindgen test runner:

```bash
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=\
native/target/wasm-bindgen-cli-0.2.125/bin/wasm-bindgen-test-runner \
cargo test --manifest-path native/Cargo.toml \
  --target wasm32-unknown-unknown -p mclone-web-client --lib \
  test_only_coarse_operation_runs_through_generic_drain
```

The existing source lock now requires the runtime test and both shared drain
calls while continuing to assert zero TypeScript references. No product wasm
export was added; the widened exact export pins remain 37 / 3 / 6 / 6 / 8.
The production wasm check and full 74-test web-client suite passed. A headed
catalog smoke then exercised the refactored drain, reported a nonzero
`frameRenderViewsMs`, and produced an inspected non-black/non-transparent
world-list capture. This behavioral proof deliberately adds 81 lines of
web-only Rust (19,555 → 19,636) and leaves TypeScript at 3,593, so the combined
boundary is 23,148 → 23,229; the measured runtime evidence is the gain that
justifies the local growth.

## Slice 7 (optional, mechanical): file split and micro-cleanup

Do these only after Slices 1–6; they are hygiene, not convergence.

1. Split `mclone-scene/src/session.rs` (6,606 lines; one
   `impl McloneSceneHost` block spanning `:279-5888`, ~174 methods) into
   sibling `impl` extension files along the visible clusters: local
   startup/session lifecycle, lobby launch (~`:1200-2050`), warm-world
   standby, embedded scenarios, storage-profile/local-data UI, XR effect
   application. Zero API change; pure relocation. Do Slice 1 first — it
   edits the XR-effects cluster.
2. TypeScript micro-cleanup (~70 lines): delete dead `finiteInteger`
   (`mclone-web-app.ts:744-747`); delete the dead
   `mclone-render-compiler-shared.ts` exports
   (`renderCompilerSharedMemorySupported`, `byteLengthOf`,
   `isSharedArrayBuffer` — only `fetchAssetPack` is live); consolidate
   the six `stringifyError` copies into one shared mechanical util;
   dedupe the near-identical IndexedDB promise wrappers
   (`mclone-web-world-catalog.ts:357-370` vs
   `mclone-web-persistence-executor.ts:443-460`).

Exit gate: behavior-neutral; `pnpm native:web:typecheck` and the lock
suites green; counts reported.

Closeout disposition: skipped. Neither the large mechanical file split nor
the unrelated TypeScript micro-cleanups are required by an audit finding or a
closure criterion. Starting them after all mandatory gates passed would widen
the review surface without strengthening the Phase 9 evidence. They may be
picked up later as independent hygiene if they become locally useful.

## Closeout

Mandatory Slices 0–6 are complete. Relative to the Phase 7 audit column,
authored web TypeScript is 3,555 → 3,593 (+38), web-only Rust is
20,566 → 19,636 (-930), and the combined boundary is 24,121 → 23,229
(-892). The TypeScript growth is query-gated smoke observation for the ABI
split, while the web-Rust reduction is principally the 1,161-line catalog-plan
ownership move. Shared scene Rust is 24,731 → 24,730 (-1) and shared
app-runtime is 33,877 → 35,111 (+1,234), primarily because that catalog policy
and its tests now live at their shared owner.

The exact production/smoke export pins are 37 / 3 / 6 / 6 / 8, async mutable
wasm exports remain zero, the two rim guards are gone, and the scoreboard's
negative-wasm cfg counts are 70 / 98. Headed operation traces still advance
active frame/render/input counters. Slice 6 supplies the previously missing
runtime behavioral fixpoint proof.

What remains for the parent concern is the separately executed Phase 9 audit,
not more implementation in this tactical. That reviewer must freshly read the
code and validate the closure protocol before changing the parent status. The
unchanged actor-ID/age lifecycle fixture remains separately recorded baseline
debt; it does not invalidate the operation-specific asset replacement,
resource rebuild, cancellation, quiescence, or shutdown evidence. No legacy
web-world migration or obsolete-schema accommodation remains or is planned.

## Ordering

Slice 0 is complete. Continue 1 → 2 → 3 → 4 → 5 → 6 → 7. Slices 1–3 are
independent of 4–6 and may be reordered among themselves; Slice 5's
`render_resource_generation` item and Slice 4's smoke surface interact, so
land 4 before 5 or combine those two commits. Slice 7 is optional and last.

## Validation Matrix

Per slice, minimum:

- `cargo test` in `native/` for the touched crates; full workspace test
  before closeout.
- Wasm build and test-target compilation for web-touching slices.
- `pnpm native:web:typecheck`, `pnpm --silent native:web:scene-adapters`,
  `pnpm --silent native:web:worker-ownership`,
  `pnpm --silent native:web:scene-host-adoption`,
  `pnpm native:thin-adapters:purity`.
- Headed Wayland pixel/semantic lanes for web-behavior slices
  (`pnpm host:check` first; catalog/asset/lobby lanes as relevant).
- Native compile controls for shared-crate slices (desktop build; the
  platform matrix in `docs/platforms.md` at closeout).

## Explicit Non-Goals

- No Slice 5b storage-address lowering: the TS-authored IndexedDB schema
  tables (`mclone-web-world-catalog.ts`,
  `mclone-web-persistence-executor.ts` `NAMESPACE_SPECS`) stay where
  they are. Reopen only with a measured net-deletion case driven by real
  schema work.
- No retirement of native's synchronous session-start pump onto the
  neutral external-operation seam (`session.rs:931/942, 1555, 5183`,
  `SceneLocalStartup`). Recorded as optional future work; requires a
  concrete motivating defect or feature, not uniformity.
- No new campaign around cfg-fork counts or web-Rust line mass; the
  audit classified ~85% of forks as legitimate mechanics.
- No changes to the actor-ID/age persistence fixture; it is separately
  recorded baseline debt owned outside this concern.
- No universal actor, no new coordinator, no product behavior changes.

## Stop Conditions

Stop and reassess (append here before continuing) if:

- a slice completes with a combined both-language increase and no
  measured behavioral gain;
- a consolidation turns into a facade — old paths compiled in behind
  delegation instead of deleted;
- the smoke-surface split (Slice 4) forces a fork of the production
  build path or regresses any smoke lane;
- Slice 3's moved code needs a wasm-only type to compile — that means
  the policy/mechanics cut was drawn in the wrong place; or
- any change requires TypeScript to learn an operation's engine meaning.

## Closeout Requirements

1. Append a pass-ledger row and a full scoreboard column to the parent
   topic, with before/after for every row and honest notes on
   relocations.
2. Update the affected child topic docs if any contract changed
   (expected: none — these are cleanups within existing contracts).
3. Recommend opening the Phase 9 audit; do not declare the parent
   closed.
