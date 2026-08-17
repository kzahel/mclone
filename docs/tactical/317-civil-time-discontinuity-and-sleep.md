# Tactical 317: Civil-Time Discontinuity And Sleep

Status: planned 2026-08-17; ready for implementation

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

- [ ] Add and test checked typed civil-time operations.
- [ ] Add ordinary placeable and persistent sleeping-mat content.
- [ ] Add sleep commands, state, cancellation, rule, quorum, and wake updates.
- [ ] Add client storage and shared UI feedback.
- [ ] Prove exactly one executed tick and no skipped-work replay.
- [ ] Pass focused native, Wasm, persistence, multiplayer, and pixel gates.
- [ ] Update Tactical 315 and the seasons topic with final evidence.
