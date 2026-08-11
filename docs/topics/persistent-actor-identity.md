# Vanilla Actor Persistence Across World Relaunch

Topic: `persistent-actor-identity`

Status: implemented and validated on 2026-07-22. Runtime numeric IDs and the
generic tick counter now follow vanilla reconstruction semantics, durable
identity and subtype state survive save/load, and persistent destinations do
not rerun runtime showcase authoring. Native SQLite and headed-browser
IndexedDB relaunch coverage is green. The original finding was real test
evidence but was misclassified: the old fixture required stable runtime entity
IDs and a monotonic generic chicken tick counter, neither of which is a vanilla
invariant.

This concern was repeatedly reproduced and then explicitly deferred by the
platform-boundary campaign (Tacticals 197, 202, 207, 212, and 213). The
platform-boundary topic is closed; this is a shared entity/persistence contract
and must not reopen that concern.

## Decision: Follow Vanilla's Identity and Save Semantics

Mclone uses Minecraft Java 1.17.1 semantics for actor identity and state across
save/load:

- **Runtime entity IDs are ephemeral.** Vanilla's numeric `Entity.id` is
  allocated from a process-local counter when an entity object is constructed
  (`Entity.java`, `ENTITY_COUNTER` and the `id` field). Loading NBT constructs
  a fresh entity and then applies the saved data (`EntityType.create`), while
  base entity save/load does not serialize or restore the numeric ID. An actor
  may therefore be `3` during one world session and `5` after relaunch. Equality
  or inequality across relaunch has no contract; only uniqueness within the
  live runtime matters. Mclone must not persist `next_entity_id` merely to make
  a relaunch fixture retain these numbers.
- **Persistent identity is the UUID-equivalent.** Vanilla writes `UUID` in
  `Entity.saveWithoutId` and restores it in `Entity.load`. Mclone's
  `EntityPersistentId` is the corresponding durable identity. The same saved
  actor must retain that identity even when it receives a fresh runtime
  `EntityId`.
- **A generic runtime tick counter is not persistent.** Vanilla's base
  `Entity.tickCount` advances while the entity is ticked but is not part of the
  base entity NBT. It normally restarts when the entity is reconstructed. A
  generic animation or observation clock must not be used as evidence that a
  chicken retained its saved identity.
- **Gameplay-semantic state is persistent where vanilla saves it.** Base entity
  state includes UUID, position, movement, rotation, ground state, fire/air,
  portal cooldown, names, flags, tags, and passengers. Subtypes add the state
  that affects their behavior. `AgeableMob` saves baby/breeding `Age` and
  `ForcedAge`; `Chicken` saves `EggLayTime`; `ItemEntity` saves item age,
  health, pickup delay, owner/thrower, and its stack. Health, equipment,
  effects, AI state, and other subtype data follow their vanilla owners as
  those systems are implemented.
- **Closed worlds do not receive wall-clock catch-up.** Saved semantic values
  resume from their stored values when the world runs again. They do not
  advance according to how long the application was closed.
- **Authored content seeds a persistent destination only once.** Once an
  authored destination has a saved world instance, subsequent launches load
  its entity records. They must not silently re-author the template actors,
  resurrect actors that were removed, or duplicate actors. A deliberately
  transient lobby may have separate re-authoring semantics, but those semantics
  must not leak into its persistent destinations.

Relevant vanilla reference points, all under
`reference/minecraft-1.17.1/src/net/minecraft/world/entity/`, are:

- `Entity.java`: runtime `ENTITY_COUNTER`/`id`, base `saveWithoutId`, and
  `load`/`UUID`;
- `EntityType.java`: construct-then-load behavior in `create(CompoundTag,
  Level)`;
- `AgeableMob.java`: `Age` and `ForcedAge` save/load;
- `animal/Chicken.java`: `EggLayTime` save/load; and
- `item/ItemEntity.java`: item `Age`, health, pickup delay, ownership, and stack
  save/load.

## Reclassified Lifecycle Evidence

The browser lobby lifecycle aggregate in
`native/apps/mclone-web-client/scripts/browser-smoke.mjs` launches an authored
destination, observes its actors, mutates terrain, waits for persistence to
drain, quits to title, relaunches, and reopens the destination. It established
that:

