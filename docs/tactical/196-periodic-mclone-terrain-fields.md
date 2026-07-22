# Tactical 196: Periodic Mclone Terrain Fields

Status: active after Human Review 3 accepted Tactical 192's field revision 6
on 2026-07-22. Tactical 195's cylinder runtime contract is complete; Mclone
Overworld remains explicitly unsupported on periodic topology until this
tactical lands. This is the selected enabling phase between mountain/valley
acceptance and the first river/wetland field.

Topic: `bounded-world-topology`

Topic: `mclone-overworld-generation`

Workstream: shared native Rust world generation, scheduler descriptors, feature
planning, and deterministic review evidence. Desktop/offscreen validation first,
then production browser Worker and persisted host closeout.

## Result

Make `mclone-overworld-v1` a real periodic-X terrain caller without changing
the reference Overworld or pretending that canonical chunk wrapping makes
planar noise periodic.

The first result is a cylinder whose:

- terrain, biome, surface, and decoration facts meet at the X seam;
- generator output has one canonical chunk identity;
- rectangular generation algorithms receive a coherent target-relative work
  lift;
- dependency halos and feature writes wrap and deduplicate;
- worker and dependency caches cannot cross seed/profile/topology identity;
- native and browser Workers produce identical payloads; and
- existing unbounded `mclone-overworld-v1` output remains locked unless an
  intentional Tactical 192 field revision changes it first.

This tactical does not add a torus, rivers, hydrology, caves, structures,
multiple visible lifts, nonlinear visual curvature, or patch-atlas generation.

## Sequencing With Tactical 192

Tactical 192 may land and receive human review for its first mountain/valley
field family before this work. Periodicity must begin before rivers, climate
breadth, or structures add more planar ownership assumptions.

At the start of this tactical, inventory the final live field scales and seed
domains after the accepted Tactical 192 geometry. Do not design a period from
the current six foundation octaves and then silently ignore newly landed ridge
or regional fields.

The unbounded sampler is the control. Periodic support is an explicit sampler
mode selected from the dimension definition, not a global change to
`ValueNoise2d`, `BlockPos`, or every Mclone world.

## First Period Decision

The current foundation scales are 2,048, 1,024, 512, 384, 128, and 48 blocks.
Their least common multiple is 6,144 blocks, or 384 chunks. A wrapped-lattice
sampler can preserve those scales exactly when the circumference is a multiple
of every live lattice scale.

The provisional first Mclone terrain cylinder is therefore:

```text
X period = 6,144 blocks = 384 chunks
Z topology = unbounded
ordinary view/tracking radius <= 6 chunks
```

This period is a candidate, not a frozen product constant. Slice 0 must either:

1. confirm that every accepted Tactical 192 field scale divides 6,144 blocks;
2. choose the smallest larger common block period and record its storage and
   exploration implications; or
3. reject lattice wrapping in favor of a measured circle-embedded field
   implementation.

Do not change field scales solely to obtain a convenient period without the
same field-map and landscape review required for an ordinary terrain tune.
The 32-chunk Flat Grass cylinder remains the compact gameplay fixture; Mclone
terrain may require an explicit larger `cylinder-x:PERIOD_CHUNKS` definition.

## Sampling Contract

The production sampler must receive topology context explicitly:

```text
McloneOverworldSampler
    seed
    stable field domains
    sampling topology/mode

sample(work_x, work_z)
    -> topology-compatible macro sample
```

For a wrapped-lattice implementation, each field wraps its X lattice index by
the field-specific lattice period before hashing. Interpolation uses the local
unwrapped coordinate so value and first-order slope meet across the world seam.
Applying `world_x rem_euclid(period_blocks)` only before ordinary planar noise
is insufficient: it repeats identity but can expose a discontinuity between
the last and first columns.

Required identities include:

```text
sample(x, z) == sample(x + period_blocks, z)
sample(-1, z) == sample(period_blocks - 1, z)
sample_region across the seam == repeated point sampling
```

The exact ordinary unbounded constructor and output remain available. Periodic
field state must be immutable and cheap to copy like the existing sampler.
Point sampling, bounded-region probes, chunk generation, biome choice, surface
rules, spawn search, and review maps must all call the same production path.

## Canonical Output And Work-Lift Contract

