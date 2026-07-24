# Client Entry Lifecycle

Topic: `client-entry-lifecycle`

Status: active. The product decision, shared entry/lifecycle policy, bounded
accounting correction, desktop/SteamOS menu-first entry, and Steam Deck Devkit
process ownership are implemented. Android, web, XR, and idle-cadence
migration remain active.

## Scope

This topic owns the product-level answer to:

- which experience appears when a client becomes usable;
- how explicit launch requests select a world, remote session, or scenario;
- how foreground, background, presentation loss, and presentation restoration
  affect that experience;
- what work is permitted while the title menu is idle; and
- which cross-platform sanity checks every interactive launch must satisfy.

It does not own how a local or remote session is constructed after one has
been requested. That execution contract lives in Tacticals
[`167`](../tactical/167-shared-session-startup-contract.md) and
[`173`](../tactical/173-shared-startup-configuration.md). It also does not own
the internal implementation of bounded frame telemetry; that remains in
[`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md) and
[`performance.md`](performance.md).

## Accepted Product Decision

Every ordinary interactive client starts at the title menu. No platform starts
a world, opens a save, joins a server, or launches a scenario merely because of
its platform identity.

Entering a session at launch requires an explicit host-neutral request supplied
through a supported source such as:

- a desktop CLI option;
- a Steam or managed-desktop launch option;
- an Android activity intent or property;
- a browser URL/deep link;
- an XR launch intent; or
- an automation, smoke, benchmark, capture, or authored-scenario harness.

The safe fallback for an absent, invalid, stale, or unavailable destination is
the title menu with a visible status. The client must not silently create a
replacement world.

Resume-last-session is not implicit in this decision. It may be added later as
an explicit product policy after its persistence, failure, and user-control
contract is defined.

## Separate The Axes

Launch behavior is currently difficult to reason about because several
orthogonal concerns are expressed through app-local construction choices.
The target keeps them separate:

| Axis | Shared meaning | Examples |
|---|---|---|
| Entry intent | desired initial product destination | title, session request, authored scenario |
| Startup configuration | neutral engine/session configuration | render distance, seed, camera, storage intent |
| Presentation profile | capabilities and presentation policy | flat, XR, touch, controller, fullscreen |
| Host lifecycle | availability facts that may repeat | foreground, background, surface gained/lost |
| Session lifecycle | product state and requested transitions | no session, starting, active, failed, teardown |
| Activity demand | useful work class | dormant, static UI, animated UI, active session, suspended |

SteamOS is a managed desktop environment, not a separate gameplay client.
Gamescope, Steam launch options, fullscreen policy, and Devkit process control
remain desktop adapter or deployment facts. They do not select the initial
product destination.

Android activity creation is not equivalent to a new product launch. An
activity or surface may be recreated while the process and product state
survive. Browser canvas/visibility and OpenXR session/swapchain availability
have the same general property: presentation lifecycle events can repeat
without authorizing a new world.

## Target Shared Contract

The existing `mclone-app-runtime` client-experience and session types are the
preferred owner. Do not create a platform-shaped launch crate or duplicate
`SessionStartRequest`.

A minimal entry vocabulary is:

```rust
enum ClientEntryIntent {
    Title,
    StartSession(SessionStartRequest),
    LaunchScenario(ScenarioLaunchIntent),
}
```

The exact public names may change during implementation, but the following
separations are required:

- source syntax and provenance are not product policy;
- presentation/profile configuration is not an entry destination;
- a title screen does not imply that a hidden world should exist;
- session construction starts only after shared policy emits or accepts a
  `SessionStartRequest`; and
- a host lifecycle event never synthesizes a session start by itself.

Shared launch resolution uses this precedence:

1. a valid explicit entry request;
2. a future explicitly enabled and valid resume policy;
3. the ordinary interactive product default, `Title`; and
4. `Title` with a visible error/status fallback.

Automation profiles must provide their destination explicitly. A smoke test
that needs a transient local world and an XR product that wants an immersive
lobby are both valid, but neither is a platform default.

## State And Effect Shape

The shared controller should be a deterministic sans-I/O policy component
composed with `ClientExperienceController`. A useful conceptual state set is:

```text
AwaitingHost
  -> Title
  -> StartingSession
  -> ActiveSession
  -> Title

Any presentation state
  -> Suspended / PresentationUnavailable
  -> previous product state when the host is usable again
```

Adapters report facts such as:

- host bootstrap completed or failed;
- presentation became available or unavailable;
- app entered foreground or background;
- an explicit entry request arrived;
- a requested platform operation completed or failed; and
- exit was requested.

Shared policy emits typed effects. Adapters execute storage, transport,
surface, worker, Steam, browser, activity, or OpenXR mechanics and return
typed completions. This extends the existing effect/completion direction in
[`../client-experience-architecture.md`](../client-experience-architecture.md)
and [`platform-host-boundary.md`](platform-host-boundary.md).

`mclone-scene` remains the executor and owner of a live scene/session. App
crates collect platform facts, supply resources, drive cadence, and present
frames.

## Current Divergence

As of 2026-07-24:

| Lane | Current ordinary entry | Current decision point |
|---|---|---|
| Desktop flat | title menu; explicit `--start-in-world true`, remote address, world directory, or automation frame report starts a session | shared `ClientEntryResolution` projected from desktop CLI syntax |
| Steam Deck interactive Devkit payload | title menu | the managed payload and ordinary desktop default resolve through the same shared entry policy; `steamos` remains only a presentation profile |
| Flat Android | starts a local session and constructs in-game UI | Android surface driver |
| Web | constructs local/remote runtime and then clears the UI screen | browser scene-host startup |
| Desktop/Android XR | session construction and visible UI selection are not one consistent entry policy | XR app/scene assembly |
| Offscreen/smoke/perf | intentionally starts the requested workload | harness-specific options |

This matrix is evidence of missing shared entry policy, not justification for
platform defaults.

## Idle And Frame-Demand Contract

The shared product state should expose useful activity demand without trying to
replace the host event loop. The likely vocabulary is:

- `Dormant`;
- `StaticUi`;
- `AnimatedUi` with a bounded update requirement;
- `ActiveSession`; and
- `Suspended`.

Adapters map that meaning to their real mechanics:

- desktop/winit and SteamOS may wait for events while static;
- browser may stop or reduce `requestAnimationFrame` work when no animation is
  useful or the page is hidden;
- Android renders only while foreground presentation is available; and
- OpenXR retains runtime-required frame sequencing while shared scene policy
  skips unnecessary world work.

One universal event loop is not a goal. One shared statement of useful work is.

## Baseline Sanity Contract

Every interactive client must prove:

1. an ordinary launch with no explicit entry request reaches the title menu;
2. the title menu has no active world, integrated server, remote session, or
   world workers unless a named prewarm/scenario contract explicitly requests
   them;
3. an explicit local, saved-world, remote, or scenario request reaches the
   same shared session vocabulary on every supporting host;
4. an invalid request returns to title with a visible reason;
5. presentation loss and backgrounding do not create or duplicate sessions;
6. static-title CPU and memory remain bounded during a long soak;
7. diagnostics overhead remains bounded independently of process uptime;
8. managed development launchers own at most one interactive process;
9. logs identify the normalized entry intent, provenance, resolution, and
   fallback reason; and
10. quit-to-title tears down the active session and returns to a genuinely
    session-free title state.

Items 6 and 7 now use a bounded rolling live collector with O(1) steady-state
recording, incremental lifetime counters, bounded top-K worst frames, 512
recent percentile observations, and decimated rich-report publication. Finite
exact benchmark capture remains a separate explicitly bounded mode.

The desktop/SteamOS host now constructs a session-free scene shell for title
entry. It prepares retained UI/render/audio assets and catalog services, but
does not construct a client runtime, integrated server, remote connection, or
world worker. Explicit session entry starts afterward through the shared
`SessionStartRequest` path. Source-contract tests lock this distinction.

## Steam Deck Devkit Process Ownership

The Devkit lane is deployment tooling, not shared launch policy, but it must
obey the one-process sanity contract.

As of this topic's first slice:

- the staged interactive payload holds an OS file lock for its lifetime;
- its owner record contains a PID plus Linux process-start identity, avoiding
  PID-reuse and name-wide-kill hazards;
- direct duplicate payload starts are rejected;
- `steamdeck:launch` and interactive deploy stop the exact recorded owner
  before issuing Valve's `run-game` RPC;
- stop first sends `TERM`, waits five seconds, and uses `KILL` only if the
  owned development process does not exit; and
- `steamdeck:stop` provides an explicit manual cleanup command.

The physical SteamRT4 lane validated launch, exact-owner replacement, direct
duplicate rejection, and final cleanup on 2026-07-24. Process count remained
one across replacement and zero after explicit stop.

This replacement behavior is for iterative Devkit runs. It is not evidence for
normal Steam single-instance behavior and must not be used as the save-on-quit
acceptance path.

## Recommended Implementation Order

1. Correct bounded live frame accounting first, because the defect worsens
   with uptime and contaminates subsequent performance evidence.
2. Add a shared entry-intent/controller contract in `mclone-app-runtime`,
   reusing `SessionStartRequest` and `ScenarioLaunchIntent`.
3. Make desktop flat and SteamOS project CLI/managed-launch sources into that
   contract; change the ordinary desktop default to title.
4. Migrate flat Android, web, desktop XR, and Android XR away from hard-coded
   UI/session construction decisions.
5. Add shared activity-demand output and map it onto each existing platform
   cadence mechanism.
6. Run cross-platform conformance, lifecycle, rendered-menu, long-idle, and
   physical-device acceptance.

Each implementation tactical should take a bounded slice from this sequence.
Do not combine all platform migration, frame-accounting internals, and cadence
changes into one review unit.

## Validation And Human Gates

Most implementation and automated validation can be driven without human
input:

- shared controller/state/effect implementation and deterministic tests;
- desktop, offscreen, web, Android, and XR compile/test gates;
- desktop, headed-browser, AVD, synthetic-XR, Quest, and Steam Deck automated
  launch/capture/performance lanes;
- long-running process-count, CPU, and memory soaks; and
- machine-readable launch-resolution and lifecycle assertions.

Human validation is required before declaring the concern complete for:

- whether title, loading, and fallback transitions feel intentional rather
  than flashing or stalling;
- built-in Steam Deck controller navigation and clean visual presentation in
  Gaming Mode;
- physical suspend/resume and dock/undock behavior;
- Android task switching, activity recreation, and OS back behavior on a real
  touch device;
- browser back/forward/deep-link expectations as a product decision;
- physical OpenXR runtime interruption/resume and headset comfort; and
- any future resume-last-world UX, privacy, or destructive recovery policy.
