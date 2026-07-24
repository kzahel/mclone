# Input Observation Timeline

Topic: `input-observation-timeline`

Status: local end-to-end implementation and automated cross-platform
validation completed 2026-07-23; real-device acceptance remains open.
The shared bounded `ControllerInputBatch` and multi-observation semantic
reduction preserve ordered input through platform collection and scene
routing. The scene materializes timestamped semantic state as sequenced 60 Hz
`PlayerMovementCommand` records, applies the same records to local prediction,
and retains a bounded recording for deterministic replay. The accepted
permissive movement-authority direction deliberately keeps semantic commands
local: server transport, authoritative replay, and correction replay are not
planned. See [`client-prediction.md`](client-prediction.md).

Scope: preserve the best physical-input order and timing each platform can
provide, normalize it behind a host-neutral contract, reduce it into shared
semantic input, and materialize deterministic 60 Hz player commands without
making render cadence, browser limitations, or platform event APIs part of
gameplay policy.

Controller mappings, contexts, action meanings, prompts, and device
capabilities remain owned by
[`controller-input.md`](controller-input.md). Movement authority, network
sequence numbers, server validation, prediction, and correction replay remain
owned by [`client-prediction.md`](client-prediction.md). This topic owns the
observation timeline between physical collection and those shared semantic
consumers.

## Problem

The render frame is a valid place to drain or poll a platform input API. It
must not become the unit of input truth.

The current gamepad paths reduce every source to one final snapshot per render
frame before shared semantics run. That is the browser's unavoidable common
denominator, but it throws information away on platforms that provide more:

```text
press ── release ─────────────── next render-frame drain
          platform queue has both events

current:
platform queue -> final released snapshot -> no shared press edge

target:
platform queue -> ordered observations -> shared press + release edges
                                      \-> final held state is released
```

The same loss affects fast axis changes during a long frame. A 60 Hz movement
clock cannot reconstruct six distinct command quanta from one state sampled
after a 100 ms stall. Increasing the fixed movement rate alone does not solve
that information loss.

The target is therefore not an independent busy input thread on every
platform. It is a loss-aware timeline:

```text
platform API
    |
    v
ordered neutral observations + terminal source snapshots
    |
    v
shared semantic reducer
    |
    v
timestamped, sequenced 60 Hz PlayerCommands
    |                              |
    v                              v
local prediction             local recording/tests
```

## Current State (Verified 2026-07-24)

All interactive hosts already use the correct broad ordering:

```text
collect platform input -> route shared semantics -> move -> render
```

Keyboard, mouse, and touch enter through event-oriented paths. Controller-like
sources now project their available capabilities as follows:

`mclone-input` now owns the bounded physical batch and semantic result types.
An event-capable batch may contain multiple ordered canonical snapshots per
source plus terminal snapshots. `ControllerInputSession::sample_batch`
preserves within-batch press/release order, unions observed edges for legacy
frame consumers, exposes the ordered semantic states for later command
materialization, clamps regressing order deterministically, and resynchronizes
discontinuous batches without inventing a press.

| Platform | API shape available | Current projection | Information lost |
| --- | --- | --- | --- |
| Browser | `navigator.getGamepads()` current snapshots, normally polled once per `requestAnimationFrame` | Rust/Wasm emits one timestamped W3C standard-mapped observation plus terminal state per source before scene advance | Transitions between browser samples remain unavailable to the application |
| macOS desktop | Apple GameController extended-profile value-change callbacks plus current state | The host captures one canonical snapshot per callback and polls final profile state for recovery | Framework history is preserved between main-queue drains; real-device feel and lifecycle acceptance remain |
| Linux/Windows desktop | GilRs ordered events plus cached gamepad state | The host captures canonical state after each drained event and supplies final cached state for recovery | Backend history is preserved; hardware/driver acceptance remains |
| Android | Java `KeyEvent` and `MotionEvent` callbacks queued through JNI | Java forwards event times and historical motion samples; Rust emits canonical state after each queued sample and terminal state | Backend history is preserved within the bounded queue; hardware/driver acceptance remains |
| OpenXR | Action state observed at `sync_actions` cadence, including runtime change/time facts where exposed | The host reads current action state once per XR frame and retains changed-since-sync/source-time metadata per action | The runtime does not promise arbitrary physical event history |
| Scripted/offscreen | Fully controlled synthetic input | Usually one requested snapshot per frame | Nothing inherent; it can exercise the richer contract deterministically |

Browser TypeScript is already a deliberately mechanical forwarder. Browser
gamepad polling lives in Rust/Wasm before scene advancement; no JavaScript
gameplay mapping needs to be added. Running `setInterval` beside
`requestAnimationFrame` would not create a dependable high-frequency input
lane: browsers may throttle timers, the Gamepad API remains snapshot-based,
and the extra callbacks still need ordering against scene time. Polling once
per animation frame is the honest browser behavior.

