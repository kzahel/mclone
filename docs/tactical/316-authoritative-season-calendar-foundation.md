# Tactical 316: Authoritative Season Calendar Foundation

Status: implemented 2026-08-17; automated native/Wasm and pixel-restoration
evidence complete, live pace review remains open

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

- [x] Bind constants and pure calendar policy/sample.
- [x] Persist and upgrade the exact policy.
- [x] Replicate policy with authoritative clocks.
- [x] Store and expose the client sample.
- [x] Add World Calendar/Manual Preview source selection.
- [x] Make existing season evaluation consume the selected phase.
- [x] Pass focused native and Wasm tests.
- [x] Update Tactical 315 and `docs/topics/seasons.md` with evidence.

## Final Report

Implemented on 2026-08-17. `mclone-season` now owns revision-1 calendar policy,
a 56-day year beginning at the northward equinox, overflow-safe fixed-point
sampling, and explicit World Calendar versus Manual Preview resolution. The
sample carries cumulative civil ticks, absolute day, day tick, year, one-based
day of year, year length, and effective orbital phase. Unknown rule revisions,
zero-length years, and invalid phase origins fail closed.

World metadata codec 4 persists the exact policy. Versions 1 through 3 derive
it once from their saved generation profile and return an upgraded record;
noncanonical profile-policy combinations are incompatible. Only
`mclone-overworld-v1` opts in. The authoritative time update now publishes the
policy beside both clocks, and native, dedicated, Web-worker, and client
runtime paths preserve it exactly. Clients continue to interpolate only
`game_time` and running `day_time`.

The shared scene resolves the existing solar, celestial, local-season, and
exact-ground consumers from current replicated civil time by default. Seasonal
Debug always names its source, shows read-only `Year N, Day N/56` and the
effective milestone in World Calendar mode, reports `Unavailable` for disabled
profiles, and enables the old 112-day date scrubber only in unsaved Manual
Preview. Both visual toggles remain off by default.

Validation completed:

- `cargo test -p mclone-season -p mclone-protocol -p mclone-client -p
  mclone-ui -p mclone-server` passed: 34, 59, 146, 113, and 755 library tests,
  including calendar extrema, v3-to-v4 migration, policy publication, client
  interpolation, source selection, and world/manual/unavailable UI states.
- `cargo test -p mclone-scene --lib` passed all 180 unit tests. The broader
  scene contract run retains a pre-existing unrelated stale assertion that
  expects 101 host fields after successfully matching the 99-field canonical
  list; this slice did not change scene-host fields.
- `cargo check --target wasm32-unknown-unknown -p mclone-web-client` passed.
- `pnpm native:seasons:appearance-capture` passed its 22-case mono/stereo/menu
  matrix and exact preview-off restoration. The inspected contact sheet and
  receipts are under
  `/tmp/mclone-seasonal-appearance-9352701aa346-1786942565999/`.

The 56-day pace is now the implemented internal rule. Its live experiential
Human Review remains open; changing it later requires an explicit metadata
migration or disposable-world regeneration decision, not a silent constant
edit.
