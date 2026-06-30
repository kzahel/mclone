# 116: Simulation Cadence and Stepping

Status: active; initial architecture note recorded, 60 Hz physics substeps landed
inside the existing 20 Hz server tick, and the first shared server cadence
primitive landed for future configurable host pump work.

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

## Landed Slices

For the debug physics experiment, keep the server runner at 20 Hz and step
Rapier three times at 1/60 second per server tick. Publish entity state once per
server tick for now.

The expected benefit is better collision/contact integration and more stable
settling. Visual smoothness remains limited by the 20 Hz entity publication path
and the current actor interpolation model.

- Added `SimulationCadence` in `mclone-server` as the first host/lane stepping
  primitive. It maps one configurable host frame into fixed-rate gameplay and
  physics lane work without changing live runner behavior yet.
- Current tested mappings:
  - 20 Hz host: one 20 Hz gameplay tick and three 60 Hz physics steps per host
    frame.
  - 60 Hz host: one 60 Hz physics step per host frame and one 20 Hz gameplay
    tick every third host frame.
  - 30 Hz host: deterministic fractional gameplay cadence with two 60 Hz physics
    steps per host frame.
- The elapsed-time wrapper accumulates partial host frames and caps catch-up
  work so a long frame cannot force unlimited simulation work in one pump.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml -p mclone-server -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-server cadence -- --nocapture
cargo check --manifest-path native/Cargo.toml -p mclone-server
```

## Follow-Up Work

- Add explicit diagnostics for physics substep count and physics lane timing.
- Extend the full-orientation path with angular velocity or buffered orientation
  samples if active physics presentation still looks too 20 Hz.
- Evaluate active-physics presentation options: higher-rate snapshots for local
  integrated play, velocity/angular-velocity extrapolation, or a shared
  prediction path.
- Route the native server runner through `SimulationCadence` while preserving
  the default 20 Hz host behavior.
- Add a dev-only configurable host pump rate without changing the default 20 Hz
  vanilla gameplay lane.
- Decide which entity classes can opt into higher-rate network snapshots.
