# Client Prediction and Movement Authority Topic

Topic: client-prediction

Status: client-authoritative movement (vanilla-shaped) is the deliberate
interim; server-side validation is planned; sequenced input replay is a
preserved future path, not scheduled work.

Scope: who owns player movement truth, what the server checks, how remote
entities are smoothed, and which protocol/design decisions must be made now
so that server-side input replay and stronger validation remain cheap to add
later. The wire/session/tick plan lives in
[`multiplayer-networking.md`](multiplayer-networking.md); vanilla receipts in
[`vanilla/networking.md`](vanilla/networking.md)
(movement send, validation walkthrough, teleport/ack, interpolation).

## Current state (verified 2026-07-14)

- **Local movement is client-simulated and client-authoritative**, matching
  vanilla's model. The client runs its own physics
  (`mclone-client/src/player.rs:864-1078`) and emits vanilla-shaped
  `MovePlayer` variants with vanilla's exact thresholds: position when
  squared delta > `9.0e-4`, rotation on change, reminder every 20 publication
  attempts, `StatusOnly` on ground flip
  (`LOCAL_PLAYER_POSITION_SYNC_DELTA_SQR`/`..._REMINDER_INTERVAL`,
  `mclone-client/src/player.rs:65-66,909-964`).
- **Pose publication is scene-owned across platforms.** Mono and XR frame
  entry points advance one `mclone-scene` deadline at no more than 20 Hz,
  with wall-clock slip instead of catch-up bursts. Look-only changes reach the
  selector and produce `MovePlayerCommand::Rot`; idle calls make the
  20-attempt position reminder approximately one second. Desktop, browser,
  Android, and XR adapters cannot select or commit movement packets directly.
  Publication uses `SendOnly`, so it enqueues outbound work without draining
  inbound updates or waiting for an acknowledgement.
- **The server accepts reported positions with clamps only**: finiteness
  checks and vanilla coordinate clamps, plus the pending-teleport gate
  (`mclone-server/src/player.rs:63-95`). There is **no collision replay, no
  speed check, no floating check** — a client can teleport anywhere.
- **The teleport-correction/ack loop is implemented**: corrections carry
  relative flags + `teleport_id`; moves are ignored while a teleport is
  outstanding and the correction is re-sent; the client acks with
  `AcceptTeleport` and resyncs
  (`mclone-server/src/player.rs:97-157`,
  `mclone-server/src/integrated.rs:1230-1258`,
  `mclone-app-runtime/src/camera_reconcile.rs:149-186`). This is the vanilla
  mechanism and is the foundation the validation checks reject into.
- **Dedicated sessions buffer movement** and flush it before other ordered
  commands with packet-count bookkeeping
  (`mclone-dedicated-server/src/session.rs:138-183`) — the counter the
  vanilla burst-factor check needs already exists.
- **Remote entity smoothing diverges from vanilla**: mclone smooths actors
  toward the latest authoritative target with a frame-rate-independent
  exponential half-life (`interpolation_factor(dt, half_life)`,
  `mclone-client/src/actor.rs:155,298-312`), where vanilla converges exactly
  in N ticks (N=3) against tracked packet coordinates. The exponential form
  is cadence-independent (good for variable tick and XR frame rates) but
  never exactly converges and can lag differently at different frame rates.
  Keep, but note the parity lever: vanilla's fixed-window lerp is the
  fallback if remote motion ever needs exact vanilla feel.

## Vanilla baseline (what "as good as Minecraft" means here)

Vanilla 1.17.1 is client-authoritative with server sanity checks — full
receipts in the reference doc. The checks, in order: NaN/coordinate clamps,
pending-teleport gate, packet-burst factor (>5/tick clamped),
"moved too quickly" (`distSq - velocitySq > 100 (300 elytra) × burst`),
server-side collision replay of the client delta, "moved wrongly"
(post-replay residual > 0.0625 dist-sq), accept-or-rollback against new
collisions, and the 80-tick floating kick. Corrections are hard snaps with a
teleport id; vanilla does **no input replay** on correction. The
singleplayer owner is exempt from the speed check.

Block/item actions are separately speculative in vanilla (per-action break
acks, unconditional placement corrections, stateId inventory resync) — see
the reference doc; those follow the same "predict locally, reconcile via
authoritative fact" pattern and are out of scope for this topic.

