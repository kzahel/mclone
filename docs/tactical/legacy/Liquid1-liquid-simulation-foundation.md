# Liquid1 - Liquid simulation foundation

Implementation tactical for the first simulation slice after [`Liquid0-liquid-oracle-foundation.md`](Liquid0-liquid-oracle-foundation.md). Liquid0 provides the `water_slope_10_ticks` official-server fixture and reusable comparison helpers.

## Goal

Port enough of Minecraft Java 1.17.1's water simulation to make generated and edited source water flow through the same scheduled-tick path vanilla uses.

At the end of Liquid1, a controlled TS test world should:

- store and hydrate `minecraft:water[level=...]` block states
- derive source, flowing, and falling `FluidState`s from `LiquidBlock.LEVEL`
- execute scheduled water ticks in vanilla order
- write non-source water levels and air decay through block updates
- match the Liquid0 water-slope fixture for the selected tick count

Whether this slice integrates with the full browser host immediately depends on scope pressure. The minimum useful end state is an authoritative simulation module with tests; the preferred end state also wires it into generated-world host ticks for local browser validation.

## Implementation Status

Landed implementation:

- `FluidState` now carries source/flowing/falling facts and can execute its owning fluid tick.
- `Fluids` now has exact `WATER` / `FLOWING_WATER` identities, with scheduled tick serialization preserving `minecraft:flowing_water`.
- `LiquidBlock` ports the vanilla `level` state cache plus `onPlace`, `updateShape`, and `neighborChanged` liquid scheduling hooks.
- `FlowingFluid` and `WaterFluid` cover the water spread path exercised by the Liquid0 water-slope oracle: new-liquid computation, side spread, source conversion, legacy level writeback, and water constants.
- `ServerTickList` / `TickNextTickData` provide a vanilla-shaped scheduled tick queue for simulation tests: duplicate suppression by position plus exact target identity, trigger-time/priority/insertion ordering, and the vanilla per-tick cap.
- `test/world/liquid-simulation.test.ts` includes the mutable test level, command hydration for the Liquid0 scenario, and exact comparison against `water_slope_10_ticks`.

Runtime host hydration/execution is explicitly deferred to Liquid2. Liquid1 proves the simulation core and tick semantics in a controlled level first.

## Reference Source

