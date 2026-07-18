# Tactical 188: Mclone Overworld V1 Biome and Decoration Fork

Status: planned follow-up to completed Tactical 187. Do not begin implicitly
while fixing parity in the stored `overworld` profile.

Topic: `world-generation-profiles`

Workstream: shared native Rust world generation and server scheduling, desktop
validation first, with persistence, dedicated, Web Worker, Android, and XR
adoption through the profile contract already proven by Tactical 187.

## Objective

Create the first full original mclone overworld as a separately stored,
versioned profile without changing the meaning or output of the current
Minecraft-1.17-shaped `overworld` profile.

The first `mclone-overworld-v1` should deliberately reuse proven terrain
primitives where useful, but own its biome placement, decoration rules, random
domains, compatibility fixtures, and product identity. It is the point where
the project stops treating further exact vanilla expansion as the only world
generation direction.

This tactical begins only when there is at least one concrete original biome
or decoration combination to implement. It does not create an empty generic
framework in anticipation of unspecified content.

## Current Internal Compatibility Boundary

- `overworld` remains binary tag `0`, stored label `overworld`, and the exact
  Java 1.17.1 oracle/parity target.
- `authored-only`, `flat-grass-v1`, and `small-island-v1` currently use tags
  `1`, `2`, and `3`, but those internal/unshipped identities and algorithms may
  change in place under the compatibility safety ledger;
- the provisional new identity is stored label `mclone-overworld-v1` and the
  next appended binary tag `4`;
- a new stored profile such as `mclone-overworld-v2` becomes mandatory only
  after the safety ledger records a release freeze or a concrete world that
  must survive the change;
- persistence hits always win. The stored per-dimension profile governs only a
  true missing chunk and remains fixed before scheduling;
- vanilla oracle fixtures never become acceptance tests for the original
  profile, and original fixture changes never update vanilla expected output.

