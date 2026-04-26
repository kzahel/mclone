# Tactical 53 - Swamp-oak vine follow-through

Finish the last clearly known missing overworld vine-decoration slice by restoring vanilla `SWAMP_OAK` leaf-vine decoration and validating that the live swamp vegetation path no longer routes through the old vine-less tree config.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/data/worldgen/Features.java` (`SWAMP_OAK`, `TREES_SWAMP`) | `src/worldgen/levelgen/feature/tree-features.ts`, existing `VegetationFeatures.TREES_SWAMP` consumer |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/treedecorators/LeaveVineDecorator.java` | existing translated `src/worldgen/levelgen/feature/treedecorators/leave-vine-decorator.ts` consumer wiring |
| `test/worldgen/levelgen/feature/{tree-feature,vegetation-parity}.test.ts`, `test/browser/probes/swamp-oak-vines-parity.probe.ts` | same |

## What landed

- `TreeFeatures.SWAMP_OAK` now matches vanilla again by attaching `LeaveVineDecorator.INSTANCE` to the translated swamp-oak tree configuration.
- Focused unit coverage now proves both the tree config itself and the live swamp vegetation consumer path emit the translated leaf-vine decorator instead of the old bare oak shape.
- A dedicated worker-world browser probe now targets the swamp ridge frame at `/tmp/mclone-debug-swamp-oak-vines.png` so the slice keeps a rendered validation step instead of stopping at config-only tests.

## Scope choice

- Landed here: the narrow tree-decorator exactness fix that was still explicitly deferred after the bee and sunflower follow-through slices.
- Kept intentionally narrow: no new vine feature family, no cave-vine work, no broader swamp-table reshaping, and no attempt to expand beyond the already translated `LeaveVineDecorator` implementation.
- Kept exact where it matters: this is not a new decorative approximation; it restores the missing decorator on the existing vanilla `SWAMP_OAK` config that `TREES_SWAMP` already consumes.

## Oracle / done-when

**Unit (Vitest):**

- `tree-feature.test.ts` proves a translated swamp-oak placement can emit vine blocks, so the decorator omission cannot regress silently.
- `vegetation-parity.test.ts` proves the live swamp vegetation path now unwraps to a `TREE` feature carrying `LeaveVineDecorator`.

**Browser validation:**

- `pnpm probe:browser -- test/browser/probes/swamp-oak-vines-parity.probe.ts` passes.
- `/tmp/mclone-debug-swamp-oak-vines.png` is manually inspected and shows the worker-generated swamp ridge / swamp-oak canopy target used for this follow-through. Hanging vines are subtle from this camera distance, but the probe is pointed at the translated swamp-oak consumer path rather than a synthetic placement harness.

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for `tree-feature.test.ts` and `vegetation-parity.test.ts`
- `pnpm probe:browser -- test/browser/probes/swamp-oak-vines-parity.probe.ts` passes
- `docs/worldgen-status.md` stops naming swamp-oak vine consumers as the next simple decoration gap

## Next

Stay in the same narrow visible-exactness lane and do `deep_warm_ocean` `SEAGRASS_SIMPLE` / `CARVING_MASK` follow-through next.
