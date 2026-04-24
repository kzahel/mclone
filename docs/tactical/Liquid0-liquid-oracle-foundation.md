# Liquid0 - Liquid oracle foundation

Standing after the durable liquid reference in [`../liquids.md`](../liquids.md). This is the first liquid-system tactical. It builds the oracle substrate needed to validate dynamic water simulation before porting `FlowingFluid`.

## Goal

Extend the existing Minecraft 1.17.1 oracle infrastructure so it can capture small, scripted liquid-simulation scenarios after an exact number of vanilla server ticks:

- run the official 1.17.1 server with deterministic scripted setup
- place controlled terrain and water using vanilla commands/functions
- advance exactly `N` server ticks
- stop/save the world deterministically
- dump a bounded region preserving full block-state properties, especially `minecraft:water[level=...]`
- dump pending `LiquidTicks` in the touched chunks
- add comparison helpers for bounded liquid fixtures
- commit one or two small fixtures that prove the path

At the end of `Liquid0`, we should be able to say: "for this scripted vanilla scenario after N ticks, these exact water levels and pending liquid ticks are Minecraft's result." The TypeScript liquid simulator does not exist yet.

## Implementation Status

Landed implementation:

- scenario specs live under `test/fixtures/liquid-scenarios/`
- `oracle/integration/gen-liquid-fixture.sh` runs the official 1.17.1 server, injects the generated datapack, advances the scenario for an exact tick count, and dumps a fixture
- `oracle/integration/prepare-liquid-server.ts` writes the deterministic datapack and server config
- `oracle/integration/dump-liquid-fixture.ts` decodes bounded Anvil output into `module: "liquid-sim"` JSON
- `src/oracle/integration/liquid-scenario.ts` validates scenario specs
- `src/oracle/integration/liquid-fixture.ts` builds fixtures, decodes persisted `LiquidTicks`, and compares bounded liquid regions
- `test/fixtures/liquid/water-slope-10-ticks.json` is the first committed dynamic water fixture

The first fixture proves source water spreading into `level=1` and `level=2` states after 10 vanilla server ticks, and preserves the remaining scheduled `minecraft:flowing_water` ticks with delay `5`.

## Why this slice first

Liquid simulation has two independent risks:

1. whether we faithfully port vanilla's flow algorithm
2. whether we can observe vanilla's dynamic output accurately

`Liquid0` solves only the second risk. This avoids implementing water against guesses or screenshots. The later implementation slice can run the same scenario in TS and compare exact block states and scheduled ticks.

The high-value validation mode is:

```text
official MC server scripted setup
  -> run N ticks
  -> dump bounded water-level block states and pending LiquidTicks
  -> run TS simulator for N ticks from same initial setup
  -> compare exact region and tick queue
```

## Reference Source

