import { describe, expect, test } from "vitest";
import committed from "../fixtures/liquid/water-slope-10-ticks.json" with { type: "json" };
import scenarioSpec from "../fixtures/liquid-scenarios/water-slope.json" with { type: "json" };

import { BlockPos } from "../../src/core/block-pos";
import { Direction } from "../../src/core/direction";
import { Registry } from "../../src/core/registry";
import { ResourceLocation } from "../../src/core/resource-location";
import {
  compareLiquidRegion,
  type BlockStateFixtureEntry,
  type LiquidRegionFixture,
  type LiquidTickPriority,
  type ScheduledLiquidTickFixture,
} from "../../src/oracle/integration/liquid-fixture.ts";
import { normalizeLiquidScenario, type LiquidScenarioSpec } from "../../src/oracle/integration/liquid-scenario.ts";
import { Heightmap } from "../../src/worldgen/levelgen/heightmap";
import type { Biome } from "../../src/worldgen/biome/biome";
import { getLayeredBiomeByKey } from "../../src/worldgen/biome/biome-data";
import { Block } from "../../src/world/level/block/block";
import { LiquidBlock } from "../../src/world/level/block/liquid-block";
import type { BlockState } from "../../src/world/level/block/state/block-state";
import type { Property } from "../../src/world/level/block/state/properties/property";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { LightLayer } from "../../src/world/level/light-layer";
import { Fluids } from "../../src/world/level/material/fluids";
import type { Fluid } from "../../src/world/level/material/fluid";
import type { FluidState } from "../../src/world/level/material/fluid-state";
import { ServerTickList, TickPriority } from "../../src/world/level/server-tick-list";
import type { TickAccess } from "../../src/world/level/tick-access";
import type { WorldGenLevel } from "../../src/world/level/world-gen-level";

class MutableLiquidLevel implements WorldGenLevel {
  private readonly states = new Map<bigint, BlockState>();
  private gameTime = 0;
  private readonly liquidTicks = new ServerTickList<Fluid>(
    (fluid) => fluid === Fluids.EMPTY,
    () => this.gameTime,
    (tick) => {
      const fluidState = this.getFluidState(tick.pos);
      if (fluidState.getType() === tick.getType()) {
        fluidState.tick(this, tick.pos);
      }
    },
  );

  public constructor(
    private readonly airState: BlockState,
    private readonly defaultBiome: Biome = getLayeredBiomeByKey("minecraft:plains"),
  ) {}

  public getGameTime(): number {
    return this.gameTime;
  }

  public runTicks(count: number): void {
    for (let tick = 0; tick < count; tick++) {
      this.gameTime++;
      this.liquidTicks.tick();
    }
  }

  public setBlock(pos: BlockPos, state: BlockState, _flags = 3): boolean {
    const oldState = this.getBlockState(pos);
    if (state.isAir()) {
      this.states.delete(pos.asLong());
    } else {
      this.states.set(pos.asLong(), state);
    }

    if (oldState !== state) {
      state.getBlock().onPlace(state, this, pos, oldState, false);
      this.updateNeighborsAt(pos, state);
    }

    return true;
  }

  private updateNeighborsAt(pos: BlockPos, state: BlockState): void {
    for (const direction of Direction.values()) {
      const neighborPos = pos.relative(direction);
      const neighborState = this.getBlockState(neighborPos);
      const updated = neighborState.updateShape(direction.getOpposite(), state, this, neighborPos, pos);
      if (updated !== neighborState) {
        this.setBlock(neighborPos, updated);
      }
      neighborState.getBlock().neighborChanged(neighborState, this, neighborPos, state.getBlock(), pos, false);
    }
  }

  public getBlockState(pos: BlockPos): BlockState {
    return this.states.get(pos.asLong()) ?? this.airState;
  }

  public isStateAtPosition(pos: BlockPos, predicate: (state: BlockState) => boolean): boolean {
    return predicate(this.getBlockState(pos));
  }

  public getFluidState(pos: BlockPos): FluidState {
    return this.getBlockState(pos).getFluidState();
  }

  public isEmptyBlock(pos: BlockPos): boolean {
    return this.getBlockState(pos).isAir();
  }

  public getHeight(type: Heightmap.Types, x: number, z: number): number {
    for (let y = this.getMaxBuildHeight() - 1; y >= this.getMinBuildHeight(); y--) {
      if (type.isOpaque(this.getBlockState(new BlockPos(x, y, z)))) {
        return y + 1;
      }
    }
    return this.getMinBuildHeight();
  }