- the terrain mutation survives relaunch;
- actors are present and observable after relaunch;
- actor runtime IDs observed as `3`/`4` on the first launch appeared as `5`/`6`
  on relaunch; and
- the compared chicken's generic `age_ticks` did not continue from the prior
  session.

The last two observations are vanilla-compatible by themselves. They did not
show whether the actors were correctly loaded or incorrectly re-authored. The
fixture now correlates actors by `EntityPersistentId`, follows a deliberately
spawned persistent chicken through IndexedDB relaunch, and requires that UUID
exactly once in both the relaunched preview and reopened active destination.
Runtime-ID, vector-order, generic-tick continuation, and off-crop pixel-motion
assumptions were removed.

The corrected native and browser fixtures answer the original questions:

- an authored actor retains its `EntityPersistentId` across a full SQLite
  relaunch while receiving reconstructed runtime state;
- saved position and implemented subtype fields resume correctly;
- authored records win over any provisional showcase actor, and persistent
  storage routes do not rerun showcase authoring;
- the authored SQLite fixture reloads exactly its saved two actors without a
  loaded-plus-authored duplicate; and
- IndexedDB restores the deliberately spawned actor under the same durable ID
  exactly once in the preview and active destination.

Run the existing observation under the headed Wayland lane (`pnpm host:check`
first; see `docs/native-web.md`):

```bash
pnpm native:web:lobby-scenario-lifecycle-smoke
```

The corrected aggregate passes. Tactical 202's historical false result remains
useful evidence that the invalid runtime-ID/tick assumptions predated the
Tactical 207 operation-boundary cutover; it is not a regression baseline.

## Implemented Mclone Shape

The shared stack now implements the intended identity shape:

- `mclone-protocol` owns `EntityPersistentId`, carries it as immutable snapshot
  data, and leaves ordinary updates addressed by runtime `EntityId`;
- `native/crates/mclone-server/src/entity/store.rs` deliberately allocates a
  fresh runtime `EntityId` when hydrating a saved entity, then associates the
  restored `EntityPersistentId` with it;
- the generic field is now `tick_count`, is absent from entity save records,
  and starts at zero when an entity is reconstructed;
- dropped items own a separate persisted `age` beside pickup delay and stack
  state, while chickens retain their persisted egg timer;
- entity chunk format version 2 encodes the new ownership directly and has no
  compatibility reader, as authorized for this unshipped format;
- scene/client/browser observations expose durable identity without turning it
  into an update address; and
- session policy permits runtime showcase authoring only for transient authored
  routes. Persistent catalog and app-private routes load saved actors instead.

`AgeableMob` baby/breeding age, health, equipment, effects, and other subtype
state remain future system work. They must follow the same vanilla owner rule
rather than reuse `tick_count`.

## Constraints

- Keep runtime `EntityId` and durable `EntityPersistentId` distinct. Never
  persist the runtime allocator to manufacture cross-relaunch numeric-ID
  stability.
- Fix shared entity, persistence, authoring, or session ownership as indicated
  by the evidence; do not patch a browser platform adapter.
- Correcting invalid fixture assertions is not weakening the smoke. Replace
  them with positive UUID-equivalent and semantic-state assertions so the test
  detects re-authoring, resurrection, duplication, and state loss.
- Validate native SQLite and browser IndexedDB paths. A shared server-level
  round trip is necessary but not sufficient for the authored-destination
  lifecycle.
- Do not add offline wall-clock aging.
- Read the relevant Minecraft 1.17.1 source before adding each subtype's saved
  state.

## Outcome and Follow-on Scope

Tactical 218 completed this concern. Further entity kinds should extend their
own vanilla-shaped save payload and round-trip tests. They should not reopen
runtime-ID persistence, generic tick persistence, offline catch-up, or browser
platform policy. Tactical
[`277`](../tactical/277-habitat-driven-creature-ecology-foundation.md) now
applies the same durable-identity contract to naturally spawned cows and
chickens: persistent worlds save them immediately and hydrate the same
`EntityPersistentId` with a fresh runtime ID after chunk unload/reload.
Null-store sessions retain an explicitly volatile path. The broader habitat
and population direction remains a separate concern in
[`habitat-driven-creature-ecology.md`](habitat-driven-creature-ecology.md).
