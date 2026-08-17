# Tactical 317: Civil-Time Discontinuity And Sleep

Status: implemented 2026-08-17; automated Review Gate B accepted

Parent: [`315`](315-authoritative-seasonal-calendar-and-squirrel-ecology.md)
Phase B

Topic: `seasons`

## Instruction Synthesis

Implement Tactical 315 Phase B as one bounded authoritative-server slice.
Civil `day_time` may jump, but executed `game_time` / `simulation_tick` never
does: a date command or successful night sleep changes current conditions and
publishes the new clock without replaying skipped crops, scheduled ticks,
resources, animals, cooldowns, or lifecycle work.

Add an original Mclone sleeping mat as the smallest ordinary placeable sleep
site. It is a durable shared-world entity placed through the normal held-item
interaction, rendered through the existing figure path, and used through the
existing entity interaction target. It is not showcase-only furniture and
does not introduce Java bed parity, respawn assignment, recipes, weather,
hostile checks, comfort, or multi-block furniture rules.

Sleep is ephemeral realm-player state. At night, every connected living
ordinary player sharing the realm calendar must be sleeping at a still-valid
site under the initial 100-percent rule. The server resolves that quorum once
at an authoritative tick boundary, jumps to exact morning, clears all sleep
states, publishes wake and time state, and executes exactly one ordinary
simulation tick.

This tactical does not implement seasonal resource response, squirrels,
caches, migration, weather, unloaded advancement, or a generalized furniture
framework.

## Bound Values

- One civil day remains `24_000` ticks.
- The accepted sleep window is day tick `12_500..=23_999`.
- Morning is exact day tick `0` of the following absolute day.
- `SleepRule { required_percent: 100 }` is the initial typed realm rule;
  accepted values are `1..=100`, and required sleepers use ceiling division.
- Interaction reach uses the existing authoritative entity-interaction reach
  rule rather than a sleep-only distance.
- Sleeping ends on any accepted player displacement, damage, death or
  respawn, site removal/unload, dimension change, disconnect, a second
  interaction with the occupied site, or explicit cancel.
- Sleep state is never serialized in player or world records.

## Typed Civil-Time Operations

Add server-owned operations with a shared error type:

```text
set_time_of_day_preserving_date(day_tick)
set_calendar_date(year_index, day_of_year, optional_day_tick)
advance_to_next_morning()
```

The first accepts `0..23_999` and replaces only the current day remainder.
The second requires an enabled valid calendar policy, uses one-based
`day_of_year`, defaults the omitted day tick to the current time of day, and
rejects policy-range errors or checked-arithmetic overflow. The third computes
the next absolute-day boundary with checked arithmetic and never wraps.

Every successful ordinary operation updates durable metadata, marks it dirty,
and enqueues exactly one authoritative `TimeUpdate` per connected participant.
The existing startup/debug whole-value setter remains explicitly non-durable
and does not masquerade as an ordinary operation. Failed operations mutate
nothing and return a stable diagnostic reason.

## Sleeping Mat Content Contract

Add `ItemKind::SleepingMat` and `EntityKind::SleepingMat` through the existing
exhaustive protocol, inventory, persistence, client actor, figure, and item
presentation tables. One mat is supplied by the ordinary initial inventory so
the mechanic is reachable before recipes exist. Using the selected mat on a
loaded solid floor places one horizontal persistent sleeping-mat entity in the
adjacent free space, consumes one item, and publishes the ordinary entity and
inventory updates.

The mat has a compact floor-level collision footprint, no AI, health,
despawn, gravity, drops, or animation state. Its persistent record contains
the ordinary stable entity identity, dimension, position, and orientation.
Removing the mat through the existing entity-removal/attack interaction (or
world invalidation) invalidates sleepers; item recovery and crafting remain
out of scope. Placement and interaction must be shared behavior consumed by
native, Web, flat Android, and XR hosts without platform branches.

## Sleep State And Quorum

Add shared protocol commands equivalent to:

```text
InteractEntity(sleeping_mat)  -> request or toggle sleep
CancelSleep                   -> explicit cancellation
SleepStateUpdate {
    sleeping,
    sleeping_players,
    eligible_players,
}
```

The server records, per sleeping player, the site persistent ID, dimension,
site position, player position, and admission tick. Admission requires a
connected ordinary player who is alive, not awaiting respawn or teleport, in
the accepted night window, and able to reach a loaded mat in the same
dimension. Observers and preview cameras neither enter sleep nor count.

At each authoritative tick boundary, first invalidate stale sleep states,
then count all connected eligible realm players across loaded dimensions that
share the one calendar. If the typed quorum is met, call the same checked
next-morning operation used by direct server control, clear every state, and
publish wake/quorum state plus one time update. The ordinary daylight
increment for that tick is suppressed so the emitted and retained clock stays
exactly at day tick `0`; `simulation_tick` advances once and every ordinary
simulation subsystem executes once.

