# Tactical 310: Shared Local-Session Launch Semantics

Status: implementation complete 2026-08-16; parent topic remains reopened for
the separately staffed independent fixpoint audit required by its closure
protocol

Topic: `platform-boundary-convergence`

## Instruction Synthesis

Correct the reproduced browser-only Mclone Wild entry defect at its ownership
boundary. Creating the same local world from the shared in-game menu must
produce one host-neutral launch plan before native threads, browser Workers,
filesystem/SQLite, or IndexedDB resources are selected.

Do not patch the browser with another call to the spawn selector and leave the
parallel builder chains in place. Audit every policy-bearing input that crosses
local-session construction, classify genuine platform mechanisms explicitly,
and make silent omission structurally difficult. Add semantic conformance and
real menu-flow gates that fail when a platform defaults, ignores, or
reinterprets shared launch intent.

Do not change generated terrain, the Mclone Overworld spawn-search algorithm,
the accepted Homestead scout origin, or persistence formats in this tactical.

## Starting Evidence

Commit `92f6f5cb` (`Default new worlds to inland Mclone starts`, 2026-08-15)
correctly strengthened the shared `mclone-overworld-v1` selector and server
admission. It did not change the browser runtime-start lowering.

The browser defect reproduces with Mclone Overworld seed
`553534047293117028`:

- shared selection resolves chunk `(-48, 20)`, whose center has a dry surface
  at Y=72;
- the raw scene center near `(0, 0)` has a water-covered surface at Y=57;
- a production Web Worker launch requested the raw center, the authority
  accepted it, and the camera appeared underwater; and
- the intended selector itself remained dry across the investigation corpus.

The exact control flow is:

```text
shared menu/catalog policy
  -> project seed/profile/starter content into McloneSceneHostOptions
  -> mark the procedural entry as profile-preferred
  -> queue one ExternalSceneSessionStart

native local execution
  -> mclone-scene::local_integrated_scene_options
  -> resolve profile-preferred center
  -> construct native runner

browser local execution
  -> web-client::lower_runtime_start
  -> copy pending.scene.center() directly
  -> construct Web Worker runner
  -> send SetChunkView at the copied center
```

Wild player admission accepts the requested view. Homestead has a separate
realm-primary route, which is why that starter did not expose the same entry
defect.

The August 15 server regression test starts at a center it computes and
injects itself. It proves selector and admission behavior, not that a product
host selected that center. The browser catalog smoke also cycles the default
profile before its first ordinary create, so it never creates the default
Mclone Wild combination it was intended to protect.

## Audit Scope And Method

The audit traced all current local-session entry shapes rather than searching
only for the failing boolean:

- initial explicit launch from argv, Android properties, or browser query;
- shared in-game Create World and Open World actions;
- transient, filesystem/SQLite, and IndexedDB worlds;
- ordinary Wild, Intro Homestead, authored fixtures, playable showcases, and
  retained lobby destinations;
- native flat/offscreen, flat Android, desktop XR, Android XR, and Web Worker
  runtime construction; and
- non-default policy values in `StartupSceneOptions`,
  `McloneSceneHostOptions`, `LocalIntegratedSceneOptions`, native runner
  configuration, and `WebIntegratedServerRunnerConfig`.

The audit distinguishes three categories:

1. shared semantics that must be identical before platform binding;
2. platform mechanisms that may differ but must consume a typed shared
   request or report an explicit normalization; and
3. scene-resident presentation policy already consumed by `mclone-scene` and
   therefore not part of local-authority lowering.

Defaults are not evidence of propagation. A value was counted as applied only
when a non-default sentinel could reach the final consumer or a final
configuration/receipt exposed the value.

## Pre-Implementation Audit Findings

### A. Local entry intent is represented by a late boolean

`McloneSceneHostOptions::use_initial_spawn_center` means “replace the stored
scene center with the descriptor's preferred procedural center when building
a local authority.” The same record also carries raw `chunk_x/chunk_z`. That
leaves two competing centers alive until platform lowering.

The shared ordinary-world projection sets the boolean correctly, but only the
native lowering reads it. The Web lowering reads the raw center and never
reads the boolean. The browser therefore inherits the title/lobby/previous
scene center, commonly `(0, 0)`, instead of resolving the new world.

Initial launch is inconsistent even before menu replacement:

