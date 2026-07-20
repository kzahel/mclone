# Tactical 201: Lobby Content Simplification

Status: completed 2026-07-20. All implementation slices are closed. Native
flat/stereo pixels were inspected; the current Linux Chromium/WebGPU capture
environment returned transparent images for lobby and unrelated controls, so
the browser pixel limitation is recorded explicitly rather than hidden by a
weakened gate.

Topic: `embedded-worlds`

Related topics:

- [`../topics/cross-platform-operation-execution.md`](../topics/cross-platform-operation-execution.md)
- [`../topics/unified-persistence-interface.md`](../topics/unified-persistence-interface.md)
- [`../topics/world-dimension-storage-layout.md`](../topics/world-dimension-storage-layout.md)

## Goal

Preserve the completed lobby, retained-world preview, and A-to-B-to-A product
behavior while deleting the accidental managed-content installation subsystem
that grew around the original embedded-world proof.

The protected authored lobby becomes a transient Rust-authored world bootstrap.
An existing compatible catalog world remains the preferred destination. When
no catalog world exists, the fallback becomes an ordinary persistent
app-private generated world rather than a versioned managed scenario payload.

This tactical is the prerequisite cleanup before any new coarse-operation
actor work. It deliberately tests whether deleting the special workflow leaves
a smaller real operation problem; it does not port the existing TypeScript
workflow into a new shared actor.

## Why This Comes First

The current lobby implementation proved valuable core capabilities:

- two concurrent `RealmServer` instances;
- one active and one retained drawable world;
- observer-backed destination warmup and player promotion;
- bounded embedded terrain/entity/player presentation;
- complete-slot A-to-B-to-A activation;
- shared native/browser scene and renderer policy; and
- ordinary persistent catalog-world destinations.

Those capabilities do not require the lobby itself to be a durably installed,
versioned world. The installation layer exists because the first product proof
persisted authored primary and fallback content before their requirements were
clear. It now contributes scenario manifests, content versions, fingerprints,
stored-state classification, repair policy, publication races, an extra native
service, a one-shot browser Worker, and a TypeScript workflow.

Promoting that workflow into a shared actor would remove the TypeScript policy
fork but preserve an unnecessary subsystem. Clean the product boundary first;
then derive later actor work from remaining core consumers.

## Fixed Product Contracts

1. `Enter Lobby` remains a production shared title action on supported native
   and browser hosts.
2. The lobby remains authored, protected, and deterministic. Its source content
   is immutable; each launch may instantiate fresh mutable runtime state in a
   transient store.
3. The lobby becomes playable without waiting for the destination.
4. The destination remains the most recent compatible catalog world when one
   exists, without changing recency during preview warmup.
5. An empty/incompatible catalog uses a stable app-private persistent ordinary
   Overworld fallback with the current seed and generator profile.
6. Successful outbound activation records catalog play exactly once where the
   destination is catalog-owned.
7. The retained preview, supported-arrival gate, complete-slot swap, return
   preview, and A-to-B-to-A behavior remain shared Rust policy.
8. The direct one-world path does not allocate lobby, second-runtime, placed-
   world, or fallback-world state.
9. TypeScript owns browser APIs and transport mechanics only. It does not
   prepare, validate, classify, repair, publish, or construct a semantic lobby
   result.
10. No existing ordinary catalog world may be deleted, overwritten, reset, or
    silently adopted as the private fallback.

## Target Shape

```text
shared Rust lobby launch coordinator
  |
  +-- primary source: TransientAuthored(lobby fixture)
  |     native: inject a memory-backed store through ordinary server assembly
  |     web: do the same inside the integrated-server Worker Rust
  |
  `-- destination source
        +-- Catalog(LocalWorldId)
        |     -> ordinary persistent WorldStore
        |
        `-- AppPrivate(PrivateWorldKey)
              -> ordinary persistent generated WorldStore
```

The authored fixture is a bootstrap input, not a physical storage identity.
The server may mutate its in-memory copy during one lobby session, but the next
launch starts from the immutable authored source again.

