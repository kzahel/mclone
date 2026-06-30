# 116: Simulation Cadence and Stepping

Status: active; initial architecture note recorded, with 60 Hz physics substeps
landed inside the existing 20 Hz server tick.

## Purpose

Keep Minecraft-compatible gameplay timing where it matters without letting one
global 20 Hz tick become the permanent cadence for physics, AI, networking, and
render presentation.

Minecraft Java's simple 20 TPS model is useful for vanilla parity, scheduled
block/fluid behavior, deterministic chunk work, and gameplay rules. It is a poor
fit for rigid-body physics, high-speed collisions, camera/render smoothness, and
eventual AI or networking policies that naturally want different cadences.

The target architecture is a configurable host pump with named fixed-rate
simulation lanes. A higher host cadence must not automatically make every
vanilla system run more often.

## Cadence Lanes

- Host/server pump: configurable long-term, with 20/30/60 Hz as plausible
  modes. This owns wall-clock pacing and command intake, not gameplay policy.
- Vanilla gameplay lane: default 20 Hz. Scheduled block ticks, fluid ticks,
  random ticks, entity age, and parity-sensitive world rules belong here unless
  a specific tactical changes the vanilla target.
- Physics lane: fixed at 60 Hz for the first Rapier-backed implementation, with
  room for 120 Hz later if stability or tunneling requires it. Physics can
  substep inside a 20 Hz host tick or run once per host frame in a 60 Hz host
  mode.
- AI lane: lower-frequency decision cadence, likely 5-10 Hz for expensive
  thinking, with movement intents fed into the normal authoritative simulation.
- Networking lane: snapshot and event publication rates are selected per entity
  class and connection mode. Network publication rate is not the same thing as
  internal simulation rate.
- Client render lane: frame-rate driven. Rendering consumes presentation state,
  interpolation, and later prediction/extrapolation rather than driving
  authoritative simulation.

## Invariants

- Do not use a single global `tick` value to mean all subsystem time.
- The vanilla gameplay lane remains 20 Hz by default even if the host pump runs
  faster.
- Physics substeps may improve authoritative simulation quality without
  increasing network snapshot frequency.
- Client prediction or local render sampling is allowed only behind shared
  command/state contracts. It must not create a local-only architecture fork.
- Web/WASM remains part of the target shape: browser worker timing should map to
  the same lanes rather than becoming a reduced single-threaded design.

## Current Slice

For the debug physics experiment, keep the server runner at 20 Hz and step
Rapier three times at 1/60 second per server tick. Publish entity state once per
server tick for now.

The expected benefit is better collision/contact integration and more stable
settling. Visual smoothness remains limited by the 20 Hz entity publication path
and the current actor interpolation model.

## Follow-Up Work

- Add explicit diagnostics for physics substep count and physics lane timing.
- Publish full physics orientation for debug cubes instead of yaw/pitch only.
- Evaluate active-physics presentation options: higher-rate snapshots for local
  integrated play, velocity/angular-velocity extrapolation, or a shared
  prediction path.
- Design configurable host pump rates without changing the default 20 Hz
  vanilla gameplay lane.
- Decide which entity classes can opt into higher-rate network snapshots.
