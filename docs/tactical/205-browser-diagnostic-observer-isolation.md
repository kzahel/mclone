# Tactical 205: Browser Diagnostic Observer Isolation

Status: active 2026-07-21. Tactical 204 removed gameplay meaning from the
ordinary browser input path. This tactical starts from the remaining
production report mirror and smoke-only global command registry.

Topic: `platform-host-boundary`

Related topic:

- [`../topics/platform-host-boundary.md`](../topics/platform-host-boundary.md)

Predecessor:

- [`204-browser-raw-input-adoption.md`](204-browser-raw-input-adoption.md)

## Goal

Make browser diagnostics an explicit test client rather than part of the
production platform adapter. Ordinary TypeScript should retain only browser
mechanics and the operational information needed to execute them. Semantic
snapshots and intentional test commands should be Rust-authored, exposed only
through a clearly named opt-in smoke observer, and consumed only by tests.

This tactical is not an attempt to hide all engine vocabulary from tests.
Playwright and offscreen harnesses are allowed to understand the game. The
boundary is that production application code must not maintain that semantic
model or offer it as an ordinary runtime API.

## Starting Shape

`WebSceneHost::report` returns a large mixed object containing both operational
requests and semantic observations. `mclone-web-app.ts` copies camera,
movement, world, UI, statistics, render/compiler, lobby, and session fields
into a large global `__mcloneWebApp.state` on ordinary frames.

The same production global installs smoke-only commands for direct block
queries/interactions, UI actions, settings, render proofs, lobby setup,
background save, resource rebuilding, and shutdown. The only consumers outside
`mclone-web-app.ts` are the browser smoke scripts; production page modules do
not use this registry.

The raw input cutover means these methods no longer form a production input
path, but their placement still invites semantic growth in TypeScript and
makes a test API look like a product contract.

## Fixed Contracts

1. Ordinary `mclone-web-app.ts` exposes no semantic global state mirror and no
   semantic smoke command registry.
2. Smoke support lives in a clearly named, opt-in observer module. It is
   installed only under an explicit smoke mode and is absent from the ordinary
   product interface.
3. The observer is a test client. It may use game vocabulary, but ordinary
   input, lifecycle, rendering cadence, and platform operations do not depend
   on it.
4. Rust authors semantic diagnostic snapshots and receipts. TypeScript may
   transport or expose them, but must not reconstruct engine meaning by
   copying and interpreting dozens of individual fields.
5. Rust authors any retained semantic smoke operation. TypeScript observer
   methods may invoke it mechanically; they do not implement the operation.
6. Prefer real DOM input for input-adapter tests and shared Rust tests for
   action meaning. Retain direct commands only when a test intentionally needs
   a semantic setup/probe that physical input cannot express reliably.
7. The production frame result carries only operational dispositions and
   requests needed for browser mechanics, such as render/session/catalog/asset
   work, pointer-capture state, and typed failures.
8. Product TypeScript may keep browser-mechanical bookkeeping such as Wasm
   borrow state, rAF timing, canvas size, visibility, pointer/fullscreen state,
   and operation queues. It must not mirror movement mode, selected items,
   statistics, world state, UI screen identity, or renderer internals.
9. Browser Rust may retain web-specific lowering and serialization, but shared
   scene/client owners remain the source of diagnostic meaning.
10. No diagnostic extraction may weaken Worker-resident actors, opaque socket
    and IndexedDB executors, catalog/session operations, or the raw input route.
11. A structural gate rejects reintroduction of the production semantic
    registry, broad `lastReport`/`lastUiAction` state, or game diagnostic field
    copying in `mclone-web-app.ts`.
12. This tactical does not move preference schema/defaults or bootstrap/session
    selection. Tactical 206 owns those after the production host is smaller.

## Implementation Slices

### Slice 0: Consumer and field classification

- Inventory every `__mcloneWebApp` consumer and confirm it is smoke-only.
- Classify every report field read by production TypeScript as browser
  operation, temporary preferences/bootstrap debt, diagnostic observation, or
  unused residue.
- Classify direct `WebSceneHost` exports as raw platform input, production
  operation, diagnostic observation, diagnostic command, or dead API.
- Add a lock for the starting global registry and semantic mirror so removal
  is measurable.

### Slice 1: Opt-in smoke observer

- Add a clearly named smoke observer module and explicit smoke-mode activation.
- Move `__mcloneWebApp`, its semantic state, and its direct smoke methods out of
  the ordinary application module.
- Give the observer a narrow connection to the live Rust host and mechanical
  frame driver without making production behavior depend on observer presence.
- Update browser smoke launch URLs and readiness waits to request the observer.

### Slice 2: Rust-authored diagnostic projection

