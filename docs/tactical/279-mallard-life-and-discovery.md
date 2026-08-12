# Tactical 279: Mallard Life and Discovery

Status: **active 2026-08-12**

Topic:

- `habitat-driven-creature-ecology`

## Instruction Synthesis

Continue the implemented mallard ecology direction and complete all recommended
follow-ups: nests with incubation and hatching, breeding, shallow-water
swimming, flock cohesion, audible calls, feathers and tracks, and field-guide
discovery. Keep the work shared, persistent, mechanically meaningful, and
validated end to end, committing logical slices as they land.

## Decision

Turn mallards from habitat-qualified ambient animals into a compact life and
discovery chapter. A mallard egg placed through an explicit nest interaction
becomes a durable incubating nest. A valid, attended wetland nest hatches a
durable duckling; two adult mallards near the nest satisfy the first breeding
contract without adding a generic feed-to-clone system. Removing habitat or
attendance pauses incubation rather than silently destroying progress.

Mallards may enter shallow water, float at its surface, paddle toward nearby
flockmates, and return to shore. Their ordinary server state exposes the
presentation facts needed for a distinct swim pose. Calls are bounded audible
contact signals: nearby mallards occasionally quack, with flock-level cooldown
and spatial attenuation, so sound helps a player locate wetland life without
becoming a continuous chorus.

Feathers and tracks are evidence, not more immortal decorative entities.
Feathers are collectible item traces shed at bounded intervals in suitable
habitat. Tracks are short-lived presentation traces derived from recent
authoritative shore movement and capped locally. Seeing a mallard, hearing a
call, finding a feather, finding a track, finding a nest, and witnessing a
hatch unlock explicit durable observations in one mallard field-guide entry.

## Objective

Deliver this shared path:

```text
adult wetland flock
  -> shallow-water paddling + flock cohesion + bounded contact calls
  -> collectible feathers + ephemeral shore tracks
  -> place mallard egg at covered wetland shore
  -> durable nest with habitat and pair attendance
  -> paused/resumed incubation
  -> durable duckling hatch and growth
  -> authoritative observation events
  -> durable mallard field-guide progress and cross-platform HUD presentation
```

## Scope

- Add a persistent nest entity and a distinct nest-with-egg item interaction
  rather than reserving an app-local block ID.
- Consume one carried mallard egg when a valid covered shore interaction
  creates a nest; reject dry, exposed, obstructed, distant, or occupied sites.
- Tick incubation only while the local wetland remains valid and at least two
  adult mallards attend within a bounded radius. Persist accumulated progress.
- Hatch one durable duckling with parent identities, then preserve age and
  growth through unload/reload.
- Add mallard shallow-water detection, float/paddle travel, water destinations,
  and separation/cohesion steering without weakening land collision rules for
  other mobs.
- Emit rate-limited authoritative mallard call cues and spatialize them in the
  shared scene/audio path with a distributable first-party call family.
- Add a collectible `MallardFeather` item shed under a species cooldown and
  ordinary item persistence/pickup rules.
- Add bounded client presentation tracks from authoritative mallard shore
  movement; tracks expire and never impersonate durable world mutations.
- Add protocol observation kinds plus authoritative per-player observation
  state, persistence, client replication, field-guide model, and compact HUD
  presentation.
- Preserve ordinary mono, stereo, multiview, browser, Android, and XR ownership
  boundaries.

## Non-goals

- Do not build a general encyclopedia, quest journal, animal genetics system,
  seasons, migration, disease, predation, or unloaded population simulation.
- Do not add sexed figure variants; the first pair contract treats two nearby
  adult mallards as eligible attendants.
- Do not make calls a random ambience track detached from real entities.
- Do not persist individual track marks or replay calls while chunks are
  unloaded.
- Do not change Java 1.17.1 Overworld worldgen, spawning, or animal parity.
- Do not add gameplay policy to desktop, web, Android, or XR app crates.

## Contracts

### Nest and breeding

One nest owns its position, incubation progress, contained egg count, optional
parent persistent IDs, and hatch state. It is immediately durable. Progress
requires canonical loaded blocks, covered wetland habitat, and two living adult
mallards in range. Failed conditions pause. One egg produces at most one
duckling; hydration must never replay the hatch.

### Swimming and flocking

Only mallards receive water traversal. Shallow-water occupancy applies bounded
buoyancy and horizontal drag, and paddle intent targets traversable water or
shore. Each mallard may use the nearest visible adult flockmate as a cohesion
anchor while preserving minimum separation. Missing peers or edited water
falls back to independent shore behavior.

### Calls and traces

A call is an authoritative, lossy presentation cue with entity identity,
position, kind, sequence, and finite audible radius. Cooldowns are deterministic
server state and flock-level admission prevents overlapping spam. A feather is
a normal durable item. A track is an explicitly ephemeral client trace with a
bounded lifetime and local cap.

### Field guide

Observation state is authoritative per-player realm data and is durable in
saved worlds. Repeating an observation is idempotent. The mallard entry reports
six discoveries—seen, heard, feather, track, nest, hatch—and derives completion
from their bitset. UI reads the replicated model; it does not infer discoveries
from rendered pixels or audio playback success.

## Acceptance

- Protocol/persistence tests round-trip nests, duckling age/parents, feathers,
  call cues, and observation state.
- Nest tests cover placement validation, item consumption, attendance, pause,
  resume, single hatch, unload/reload, and habitat edits.
- Movement tests cover buoyancy, shallow-water paddling, cohesion, separation,
  shore fallback, and unchanged cow/chicken movement.
- Call tests prove real-entity origin, range, deterministic cooldown, flock
  suppression, and first-party bank preparation.
- Trace tests prove bounded shedding, pickup, track admission/expiry, and no
  durable track records.
- Field-guide tests prove every unlock source, idempotence, persistence,
  replication, and flat/stereo HUD projection.
- Shared native all-target, browser/Wasm, and affected crate test gates pass.
- Inspected pixels show a nest/duckling or paddling flock plus field-guide
  feedback in a generated Mclone wetland.

## Execution Record

Pending implementation and evidence.
