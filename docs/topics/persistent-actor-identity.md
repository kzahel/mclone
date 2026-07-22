# Vanilla Actor Persistence Across World Relaunch

Topic: `persistent-actor-identity`

Status: open; the contract was classified against Minecraft Java 1.17.1 on
2026-07-22. The original finding was real test evidence, but it was
misclassified as a persistent-identity defect: the fixture requires stable
runtime entity IDs and a monotonic generic chicken tick counter across a world
relaunch, neither of which is a vanilla invariant. No implementation has
started. The remaining work is to correct the fixture and prove that authored
destination worlds restore the vanilla-persistent identity and per-kind state
described below.

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

The last two observations are vanilla-compatible by themselves. They do not
show whether the actors were correctly loaded or incorrectly re-authored. The
aggregate is red because it currently asserts that both runtime IDs remain
equal and the chicken's generic `age_ticks` remains monotonic. Those assertions
must be replaced with the vanilla-persistent invariants; preserving them would
encode the wrong product contract.

The fixture therefore leaves these important questions unanswered:

- does each actor retain its `EntityPersistentId` across the full destination
  relaunch, not merely a server-level codec round trip;
- do saved position, movement, and implemented subtype fields resume correctly;
- does a removed actor remain removed rather than being restored from the
  authored template;
- are there no duplicate loaded-plus-authored actors; and
- do the browser IndexedDB and native SQLite paths produce the same result?

Run the existing observation under the headed Wayland lane (`pnpm host:check`
first; see `docs/native-web.md`):

```bash
pnpm native:web:lobby-scenario-lifecycle-smoke
```

Until the fixture is corrected, its combined false verdict is expected and is
not evidence that runtime identity or generic chicken age should be persisted.
Tactical 202 confirmed the same result on pre-cutover controls, so the result
predates the Tactical 207 operation-boundary cutover.

## Current Mclone Shape and Mismatch

The shared server already has most of the intended identity shape:

- `native/crates/mclone-server/src/persistence.rs` defines
  `EntityPersistentId` and stores it in `EntitySaveRecord`;
- `native/crates/mclone-server/src/entity/store.rs` deliberately allocates a
  fresh runtime `EntityId` when hydrating a saved entity, then associates the
  restored `EntityPersistentId` with it;
- the store round-trip test explicitly requires a persistent ID to survive a
  fresh runtime-ID assignment; and
- the save record currently contains position, movement, orientation,
  `age_ticks`, chicken egg time, item pickup delay, and item stack state.

`age_ticks` currently combines concepts that vanilla keeps separate: it is a
generic actor tick/animation value for mobs, but it is also the dropped-item
lifetime used for despawning. Applying one persistence rule to that field for
every entity kind is not a faithful long-term model. Alignment should separate
the meanings when necessary:

- a generic runtime tick or animation clock resets on reconstruction;
- `AgeableMob` baby/breeding age persists once implemented;
- chicken egg time persists;
- dropped-item age and pickup delay persist; and
- other semantic timers follow the corresponding vanilla subtype save data.

The existing server codec already writes and restores `age_ticks`; that proves
the field is not simply absent from serialization. It does not prove that an
authored destination used the loaded record, nor does it make a generic chicken
tick counter a vanilla-persistent field.

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

## Recommended Next Work

Open a bounded tactical:

1. Update the lifecycle probe so it can correlate actors by
   `EntityPersistentId`, or by an equally direct shared diagnostic if exposing
   persistent identity in the product protocol is deferred. Do not correlate
   actors by runtime `EntityId` or vector/order position.
2. Replace the runtime-ID equality and generic chicken-`age_ticks` continuation
   assertions with vanilla invariants: stable persistent identity, saved
   position/behavior state, no resurrection, and no duplication. Use a
   subtype-specific persisted value such as chicken egg time or dropped-item
   age when testing timer persistence.
3. Reproduce through browser IndexedDB and native SQLite, then classify any
   remaining failure as record loading, destination re-authoring, save timing,
   or subtype codec loss with file-and-line evidence.
4. If needed, split generic runtime ticks from per-kind semantic age/timers in
   the shared entity/protocol/persistence model and add server-level round-trip
   coverage.
5. Make the corrected lifecycle aggregate a required pass and replace active
   "separate baseline debt" descriptions with the classified result. Preserve
   completed tacticals as historical execution records.
