# Tactical 188: Mclone Overworld V1 Terrain Foundation

Status: planned first implementation tactical for
[`mclone-overworld-generation`](../topics/mclone-overworld-generation.md). No
profile identity or terrain behavior has landed yet.

Topic: `mclone-overworld-generation`

Workstream: shared native Rust world generation and server scheduling, with
desktop visual validation first and adoption through the existing profile
contract on every host.

## Result

Land a separately stored provisional `mclone-overworld-v1` profile with:

- unbounded oceans, coasts, lowlands, and rolling uplands;
- a deliberately small biome, surface, and decoration palette;
- safe spawn and deterministic native/browser output;
- production-backed field maps and reusable landscape review cards; and
- explicit pauses for reuse review and behavior-preserving refactoring.

Mountains, rivers, caves, custom biome registry content, and true structures
remain later tacticals. The topic document owns their sequencing and the
durable architecture; this document owns only the first executable slice.

## Product Gate

Before implementation, approve a concise visual target similar to:

> A temperate continuous world with broad oceans, readable sand coasts, open
> grass lowlands, wooded rolling uplands, occasional exposed stone, and enough
> large-scale variation that several seeds do not read as the same island.

Also approve the terrain scale and the seed/region review matrix. Do not land a
profile enum with placeholder terrain while those decisions remain open.

## Fixed Boundaries

- Keep `overworld` and all Java 1.17.1 output locks byte-exact.
- Keep scheduler admission, readiness, lighting, persistence, publication,
  and unload policy profile-neutral.
- Append the `mclone-overworld-v1` persisted tag without renumbering existing
  identities; persistence hits continue to win.
- Keep terrain rules in shared Rust. Apps and browser TypeScript transport and
  display the descriptor without interpreting it.
- Isolate terrain, biome, surface, and decoration seed domains.
- Treat new-profile fixtures as internal regression evidence, not a shipped
  compatibility promise.
- Add concrete profile modules only as behavior needs them. Do not introduce a
  plugin ABI, density DSL, node graph, or speculative trait hierarchy.

## Execution Checklist

### Slice 0: design, inventory, and baselines

- [ ] Approve the product rule, scale vocabulary, and non-goals.
- [ ] Select at least three contrasting seeds, including a negative seed, and
  positive/negative region centers.
- [ ] Define only the structured field values and stable seed domains needed
  by the first terrain rule.
- [ ] Classify relevant reference Overworld and Small Island mechanisms as
  reuse-as-is, extraction candidate with output locks, or profile-owned.
- [ ] Identify affected compatibility-safety ledger rows.
- [ ] Capture clean Overworld oracle/order results and Small Island
  fingerprints/cards before shared changes.
- [ ] Define field hashes/ranges, terrain distributions, and a performance
  comparison command.

Gate: approve the aesthetic target, reuse inventory, evidence commands, and
review matrix. No generator identity lands in this slice.

### Slice 1: first continuous terrain caller

- [ ] Add the profile label/tag and catalog, protocol, persistence, and
  descriptor round trips without changing old values.
- [ ] Add pure absolute-coordinate single-point and bounded-region sampling
  through the same implementation used by chunk generation.
- [ ] Generate continuous ocean, coast, lowland, and rolling-upland terrain
  with correct water, blocks, biomes, heightmaps, and ticks.
- [ ] Add generator-aware safe spawn over dry traversable terrain.
- [ ] Carry descriptor-keyed sessions through native workers and browser
  codecs.
- [ ] Pin seam, negative-coordinate, repeated/reversed/partitioned batch, and
  native/browser-equivalence fingerprints.

Gate: the profile produces real continuous terrain, opens at a safe spawn, and
leaves reference Overworld output exact.

### Review 1: terrain before abstraction

- [ ] Render the approved seed/center matrix before decoration breadth.
- [ ] Map every live field and derived height using the production sampler.
- [ ] Inspect terrain scale, repetition, lattice artifacts, shoreline noise,
  and spawn quality.
- [ ] Record whether to accept, tune, or add one necessary field.

Gate: obtain human review before refactoring or adding content families.

### Slice 2: first reuse and refactoring checkpoint

- [ ] Compare working terrain code with Small Island and reference Overworld.
- [ ] Extract only mechanisms with two concrete consumers or an already
  frozen boundary.
