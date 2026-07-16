# World Generation Profiles

Topic: `world-generation-profiles`

Status: **Tactical 187 is active through Slice 4. `flat-grass-v1` and
`small-island-v1` are live, persisted, target-only shared-Rust generators
beside the unchanged Overworld; authored-only misses still produce void.
Shared catalog/UI selection and desktop, browser, Android, XR, dedicated,
multi-dimension, and stored-reopen paths now carry the same profile contract.
Fork-readiness cleanup is next.**

This topic owns the current truth and durable decisions for selectable,
versioned world-generation profiles. Detailed refactoring and implementation
order lives in
[`187-generator-profile-flat-grass-and-seeded-island.md`](../tactical/187-generator-profile-flat-grass-and-seeded-island.md).

## Product Direction

Continue building broad agreement with Minecraft Java 1.17.1 biomes, terrain,
and ordinary decoration as a reference baseline. At an explicit fork, retain
that oracle-tested behavior as a frozen reference profile and develop a
versioned mclone profile with original biome combinations, decoration,
structures, landmarks, and eventually terrain changes.

The first alternate generators are intentionally smaller:

- flat grass proves a seed-independent, target-only procedural generator;
- seeded small island proves original, nontrivial, position-dependent terrain,
  shoreline continuity, biome output, and guaranteed spawn;
- neither is the final mclone overworld.

## Current Truth

The stored server-owned `WorldGenerationProfile` has four values:

- `Overworld`: current procedural vanilla-1.17-shaped generation;
- `FlatGrassV1`: exact bedrock/dirt/dirt/grass layers with plains biomes and no
  decoration, ticks, or generator neighbors;
- `SmallIslandV1`: a bounded original seeded radial/noise field with a safe
  central grass patch, sand shoreline, ocean, and no generator neighbors;
- `AuthoredOnly`: persistence-backed content whose true misses become void.

The profile already crosses world catalogs, realm/dimension metadata,
integrated and dedicated startup, native and browser hosts, and persistence.
It is fixed before chunk scheduling starts.

Product world creation cycles the three procedural profiles through shared
catalog policy and generator-agnostic UI text. Scene replacement, warm-world
startup, managed previews, and all host adapters copy the selected descriptor
before using the shared profile-aware spawn policy. Native SQLite and browser
IndexedDB reopen preserve it; the browser Worker applies stored metadata
profiles before validating or scheduling the world.

Scheduler and worker requests now carry an immutable profile-plus-seed
descriptor through native messages, WASM codecs, responses, and diagnostics.
The closed shared-Rust dispatcher selects the unchanged overworld cache, flat
grass, or seeded island, and resident state resets when either descriptor fact
changes.

The overworld implementation itself remains concrete:

- shared batch timing/report types still retain Overworld-specific names;
- resident worker state contains `OverworldFeatureDependencyCache` for the
  Overworld dispatch case;
- surface, carver, feature-biome, and feature-table internals remain specific
  to `OverworldBiomeSource` and the current Overworld case.

The generic `NoiseBiomeSource` used by terrain sampling is only a partial seam.
Flat grass and seeded island intentionally bypass that machinery. Authored
island/table fixtures remain a separate persistence-backed content path.

There is also no live native true-structure system. The old buried treasure,
desert well, monster room, and fossil history belonged to the retired
TypeScript engine; current structure docs must distinguish that history from
native status.

## Binding Decisions

1. Preserve the existing stored `overworld` identity and serialized
   discriminant. A source rename may happen later, but old saves and external
   labels retain their meanings.
2. New generator algorithm versions are immutable profile identities, starting
   with `flat-grass-v1` and `small-island-v1`.
3. A later original overworld receives its own versioned identity, provisionally
   `mclone-overworld-v1`; it never silently replaces `overworld`.
4. Persistence hits win for every profile. Profiles govern only what a true
   missing chunk produces.
5. Generator dependency footprints are separate from lighting and publication
   dependencies.
6. Shared Rust owns all algorithms. App crates select/display profiles, and
   browser TypeScript transports descriptors without interpreting them.
7. Closed enum dispatch is sufficient. Dynamic generator plugins and Mojang's
   codec/registry framework are deferred until a real requirement exists.
8. Flat and island initially emit existing biome IDs. Original biome registry
   work waits for the actual mclone fork.
9. Generator identity and behavior version are stored per dimension so unseen
   chunks in an old world never change meaning after an upgrade.
10. Remote clients consume authoritative chunks and do not need the server's
    generator implementation.

## First Generator Proofs

### Flat grass v1

- Y `0`: bedrock
- Y `1..=2`: dirt
- Y `3`: grass
- above: air
- plains biome throughout
- no carvers, features, structures, or ticks
- origin spawn on the surface
- requested chunks are the entire generation dependency set

### Seeded small island v1

- original mclone seed domain and world-coordinate height field
- bounded distorted radial island centered at origin
- guaranteed dry, reasonably level central spawn patch
- stone/dirt/grass interior, sand shoreline, ocean floor and water
- existing plains/beach/ocean biome IDs
- no decoration, caves, carvers, structures, or cross-chunk writes
- target-only generation with exact seam and batch-order determinism
- at least two pinned seeds with materially different valid islands

The exact island constants and formulas are frozen in Tactical 187's Slice 3
execution record. Two pinned seeds, X/Z seam checks, partition invariance,
save/reopen, dedicated spawn, and inspected overview/shoreline captures now
establish the v1 compatibility shape.

## Architecture Direction

```text
stored DimensionDefinition
  -> seed + stable generation profile/version
  -> persistence hit or profile-selected miss handling
  -> generator-specific plan and worker session
  -> canonical GeneratedChunk
  -> shared lighting, publication, persistence, and runtime mutation
```

The current overworld cache becomes one dispatch case rather than the worker
protocol itself. Flat and island prove the zero-neighbor case. Future
decoration or structure profiles may request broader dependencies without
changing holder, light, publication, or client contracts.

Spawn policy becomes generator-aware inside shared server/worldgen ownership.
Desktop, web, Android, and XR accept the authoritative spawn rather than
adding profile branches.

## Acceptance Themes

- exact legacy world/profile decode;
- unchanged overworld oracle fixtures and random order;
- deterministic output independent of request order/batching;
- explicit profile/seed reset of worker-resident state;
- native and Web Worker equivalence;
- SQLite and IndexedDB save/reopen equivalence;
- safe spawn for every profile;
- inspected flat and island desktop captures;
- profile identity visible in useful diagnostics;
- no app-local or TypeScript terrain implementation.

## Next Work

Tactical 187 has completed steps 1 through 5; the remaining step is:

1. extract only the biome/rules seams demonstrated necessary for the later
   vanilla/mclone fork.

True native structure infrastructure and original mclone structures remain a
separate follow-up. The generator plan must accommodate their future
dependencies and metadata, but Tactical 187 does not implement them.

## Related

- [`../worldgen-status.md`](../worldgen-status.md)
- [`../reference-minecraft.md`](../reference-minecraft.md)
- [`../structures.md`](../structures.md)
- [`embedded-worlds.md`](embedded-worlds.md)
- [`realm-dimension-runtime.md`](realm-dimension-runtime.md)
- [`../tactical/103-decorated-biome-fixture-matrix.md`](../tactical/103-decorated-biome-fixture-matrix.md)
- [`../tactical/135-overworld-biome-palette-matrix.md`](../tactical/135-overworld-biome-palette-matrix.md)
- [`../tactical/146-overworld-macro-terrain-geometry-parity.md`](../tactical/146-overworld-macro-terrain-geometry-parity.md)