Read before implementing:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/Fluid.java` | base API shape |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/FluidState.java` | wrapper methods and block-state conversion |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/FlowingFluid.java` | core spread algorithm |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/WaterFluid.java` | water constants and source conversion |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/Fluids.java` | `WATER` vs `FLOWING_WATER` split |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/LiquidBlock.java` | `LEVEL` cache and scheduling hooks |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/ServerTickList.java` | due queue behavior |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/TickNextTickData.java` | duplicate identity and ordering |
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java` | block/fluid tick entry points |

## Current TS Context

| TS source | Current role |
|---|---|
| `src/world/level/material/fluid.ts` | base fluid API with tick, replacement, and legacy-block hooks |
| `src/world/level/material/fluid-state.ts` | source/flowing/falling state wrapper and tick entry point |
| `src/world/level/material/flowing-fluid.ts` | direct water-relevant `FlowingFluid` port |
| `src/world/level/material/water-fluid.ts` | water constants, source/flowing identity, and legacy block-state mapping |
| `src/world/level/material/fluids.ts` | `WATER` / `FLOWING_WATER` identities plus minimal lava compatibility |
| `src/world/level/block/liquid-block.ts` | `LEVEL` cache and liquid scheduling hooks |
| `src/world/level/server-tick-list.ts` | vanilla-shaped due tick queue for simulation tests |
| `src/world/level/tick-access.ts` | shared scheduling interface |
| `src/world/level/static-render-level.ts` | records generated liquid ticks to chunks |
| `src/world/level/chunk/level-chunk.ts` | stores block states and recorded scheduled ticks |
| `src/world/level/chunk-snapshot.ts` | carries scheduled tick records |
| `src/runtime/` | authoritative host path that Liquid2 should wire to live ticks |

## Scope

| # | Module | Expected result |
|---|---|---|
| 1 | Fluid state model | add flowing/source/falling state support needed by water |
| 2 | `Fluids` | introduce `FLOWING_WATER` and vanilla `WATER` source identity |
| 3 | `LiquidBlock` | port `stateCache`, `getFluidState`, `onPlace`, `updateShape`, and `neighborChanged` scheduling behavior as far as current block API allows |
| 4 | `FlowingFluid` | direct port of `getNewLiquid`, `spread`, side-spread selection, source conversion, and legacy-level conversion |
| 5 | `WaterFluid` | direct port of water constants: delay `5`, dropoff `1`, slope distance `4`, source conversion enabled |
| 6 | Tick queue | implement a vanilla-shaped scheduled liquid tick queue for simulation tests |
| 7 | Test level | add a small mutable level implementing the block/fluid/tick APIs needed by `FlowingFluid` |
| 8 | Oracle comparison | run TS scenarios against Liquid0 fixtures |
| 9 | Runtime hook | deferred to Liquid2: hydrate generation-created `liquidTicks` into the host queue and execute due ticks in authoritative host ticks |
| 10 | Visual probe | deferred to Liquid2 with the host/runtime hook |

## Explicit Non-Goals

- no broad block-physics system beyond what water needs
- no entities, swimming, boats, bubbles, or player collision
- no particles or sounds
- no lava parity unless the shared abstractions make it nearly free
- no exhaustive waterlogged block family
- no aquifer or disabled Caves & Cliffs Part 1 cave systems
- no client-side or renderer-owned water simulation
- no arbitrary load-time settling pass

## Porting Notes

### Preserve Names And Flow

Use the Java class/method names where the local TS style allows it:

- `FlowingFluid.getNewLiquid`
- `FlowingFluid.spread`
- `FlowingFluid.spreadToSides`
- `FlowingFluid.getSpread`
- `FlowingFluid.getSlopeDistance`
- `FlowingFluid.canSpreadTo`
- `FlowingFluid.getLegacyLevel`
- `WaterFluid.getTickDelay`
- `LiquidBlock.getFluidState`

Do not replace the algorithm with a global flood fill or a breadth-first water solver. Vanilla is local scheduled propagation.

### Block-State Mapping

`LiquidBlock.getFluidState(state)` should follow vanilla:

```text
level 0 -> source water
level 1 -> flowing amount 7, falling false
level 2 -> flowing amount 6, falling false
level 3 -> flowing amount 5, falling false
level 4 -> flowing amount 4, falling false
level 5 -> flowing amount 3, falling false
level 6 -> flowing amount 2, falling false
level 7 -> flowing amount 1, falling false
level 8 -> flowing amount 8, falling true
```

`FlowingFluid.getLegacyLevel(state)` should produce the same `level` values when water writes back to a block state.

### Tick Queue

Start with a host-independent queue that can run in unit tests:

- identity key: position plus target fluid identity
- duplicate scheduling suppressed like vanilla
- trigger time: `currentGameTime + delay`
- ordering: trigger time, priority, insertion counter
- execute no more than the vanilla cap if a cap is implemented
- if target fluid no longer matches current fluid at position, skip

Priority can default to normal until block systems require non-normal priorities, but the API should not make priority impossible to add.

### Ticking Chunk Eligibility

Synthetic tests can treat all positions as ticking. Runtime integration must route through host-owned chunk residency/ticking eligibility. If a due liquid tick is not currently eligible, requeue it with delay `0`, matching vanilla.

### Updates And Deltas

When runtime integration lands, water mutation must dirty the authoritative chunk and publish normal block deltas. The renderer should not know the change came from water.

## Test Plan

### Unit Tests

- `water[level]` to `FluidState` mapping
- `FluidState` to legacy block-state mapping
- source water remains source unless disturbed
- downward spread creates falling water
- horizontal spread creates decreasing levels
- water decays to air when no support/source remains
- two-source-neighbor conversion creates source water
- duplicate scheduled ticks are suppressed
- due ticks execute in trigger-time order
- due tick skips when target fluid no longer matches current fluid

### Oracle Tests

Use Liquid0 fixtures:

- hydrate initial scenario in the TS test level
- run exactly the fixture's tick count
- compare bounded block states including water `level`
- compare pending liquid ticks if the fixture includes them

Start with `water_slope_10_ticks`. Add source regeneration and falling-water fixtures as soon as the first water-flow port is stable enough to make those failures useful.

### Browser Visual

Deferred to Liquid2 with runtime integration:

```bash
pnpm probe:browser -- test/browser/probes/<small-liquid-probe>.probe.ts
```

The probe should frame the current hill/spring case or a dedicated debug water slope. Save the screenshot under `/tmp` and inspect it.

## Done When

- water block states preserve exact `level` values through state/snapshot paths used by tests
- `FlowingFluid` and `WaterFluid` water behavior is directly ported for the covered branches
- the scheduled liquid tick queue can execute water ticks deterministically
- at least one Liquid0 fixture passes against the TS implementation
- existing water rendering tests still pass
- runtime integration is either landed or explicitly deferred to Liquid2 with a clear boundary
- targeted unit/oracle tests and `pnpm typecheck` pass

## Follow-Up

`Liquid2`: done in [`Liquid2-authoritative-host-integration.md`](Liquid2-authoritative-host-integration.md). Host chunks now hydrate pending `liquidTicks`, execute due water ticks, republish dirty chunks through the existing snapshot protocol, and persist remaining pending ticks.

`Liquid3`: broaden verification and parity surface before adding more fluid families: browser liquid probe, cross-chunk/source-regeneration oracles, then lava, waterlogging, and block-family follow-through.