| Entry path | Current center interpretation |
|---|---|
| Desktop explicit local launch | always preserves raw startup center because the desktop host sets the boolean false |
| Flat Android / Android XR explicit local launch | defaults to profile-preferred because `from_startup_scene` leaves the boolean true |
| Web explicit local launch | preserves the raw center because the Web host sets false, and later ignores the flag in all cases |
| Native in-game Create/Open | profile-preferred for procedural profiles |
| Web in-game Create/Open | raw inherited scene center despite profile-preferred intent |
| Authored fixture/showcase | explicit authored center is intended and must remain exact |
| Reopened persistent world | persisted player position should win; the initial view still needs a deliberate typed fallback rather than an unrelated active-scene center |

The boolean also loses provenance: code cannot tell whether `(0, 0)` was an
explicit requested coordinate, a neutral parser default, an authored entry, a
fallback for a persisted player, or stale state cloned from the active scene.

### B. Native and Web independently enumerate authority semantics

Native construction goes through
`mclone-scene::local_integrated_scene_options`, then through
`mclone-app-runtime::native_runner_config`. Browser construction independently
builds `WebIntegratedServerRunnerConfig` in `lower_runtime_start`.

The browser chain currently carries seed, generation profile, starter content,
topology, world behavior, fluid freeze, day time, time freeze, passive
showcase, auxiliary-player script, observer status, and storage selection.
Each field is copied individually and absent values silently take a Web config
default.

That shape has already produced a second confirmed omission:

- `light_status_batch_size` is supported by the Web runner and startup frame,
  but `lower_runtime_start` does not copy the scene value. A non-default
  browser query therefore becomes the default at Worker startup.

The desktop app also retains a second app-local
`local_integrated_scene_options` for direct headless/perf/runtime helpers. It
does not carry all fields handled by the scene-owned projector. Even where a
current caller does not exercise a missing value, this is another unguarded
semantic projection and must not remain a production alternative.

### C. Several values have no explicit cross-host disposition

The following are audit findings, not automatic claims that every platform
must use an identical physical knob:

| Value or behavior | Native disposition | Current Web disposition | Required tactical decision |
|---|---|---|---|
| profile-preferred entry | resolved before initial view | ignored | shared resolved entry, no app-local interpretation |
| light-status batch size | applied to authority | Web supports it, lowering drops it | carry through the shared authority plan |
| authority lighting enabled | applied to server and render policy | render-side fullbright can be derived, but Worker authority has no field | either apply one shared authority policy or reject/reclassify the option explicitly |
| simulation cadence | configured and live-changeable | Worker starts at its own default; runtime setter rejects changes | add a typed Worker operation/config path or explicitly narrow the product capability |
| adaptive chunk publication | applied to native runner | browser background progress has separate fixed mechanics | express one semantic budget request with platform-specific execution, or document and test a normalization |
| render compiler workers/max pending/timing | applied by native compiler | browser uses its resident Worker topology and ignores these values | move physical knobs out of shared semantic launch or return an explicit applied-capacity receipt |
| local player identity | native profile provider | durable browser profile existed, but identity was provisioned after the split rather than carried by the semantic plan | provision identity before the split, carry it whole, and retain the durable Worker fallback only as defense in depth |

No row may close as “Web happens to use the same default.” It must become one
of:

- a field carried by the shared semantic plan;
- a typed platform capability request with an explicit applied/normalized
  receipt;
- an unsupported combination rejected before runtime construction; or
- a separately chartered product gap whose current behavior is explicit and
  cannot masquerade as parity.

### D. The rest of the launch surface has an explicit audit classification

The audit did not find a present value mismatch in every field. It did find
that even the currently matching authority fields are protected only by two
manual projections:

| Classification | Audited values | Current result |
|---|---|---|
| authority semantics copied by both paths | seed, generation profile, starter content, topology, world behavior, day-time override/freeze, scheduled-fluid freeze, passive-showcase mode, auxiliary-player script, observer role | currently carried, but still omission-prone because native and Web enumerate them independently |
| authority semantics broken or unresolved | entry intent/center, authority lighting, light-status batch, cadence, adaptive publication, local identity | covered by Findings A–C and must close in the shared plan/disposition ledger |
| shared client-interest semantics | render distance and resolved initial `ChunkView` | render distance is carried; center is the reproduced mismatch; both belong in the resolved launch receipt |
| scene-resident presentation/input semantics | movement speed and mode, player-movement cadence, adaptive render admission, first-person visibility, underwater mode, debug UI screen, actor visibility, terrain presentation and its explicit-selection marker | consumed by `mclone-scene`; keep them out of authority config while retaining focused scene tests |
| platform resource/mechanism selection | local versus remote host mode, native world root/directory, IndexedDB source, Worker/bindgen URLs, Worker transport sizing, compiler worker count/max-pending/timing | may stay platform-specific only after semantic requests are separated from physical resources and their applied normalization is observable |

