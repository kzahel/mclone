# World Height And Volumetric Streaming

Topic: `world-height-and-volumetric-streaming`

Status: **research and near-term contract accepted 2026-07-18;
implementation has not started. Live generated dimensions still use
`min_y = 0`, `height = 256`. The immediate direction is to make a finite
vertical range an authoritative per-dimension fact for Mclone's original
worlds. The legacy Java-shaped profile is not a reference lock on that work.
Partial vertical residency and cubic worlds remain later experimental work.**

This topic owns current truth, decisions, practical limits, and next work for
dimension height and three-dimensional world residency. It does not own
horizontal bounds or wrapping, generator-specific terrain design, general
realm topology, or far-terrain presentation.

## Short Answer

- Mclone's current column is 16 blocks wide, **256 blocks high**, and 16 blocks
  deep: Y `0..=255`, or 65,536 possible block positions.
- Persisted snapshot block storage and terrain meshing are divided into
  **16x16x16 sections**. A current column spans 16 sections; it is not emitted
  as one 256-block-tall mesh. Generation and some compile staging are still
  dense across the full column.
- Minecraft Java 1.18 raised the default Overworld from 256 blocks to **384
  blocks**, Y `-64..=319`. Current Java 26.2 still uses that default.
- Current Java permits custom dimension types much taller than its default
  Overworld. Its present codec/packed-position envelope is approximately Y
  `-2032..=2031`, with aligned `min_y` and `height` and no world top above
  2032. That is a technical coordinate envelope, not a sensible default cost
  target.
- Mclone already serializes `min_y` and `height` in each
  `DimensionDefinition`, and snapshots already carry the same pair. The live
  generator descriptor, gameplay bounds, client world announcement, and
  several staging buffers do not consistently consume those facts yet.
- Configurable finite height should therefore be a **dimension creation/save
  setting**, not a graphics setting. Graphics quality may control how much of
  that authoritative world is resident, meshed, drawn, or represented by LOD.
- The present architecture is not simply obsolete. It is a hybrid: 2D column
  identity and ticketing, sparse 16-cubed storage, and section-scoped meshes.
  That is efficient for surface worlds, but full-column snapshots and some
  dense staging paths become poor fits for extremely tall or vertically
  unbounded worlds.
- A logically infinite vertical world is feasible when only a bounded 3D
  neighborhood is resident. It is a different streaming, generation,
  lighting, persistence, and protocol contract—not a multiplication of the
  existing height constant.

## Terms And Coordinate Convention

Use these terms consistently:

- **Vertical range** is `[min_y, max_y_exclusive)`, where
  `max_y_exclusive = min_y + height`.
- **Column** is the current authoritative `ChunkPos { x, z }` object spanning
  one dimension's complete finite vertical range.
- **Section** is a 16x16x16 block-storage and render unit identified by X, Y,
  and Z section coordinates.
- **Cubic residency** means sections or cubes have independent authoritative
  load/unload identity in all three axes. Merely storing sparse sections inside
  a full-height column is not cubic residency.
- **Logically unbounded** means coordinates can continue without a configured
  world edge while the loaded set remains finite. It never means keeping an
  infinite amount of state resident.

## Height Comparison

The top coordinate is inclusive; the boundary is the first invalid Y.

| World or envelope | Minimum Y | Height | Top Y / boundary | 16-block sections |
|---|---:|---:|---:|---:|
| Mclone live dimensions | 0 | 256 | 255 / 256 | 16 |
| Java 1.17.1 Overworld | 0 | 256 | 255 / 256 | 16 |
| Java 1.18 through stable 26.2 Overworld | -64 | 384 | 319 / 320 | 24 |
| JJThunder v0.8 custom Overworld | -64 | 2,096 | 2,031 / 2,032 | 131 |
| Current Java custom-dimension coordinate envelope | -2,032 | 4,064 | 2,031 / 2,032 | 254 |

The final row shows the widest interval admitted by current Java's dimension
type and 12-bit packed block-Y design. Other combinations inside that envelope
are possible. It does not say Java's normal generator, client, server, or a
particular datapack performs acceptably across all 254 sections.

## Minecraft Java State

### 1.17.1 Reference Specimen

