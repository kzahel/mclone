# Mclone Overworld Generation

Topic: `mclone-overworld-generation`

Status: accepted direction, not yet implemented. The original generator will
land as a separate provisional `mclone-overworld-v1` profile while the current
`overworld` remains the Minecraft Java 1.17.1 reference path. Tactical
[`188`](../tactical/188-mclone-overworld-v1-terrain-foundation.md) owns the
first bounded continuous-terrain foundation.

## Scope

This topic owns the creative and technical direction for mclone's original
Overworld terrain: macro fields, terrain shape, biome placement, surface
language, decoration, rivers, caves, geology, landmarks, and eventual
structures.

It does not own generic profile persistence, worker transport, scheduling,
lighting, or publication. Those contracts and their compatibility safety
ledger remain in
[`world-generation-profiles.md`](world-generation-profiles.md). It also does
not change the reference target recorded in
[`../reference-minecraft.md`](../reference-minecraft.md).

## Product Roles

- `overworld` remains reference-locked to Minecraft Java 1.17.1 output. It is
  the behavior baseline and a source of proven algorithms, not the creative
  profile to mutate.
- `small-island-v1` remains an internal, bounded proving ground for reusable
  noise, surfaces, placed features, dependency planning, and visual review.
- provisional `mclone-overworld-v1` becomes the continuous, explorable,
  original terrain profile.

Small Island must not gradually become the full Overworld. It should pressure
the new profile toward useful shared mechanisms while keeping its own bounded
landform rules. The new profile must not fork scheduler or platform policy.

## Intended Experience

The first original Overworld should be recognizably Minecraft-like in its
block scale, legibility, and progression from terrain to surface to features,
while owning its exact seeds, fields, rules, and content combinations.

The long-term terrain vocabulary includes:

- continuous oceans, coasts, lowlands, uplands, mountain ranges, and valleys;
- climate- and terrain-aware biome placement;
- rivers and wetlands that participate in macro terrain before surface
  materials are applied;
- original surface and decoration combinations built from shared feature
  mechanisms;
- later caves and geological formations with independently seeded 3D rules;
- local landmarks as placed features and, when justified, persistent
  cross-chunk structures.

Minecraft terrain systems are a design and architecture reference, not a seed
parity target for this profile. Concepts such as continentalness, erosion,
ridges, temperature, and moisture are useful vocabulary, but the profile does
not port the Minecraft 1.18 density-function/spline stack or the disabled
1.17.1 Caves & Cliffs paths.

## Pipeline And Ownership

```text
stored mclone-overworld-v1 descriptor
  -> profile-owned seed domains and sampler session
  -> macro terrain fields
  -> terrain intent and biome choice
  -> base chunk blocks and biome payload
  -> profile-owned surface recipes
  -> later profile-owned carvers
  -> profile-owned decoration recipes on shared feature execution
  -> later landmark/structure metadata and placement
  -> shared lighting, publication, persistence, and runtime mutation
```

The generator declares outputs, backend work, and typed prerequisites through
the existing `ChunkGenerationPlan`. The scheduler continues to own readiness,
priority, admission, batching, worker capacity, lighting, persistence,
publication, and unloads. Terrain semantics must not leak back into scheduler
matches or app/platform code.

The profile implementation remains concrete. Closed profile dispatch is
sufficient; this direction does not require dynamic plugins, a worldgen node
graph, Mojang's registry/codec framework, or per-sample trait-object dispatch.

## Structured Sampling Contract

The useful structured request/response seam lives inside worldgen rather than
across another worker round trip. A pure sample should make macro intent
inspectable before it becomes blocks. The initial shape is conceptually:

```rust
struct MacroTerrainSample {
    continentalness: f32,
    ruggedness: f32,
    ridges: f32,
    temperature: f32,
    moisture: f32,
    river: f32,
    base_height: f32,
}
```

Only fields used by a landed slice belong in the live type. The conceptual
shape records the intended separation, not a requirement to add unused zeroes
or reserve an abstract schema prematurely.