Storage backends are intentionally different; logical world/source identity is
not. Remote sessions are also intentionally separate because their server owns
spawn and authority policy. These distinctions prevent the common plan from
becoming a bag of native and browser resources.

### E. Existing safeguards protect schema ownership, not semantic use

Tactical 173's `startup_config_ownership_lock` successfully prevents app DTOs
and TypeScript from restating `StartupSceneOptions`. It proves the canonical
record reaches `McloneSceneHostOptions`. It does not prove that later runtime
assembly consumes every relevant value.

The thin-adapter and Web scene-host gates look for known policy symbols and
retired owners. They do not reject a new field-by-field builder chain or a
platform adapter reading raw scene policy. Per-layer unit tests mostly compare
defaults, so a dropped value can remain invisible when the destination has the
same default.

The missing end-to-end assertion is:

> A shared product action, lowered through every supported local host, yields
> the same normalized semantic launch receipt and the authority accepts its
> resolved entry view.

## Correctness Target

One shared Rust owner must resolve local-session meaning before native/Web
assembly diverges:

```text
SessionStartRequest + catalog/scenario facts + scene policy
  -> shared LocalSessionLaunchPlan
       authority semantics
       typed entry intent
       resolved initial ChunkView
       logical storage/source identity
       render/camera semantic requests
       diagnostic receipt
  -> native binding: threads + filesystem/SQLite + native compiler
  -> web binding: Workers + IndexedDB + browser compiler transport
```

The exact type names may follow the existing crate vocabulary, but the shape is
binding:

- `mclone-server` owns a host-neutral local-authority start configuration;
- `mclone-scene` or `mclone-app-runtime` owns resolution from product/session
  intent into one local-session launch plan;
- platform configs nest or carry that shared value whole and add only physical
  resources/mechanisms; and
- an adapter cannot choose a different seed/profile/entry/cadence/lighting
  meaning by omitting a builder call.

Do not put filesystem paths, IndexedDB store names, Worker URLs, WebSocket
objects, threads, JavaScript values, or GPU handles in the shared plan.

## Binding Design Decisions

### Replace the boolean with typed entry intent

Retire `use_initial_spawn_center` as a value that crosses a platform boundary.
Use a typed entry vocabulary capable of distinguishing at least:

- profile-preferred procedural entry;
- explicit host/user coordinate;
- authored/scenario entry coordinate; and
- persisted-player resume with a deliberate fallback.

By the time a platform starts a runtime, the local launch plan carries one
resolved initial `ChunkView` plus a diagnostic entry receipt. The receipt
names the intent and resolved center; it is not a second source of policy.

Default parser coordinates are not explicit coordinates. CLI/query parsing
must retain coordinate provenance so `--start-in-world` or `startInWorld=true`
without a coordinate uses the same profile-preferred entry on desktop, Web,
Android, and XR. Supplying a coordinate continues to honor it.

Authored and persisted entry semantics must not be approximated with the
currently active scene center. If current catalog metadata is insufficient for
an authored fallback, stop and define the smallest typed metadata addition or
explicitly reject that unsupported open path; do not preserve stale-state
inheritance.

### Carry authority semantics as one nested value

Extract the host-neutral portion currently duplicated by
`NativeIntegratedServerRunnerConfig` and
`WebIntegratedServerRunnerConfig`. Native and Web configs may keep their
specialized storage, scheduling, transport, and Worker/thread fields, but they
must carry the semantic authority config whole.

Fields should be private where practical and constructed by the shared owner.
Adding a new semantic field should change the shared constructor and authority
consumer, not both platform adapters.

Delete the desktop app's local-authority projection or reduce it to a call
that accepts the already-resolved shared plan. Direct headless/perf lanes are
valid consumers, not permission for a second policy owner.

### Keep semantic equivalence separate from mechanism equality

