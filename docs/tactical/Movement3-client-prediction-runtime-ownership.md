# Movement3 - Client prediction runtime ownership

Standing after [`Movement2-authoritative-host-command-integration.md`](Movement2-authoritative-host-command-integration.md). `Movement2` made the host consume sequenced commands and publish acked movement snapshots. This slice plans the client-side owner for prediction before adding correction smoothing or remote interpolation.

## Goal

Define and test where client prediction runs, what world facts it owns, and what crosses the render-thread boundary.

At the end of `Movement3`, `mclone` should have a clear client prediction runtime model:

- browser render/UI thread stays lightweight
- command buffering, replay, reconciliation, and prediction diagnostics do not live on the render thread
- multiplayer clients carry a bounded prediction world, not a full authoritative host clone
- local singleplayer uses the same client-facing prediction path as remote multiplayer
- missing collision/prediction facts are explicit diagnostics or fallback states, not hidden drift

## Problem

Prediction is not just a math helper. To replay commands, the client needs the same movement parameters and collision-relevant world facts the host used. If that is not planned, one of two bad designs happens:

- prediction runs on the render thread and forces raw chunk/collision state into UI/GPU code
- remote clients grow into partial servers with worldgen, ticks, AI, persistence, and scheduling just to predict a local body

Neither should be the target.

## Target Ownership

| Owner | Responsibilities |
|---|---|
| Render/UI thread | raw input sampling, pointer lock, UI, GPU resources, draw submission, small presentation-state consumption |
| Client prediction worker | command clock, command records, unacked ring buffer, local predictor, reconciliation, correction diagnostics, bounded prediction world |
| Render-world/mesh worker | client chunk cache for render ingestion, section meshing inputs, mesh job products |
| Authoritative host | canonical chunks/entities/player bodies, worldgen, AI, block/liquid ticks, lighting, persistence, gameplay consequences |

The render thread may keep immediate look orientation for feel, but body prediction/replay should be worker-owned. It should receive compact presentation state such as predicted local pose, correction offset, remote interpolated poses, and debug counters.

## Prediction World Scope

The prediction worker should own a bounded client mirror:

- collision-relevant block/shape facts near the local player plus a safety margin
- chunk/collision revision facts for that prediction window
- movement physics profile and `physicsRevision`
- dynamic colliders only when they can affect local movement prediction
- authoritative local-player body snapshots with ack sequence

It should not own:

- worldgen
- NPC AI/pathfinding/spawning
- block ticks, liquid ticks, lighting, or scheduled tick execution
- persistence or save metadata mutation
- renderer meshes as collision truth
- the full server chunk set

If a command reaches unknown collision space, prediction should mark missing collision and either stop/reduce prediction or accept a classified correction on the next authoritative snapshot.

## Scope

Add or plan:

| # | Module | Expected result |
|---|---|---|
| 1 | Runtime boundary | interfaces for raw input samples in and small predicted presentation state out |
| 2 | Prediction world | minimal collision-window data model hydrated from client-facing chunk/collision snapshots |
| 3 | Worker ownership | client prediction worker shape for command clock, replay buffer, reconciliation, and diagnostics |
| 4 | Singleplayer parity | local host path feeds the same client prediction model instead of granting predictor host internals |
| 5 | In-memory harness | deterministic latency/jitter/loss tests that do not rely on HTTP and do not run prediction on render-thread objects |
| 6 | Fallback policy | explicit no-prediction or reduced-prediction behavior when collision facts are missing |

Do not add:

- WebSocket/WebRTC/WebTransport implementation
- final correction smoothing
- remote entity interpolation buffers beyond interface sketches
- non-full block shape expansion unless needed to define the data boundary
- any render-thread chunk/collision cache

## Open Questions

- Is the prediction worker a dedicated worker, or part of a broader client runtime worker that also owns transport and client session state?
- Do render-world and prediction-world workers each keep derived views of packed chunks, or should a client world worker own packed facts and serve both?
- What is the first collision-window margin around the local player, and how does it relate to chunk interest?
- What exact message tells the predictor that collision facts are missing or revision-mismatched?
- How much look/camera immediacy remains on the render thread while body prediction is worker-owned?

## Validation

Minimum planning/implementation validation:

- unit tests prove predictor replay uses only prediction-world interfaces, not host internals
- in-memory transport tests cover latency, jitter, dropped/superseded snapshots, and delayed acks
- missing collision fixtures produce explicit diagnostics or fallback, not silent drift
- render-thread-facing API contains only input samples, presentation poses, and debug counters
- local singleplayer and remote multiplayer use the same prediction-facing contracts
- `pnpm typecheck`
- `git diff --check`

## Done When

- The docs and interfaces make it impossible to assume prediction runs in the render thread.
- The client prediction model is bounded to collision-relevant facts instead of a full host clone.
- The next smoothing/interpolation slice has a concrete owner for predictor state and presentation output.

## Next Step

`Movement4-interpolation-and-correction-smoothing.md`: once prediction ownership is explicit, add local visual correction offsets, remote interpolation buffers, and browser-facing smoothing controls on top of the worker-owned presentation state.