The app-private fallback has a stable logical identity but no visible catalog
row. Native assembly resolves it to an app-private world directory; browser
assembly uses a stable IndexedDB `worldId`. Once opened, it uses the same
metadata, generation, chunk/entity/player, revision, flush, close, and writer-
lease behavior as any other persistent world.

## Ownership Boundaries

### Shared engine and app runtime

- authored fixture selection and bootstrap records;
- protected lobby behavior profile;
- destination selection and preview configuration;
- path-free transient/catalog/app-private source identity;
- launch ordering, cancellation, stale completion rejection, readiness, and
  slot installation; and
- typed diagnostics and product outcomes.

### Native platform assembly

- app-private fallback directory resolution;
- direct SQLite opening through the ordinary store path; and
- native thread/channel mechanics already required by session startup.

### Browser platform assembly

- Worker construction, Wasm initialization, messages, cancellation, and
  termination;
- ordinary IndexedDB executor and world-key mapping for the persistent
  destination; and
- Web Locks, promises, callbacks, transfer/SAB mechanics, and browser errors.

Browser TypeScript must not receive authored lobby chunks merely to write them
back to Rust or IndexedDB. The browser server Worker already contains the Rust
fixture and store implementations needed to instantiate the primary locally.

## Explicit Deletion Target

Delete or collapse the following after both replacement sources are live:

- `ProvisionManagedScenarioWorld` and its provisioning completion path;
- persistent primary-world `ManagedWorldKey` routing;
- `ManagedScenarioStoredWorldMetadata`, payload fingerprints, stored-record
  normalization, and the five-way managed validation classifier;
- native scenario staging/publication, rename-race recovery, background
  provision threads, and `ManagedWorldKey -> PathBuf` cache;
- primary and fallback provisioning ledgers/queues in
  `ManagedScenarioLaunchState`;
- Rust Wasm exports that prepare and validate managed IndexedDB payloads;
- `mclone-managed-scenario-provision-worker.ts`;
- TypeScript managed-world inspection, publication, repair, conflict recovery,
  and loose success-report construction; and
- managed-provision operation/controller bookkeeping in the main web app.

Retain or rename only the feature-neutral pieces still used after the cut:

- authored fixture definitions and record codecs;
- `ScenarioLaunchIntent` or a smaller lobby launch request;
- retained-world presentation, runtime start, and activation types;
- generic `PlatformOperationService` where another real consumer needs it;
- ordinary catalog execution; and
- opened-world persistence coordination and physical executors.

Do not keep old and new production lobby paths behind a selector.

## Persistence And Compatibility Policy

The lobby primary has no durable data to migrate. Existing native scenario
directories and IndexedDB managed-world records are legacy app-private data,
not user catalog worlds.

This tactical must not add a live schema upgrade or migrate content while a
world is running:

- stop reading and writing legacy managed primary records;
- leave the existing IndexedDB version-6 `managedWorlds` object store inert
  until an independently justified schema change or factory reset removes it;
- do not scan or delete ordinary records solely because their id resembles an
  old managed id;
- allow an explicit factory reset to remove legacy app-private content through
  its already reviewed storage domain; and
- decide during the baseline whether native legacy scenario directories may be
  left inert or removed only by the explicit reset path.

The persistent fallback keeps its current logical identity if that can be done
without carrying managed-content installation semantics. Otherwise introduce
a clearly app-private key and make any intentional loss of the old internal
fallback explicit before cutover.

## Slices

### Slice 0: dependency and behavior baseline

- inventory every production and test consumer of managed scenario manifests,
  keys, provisioning, metadata, validation, storage, and Worker code;
- distinguish embedded-world product behavior from installer-specific tests;
- capture native and browser lobby-first, catalog-destination, empty-catalog
  fallback, cancellation, Quit, relaunch, persistence, and A-to-B-to-A
  receipts;
- record direct one-world controls, Worker/thread counts, maximum frame gaps,
  and managed TypeScript/Rust source size; and
