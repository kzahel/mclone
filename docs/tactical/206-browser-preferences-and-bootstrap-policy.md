# Tactical 206: Browser Preferences And Bootstrap Policy

Status: complete 2026-07-21. Browser preferences, initial host/resource
selection, presentation defaults, post-Wasm status, and rich diagnostics now
have Rust owners. TypeScript retains browser initialization and mechanical
execution without reconstructing those policies.

Topic: `platform-host-boundary`

Related topic:

- [`../topics/platform-host-boundary.md`](../topics/platform-host-boundary.md)

Predecessor:

- [`205-browser-diagnostic-observer-isolation.md`](205-browser-diagnostic-observer-isolation.md)

## Goal

Make browser startup and preference persistence follow the same ownership rule
as input and asynchronous operations: Rust owns identity, types, defaults,
validation, ordering, and engine consequences; TypeScript owns only browser
module loading, URL/cache decoration, byte fetching, Worker construction,
local browser API mechanics, and failure capture before Rust can exist.

This is not a request for Rust to micromanage browser initialization. The
browser should still create its canvas, load Wasm, install listeners, run rAF,
construct Workers, fetch bytes, and execute local storage calls naturally. The
change is that TypeScript no longer decides which engine host to create, which
resource has which logical role, or what a stored setting means.

## Starting Shape

After Tactical 205, production `mclone-web-app.ts` is 1,261 lines and no longer
contains a semantic diagnostic mirror. Its remaining policy seams are now easy
to name:

- `mclone-web-settings.ts` owns the look-sensitivity default and range,
  touch-control mode parsing, and two semantic localStorage keys;
- `WebFrameDriver` loads those values, clamps them again, persists them after
  Rust-authored setting effects, and owns typed fields for their meaning;
- TypeScript reads a Rust startup plan back into semantic fields, clamps render
  distance, and mirrors occlusion, fullbright, color-profile, and generation
  options;
- TypeScript chooses between separate local-Worker and remote-WebSocket scene
  constructors;
- three hard-coded URLs and positional arguments assign reference, authored,
  and generated-fallback asset roles in TypeScript;
- TypeScript chooses initial debug-overlay visibility from browser device/media
  facts and translates post-Wasm startup phases into product status strings;
  and
- the ordinary app validates a method-by-method semantic `WebSceneHost`
  surface rather than relying on one coherent bootstrap boundary.

Several neighboring responsibilities are already correct and must survive:

- query parsing produces an opaque `WebStartupConfig` Rust handle;
- Workers host Rust actors while TypeScript owns construction and transport;
- catalog, persistence, socket, lobby, and session continuations are
  Rust-authored and mechanically executed;
- rAF, canvas sizing, visibility, pointer/fullscreen mechanics, and pre-Wasm
  fatal errors are browser-owned; and
- Rust already has shared touch settings, client-experience setting effects,
  and browser-Rust localStorage precedents for profile and asset-pack
  preferences.

## Fixed Contracts

1. Shared Rust owns preference names/identity, types, defaults, ranges,
   parsing, clamping, and application to input/UI/scene state.
2. A platform preference adapter only reads, writes, or deletes opaque
   key/value bytes or strings and reports availability/failure. It does not
   know that a key controls touch, movement, rendering, or any other feature.
3. Browser preference access may be implemented by a domain-blind TypeScript
   localStorage executor or directly by a browser-Rust adapter over `web_sys`.
   The latter is preferred if it is smaller and uses the same shared
   preference contract; zero TypeScript is not itself the goal.
4. Stored values are normalized once by Rust. TypeScript does not provide
   duplicate defaults, enums, finite-number checks, or clamp ranges.
5. Preference persistence is driven by Rust-owned setting changes, not by
   TypeScript interpreting report fields or UI action labels.
6. Browser setup remains autonomous where no engine choice is involved:
   canvas/Wasm setup, rAF, DOM listeners, URL version decoration, fetch,
   Workers, WebSockets, IndexedDB, localStorage calls, and browser capability
   probes need no per-step Rust command.
7. Rust authors one initial browser bootstrap plan from query/startup
   configuration. It decides local-integrated versus remote session meaning,
   resource roles, initial engine settings, and which logical constructor path
   applies.
