# Tactical 259: Modern And Historical Coast Reference Survey

Status: active 2026-07-26.

Topics:

- `modern-minecraft-reference`
- `mclone-macro-landscape-planning`
- `mclone-overworld-generation`

Workstream: reference tooling and source-grounded Mclone coast planning.

## Objective

Ground the first Mclone coast campaign before terrain behavior changes:

1. establish a reproducible pinned current-stable Java side reference;
2. compare the coast-selection, geometry, and material mechanisms in Alpha
   v1.1.2_01, Beta 1.7.3, Java 1.17.1, Java 26.2, and current Mclone;
3. capture and measure the current Mclone coast baseline through production
   sampling;
4. identify the smallest useful first coast-family vocabulary and inputs; and
5. update the living macro plan with an evidence-backed next tactical.

This tactical changes no Mclone terrain, biome, surface, chunk, persistence,
worker, LOD, or topology output.

## Slices

### Slice 1: modern reference bootstrap

- [x] Teach the shared mapped-release bootstrap to distinguish an official
  current unobfuscated jar from an old unmapped obfuscated release.
- [x] Add repeatable class/package-prefix selective decompilation.
- [x] Pin Java 26.2 behind `pnpm reference:modern`.
- [x] Record naming, hashes, release, tool, and selection provenance.
- [x] Preserve the Java 1.17.1 mapped/Parchment path unchanged.

### Slice 2: cross-era source survey

- [ ] Record explicit and emergent coast families in Alpha, Beta, 1.17.1, and
  26.2.
- [ ] Separate selection, macro geometry, surface material, climate response,
  and direct-water exceptions.
- [ ] Preserve exact constants and representative fixtures where they clarify
  the mechanism.

### Slice 3: current Mclone evidence

- [ ] Add coast metrics to the existing production field receipt without
  changing sampled terrain.
- [ ] Run equal 6,144-by-6,144-block, eight-block-spacing maps for positive,
  negative, ordinary, and known water-review seeds.
- [ ] Inspect terrain-language maps and record coast-adjacent materials,
  heights, slopes, sampled shoreline length, and land-intent beach distance.

### Slice 4: decision and closeout

- [ ] Select the first bounded Mclone coast vocabulary and classifier inputs.
- [ ] State topology, performance, exact/preview, and visual acceptance
  obligations.
- [ ] Update the reference, macro-planning, Overworld-generation, breadth,
  tactical, and topic indexes.
- [ ] Commit the research/tooling without changing terrain output.

## Initial Constraints

- Java 1.17.1 remains the parity target; Java 26.2 is comparative evidence.
- A visual family is not automatically a distinct persisted biome ID.
- Material choice may respond to coast intent but must not flatten accepted
  terrain merely to make the material fit.
- Coast facts must be reconstructible from the stored dimension descriptor,
  seed, and topology.
- Exact generation and broad previews must consume the same coast intent at
  different declared resolutions.
- No classifier may create a low-quality or featureless wrap meridian.
- The first implementation should remain primarily two-dimensional and
  preserve the measured macro sampling baseline.

## Validation

Required before closeout:

- `bash -n scripts/decompile-mc.sh scripts/decompile-modern-mc.sh`;
- a clean focused 26.2 bootstrap plus idempotent rerun;
- rejection of Parchment and mixed selective receipts on the modern branch;
- an idempotent mapped 1.17.1 bootstrap pass;
- release build/check of `mclone-overworld-review`;
- three equal-grid Mclone coast receipts and inspected maps; and
- clean repository diff review proving no terrain-rule change.