Native threads and browser Workers need not have equal counts, timers, queues,
or storage APIs. A mechanism-specific value may be normalized, but the
normalization must be named, observable, and tested. Silently falling back to a
constructor default is forbidden.

Render compiler capacity is the canary: either represent a neutral capacity
request that each platform resolves and reports, or move native-only physical
worker knobs out of the shared scene semantic record. Do not make Web pretend
to run multiple Workers merely for structural symmetry.

### Make the final receipt authoritative for tests and diagnostics

Expose a cheap `LocalSessionLaunchReceipt` or equivalent containing normalized
semantic facts, including:

- descriptor seed/profile/starter/topology;
- entry intent and resolved initial center/view;
- world behavior and authority-lighting policy;
- cadence and publication-budget disposition;
- time/fluid/debug policy;
- storage/source kind without physical names; and
- requested versus applied mechanism capabilities where they can differ.

Native and Web smoke reports consume this receipt. They must not reconstruct
it from camera state or platform config defaults.

## Implementation Sequence

### Slice 0: Land failing semantic and product-flow probes

- Add a non-default sentinel fixture covering every policy-bearing local
  authority field. Use values deliberately different from all constructor
  defaults, including both boolean directions.
- Lower the same shared Create/Open request toward native and Web assembly and
  record the normalized receipts. Lock the current center and light-batch
  mismatches as failing assertions before production correction.
- Add the exact seed `553534047293117028` as a Web default-Mclone-Wild menu
  regression. Do not click the profile selector before Create.
- Assert requested center, authority-accepted center, entry receipt, final
  player chunk, solid dry floor, empty feet/head columns, and no fluid at the
  final pose.
- Add direct-launch cases with and without explicit coordinates, plus authored
  and persisted-resume controls.
- Record current unresolved dispositions for cadence, lighting, publication,
  render capacity, and identity. Do not let the spawn assertion hide them.

Exit: tests explain the current failure at the host boundary and would also
fail for the already-confirmed non-default light-batch drop.

### Slice 1: Introduce the shared resolved launch plan

- Add the host-neutral authority config and typed local-entry intent.
- Resolve one initial `ChunkView` in shared session/scene policy before either
  platform assembly path.
- Preserve explicit coordinate provenance through argv/query parsing.
- Store the launch plan in the pending session payload consumed by both native
  and external/Web startup.
- Add focused tests for plane and periodic topology canonicalization,
  procedural profile selection, explicit coordinates, authored entry, and
  persisted-player fallback/override.
- Remove the boolean from the external boundary; delete it entirely if no
  purely scene-internal consumer remains.

Exit: `ExternalSceneSessionStart` cannot contain an unresolved local entry
whose interpretation is left to the platform adapter.

### Slice 2: Cut native and Web assembly over atomically

- Make native runner assembly consume the shared authority config and resolved
  view without restating semantic fields.
- Make `WebIntegratedServerRunnerConfig` carry the same shared authority value
  plus Worker/IndexedDB/transport resources.
- Start both runtimes from the plan's resolved view.
- Remove the browser field-by-field semantic builder chain.
- Remove the desktop app-local local-authority projector and route direct
  headless/perf consumers through the shared plan.
- Carry `light_status_batch_size` through the common authority config.
- Preserve remote-session behavior through its separate plan; do not apply a
  local profile spawn to a remote server.

Exit: the original seed starts at `(-48, 20)` through the Web menu, and one
code path owns local-session semantic projection.

### Slice 3: Close every audit disposition

- Apply shared authority-lighting and cadence semantics in the Web Worker, or
  reject/reclassify the corresponding option before startup.
- Express adaptive publication as one semantic budget request with native/Web
  implementations, or land an explicit tested normalization.
- Separate physical render-compiler controls from shared semantic launch, or
  expose and assert an applied-capacity receipt.
- Replace the Web production test identity fallback with an explicit identity
  provider contract. If durable browser identity requires a larger persistence
  campaign, keep that follow-up explicit while rejecting accidental test
  identity as ordinary production policy.
- Add a table to the execution record showing the final owner and disposition
  of every audited field.

Exit: no policy-bearing field is silently ignored or defaulted by a supported
local host. Any deferred product capability has an explicit typed result and a
separate tactical, not a parity claim.

### Slice 4: Add structural and semantic safeguards