- confirm which old app-private data may be abandoned without user-world loss.

Exit: deletion targets and preserved behavioral outcomes are exact; no source-
shape lock is mistaken for a product requirement.

Completed 2026-07-20. The production dependency inventory confirms that the
installer is confined to `scenario_content`, the scene launch ledgers, native
scenario staging/path resolution, browser Wasm preparation/validation exports,
the one-shot provision Worker, main-app operation draining, and their source-
shape tests. Ordinary catalog creation and opened-world persistence are
separate consumers and already provide the replacement fallback path.

The shared authored fixture builder already produces canonical chunk and
entity-chunk records, while both native and browser integrated-server assembly
already accept injected `WorldStore` implementations. The first implementation
seam is therefore a memory-backed authored bootstrap selected in opaque Rust
startup configuration; it requires no authored record transport through
TypeScript.

Baseline validation:

- `cargo test -p mclone-app-runtime scenario_content --lib`: 18 passed;
- `cargo test -p mclone-scene --test one_world_ownership_contract --test
  composable_world_presentation_contract`: 27 passed and one GPU
  characterization ignored; and
- two warm native `pnpm native:lobby-scenario:smoke` attempts reached a visible,
  switchable preview and respectively the outbound and return activation, but
  missed the existing fixed 320-frame completion deadline. This pre-change
  timing failure is retained for comparison rather than attributed to the
  implementation.

Current browser topology is one render Worker plus one active and optionally
one standby integrated-server Worker, with server-job Workers beneath each as
needed. Managed startup temporarily adds one one-shot provisioning Worker.
The direct path has no managed Worker or second integrated-server runtime.
Legacy native scenario directories and IndexedDB `managedWorlds` records are
app-private and can remain inert; no ordinary catalog data is in the deletion
set.

### Slice 1: shared transient authored bootstrap

- define a path-free transient-authored session source or bootstrap contract;
- construct and populate the ordinary memory-backed record/store
  implementation from shared authored fixture data before server startup, then
  hand it to the normal persistence mailbox/owner;
- validate world metadata, dimension metadata, chunk/entity records, protected
  behavior, deterministic identity, and fresh-launch reset in shared Rust; and
- keep platform objects and JavaScript values outside the contract.

Exit: the fixture starts one normal `RealmServer` entirely from shared Rust in
tests, with no filesystem or IndexedDB requirement.

Completed 2026-07-20. `authored_world_fixture_memory_store` now materializes
the canonical authored chunk and entity records into the ordinary
`MemoryWorldStore` through the `WorldStore` trait on every target. Tests prove
exact record identity, entity preservation, independent fresh stores after one
copy is mutated, protected-lobby metadata initialization, expected spawn, and
zero world-generation jobs in a normal `LocalRealmSession`. The focused native
tests pass, and the `mclone-server` Wasm library check confirms the bootstrap is
browser-available.

### Slice 2: production primary cutover

- start the native lobby primary from the transient bootstrap;
- start the browser lobby primary from the same bootstrap inside the existing
  integrated-server Worker Rust;
- remove primary provisioning from launch admission;
- preserve lobby-first readiness, Back/Quit cancellation, protected gameplay,
  UI state, and direct-path isolation; and
- capture and inspect native flat/stereo and production browser desktop/mobile
  lobby pixels at the first drawable milestone.

Exit: primary launch performs no managed database inspection or publication on
either platform.

Implemented 2026-07-20. Shared launch policy now issues the primary runtime
start directly as `TransientAuthored(LobbyTableV2)` and never issues a primary
provision operation. Native server assembly seeds a fresh `MemoryWorldStore`
and retains the ordinary dedicated persistence actor/thread. Browser assembly
carries the fixture identity only in the opaque Rust startup frame and seeds
the same store inside the integrated-server Worker; TypeScript sees neither
fixture records nor a fixture-specific branch.