Read these before implementing:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/FlowingFluid.java` | determines which scenarios exercise meaningful branches |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/WaterFluid.java` | water delay/dropoff/source rules |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/LiquidBlock.java` | block-state `level` mapping and scheduling triggers |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/ServerTickList.java` | due-tick ordering, duplicate suppression, and max-per-tick cap |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/TickNextTickData.java` | tick comparator and identity semantics |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkSerializer.java` | persisted `LiquidTicks` shape |
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java` | liquid tick execution path |

Use the server oracle path for committed dynamic fixtures. Do not use wiki behavior or visual expectation as the oracle.

## Current TS Context

| TS source | Current role |
|---|---|
| `oracle/integration/run-server.sh` | starts official server, waits for startup, then stops |
| `oracle/integration/gen-fixture.sh` | end-to-end server fixture wrapper |
| `oracle/integration/dump-chunks.ts` | dumps complete chunks from generated Anvil region files |
| `oracle/integration/run-liquid-server.sh` | starts the official server with a generated liquid datapack and waits for scenario stop |
| `oracle/integration/gen-liquid-fixture.sh` | end-to-end dynamic liquid fixture wrapper |
| `oracle/integration/dump-liquid-fixture.ts` | dumps bounded property-preserving liquid fixtures |
| `src/oracle/anvil/chunk.ts` | decodes section palettes and block-state properties from Anvil |
| `src/oracle/integration/chunk-fixture.ts` | currently serializes palette entries as resource keys only, dropping properties |
| `src/oracle/integration/liquid-fixture.ts` | serializes bounded liquid regions and persisted liquid ticks |
| `src/world/level/scheduled-tick.ts` | current TS tick snapshot shape has position, target, and delay only |
| `test/fixtures/integration/` | committed chunk/server oracle fixtures |
| `test/fixtures/liquid/` | committed dynamic liquid oracle fixtures |
| `oracle/README.md` | documents oracle command usage |

The existing integration fixture builder is chunk-oriented and currently drops block-state properties in its JSON palette. Liquid fixtures must preserve properties, because water `level` is the observable state being tested.

## Scope

| # | Module | Expected result |
|---|---|---|
| 1 | Scripted server runner | run official 1.17.1 server with a generated datapack/function script and deterministic stop-after-N-ticks behavior |
| 2 | Scenario format | describe bounded setup commands, forced chunks, tick count, and dump bounds in a small JSON or TS-owned spec |
| 3 | Region dumper | emit a bounded region fixture that preserves full palette entries `{ name, properties }` |
| 4 | Liquid tick extraction | decode persisted `LiquidTicks` from touched full chunks, including target, position, remaining delay, and priority if present |
| 5 | Fixture shape | add `module: "liquid-sim"` fixtures separate from broad integration chunk fixtures |
| 6 | Comparison helpers | compare bounded block-state regions and pending liquid ticks with useful diffs |
| 7 | First fixtures | commit a water-slope fixture; source-regeneration/falling-water fixtures can follow in Liquid1 validation |
| 8 | Tests | cover fixture encode/decode, palette properties, tick extraction, and comparison helpers |
| 9 | Docs | document how to regenerate liquid oracle fixtures |

## Explicit Non-Goals

- no TypeScript `FlowingFluid` port
- no runtime tick queue execution
- no host/client protocol changes
- no renderer/meshing changes
- no screenshots required
- no lava fixture requirement in this slice
- no waterlogged block fixture requirement in this slice
- no aquifer, `NormalNoise`, or Caves & Cliffs Part 1 work

## Scripted Server Plan

The current `run-server.sh` stops immediately after startup. Dynamic liquid fixtures need the server to execute setup and tick for a known duration.

Recommended approach:

1. Add a new scripted runner or extend the existing runner behind explicit flags.
2. Generate a datapack into the temporary world before startup.
3. Set `function-permission-level=4` in the generated `server.properties` for scripted oracle worlds.
4. Use a `load` function to:
   - force-load the scenario chunks
   - clear/build the test volume
   - place stone/air/water
   - initialize a scoreboard tick counter
5. Use a `tick` function to increment the counter once per server tick.
6. When the counter reaches the requested tick count, run `save-all flush` and `stop`.

If `stop` from a function proves awkward in 1.17.1, use the same scoreboard marker but have the shell poll the log for a command-emitted marker and then send `stop` on stdin. The important property is exact server tick count, not the specific control mechanism.

Do not use real-time sleeps as the correctness mechanism. They are acceptable as timeouts only.

## Fixture Shape

Keep dynamic liquid fixtures bounded and property-preserving. Do not reuse the current chunk fixture shape that collapses palette entries to names.

Recommended JSON shape:

```jsonc
{
  "module": "liquid-sim",
  "minecraftVersion": "1.17.1",
  "dataVersion": 2730,
  "scenario": "water_slope_5_ticks",
  "ticks": 5,
  "bounds": {
    "minX": 0,
    "minY": 64,
    "minZ": 0,
    "sizeX": 12,
    "sizeY": 6,
    "sizeZ": 12
  },
  "wireFormat": {
    "blockOrder": "y-major,z-major,x-minor",
    "paletteEntries": "block-state"
  },
  "palette": [
    { "name": "minecraft:air" },
    { "name": "minecraft:stone" },
    { "name": "minecraft:water", "properties": { "level": "0" } },
    { "name": "minecraft:water", "properties": { "level": "8" } }
  ],
  "blocks": [0],
  "liquidTicks": [
    { "x": 4, "y": 65, "z": 4, "target": "minecraft:water", "delay": 5, "priority": "normal" }
  ]
}
```

Rules:

- `blocks` is flattened in `y-major,z-major,x-minor` order.
- `palette` entries preserve sorted string properties exactly as decoded from Anvil.
- include only the bounded region, not full chunks, unless a fixture intentionally needs full-chunk evidence.
- `liquidTicks` should include ticks from all chunks touched by `bounds` plus a one-block horizontal margin when practical.
- sort ticks by target time/delay, priority, then position for stable JSON if vanilla insertion order is not part of the assertion.
- if priority is absent in a generation/proto source, normalize to vanilla default `"normal"` in the fixture.

## Initial Scenarios

Start with small, easy-to-debug cases.

### Water Slope

Purpose: prove delay-0/delay-5 execution and visible non-source levels.

Setup:

- stone platform with descending stair-step trench
- one source water block at the top
- forced chunk covering the whole region
- dump after `0`, `1`, `5`, and `10` ticks if fixture size stays reasonable

Expected value:

- after `0` or `1` ticks, source exists and scheduled follow-up may be pending
- after `5` ticks, first spread should be visible
- after `10` ticks, second wave should be visible

### Source Regeneration

Purpose: prove `canConvertToSource()` and two-source-neighbor rule.

Setup:

- small basin
- two adjacent source blocks
- empty center candidate
- dump after enough ticks for conversion

Expected value:

- center becomes `water[level=0]` when vanilla source conversion rules allow it

### Falling Water

Purpose: prove `level=8` falling representation.

Setup:

- source water over a vertical air shaft
- dump after first downward spread

Expected value:

- vertical column uses falling water block state where vanilla does

### Cross-Chunk Edge

Purpose: prove chunk-border scheduling and dump margin behavior.

Setup:

- source near `x=15` or `z=15`
- neighboring chunk forced and included in dump bounds

Expected value:

- flow across the border matches vanilla and pending ticks are captured from both chunks

The first committed Liquid0 implementation can land only the water-slope fixture if the harness pieces are otherwise complete. The other scenarios can follow quickly or move into Liquid1 validation.

## Comparison Helpers

Add helpers that can be reused by Liquid1 tests:

```ts
interface LiquidRegionFixture {
  readonly bounds: RegionBounds;
  readonly palette: readonly BlockStateSnapshot[];
  readonly blocks: readonly number[];
  readonly liquidTicks: readonly ScheduledLiquidTickFixture[];
}