8. TypeScript may see opaque resource request IDs and URLs. It fetches bytes
   and returns them without assigning authored/reference/fallback meaning or
   depending on positional semantic argument order.
9. One browser-Rust creation boundary consumes the startup handle, fetched
   resource results, canvas, and mechanical capability handles/URLs. Product
   TypeScript does not branch between engine host kinds.
10. Rust receives raw browser capability facts needed for policy, including
    touch/media characteristics. TypeScript may still apply a universal local
    browser rule, but it does not choose a game debug/UI default.
11. Post-Wasm loading/readiness labels and overlays are Rust-authored. A small
    generic pre-Wasm `loading`/fatal browser surface remains legitimate because
    no Rust owner exists yet.
12. The refactor must preserve the existing Worker-resident actors, SAB
    transport, render-worker factory, remote socket executor, IndexedDB
    executors, session/catalog/lobby continuations, and rAF loop.
13. Native typed hot paths do not adopt JavaScript promises, objects, resource
    bags, or browser-specific bootstrap shapes. Shared policy types may be
    reused; browser physical assembly remains a browser adapter.
14. Source locks reject semantic preference constants, local/remote host
    selection, asset-role assembly, post-Wasm phase interpretation, and
    product-only debug-default policy returning to production TypeScript.
15. Closeout separates the always-on operational frame result from the rich
    observer snapshot, unless code evidence shows that doing so would duplicate
    diagnostic assembly; ordinary product TypeScript must never interpret the
    rich fields either way.

## Implementation Slices

### Slice 0: Exact residual inventory and locks

- Classify every remaining production `report.*` read and every mutable
  `WebFrameDriver` field as browser mechanics, preference debt, bootstrap
  debt, or an opaque async operation.
- Add source locks for the current semantic localStorage keys/defaults/clamp,
  local-versus-remote constructor branch, role-bearing asset constants and
  positional arguments, default debug-overlay selection, and post-Wasm status
  translation.
- Pin ordinary and smoke-mode desktop/mobile behavior before moving ownership.

### Slice 1: Shared preference codec and platform store

- Define or extend a small shared Rust preference schema/codec around the
  existing touch settings and client-experience effects.
- Keep storage physical: a key/value port with browser-Rust localStorage and
  test-memory implementations is sufficient; do not build a general database.
- Load and normalize preferences before the live browser input context is
  initialized.
- Persist Rust-owned setting effects after successful application, with
  typed/bounded failure reporting that does not break gameplay when storage is
  unavailable.
- Delete semantic constants, types, parsing, clamping, and persistence
  branches from `mclone-web-settings.ts` and `mclone-web-app.ts`.

### Slice 2: Rust-authored resource and host plan

- Replace the semantic `WebStartupPlan` projection with an opaque Rust plan
  that exposes only mechanical resource requests and any browser capability
  inputs it needs.
- Return stable opaque resource request IDs plus relative URLs; TypeScript
  version-decorates and fetches them generically.
- Add one browser-Rust scene-host creation boundary that resolves resource IDs
  to roles and chooses local-integrated versus remote construction internally.
- Delete TypeScript's three role-bearing asset fields/constants, positional
  semantic pack arguments, remote-host branch, and duplicated render-distance
  clamp/settings mirror.

### Slice 3: Capability and startup/status cleanup

- Pass raw touch/media capability facts into Rust-owned initial policy and
  remove TypeScript's game debug-overlay default decision.
- Make Rust own status-overlay changes after Wasm initialization; keep only
  generic module/bootstrap/fatal reporting before Rust exists.
- Shrink product method validation to the coherent creation, raw input, frame,
  capability, and opaque-operation surface actually used by browser mechanics.
- Give the opt-in observer an explicit snapshot/receipt read so ordinary frame
  results do not serialize or carry the rich semantic diagnostic bag.
- Remove now-dead TypeScript startup helpers and semantic runtime-state fields;
  retain observer-only state in the explicit smoke observer.

### Slice 4: Closeout and cross-platform validation