Focused scene tests (122), startup-frame tests (4), native compilation, Wasm
library compilation, and browser TypeScript build/typecheck pass. The native
lobby smoke reached lobby, preview, outbound activation, and return before the
known fixed 320-frame relaunch deadline. The browser probe made the protected
lobby playable in 731 ms, denied break/place, produced a switchable preview,
and completed A-to-B-to-A with no page errors. Final closeout subsequently
passed and inspected native flat/stereo output. Browser screenshots remained
fully transparent despite nonzero drawn-section and actor receipts, including
in unrelated WebGPU controls; Slice 5 records the environment limitation and
keeps the pixel gate intact.

### Slice 3: ordinary app-private fallback

- retain recent-compatible catalog selection unchanged;
- define the stable app-private fallback identity and normal persistent open-
  or-create assembly;
- route native through the ordinary SQLite world store and browser through the
  ordinary IndexedDB record executor;
- preserve the current seed, generation profile, preview bounds, supported
  arrival, mutation persistence, shutdown, reopen, and writer admission; and
- prove the private fallback remains absent from ordinary catalog listing,
  recency, and deletion semantics.

Exit: neither primary nor fallback requires managed content installation.

Completed 2026-07-20. Empty-catalog selection now constructs an
`AppPrivate(LobbyFallback)` destination immediately in shared Rust. Native
maps that identity directly to the existing private fallback directory and
opens its SQLite store normally; browser Rust maps it to the existing stable
IndexedDB world id and uses the ordinary record executor and writer lease. The
old identity and location are deliberately preserved so an internal fallback
already generated by the prior workflow remains usable.

The browser product probe created only integrated-server and render-compiler
Workers: the managed-content Worker was absent, the destination reported
`storageSourceKind=app-private`, the catalog remained empty, and A-to-B-to-A
completed with no page errors. Native likewise reached preview, outbound
activation, return, and a second fresh lobby launch. Slice 5 increased only the
native smoke allowance and now passes the ordinary-world relaunch. The
transparent browser capture remains the host-specific limitation recorded
under Slice 2.

### Slice 4: atomic old-path deletion

- remove the Rust and TypeScript deletion targets listed above;
- simplify and rename the surviving lobby launch coordinator and storage
  source vocabulary around transient, catalog, and app-private sources;
- remove obsolete source locks, fixtures, metrics, and smoke steps while
  replacing each product assertion with the new path;
- keep IndexedDB version 6 and its inert legacy store unchanged; and
- strengthen the Worker ownership checker so scenario IDs, roles, validation
  states, content versions, fingerprints, and repair branches cannot return to
  production TypeScript.

Exit: there is one production lobby path and no managed provisioning workflow
in native Rust, browser Rust bindings, or TypeScript.

Completed 2026-07-20. The managed manifest, payload, metadata, validation,
repair, staging, native adapter/cache, browser Wasm payload exports, one-shot
Worker, and TypeScript provisioning controller were deleted. The surviving
shared types are now a small `LobbyScenarioContent` recipe plus
`LobbyWorldSource::{TransientAuthored, AppPrivate, Catalog}`. The launch state
owns only opaque runtime-start operations; native and browser adapters report
those starts through the same token/epoch boundary.

The browser starts with its final supported profile immediately. TypeScript
constructs Workers from Rust-owned tickets and completes them, but receives no
scenario id, world role, authored records, validation state, content version,
fingerprint, or repair instruction. The Worker ownership gate now locks those
terms at zero. IndexedDB remains version 6 and retains the inert legacy
`managedWorlds` object store; the stable fallback world id is intentionally
unchanged.

Deletion validation:

- `cargo test -p mclone-app-runtime -p mclone-scene -p mclone-web-client
  --lib --tests`: app-runtime and scene unit suites plus focused integration
  suites passed after updating old-path source locks;
- `cargo test -p mclone-native-client cli_`: 106 focused CLI tests passed;
- `cargo check -p mclone-android-platform`: passed;
- `pnpm native:web:typecheck`: Wasm build, bindgen staging, and TypeScript
  typecheck passed; and
