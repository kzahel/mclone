# Client Prediction and Movement Authority Topic

Topic: client-prediction

Status: permissive client-authoritative movement is the accepted direction as
of 2026-07-23, not an interim awaiting anti-cheat work. Local movement runs
from recorded, sequenced 60 Hz semantic commands. The server accepts reported
finite poses after world-bound clamping and pending-teleport checks; speed,
collision, floating, semantic-command transport, authoritative movement
replay, and correction replay are intentionally not planned.

Scope: who owns player movement truth, what the server checks, and the
accepted boundary between responsive local movement and server-owned world
state. Remote-player cadence, component poses, and interpolation live in
[`remote-player-presentation.md`](remote-player-presentation.md). The
wire/session/tick plan lives in
[`multiplayer-networking.md`](multiplayer-networking.md); vanilla receipts in
[`vanilla/networking.md`](vanilla/networking.md)
(movement send, validation walkthrough, teleport/ack, interpolation).
Preservation of physical input order and timing before semantic command
materialization lives in
[`input-observation-timeline.md`](input-observation-timeline.md).

## Current state (verified 2026-07-23)

- **Local movement is client-simulated and client-authoritative**, matching
  vanilla's model. The client runs its own physics
  (`mclone-client/src/player.rs:864-1078`) and emits vanilla-shaped
  `MovePlayer` variants with vanilla's exact thresholds: position when
  squared delta > `9.0e-4`, rotation on change, reminder every 20 publication
  attempts, `StatusOnly` on ground flip
  (`LOCAL_PLAYER_POSITION_SYNC_DELTA_SQR`/`..._REMINDER_INTERVAL`,
  `mclone-client/src/player.rs:65-66,909-964`).
- **Local movement integration is fixed at 60 Hz by default and is not driven
  by render delta.** `mclone-scene` owns a configurable player movement clock,
  consumes a bounded timestamped semantic-input timeline, emits recorded
  `PlayerMovementCommand { epoch, sequence, input }` quanta, retains jump
  edges across zero-step frames, and interpolates only the presentation eye
  between committed movement poses.
  Desktop, browser, flat Android, and XR all use that owner. The default
  12-step catch-up bound preserves full movement through a 200 ms visible
  frame and exposes an explicit sequence gap for dropped work beyond it.
  Lifecycle transitions, teleports, server corrections, world swaps, and
  movement-policy changes advance the command epoch and snap the timeline
  instead of interpolating from stale state.
- **Walking recurrence is rate-independent at free motion boundaries.** The
  vanilla-style impulse/drag/gravity affine recurrence now has a fractional
  fixed-step form, so three 60 Hz steps compose to one former 20 Hz step in
  unobstructed motion. Collision is still evaluated at every smaller quantum,
  which is the intended higher-rate behavior. Fly/no-clip movement also keeps
  sub-unit analog throttle instead of normalizing every non-zero stick vector
  to full speed.
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
  speed check, no floating check** — a finite client can teleport, fly, or
  pass through collision. This is the accepted cooperative trust model.
- **The teleport-correction/ack loop is implemented**: corrections carry
  relative flags + `teleport_id`; moves are ignored while a teleport is
  outstanding and the correction is re-sent; the client acks with
  `AcceptTeleport` and resyncs
  (`mclone-server/src/player.rs:97-157`,
  `mclone-server/src/integrated.rs:1230-1258`,
  `mclone-app-runtime/src/camera_reconcile.rs:149-186`). This is the vanilla
  mechanism for server-directed spawn, restore, transfer, respawn, and other
  explicit relocation; it is not a routine movement-validation loop.
- **Dedicated sessions buffer movement** and flush it before other ordered
  commands with packet-count bookkeeping
  (`mclone-dedicated-server/src/session.rs:138-183`). This preserves transport
  ordering; the bookkeeping is not a commitment to movement validation.
