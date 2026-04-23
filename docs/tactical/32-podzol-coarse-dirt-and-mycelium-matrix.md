# Tactical 32 — Podzol, coarse dirt, and mycelium matrix

Finish the remaining live surface/material families that classic 1.17.1 overworld carvers still depend on after tacticals 28-31. This slice ports the missing giant-tree-taiga / shattered-savanna / mushroom follow-through in the overworld surface path, restores the matching `LakeFeature` mycelium/ice follow-through that vanilla expects, adds committed oracle chunks that exercise those families directly, and validates each surface family in the browser instead of relying on generic terrain smoke.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/surfacebuilders/{GiantTreeTaigaSurfaceBuilder,ShatteredSavanaSurfaceBuilder}.java` | `src/worldgen/surface/surface-builders.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/LakeFeature.java` | `src/worldgen/levelgen/feature/lake-feature.ts` |
| `test/worldgen/levelgen/{surface-oracle-fixture,carver-oracle-fixture,noise-based-chunk-generator}.test.ts` | same |
| `test/worldgen/levelgen/feature/water-feature.test.ts`, `test/browser/surface-material-matrix.test.ts` | same |
| `docs/{carver-status,worldgen-status}.md`, `docs/tactical/README.md` | same |

## What landed

- `surface-builders.ts` now ports the missing giant-tree-taiga and shattered-savanna builder branches instead of throwing on those biome families. The noise-threshold dispatch matches vanilla: giant-tree taiga chooses between grass, podzol, and coarse dirt; shattered savanna chooses between grass, coarse dirt, and stone.
- Mushroom fields and mushroom field shore now route through the live overworld surface path with a real `mycelium` top-material config instead of falling back to grass.
- `LakeFeature` now restores carved mushroom-field ceilings to `mycelium` and ports the vanilla water-lake ice follow-through loop, using the existing `Biome.shouldFreeze(...)` path instead of the older "grass only" shortcut.
- The committed oracle matrix now includes three more surface fixtures and three more AIR-step carved fixtures generated from the Java harness:
  - giant-tree taiga at `(-9, 68)`
  - shattered savanna at `(60, 199)`
  - mushroom fields at `(-446, 387)`
- The fixture tests now pin exact top-column counts for those families, which is the only practical way to prove the surface-builder branch choices without letting the surrounding stone bulk hide the signal.
- Browser validation now captures `/tmp/mclone-debug-giant-taiga.png`, `/tmp/mclone-debug-shattered-savanna.png`, and `/tmp/mclone-debug-mushroom-fields.png` through the existing surface-material matrix test.

## Scope choice

Landed here on purpose:

- finish the last remaining live surface/material families that the current classic-carver oracle matrix was still missing
- keep the runtime/oracle palette stable instead of widening it again; this slice exercises the material IDs tactical 28 already introduced
- restore the missing `LakeFeature` follow-through that depends on those same materials
- validate the new families with both committed Java fixtures and targeted browser frames

Still deferred on purpose:

- structure-start village rejection inside `LakeFeature`, because the repo still has no structure-start pipeline to consult
- broader carved-stage block-state expansion where the numeric chunk/oracle model still intentionally flattens vanilla distinctions
- the broader biome-decoration parity work that still makes dark forest, jungle, savanna, snowy biomes, giant-tree taiga, and mushroom fields fall back to reduced feature tables even though their surface path is now correct

## Oracle / done-when

**Unit + integration (Vitest):**

- surface-oracle and carved-oracle fixture tests pin the exact top-column material counts for the new giant-tree-taiga, shattered-savanna, and mushroom chunks
- chunk-generator parity covers all six new oracle fixtures
- `water-feature.test.ts` proves `LakeFeature` restores mushroom-field ceilings to `mycelium`

**Java oracle fixtures:**

- add one surface/carved pair for each newly surfaced family from the existing Java oracle harness, not handwritten JSON
- keep the older frozen/badlands/liquid fixtures stable while the widened surface matrix grows

**Browser validation:**

- `pnpm test:browser -- test/browser/surface-material-matrix.test.ts` passes
- `/tmp/mclone-debug-giant-taiga.png`, `/tmp/mclone-debug-shattered-savanna.png`, and `/tmp/mclone-debug-mushroom-fields.png` are manually inspected for the intended top-material families

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for surface-oracle, carved-oracle, chunk-generator parity, and water-feature suites
- `pnpm test:browser -- test/browser/surface-material-matrix.test.ts` passes
- `docs/tactical/README.md`, `docs/carver-status.md`, and `docs/worldgen-status.md` describe the widened podzol/coarse-dirt/mycelium matrix accurately

## Next

Tactical 33 should move back to biome identity instead of another surface-material catch-up pass: dark-forest parity via the missing dark-oak / huge-mushroom / minimal decorator stack, now that the current classic-carver material matrix is broad enough to stop blocking that biome.