- `pnpm native:web:worker-ownership`: passed with five production Worker
  entries, one generic TypeScript construction site, and every registered
  domain-policy debt at zero.

### Slice 5: lifecycle, platform, and documentation closeout

- prove catalog and fallback destinations across native, dedicated-compatible
  storage assembly, browser Workers, Android, and Quest packaging/lifecycle
  boundaries affected by the cut;
- repeat cancellation, stale/duplicate completion, hidden/resume, Worker
  failure, two-runtime shutdown, and relaunch tests against the simplified
  coordinator;
- rerun native/web persistence and catalog conformance to prove the cleanup did
  not weaken ordinary worlds;
- measure direct-path and lobby-on CPU, frame, memory, Worker/thread, and
  retained-byte deltas;
- capture and inspect the final lobby, preview, activation, and return images;
  and
- update the embedded-world, cross-platform operation, persistence, and
  architecture docs to describe only the surviving system.

Exit: the embedded-world proof and product remain live, while the accidental
installer and its TypeScript policy surface are absent.

Completed 2026-07-20. Native fallback, catalog, and synthetic-stereo product
smokes all pass after replacing the old 320-frame managed-payload deadline
with a 480-frame allowance for a second ordinary generated-world warmup. The
flat lane records one cancelled launch, one relaunch, two complete-slot
switches, protected lobby authority, mutable app-private persistence, supported
arrival, and no switch-boundary compile/upload/materialization. Its current
scenario-on receipt records 49 destination chunks, 63 GPU sections, 9,574,992
estimated terrain bytes, 1,534,376 startup-seed bytes, one shared 8,388,608-byte
atlas, zero duplicated atlas bytes, and two terrain resource owners. The
catalog lane selects `recent-lobby-world`, leaves both entries' recency
unchanged during warmup, updates only the selected row on activation, and also
completes A-to-B-to-A plus relaunch. The stereo capture has 217,019 differing
eye pixels and two switches. A flat four-stage contact sheet and the final
stereo image were inspected directly.

Browser semantic receipts remain strong despite the host capture failure:

- desktop and mobile fallback probes make the transient authored lobby
  playable, reject break/place, open an app-private destination, draw the
  bounded preview, and complete A-to-B-to-A with supported first-uncovered
  frames and zero boundary work;
- the IndexedDB reload probe reports `ok`, persists a block mutation, rejects
  a second writer, admits a different world, classifies quota failure, and
  passes the generic record-executor transaction probe;
- the catalog UI probe reports `ok` across create, list, open, recency, and
  delete; and
- the lifecycle probe passes cancelled primary start, repeated launch,
  destination-only Worker failure, A-to-B-to-A, actor identity continuity,
  mutation/reopen persistence, hidden/resume, Quit during destination startup,
  and asset replacement before the independent render-resource-rebuild step
  hits the broken WebGPU device.

The browser limitation is reproducible outside the lobby. Google Chrome 150,
Playwright Chromium, headed Xvfb, the ordinary IndexedDB reload probe, catalog
UI, and the half-space terrain control all produce a one-color transparent or
clear canvas while Rust reports real sections, indices, actors, and accepted
compiler work. The lifecycle resource-rebuild control eventually receives
zero/invalid WebGPU limits and rejects even a 96-byte mapped buffer. The
catalog-lobby probe similarly cannot uncover its activation after the device
stops producing covered frames. No production alpha-mode change fixed the
host, and that experiment was reverted. Pixel thresholds remain unchanged.

Platform and ownership closeout:

- `pnpm native:android:apk` built the flat Android debug APK through the
  repository NDK/Gradle script;
- `pnpm native:android-xr:apk` built the Quest/OpenXR release APK through its
  repository script;
- the earlier combined Rust suites, native CLI tests, Android platform check,
  Wasm/typecheck, Worker ownership gate, and formatting checks remain green;
- the Worker gate finds five production Worker entries, one generic
  TypeScript construction site, and zero registered domain-policy debt; and
