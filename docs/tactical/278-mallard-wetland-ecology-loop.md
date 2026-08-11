# Tactical 278: Mallard Wetland Ecology Loop

Status: **complete 2026-08-11**

Topic:

- `habitat-driven-creature-ecology`

## Instruction Synthesis

Continue the creature/worldgen/gameplay workstream with a tactical and
autonomous end-to-end implementation, committing logical slices as they land.
Make the first distinctive Creature Lab promotion demonstrate the virtuous
cycle: terrain should create a meaningful habitat, a creature should visibly
read and inhabit it, and the creature should add a collectible mechanic rather
than merely expanding a generic spawn catalogue.

## Decision

Promote the Creature Lab mallard as an original Mclone Overworld species. Its
first habitat is a generated grassy shore beside bounded shallow water, not a
biome-name shortcut. Mclone's existing wetland pools and river margins provide
the physical base; sparse profile-owned lily pads and reeds make selected
wetlands more legible and add cover evidence without changing Java 1.17.1
Overworld output.

Mallards spawn as small local flocks, seek nearby shore habitat while
waddling, and lay collectible mallard eggs only while standing in qualifying
wetland habitat. Both birds and dropped eggs use the ordinary persistent entity
path immediately in saved worlds. The existing visible-entity persistence
decision from Tactical 277 remains unchanged.

## Objective

Deliver this shared path:

```text
Mclone wetland water + grassy bank + sparse cover
  -> live block-derived WetlandHabitatSample
  -> bounded 2-4 mallard flock request
  -> persistent authoritative mallards
  -> shore-biased waddle behavior
  -> habitat-gated mallard egg
  -> persistent item entity and ordinary player pickup
  -> shared mono/stereo/multiview/browser actor presentation
```

## Scope

- Add deterministic sparse lily-pad and sugar-cane cover around actual Mclone
  inland watercourse sites in the Mclone feature pass. Keep the feature
  topology-aware and bounded.
- Add a shared server wetland habitat sample over published blocks. Record
  nearby water, shallow-water support, qualifying land, cover, and suitability
  from a fixed bounded footprint.
- Keep Java 1.17.1 farm-animal tables unchanged. Admit mallards only when the
  active generation profile is `mclone-overworld-v1` and the local block
  habitat qualifies.
- Plan one flock as 2-4 nearby validated members while respecting the existing
  per-tick creature budget, player distance, collision, light, readiness, and
  category cap.
- Promote `EntityKind::Mallard`, a first-party `mallard_duck` figure, metadata,
  runtime species state, protocol codec, persistence payload, diagnostics, and
  renderer mappings through shared owners.
- Give mallards a bounded shore-biased stroll goal that prefers valid land
  destinations near water and falls back safely when the habitat changes.
- Persist the mallard egg timer, emit a distinct `MallardEgg` item only at a
  qualifying shore, and reuse ordinary item persistence, merging, pickup,
  inventory, and egg-shaped rendering.
- Prove a naturally spawned flock and its egg survive entity-chunk unload and
  reload with stable persistent IDs.
- Capture and inspect in-world pixels showing the promoted figure in its
  generated wetland habitat.
- Update the living habitat, animal catalogue, creature/entity architecture,
  Mclone breadth, asset, and tactical documentation.

## Non-goals

- Do not change Java 1.17.1 Overworld terrain, biome tables, or spawn parity.
- Do not promote the rest of Creature Lab or make mallards generic to every
  profile containing water.
- Do not implement full swimming/buoyancy, flight, migration, unloaded
  population simulation, predator/prey simulation, sounds, combat, or drops.
- Do not implement persistent nest blocks, incubation, hatching, lineage, or
  breeding in this slice. The distinct egg kind preserves a clean next step.
- Do not require semantic Mclone sampler fields in the live gameplay query;
  actual published blocks remain authoritative after edits.
- Do not put habitat, entity, item, or rendering policy in desktop, web,
  Android, or XR app adapters.

## Contracts

### Wetland habitat

The first `WetlandHabitatSample` is intentionally local and inspectable. A
spawn or lay site must be ordinary traversable land with daylight and headroom,
must have water within the fixed sample radius, and must find at least one
shallow-water column whose bed is within two blocks of its water surface.
Cover is reported separately so it can tune rarity and later nest mechanics;
the first flock must not require a decoration roll to make otherwise valid
wetlands usable.

The query consumes loaded canonical block state. A missing sample block is a
readiness failure, never a fallback to seed-only terrain intent.

