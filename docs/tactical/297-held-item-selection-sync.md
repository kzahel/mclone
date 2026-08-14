# Tactical 297: Held Item Selection Sync

Status: active 2026-08-14

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
2. Focused scene coverage proves an idle frame sends the newly selected slot
   without attack or use and does not resend it on the following frame.
3. The data-only rabbit showcase selects the carrot through ordinary desktop
   and rendered touch-hotbar input, observes a visible adult approach, then
   selects a non-carrot without using either item and observes that same rabbit
   flee and gain meaningful separation.
4. Existing refuge, garden, raid, separation, collapse, identity, and
   zero-browser-persistence gates remain green.
5. Inspect native and Web captures, deploy the exact pushed revision, and
   repeat desktop and phone acceptance against the public URL.

## Non-Goals

- changing rabbit temptation distance, feeding, breeding, or avoidance AI;
- adding hand models, item-use animations, stealth, scent, or vision;
- showcase-specific simulation or direct harness commands to rabbit AI; or
- redesigning inventory replication beyond the selected carried slot.