Generation scheduling already distinguishes the canonical output chunk from a
target-relative `work_lift`. This tactical makes the distinction part of real
Mclone generation rather than only the synthetic Flat Grass proof.

Near the seam, a target at canonical chunk `383` may use this local work
neighborhood:

| Canonical chunk | Work lift |
|---:|---:|
| `381` | `381` |
| `382` | `382` |
| `383` | `383` |
| `0` | `384` |
| `1` | `385` |

Terrain and feature algorithms may use work-lift coordinates for local
rectangular iteration. Every read/write, generated payload, cache entry, and
persistence key maps back to one canonical owner. A feature centered in chunk
`383` may write into lifted chunk `384`, but that output is stored once as
canonical chunk `0`.

The generation plan must expose enough information to prevent a consumer from
reconstructing raw `center + dx/dz` coordinates and losing the lift. Names are
not frozen, but canonical identity and work placement may not collapse into one
context-free `ChunkPos`.

## Feature And Cache Contract

The current Mclone feature stage expands a 3x3 center band over 5x5 Surface
dependencies and uses `FeatureRegion` plus `SurfaceDependencyCache`. Periodic
support must preserve that execution model while changing its addresses:

- canonicalize and stably deduplicate backend centers and prerequisites;
- retain one coherent work lift for each center and dependency relative to the
  target batch;
- canonicalize `FeatureWorld` reads and writes at its adapter boundary;
- derive candidate-grid and decoration PRNG ownership from canonical feature
  identity, not from whichever lift requested it;
- store cached Surface dependencies under canonical positions plus immutable
  seed/profile/topology identity; and
- prove combined and partitioned seam batches are byte-identical.

If `FeatureRegion` cannot represent the mapping honestly without becoming a
topology-aware policy owner, add a small Mclone generation adapter around it.
Do not add implicit modulo behavior to the generic region or to block/chunk
leaf values.

## Execution Checklist

### Slice 0: field and caller audit

- [x] Freeze the accepted Tactical 192 live field/domain inventory.
- [x] Reconfirm the compatibility safety ledger and ordinary Mclone output
  locks.
- [x] Choose wrapped lattice versus circle embedding with maps, cost evidence,
  and an exact first circumference.
- [x] Inventory point/region sampling, terrain, biome, surface, spawn, feature,
  dependency-cache, worker-codec, and review-tool callers.
- [x] Record unsupported rivers/caves/structures as absent rather than
  topology-complete.

Gate: one explicit sampler rule and circumference cover every live field.

Execution record 2026-07-22:

- the accepted field revision 6 inventory is continentalness at `2048`,
  `1024`, and `512` blocks; relief at `384`, `128`, and `48`; ruggedness at
  `1536` and `512`; ridges at `384` and `128`; and warped gradient mountain
  detail at `32` and `8`. Every scale divides 6,144 blocks exactly. The twelve
  existing seed domains remain the frozen identity; periodicity does not add a
  semantic field or domain;
- the compatibility ledger remains `internal-unshipped` with no preservation
  consumer. Existing ordinary field, surface, decorated-payload, reference
  Overworld, and Worker locks are regression guards. The unbounded constructor
  remains the exact control rather than being silently converted;
- select an explicit X-wrapped lattice mode at 6,144 blocks / 384 chunks with
  unbounded Z. It preserves accepted scales and performs only bounded lattice
  index wrapping in the opt-in mode. Circle embedding would require new
  higher-dimensional primitives and a fresh terrain tune without solving a
  demonstrated problem. Release cost and seam maps remain Slice 1/2 evidence;
- live callers are point and region sampling, slope halos, surface fill, biome
  payloads, spawn search, 3x3 feature-center/5x5 Surface work, dependency
  caching, scheduler and Worker descriptors/codecs, field review, terrain
  characteristics, performance, and rendered cards. Each must receive or
  derive the same explicit topology rather than canonicalizing coordinates
  before ordinary noise; and
- rivers, wetlands, streams, caves, carvers, structures, and climate breadth
  are absent from this profile. Tactical 196 makes only the accepted terrain,
  biome, surface, and current vegetation family topology-complete.

### Slice 1: periodic production fields

- [ ] Add an explicit periodic sampler mode without changing the ordinary
  `ValueNoise2d` path.