- Extend `startup_config_ownership_lock` from schema retention to downstream
  plan retention.
- Add a source/API lock that rejects production local-authority config
  construction in app crates and rejects direct reads of local semantic fields
  in Web lowering.
- Require native and Web platform configs to nest the shared authority config;
  do not maintain an allowlist of copied field names as the primary defense.
- Add a table-driven cross-host conformance suite over non-default sentinel
  plans. Compare normalized receipts, not platform resource structs.
- Add a mutation-style test: changing each semantic sentinel independently
  must change the corresponding final receipt/authority fact on both hosts.
- Make the default browser catalog smoke create default Mclone Wild before any
  profile cycling. Keep profile-cycle coverage as a separate action.
- Assert the smoke's accepted center against
  `initial_spawn_center_for_descriptor`; never infer it from the camera.
- Extend thin-adapter/Web-adoption gates so a new semantic plan expressible by
  existing platform capabilities requires zero app/TypeScript policy changes.

Exit: adding a new field to the shared authority config cannot compile through
one host while being silently absent from the other, and the exact product
flow catches a wrong entry center.

### Slice 5: Cross-platform and rendered closeout

- Run shared tests plus desktop/offscreen and production Web Worker controls.
- Run the browser menu regression sequentially on the headed WebGPU lane and
  inspect the first drawable frame at the resolved entry.
- Exercise flat Android and Android XR session-start gates because they share
  native resolution but currently differ in initial-center defaults.
- Cover Wild, Intro Homestead, authored/showcase, persisted reopen, and remote
  controls without changing their ordinary producers.
- Update the platform-parity, client-entry/runtime, Mclone Overworld, and
  world-generation-profile docs with the final contract and evidence.
- Append implementation deltas and remaining work to the reopened
  `platform-boundary-convergence` ledger. This tactical may close its own
  scope, but the parent requires a later independent fixpoint audit before it
  can be closed again.

## Acceptance Gates

### Entry correctness

- Default Mclone Wild Create World resolves the same center on native, Web,
  flat Android, desktop XR, and Android XR before platform binding.
- Seed `553534047293117028` resolves and is accepted at chunk `(-48, 20)` in
  the current profile revision; the final player surface is dry and safe.
- A direct launch without an explicit coordinate uses profile-preferred entry
  on every client host.
- A direct launch with an explicit coordinate preserves it on every host.
- Authored/showcase entry remains exact, Homestead retains its realm-primary
  route, persisted player state overrides only through the typed resume path,
  and remote entry remains server-authoritative.
- No ordinary new world inherits an unrelated active scene center.

### Semantic propagation

- Native and Web normalized launch receipts are equal for the non-default
  sentinel plan except for explicitly named mechanism dispositions.
- Seed, profile, starter, topology, behavior, entry, time, fluid, debug,
  lighting, light batch, cadence, publication, identity, and observer facts
  each have an asserted final owner.
- A platform cannot accept a shared option and silently run its default.
- Stored-world metadata remains authoritative over launch defaults where the
  existing persistence contract requires it.

### Ownership and prevention

- Exactly one production shared function resolves local-session semantics.
- Native/Web configs carry that semantic plan whole; app crates add resources
  and mechanisms only.
- No app-local `local_integrated_scene_options` equivalent remains.
- No TypeScript field, branch, or Worker message interprets entry or authority
  semantics.
- The source locks are secondary defense; compile-time nesting and product-flow
  tests are the primary safeguards.
- A test-only semantic field using an existing platform capability requires no
  Web adapter or TypeScript edit, providing the boundary fixpoint proof.

## Validation Matrix

