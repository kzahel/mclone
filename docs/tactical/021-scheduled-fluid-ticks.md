# 021: Scheduled Fluid Ticks

Status: active; queue plumbing, generated tick carry-through, level-bearing water/lava ids, Java-shaped water/lava spread helpers, cross-chunk runtime mutation, and the first exact liquid oracle fixture matrix have landed.

## Purpose

Introduce the first real native simulation workload after the no-op block/entity tick phases: server-owned scheduled liquid ticks that can mutate live chunk blocks, publish updated snapshots, and report their cost through the integrated runtime.

This slice started as the tick queue and runtime boundary. It now also owns the first native `FlowingFluid` parity pass for water and deterministic overworld lava, checked against server-backed Java fixtures.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/ServerTickList.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/TickNextTickData.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/FlowingFluid.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/WaterFluid.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/LavaFluid.java`

Key Java facts:

- `ServerLevel` owns `ServerTickList<Block>` and `ServerTickList<Fluid>` separately.
- `ServerTickList` dedupes by `(position, type)` while ordering due work by trigger tick, priority, and sequence.
- A server tick executes at most `65536` scheduled ticks from a list.
- Due entries outside chunks that are ticking with entities loaded are left pending instead of removed.
- Water tick delay is `5`; overworld lava tick delay is `30`.
- `FlowingFluid.tick(...)` mutates/reschedules fluid state, updates neighbors, then spreads downward/sides.

## First Slice

- Add native `WorldBlockPos` and `FluidKind` facts to `mclone_server`.
- Add a server-owned `FluidTickList` with Java-shaped dedupe, trigger ordering, and the `65536` per-tick cap.
- Execute fluid ticks during `IntegratedServer::try_simulation_tick_report()` after the block phase and before the entity phase.
- Keep due ticks pending unless their chunk is currently entity-ticking.
- Add live mutable chunk blocks to `ChunkHolder` so scheduled ticks can mutate generated snapshots after publication.
- Promote block-ticking and entity-ticking chunks to full generated live chunks even when they are not client-visible. Client publication stays view-radius limited, but runtime systems can read/mutate the server's block-ticking square.
- Publish updated chunk snapshots when visible live chunks are mutated.
- Report `fluid_tick_us`, executed tick count, mutated block count, and pending scheduled-fluid-tick count through the native app movement perf path and window title.
- Implement a Java-shaped native `FlowingFluid` subset for stored legacy block-state fluids: source/non-source recomputation, falling fluids, horizontal slope search, water source conversion, lava drop-off/slope distance, lava-down-into-water stone replacement, neighbor rescheduling, and block mutation through live chunks.
- Match Java `ServerTickList` batch behavior by removing all executable due fluid ticks from the pending set before invoking any tick body, so same-tick neighbor updates can re-schedule another due source exactly like the reference `currentlyTicking` phase.
- Implement the source-lava portion of `LiquidBlock.shouldSpreadLiquid(...)`: a lava source with water above or horizontally adjacent converts to obsidian while preserving already scheduled lava ticks.
- Preserve worldgen-emitted `MutableChunkBlockBuffer::liquid_ticks()` in `GeneratedChunk`.
- Register generated liquid ticks with the integrated server when a generated chunk is published.
- Use seed `12345`, chunk `(117,-128)` as the current generated-liquid smoke target; the committed `liquid-carved-chunk` oracle has many pending water ticks there.
- Add native raw block ids for `minecraft:water[level=0..8]` and `minecraft:lava[level=0..8]`; the renderer maps these canonical states back to the base vanilla water/lava model variant because the 1.17.1 blockstate assets do not expose per-level model variants.
- Extend the baseline fluid step to preserve falling water/lava as block `level=8` and horizontal water/lava flow as increasing `level=1..7`.
- Add a small Java-shaped liquid neighbor scheduling pass so placement can re-add adjacent fluids when source-neighbor shape updates require it.
- Match committed oracle fixtures exactly for bounded block regions and pending liquid ticks: water 5-tick slope, water 10-tick slope, cross-chunk water 10-tick slope, water 10-tick vertical fall, water 5-tick source conversion, lava 30-tick slope, lava 60-tick vertical fall, and lava-source/water-contact conversion.

## Current Limits

- The fluid state model now preserves block `level=0..8`, but it is still encoded as raw native ids rather than a full Java-shaped `FluidState`.
- Water flow has exact parity for the current fixture matrix, including falling water and source conversion. Deterministic overworld lava flow has exact parity for the current slope/fall/source-contact fixtures.
- Lava's randomized extra spread-delay branch, flowing-lava water contact to cobblestone, soul-soil/blue-ice basalt conversion, and fire random ticks are not ported.
- Cross-chunk fluid updates now work across live block-ticking chunks, including the case where the target chunk is not entity-ticking yet. Unloaded-boundary parity and persisted pending-tick transfer still need dedicated fixtures.
- Snapshot publication is whole-chunk. Dirty section/block delta messages are deferred.

## Oracle Verification

Generation-stage scheduled liquid ticks are now covered by native tests and existing Java oracle fixtures:

- `test/fixtures/integration/overworld-seed-12345-chunks-117--128-liquid-carved.json` proves a watery generated chunk with 90 pending water ticks at the liquid-carved stage.
- `generated_liquid_ticks_are_registered_when_watery_chunk_is_published` verifies those generated ticks enter the integrated server queue and execute when the chunk is entity-ticking.
- `scheduled_water_slope_matches_oracle_fixture_after_5_ticks` verifies the first horizontal spread from a source in the committed channel fixture.
- `scheduled_water_slope_matches_oracle_fixture_after_10_ticks` verifies the second horizontal spread in the same channel.
- `scheduled_water_cross_chunk_slope_matches_oracle_fixture_after_10_ticks` verifies a Java fixture where water crosses the `x=15/16` chunk boundary while both chunks are entity-ticking.
- `cross_chunk_water_tick_waits_until_neighbor_is_entity_ticking` verifies native scheduler semantics where an entity-ticking source chunk mutates a loaded ticking neighbor chunk, but the neighbor's due follow-up tick remains pending until the player interest moves and the neighbor becomes entity-ticking.
- `scheduled_water_fall_matches_oracle_fixture_after_10_ticks` verifies falling water propagation down a constrained shaft.
- `scheduled_water_source_conversion_matches_oracle_fixture_after_5_ticks` verifies two adjacent sources converting the middle block to a source and preserves Java pending-tick behavior for all three source positions.
- `scheduled_lava_slope_matches_oracle_fixture_after_30_ticks` verifies overworld lava horizontal drop-off and delay in the same constrained channel.
- `scheduled_lava_fall_matches_oracle_fixture_after_60_ticks` verifies falling lava propagation down a constrained shaft.
- `lava_source_water_contact_matches_oracle_fixture_after_1_script_tick` verifies source lava plus adjacent water converts to obsidian while the old lava tick remains pending.

Dynamic tick-for-tick fluid simulation should use the existing server-backed liquid oracle:

```bash
./oracle/integration/gen-liquid-fixture.sh \
    --scenario test/fixtures/liquid-scenarios/water-slope.json \
    --out test/fixtures/liquid/water-slope-10-ticks.json
