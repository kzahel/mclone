# Tactical 30 — LIQUID floor oracle and tick capture

Finish the LIQUID-step follow-through that tactical 29 intentionally deferred. This slice keeps the worldgen boundary narrow: record scheduled water / magma tick consequences on generated chunks, expose them through chunk snapshots and Java carved fixtures, add a second LIQUID oracle that actually hits the underwater floor branch, and replace the generic terrain smoke with a targeted ravine browser frame.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/carver/UnderwaterCaveWorldCarver.java` | `src/worldgen/carver/underwater-cave-world-carver.ts` |
| `reference/.../src/net/minecraft/world/level/chunk/{ProtoChunk,ProtoTickList}.java` | `src/world/level/chunk/level-chunk.ts`, `src/worldgen/chunk/chunk-block-buffer.ts`, `src/world/level/chunk-snapshot.ts` |
| `reference/.../src/net/minecraft/world/level/ChunkTickList.java` | `oracle/java/OracleDumper.java` |
| `src/world/level/{static-render-level,generated-render-level}.ts` | same |
| `src/renderer/debug/debug-free-cam.ts` | same |
| `test/worldgen/levelgen/{carver-oracle-fixture,noise-based-chunk-generator}.test.ts` | same |
| `test/world/chunk-snapshot.test.ts`, `test/browser/cave-mouth.test.ts` | same |

## What landed

- scheduled block/liquid tick capture on generated chunks and copied render chunks via [`level-chunk.ts`](../../src/world/level/chunk/level-chunk.ts), [`chunk-block-buffer.ts`](../../src/worldgen/chunk/chunk-block-buffer.ts), and [`generated-render-level.ts`](../../src/world/level/generated-render-level.ts)
- snapshot persistence for scheduled ticks in [`chunk-snapshot.ts`](../../src/world/level/chunk-snapshot.ts), with a round-trip test in [`test/world/chunk-snapshot.test.ts`](../../test/world/chunk-snapshot.test.ts)
- exact underwater LIQUID scheduling branches in [`underwater-cave-world-carver.ts`](../../src/worldgen/carver/underwater-cave-world-carver.ts): magma-block tick at `y=10`, water-fluid tick when vanilla’s `POSSIBLE_FLOW_DIRECTIONS` rule says the fluid can escape the local chunk or reach air
- Java carved fixtures now include `blockTicks` / `liquidTicks`, and a second committed LIQUID oracle at [`overworld-seed-12345-chunks--129--256-liquid-carved.json`](../../test/fixtures/integration/overworld-seed-12345-chunks--129--256-liquid-carved.json) captures the real underwater-floor `obsidian` / `magma_block` branch
- targeted browser validation through [`test/browser/cave-mouth.test.ts`](../../test/browser/cave-mouth.test.ts), which saves `/tmp/mclone-debug-cave-mouth.png` from a ravine-focused remote debug frame instead of the generic terrain smoke

## Scope choice

Landed here on purpose:

- generation-stage scheduled tick capture only
- oracle proof for underwater-floor block output plus scheduled tick consequences
- snapshot/storage follow-through so generated chunks keep those consequences across host/client boundaries
- one targeted ravine frame for manual visual inspection

Still deferred on purpose:

- executing scheduled water / magma updates after generation; this slice records and verifies them, it does not grow a broader fluid simulation loop
- the remaining carved-stage material/state matrix beyond the current live overworld families
- broader biome-table parity work such as dark forest / jungle / savanna follow-through

## Oracle / done-when

**Unit + integration (Vitest):**

- underwater-carver tests prove the new magma/water scheduling branches explicitly
- chunk snapshot tests prove scheduled ticks survive serialization / hydration
- carved-stage generator parity compares scheduled tick contents against Java fixtures, not only block palettes

**Java oracle fixtures:**

- all carved and liquid-carved fixtures now include `blockTicks` / `liquidTicks`
- one LIQUID fixture remains the ocean water-fill case
- one LIQUID fixture now hits the underwater-floor branch and commits `obsidian` / `magma_block` plus scheduled tick consequences

**Browser validation:**

- `pnpm test:browser -- test/browser/cave-mouth.test.ts` passes
- `/tmp/mclone-debug-cave-mouth.png` has been manually inspected for a ravine-focused frame rather than generic surface vegetation

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for snapshot, runtime storage, underwater carver, carved oracle, and chunk-generator parity suites
- `pnpm test:browser -- test/browser/cave-mouth.test.ts` passes
- `docs/carver-status.md`, `docs/worldgen-status.md`, and `oracle/README.md` describe scheduled underwater tick capture and the widened LIQUID oracle matrix accurately

## Next

Tactical 31 is now [`31-frozen-and-badlands-material-matrix.md`](31-frozen-and-badlands-material-matrix.md): port the missing frozen-ocean and badlands surface follow-through, widen the shared runtime/oracle/render palette for those material families, and add committed frozen/badlands fixtures before tactical 32 closes the remaining podzol/mycelium-era carver surface families.