Sampling must be:

- a pure function of the profile identity, signed seed, stable seed domain,
  dimension, and absolute coordinate;
- identical across native threads and browser Workers;
- independent of chunk request order, batching, neighboring cache residency,
  and random draws in unrelated subsystems;
- available in single-coordinate and bounded-region forms so generation,
  deterministic probes, and review maps use the same implementation.

The first terrain may use a 2.5D heightfield, but a single height per column
must not become the universal shared contract. Later cliffs, overhangs, caves,
and geological modifiers need a path to profile-owned 3D density or material
sampling. Small Island may remain columnar if forcing it through a density
pipeline adds no value.

## Reuse Boundary

Reuse mechanisms and data contracts; do not accidentally reuse the reference
profile's rule ownership.

| Surface | Direction |
|---|---|
| Profile descriptor, persistence, worker requests, typed prerequisites | reuse unchanged |
| Scheduler, readiness, lighting, publication, unload policy | keep profile-neutral |
| `SeedDomain`, value/Perlin/simplex noise, coordinate math | reuse and extend when a concrete field needs it |
| Chunk buffers, heightmaps, biomes, ticks, snapshots | reuse unchanged |
| `FeatureRegion`, configured/placed features, cross-chunk execution | reuse the executor |
| Vanilla feature and surface tables | keep reference-owned; assemble mclone tables separately |
| Vanilla `NoiseSampler` biome depth/scale composition | reference implementation, not the mclone terrain foundation |
| Density-cell interpolation and block fill mechanics | extract only if the new caller uses the same mechanism |
| Existing carver tunnel/ravine geometry | candidate later; mclone owns domains and distribution |
| Structure start/reference/piece lifecycle | reuse the model later; mclone owns candidates and content |

The strongest constraint wins when a shared primitive also affects the
reference-locked `overworld`. Any extraction from that path requires exact
oracle/output locks. New mclone rule data never enters vanilla fixtures.

## Module Direction

Start the new rules together without first reorganizing every existing
worldgen file:

```text
native/crates/mclone-worldgen/src/
  noise.rs
  feature/
  levelgen/
    chunk.rs
    planning.rs
    mclone_overworld/
      mod.rs
      fields.rs
      terrain.rs
      biomes.rs
      surface.rs
      decoration.rs
      spawn.rs
```

`noise.rs`, `feature/`, `chunk.rs`, and `planning.rs` remain shared mechanism
owners. `mclone_overworld/` owns all original rule composition. Split Small
Island out of `levelgen/profile.rs` when a material change makes that split
useful; do not begin with a mechanical namespace migration.

If a later full Overworld landmark needs the bounded island formula, extract a
pure island landform sampler or a concrete terrain-modifier specification. Do
not invoke the complete `SmallIslandV1` profile inside another profile: a
profile owns complete missing-chunk behavior and is not a composable landmark.

## Long-Term Development Sequence

Develop recognizable vertical terrain families instead of completing every
subsystem horizontally.

1. **Continuous terrain foundation**
   - Independent low-frequency land/ocean and broad-relief fields.
   - Oceans, coasts, lowlands, and rolling uplands across an unbounded world.
   - Minimal existing biome/material palette and safe spawn.
2. **Mountains and valleys**
   - Independent ridge and ruggedness/erosion composition.
   - Terrain creates relief; biome choice reacts to altitude, climate, and
     exposure rather than being the sole source of geometry.
3. **Rivers and wetlands**
   - Deterministic river influence applied before surface recipes.
   - Begin with an inspectable field; true rainfall/flow accumulation can be a
     later refinement.
4. **Biome, surface, and decoration language**
   - Temperature/moisture/altitude combinations and original regional
     recipes.
   - Reuse configured feature implementations while owning selection,
     density, and seed domains.
5. **Caves and geology**
   - Independently seeded 3D subtractive fields or carvers.
   - Reuse geometric helpers only after the first concrete mclone cave rule
     proves the shared shape.