See
[`world-generation-profiles.md`](../topics/world-generation-profiles.md#compatibility-safety-ledger)
for the current reasoned disposition and freeze triggers. A `v1` suffix or
fixture alone is not a compatibility promise.

## Evidence From Tactical 187

The generator/host boundary is sufficient. Flat and Island proved that
profile dispatch, descriptor reset, target/dependency planning, chunk output,
lighting, publication, persistence, catalogs, spawn, remote consumption, and
all product hosts do not require an app-local generator abstraction.

The remaining concrete dependencies are intentional:

- `OverworldBiomeSource` supplies terrain sampling, stored biomes, surface
  selection, carvers, decoration context, server biome queries, and vanilla
  spawn search;
- Overworld carver and feature functions accept that concrete source;
- `overworld_features_for_biome*` owns the current vanilla feature tables;
- `OverworldFeatureDependencyCache` owns the vanilla feature neighborhood and
  resident worker state;
- `NoiseBasedChunkGenerator<B: NoiseBiomeSource>` is already the narrow shared
  terrain-density seam, but the rest of the pipeline has only one ruleset
  caller today.

Therefore extraction follows the new caller. Do not genericize the carver,
surface, or feature pipeline until one specific mclone rule needs to share it.

## Seed Domains

Mclone v1 owns stable domains derived from the signed world seed with one
documented, shared-Rust 64-bit mixer. Reserve these exact domain labels before
landing fixtures:

- `mclone:overworld/v1/terrain`
- `mclone:overworld/v1/biome-placement`
- `mclone:overworld/v1/surface`
- `mclone:overworld/v1/decoration`
- `mclone:overworld/v1/structure`

Each subsystem derives its root from `(world seed, domain label)` and then uses
stable coordinate/key derivation. It must not depend on Java random draw counts
or on the execution order of another subsystem. The structure domain is
reserved for compatibility but this tactical does not require structure
runtime implementation.

Changing the mixer, domain spelling, coordinate packing, or key derivation is
an algorithm change. It requires updated internal fixtures while the profile
is internal-mutable, and a new profile version after the safety ledger records
a compatibility freeze.

## Compatibility Fixture Vocabulary

Original-profile fixtures are compact mclone determinism and regression
evidence, not Java oracle output. They become release compatibility evidence
only after the safety ledger records a freeze. Every fixture records the
profile label, signed seed, dimension key, algorithm revision, and requested
chunk/region before one or more of:

- a quart-coordinate biome sample grid with biome keys/IDs and a stable hash;
- a column-height/material fingerprint before decoration;
- surface block and biome hashes for selected chunks;
- ordered decoration feature keys and their absolute placement positions;
- final chunk block/biome/tick fingerprint;
- generation target and dependency-position sets;
- selected safe spawn center and surface result;
- repeated, reversed, and partitioned batch fingerprints.

Start with at least two contrasting seeds, positive and negative chunk
coordinates, a biome boundary, and one decoration whose placement crosses a
chunk edge. Keep screenshots as inspected integration evidence, never as the
determinism oracle.

## Slice Plan

### Slice 0: Identity and frozen-baseline locks

- append `McloneOverworldV1` without changing old labels/tags;
- add catalog/CLI/browser/dedicated round trips and legacy decode fixtures;
- run the full current Overworld oracle and random-order suite unchanged;
- add the seed-domain mixer and fixture schema with no custom content yet.

Gate: old saves decode exactly, the new identity is selectable, and no
`overworld` output changes.

### Slice 1: First concrete biome-rules caller

- choose one original biome-placement rule or biome combination with a clear
  product description;
- introduce the smallest ruleset/source boundary needed by both the frozen
  Overworld and this caller;
- initially reuse existing biome IDs only if that preserves valid renderer,
  tint, spawn, and persistence semantics; otherwise land the shared biome
  registry/key extension first;
- freeze biome-grid and terrain/surface fixtures before adding decoration.

Gate: mclone biome output differs intentionally, is deterministic under batch
and worker ordering, and the frozen Overworld remains byte-exact.

### Slice 2: Decoration fork

- add one original decoration table assembled from existing or newly authored
  configured features;
- split the feature-table/rules owner only where the second caller requires it;
- retain explicit dependency footprints and cross-chunk ordering;
- use the mclone decoration seed domain, not vanilla random draws.

Gate: the same biome can select different vanilla and mclone decoration rules
without conditionals in app/platform code or changes to vanilla fixtures.

### Slice 3: Product and persistence proof

- cover SQLite and IndexedDB create/save/reopen into unseen chunks;
- cover native and Web Worker descriptor reset, dedicated startup, remote
  consumption, independent dimensions, world replacement, and warm previews;
- capture and inspect representative desktop and browser frames;
- build flat Android and Quest XR after shared UI/startup changes.

Gate: every host presents the same stored v1 identity and authoritative output.

### Slice 4: Structure-infrastructure decision

- inventory the first proposed original landmarks/structures and their actual
  cross-chunk metadata/dependency needs;
- either write a separate tactical for native structure starts, pieces,
  references, persistence, and bounding boxes, or explicitly keep early
  landmarks as ordinary placed features when that is semantically honest;
- do not disguise a structure system as app-local decoration.

Gate: the next structure step has an explicit data/lifecycle owner and does not
compromise existing seed compatibility.

## Validation

At minimum:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml \
  --lib \
  -p mclone-worldgen -p mclone-server -p mclone-web-client \
  --target wasm32-unknown-unknown
pnpm native:web:typecheck
```

Run the live browser catalog and generator smokes, native pixel captures, and
Android/XR build lanes whenever product selection or shared startup changes.

## Non-Goals

- no mutation or rename of the current `overworld` profile;
- no dynamic generator/plugin ABI or Mojang codec/registry port;
- no custom content implemented before its product rule is stated;
- no broad biome/carver/surface trait hierarchy without a second caller;
- no structures hidden inside app/platform code;
- no claim that original fixtures are Minecraft oracle fixtures;
- no Caves & Cliffs Part 1 systems disabled by the 1.17.1 target.

## Related

- [`187-generator-profile-flat-grass-and-seeded-island.md`](187-generator-profile-flat-grass-and-seeded-island.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
- [`../worldgen-status.md`](../worldgen-status.md)
- [`../reference-minecraft.md`](../reference-minecraft.md)
- [`103-decorated-biome-fixture-matrix.md`](103-decorated-biome-fixture-matrix.md)
- [`135-overworld-biome-palette-matrix.md`](135-overworld-biome-palette-matrix.md)
- [`146-overworld-macro-terrain-geometry-parity.md`](146-overworld-macro-terrain-geometry-parity.md)
