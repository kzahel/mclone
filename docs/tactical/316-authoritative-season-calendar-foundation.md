# Tactical 316: Authoritative Season Calendar Foundation

Status: planned 2026-08-17; implementation authorized

Parent: [`315`](315-authoritative-seasonal-calendar-and-squirrel-ecology.md)
Phase A

Topic: `seasons`

## Instruction Synthesis

Implement Tactical 315 Phase A as one bounded shared calendar slice. Keep
executed `game_time` monotonic and derive a versioned global orbital calendar
from persisted cumulative civil `day_time`. Opt only `mclone-overworld-v1`
into that policy, replicate it to every client, and let Seasonal Debug choose
`World Calendar` independently from its existing unsaved `Manual Preview`.

Bind the first authoritative year to 56 civil days. At 20 ticks per second and
24,000 ticks per day this is about 18 hours 40 minutes without sleep or about
9 hours 20 minutes when each night is skipped. Its fourteen-day quarter season
keeps the current two-day rabbit, four-day mallard, and twelve-day deer loaded
cadences meaningful while making a later mast-surplus-to-lean-cache story
observable across a few ordinary sessions.

This tactical does not implement date mutation, sleep, seasonal resource
effects, squirrels, migration, weather, unloaded catch-up, or automatic
seasonal terrain enablement. Those remain later Tactical 315 children.

## Timing Calibration

The production day is 24,000 ticks, or twenty real minutes at 20 Hz. A player
who sleeps as night begins experiences approximately ten active minutes per
civil day. Candidate product timing is therefore:

| Year days | Unskipped year | Sleep-night year | Unskipped quarter | Sleep-night quarter |
|---:|---:|---:|---:|---:|
| 28 | 9 h 20 m | 4 h 40 m | 2 h 20 m | 1 h 10 m |
| 56 | 18 h 40 m | 9 h 20 m | 4 h 40 m | 2 h 20 m |
| 112 | 37 h 20 m | 18 h 40 m | 9 h 20 m | 4 h 40 m |
| 168 | 56 h | 28 h | 14 h | 7 h |

Twenty-eight days makes large environmental phases change too quickly relative
to travel and ordinary construction. The current Debug-only 112-day projection
makes one quarter take most of a long play day even with sleep and makes the
first cache story unnecessarily slow. One hundred sixty-eight days magnifies
that problem. Fifty-six days keeps a year substantial while giving a fourteen-
day broad reproduction/resource window: seven rabbit, three mallard, and one
deer cooldown intervals before condition, mate, and habitat gates.

This is the first unshipped product rule, not a release compatibility promise.
Persist its explicit rule revision so later tuning cannot silently reinterpret
an existing internal world.

## Binding Contract

Add pure `mclone-season` vocabulary equivalent to:

```text
SeasonCalendarPolicy =
    Disabled
    | Orbital {
          rule_revision: 1,
          days_per_year: 56,
          phase_origin_day: 0,
      }

AuthoritativeCalendarSample {
    civil_time_ticks,
    absolute_day,
    day_tick,
    year_index,
    day_of_year,
    orbital_phase,
}
```

Day zero begins at the northward equinox. `day_of_year` is one-based for UI;
all arithmetic remains zero-based. Fractional progress through the current day
contributes to fixed-point `OrbitalPhase`. Sampling must avoid floating-point
accumulation and remain correct at `u64::MAX` by reducing day/year components
before multiplication.

`WorldGenerationProfile::McloneOverworldV1` selects the orbital policy. Every
other profile selects `Disabled`. Persist the selected policy in world metadata
with a codec revision; old metadata upgrades from its saved profile. Reject an
unknown/noncanonical persisted policy rather than silently changing its year.

Extend the authoritative time update with the policy. Clients store the latest
policy beside `game_time`, `day_time`, and `daylight_cycle_running`, interpolate
only the two clocks under existing rules, and derive calendar samples through
`mclone-season`. Do not replicate local season labels.

Add `SeasonPhaseSource` to the existing client-local season settings:

- `WorldCalendar` uses the replicated policy and current interpolated
  `day_time`;
- `ManualPreview` uses the unsaved Debug orbital slider; and
- the default source is `WorldCalendar`, while the existing solar and ground
  toggles remain off so this slice does not change ordinary rendered pixels.

Seasonal Debug always displays the selected source. When World Calendar is
available it displays year, day/56, global milestone, and evaluated observer-
local season. The manual Date slider is disabled in World Calendar mode.
Profiles with `Disabled` show `Unavailable` rather than inventing a calendar.

## Implementation Boundaries

- `mclone-season` owns pure policy validation, sampling, source vocabulary, and
  fixed-point tests.
- `mclone-server` owns profile selection, persisted policy, and authoritative
  publication.
- `mclone-protocol` carries the neutral shared policy in the time update.
- `mclone-client` stores the policy and exposes the current pure sample.
- `mclone-scene` chooses world versus manual orbital phase for existing solar,
  local-season, celestial, and ground diagnostics.
- `mclone-ui` exposes the source and read-only world date.
- platform apps remain unaware of season policy.

Do not place calendar state in a renderer, add a calendar tick, enumerate
chunks on day/year wrap, or let Manual Preview mutate authority.

## Validation

- pure samples at day zero, all quarter boundaries, last year tick, first next-
  year tick, and `u64::MAX`;
- invalid zero-day, out-of-range origin, unknown revision, and noncanonical
  policy rejection;
- native/Wasm-safe integer-only sampling;
- profile selection proving only Mclone Overworld is enabled;
- metadata v3-to-v4 upgrade, v4 round trip, and unknown-policy rejection;
- protocol round trip with disabled and orbital policies;
- client interpolation across ordinary updates while the policy stays exact;
- client source selection proving World Calendar follows `day_time` and Manual
  Preview does not;
- Seasonal Debug row/summary tests for world, manual, and unavailable modes;
- focused `mclone-season`, `mclone-protocol`, `mclone-client`, `mclone-server`,
  `mclone-ui`, `mclone-scene`, and Wasm checks; and
- exact feature-off rendering controls if scene plumbing changes a draw input.

## Completion Checklist

- [ ] Bind constants and pure calendar policy/sample.
- [ ] Persist and upgrade the exact policy.
- [ ] Replicate policy with authoritative clocks.
- [ ] Store and expose the client sample.
- [ ] Add World Calendar/Manual Preview source selection.
- [ ] Make existing season evaluation consume the selected phase.
- [ ] Pass focused native and Wasm tests.
- [ ] Update Tactical 315 and `docs/topics/seasons.md` with evidence.

## Final Report

Do not fill this section until implementation is complete. Record exact rule,
schema, protocol, test, and visual-restoration evidence plus any remaining live
Human Review of the selected 56-day pace.
