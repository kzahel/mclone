# Tactical 218: Vanilla Actor Persistence Across Relaunch

Status: active 2026-07-22.

Topic: [`persistent-actor-identity`](../topics/persistent-actor-identity.md).

## Instruction Synthesis

Implement the classified Minecraft Java 1.17.1 actor save/load contract end to
end and commit coherent stages while working. Runtime numeric entity IDs and a
generic actor tick counter are session-local. UUID-equivalent identity and
per-kind gameplay state persist without offline catch-up. Correct the authored
destination lifecycle fixture so it proves those semantics through both native
SQLite and browser IndexedDB rather than retaining its current invalid
runtime-ID and generic chicken-age assertions.

No backward compatibility is required. Codec or schema replacement is allowed,
but shared ownership, cross-platform behavior, and rendered-output validation
remain required.

## Starting Evidence

- `EntityPersistentId` currently lives in `mclone-server` persistence records;
  it is restored after the entity store intentionally allocates a fresh runtime
  `EntityId`.
- The server round-trip test explicitly proves persistent identity survives
  fresh runtime-ID assignment.
- `EntitySnapshot` and browser preview observations expose only runtime
  `EntityId` and `age_ticks`, so the browser fixture cannot observe durable
  identity.
- `age_ticks` currently combines a generic mob/runtime tick clock with dropped
  item lifetime. The entity record serializes it for every kind even though
  vanilla persists item age but not base `Entity.tickCount`.
- The native SQLite authored-fixture test already proves moved actor positions
  reload without duplication, but does not compare persistent identity or
  per-kind timer state.
- The headed browser lifecycle reopens terrain correctly but remains red solely
  on runtime-ID equality and generic chicken tick continuation.

## Locked Contract

1. Runtime `EntityId` is unique only inside one live authority and has no
   equality or inequality contract across relaunch.
2. A neutral UUID-shaped `EntityPersistentId` identifies the saved actor across
   server, protocol, client replica, diagnostics, and persistence. It must not
   remain a storage-private type if the ordinary client snapshot needs the
   vanilla identity.
3. Entity updates continue to address an actor by runtime `EntityId`; durable
   identity is immutable spawn/snapshot data and is not repeated on every
   update.
4. Generic actor `tick_count` is session-local and resets on reconstruction.
   Naming must not imply that it is baby/breeding age or item lifetime.
5. Dropped-item age is subtype gameplay state, persists, and continues to own
   merge/despawn semantics. Pickup delay and stack persistence remain intact.
6. Chicken egg time persists. Missing `AgeableMob` baby/breeding behavior is not
   invented by this slice, but future fields must follow the vanilla subtype
   owner rather than reuse the generic counter.
7. A persistent authored destination loads its saved actor records once they
   exist. Relaunch must not re-author, resurrect, or duplicate template actors.
8. Closed-world elapsed time does not advance any timer.
9. All game/protocol/persistence behavior remains in shared crates. Browser
   IndexedDB and native SQLite are interchangeable record executors, not policy
   owners.

## Slice Plan

### Slice 0 — source locks and tactical

- Pin the vanilla `Entity` runtime ID/UUID/tick behavior plus `AgeableMob`,
  `Chicken`, and `ItemEntity` subtype save ownership.
- Record the current native/browser evidence and the exact shared owners.

### Slice 1 — neutral persistent identity

- Move or define `EntityPersistentId` in the shared protocol contract.
- Carry it on `EntitySnapshot`, retain it unchanged in the client replica, and
  include it in preview observations and browser diagnostics.
- Keep `EntityUpdate` runtime-ID addressed and prove codec round trips,
  snapshot/update behavior, and fresh runtime-ID hydration with stable durable
  identity.

### Slice 2 — vanilla timer ownership

- Rename the generic actor counter to `tick_count` across server, protocol,
  client, scene, renderer-facing observations, and diagnostics.
- Give `ItemEntityRuntimeState` its own persisted age and use it for item merge
  and despawn behavior.
- Remove generic tick count from the entity save record, place item age in the
  item payload, and update the entity-record codec without a compatibility
  reader.