### Flocks and locality

One accepted wetland anchor may produce 2-4 mallards. Every member independently
passes surface, habitat, distance, brightness, and collision checks. The flock
is clipped by the remaining creature cap and per-tick request budget. Failure
to fit all members may produce a smaller flock, but never an invalid member.

### Resource and persistence

Mallard egg timers are authoritative species state and survive save/hydrate.
When a timer expires away from wetland shore, it remains due rather than
creating an egg in arbitrary terrain. On a qualifying shore it emits one
`MallardEgg` item, resets its timer, and uses the same durable item lifecycle
and pickup rules as existing eggs.

### Shared presentation

Mallard presentation is selected from `EntityKind` in the shared render
session and uses the existing figure/actor pipelines for ordinary per-view and
XR multiview rendering. Apps may expose diagnostic labels but own no species
semantics.

## Acceptance

- Worldgen tests prove wetland cover is deterministic, bounded to real inland
  watercourse habitat, and absent from unaffected profiles.
- Habitat tests prove shallow shore acceptance plus deep/open water, dry land,
  blocked, and missing-data rejection.
- Planner tests prove Mclone-only flock admission, 2-4 grouping, independent
  member validation, and hard budget clipping while preserving farm-animal
  behavior elsewhere.
- Runtime tests prove shore-biased destination selection and safe fallback.
- Protocol, persistence, item-stack, inventory, actor-pose, and asset inventory
  tests cover the new entity, figure, and item kinds.
- Integrated tests prove natural flock durability and habitat-gated egg
  durability/pickup.
- `cargo test --manifest-path native/Cargo.toml -p mclone-worldgen -p
  mclone-protocol -p mclone-assets -p mclone-server -p mclone-render-session
  --no-fail-fast` passes.
- Shared native/WebAssembly compile gates pass.
- An in-world Mclone capture with authored passive showcases disabled is
  inspected and linked in the completed execution record.

## Execution Record

Completed in six logical commits:

- `40c55a46` promoted the approved Asset Lab mallard into the first-party
  runtime figure registry and both ordinary and multiview renderer resources.
- `8a7d3594` added deterministic Mclone-only lily-pad and sugar-cane wetland
  cover over verified inland water and supported banks.
- `d07c8412` added the durable mallard species, shared live-block habitat
  sample, shore-biased stroll, habitat-gated timer, distinct egg item,
  protocol/persistence contracts, and shared presentation.
- `adbb3b8e` connected Mclone-only 2-4-member flock planning, diagnostics,
  ordinary creature caps, persistent realization, due-egg pickup, and
  generated-world unload/reload evidence.
- `75030bbd` replaced the pre-existing ordered western-edge chunk selection
  with bounded sampling without replacement so habitats throughout the
  ticking area can participate.
- `f635ed61` made generic Mclone farm-animal requests fallback candidates until
  the fixed habitat scan finishes, preventing common animals from consuming
  the request budget before distinctive habitat content is considered.

The Java Overworld lane still uses its original farm-animal biome tables and
placement path. Mallards are admitted only for `mclone-overworld-v1` after a
published-block sample proves grass underfoot, nearby water, a shallow bed,
light, headroom, collision clearance, and player distance. Every flock member
repeats those checks. Missing sample data fails closed.

The server integration fixture loads seed `12345` chunk `(-142, -51)`, plans a
flock from actual generated wetland blocks, lays a due mallard egg, unloads the
entity chunks, and hydrates every bird and the egg under the same persistent
IDs and fresh runtime IDs. A separate inventory fixture picks up the distinct
egg through the ordinary item path.

Validation passed on 2026-08-11:

- the aggregate acceptance command passed `mclone-worldgen` (411 passed, one
  ignored), `mclone-protocol` (52 passed), `mclone-assets` (75 unit tests plus
  its runtime-boundary integration test), `mclone-server` (607 passed), and
  `mclone-render-session` (126 passed), with all doc tests passing;
- the full native workspace all-targets compile gate passed; and
- `pnpm native:web:build` compiled the shared path and web client for
  `wasm32-unknown-unknown`.

Rendered acceptance used a fresh SQLite Mclone world, ordinary live spawn
cadence, and `--debug-passive-showcase false`. Its durable records contained
seven naturally spawned mallards. Reopening the four-bird habitat chunk
reported four authoritative entities, four actors, and four drawn actors. The
inspected [accepted capture](</tmp/mclone-mallard-natural-accepted.png>) shows
the flock on grassy banks between shallow generated pools.