## Direction: two stages, one preserved path

### Stage 1 (planned): vanilla validation checks

Port the `handleMovePlayer` checks onto the existing correction loop —
phase 4 of the multiplayer-networking plan. All the ingredients exist:
per-session move-packet counts, teleport-id rejection path, and server
collision via the shared physics runtime. Scope:

- packet-burst clamp; "moved too quickly" with vanilla thresholds;
- server collision replay of the reported delta + "moved wrongly" residual
  check; rollback teleport on failure or new-collision overlap;
- floating/fly kick (once abilities/game-mode exist enough to exempt
  legitimate flight);
- exempt the local-integrated player the way vanilla exempts the
  singleplayer owner.

This delivers "the client can't just teleport around" at vanilla strength
without touching movement feel, and it is the correct baseline regardless of
whether Stage 2 ever ships.

### Stage 2 (preserved, not scheduled): sequenced input replay

The long-term shape both
[`../minecraft-client-replica-research.md`](../minecraft-client-replica-research.md)
and [`../player-movement-netcode.md`](../player-movement-netcode.md) point
at: the client sends sequenced input command records; the server drains them
by sequence and simulates the same quantum the client predicted; corrections
reference the last-applied sequence so the client replays unacked inputs
instead of hard-snapping. This upgrades movement from "validated
client-authoritative" to server-authoritative with client prediction, and is
the real anti-cheat/fairness endpoint. It is deliberately deferred — vanilla
itself never does this, and the interim model is fine for current gameplay.

Decisions to make **now** so Stage 2 stays cheap:

- **Sequence-number the movement stream early.** Adding a `seq: u32` to
  `MovePlayer` (and echoing `last_applied_seq` in corrections) is a small
  protocol change that Stage 1's burst accounting can use immediately and
  Stage 2 requires. Do it when the session-layer protocol changes land, to
  avoid an extra version bump.
- **Keep the client's input→physics quantum explicit.** The controller
  already normalizes to a 20 Hz tick-equivalent
  (`LOCAL_PLAYER_TICKS_PER_SECOND`, `mclone-client/src/player.rs:20,1276`);
  Stage 2 needs inputs recorded per fixed quantum, so avoid smearing input
  application across variable frame dt in ways that can't be replayed.
- **Keep server movement application in shared code** (`mclone-server` +
  shared physics), never app-local, so the replay simulation has one home.
- **Keep view pose, body heading, and locomotion reference conceptually
  separate.** They currently collapse to one network pose, including XR, but
  a future tracked head may rotate independently of the movement/body heading.
  Extend the shared pose/protocol contract when that distinction becomes
  observable; do not encode it as an XR-only send cadence.
- **Corrections must stay id-gated** (already true) — replay reconciliation
  is an extension of the teleport-ack loop, not a replacement.

## Variable-tick interplay

- The current scene deadline makes the 20-attempt move reminder one second at
  its fixed 20 Hz vanilla baseline. It should become handshake-rate-aware when
  variable publication rates land, so a 60 Hz server does not triple movement
  chatter.
- Vanilla's per-tick thresholds ("too quickly" per packet-burst) assume the
  server tick as the accounting window; when the tick rate is configurable,
  the burst window follows the gameplay/publication lane, not wall-clock
  frames.
- Remote-actor smoothing is already dt-based (half-life), so publication-rate
  changes only affect how stale targets are; if the publication lane runs
  faster than 20 Hz, consider tightening the half-life to match.

## Recommended next work

1. Ride the sequence-number + `last_applied_seq` fields on the phase-3
   session/protocol changes (multiplayer-networking plan).
2. Implement Stage 1 checks as their own tactical after the dedicated server
   has an autonomous tick (validation windows are per-tick).
3. Leave Stage 2 unscheduled; revisit when gameplay needs server-auth
   movement (combat, competitive play) or cheating becomes real.

## Non-goals

- Client-side prediction of other entities/AI (no vanilla precedent; remote
  actors stay interpolation-only).
- Fluid/block-tick prediction (server-authoritative; see the client-replica
  research doc).
- Anti-cheat beyond vanilla-strength movement checks.
