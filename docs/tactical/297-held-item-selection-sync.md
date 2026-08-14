# Tactical 297: Held Item Selection Sync

Status: complete 2026-08-14, including exact public desktop/phone acceptance

Topic: `held-item-selection-sync`

Topic: `rabbit-burrow-ecology`

## Instruction Synthesis

Make the selected hotbar slot be the player's authoritative held item. Merely
selecting a carrot must immediately tempt nearby rabbits, and merely selecting
a different slot must immediately restore their normal fear response. Neither
transition may require attack, use, feeding, or another world interaction.

## Diagnosis

- Flat clients update the local hotbar highlight as soon as keyboard, wheel,
  touch, or controller input selects a slot.
- `ClientInventory` correctly records that its carried slot has not yet been
  sent, but `McloneSceneHost` consumes that pending command only immediately
  before attack or use.
- The server and rabbit AI therefore observe the last *used* slot while the
  HUD displays the current slot. This is a general held-item contract defect,
  exposed clearly by carrot temptation.

## Reference Contract

Minecraft Java 1.17.1's `MultiPlayerGameMode.tick` calls
`ensureHasSentCarriedItem` every client tick. That method compares the local
inventory selection with the last sent index and emits
`ServerboundSetCarriedItemPacket` whenever they differ. Interaction methods
also perform the check to preserve command ordering.

Mclone will retain the existing interaction-time ordering check and add the
same automatic scene-cadence synchronization. Input adapters continue to
report semantic slot selection only; platform code does not own inventory or
network policy.

## Binding Behavior

- A valid selected-slot change updates the local hotbar immediately and sends
  `SetCarriedItem` at the next shared live scene cadence without waiting for a
  world action.
- Repeated frames with no selection change send nothing.
- Attack and use retain their pre-command synchronization safeguard.
- The behavior is shared by desktop, browser, flat Android, and any other
  client using the shared scene cadence. XR interaction ordering remains
  intact and uses the same pending-carried-item owner.
- Runtime absence must not falsely mark a pending selection as sent.
- Rabbit AI remains server-authoritative: selected carrot means temptation;
  carrots in other slots do not; selecting away restores avoidance.

## Validation and Acceptance

1. Focused client tests retain one-command-per-selection deduplication.
2. Shared scene tests remain green, while the ordinary-input browser gate
   proves that idle selection reaches authoritative creature behavior without
   attack or use.
3. The data-only rabbit showcase selects the carrot through ordinary desktop
   and rendered touch-hotbar input, observes a visible adult approach, then
   selects a non-carrot without using either item and observes an open-ground
   adult flee and gain meaningful separation. The attraction and avoidance
   subjects may differ because the bounded fixture bank deliberately separates
   their reachable ground, while the selected carried state is player-global.
4. Existing refuge, garden, raid, separation, collapse, identity, and
   zero-browser-persistence gates remain green.
5. Inspect native and Web captures, deploy the exact pushed revision, and
   repeat desktop and phone acceptance against the public URL.

## Non-Goals

- changing rabbit temptation distance, feeding, breeding, or avoidance AI;
- adding hand models, item-use animations, stealth, scent, or vision;
- showcase-specific simulation or direct harness commands to rabbit AI; or
- redesigning inventory replication beyond the selected carried slot.

## Execution Record

- Commit `f77594f9` makes ordinary slot input synchronize immediately and
  reconciles again on the shared flat-scene cadence, following Java 1.17.1's
  `MultiPlayerGameMode.tick` carried-item contract. Runtime absence retains the
  pending selection rather than falsely consuming it.
- Commits `542c08cc` and `55bce155` extend the unchanged data-only rabbit
  showcase gate across keyboard and rendered phone hotbar input. The gate
  selects slot 3, observes a rabbit approach at least 0.5 blocks with zero
  world interactions, selects empty slot 0, and then observes another loaded
  adult sustain `flee` with at least 1.5 blocks each of travel and added
  separation. Touch interaction acceptance now holds the real input until its
  requested gate-state effect is observed, preventing delayed worker commands
  from satisfying the wrong assertion.
- `cargo test --manifest-path native/Cargo.toml -p mclone-client --lib --quiet`
  passed 145 tests; `mclone-scene` passed 176 tests; the complete native
  workspace test suite passed with only its pre-existing ignored GPU fixtures.
- Native flat/stereo captures were inspected at revision 4 with SHA-256
  `79be5e57225b943f55df4fbc595e0e806bed1d5a487ca07037613dcb80cd1746`
  and
  `32a8947fa83b9c923f1cd88bbabe4dec82f849d50f2e31e8c2bfe0054ed4e232`.
- Exact pushed revision `55bce155f1c39765be42731d3b73a8c609d31896`
  deployed as asset version `55bce155f1c3-20260814191357` under Cloudflare
  Worker version `9f8a9a5f-689e-4653-91d1-101f8024d4c1`. Public desktop and
  phone gates measured 0.54-block carrot approaches with zero interactions,
  then 1.51/1.55 blocks of added separation after selecting away. Every one of
  the eight IndexedDB world-record stores remained empty.
- The inspected deployed clean desktop/phone first frames have SHA-256
  `a7b790ba21fb17fc82bbf64703fb5ebcec2489b805f7e08f2324e229dd86f088`
  and
  `866cbed8a4be978c199df3aaf6abe2149e92412d2b9ea23ed165ee00718a564c`.