```

The committed fixture matrix currently includes:

- `test/fixtures/liquid/water-slope-5-ticks.json`
- `test/fixtures/liquid/water-slope-10-ticks.json`
- `test/fixtures/liquid/water-cross-chunk-slope-10-ticks.json`
- `test/fixtures/liquid/water-fall-10-ticks.json`
- `test/fixtures/liquid/water-source-conversion-5-ticks.json`
- `test/fixtures/liquid/lava-slope-30-ticks.json`
- `test/fixtures/liquid/lava-fall-60-ticks.json`
- `test/fixtures/liquid/lava-source-water-contact-1-ticks.json`

Native matches those fixtures' bounded block regions and pending tick lists exactly.

## Gate

Focused server tests:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server -- --nocapture
```

Native app compile/runtime smoke:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-native-client -- --nocapture
cargo check --manifest-path native/Cargo.toml --workspace
```

Optional movement visibility/perf smoke:

```bash
pnpm native:movement:smoke
```

## Follow-Up

The next fluid work should broaden the fixture matrix before changing the algorithm further:

- 0/1/5/10 tick snapshots for each existing water scenario so regressions localize to one tick boundary.
- additional water/lava interaction fixtures, especially flowing lava to cobblestone.
- lava randomized spread-delay fixtures only after the oracle can control or record the relevant random draws.
- soul-soil/blue-ice basalt and lava fire random-tick behavior if those become runtime goals.
- unloaded-boundary fixtures and pending-tick persistence/reload transfer.
- fixture-driven checks for client snapshot publication and later dirty-section deltas.
