# Liquids

Research and implementation notes for Minecraft Java 1.17.1-style liquid simulation in `mclone`.

This document is a reference for future liquid work. It is not a tactical slice by itself. The parity-critical parts should be direct Rust ports of the 1.17.1 fluid/block/tick logic, while scheduling, worker boundaries, transport, and persistence adapters should fit the existing authoritative host architecture.

## Goals

- Match vanilla 1.17.1 water behavior for generated overworld chunks, live block edits, and saved/reloaded worlds.
- Keep liquid simulation authoritative-host owned, not renderer owned.
- Preserve water `level` block-state facts exactly enough for meshing, storage, and oracle tests.
- Execute generation-created scheduled liquid ticks instead of only recording them.
- Keep browser main-thread work limited to presentation and GPU upload.
- Preserve one logical shape for browser singleplayer, remote clients, Node hosts, storage, and oracle tests.

## Reference Source Map

| Concern | Vanilla source |
|---|---|
| Base fluid API | `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/Fluid.java` |
| Fluid state wrapper | `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/FluidState.java` |
| Flow algorithm | `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/FlowingFluid.java` |
| Water constants and source conversion | `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/WaterFluid.java` |
| Lava behavior | `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/LavaFluid.java` |
| Registered fluids | `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/Fluids.java` |
| Water/lava block bridge | `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/LiquidBlock.java` |
| Waterlogged placement hook | `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/SimpleWaterloggedBlock.java` |
| Tick list interface | `reference/minecraft-1.17.1/src/net/minecraft/world/level/TickList.java` |
| Server tick queue | `reference/minecraft-1.17.1/src/net/minecraft/world/level/ServerTickList.java` |
| Proto chunk tick capture | `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoTickList.java` |
| Saved chunk tick list | `reference/minecraft-1.17.1/src/net/minecraft/world/level/ChunkTickList.java` |
| Worldgen tick routing | `reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenTickList.java` |
| Server tick integration | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java` |
| Chunk tick promotion/packing | `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunk.java` |
| Tick persistence | `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkSerializer.java` |
| Spring feature tick seeding | `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/SpringFeature.java` |
| Lake feature block tick seeding | `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/LakeFeature.java` |
| Underwater carver tick seeding | `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/carver/UnderwaterCaveWorldCarver.java` |

Read these files before writing the port. The flow rules, state conversion, and tick semantics are simulation parity logic. The host scheduler and worker layout may diverge, but they must preserve the same observable world state.

## Terms

- **Block state**: the stored block and its properties. Vanilla water is a `minecraft:water` block with a `level` property.
- **Fluid state**: a derived API view used by the fluid code. `LiquidBlock.getFluidState(...)` maps the block state's `level` property to source, flowing, or falling fluid state.
- **Source water**: full stable water. Stored as `minecraft:water[level=0]`.
- **Flowing water**: partial horizontal water. Stored as `minecraft:water[level=1..7]`.
- **Falling water**: vertical water column. Stored as `minecraft:water[level=8]`.
- **Scheduled tick**: a queued future update for a block or fluid at one position.
- **Due tick**: a scheduled tick whose trigger time is less than or equal to current game time.
- **Ticking chunk**: a chunk whose simulation is allowed to advance. Vanilla does not execute scheduled ticks in chunks that are not eligible to tick.

## Vanilla Model

Liquids are sparse scheduled updates over stored block states. Vanilla does not scan every water block each server tick, and it does not run a special "settle liquids for N ticks" pass during world load.

Water movement starts when something schedules a fluid tick:

- a liquid block is placed
- a neighboring block changes
- block-shape update code discovers a source neighbor
- a waterlogged block needs fluid follow-up
- worldgen creates exposed spring/carver water and records a generation-time tick
- a saved pending liquid tick is loaded back into the server tick queue

When the tick fires, `FlowingFluid.tick(...)` recomputes the fluid state at that position, updates the block if its level changed, schedules follow-up ticks when needed, and spreads to neighboring cells.

### Chunk Boundary Footprint

Liquid simulation has the same loaded-neighborhood issue as lighting, but not the same fixed radius. Water can eventually travel across arbitrary chunks, yet each scheduled tick only reads a small local footprint: direct neighbors, above/below, and the water slope search (`4` blocks for vanilla water). A water source in an unloaded neighboring chunk must not flow until that chunk is loaded and ticking.

For `mclone`, do not answer fluid reads outside the loaded simulation neighborhood with fake air and then let the tick proceed. The host should defer due boundary liquid ticks until the local read footprint is loaded/decorated/published, keeping the tick pending just like other non-eligible scheduled ticks. This is a host scheduling divergence from vanilla's shared server chunk source, not a change to `FlowingFluid` rules.

### Oceans And Rivers

Oceans and rivers use the same stored water block states as any other water. They are cheap because most of their cells are stable `water[level=0]` source blocks and are not scheduled to tick.

The important rule is:

```text
many stored water blocks
few queued liquid ticks
```

A large ocean only becomes active near disturbances: exposed edges, new holes, block edits, flowing boundaries, or generated positions that explicitly schedule a tick. Stable interior source water does not continuously consume CPU.

## Stored Data

Water level is persisted in block state.

`LiquidBlock.LEVEL` uses vanilla's legacy mapping:

| Block state | Fluid meaning |
|---|---|
| `minecraft:water[level=0]` | source water, amount `8` |
| `minecraft:water[level=1]` | flowing amount `7` |
| `minecraft:water[level=2]` | flowing amount `6` |
| `minecraft:water[level=3]` | flowing amount `5` |
| `minecraft:water[level=4]` | flowing amount `4` |
| `minecraft:water[level=5]` | flowing amount `3` |
| `minecraft:water[level=6]` | flowing amount `2` |
| `minecraft:water[level=7]` | flowing amount `1` |
| `minecraft:water[level=8]` | falling water, amount `8` |

`FlowingFluid.getLegacyLevel(...)` performs the reverse conversion when a fluid state writes a block state.

Pending future updates are stored separately from block states:

- worldgen/proto chunks use `ToBeTicked` and `LiquidsToBeTicked`.
- full chunks use `TileTicks` and `LiquidTicks`.
- saved tick delays are relative to the save time.
- loaded tick delays are scheduled relative to the current game time.

The persisted "settled state" is therefore just normal block states plus any pending `LiquidTicks`.

## Tick Lifecycle

### Generation

During generation, `WorldGenRegion` exposes `WorldGenTickList`s. Those route scheduled block/fluid ticks back to the target chunk's `ProtoTickList`.

Examples:

- `SpringFeature.place(...)` places a source fluid block and schedules a liquid tick with delay `0`.
- `UnderwaterCaveWorldCarver` schedules water ticks when carved water borders air or crosses the chunk edge.
- `LakeFeature` schedules block ticks for generated `cave_air`; this matters because liquid simulation interacts with block updates but is not itself the whole post-processing path.

`ProtoTickList` stores only packed positions. When a proto chunk is promoted, `copyOut(...)` schedules each entry into the real server tick list with delay `0` and derives the current target fluid/block from the world at that position.

### Chunk Promotion

`LevelChunk.postProcessGeneration()` runs post-processing, then calls `unpackTicks()`.

`unpackTicks()` copies chunk-local proto/saved ticks into `ServerLevel.getBlockTicks()` and `ServerLevel.getLiquidTicks()`, then clears the chunk-local lists. The ticks still do not execute until the normal server tick loop reaches them and the position is in a ticking chunk.

### Server Tick

Every normal server tick:

1. world time advances
2. block tick queue runs due block ticks
3. liquid tick queue runs due liquid ticks
4. chunk/entity and other server work continues

The liquid queue is a `ServerTickList<Fluid>`. It is ordered by trigger time, priority, then insertion counter. It has a hard cap of `65536` scheduled entries processed per server tick.

When a due tick is not in an eligible ticking position, vanilla re-schedules it with delay `0` rather than executing it.

### Delay `0`

Delay `0` means "eligible on the next scheduled-tick pass at the current game time." It does not mean "run immediately inside chunk load" and it does not mean "run until settled."

For generation-created fluid ticks this means:

```text
generate proto chunk
record packed liquid tick
promote chunk to active/ticking chunk
copy liquid tick to server queue with delay 0
normal server tick consumes it if the position is ticking
```

## Flow Rules

Port `FlowingFluid` directly. The high-level behavior is:

- If current fluid is not a source, compute `getNewLiquid(...)` from horizontal neighbors and above.
- If the new state is empty, replace the block with air.
- If the new state differs, write its legacy block state, schedule another tick at the same position, and update neighbors.
- Then attempt to spread downward first.
- If downward spread is impossible, or if source-neighbor rules require side spread, spread horizontally.
- Horizontal spread prefers directions that reach a drop within the configured slope search distance.
- Water can convert back to source when at least two horizontal source neighbors exist and the block below is solid or same-source water.

Water-specific values:

- `getTickDelay(...)`: `5`
- `getDropOff(...)`: `1`
- `getSlopeFindDistance(...)`: `4`
- `canConvertToSource()`: `true`

Do not replace this with a simplified flood fill. The exact local rules determine visible water levels, source regeneration, flow direction, and cross-chunk behavior.

## Current Mclone Context

The repo now has the live liquid simulation path wired through the native authoritative server:

| Native source | Current role |
|---|---|
| `native/crates/mclone-server/src/fluid.rs` | fluid kind, scheduled tick list, water/lava tick behavior, and fixture-backed spread/source-conversion tests |
| `native/crates/mclone-server/src/scheduler.rs` | generated/liquid tick hydration, runtime tick eligibility, dirty chunk persistence, and publication timing |
| `native/crates/mclone-server/src/integrated.rs` | local integrated server command/tick facade over the scheduler-owned liquid path |
| `native/crates/mclone-server/src/timing.rs` | liquid tick counters and timing diagnostics |
| `native/crates/mclone-worldgen/src/carver.rs` | underwater liquid carver tick seeding and liquid-carved oracle comparisons |
| `native/crates/mclone-worldgen/src/levelgen.rs` | generation-stage preservation of scheduled liquid ticks |
| `native/crates/mclone-core/src/chunk.rs` | chunk snapshots carry `blockTicks` and `liquidTicks` |
| `native/crates/mclone-mesh/src/builder.rs` | visible water/lava mesh generation and liquid-height sampling |
| `oracle/lib/integration/liquid-fixture.ts` | bounded fixture builder, persisted `LiquidTicks` decoder, and comparison helpers |
| `test/fixtures/liquid/*.json` | committed official-server dynamic liquid oracle fixtures |

Generation-created and saved `liquidTicks` are promoted into the host queue when loaded chunks are snapshotted or ticked. Live water mutations dirty the owning chunk and are currently published as replacement `chunk_snapshot` messages, matching the existing protocol surface. A future protocol slice can replace that coarse update with granular block deltas without changing the simulation ownership.

## Mclone Architecture

Keep the same division used by lighting:

| Layer | Liquid responsibility |
|---|---|
| Simulation core | direct ports of fluid state, `FlowingFluid`, `WaterFluid`, `LiquidBlock`, tick-list semantics, block updates |
| Authoritative host | owns game time, due liquid tick queue, ticking-chunk eligibility, mutation batching, persistence dirtying |
| Chunk workers | generate chunks and return block states plus generation-created scheduled tick records |
| Storage adapters | persist block states and pending scheduled ticks behind engine-native records |
| Protocol | publish chunk snapshots and later chunk deltas with tick/revision context |
| Renderer/meshing | consume authoritative block states; never decide liquid simulation |

The host may budget liquid work, but budgeting must preserve vanilla ordering within the work that is due. If we need to defer due work under load, document that as host scheduling backpressure, not as a changed fluid algorithm.

### Architectural Divergence

Vanilla centralizes scheduled liquid mutation on the server tick thread. `mclone` can map that to a browser worker or Node event-loop authority lane instead of a JVM thread.

The allowed divergence is:

- host/session authority lane owns mutation and queue ordering
- chunk jobs can run off-thread and return generated scheduled ticks
- renderer receives snapshots/deltas after authoritative mutation

The not-allowed divergence is:

- renderer-owned flow
- per-frame scan of loaded water cells
- simplified flood fill for vanilla profile
- dropping water `level` from persisted/snapshot block states
- advancing chunks by an arbitrary "settle count" on load

## Persistence And Protocol

Chunk records need both:

- block states, including `minecraft:water[level=...]`
- pending scheduled liquid ticks, including position, target fluid, delay, and eventually priority

Current snapshots already carry `liquidTicks`, and host snapshots merge pending queue entries back into chunk records with delays relative to the current host game time. `ScheduledTickSnapshot` still lacks priority; a later liquid/block-physics slice should decide whether to extend the logical tick record with priority immediately or keep default priority until non-normal priorities are exercised.

Chunk deltas should eventually carry block mutations caused by liquid ticks. Until that protocol exists, Liquid2 republishes dirty chunks as whole `chunk_snapshot` messages; the client cache and render-world worker already treat those like normal authoritative chunk updates and rebuild affected sections/neighborhoods.

## Verification

Use three tiers.

### Direct Unit Tests

These should use small synthetic worlds and compare exact local outcomes:

- water `level` block-state to fluid-state mapping
- `getLegacyLevel(...)` conversion
- downward spread
- horizontal drop preference
- source regeneration
- no-source decay to air
- duplicate scheduled tick suppression
- due-tick ordering
- chunk-boundary lookup behavior

### Java Oracles

For direct algorithm fixtures, prefer the official server when behavior depends on chunk ticking, save/load, or command-driven setup. A pure Java harness can still be useful for isolated state conversion if it avoids fabricating fake `Level` behavior.

The high-value dynamic oracle shape is:

```text
scenario setup
run exactly N server ticks
stop/save
dump bounded block-state region including water levels
dump pending LiquidTicks
compare TS result after same N ticks
```

Do not rely on screenshots to validate liquid logic. Screenshots are only for visible follow-through after data parity is covered.

### Browser Visual Checks

When liquid ticks affect rendered pixels, run the smallest relevant browser probe:

- hill spring / water patch flowing downhill
- cross-chunk water edge
- ocean/river surface remains stable and does not trigger broad CPU work

Save screenshots to `/tmp` per project policy and inspect them before moving on.

## Implementation Sequence

1. `Liquid0`: oracle foundation for dynamic liquid scenarios. **Done** for the first water-slope official-server fixture.
2. `Liquid1`: direct water simulation foundation in TS, using Liquid0 fixtures. **Done** for the test-local water-slope path.
3. `Liquid2`: authoritative host integration and dirty-chunk publication. **Done** for host queue hydration/execution, pending-tick persistence, and coarse dirty chunk snapshots.
4. `Liquid3`: broaden verification and parity surface: browser visual probe for the hill/spring case, cross-chunk and source-regeneration oracle fixtures, then granular block-delta protocol or lava/waterlogged follow-through depending on the failure found first.
5. Later: interaction with block entities, entity physics, boats, particles/sounds, and visual polish.

Do not include disabled Caves & Cliffs Part 1 aquifer work in this liquid track. The 1.17.1 vanilla overworld target has aquifers disabled, per `AGENTS.md`.