- Run shared preference/client-experience/scene tests and browser-Rust tests.
- Run Wasm check/build, generated bindings, TypeScript, Worker ownership,
  scene-host adoption, thin-adapter, and host-boundary locks.
- Run ordinary pages without the observer and all observer-backed browser
  smokes, including persistence/settings and local/remote startup.
- Run headed Wayland desktop and mobile browser smokes and inspect captures at
  startup, first drawable output, settings/UI, and resumed gameplay.
- Run affected native/Android/XR compile and settings gates when shared types
  change.
- Refresh this topic, `platform-parity.md`, and current line-count/inventory
  evidence; record any honest residual exception rather than hiding it.

## Acceptance Criteria

1. Production TypeScript contains no semantic preference key, default, range,
   parser, enum, clamp, or setting-sensitive persistence branch.
2. Rust loads, validates, applies, and persists touch look sensitivity and
   touch-control mode through one typed owner.
3. Missing, unavailable, malformed, non-finite, and out-of-range stored values
   have Rust tests and deterministic fallback behavior.
4. Production TypeScript does not choose local-integrated versus remote engine
   construction.
5. Production TypeScript does not name or positionally assign reference,
   authored, or generated-fallback asset roles.
6. Rust authors the initial host/resource plan and one browser creation
   boundary consumes the mechanically fetched responses.
7. Browser module loading, cache/version URL decoration, byte fetch, Worker
   construction, canvas/rAF, DOM, and browser failure mechanics remain simple
   platform code.
8. Production TypeScript does not independently clamp or mirror initial render
   distance, generation profile, fullbright, occlusion, or color policy merely
   to recreate Rust startup state.
9. Default debug/UI policy and post-Wasm status meaning are Rust-owned; generic
   pre-Wasm loading/fatal reporting remains browser-owned.
10. The product `WebSceneHost` validation/call surface is smaller and grouped
    by neutral input, cadence, capabilities, and opaque operations.
11. Local integrated, remote WebSocket, IndexedDB persistence, world catalog,
    lobby/session transitions, render Workers, and asset replacement retain
    existing behavior.
12. Ordinary pages remain free of diagnostic globals and the opt-in observer
    still supplies all smoke diagnostics.
13. Source locks prevent the removed policy from returning to TypeScript.
14. Headed desktop and mobile screenshots are captured and inspected.
15. The master platform-host topic can close with any deferred exceptions
    named explicitly and assigned to their proper independent topic.
16. The ordinary frame result is an operational projection; the rich semantic
    snapshot is requested only by the opt-in observer.

## Landed Outcome

`ClientInputPreferences` in `mclone-app-runtime` now owns the browser-visible
touch preference keys, defaults, parsing, normalization, and persistence
format. Browser Rust uses a domain-blind `web_sys` key/value adapter, applies
the resulting settings before live input begins, and persists shared setting
effects. The deleted `mclone-web-settings.ts` no longer supplies a second
schema or clamp.

Rust now authors the bootstrap resource plan as opaque numeric request IDs and
URLs. TypeScript version-decorates and fetches those resources generically into
`WebBootstrapResources`; only Rust resolves IDs to reference, authored, and
fallback roles. One `mclone_web_create_scene_host_with_startup` entry chooses
the local-integrated or remote-WebSocket runtime from the opaque startup
handle, so product TypeScript no longer branches on engine host meaning or
mirrors render/generation options.

`WebHostCapabilities` carries raw touch availability and viewport width into
Rust-owned initial presentation policy. Rust chooses the initial debug overlay
and owns all post-Wasm status overlays. TypeScript retains only the generic
pre-Wasm `Starting mclone…` element and fatal fallback, plus natural canvas,
module, fetch, Worker, rAF, DOM, pointer/fullscreen, storage, and WebSocket
mechanics.

Ordinary host methods now return a small operational projection containing
only cadence, sizing, capture/UI disposition, lifecycle, and pending-operation
facts. The query-gated smoke observer explicitly requests
`diagnosticSnapshot()` when tests need camera, world, gameplay, render, Worker,
or statistics state. The product adapter neither reads those fields nor
installs the semantic global on ordinary pages.