Linux and Windows desktop use GilRs rather than SDL for ordinary controllers.
GilRs events carry source identity, event kind, and a timestamp. The drain loop
captures the canonical cached state after every event and appends a final
cached snapshot for recovery.

macOS uses Apple GameController extended profiles. The framework normalizes
sticks, triggers, buttons, and controller families instead of exposing generic
HID usage guesses. A copied value-change handler appends a bounded canonical
snapshot after each changed element, and the render/input drain polls terminal
profile state for loss recovery. The native binary embeds controller-support
metadata so direct Cargo-launched builds receive the same framework enumeration
as a bundled application.

Android Java remains correctly domain-blind. It now forwards Android event
times and historical controller-axis samples through JNI. The Rust collector
applies them in order and appends a terminal snapshot without moving action
mapping into Java.

OpenXR action sampling is intentionally frame-shaped. Preserve the facts the
runtime supplies, but do not invent a transition history or add a free-running
poller outside the OpenXR action-sync lifecycle.

## Accepted Direction

### Preserve capabilities instead of choosing the lowest denominator

Platform collectors remain thin and mechanical, but may emit a richer neutral
batch than one terminal snapshot:

- ordered source connect, disconnect, and replacement observations;
- ordered normalized control observations when the backend exposes them;
- a normalized sample time and monotonically increasing sequence tie-breaker;
- a terminal snapshot for every connected source represented in the batch;
- an explicit discontinuity marker if a bounded queue overflows or a backend
  cannot preserve continuity.

Exact Rust names and whether observations carry full canonical snapshots or
canonical control deltas are implementation details. The stable contract is
that applying observations in order reaches the terminal snapshot, while a
snapshot-only backend may legally provide just one observation.

No platform handle, wall-clock timestamp, browser index, Android device ID,
GilRs ID, or OpenXR path crosses this boundary. Existing session-local
`InputSourceId` values remain the shared identities.

### Normalize time without claiming false precision

Absolute timestamps from different platform APIs are not comparable. Each
collector maps available source times onto a monotonically nondecreasing
duration since its local input-session origin:

- preserve source ordering first;
- use a collector-assigned sequence number to break equal timestamps;
- clamp or diagnose regressing backend timestamps;
- stamp receipt/order time when the backend exposes no useful source time;
- keep backend-native times only as local diagnostics;
- never serialize a platform or wall-clock timestamp as replay truth.

Movement command boundaries use the scene's monotonic movement timeline. An
adapter maps observations into that timeline when it drains them. Observations
older than the retained window are handled by the explicit discontinuity
policy rather than silently replayed late.

### Reduce physical observations into shared semantics

`mclone-input` continues to own dead zones, response curves, trigger
hysteresis, bindings, contexts, edge generation, repeat, active-source
arbitration, and UI meaning.

The reducer must accept zero or more observations for the same source in one
presentation frame. It must:

- preserve a press and release that both occur between render frames;
- expose the final held state separately from all edges observed in the batch;
- retain ordered semantic changes needed to build movement commands;
- perform UI repeat and active-source decisions from normalized time, not
  render-frame count;
- produce the same semantic trace for equivalent canonical observations from
  different platform adapters.

Platform code must not decide which physical transition means Jump, Confirm,
Attack, or any other game action.

### Materialize movement commands at the simulation boundary

The scene-owned fixed clock consumes the semantic timeline and builds one
`PlayerMovementCommand` for each 60 Hz quantum:

- a sequence number;
- the quantum duration or movement-clock boundary;
- movement axes and locomotion modifiers applicable to that quantum;
- ordered one-shot action edges assigned to that quantum;
- view/body facts only to the extent required by the movement contract.

Pointer-look deltas, controller look rate, and held movement axes have
different physical meanings. The command builder should integrate or sample
each according to that meaning rather than treating every control as a
variable-frame delta.

`PlayerMovementCommand` carries an input epoch, a quantum sequence number, and
the complete host-neutral `EngineCameraInput` applied for that quantum. The
command builder:

- assigns ordered held-state and one-shot jump changes to their command
  boundary;
- samples movement axes and modifiers as state;
- integrates keyboard/controller look rates across command time;
- preserves exact pointer/touch view yaw and pitch observations;
- does not backdate a newly sampled XR locomotion heading onto older ordinary
  gamepad history;
- preserves view-directed fly/no-clip pitch while retaining XR's explicit
  horizontal locomotion-yaw contract.