- [ ] Prove periodic value and seam-slope continuity for positive and negative
  coordinates and every field scale.
- [ ] Route point and region samples through the same production implementation.
- [ ] Generate adjacent canonical seam chunks through coherent work lifts.
- [ ] Extend field receipts with topology, period, seam strips, and ordinary
  control hashes.

Gate: raw fields and derived terrain meet before feature planning changes.

### Slice 2: terrain, biome, and surface seam

- [ ] Enable the selected Mclone profile/topology pair at dimension admission.
- [ ] Generate canonical chunks `P - 1` and `0` with continuous terrain,
  biome, surface, and packed payload facts.
- [ ] Prove ordinary unbounded chunks and reference Overworld locks unchanged.
- [ ] Capture broad periodic field maps and rendered views approaching,
  crossing, and looking along the seam.
- [ ] Review repetition, circumference-scale landmarks, silhouette, coast, and
  valley continuity before adding decoration.

Gate: a player can cross a generated Mclone seam with no content discontinuity.

### Slice 3: dependency and decoration seam

- [ ] Make the Mclone feature plan enumerate canonical outputs plus coherent
  lifted centers and Surface prerequisites.
- [ ] Adapt `FeatureRegion` reads/writes at the topology boundary and retain one
  canonical owner per placed feature.
- [ ] Make `SurfaceDependencyCache` topology-qualified and seam-deduplicated.
- [ ] Prove trees, grass, and flowers crossing the seam under combined,
  partitioned, reversed-order, native-thread, and browser-Worker execution.
- [ ] Verify spawn selection and safe player placement use the same periodic
  terrain facts.

Gate: the current complete Mclone generation plan, not only base terrain, is
periodic and deterministic.

### Slice 4: persisted and platform closeout

- [ ] Save edits and generated seam chunks through SQLite and IndexedDB,
  reopen, and retain one canonical payload per chunk.
- [ ] Prove local, TCP, and WebSocket clients receive the same topology and
  generated chunk facts.
- [ ] Compare ordinary/cylinder generation time, cache hits, worker bytes,
  startup, storage, and resident memory.
- [ ] Run workspace, native pixel, production browser Worker, Android proxy,
  and available XR gates.
- [ ] Update topology and Mclone support matrices with exact unsupported
  families and the next river/wetland boundary.

Gate: periodic Mclone terrain is a supported persisted profile/topology pair
and later content work consumes its sampler contract.

The preferred next content tactical is rivers and wetlands. It should add one
inspectable production river influence and the minimum water-level, bank,
direction, continuity, and reach facts required by terrain and biome/surface
callers. Stream, cascade, and waterfall realization remains a following slice.

## Evidence

Extend the existing `native:worldgen:fields`, worldgen card, perf, and browser
Worker tools. Do not create a second debug-only field implementation.

Minimum deterministic evidence:

- each raw field at `-1`, `0`, `period - 1`, `period`, and ordinary controls;
- seam strips wide enough to cover the largest interpolation footprint;
- point/region equality and `x == x + period` properties;
- canonical chunk payload equality through negative and positive lifts;
- 3x3/5x5 plan deduplication and coherent work-lift receipts;
- combined/partitioned/reversed feature batch hashes;
- ordinary Mclone and reference Overworld locks;
- native-thread/browser-Worker equivalence; and
- inspected normal, debug-seam, along-seam, and post-reopen captures under
  `/tmp`.

## Stop Conditions

Split follow-up work if the selected implementation requires:

- changing accepted Tactical 192 terrain merely to conceal a seam;
- a generic N-dimensional noise graph without a second concrete caller;
- river ownership, hydrology, caves, structures, or climate breadth;
- multiple visible lifts or a circumference smaller than admitted views;
- topology policy in app/platform code; or
- a change to reference Overworld output.

## Related

- [`195-periodic-cylinder-topology-proof.md`](195-periodic-cylinder-topology-proof.md)
- [`192-mclone-overworld-mountains-and-valleys.md`](192-mclone-overworld-mountains-and-valleys.md)
- [`191-guarded-generation-planning-refactor.md`](191-guarded-generation-planning-refactor.md)
- [`../topics/bounded-world-topology.md`](../topics/bounded-world-topology.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