- Separate semantic diagnostic snapshots from ordinary frame/operation
  results.
- Make the observer fetch or receive that snapshot without TypeScript
  field-by-field semantic reconstruction.
- Keep time-series data only where a smoke actually tests a time series;
  otherwise prefer current typed snapshot/receipt state in Rust.
- Delete duplicated TypeScript defaults, aggregations, and copied semantic
  fields as their tests move.

### Slice 3: Command and export reduction

- Route retained semantic smoke operations through an explicitly diagnostic
  Rust facade or delete them when real DOM/shared Rust tests supersede them.
- Remove direct production validation and installation of per-action UI,
  block, render-proof, lobby, and lifecycle smoke methods.
- Delete unused web-only semantic exports rather than preserving them for
  hypothetical tests.
- Keep ordinary raw input, frame, async operation, and browser capability
  exports small and separate.

### Slice 4: Smoke migration and closeout

- Migrate all browser smoke consumers to the opt-in observer.
- Run ordinary page coverage without smoke mode and prove the semantic global
  does not exist.
- Run shared Rust, web Rust, Wasm/bindgen, TypeScript, Worker ownership,
  scene-host adoption, and thin-adapter gates.
- Run headed Wayland desktop and mobile browser smokes and inspect their
  screenshots at the first drawable milestone and after observer migration.
- Refresh the master topic from the landed boundary and open Tactical 206 for
  preferences/bootstrap using the smaller production adapter.

## Acceptance Criteria

1. `mclone-web-app.ts` defines no `__mcloneWebApp` semantic registry and no
   broad semantic state defaults or field-copy loop.
2. An ordinary product page does not install the diagnostic observer/global.
3. Explicit smoke mode installs a separately named observer module and all
   browser smokes use it successfully.
4. The observer exposes Rust-authored snapshots/receipts rather than building
   an independent TypeScript model of scene state.
5. Ordinary frame handling reads only fields required to execute browser
   mechanics or drain Rust-authored operations.
6. Production TypeScript does not name gameplay actions, movement modes,
   hotbar semantics, world block meaning, UI screen identities, statistics, or
   renderer/compiler diagnostic internals to operate the app.
7. Raw DOM input remains the ordinary input route and is unaffected by
   observer installation.
8. Direct semantic commands that remain have a documented test consumer and
   an explicit diagnostic owner; unused exports are deleted.
9. Source locks distinguish permitted observer/test vocabulary from forbidden
   production adapter vocabulary.
10. Ordinary and smoke-mode browser startup, session/catalog/persistence,
    render Worker, input, UI, and shutdown paths still pass.
11. Headed desktop and mobile captures are inspected.
12. The residual preference/bootstrap debt is smaller and recorded precisely
    for Tactical 206.

## Expected Follow-Up

Tactical 206 should move browser preference identity, defaults, clamping, and
application into shared Rust; reduce TypeScript to generic preference storage;
and consolidate initial session/asset bootstrap policy behind a Rust-authored
plan while preserving autonomous browser setup. It must start from the actual
production fields left after this tactical rather than from the old mixed
report surface.

## Stop Conditions

Stop for renewed review if:

- a real product or external support consumer depends on the semantic global;
- removing a report field prevents TypeScript from executing a browser API and
  no neutral operational disposition can express the need;
- observer isolation would require a second engine state mirror rather than a
  Rust-authored snapshot;
- a compile/query gate would materially change the shipped Wasm engine rather
  than only the test exposure; or
- the change would weaken existing Worker, storage, socket, catalog, session,
  persistence, or shared input ownership.

Routine smoke-script migration, bindgen regeneration, test-only query flags,
and deletion of unconsumed diagnostic exports are not stop conditions.

## Non-Goals

- hiding semantic vocabulary from tests;
- replacing Playwright screenshots with only Rust tests;
- eliminating all TypeScript or moving browser APIs into `web_sys`;
- creating a universal diagnostics protocol or production telemetry system;
- redesigning render/compiler diagnostics themselves;
- moving preferences or bootstrap/session policy in this tactical;
- changing IndexedDB, WebSocket, Worker, persistence, or catalog semantics; or
- changing gameplay, UI appearance, input bindings, or world behavior.

## Code Map

- `native/apps/mclone-web-client/www/mclone-web-app.ts`
- `native/apps/mclone-web-client/src/web_scene_host.rs`
- `native/apps/mclone-web-client/scripts/browser-smoke.mjs`
- `native/apps/mclone-web-client/www/mclone-web-smoke.js`
- `native/apps/mclone-web-client/tests/platform_host_boundary_lock.rs`
- `scripts/check-web-scene-host-adoption.mjs`
- `scripts/check-web-worker-ownership.mjs`
- `scripts/check-thin-platform-adapters.mjs`