  public getHeightmapPos(type: Heightmap.Types, pos: BlockPos): BlockPos {
    return new BlockPos(pos.getX(), this.getHeight(type, pos.getX(), pos.getZ()), pos.getZ());
  }

  public getMinBuildHeight(): number {
    return 0;
  }

  public getMaxBuildHeight(): number {
    return 256;
  }

  public getBiome(_pos: BlockPos): Biome {
    return this.defaultBiome;
  }

  public getBrightness(_layer: LightLayer, _pos: BlockPos): number {
    return 15;
  }

  public getMaxLightLevel(): number {
    return 15;
  }

  public getBlockTicks(): TickAccess<Block> {
    return { scheduleTick: () => {} };
  }

  public getLiquidTicks(): ServerTickList<Fluid> {
    return this.liquidTicks;
  }
}

function parseBlockState(spec: string): BlockState {
  const match = /^([a-z0-9_:.-]+)(?:\[(.*)\])?$/.exec(spec);
  if (match === null) {
    throw new Error(`unsupported block-state spec ${spec}`);
  }

  const block = Registry.BLOCK.get(new ResourceLocation(match[1]!)) as Block | undefined;
  if (block === undefined) {
    throw new Error(`unknown block in spec ${spec}`);
  }

  let state = block.defaultBlockState();
  const properties = match[2];
  if (properties !== undefined) {
    for (const assignment of properties.split(",")) {
      const [name, rawValue] = assignment.split("=");
      const property = state.getProperties().find((candidate) => candidate.getName() === name);
      if (property === undefined || rawValue === undefined) {
        throw new Error(`unknown property assignment ${assignment} in ${spec}`);
      }
      const value = (property as Property<unknown>).getValue(rawValue);
      if (value === undefined) {
        throw new Error(`invalid property value ${assignment} in ${spec}`);
      }
      state = state.setValue(property as Property<unknown>, value);
    }
  }

  return state;
}

function runScenarioCommands(level: MutableLiquidLevel, commands: readonly string[]): void {
  for (const command of commands) {
    const parts = command.split(/\s+/);
    if (parts[0] === "forceload") {
      continue;
    }
    if (parts[0] === "setblock") {
      level.setBlock(new BlockPos(Number(parts[1]), Number(parts[2]), Number(parts[3])), parseBlockState(parts[4]!));
      continue;
    }
    if (parts[0] === "fill") {
      const minX = Math.min(Number(parts[1]), Number(parts[4]));
      const maxX = Math.max(Number(parts[1]), Number(parts[4]));
      const minY = Math.min(Number(parts[2]), Number(parts[5]));
      const maxY = Math.max(Number(parts[2]), Number(parts[5]));
      const minZ = Math.min(Number(parts[3]), Number(parts[6]));
      const maxZ = Math.max(Number(parts[3]), Number(parts[6]));
      const state = parseBlockState(parts[7]!);
      for (let y = minY; y <= maxY; y++) {
        for (let z = minZ; z <= maxZ; z++) {
          for (let x = minX; x <= maxX; x++) {
            level.setBlock(new BlockPos(x, y, z), state);
          }
        }
      }
      continue;
    }
    throw new Error(`unsupported liquid scenario command: ${command}`);
  }
}

function blockStateEntry(state: BlockState): BlockStateFixtureEntry {
  const location = state.getBlock().getLocation();
  if (location === undefined) {
    throw new Error(`unregistered block state ${state}`);
  }

  const properties: Record<string, string> = {};
  for (const property of state.getProperties()) {
    const value = state.getValue(property as Property<unknown>);
    properties[property.getName()] = (property as Property<unknown>).getNameForValue(value);
  }

  return Object.keys(properties).length === 0
    ? { name: location.toString() }
    : { name: location.toString(), properties };
}

function priorityName(priority: TickPriority): LiquidTickPriority {
  switch (priority) {
    case TickPriority.EXTREMELY_HIGH:
      return "extremely_high";
    case TickPriority.VERY_HIGH:
      return "very_high";
    case TickPriority.HIGH:
      return "high";
    case TickPriority.NORMAL:
      return "normal";
    case TickPriority.LOW:
      return "low";
    case TickPriority.VERY_LOW:
      return "very_low";
    case TickPriority.EXTREMELY_LOW:
      return "extremely_low";
  }
}

function fluidName(fluid: Fluid): string {
  if (fluid === Fluids.WATER) {
    return "minecraft:water";
  }
  if (fluid === Fluids.FLOWING_WATER) {
    return "minecraft:flowing_water";
  }
  if (fluid === Fluids.LAVA) {
    return "minecraft:lava";
  }
  if (fluid === Fluids.FLOWING_LAVA) {
    return "minecraft:flowing_lava";
  }
  return "minecraft:empty";
}