6. **Landmarks and structures**
   - Keep bounded local content in placed features when honest.
   - Add true starts, references, pieces, bounding boxes, and persistence when
     the first cross-chunk landmark requires them.

Each stage may add a new profile version after release freeze. While the
profile remains `planned-unallocated` or internal-mutable, fixtures are
determinism/regression evidence rather than user-save compatibility promises.

## Reuse And Refactoring Review Protocol

Every implementation tactical must reserve explicit review stages. Maximum
reuse does not mean extracting before a caller exists; it means creating time
to compare real callers before the next content layer hides the decision.

### Review A: pre-slice inventory

- State the player-visible terrain rule being added.
- Identify existing reference Overworld and Small Island mechanisms that
  appear relevant.
- Mark each candidate as reuse-as-is, reuse-after-output-locked extraction, or
  intentionally profile-owned.
- Record affected safety-ledger rows and exact non-regression evidence.

### Review B: first concrete caller

- Implement the smallest profile-owned rule through existing generic runtime
  contracts.
- Do not generalize merely to make the first implementation look abstract.
- Capture deterministic field/block facts and first drawable pixels before
  adding the next content family.

### Review C: dedicated reuse/refactor checkpoint

- Compare the working mclone code with Small Island and reference Overworld.
- Extract only behavior with two concrete consumers or a clear frozen
  mechanism boundary.
- Keep rule tables, seed domains, and compatibility fixtures profile-owned.
- Land behavior-preserving refactors separately from intentional output
  changes whenever practical.
- Re-run exact vanilla locks and Small Island seam/order fingerprints for any
  shared change.

### Review D: visual and distribution review

- Inspect multiple positive and negative seeds, more than one region center,
  and boundary coordinates.
- Review both rendered landscapes and field/distribution maps.
- Measure land fraction, height percentiles, slopes, shoreline, and biome
  proportions where they express the intended rule.
- Treat screenshots as design evidence, not the determinism oracle.

### Review E: slice closeout

- Confirm no scheduler or platform policy fork was introduced.
- Record native/browser Worker equivalence and relevant persistence evidence.
- Update this topic, the profile safety ledger, and the tactical result.
- Decide explicitly whether the next slice is content, refactoring, tooling,
  or blocked on a new shared contract.

## Review Tooling Direction

The Small Island review card proves the value of fixed, receipt-backed visual
comparisons. The original Overworld will also need:

- low-resolution regional maps for each landed macro field and derived height;
- rendered landscape views at explicit seed/center/camera coordinates;
- receipts containing profile, seed domains/revision, region, field ranges,
  terrain distributions, readiness, and camera facts;
- output under `/tmp`, with selected numeric fingerprints committed separately
  from disposable screenshots.

Review tooling must sample the same pure field implementation as generation.
Do not create a debug-only approximation of the terrain formula.

## Acceptance Themes

- distinct profile identity without any reference Overworld output change;
- absolute-coordinate seams and deterministic negative-coordinate behavior;
- request-order, batch, partition, native-thread, and Web Worker equivalence;
- explicit seed-domain isolation between terrain, biome placement, surface,
  decoration, caves, rivers, and structures;
- useful safe spawn in the generated terrain;
- no app-local or TypeScript terrain rules;
- reusable code proven by concrete callers, not framework breadth;
- inspected visual and measured distribution evidence before each new terrain
  family.

## Next Work

Execute
[`Tactical 188`](../tactical/188-mclone-overworld-v1-terrain-foundation.md).
Its first gate is a planning/reuse review and concrete aesthetic target, not a
generator identity with placeholder behavior.

## Related

- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`../worldgen-status.md`](../worldgen-status.md)
- [`../reference-minecraft.md`](../reference-minecraft.md)
- [`../structures.md`](../structures.md)
- [`../tactical/187-generator-profile-flat-grass-and-seeded-island.md`](../tactical/187-generator-profile-flat-grass-and-seeded-island.md)
- [`../tactical/191-guarded-generation-planning-refactor.md`](../tactical/191-guarded-generation-planning-refactor.md)