- **Remote entity smoothing diverges from vanilla**: mclone smooths actors
  toward the latest authoritative target with a frame-rate-independent
  exponential half-life (`interpolation_factor(dt, half_life)`,
  `mclone-client/src/actor.rs:155,298-312`), where vanilla converges exactly
  in N ticks (N=3) against tracked packet coordinates. The exponential form
  is cadence-independent (good for variable tick and XR frame rates) but
  never exactly converges and can lag differently at different frame rates.
  Keep, but note the parity lever: vanilla's fixed-window lerp is the
  fallback if remote motion ever needs exact vanilla feel. The replacement
  direction for remote players is owned by
  [`remote-player-presentation.md`](remote-player-presentation.md).

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

## Accepted direction: trust the client for movement

Movement uses a cooperative-client trust boundary. In both integrated and
dedicated sessions, the client simulates its player and reports the resulting
pose. The server:

- rejects malformed protocol data and non-finite position or rotation values;
- clamps coordinates to the supported world bounds;
- normalizes rotations;
- ignores ordinary movement while a server teleport awaits acknowledgement;
- otherwise accepts the reported position, rotation, and on-ground state.

The server does not replay player collision, check speed, detect floating or
flight, transport semantic movement commands, or reconcile a predicted input
history. A finite modified client can therefore teleport, fly, or move through
collision. That tradeoff is accepted indefinitely for the current cooperative
product direction.

The accepted pose becomes the server's current fact for interest management,
observer replication, persistence, and location-dependent gameplay. This does
not make inventory, entities, world mutation, health, or other gameplay facts
client-authoritative; those systems retain their own server-owned contracts.

The integrated server must not duplicate the client's 60 Hz movement or
collision work. It consumes the published pose and spends its simulation
budget on the world. Dedicated sessions deliberately use the same permissive
movement policy rather than maintaining a second authority model.

Existing `MovePlayer` sequence values and
`PlayerPositionUpdate.last_applied_move_sequence` preserve pose-publication
ordering and teleport continuity. They are not acknowledgements for semantic
movement commands and do not imply replay. The local
`PlayerMovementCommand` recording remains useful for diagnostics,
deterministic tests, and future local replay without requiring a network
schema, server command queue, or server movement consumer.

### Dormant alternatives are not roadmap work

Vanilla-style movement validation and full authoritative command replay remain
known alternatives, not scheduled stages. Do not add complexity solely to
prepare for either one. Reopen this decision only through an explicit product
direction change, such as competitive play or a concrete mechanic that cannot
work with client-reported poses.

## Variable-tick interplay

- The 60 Hz local movement clock, the current 20 Hz pose-publication deadline,
  the configurable host/world/physics cadence, and render cadence are separate
  lanes. Changing the integrated server from 20 to 60 Hz therefore does not
  triple local movement or movement packets.
- The current publication deadline makes the 20-attempt move reminder one
  second at its fixed 20 Hz vanilla baseline. It should become
  handshake-rate-aware when variable publication rates land.
- No server-side 60 Hz player-movement lane is required. A server cadence
  change affects when accepted poses are consumed and published, not how often
  local movement is simulated.
- Remote-actor smoothing is already dt-based (half-life), so publication-rate
  changes only affect how stale targets are; if the publication lane runs
  faster than 20 Hz, consider tightening the half-life to match.

## Recommended next work

1. Continue improving local input collection, movement feel, and presentation
   independently of server, world, AI, render, and publication rates.
2. Implement the independently configurable pose-report, replication, and
   remote snapshot-presentation work stream in
   [`remote-player-presentation.md`](remote-player-presentation.md).
3. Preserve finite-value rejection, coordinate clamps, sequence ordering, and
   the teleport acknowledgement gate. Do not add movement validation or replay
   without an explicit product-direction change.

## Non-goals

- Client-side prediction of other entities/AI (no vanilla precedent; remote
  actors stay interpolation-only).
- Fluid/block-tick prediction (server-authoritative; see the client-replica
  research doc).
- Vanilla-strength movement anti-cheat.
- Authoritative server movement simulation, semantic-command transport, or
  correction replay.