- the direct path still constructs no lobby, standby runtime, placed world, or
  app-private fallback. Its accepted no-scenario control remains the current
  2.470/4.262 ms average/P95 median versus the 2.492/4.338 ms control; this
  tactical changed only explicit lobby launch paths. A fresh release
  frame-budget control on this different Linux host measured 2.840/4.578 ms
  average/P95 across 240 frames with zero over-budget frames and zero
  accounting violations. It is a clean feature-off control, not a valid
  cross-host comparison to the earlier M4 medians.

## Validation Requirements

At minimum:

- focused shared fixture/bootstrap and scene launch tests;
- exact same-coordinate and entity-record fixture loads;
- primary fresh-reset proof across two launches;
- catalog-destination and empty-catalog fallback selection;
- fallback edit, flush, close, reopen, and edit recovery;
- protected lobby break/place refusal;
- Back during primary startup, Quit during destination startup, late result,
  and failed destination handling;
- A-to-B-to-A activation with supported arrival and first-uncovered-frame
  readiness;
- browser Worker ownership and TypeScript typecheck/build gates;
- native SQLite and browser IndexedDB catalog/persistence regressions;
- desktop flat/stereo and browser desktop/mobile rendered-output inspection;
- Android and Quest build/lifecycle lanes affected by source resolution; and
- direct one-world feature-off performance comparison.

All screenshots and transient receipts remain under `/tmp`.

## Stop Conditions

Stop for review if:

- the authored lobby has a concrete requirement for durable state across
  launches that cannot be expressed as a separately scoped overlay;
- transient fixture startup would require TypeScript to interpret or transport
  domain records;
- the app-private fallback cannot use ordinary persistence semantics without
  changing or risking a user catalog world;
- removing managed storage requires a destructive browser/native migration or
  physical schema upgrade;
- a preserved embedded-world behavior requires restoring installer policy;
- the direct single-world path regresses materially after bounded repair; or
- native and browser require different scene, destination, or activation
  semantics.

Routine refactoring, test replacement, inert legacy app-private records, and
platform-specific physical source resolution are not stop conditions.

## Post-Completion Reassessment

Tactical 201 ends before this reassessment. Start a fresh review from the
smaller production system using this handoff:

> Review completed Tactical 201 and the current cross-platform operation
> topic. Inventory remaining production TypeScript that understands engine
> semantics and any native/web sequencing still duplicated. Recommend the
> smallest next tactical based on the surviving code. Do not assume a
> replacement for managed provisioning is needed.

The review should read this completed execution record together with
[`cross-platform-operation-execution.md`](../topics/cross-platform-operation-execution.md)
and inspect the current code rather than carrying forward the pre-cleanup
inventory. It should classify each remaining concern as one of:

1. no further work because an existing actor/mailbox already owns the policy;
2. bounded TypeScript ownership cleanup behind an existing Rust owner; or
3. a real surviving operation that justifies a new shared Rust actor and
   numbered tactical.

Create the next tactical only after that classification identifies a concrete
consumer, state owner, duplicated semantic sequence, and measurable acceptance
boundary. Absence of a successor is a valid outcome.

The fresh review completed on 2026-07-20. It found no replacement managed-
provisioning consumer and no need for a new shared actor. It did find a bounded
existing-owner cleanup around browser scene-session dispatch, the redundant
web-only lifecycle mirror, clear lobby runtime-start descriptors, and
TypeScript readiness reconstruction. That completed work is Tactical
[`202`](202-web-scene-async-boundary-cleanup.md).

## Deliberate Non-Goals

- a general downloadable scenario or template installer;
- persistent mutable lobby state;
- base-content-plus-user-overlay storage;
- cloud content distribution or content migration UX;
- a universal actor framework or Worker ABI;
- per-dimension persistence sharding;
- an IndexedDB schema upgrade; and
- redesigning embedded rendering, observers, transfer, or world generation.

If a later product requires installed or mutable authored worlds, start a new
topic from those concrete requirements rather than reviving this subsystem by
default.
