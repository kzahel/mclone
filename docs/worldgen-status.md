# Native Worldgen Status

Living status page for the native Rust Minecraft Java 1.17.1 overworld worldgen port.

The durable target is seed parity against vanilla 1.17.1 overworld output. The implementation lives primarily in `native/crates/mclone-worldgen`, with scheduler/publication integration in `native/crates/mclone-server` and shared chunk data in `native/crates/mclone-core`.

## Current Shape

Landed native coverage:

- Java-compatible PRNG and worldgen seed helpers.
- Noise primitives, octaved noise, blended noise, and `NoiseSampler`.
- `OverworldBiomeSource` and sampled biome fixtures.
- Terrain density fill, bedrock, and current surface material path.
- Classic overworld AIR and LIQUID carvers, including committed carved-stage oracle fixtures.
- First decoration/feature framework slices, including tree/vegetation/ore/fossil/monster-room follow-through where current native tacticals record it.
- Scheduler-owned `FEATURES` publication and clean fixture comparisons for the current target chunks.
- Generated scheduled tick carry-through for fluids.

Important native entry points:

| Area | Native source |
|---|---|
| PRNG/noise/biomes/terrain/features | `native/crates/mclone-worldgen/src/` |
| Carver status detail | [`carver-status.md`](carver-status.md) |
| Shared chunk/snapshot data | `native/crates/mclone-core/src/chunk.rs` |
| Server scheduler/publication | `native/crates/mclone-server/src/scheduler.rs` |
| Native worldgen smoke | `pnpm native:worldgen:smoke` |
| Shared oracle fixtures | `test/fixtures/` |
| Oracle generation tooling | `oracle/` |

## Oracle Inputs

The retained fixture root is `test/fixtures/`. These fixtures are generated from Java/oracle tooling under `oracle/` and are consumed directly by native Rust tests.

Current fixture families include:

- `test/fixtures/prng/`
- `test/fixtures/noise/`
- `test/fixtures/biome/`
- `test/fixtures/integration/`
- `test/fixtures/scheduler/`
- `test/fixtures/liquid/`
- `test/fixtures/creatures/`

Do not move these fixtures without updating native consumers in `mclone-worldgen`, `mclone-server`, and `mclone-mesh`.

## Deferred Or Incomplete

Still not full vanilla parity:

- full decorated chunk parity remains an active gauntlet rather than a finished guarantee
- broad biome/decorator confidence still needs more targeted fixtures
- most structure families are still missing or skeletal
- full entity/natural-spawn parity is incomplete
- block-state breadth is intentionally narrower than exhaustive vanilla state coverage
- Caves & Cliffs Part 1 systems disabled in 1.17.1 vanilla overworld remain out of scope unless the target changes

## Validation

Use native validation lanes first:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen
pnpm native:worldgen:smoke
pnpm test
```

For cross-system changes, include the fixture-consuming crates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen -p mclone-server -p mclone-mesh
```

## Tactical Trail

Current native tacticals live under [`docs/tactical/`](tactical/README.md). Start with:

- [`003-native-ts-parity-roadmap.md`](tactical/003-native-ts-parity-roadmap.md) for the broad native parity horizon.
- [`015-decoration-framework-foundation.md`](tactical/015-decoration-framework-foundation.md) through the later worldgen tacticals for feature/decorator progress.
- [`017-full-decorated-chunk-parity-gauntlet.md`](tactical/017-full-decorated-chunk-parity-gauntlet.md) for the current full decorated chunk parity target.