If no quorum is met, the tick follows the existing daylight rule. Disabling
the daylight cycle does not block an explicitly requested sleep jump: sleep
is a discontinuous player operation, while subsequent automatic civil time
remains paused.

## Presentation Boundary

The client stores the latest owner-specific sleep/quorum state. Shared UI
shows a small status notice while sleeping or waiting for other eligible
players and removes it on wake/cancel. The ordinary world interaction is the
only production entry path; Debug UI and the seasonal manual preview cannot
request sleep or mutate the calendar.

The sleeping mat uses one original figure asset with a deliberately low,
readable silhouette and existing single-view, stereo, and multiview actor
render paths. No new renderer or platform-owned sleep behavior is permitted.

## Validation

- time-of-day change preserves the absolute day and rejects tick `24_000`;
- date changes cover first/last day, year wrap, current-time preservation,
  disabled policy, invalid one-based dates, and arithmetic overflow;
- successful operations dirty/save metadata and emit one time update, while
  failures emit none;
- forward and backward jumps leave simulation tick, scheduled deadlines,
  crop age, wildlife age/condition/pregnancy, resource state, and world edits
  unchanged;
- one-player night success, daytime rejection, exact morning, one executed
  tick, and paused-daylight success;
- two-player 100-percent waiting/completion/wake, local and remote participant
  equivalence, and observer exclusion;
- movement, damage, death, respawn, disconnect, explicit cancel, site removal
  or unload, and dimension change cancellation;
- sleeping-mat item placement, collision, persistence/reopen, interaction,
  protocol round trip, actor/figure resolution, and cross-platform-safe asset
  loading;
- save/reopen after a successful jump restores exact civil time and no sleep
  state;
- native and browser ordinary-interaction captures visibly show the mat,
  waiting/sleep feedback, and exact post-wake World Calendar time; and
- Wasm compilation plus existing mono/stereo/multiview feature-off controls.

## Completion Checklist

Implementation landed in commits `5fa34fb1`, `92712f1d`, and `8fe84f08`.
The server now owns checked durable time-of-day, calendar-date, and
next-morning operations. The sleeping mat is ordinary item/entity content,
persists through the shared entity record, and enters sleep through normal
world interaction. Sleep uses ephemeral per-player state, the typed
100-percent realm rule, exact next-morning civil assignment, one ordinary
simulation tick, and no elapsed-time replay.

Automated evidence:

- the complete `mclone-server` suite passed with 763 tests, including direct
  date/time persistence, forward/backward no-catch-up sentinels, one- and
  two-player quorum, observer exclusion, cancellation causes, exact dawn,
  paused daylight, save/reopen, and sleeping-mat persistence;
- `cargo test -p mclone-native-client --bin mclone-native-client
  --no-fail-fast` passed 182 tests;
- `cargo check --workspace`, `pnpm native:web:typecheck`, and
  `pnpm native:thin-adapters:purity` passed;
- `pnpm native:web:sleep-smoke` placed and targeted entity `3` through
  ordinary browser input, observed the stable waiting state at civil tick
  `12_500` with quorum `1/1`, and then observed exact civil tick `24_000`
  with sleep cleared and the mat retained;
- the browser waiting and morning captures are
  `/tmp/mclone-native-web-sleep-waiting-canvas.png` and
  `/tmp/mclone-native-web-sleep-morning-canvas.png`; both were inspected and
  show the ordinary mat scene, with the waiting overlay present only before
  dawn;
- the native offscreen command lane placed and used the mat through ordinary
  shared input, while `/tmp/mclone-native-sleep-waiting.png` was inspected as
  the native transient-UI pixel receipt; the offscreen runner stages the
  already protocol-tested waiting update because its single final frame does
  not retain the short authority interval; and
- the first-party sleeping-mat figure was inspected independently in the
  prepared figure-review captures under
  `/tmp/mclone-figure-review/sleeping-mat/`.

The continuous browser run is the full ordinary waiting-to-dawn behavioral
receipt. Native supplies an ordinary command-path receipt plus deterministic
presentation evidence; exact one-tick and no-catch-up semantics remain owned
by the shared server tests rather than inferred from pixels.

- [x] Add and test checked typed civil-time operations.
- [x] Add ordinary placeable and persistent sleeping-mat content.
- [x] Add sleep commands, state, cancellation, rule, quorum, and wake updates.
- [x] Add client storage and shared UI feedback.
- [x] Prove exactly one executed tick and no skipped-work replay.
- [x] Pass focused native, Wasm, persistence, multiplayer, and pixel gates.
- [x] Update Tactical 315 and the seasons topic with final evidence.