The pending semantic queue is bounded at 2,048 observations. Local command
recording retains the latest 1,024 commands and reports dropped recordings.
Catch-up remains bounded; skipped movement work advances the command sequence,
so the next emitted record exposes an explicit gap rather than pretending the
commands ran.

Raw physical observations are local and disposable. Deterministic local replay
uses semantic player commands, not GilRs, Android, browser, or OpenXR records.
This keeps replay independent of controller layout and platform API without
implying a future network transport.

The default command rate remains 60 Hz. It is a configurable simulation lane,
not a promise that the renderer, server world, AI, pose publication, or
network packet cadence also runs at 60 Hz.

## Platform Projections

### Browser

- Continue polling standard-mapped Gamepad sources once near the
  `requestAnimationFrame` scene boundary in Rust/Wasm.
- Emit one timestamped terminal observation per connected source.
- Treat `Gamepad.timestamp`, where useful and trustworthy, as ordering or
  diagnostic input rather than proof of unseen history.
- Do not add a timer poller merely to imitate an event API.
- Keep keyboard, pointer, and touch on their existing event-oriented paths.

At very low browser frame rates, a gamepad tap that begins and ends between
two browser samples may remain unobservable. That is a platform capability
limit, not a reason to lower fidelity on native targets.

### Desktop

- On macOS, retain every Apple GameController extended-profile value-change
  callback in dispatch order.
- On Linux and Windows, retain every meaningful GilRs event in drain order.
- After each callback/event, project the resulting source state into a
  canonical observation, or project an equivalent canonical control delta.
- Normalize backend sample time onto the local monotonic input timeline.
- Emit a terminal snapshot for resynchronization.
- Preserve hotplug/source lifecycle order.

No separate input thread is required for the current implementation:
GameController dispatches copied handlers and GilRs queues events between
render-loop drains. A collector thread becomes useful only if measurement shows
backend queue loss or latency that draining at the host event boundary cannot
address.

### Android

- Extend the domain-blind Java/JNI bridge to forward event time and ordered
  samples.
- Forward useful `MotionEvent` historical controller-axis samples before the
  current sample.
- Apply each queued sample to the Rust per-device state and emit the
  corresponding canonical observation.
- Retain a terminal snapshot and explicit device lifecycle order.

Java remains unaware of dead zones, bindings, contexts, or gameplay actions.

### OpenXR

- Continue sampling only inside the required OpenXR action synchronization
  lifecycle.
- Preserve current state and useful runtime-supplied change/time metadata.
- Project the observations into the same semantic timeline where possible.
- Keep tracked poses, rays, hands, and XR-only analog facts in typed XR input,
  not in an ordinary-gamepad fiction.
- Do not fabricate intermediate samples the runtime did not expose.

## Boundedness, Lifecycle, and Recovery

Every collector queue and retained semantic history must be bounded.
Overflow is observable and recoverable:

- record a diagnostic counter and a discontinuity marker;
- discard stale intermediate history;
- apply the newest terminal source snapshot as authoritative held state;
- do not invent pressed or released edges for the missing interval;
- prevent a lost release from leaving a button or action stuck;
- prevent stale pre-pause input from replaying after resume.

Lifecycle clear, focus loss, controller replacement, world/session replacement,
and movement timeline snap advance an input epoch. Pending observations from
an older epoch are rejected. A source that remains physically held across a
clear re-arms according to the existing neutral/reconnect policy rather than
creating an unexplained fresh press.

Long-frame movement catch-up remains bounded by the movement clock. Input
history does not authorize an unbounded simulation spiral; if movement quanta
are dropped, their associated command-history discontinuity is recorded
explicitly.

## Validation Contract

Automated acceptance requires:

1. A press and release between two presentation frames yields both semantic
   edges and a released final held state on event-capable collectors.
2. Several axis changes across a 100 ms presentation stall are assigned to
   the correct six 60 Hz commands rather than collapsed into one final state.
3. Equivalent desktop, Android, browser-snapshot, OpenXR-snapshot, and
   scripted canonical traces reduce equivalently within their declared
   capability.
4. Lifecycle clear cannot replay an old edge or leave an action held.
5. Queue overflow resynchronizes to the terminal snapshot without a stuck
   control and reports the discontinuity.
6. Equal and regressing timestamps remain deterministic through sequence
   ordering and clamping.
7. UI edges and repeat are independent of presentation-frame count.
8. Command recording and replay reproduce the same movement result under
   different render cadences.
9. Browser tests explicitly prove snapshot semantics rather than claiming
   unobservable between-frame transitions.

Compilation and synthetic tests can close the shared contract on every target.
Final product acceptance still requires representative physical controllers,
Android hardware, and OpenXR runtimes because backend drivers can differ in
event shape, timestamps, noise, and lifecycle behavior.

### Automated evidence (2026-07-23)

