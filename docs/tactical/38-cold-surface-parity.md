# Tactical 38 — Cold surface parity

Finish the next high-value cold-family slice behind shoreline parity by porting the missing top-layer snow/ice pass and the two `ice_spikes` surface-structure features it depends on. This slice lands `SnowAndFreezeFeature`, `IceSpikeFeature`, the disk-based `IcePatchFeature` path, the minimal biome/block support they need, and the biome/surface wiring that makes `snowy_beach`, `ice_spikes`, and the already-translated frozen families stop reading like reduced shoreline fallback.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/feature/{SnowAndFreezeFeature,IceSpikeFeature,IcePatchFeature,BaseDiskFeature,DiskReplaceFeature}.java` | `src/worldgen/levelgen/feature/{snow-and-freeze-feature,ice-spike-feature,ice-patch-feature,base-disk-feature,disk-replace-feature}.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/configurations/DiskConfiguration.java` | `src/worldgen/levelgen/feature/configurations/disk-configuration.ts` |
| `reference/.../src/net/minecraft/world/level/biome/Biome.java` (`shouldFreeze`, `shouldSnow`) | `src/worldgen/biome/biome.ts` |
| `reference/.../src/net/minecraft/world/level/block/SnowLayerBlock.java` (`canSurvive`) | `src/world/level/block/snow-layer-block.ts` |
| `reference/.../src/net/minecraft/data/worldgen/{Features,SurfaceBuilders,BiomeDefaultFeatures}.java` | `src/worldgen/levelgen/feature/{features,vegetation-features}.ts`, `src/worldgen/surface/surface-builders.ts`, `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `reference/.../src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` (`tundraBiome`, `beachBiome`, `riverBiome`, `oceanBiome`, `coldOceanBiome`, `lukeWarmOceanBiome`, `frozenOceanBiome`, `swampBiome`) | `src/worldgen/biome/overworld-biome-generation-settings.ts` |
| `test/worldgen/levelgen/feature/{vegetation-parity,cold-surface-feature}.test.ts`, `test/browser/probes/{shoreline-parity,cold-biome-parity}.probe.ts` | same |

## What landed

- `DiskConfiguration`, `BaseDiskFeature`, `DiskReplaceFeature`, `IcePatchFeature`, `IceSpikeFeature`, and `SnowAndFreezeFeature` are now real TS ports instead of missing registry holes.
- `Biome.shouldFreeze(...)` now honors the edge-water check, `Biome.shouldSnow(...)` exists, and `SnowLayerBlock.canSurvive(...)` covers the support rules this worldgen path needs.
- `Features` and `VegetationFeatures` now expose `FREEZE_TOP_LAYER`, `ICE_SPIKE`, and `ICE_PATCH` in the same configured/decorated shapes vanilla uses for the current scope.
- `overworld-biome-generation-settings.ts` now routes `minecraft:ice_spikes` through a real translated table and adds the top-layer freeze pass across the current translated shoreline / frozen / snowy biome families instead of leaving that entire path absent.
- `surface-builders.ts` now treats `minecraft:snowy_beach` like vanilla desert-surface beach sand instead of the old generic grass fallback, and gives `minecraft:ice_spikes` the correct snow-block top material.
- Focused tests now pin the three cold-surface features, the new biome-table wiring, and the cold-surface top-material routing, while browser free-cam validation adds a real `ice_spikes` frame and retakes the shoreline set.

## Scope choice

- Landed here: the minimal cold-surface parity slice that unlocks the visible `ice_spikes` family and removes the missing top-layer freeze path from the already-covered shoreline/frozen tables.
- Kept intentionally narrow at landing time: no warm-ocean coral / `SEA_PICKLE` / `WARM_OCEAN_VEGETATION`, and no bamboo-jungle follow-through. Tactical 39 now covers the warm-ocean slice, and bamboo-jungle remains the next broader biome-identity follow-through after that.
- Also kept honest: the `ice_spikes` browser frame now clearly shows packed-ice spires, while the refreshed `snowy_beach` frame mainly proves the corrected cold shoreline surface mix; the one-layer snow pass is still visually subtle under the current renderer.

## Oracle / done-when

**Unit (Vitest):**

- `cold-surface-feature.test.ts` proves the translated `FREEZE_TOP_LAYER`, `ICE_PATCH`, and `ICE_SPIKE` paths place snow/ice/packed-ice as expected and that `snowy_beach` / `ice_spikes` resolve to the intended top materials.
- `vegetation-parity.test.ts` proves `snowy_beach`, `frozen_river`, frozen-ocean variants, snowy families, and `ice_spikes` now expose the translated cold-surface features instead of the old gaps.

**Browser validation:**

- `pnpm probe:browser -- test/browser/probes/shoreline-parity.probe.ts test/browser/probes/cold-biome-parity.probe.ts` passes.
- `/tmp/mclone-debug-ice-spikes.png` is manually inspected and clearly reads as packed-ice spires over snowy terrain.
- `/tmp/mclone-debug-snowy-beach.png` is manually inspected and at least reads as the corrected cold shoreline surface family rather than the old generic grass fallback, even though the snow-layer pass is still visually subdued.

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for cold-surface feature and biome-table coverage
- `pnpm probe:browser -- test/browser/probes/shoreline-parity.probe.ts test/browser/probes/cold-biome-parity.probe.ts` passes
- `docs/tactical/README.md`, `docs/worldgen-status.md`, and `docs/carver-status.md` stop listing cold-surface cleanup as the next missing slice

## Next

The next numbered parity slice should be tactical 41, because [`40-debug-free-cam.md`](40-debug-free-cam.md) is already reserved for temporary debug tooling. That tactical should cover bamboo-jungle parity: the bamboo block / feature path plus the `bamboo_jungle` / `bamboo_jungle_hills` biome tables so the still-missing jungle-family identity stops falling back.
