# Persistent Actor Identity Across World Relaunch

Topic: `persistent-actor-identity`

Status: open; unowned defect adopted into its own topic on 2026-07-22. No
implementation has started. This concern was repeatedly reproduced and then
explicitly deferred by the platform-boundary campaign (Tacticals 197, 202,
207, 212, 213 — each recorded it as "separate baseline debt, route a
separate owner"), and until this document no owner existed. The
platform-boundary topic is closed; this defect is not a boundary problem
and must not reopen that concern.

## Symptom

Relaunching a previously opened authored/embedded destination world
restores terrain correctly (a mined block reopens as air), but the
persisted actors come back wrong:

- reopened actors receive **new entity IDs** — observed identities changed
  from `3`/`4` on first launch to `5`/`6` on relaunch (Tactical 202); the
  IDs are valid and later, which is what implicates a non-restored ID
  allocation rather than corruption; and
- actor **age ticks reset** — the compared chicken's age did not advance
  across the relaunch even though it had aged before quitting.

Player identity, remote-player identity, terrain mutations, world instance
identity, and observation counts all persist correctly through the same
relaunch; the defect is specific to non-player actor identity/age.

## Reproduction and Fixture

The defect is pinned by the browser lobby lifecycle aggregate in
`native/apps/mclone-web-client/scripts/browser-smoke.mjs`. The
`relaunchedPersistenceOk` conjunction (defined near line 1893 as of
2026-07-22) launches a lobby destination, records first/second actor
entity IDs and age ticks, mutates terrain, waits for
`runnerPendingPersistenceSaves === 0`, quits to title, relaunches, and
then requires among other things:

- `embeddedPreviewFirstActorEntityId` /
  `embeddedPreviewSecondActorEntityId` equal to their first-launch values
  (fails: IDs advance), and
- relaunched actor age ticks advancing from their pre-quit values
  (fails: ages reset).

Everything else in the aggregate passes; the aggregate verdict is false
solely because of this fixture. Run it under the headed Wayland lane
(`pnpm host:check` first; see `docs/native-web.md`):

```bash
pnpm native:web:lobby-scenario-lifecycle-smoke
```

Tactical 202 additionally confirmed the failure on pre-cutover controls,
so the defect predates the Tactical 207 operation-boundary cutover and is
not caused by the operation machinery.

Unknown and worth establishing first: whether the same defect reproduces
on the native SQLite path. If it does, the fault is in shared server-side
persistence (expected); if it does not, the browser IndexedDB record path
becomes the suspect.

## Code Map (starting points, verified 2026-07-22)

- `native/crates/mclone-server/src/entity/store.rs` — entity store owns
  `next_entity_id` (field at :49; allocation at :700). Whether this
  counter is persisted and restored on world reopen is the first question.
- `native/crates/mclone-server/src/persistence.rs` and
  `persistence/record_executor.rs` — `save_entity_chunk` /
  `load_entity_chunk` record paths.
- `native/crates/mclone-server/src/entity/state.rs` — per-entity state,
  including age; check what the entity-chunk codec actually serializes.
- `native/crates/mclone-scene/src/session.rs` — lobby launch and
  authored-destination startup (`begin_lobby_launch` cluster); relevant if
  authored worlds re-author actors on launch instead of loading persisted
  records.
- Background: Tactical 185 (realm/dimension runtime and qualified
  persistence), Tactical 189 (placeable persistent prepared actors),
  [`unified-persistence-interface.md`](unified-persistence-interface.md)
  (the typed persistence port these records flow through),
  [`embedded-worlds.md`](embedded-worlds.md) (lobby/destination product
  shape).

## Hypotheses To Classify (not yet investigated)

The fix tactical's first slice should classify the failure before writing
any fix:

1. **ID allocator not persisted.** Entity records restore, but
   `next_entity_id` (or the load path) assigns fresh runtime IDs instead
   of restoring persisted identities.
2. **Authored destinations re-author instead of load.** The
   transient-authored lobby semantics may leak into persistent
   destination worlds: on relaunch the world re-runs actor authoring
   (fresh spawns with fresh IDs and zero age) even though persisted
   entity-chunk records exist.
3. **Age not serialized.** Entity identity aside, `age_ticks` may simply
   be missing from the entity-chunk codec or reset on load.
4. **Fixture expectation wrong.** If the intended product contract is
   that authored worlds re-author their actors on each launch, the fix is
   a deliberate contract decision plus a fixture change — but then actor
   persistence for authored destinations must be explicitly declared
   out of scope somewhere durable, not silently tolerated as a red
   aggregate.

Hypotheses 1–3 can coexist. The observed "valid later IDs" pattern is
most consistent with 2 (re-authoring after the persisted actors already
consumed IDs 3/4) but that is inference, not evidence.

## Constraints

- Fix in the shared owner (`mclone-server` entity/persistence), not in a
  platform adapter; validate both native SQLite and browser IndexedDB
  paths per the shared-first policy.
- Do not weaken the smoke assertion to make the aggregate pass; Tacticals
  202 and 207 deliberately preserved it. The end state is the assertion
  passing (or a recorded contract decision under hypothesis 4).
- Vanilla reference: entities in Minecraft 1.17.1 persist UUID and age
  through save/load (`reference/minecraft-1.17.1/src/`, entity NBT
  save/load). Read the reference before designing the persisted identity
  shape.

## Recommended Next Work

Open a bounded tactical:

1. Reproduce on browser and attempt a native SQLite control; classify
   against the hypotheses above with file:line evidence.
2. If 1–3: decide the persisted actor-identity contract (IDs, age, and
   what else must survive relaunch), fix in the shared owner, and cover
   with a server-level persistence round-trip test — not only the
   browser smoke.
3. Flip the lifecycle aggregate to required-pass and delete the
   "separate baseline debt" carve-out language from active validation
   docs so this cannot be silently re-deferred.
