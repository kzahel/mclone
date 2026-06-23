# Carver Status

Living status page for the overworld carver path.

This doc is narrower than [`worldgen-status.md`](./worldgen-status.md): it only tracks classic 1.17.1 overworld carvers and their oracle coverage. The status-order contract for when carvers run relative to structures, decoration, lighting, and publication lives in [`worldgen-deterministic-order.md`](worldgen-deterministic-order.md).

## Scope

- Target: vanilla Java `1.17.1` overworld carver parity.
- In scope: classic overworld `GenerationStep.Carving.AIR` plus `GenerationStep.Carving.LIQUID` behavior for the default 1.17.1 overworld path, plus the biome/config/oracle plumbing needed to verify both steps.
- Still out of scope in the current native port: aquifer-enabled carving and the disabled Caves & Cliffs Part 1 cave systems called out in [`../AGENTS.md`](../AGENTS.md). Runtime fluid execution is handled by the native liquid path, not by carvers themselves.

## Current state

The repo now has a real, integrated native classic-overworld carver path for both live steps:

- `native/crates/mclone-worldgen/src/carver.rs` owns the translated `WorldCarver`, `CaveWorldCarver`, `CanyonWorldCarver`, underwater carver behavior, built-in overworld configured carvers, AIR/LIQUID step selection, and carved-stage oracle comparisons.
- `native/crates/mclone-worldgen/src/levelgen.rs` owns `NoiseBasedChunkGenerator`-style terrain/carver/surface integration and runtime generation entry points.
- `native/crates/mclone-worldgen/src/chunk.rs` owns mutable generation buffers and scheduled block/liquid tick capture.
- `native/crates/mclone-core/src/chunk.rs` owns packed chunk snapshots carrying block and liquid ticks.
- `native/crates/mclone-server/src/fluid.rs` and `native/crates/mclone-server/src/scheduler.rs` own runtime liquid tick execution after generated ticks are published.
- Surface and feature follow-through now live in `native/crates/mclone-worldgen/src/surface.rs` / `feature.rs` / `levelgen.rs`, keeping carver oracle fixtures aligned with the native worldgen path.

That is enough to truthfully say classic overworld carvers are implemented and integrated.

It is not enough to call the path full parity yet.

## Confirmed parity gaps

These are the highest-signal gaps between the current native port and the 1.17.1 reference behavior.

- Underwater scheduled tick consequences are now captured and oracled at generation time, but the runtime still only records them; it does not execute the later fluid/block updates that a full server tick loop would consume.
- The native replaceable-block set now covers the live desert/ocean/frozen/badlands/podzol/coarse-dirt/mycelium families that the repo can currently surface-build, but it is still narrower than full vanilla `WorldCarver`. The highest-signal remaining gaps are broader block-state distinctions and later families that the current 1.17.1 target still flattens or defers.
- The numeric chunk/oracle palette now includes the current live surface families plus the earlier underwater-floor outputs (`obsidian`, `magma_block`), but it still intentionally collapses some vanilla block-state distinctions that exhaustive parity work would eventually have to separate.
- The current carved-stage oracle matrix is broader, but it is still targeted coverage rather than exhaustive coverage.
- The native path still collapses some vanilla block-state distinctions in the carved-stage numeric model, which is acceptable for narrow chunk diffs but not the final form of exhaustive parity work.

## Current oracle coverage

Current carver verification is real but still intentionally narrow:

- seven committed AIR-only carved-stage oracle fixtures:
  [`overworld-seed-12345-chunks-0-0-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-0-0-carved-only.json)
  , [`overworld-seed-12345-chunks-117--128-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-117--128-carved-only.json),
  [`overworld-seed-12345-chunks-96--64-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-96--64-carved-only.json),
  [`overworld-seed-12345-chunks--320-99-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks--320-99-carved-only.json),
  [`overworld-seed-12345-chunks--9-68-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks--9-68-carved-only.json),
  [`overworld-seed-12345-chunks-60-199-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-60-199-carved-only.json),
  and [`overworld-seed-12345-chunks--446-387-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks--446-387-carved-only.json)
- two committed AIR-plus-LIQUID fixtures:
  [`overworld-seed-12345-chunks-117--128-liquid-carved.json`](../test/fixtures/integration/overworld-seed-12345-chunks-117--128-liquid-carved.json)
  and [`overworld-seed-12345-chunks--129--256-liquid-carved.json`](../test/fixtures/integration/overworld-seed-12345-chunks--129--256-liquid-carved.json)
- companion surface-stage fixtures for the current material families:
  [`overworld-seed-12345-chunks--247--247-surface-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks--247--247-surface-only.json),
  [`overworld-seed-12345-chunks--320-99-surface-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks--320-99-surface-only.json),
  [`overworld-seed-12345-chunks--9-68-surface-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks--9-68-surface-only.json),
  [`overworld-seed-12345-chunks-60-199-surface-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-60-199-surface-only.json),
  and [`overworld-seed-12345-chunks--446-387-surface-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks--446-387-surface-only.json)
- chunk-level parity assertion in [`noise-based-chunk-generator.test.ts`](../test/worldgen/levelgen/noise-based-chunk-generator.test.ts)
- fixture-shape pinning in [`carver-oracle-fixture.test.ts`](../test/worldgen/levelgen/carver-oracle-fixture.test.ts)
- explicit replaceable-material parity tests in [`world-carver-material-parity.test.ts`](../test/worldgen/carver/world-carver-material-parity.test.ts)
- explicit underwater branch tests in [`underwater-carver.test.ts`](../test/worldgen/carver/underwater-carver.test.ts)
- snapshot/tick round-trip coverage in [`chunk-snapshot.test.ts`](../test/world/chunk-snapshot.test.ts)
- biome/carver wiring tests in [`overworld-carver-wiring.test.ts`](../test/worldgen/carver/overworld-carver-wiring.test.ts)
- native carver fixture tests in `native/crates/mclone-worldgen/src/carver.rs`
- native worldgen smoke coverage through `pnpm native:worldgen:smoke`

That supports “partially oracled,” not “exhaustively covered.”

## Recommended next slices

The highest-value sequence from here is:

1. Decide whether recorded scheduled underwater ticks should stay a measured generation artifact or grow into a later runtime simulation requirement.
2. If exhaustive carved-stage diffs remain a priority, widen the flattened numeric model / oracle mapping where vanilla block-state distinctions are still collapsed instead of continuing to hide that lossiness behind a single ID.
3. Keep browser validation aimed at exposed cave mouths / ravines after each substantial carver change, and keep surface-family validation aimed at the widened matrix when the surface path changes.
4. For broader overworld recognizability, classic carvers no longer need to stay above the next biome-table slice; bamboo-jungle parity is now landed too, so the next broad follow-through is the later ore/underground slice outside the narrower exhaustive-parity work.

## Practical definition of “full parity”

For this repo, carvers should only be described as full-parity when all of the following are true:

- AIR and LIQUID carver steps are both ported for the 1.17.1 overworld target.
- Per-biome configured-carver selection matches vanilla through biome generation settings rather than ad hoc fallback logic.
- The carveable/replaceable material set matches vanilla for the current target.
- Underwater scheduled water/magma tick behavior is either executed later in runtime or intentionally measured, stored, and documented as an accepted generation-stage boundary.
- The carved-stage oracle suite covers multiple biome/material families across the current live overworld surface path, not just one chunk near spawn.
- Generated browser frames have been manually inspected for cave mouths/ravines after each substantial carver change.