- [ ] Keep radial island composition, mclone macro composition, rule tables,
  and seed domains profile-owned.
- [ ] Separate behavior-preserving extraction from intentional tuning.
- [ ] Re-run exact Overworld locks and both original-profile fingerprints
  after every shared extraction.
- [ ] Reject helpers that add material allocation, payload, or cache cost.

Gate: every extraction names its consumers and evidence. Extracting nothing is
an acceptable review result.

### Slice 3: first terrain language

- [ ] Distinguish open lowlands and wooded uplands with the smallest useful
  profile-owned biome rule and existing biome IDs.
- [ ] Add mclone-owned grass/soil, beach, ocean-floor, and exposed-stone
  surface recipes through shared writing mechanisms.
- [ ] Add an independent decoration domain and a small table of existing
  configured features such as trees, grass, and flowers.
- [ ] Declare exact feature work and prerequisites through
  `ChunkGenerationPlan` without scheduler branches.
- [ ] Prove cross-chunk placement, cache reuse, request-order independence,
  and safe-spawn preservation.
- [ ] Pin field, surface, ordered-feature, and final-chunk fingerprints.

Gate: terrain, biome choice, surface, and decoration have separate ownership
and form a recognizable first original world.

### Review 2: complete foundation

- [ ] Re-run all field maps and landscape cards.
- [ ] Review biome proportions, feature density, coast readability, exposed
  stone, repetition, and performance.
- [ ] Compare Small Island beside mclone output to find shared mechanisms
  without requiring similar results.
- [ ] Bound defects for later mountain/valley or river work.

Gate: obtain human acceptance before the final refactor and host rollout.

### Slice 4: second reuse and boundary checkpoint

- [ ] Compare surface writers, feature recipes, region setup, cache ownership,
  and spawn queries across the three procedural profiles.
- [ ] Extract only data/mechanism boundaries now shared by real callers.
- [ ] Reject abstractions that hide different profile rules behind switches.
- [ ] Confirm vanilla rule tables/PRNG assumptions did not enter mclone code
  and terrain knowledge did not enter scheduler or platform code.
- [ ] Re-run output locks and performance comparisons after refactors.

Gate: concrete profile modules remain clear, shared mechanisms stay small, and
the next terrain family will not copy known common policy.

### Slice 5: persistence, hosts, and closeout

- [ ] Cover shared world creation/catalog selection and descriptor display.
- [ ] Prove SQLite and IndexedDB reopen before generating unseen chunks.
- [ ] Prove native threads, production Web Workers, integrated/dedicated
  startup, remote authoritative consumption, dimension replacement, and warm
  previews.
- [ ] Validate desktop pixels first, then required browser, Android, and XR
  lanes affected by selection/startup changes.
- [ ] Record review receipts, distribution facts, commands, and performance.
- [ ] Update the safety ledger and living topic with the new internal state,
  accepted defects, shared extractions, and selected next tactical.

Gate: every host carries the same stored identity and authoritative output,
and the next content slice is explicitly chosen.

## Evidence And Validation

Screenshots are design evidence; deterministic field and chunk facts are the
regression oracle. Each review receipt records commit/dirty state, profile,
seed, dimension, center, field revision, distributions, generator plan,
worker mode, timing, readiness, and camera facts. Disposable maps and cards
stay under `/tmp`.

Minimum code validation after behavior lands:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --lib \
  -p mclone-worldgen -p mclone-server -p mclone-web-client \
  --target wasm32-unknown-unknown
pnpm native:web:typecheck
```

Run pixel captures at the first drawable milestone and after each visual slice.
Run live browser and platform smokes when profile selection or startup changes.

## Follow-Up Boundary

Use the closeout evidence to choose a mountain/valley tactical, a river/wetland
tactical, or a tooling/refactor tactical. Do not silently expand this one into
caves or structures.

## Related

- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
- [`187-generator-profile-flat-grass-and-seeded-island.md`](187-generator-profile-flat-grass-and-seeded-island.md)
- [`191-guarded-generation-planning-refactor.md`](191-guarded-generation-planning-refactor.md)
- [`../worldgen-status.md`](../worldgen-status.md)