function fixtureFromLevel(template: LiquidRegionFixture, scenario: LiquidScenarioSpec, level: MutableLiquidLevel): LiquidRegionFixture {
  const palette: BlockStateFixtureEntry[] = [];
  const paletteIndex = new Map<string, number>();
  const blocks: number[] = [];

  for (let yOffset = 0; yOffset < scenario.bounds.sizeY; yOffset++) {
    const y = scenario.bounds.minY + yOffset;
    for (let zOffset = 0; zOffset < scenario.bounds.sizeZ; zOffset++) {
      const z = scenario.bounds.minZ + zOffset;
      for (let xOffset = 0; xOffset < scenario.bounds.sizeX; xOffset++) {
        const x = scenario.bounds.minX + xOffset;
        const entry = blockStateEntry(level.getBlockState(new BlockPos(x, y, z)));
        const key = `${entry.name}${entry.properties === undefined ? "" : JSON.stringify(entry.properties)}`;
        let index = paletteIndex.get(key);
        if (index === undefined) {
          index = palette.length;
          paletteIndex.set(key, index);
          palette.push(entry);
        }
        blocks.push(index);
      }
    }
  }

  const liquidTicks: ScheduledLiquidTickFixture[] = level.getLiquidTicks().getPendingTicks().map((tick) => ({
    x: tick.pos.getX(),
    y: tick.pos.getY(),
    z: tick.pos.getZ(),
    target: fluidName(tick.getType()),
    delay: tick.triggerTick - level.getGameTime(),
    priority: priorityName(tick.priority),
  }));

  return {
    ...template,
    palette,
    blocks,
    liquidTicks,
  };
}

describe("Liquid1 water simulation foundation", () => {
  test("maps LiquidBlock levels to vanilla source, flowing, and falling FluidStates", () => {
    registerGeneratedRenderBlocks();
    const water = parseBlockState("minecraft:water");

    expect(water.getFluidState().getType()).toBe(Fluids.WATER);
    expect(water.getFluidState().isSource()).toBe(true);
    expect(water.getFluidState().getAmount()).toBe(8);

    const flowing = water.setValue(LiquidBlock.LEVEL, 3).getFluidState();
    expect(flowing.getType()).toBe(Fluids.FLOWING_WATER);
    expect(flowing.isSource()).toBe(false);
    expect(flowing.getAmount()).toBe(5);
    expect(flowing.isFalling()).toBe(false);

    const falling = water.setValue(LiquidBlock.LEVEL, 8).getFluidState();
    expect(falling.getType()).toBe(Fluids.FLOWING_WATER);
    expect(falling.getAmount()).toBe(8);
    expect(falling.isFalling()).toBe(true);
    expect(falling.createLegacyBlock().getValue(LiquidBlock.LEVEL)).toBe(8);
  });

  test("suppresses duplicate liquid ticks by position and exact fluid identity", () => {
    registerGeneratedRenderBlocks();
    let gameTime = 0;
    const executed: string[] = [];
    const ticks = new ServerTickList<Fluid>(
      (fluid) => fluid === Fluids.EMPTY,
      () => gameTime,
      (tick) => executed.push(`${fluidName(tick.getType())}@${tick.pos.asLong()}`),
    );
    const pos = new BlockPos(1, 2, 3);

    ticks.scheduleTick(pos, Fluids.FLOWING_WATER, 5);
    ticks.scheduleTick(pos, Fluids.FLOWING_WATER, 1);
    ticks.scheduleTick(pos, Fluids.WATER, 5);
    expect(ticks.size()).toBe(2);

    gameTime = 5;
    ticks.tick();
    expect(executed).toEqual([
      `minecraft:flowing_water@${pos.asLong()}`,
      `minecraft:water@${pos.asLong()}`,
    ]);
  });

  test("matches the committed Liquid0 water-slope oracle after 10 ticks", () => {
    const blocks = registerGeneratedRenderBlocks();
    const scenario = normalizeLiquidScenario(scenarioSpec);
    const expected = committed as LiquidRegionFixture;
    const level = new MutableLiquidLevel(blocks.airState);
    runScenarioCommands(level, scenario.commands);

    level.runTicks(scenario.ticks);

    const actual = fixtureFromLevel(expected, scenario, level);
    expect(compareLiquidRegion(expected, actual)).toEqual([]);
  });
});