The final authored browser inventory is 3,780 TypeScript lines across 16
modules. `mclone-web-app.ts` is 1,057 lines, the explicit smoke observer is 391
lines, and all registered Worker/scene domain-aware debts remain at zero. The
remaining TypeScript is deliberate platform machinery or explicit test code,
not a second gameplay/runtime policy layer.

## Execution Record

Commits:

- `5164f959` (`Move browser input preferences into Rust`)
- `a2a6ac43` (`Move browser host bootstrap policy into Rust`)
- `2a3e6fd3` (`Move browser startup presentation policy into Rust`)
- `18736f1c` (`Separate browser operations from diagnostics`)
- `68b9fd5c` (`Wait for remote browser queues to drain`)

Validation completed on 2026-07-21:

- shared preference normalization/round-trip tests and startup-ownership,
  native-scene, and browser host-boundary locks;
- wasm32 web-client, desktop, flat-Android, and Android-XR compile checks;
- generated bindings/TypeScript, Worker unit tests and ownership, scene-host
  adoption, and thin-adapter purity gates;
- headed direct-host, local app-loop, mobile, world-catalog, asset-pack,
  IndexedDB reload, and remote-WebSocket browser smokes; and
- the ordinary-page absence probe, which confirmed that the semantic smoke
  global is not installed without explicit opt-in.

The headed desktop, mobile, catalog, asset-pack, IndexedDB, and remote captures
were inspected. They showed normal world/HUD rendering, touch controls and
options, world create/open/delete, authored-only asset restoration, persisted
block placement after reload, and remote-WebSocket movement and interaction.

The final browser runs exposed and closed three integration races rather than
weakening their assertions: restored asset preferences now dispatch their
Rust-authored pending operation during warm-up, frame-idle state is published
after the active rAF callback completes, and the remote smoke waits for the
zero-queue state that its final assertion already requires.

## Stop Conditions

Stop for renewed review if:

- a proposed shared preference contract would force platform-specific settings
  into all hosts or replace an existing broader settings owner;
- a generic resource-response plan cannot preserve current fetch caching,
  Worker initialization, or asset error quality without a materially larger
  protocol;
- one browser constructor would duplicate or hide the existing local/remote
  shared runtime owners rather than selecting between them cleanly in Rust;
- moving startup labels would require TypeScript to reconstruct a second Rust
  readiness state machine; or
- the work would weaken actor/Worker, socket, persistence, catalog, session,
  or render ownership established by Tacticals 197-205.

Routine bindgen signature changes, an opaque resource response DTO, deletion
of semantic settings helpers, direct browser-Rust localStorage use, and moving
the local/remote branch into browser Rust are not stop conditions.

## Non-Goals

- moving every browser API call into Rust;
- making native use browser URLs, promises, resource response arrays, or
  JavaScript storage;
- building account/cloud preference synchronization or a universal settings
  database;
- changing user-facing settings values, asset contents, world generation,
  session semantics, or visual design;
- redesigning asset replacement after initial startup;
- adding runtime database schema migration semantics;
- changing IndexedDB catalog/persistence addressing; or
- removing semantic vocabulary from the explicit smoke observer or tests.

## Code Map

- `native/apps/mclone-web-client/www/mclone-web-app.ts`
- `native/apps/mclone-web-client/www/mclone-render-compiler-shared.ts`
- `native/apps/mclone-web-client/www/mclone-web-smoke-observer.ts`
- `native/apps/mclone-web-client/src/web_bootstrap.rs`
- `native/apps/mclone-web-client/src/web_scene_host.rs`
- `native/apps/mclone-web-client/src/web_canvas.rs`
- `native/crates/mclone-input/src/lib.rs`
- `native/crates/mclone-app-runtime/src/input_preferences.rs`
- `native/crates/mclone-app-runtime/src/client_experience.rs`
- `native/crates/mclone-scene/src/host_effects.rs`
- `native/apps/mclone-web-client/tests/platform_host_boundary_lock.rs`
- `native/crates/mclone-app-runtime/tests/startup_config_ownership_lock.rs`
- `scripts/check-web-worker-ownership.mjs`
- `scripts/check-web-scene-host-adoption.mjs`
- `scripts/check-thin-platform-adapters.mjs`
