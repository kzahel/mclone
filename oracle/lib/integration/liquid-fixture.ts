import type { DecodedChunk, DecodedPaletteEntry } from "../anvil/chunk.ts";
import {
  NBT_TAG_COMPOUND,
  type NbtCompound,
  type NbtList,
  type NbtValue,
} from "../anvil/nbt.ts";
import {
  boundsMaxX,
  boundsMaxY,
  boundsMaxZ,
  type LiquidScenarioSpec,
  type RegionBounds,
} from "./liquid-scenario.ts";

export type LiquidTickPriority =
  | "extremely_high"
  | "very_high"
  | "high"
  | "normal"
  | "low"
  | "very_low"
  | "extremely_low";

export interface BlockStateFixtureEntry {
  readonly name: string;
  readonly properties?: Readonly<Record<string, string>>;
}

export interface ScheduledLiquidTickFixture {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly target: string;
  readonly delay: number;
  readonly priority: LiquidTickPriority;
}

export interface LiquidRegionFixture {
  readonly module: "liquid-sim";
  readonly minecraftVersion: string;
  readonly dataVersion: number;
  readonly seed: string;
  readonly generator: string;
  readonly generateStructures: boolean;
  readonly scenario: string;
  readonly ticks: number;
  readonly bounds: RegionBounds;
  readonly wireFormat: {
    readonly blockOrder: "y-major,z-major,x-minor";
    readonly paletteEntries: "block-state";
  };
  readonly palette: readonly BlockStateFixtureEntry[];
  readonly blocks: readonly number[];
  readonly liquidTicks: readonly ScheduledLiquidTickFixture[];
}

export interface ChunkLiquidTickInput {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly ticks: readonly ScheduledLiquidTickFixture[];
}

export interface LiquidRegionDiff {
  readonly kind: "block_mismatch" | "missing_tick" | "extra_tick" | "bounds_mismatch" | "palette_mismatch";
  readonly x?: number;
  readonly y?: number;
  readonly z?: number;
  readonly expected?: unknown;
  readonly actual?: unknown;
}

const AIR_STATE: BlockStateFixtureEntry = { name: "minecraft:air" };
const BLOCKS_PER_SECTION = 16 * 16 * 16;

export function buildLiquidRegionFixture(
  scenario: LiquidScenarioSpec,
  chunks: readonly DecodedChunk[],
  chunkTicks: readonly ChunkLiquidTickInput[],
): LiquidRegionFixture {
  if (chunks.length === 0) {
    throw new Error("buildLiquidRegionFixture requires at least one decoded chunk");
  }

  const dataVersion = chunks[0]!.dataVersion;
  for (const chunk of chunks) {
    if (chunk.dataVersion !== dataVersion) {
      throw new Error(
        `mixed data versions in liquid fixture: chunk (${chunk.chunkX}, ${chunk.chunkZ}) has ${chunk.dataVersion}, expected ${dataVersion}`,
      );
    }
    if (chunk.status !== "full") {
      throw new Error(`liquid fixture chunk (${chunk.chunkX}, ${chunk.chunkZ}) must be full, got ${chunk.status}`);
    }
  }

  const chunkByKey = new Map<string, DecodedChunk>();
  for (const chunk of chunks) {
    chunkByKey.set(chunkKey(chunk.chunkX, chunk.chunkZ), chunk);
  }

  const palette: BlockStateFixtureEntry[] = [];
  const paletteIndexByKey = new Map<string, number>();
  const blocks: number[] = [];

  for (let yOffset = 0; yOffset < scenario.bounds.sizeY; yOffset++) {
    const y = scenario.bounds.minY + yOffset;
    for (let zOffset = 0; zOffset < scenario.bounds.sizeZ; zOffset++) {
      const z = scenario.bounds.minZ + zOffset;
      for (let xOffset = 0; xOffset < scenario.bounds.sizeX; xOffset++) {
        const x = scenario.bounds.minX + xOffset;
        const state = getBlockState(chunks, chunkByKey, x, y, z);
        const key = blockStateKey(state);
        let paletteIndex = paletteIndexByKey.get(key);
        if (paletteIndex === undefined) {
          paletteIndex = palette.length;
          palette.push(state);
          paletteIndexByKey.set(key, paletteIndex);
        }
        blocks.push(paletteIndex);
      }
    }
  }

  return {
    module: "liquid-sim",
    minecraftVersion: scenario.minecraftVersion,
    dataVersion,
    seed: scenario.seed,
    generator: scenario.generator,
    generateStructures: scenario.generateStructures,
    scenario: scenario.scenario,
    ticks: scenario.ticks,
    bounds: scenario.bounds,
    wireFormat: {
      blockOrder: "y-major,z-major,x-minor",
      paletteEntries: "block-state",
    },
    palette,
    blocks,
    liquidTicks: normalizeLiquidTicks(
      chunkTicks.flatMap((entry) => entry.ticks).filter((tick) => isTickInBounds(tick, scenario.bounds, scenario.tickMargin)),
    ),
  };
}