Minecraft Java 1.17.1 has a 256-block default Overworld, `min_y = 0` and
`height = 256`. Its chunk column is internally sectioned rather than one dense
mesh. The local decompiled reference anchors are
[`DimensionType.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/dimension/DimensionType.java)
and
[`NoiseGeneratorSettings.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseGeneratorSettings.java).

This is no longer Mclone's seed-parity target. The legacy `overworld`
generation profile currently uses the 1.17.1 range, but it is internal-mutable
under the compatibility ledger. Mclone Overworld may adopt a different range
through an explicit dimension/profile change without preserving Java output.

### 1.18 And Current Vanilla

Java 1.18 Caves & Cliffs Part II changed the default Overworld to Y
`-64..=319`, a 384-block span. Mojang's
[Part II feature summary](https://www.minecraft.net/en-us/article/caves---cliffs-part-ii-the-features)
states the new lower and upper boundaries. The height did not grow again in
the default Overworld through stable
[Java 26.2](https://www.minecraft.net/en-us/article/minecraft-java-edition-26-2),
and the latest preview reviewed for this topic,
[26.3 Snapshot 4](https://www.minecraft.net/en-us/article/minecraft-26-3-snapshot-4),
did not announce another height change.

The official Java 26.2 server artifact still contains:

```text
dimension_type/overworld: min_y=-64, height=384, logical_height=384
noise_settings/overworld: min_y=-64, height=384
```

The reproducible artifact/hash receipt is recorded in the
[`jjthunder-to-the-max-reference`](jjthunder-to-the-max-reference.md#height-comparison)
study. Its current `DimensionType` codec and constructor enforce:

- `min_y` in `-2032..=2031`;
- positive `height` from 16 through 4,064;
- `min_y` and `height` aligned to 16 blocks; and
- `min_y + height <= 2032`.

The approximately four-thousand-block envelope follows from Java's 12 packed
bits for block Y, after reserving a small margin. Section positions have a
larger Y field, but any block-position consumer still constrains the usable
range. JJThunder demonstrates a real current datapack at 2,096 blocks high;
it also documents substantial generation and product cost. It is evidence
that modern vanilla data paths are configurable, not evidence that such a
height is cheap.

The distinction to preserve is:

```text
default Overworld: stable at 384 blocks since 1.18
custom dimension: configurable inside a much larger finite codec envelope
```

## Verified Current Mclone State

### Storage And Rendering Shape

[`mclone-core/src/chunk.rs`](../../native/crates/mclone-core/src/chunk.rs)
defines 16-block chunk width and 16-block section height. `ChunkSnapshot`
already carries `min_y`, `height`, and a vector of packed sections. It requires
the range to align to sections and omits all-air sections from that packed
vector. Persisted and transmitted empty vertical space can therefore be cheap.

The renderer similarly builds ordinary terrain per 16-high section. That is
the correct granularity for culling and GPU draws, and a taller dimension does
not require a single taller mesh. The important remaining full-column cost is
earlier in the path:
[`mesh_inputs.rs`](../../native/crates/mclone-render-session/src/mesh_inputs.rs)
rehydrates every participating snapshot into a dense
`height * 16 * 16` `Vec<BlockStateId>` before section meshing. Empty packed
sections regain their full dense cost there.

World generation has the same mixed shape.
[`MutableChunkBlockBuffer`](../../native/crates/mclone-worldgen/src/levelgen/chunk.rs)
accepts arbitrary aligned `min_y` and `height`, but allocates one dense byte per
block across the complete column. The 1.17-derived
[`NoiseSettings`](../../native/crates/mclone-worldgen/src/levelgen/settings.rs)
also has range fields and the Java-shaped 2,032 upper boundary, while its live
built-in settings still select `0, 256`.

Lighting is substantially range-aware already: snapshots contain sparse light
sections and the light world is configured from `min_y` and `height`. However,
[`mclone-light/src/pos.rs`](../../native/crates/mclone-light/src/pos.rs) retains
the vanilla 12-bit packed block-Y field, and the current lighting topic still
records missing skylight propagation behavior across absent vertical storage
sections. Variable finite height must be proven against those gaps before
claiming it complete.

### Authority And Configuration Shape

[`DimensionDefinition`](../../native/crates/mclone-server/src/persistence.rs)
already persists these dimension-local facts:

```text
seed
generation profile
horizontal topology
min_y and height
environment flags
```

That is the right durable owner. One realm may contain multiple dimensions
with different ranges. Height should not be promoted to one realm-global
constant, and it should be immutable after dimension creation unless an
explicit migration rewrites or invalidates every affected record.

The current persistence codec only verifies that height is positive. It does
not centrally establish alignment, overflow-safe top calculation, packed-Y
limits, or profile compatibility. Snapshot constructors validate more than
the owning definition, so malformed or incompatible facts can currently fail
late or disagree between subsystems.

The live generation/session identity is still incomplete:

- [`WorldGenerationDescriptor`](../../native/crates/mclone-server/src/world_generation_profile.rs)
  includes profile, seed, and horizontal topology but not vertical range;
- generation scheduling still uses `AUTHORED_WORLD_MIN_Y = 0` and
  `AUTHORED_WORLD_HEIGHT = 256`;
- [`WorldInfo` and `DimensionChange`](../../native/crates/mclone-protocol/src/lib.rs)
  announce dimension key, biome seed, and horizontal topology but not the
  dimension's vertical range; and
- chunk load/unload and tickets use `ChunkPos { x, z }`, although block update
  packets can address a `section_y`.

Several consumers already respect each received snapshot's local range,
including client block lookup and parts of lighting/physics. Other authority
paths still use Java-Overworld constants:

- breaking/placement has a maximum build height of 256 and no dimension-owned
  minimum in
  [`game_mode.rs`](../../native/crates/mclone-server/src/game_mode.rs);
- safe spawn, dry-run mob spawning, and random-position navigation scan
  `0..256`;
- built-in generation and job-codec fixtures construct `0, 256` buffers; and
- some metadata documentation predates `DimensionDefinition` and still calls
  vertical facts future work.

The result is **representation-ready but not contract-ready**. Changing only
the two stored integers would create inconsistent authority.

## Cost Model And Practical Limits

### Finite Taller Columns In The Current Architecture

Height-proportional dense work grows linearly when horizontal radius and
content are held constant. The table shows the block-state array for one
rehydrated mesh-input column only; it excludes biome arrays, light, generated
mesh vertices/indices, neighboring halo columns, physics state, and allocator
overhead.

| Height | Sections | Relative to 256 | Dense mesh block states | Dense worldgen block ids |
|---:|---:|---:|---:|---:|
| 256 | 16 | 1.0x | 0.250 MiB | 0.0625 MiB |
| 384 | 24 | 1.5x | 0.375 MiB | 0.0938 MiB |
| 512 | 32 | 2.0x | 0.500 MiB | 0.1250 MiB |
| 768 | 48 | 3.0x | 0.750 MiB | 0.1875 MiB |
| 2,096 | 131 | 8.19x | 2.047 MiB | 0.512 MiB |
| 4,064 | 254 | 15.88x | 3.969 MiB | 0.992 MiB |

Packed persistence, network snapshots, and section rendering can be much
cheaper than these ratios when most extra sections are air. Dense generation
sampling, dense mesh rehydration, biomes, lighting source discovery, or a
mountain that actually fills the span can still approach the ratios. World
height is therefore not equivalent to worst-case frame cost, but neither is it
free because the save representation is sparse.

Practical tiers for Mclone are:

1. **256 blocks:** live/reference behavior and the only currently supported
   authoritative contract.
2. **384 to roughly 768 blocks:** credible opt-in finite-height targets after
   plumbing and benchmarks. They are useful for proving negative Y and taller
   original dimensions without redesigning every identity.
3. **Up to the Java-shaped 4,064-block envelope:** technically representable
   only after every packed-coordinate and dense-stage audit. This is an
   experimental ceiling, not a preset recommendation.
4. **Beyond approximately -2,032/2,031:** requires replacing vanilla-derived
   12-bit packed block-Y keys and auditing formats, hashes, light graphs, and
   algorithms that assume that range.
5. **Vertically unbounded:** requires independent 3D authority/residency rather
   than a larger finite `height` value.

### View Distance And A Bounded Resident Set

For a column loader that keeps all `s` vertical sections at horizontal radius
`r`, resident section count is approximately proportional to:

```text
r^2 * s
```

Doubling finite height can therefore be offset in raw section count by reducing
horizontal radius by about `sqrt(2)`, not necessarily by halving it. This says
nothing about player-visible quality or generator dependencies, but it
captures why a 384- or 512-block opt-in range is tractable.

An isotropic cubic loader has resident volume proportional to `r^3`. Halving
its section radius retains roughly one eighth as many sections. The user's
intuition is correct: substantially shorter view distance can buy a bounded
3D working set in a logically unbounded world. The performance penalty is not
inherently two or three times; it depends on the shape of that working set,
how much solid content it contains, and which generation/light dependencies
cross its boundary.

Surface worlds should not normally use an isotropic sphere. A practical policy
is anisotropic:

- a long horizontal surface band for ordinary exploration;
- a smaller vertical band around the player;
- a local 3D bubble while underground, flying, falling, or observing a deep
  portal; and
- distant surface/cave representations supplied by LOD or summaries rather
  than authoritative full sections.

Reducing voxel side length is a separate multiplier. Halving the physical
voxel size creates eight times as many samples per cubic metre before
compression, sparsity, or LOD. Small voxels and fully 3D loading are often
demonstrated together, but neither makes the other cheap.

## Why Finite Column Worlds Remain Popular

Finite-height columns fit the dominant surface-world workload:

- horizontal exploration and player interest are mostly two-dimensional;
- heightmaps answer surface, ocean-floor, spawn, precipitation, and visibility
  questions cheaply;
- skylight has a known top source and a bounded downward solve;
- terrain, biome, structure, and feature generation often needs a whole
  vertical profile at one X/Z location;
- entity/tick ownership, save regions, networking, and anti-abuse budgets have
  simple horizontal keys; and
- section-based persistence, transport, and rendering can avoid storing,
  sending, or meshing most air.

That is not intrinsically wasteful. It combines column-oriented authority with
cubic storage/render granularity. It becomes inefficient when the legal
vertical envelope is enormous but only a small disconnected fraction should
be generated or resident.

Columns also simplify server authority, but 3D authority is not fundamentally
unsafe. The server must still canonicalize positions, own generation and
simulation, and bound each client's interest. Cubic authority adds more
neighbors and boundary states, vertical ticket movement, partial-column
publication, unloaded-volume semantics, and denial-of-service budgets. Those
are significant engineering costs, not a reason it cannot work.

## Evidence From 3D Voxel Engines

- [Luanti mapblocks](https://api.luanti.org/map-terminology-and-coordinates/)
  are 16x16x16 fundamental storage, transmission, and load units. Its
  [world boundary](https://docs.luanti.org/for-players/world-boundaries/) is a
  very large finite cube across all three axes. This proves practical cubic
  server authority and lazy loading, though not literal infinity.
- [Godot Voxel Tools generators](https://voxel-tools.readthedocs.io/en/latest/generators/)
  use 16-cubed generation blocks for infinite terrain. The same documentation
  explains that multipass generation uses fixed-height columns because many
  terrain algorithms and dependencies are simpler in that shape even though
  `VoxelTerrain` itself can remain vertically unlimited.
- [Godot Voxel](https://github.com/Zylann/godot_voxel) demonstrates paged
  infinite terrain, chunked meshes, and smooth LOD. Its still-evolving
  multiplayer and blocky-LOD surface is a reminder that rendering an infinite
  field is a smaller problem than shipping mutable authoritative gameplay.
- [OpenCubicChunks CubicChunks3](https://github.com/OpenCubicChunks/CubicChunks3)
  is a current attempt to retrofit nearly infinite height into modern
  Minecraft. Its own status says it is not yet usable or functional; that is
  useful evidence that replacing column assumptions inside an established
  engine is invasive.
- [Sparse voxel octrees](https://research.nvidia.com/publication/2010-02_efficient-sparse-voxel-octrees)
  can compact and ray-cast sparse geometry. A strong rendering acceleration
  structure is not automatically a good mutable simulation, lighting,
  networking, or persistence store. Mclone should not conflate those layers.

The research does not identify one universally superior shape. Mature systems
often keep multiple views: section/cube storage, column summaries and
heightmaps, region persistence, and render-specific sparse structures.

## Accepted Product And Architecture Decisions

### Vertical Range Is Dimension Authority

The canonical range belongs in each persisted `DimensionDefinition` and is
fixed when that dimension is created. All clients, workers, servers, and
reloads must observe the same legal coordinates. Changing graphics quality
must never change which blocks exist, where terrain generates, or whether a
saved position is valid.

A generation profile declares which ranges it supports; it does not secretly
derive the range after a save exists. This keeps generation behavior and
dimension geometry separate but compatibility-checked:

```text
DimensionDefinition
  vertical range: authoritative geometry
  generation profile: content algorithm and compatibility
```

The legacy `overworld` profile currently supports exactly `0, 256`. Internal
mutable profiles, including Mclone Overworld, may gain broader range support
in place after the
[`world-generation-profiles` compatibility ledger](world-generation-profiles.md#compatibility-safety-ledger)
is updated. Authored-only dimensions are the lowest-risk first proof because
they do not require a complete procedural generator to fill the new range.

Candidate creation choices, after implementation and measurement, are:

- **1.17 Reference:** `0, 256`, retained as a legacy comparison preset;
- **Modern:** `-64, 384`, opt-in for a compatible non-reference profile;
- **Tall Experimental:** initially 512 or 768 blocks, benchmark-gated; and
- **Custom:** an advanced later option constrained by the selected profile and
  engine coordinate envelope.

These labels and exact non-reference ranges are product candidates, not an
implemented UI contract.

### Quality Controls Residency, Not World Geometry

Quality presets may later tune:

- horizontal and vertical section interest radii;
- mesh/light compile and upload budgets;
- surface versus cave residency bands;
- occlusion and far-terrain/cave LOD; and
- retention/hysteresis around vertical movement.

The current full-column `ChunkSnapshot` and `ChunkUnload` contract prevents a
quality setting from avoiding most network and client snapshot cost for an
unwanted vertical band. Real vertical quality scaling therefore depends on
section-addressed publication, partial residency, and explicit unknown versus
known-air semantics. Merely culling unseen section meshes is useful but not
the complete feature.

### Keep The Fast Column Path

Do not rewrite `ChunkPos` into a 3D key everywhere as part of finite-height
cleanup. Preserve the ordinary column path for reference and surface worlds.
If cubic residency is pursued, introduce a separate `CubePos`/`SectionPos`
authority and a dimension capability selecting that storage/streaming policy.
Column summaries and heightmaps will remain valuable even for a cubic store.

The best first cubic experiment is likely **finite sky, unbounded or very deep
subterranean space**:

- a known upper boundary preserves skylight source and surface heightmaps;
- ordinary surface generation and long horizontal interest can stay column
  shaped;
- deep cubes can be generated and loaded lazily around explorers; and
- downward expansion exercises real 3D persistence without immediately
  defining weather, sun, atmosphere, and escape semantics for an unbounded
  sky.

This is a lab direction, not a near-term product promise. Generation across
cube boundaries, structures, aquifers, fluids, lighting, falling entities,
teleports, portals, and saves all need explicit unloaded-volume rules.

## Near-Term Cleanup Plan

### Phase 0: One Validated Range Contract

1. Add one shared `VerticalRange`/`DimensionHeight` value type, preferably in
   `mclone-core`, with `min_y`, positive `height`, section alignment,
   overflow-safe `max_y_exclusive`, containment helpers, section bounds, and
   an initial Java-shaped packed-Y envelope.
2. Make `DimensionDefinition` use that validation while preserving an explicit
   persistence codec migration path. Reject invalid dimension records at open,
   rather than allowing a snapshot or light operation to fail later.
3. Treat the range as immutable dimension identity. Reopening with a different
   range is an incompatibility/migration decision, not a cache reset disguised
   as configuration.
4. Include the range in `WorldGenerationDescriptor` and every native/WASM job
   codec so a worker cannot reuse generator state across incompatible ranges.
5. Include the authoritative range in `WorldInfo`, `DimensionChange`, observer
   session setup, and client replica identity before publishing chunks.

This phase should not change generated terrain or expose a UI option. Its exit
criterion is that one canonical `0, 256` fact reaches every owner.

### Phase 1: Remove Behavioral `0..256` Assumptions

1. Route break/place validation, spawn search, mob spawning/navigation,
   teleport/safe-respawn, scheduled simulation bounds, and entity admission
   through the owning dimension range.
2. Make each generator declare and validate supported ranges. Keep reference
   `overworld` exact; prove an authored or internal mutable profile first.
3. Generate buffers and biome/light ranges from the definition instead of
   `AUTHORED_WORLD_*` constants.
4. Validate every loaded/generated snapshot against its owning definition,
   including biome and light section bounds.
5. Reconcile
   [`persistence-architecture.md`](../persistence-architecture.md) with the
   already-landed dimension record and state the range compatibility policy in
   one place.

The first behavior proof should use negative Y and a non-256 top so bugs cannot
pass by accidentally retaining either legacy boundary.

### Phase 2: Bound Height-Proportional Work

1. Replace full-column mesh-input rehydration with sparse or section-scoped
   compile inputs plus explicit neighbor halos.
2. Measure whether procedural generation can emit sections/implicit fills
   without permanently materializing a dense complete column.
3. Audit biome storage, heightmaps, light sources, persistence/network bytes,
   physics extraction, and future distant terrain for work proportional to
   legal height rather than occupied or interested sections.
4. Benchmark 256, 384, 512, and 768-block fixtures on desktop, web/WASM,
   Android, and XR before selecting presets.
5. Only after this evidence, consider section-addressed snapshots/unloads and
   quality-controlled vertical interest.

### Phase 3: Separate Cubic Residency Lab

1. Add an experimental dimension with an independent 3D ticket, persistence,
   revision, publication, and unload identity.
2. Start with a finite top and lazy deep cubes rather than an unlimited sky in
   both directions.
3. Define unknown, generated-air, persisted-empty, and unavailable cube states
   explicitly.
4. Preserve the column fast path and derive column summaries where surface
   gameplay needs them.
5. Compare anisotropic, spherical, and activity-aware interest using resident
   section count and latency—not nominal infinite coordinate range—as the
   primary measures.

## Validation And Measurements

Finite-height cleanup is complete only with:

- unit tests for alignment, positive height, safe top calculation, envelope
  limits, containment, section conversion, codec round-trip, and reopen
  mismatch;
- generator/profile tests for accepted and rejected ranges, including exact
  reference Overworld fingerprints;
- persistence tests proving dimension and every chunk/light snapshot agree
  across restart;
- native and WASM protocol/job-codec tests proving range delivery before chunk
  data and across dimension transfer;
- gameplay tests at `min_y`, `min_y - 1`, `max_y - 1`, and `max_y`, with a
  negative-Y fixture;
- lighting fixtures for padded boundary sections, tall empty gaps, skylight
  source ownership, and propagation across absent stored sections;
- first-draw screenshots at 384 and then 512 blocks in mono, XR per-eye, and
  multiview paths; and
- performance records for generation time, worker bytes, resident packed and
  dense blocks, light nodes, mesh compile/upload time, drawn sections, memory,
  and time to first playable frame.

Rendered-output captures belong under `/tmp`, following the repository's
normal native validation policy.

## Open Questions

- Should Mclone Overworld, authored-only, or a dedicated experimental profile
  be the first to support `-64, 384`?
- Should the first tall preset be 512 or 768 blocks after dense-stage cleanup?
- Which mechanics need a separate `logical_height` concept rather than only a
  physical build range?
- What is the right vertical interest shape for surface, underground, flight,
  spectator, portal, and observer sessions?
- Can lighting and generation treat unloaded deep cubes as sealed/unknown
  without visible discontinuities or nondeterminism?
- Does a deep-only cubic experiment justify its second authority path, or does
  sparse finite height plus section-addressed streaming cover the product need?

## Related Ownership

- [`realm-dimension-runtime.md`](realm-dimension-runtime.md) owns the realm and
  per-dimension runtime/persistence topology.
- [`bounded-world-topology.md`](bounded-world-topology.md) owns horizontal
  finite/periodic identity; it deliberately keeps vertical range as an
  independent finite dimension fact.
- [`world-generation-profiles.md`](world-generation-profiles.md) owns profile
  compatibility and the current all-internal-mutable ledger.
- [`jjthunder-to-the-max-reference.md`](jjthunder-to-the-max-reference.md)
  owns the reproducible 2,096-block community-worldgen specimen and its terrain
  analysis.
- [`lighting.md`](lighting.md) owns solver/storage correctness and the optional
  vertical-gap skylight comparison.
- [`far-lod.md`](far-lod.md) records retirement of the old non-authoritative
  distant surface representation; it does not own authoritative cubic
  residency.
- [`../persistence-architecture.md`](../persistence-architecture.md) owns
  durable record families and migration policy.