- Preserve chicken egg time, item pickup delay, item stack, and all existing
  movement/orientation state.

### Slice 3 — lifecycle truth

- Strengthen the native SQLite authored-world reopen test with stable
  persistent IDs, fresh runtime-ID tolerance, reset generic tick counts, saved
  positions, subtype state, and no duplication.
- Correct the browser lifecycle fixture to correlate actors by persistent ID
  and assert implemented semantic state rather than runtime ID/order or generic
  chicken tick continuation.
- If the corrected proof finds destination re-authoring, fix it in shared
  server/session ownership and add resurrection/duplication coverage.

### Slice 4 — cross-backend and pixels

- Run formatting and focused protocol, client, server, scene, app-runtime, and
  web/Wasm gates.
- Run native SQLite reopen and headed Wayland browser IndexedDB lifecycle
  acceptance. Capture and inspect the first corrected actor-relaunch pixels and
  final lifecycle images under `/tmp`.
- Run broader affected workspace gates in proportion to changed contracts.

### Slice 5 — closeout

- Reconcile this tactical, the living topic, topic index, active validation
  language, and exact evidence.
- Preserve completed tacticals as historical records.

## Required Gates

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-protocol
cargo test --manifest-path native/Cargo.toml -p mclone-client
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo check --manifest-path native/Cargo.toml -p mclone-web-client \
  --target wasm32-unknown-unknown
pnpm host:check
pnpm native:web:lobby-scenario-lifecycle-smoke
```

Add the smallest relevant native SQLite focused test command once the exact
test name is finalized. Browser capture must use the headed Wayland lane; a
headless black/transparent result is not acceptance evidence.

## Completion Bar

- Durable actor identity is observable and unchanged through an authored-world
  relaunch while runtime IDs remain explicitly ephemeral.
- Generic actor ticks reset; item age and implemented subtype timers persist.
- Native SQLite and browser IndexedDB both load saved actors without
  re-authoring, resurrection, or duplication.
- The corrected lifecycle aggregate passes for vanilla semantics and its actor
  pixels are inspected.
- Shared tests and affected target boundaries are green, and the living topic
  records exact final evidence and remaining per-subtype parity work.

## Progress Evidence

### Slice 1 — neutral persistent identity

Implemented on 2026-07-22. `EntityPersistentId` now lives in
`mclone-protocol` as the UUID-equivalent entity identity, formats as canonical
UUID text, and is immutable `EntitySnapshot` data. Protocol version 32 carries
it on initial snapshots while `EntityUpdate` remains addressed only by the
session-local runtime `EntityId`. The server entity state allocates one durable
identity for every actor, restores saved identities while assigning fresh
runtime IDs, and publishes the durable value to ordinary clients. Client
updates retain it unchanged.

Shared embedded-preview observations now carry the durable identity through
scene ownership. Browser diagnostics expose first/second actor persistent IDs,
and the native offscreen motion receipt records the same canonical value. The
protocol codec/format tests, all 126 client tests, all 536 server tests, all 127
scene tests plus scene contract suites, and the web-client Wasm check pass. The
Wasm check retains only pre-existing target-conditional warnings.

### Slice 2 — vanilla timer ownership

Implemented on 2026-07-22. The generic entity counter is now named
`tick_count` end to end and is no longer part of `EntitySaveRecord`. Hydrating
any saved actor starts a fresh runtime counter at zero. Dropped items now own a
separate `age` in `ItemEntityRuntimeState`; that value controls item merge and
despawn behavior and round-trips in the item save payload beside the existing
stack and pickup delay. Chicken egg time remains in the chicken payload.

The entity-chunk codec is version 2 with no legacy reader, as authorized for
this unshipped format. Store tests prove fresh runtime IDs and zero generic
tick counts after hydration while durable IDs, item age, item pickup delay, and
chicken egg time survive. Item tests prove semantic age advances, expires, and
selects the younger merge age independently of the generic counter. All 47
protocol tests, 126 client tests, 536 server tests, and 127 scene tests pass.