export function decodeChunkLiquidTicks(root: NbtCompound): ScheduledLiquidTickFixture[] {
  const level = asCompound(root["Level"], "Level");
  const raw = level["LiquidTicks"];
  if (raw === undefined) {
    return [];
  }

  const list = asList(raw, "Level.LiquidTicks");
  if (list.values.length === 0) {
    return [];
  }
  if (list.type !== NBT_TAG_COMPOUND) {
    throw new Error(`Level.LiquidTicks must be a compound list, got element type ${list.type}`);
  }

  return list.values.map((entry, index) => {
    const tick = asCompound(entry, `Level.LiquidTicks[${index}]`);
    return {
      x: asInt(tick.x, `Level.LiquidTicks[${index}].x`),
      y: asInt(tick.y, `Level.LiquidTicks[${index}].y`),
      z: asInt(tick.z, `Level.LiquidTicks[${index}].z`),
      target: asString(tick.i, `Level.LiquidTicks[${index}].i`),
      delay: asInt(tick.t, `Level.LiquidTicks[${index}].t`),
      priority: tick.p === undefined ? "normal" : tickPriorityName(asInt(tick.p, `Level.LiquidTicks[${index}].p`)),
    };
  });
}

export function compareLiquidRegion(expected: LiquidRegionFixture, actual: LiquidRegionFixture): LiquidRegionDiff[] {
  const diffs: LiquidRegionDiff[] = [];
  if (JSON.stringify(expected.bounds) !== JSON.stringify(actual.bounds)) {
    diffs.push({ kind: "bounds_mismatch", expected: expected.bounds, actual: actual.bounds });
    return diffs;
  }

  const expectedBlocks = decodeFixtureBlocks(expected, "expected");
  const actualBlocks = decodeFixtureBlocks(actual, "actual");
  if (expectedBlocks.length !== actualBlocks.length) {
    diffs.push({ kind: "palette_mismatch", expected: expectedBlocks.length, actual: actualBlocks.length });
    return diffs;
  }

  for (let index = 0; index < expectedBlocks.length; index++) {
    if (expectedBlocks[index] !== actualBlocks[index]) {
      const pos = positionForIndex(expected.bounds, index);
      diffs.push({
        kind: "block_mismatch",
        ...pos,
        expected: expectedBlocks[index],
        actual: actualBlocks[index],
      });
      break;
    }
  }

  diffs.push(...compareLiquidTicks(expected.liquidTicks, actual.liquidTicks));
  return diffs;
}

export function blockStateKey(state: BlockStateFixtureEntry): string {
  const properties = Object.entries(state.properties ?? {}).sort(([left], [right]) => left.localeCompare(right));
  if (properties.length === 0) {
    return state.name;
  }
  return `${state.name}[${properties.map(([key, value]) => `${key}=${value}`).join(",")}]`;
}

export function normalizeLiquidTicks(ticks: readonly ScheduledLiquidTickFixture[]): ScheduledLiquidTickFixture[] {
  return [...ticks].sort((left, right) =>
    (left.delay - right.delay)
    || (priorityValue(left.priority) - priorityValue(right.priority))
    || left.target.localeCompare(right.target)
    || (left.x - right.x)
    || (left.y - right.y)
    || (left.z - right.z)
  );
}

function compareLiquidTicks(
  expectedTicks: readonly ScheduledLiquidTickFixture[],
  actualTicks: readonly ScheduledLiquidTickFixture[],
): LiquidRegionDiff[] {
  const expected = normalizeLiquidTicks(expectedTicks).map(liquidTickKey);
  const actual = normalizeLiquidTicks(actualTicks).map(liquidTickKey);
  const max = Math.max(expected.length, actual.length);
  const diffs: LiquidRegionDiff[] = [];
  for (let index = 0; index < max; index++) {
    const expectedTick = expected[index];
    const actualTick = actual[index];
    if (expectedTick === actualTick) {
      continue;
    }
    if (expectedTick === undefined) {
      diffs.push({ kind: "extra_tick", actual: actualTick });
    } else if (actualTick === undefined) {
      diffs.push({ kind: "missing_tick", expected: expectedTick });
    } else {
      diffs.push({ kind: "missing_tick", expected: expectedTick, actual: actualTick });
    }
    break;
  }
  return diffs;
}