- On 2026-07-24, macOS-specific native-client tests proved Apple extended
  profile projection for both sticks, independent triggers, face/menu buttons,
  and bounded callback overflow reporting. A rebuilt direct-launch product
  binary embedded the required controller metadata and enumerated the attached
  Xbox source through GameController. A physical product trace showed
  independent left/right stick and button transitions, and the operator
  accepted all Xbox bindings in-game.
- The full native Rust workspace test suite passes, including scripted
  controller ordering, Android historical samples, scene command assignment,
  fly/no-clip historical pitch, queue overflow recovery, epoch reset, command
  gaps, cadence-independent traces, and local command replay.
- The 2026-07-24 `pnpm native:thin-adapters:purity` rerun still reports the
  pre-existing player-pose publication-policy finding in
  `winit_frame_driver.rs`; it did not flag the controller adapter.
- `pnpm native:web:build` and `pnpm native:web:typecheck` pass for
  `wasm32-unknown-unknown`. The browser collector test proves that one rAF poll
  emits exactly one timestamped snapshot and terminal state rather than
  claiming unobservable history.
- Headed-Wayland `native:web:app-smoke` passes with fly/no-clip movement at the
  shared 60 Hz player rate. `native:web:mobile-smoke` passes touch movement,
  look, jump, attack, use, menu, and lifecycle probes; its active joystick
  segment moved the camera and rendered an in-bounds joystick. Both captures
  were visually inspected.
- `pnpm native:android:apk` and `pnpm native:android-xr:apk` pass, compiling
  the event-time/history Java bridge, JNI contract, shared timeline, and both
  Android app surfaces for `arm64-v8a`.
- `pnpm native:movement:smoke` and `pnpm native:xr-emulation:smoke` pass; the
  stereo capture was visually inspected.

Still required for product acceptance: physical desktop controllers, a phone
with touch and browser Gamepad hardware, Android controller hardware, and
representative OpenXR runtimes/headsets. Those checks validate backend feel
and driver facts; they do not require another architecture.

## Implementation Sequence

1. Complete: add a bounded, timestamped canonical observation batch and
   deterministic scripted fixtures in `mclone-input`.
2. Complete: teach `ControllerInputSession` to consume multiple ordered
   observations per source while preserving final state and all observed
   edges.
3. Complete: adapt macOS GameController, Linux/Windows GilRs, and Android
   Java/JNI/Rust collectors to retain their event histories; project browser
   and OpenXR honestly onto the same contract.
4. Complete: extend scene routing with a bounded semantic timeline and
   materialize one sequenced command per fixed movement quantum.
5. Complete: record/replay those semantic commands locally and prove
   cadence-independent command traces.
6. Complete by decision: keep the command stream local under the accepted
   permissive movement-authority policy. Protocol/server replay requires a
   future explicit product-direction change.
7. Run platform builds and synthetic parity fixtures, then perform separately
   recorded real-device acceptance.

The first five slices are one coherent local end-to-end implementation.
Server-authoritative correction replay is not a remaining milestone. The
local command boundary may be reused if that product decision ever changes,
but no preparatory network or server complexity is required now.

## Code Map

- `native/crates/mclone-input/src/controller_session.rs`: canonical controller
  reduction, semantic edges, held state, repeat, and active-source policy.
- `native/apps/mclone-native-client/src/desktop_gamepad.rs`: macOS
  GameController and Linux/Windows GilRs collection.
- `native/crates/mclone-android-platform/src/android_controller.rs`: Android
  Rust queue and source state.
- `android-common/src/main/java/com/kzahel/mclone/controller/ControllerInputBridge.java`:
  domain-blind Android event/JNI bridge.
- `native/apps/mclone-web-client/src/lib.rs`: browser Gamepad API collection
  and scene routing.
- `native/crates/mclone-xr-host/src/actions.rs`: OpenXR action synchronization
  and state projection.
- `native/crates/mclone-scene`: semantic frame routing, fixed movement clock,
  command materialization, and local replay.
- `native/crates/mclone-client`: predicted player movement consumer.
- `native/crates/mclone-protocol` and `native/crates/mclone-server`: accepted
  pose publication, finite-value/bounds checks, and teleport continuity; no
  semantic command replay.

## Non-Goals

- Polling every backend on a dedicated high-frequency thread by default.
- Pretending browser gamepad snapshots contain events they cannot expose.
- Moving bindings or gameplay action names into JavaScript, Java, Apple
  GameController, GilRs, or OpenXR adapters.
- Sending raw device events over the network or storing them as portable
  gameplay replays.
- Coupling player command rate to render rate, AI/world tick rate, or packet
  publication rate.
- Server-authoritative prediction, movement validation, or correction replay
  without an explicit product-direction change.