Use the current commands in [`../platforms.md`](../platforms.md#validation-policy).
At minimum:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-web-client \
  --test scenario_parity_ownership_lock
cargo check --manifest-path native/Cargo.toml -p mclone-web-client \
  --target wasm32-unknown-unknown
pnpm native:thin-adapters:purity
pnpm native:web:typecheck
pnpm native:web:scene-host-adoption
pnpm native:web:catalog-smoke
pnpm native:web:indexeddb-smoke
pnpm native:desktop-offscreen:smoke
pnpm native:android:avd-session-smoke
pnpm native:android-xr:session-smoke
git diff --check
```

Before browser capture on Linux, run `pnpm host:check` and use the headed
Wayland WebGPU path. Run GPU browser lanes sequentially. Store screenshots and
reports under `/tmp`, inspect the images, and record the exact revision and
launch receipt in this tactical's execution record.

## Execution Record

Implementation landed as the commit series beginning at `dd5a9e02` and ending
at `e44759f4`. Two distinct host-boundary defects were corrected:

1. Web lowered the inherited scene center instead of the shared
   profile-preferred entry intent.
2. Browser catalog creation rounded full-width `i64` seeds through JavaScript
   `number`, so the authority could generate a different world than the seed
   shown in the menu and receipt.

The first fix resolves local-session meaning once in
`mclone-app-runtime::local_session_launch`. The resulting
`LocalSessionLaunchPlan` contains one nested
`mclone-server::LocalAuthorityStartConfig`, one resolved initial `ChunkView`,
and a typed entry receipt. Native threads and browser Workers receive that
authority value whole. The app-local native helper is now only a forwarding
compatibility entry into the shared scene projector, and the Web lowering no
longer enumerates authority fields.

The second fix keeps catalog seeds as exact signed 64-bit values in Rust and
uses decimal text at the JavaScript observation boundary. The Worker startup
frame is version 5 and round-trips the whole authority config, including the
durable local player identity.

### Final disposition ledger

| Audited fact | Final owner and disposition |
|---|---|
| seed, generation profile, starter content, topology, behavior | fields of `LocalAuthorityStartConfig`; nested whole by native and Web configs and round-tripped by the Worker startup frame |
| entry provenance and center | `LocalSessionEntryIntent` distinguishes profile-preferred, explicit, authored, and persisted-player fallback; `resolve_local_session_launch_plan` resolves and canonicalizes one view before platform binding |
| render distance and initial tracking view | resolved once into `LocalSessionLaunchPlan::initial_view`; both runners start from that view |
| authority lighting and light-status batch | carried whole and applied by `LocalAuthorityStartConfig::apply_runtime_policy`; the non-default Worker codec/mutation tests cover both |
| day time, frozen time, scheduled-fluid freeze, passive showcase, auxiliary script | carried whole and applied by the authority config |
| simulation cadence | carried whole at startup and supported by the typed live Worker cadence operation |
| adaptive chunk publication | one semantic boolean resolves through `LocalAuthorityStartConfig::publication_budget` on both hosts |
| local player identity and observer role | durable native/browser profile identity is provisioned before the split and carried in the authority config; the Worker repeats the durable browser lookup only if a caller omitted it; observer role is carried whole |
| logical world/source identity | remains in the shared `SessionStartRequest` and typed `LobbyWorldSource` pending-session payload; it is reported as `storageSourceKind` and is not duplicated into authority policy |
| physical storage | intentionally host-specific: filesystem/SQLite or transient native storage versus IndexedDB or transient browser storage |
| render compiler capacity | classified as a platform mechanism; `RenderCompileMechanismReceipt` reports requested and applied values. Native applies its thread/queue request; Web explicitly normalizes to one resident Worker, one in-flight request, and no scene compile timing |
| scene presentation/input fields | remain consumed by `mclone-scene`; they do not enter local-authority configuration |

### Safeguards landed

- Compile-time nesting replaces parallel field-by-field authority builders.
- The startup ownership lock pins exactly one production resolver call and
  rejects direct Web reads of the retired semantic boundary.
- A table-driven mutation suite changes every authority field independently
  and proves that the plan receipt, native config, and decoded Worker startup
  frame all change together.
- Entry tests cover profile-preferred, explicit, authored, persisted fallback,
  and periodic-coordinate canonicalization. Native CLI tests lock explicit
  coordinate provenance.
- The browser catalog smoke creates untouched default Mclone Wild before any
  profile cycling, preserves the exact seed as text, waits for settled terrain,
  and checks the requested/accepted center plus solid floor and empty feet/head
  blocks.
- The IndexedDB interaction smoke aims at nearby terrain before requiring a
  target, avoiding an assumption that every valid profile entry initially
  faces a block.
- Source-purity and thin-adapter gates reject new app/TypeScript semantic
  projectors; platform resource binding remains explicit.

### Reproduced-seed result

The ordinary browser menu flow created seed `553534047293117028` as
Mclone Wild. Its requested and authority-accepted center were both
`(-48, 20)`. The settled camera was `(-760.5, 74.62, 327.5)`; the sampled
floor at Y=72 was grass, and the feet/head blocks at Y=73/Y=74 were air. The
inspected browser frame is
[/tmp/mclone-native-web-catalog-default-mclone.png](/tmp/mclone-native-web-catalog-default-mclone.png).
The inspected flat Android menu-created frame is
[/tmp/mclone-android-avd-session.png](/tmp/mclone-android-avd-session.png) and
also shows a dry forest entry.

### Validation outcome

Passed on 2026-08-16:

- `cargo test -p mclone-server -p mclone-app-runtime -p mclone-scene`;
- `cargo test -p mclone-native-client`;
- `cargo test -p mclone-web-client`, including the launch mutation,
  ownership, codec, and boundary-fixpoint locks;
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown`;
- `pnpm native:thin-adapters:purity`, `pnpm native:web:typecheck`, and
  `pnpm native:web:scene-host-adoption`;
- `pnpm native:web:catalog-smoke` with report
  [/tmp/mclone-native-web-catalog-ui-probe.json](/tmp/mclone-native-web-catalog-ui-probe.json);
- `pnpm native:web:indexeddb-smoke` with report
  [/tmp/mclone-native-web-indexeddb-reload-probe.json](/tmp/mclone-native-web-indexeddb-reload-probe.json);
- `pnpm native:desktop-offscreen:smoke` with inspected frame
  [/tmp/mclone-desktop-offscreen.png](/tmp/mclone-desktop-offscreen.png); and
- `pnpm native:android:avd-session-smoke` with a real title-menu Create World
  flow and inspected frame
  [/tmp/mclone-android-avd-session.png](/tmp/mclone-android-avd-session.png);
- `pnpm native:android-xr:apk` for the release Quest target.

`pnpm native:android-xr:session-smoke` reached the public Quest testbed and
reported that no attached, authorized headset was available. The shared XR
consumer and release APK build pass and its semantic tests are green, but this
execution record does not claim a new physical-headset result.

### Scoreboard and remaining work

Using Tactical 212's measurement method, pre-series `d8270666^` to
implementation closeout changed authored Web TypeScript 4,061 -> 4,081, all
TypeScript gate lines/modules 4,084/17 -> 4,104/17, Web-only Rust
22,314 -> 22,638, shared scene Rust 32,231 -> 32,290, and shared app-runtime
Rust 35,960 -> 36,297. The combined browser boundary grew 344 lines. The
growth is the exact-seed/product-flow observer and Web consumption/codec tests;
semantic ownership moved into the shared authority/launch types rather than
new TypeScript policy. Product `WebSceneHost` exports remain 38, async mutable
exports remain zero, and the one operation-identity family is unchanged.

This tactical's implementation scope is complete. Per the parent topic's
closure protocol, the only remaining platform-boundary work is a separately
staffed independent fixpoint audit. Physical Quest execution remains a device
evidence gap, not an alternate semantic implementation.

## Non-Goals

- changing the 129-by-129 Mclone spawn survey or selecting a new spawn
  algorithm;
- changing terrain, biome, decoration, wildlife, structure, or profile
  fingerprints;
- changing the Homestead plan/scout origin or showcase composition;
- adding a persistence migration or release compatibility promise;
- forcing native threads and browser Workers to have identical physical
  topology;
- folding remote server authority into local-session planning;
- making TypeScript understand session semantics; or
- deploying a public build without a separate explicit request.

## Stop Conditions

Stop and request a focused decision if:

- correct authored/persisted entry requires a storage-schema change rather
  than a typed fallback over existing records;
- a currently advertised shared option cannot be honored or explicitly
  rejected on a supported host;
- the common authority config would need browser resources or native paths;
- the work expands into a general persistence/identity migration; or
- a proposed safeguard relies only on source text counts while leaving two
  semantic builder chains in production.

## Documentation Contract

This tactical reopens [`../topics/platform-boundary-convergence.md`](../topics/platform-boundary-convergence.md)
with concrete later evidence. Implementation may close this tactical after its
gates pass, but it must leave the parent open for a separately staffed
independent fixpoint audit under that topic's existing closure protocol.

[`../topics/platform-parity.md`](../topics/platform-parity.md) must show the Web
local-session launch boundary as partial until semantic receipts and the real
default-menu flow pass. [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
and [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
must distinguish the correct shared selector from the currently broken Web
adapter application.