function getBlockState(
  chunks: readonly DecodedChunk[],
  chunkByKey: Map<string, DecodedChunk>,
  x: number,
  y: number,
  z: number,
): BlockStateFixtureEntry {
  const chunkX = Math.floor(x / 16);
  const chunkZ = Math.floor(z / 16);
  const chunk = chunkByKey.get(chunkKey(chunkX, chunkZ));
  if (chunk === undefined) {
    throw new Error(`missing decoded chunk (${chunkX}, ${chunkZ}) for block (${x}, ${y}, ${z}); loaded ${chunks.map((entry) => `(${entry.chunkX},${entry.chunkZ})`).join(", ")}`);
  }

  const sectionY = Math.floor(y / 16);
  const section = chunk.sections.find((entry) => entry.y === sectionY);
  if (section === undefined) {
    return AIR_STATE;
  }
  if (section.blocks.length !== BLOCKS_PER_SECTION) {
    throw new Error(`chunk (${chunkX}, ${chunkZ}) section ${sectionY} has ${section.blocks.length} blocks`);
  }

  const localX = x & 15;
  const localY = y & 15;
  const localZ = z & 15;
  const index = (localY << 8) | (localZ << 4) | localX;
  const paletteIndex = section.blocks[index]!;
  const state = section.palette[paletteIndex];
  if (state === undefined) {
    throw new Error(`palette index ${paletteIndex} out of range in chunk (${chunkX}, ${chunkZ}) section ${sectionY}`);
  }
  return cloneBlockStateEntry(state);
}

function cloneBlockStateEntry(state: DecodedPaletteEntry): BlockStateFixtureEntry {
  if (state.properties === undefined) {
    return { name: state.name };
  }
  return {
    name: state.name,
    properties: Object.fromEntries(Object.entries(state.properties).sort(([left], [right]) => left.localeCompare(right))),
  };
}

function decodeFixtureBlocks(fixture: LiquidRegionFixture, label: string): string[] {
  const expectedLength = fixture.bounds.sizeX * fixture.bounds.sizeY * fixture.bounds.sizeZ;
  if (fixture.blocks.length !== expectedLength) {
    throw new Error(`${label} liquid fixture has ${fixture.blocks.length} blocks, expected ${expectedLength}`);
  }
  return fixture.blocks.map((paletteIndex, index) => {
    const state = fixture.palette[paletteIndex];
    if (state === undefined) {
      throw new Error(`${label} liquid fixture block ${index} has palette index ${paletteIndex} out of range`);
    }
    return blockStateKey(state);
  });
}

function isTickInBounds(tick: ScheduledLiquidTickFixture, bounds: RegionBounds, margin: number): boolean {
  return tick.x >= bounds.minX - margin
    && tick.x <= boundsMaxX(bounds) + margin
    && tick.y >= bounds.minY - margin
    && tick.y <= boundsMaxY(bounds) + margin
    && tick.z >= bounds.minZ - margin
    && tick.z <= boundsMaxZ(bounds) + margin;
}

function positionForIndex(bounds: RegionBounds, index: number): { readonly x: number; readonly y: number; readonly z: number } {
  const x = bounds.minX + (index % bounds.sizeX);
  const zOffset = Math.floor(index / bounds.sizeX) % bounds.sizeZ;
  const yOffset = Math.floor(index / (bounds.sizeX * bounds.sizeZ));
  return {
    x,
    y: bounds.minY + yOffset,
    z: bounds.minZ + zOffset,
  };
}

function liquidTickKey(tick: ScheduledLiquidTickFixture): string {
  return `${tick.delay}|${priorityValue(tick.priority)}|${tick.target}|${tick.x},${tick.y},${tick.z}`;
}

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

function tickPriorityName(value: number): LiquidTickPriority {
  switch (value) {
    case -3:
      return "extremely_high";
    case -2:
      return "very_high";
    case -1:
      return "high";
    case 0:
      return "normal";
    case 1:
      return "low";
    case 2:
      return "very_low";
    case 3:
      return "extremely_low";
    default:
      return value < -3 ? "extremely_high" : "extremely_low";
  }
}

function priorityValue(priority: LiquidTickPriority): number {
  switch (priority) {
    case "extremely_high":
      return -3;
    case "very_high":
      return -2;
    case "high":
      return -1;
    case "normal":
      return 0;
    case "low":
      return 1;
    case "very_low":
      return 2;
    case "extremely_low":
      return 3;
  }
}

function asCompound(value: NbtValue | undefined, path: string): NbtCompound {
  if (value === undefined) {
    throw new Error(`missing required compound tag ${path}`);
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`expected compound at ${path}`);
  }
  if (value instanceof Int8Array || value instanceof Int32Array || value instanceof BigInt64Array) {
    throw new Error(`expected compound at ${path}, got array`);
  }
  if ("type" in value && "values" in value) {
    throw new Error(`expected compound at ${path}, got list`);
  }
  return value as NbtCompound;
}

function asList(value: NbtValue, path: string): NbtList {
  if (
    typeof value !== "object" ||
    value === null ||
    value instanceof Int8Array ||
    value instanceof Int32Array ||
    value instanceof BigInt64Array
  ) {
    throw new Error(`expected list at ${path}`);
  }
  if (!("type" in value) || !("values" in value)) {
    throw new Error(`expected list at ${path}`);
  }
  return value as NbtList;
}

function asInt(value: NbtValue | undefined, path: string): number {
  if (typeof value !== "number") {
    throw new Error(`expected integer at ${path}, got ${typeof value}`);
  }
  return value | 0;
}

function asString(value: NbtValue | undefined, path: string): string {
  if (typeof value !== "string") {
    throw new Error(`expected string at ${path}, got ${typeof value}`);
  }
  return value;
}
