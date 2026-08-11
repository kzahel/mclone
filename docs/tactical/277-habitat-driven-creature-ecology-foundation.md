# Tactical 277: Habitat-Driven Creature Ecology Foundation

Status: **active 2026-08-11**

Topic:

- `habitat-driven-creature-ecology`

## Instruction Synthesis

Record a durable direction in which terrain generation and inhabiting
creatures form a virtuous development and gameplay cycle, choose clear
entity-persistence semantics, and implement the first end-to-end slice
autonomously. Creatures should enter the overworld through meaningful
habitats and mechanics rather than a generic catalogue dump. Complete the
shared implementation, diagnostics, tests, rendered evidence, documentation,
and commits.

## Decision

Normal visible creatures become persistent as soon as the authoritative
server creates them in a persistent world. Revisit does not reroll them merely
because the player made no block edit or explicit interaction. Explicit
ambient summaries or null-store sessions may remain lazy/volatile, but they
must be a named lifecycle rather than a hidden exception.

The first slice makes habitat selection consume the generated chunk's
canonical biome payload. It must not query an unrelated seed-only Java
Overworld biome source when the active profile generated Mclone, flat, island,
Alpha, Beta, or another selectable terrain.

## Objective

Deliver this authoritative path:

```text
entity-ticking + player-distance candidate chunk
  -> persistent entity-chunk load complete when storage supports it
  -> surface candidate from live generated blocks
  -> habitat biome from that generated chunk payload
  -> farm-animal table + light/collision/distance checks
  -> cow/chicken entity
  -> durable chunk record, or explicit volatile record in a transient world
  -> ordinary tracking, ticking, rendering, unload, and reload
```

## Scope

- Add canonical block-position biome lookup to `ChunkSnapshot` and the chunk
  scheduler, including nonzero `min_y` and missing-payload behavior.
- Change live and dry-run creature planning to query habitat at the resolved
  surface position and report missing biome data separately from an
  unsupported biome.
- Filter persistent natural-spawn candidates until entity-chunk load has
  completed, preventing late hydration races.
- Add a persistent passive-creature spawn configuration that requires the
  entity persistence capability but not hostile-mob despawn rules.
- Spawn persistent cow/chicken records when entity chunks are supported and
  retain volatile records only for explicit transient/null-store worlds.
- Rename the runtime natural-spawn option and internal planner types so their
  names do not incorrectly promise volatility.
- Mark naturally spawned persistent entity chunks dirty immediately.
- Extend diagnostics with persistence mode, entity-load exclusions, and
  missing-biome counts.
- Prove transient behavior, durable unload/reload or reopen, active-profile
  biome lookup, and load-order safety.
- Capture and inspect the smallest in-world render path that shows naturally
  spawned animals through the shared actor renderer.
- Update the topic, creature/entity architecture docs, Mclone breadth ledger,
  tactical index, and project entry points.

## Non-goals

- Do not add new `EntityKind` variants or bulk-promote Creature Lab figures.
- Do not implement breeding, hunting, drops, farming, taming, predators,
  migration, seasons, or unloaded population simulation in this slice.
- Do not claim full Java natural-spawn parity: group spawning, all species,
  shared-spawn exclusion, hostile categories, gamerule synchronization, and
  despawn remain later work.
- Do not change terrain generation output, biome IDs, surfaces, vegetation, or
  world compatibility policy.
- Do not persist renderer handles, app-local state, or platform objects.
- Do not change safe player-spawn biome selection, which is a separate
  profile-awareness concern from creature habitat selection.

## Contracts

### Habitat source

The live block surface and biome must describe the same published chunk. A
missing block, light, or biome payload is readiness failure and never falls
back to another generator. A valid biome with no farm-animal table is an
honest habitat rejection.

### Persistence and load order

If `WorldStore::entity_chunks_supported()` is true:

1. the candidate entity chunk must already be in the loaded-entity set;
2. spawned cows/chickens use ordinary persistent identity;
3. the containing entity chunk becomes dirty immediately; and
4. chunk unload/save and reopen hydrate the same persistent identity.

If entity chunks are unsupported, natural spawning remains enabled with an
explicit volatile diagnostic and full-unload discard behavior.

### Shared ownership

Gameplay policy stays in `mclone-server`; canonical biome indexing stays in
`mclone-core`. Desktop, browser, Android, and XR apps consume the same entity
protocol and actor presentation without natural-spawn policy branches.

## Acceptance

- Core tests prove canonical X/Z quart-cell and Y-quart indexing, bounds,
  empty payload, and nonzero `min_y` behavior.
- Scheduler tests prove topology-aware world-position biome lookup.
- Spawn-planner tests prove supported, rejected, and missing habitats are
  counted distinctly after surface resolution.
- Natural-spawn configuration tests prove persistent passive mode requires
  persistence but not despawn.
- Integrated tests prove null-store spawns remain volatile, a persistent store
  reports durable mode and does not spawn before entity load, and natural
  animals survive unload/reload with stable persistent IDs.
- `cargo test --manifest-path native/Cargo.toml -p mclone-core -p
  mclone-server` passes.
- Shared client/server and relevant browser compile gates pass.
- A rendered in-world capture is inspected and linked in the execution record.

## Execution Record

Pending implementation and validation.