interface LiquidRegionDiff {
  readonly kind: "block_mismatch" | "missing_tick" | "extra_tick" | "tick_mismatch";
  readonly x?: number;
  readonly y?: number;
  readonly z?: number;
  readonly expected?: unknown;
  readonly actual?: unknown;
}
```

The helpers should:

- compare block state keys including properties
- report the first mismatch position and state key
- compare pending liquid ticks in a deterministic normalized order
- validate bounds and block-array length before diffing

## Commands

Validation commands for the implementation:

```bash
pnpm test -- \
  test/oracle/liquid-scenario.test.ts \
  test/oracle/liquid-fixture.test.ts \
  test/oracle/liquid-region-diff.test.ts \
  test/oracle/committed-liquid-fixture.test.ts
pnpm typecheck
```

Fixture generation command:

```bash
./oracle/integration/gen-liquid-fixture.sh \
  --scenario test/fixtures/liquid-scenarios/water-slope.json \
  --out test/fixtures/liquid/water-slope-10-ticks.json
```

If `reference/minecraft-1.17.1/server.jar` is missing, use `scripts/fetch-server-jar.sh 1.17.1`. If that download fails because the sandbox cannot reach Mojang hosts, request escalation and rerun the same command.

## Done When

- the oracle runner can execute a scripted liquid scenario for an exact tick count
- bounded fixtures preserve water `level` properties
- pending persisted liquid ticks are decoded and serialized
- at least one committed liquid fixture exists
- fixture comparison helpers are available for Liquid1
- oracle README documents regeneration
- targeted oracle tests and `pnpm typecheck` pass

## Follow-Up

`Liquid1`: port the water simulation foundation: `FluidState` properties, `FlowingFluid`, `WaterFluid`, `LiquidBlock` level mapping, and a vanilla-shaped liquid tick queue for synthetic tests against Liquid0 fixtures.
