import { readFileSync } from "node:fs";
import { resolve } from "node:path";

export interface RegionBounds {
  readonly minX: number;
  readonly minY: number;
  readonly minZ: number;
  readonly sizeX: number;
  readonly sizeY: number;
  readonly sizeZ: number;
}

export interface LiquidScenarioSpec {
  readonly scenario: string;
  readonly seed: string;
  readonly ticks: number;
  readonly bounds: RegionBounds;
  readonly commands: readonly string[];
  readonly tickMargin: number;
  readonly minecraftVersion: string;
  readonly generator: string;
  readonly generateStructures: boolean;
}

interface RawLiquidScenarioSpec {
  readonly scenario?: unknown;
  readonly seed?: unknown;
  readonly ticks?: unknown;
  readonly bounds?: unknown;
  readonly commands?: unknown;
  readonly tickMargin?: unknown;
  readonly minecraftVersion?: unknown;
  readonly generator?: unknown;
  readonly generateStructures?: unknown;
}

export function readLiquidScenario(path: string): LiquidScenarioSpec {
  const raw = JSON.parse(readFileSync(resolve(path), "utf8")) as RawLiquidScenarioSpec;
  return normalizeLiquidScenario(raw, path);
}

export function normalizeLiquidScenario(raw: RawLiquidScenarioSpec, path = "liquid scenario"): LiquidScenarioSpec {
  const scenario = requireString(raw.scenario, `${path}.scenario`);
  const seed = requireString(raw.seed, `${path}.seed`);
  const ticks = requireNonNegativeInteger(raw.ticks, `${path}.ticks`);
  const bounds = normalizeBounds(raw.bounds, `${path}.bounds`);
  const commands = normalizeCommands(raw.commands, `${path}.commands`);
  const tickMargin = raw.tickMargin === undefined ? 1 : requireNonNegativeInteger(raw.tickMargin, `${path}.tickMargin`);
  const minecraftVersion = raw.minecraftVersion === undefined ? "1.17.1" : requireString(raw.minecraftVersion, `${path}.minecraftVersion`);
  const generator = raw.generator === undefined ? "default" : requireString(raw.generator, `${path}.generator`);
  const generateStructures = raw.generateStructures === undefined
    ? false
    : requireBoolean(raw.generateStructures, `${path}.generateStructures`);

  return {
    scenario,
    seed,
    ticks,
    bounds,
    commands,
    tickMargin,
    minecraftVersion,
    generator,
    generateStructures,
  };
}

export function boundsMaxX(bounds: RegionBounds): number {
  return bounds.minX + bounds.sizeX - 1;
}

export function boundsMaxY(bounds: RegionBounds): number {
  return bounds.minY + bounds.sizeY - 1;
}

export function boundsMaxZ(bounds: RegionBounds): number {
  return bounds.minZ + bounds.sizeZ - 1;
}

function normalizeBounds(raw: unknown, path: string): RegionBounds {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    throw new Error(`${path} must be an object`);
  }
  const record = raw as Record<string, unknown>;
  const bounds = {
    minX: requireInteger(record.minX, `${path}.minX`),
    minY: requireInteger(record.minY, `${path}.minY`),
    minZ: requireInteger(record.minZ, `${path}.minZ`),
    sizeX: requirePositiveInteger(record.sizeX, `${path}.sizeX`),
    sizeY: requirePositiveInteger(record.sizeY, `${path}.sizeY`),
    sizeZ: requirePositiveInteger(record.sizeZ, `${path}.sizeZ`),
  };

  if (boundsMaxX(bounds) < bounds.minX || boundsMaxY(bounds) < bounds.minY || boundsMaxZ(bounds) < bounds.minZ) {
    throw new Error(`${path} overflows numeric bounds`);
  }

  return bounds;
}

function normalizeCommands(raw: unknown, path: string): string[] {
  if (!Array.isArray(raw)) {
    throw new Error(`${path} must be an array`);
  }
  return raw.map((entry, index) => {
    const command = requireString(entry, `${path}[${index}]`);
    if (command.includes("\n") || command.includes("\r")) {
      throw new Error(`${path}[${index}] must be a single command line`);
    }
    if (command.startsWith("/")) {
      throw new Error(`${path}[${index}] should omit the leading slash for mcfunction output`);
    }
    return command;
  });
}

function requireString(value: unknown, path: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${path} must be a non-empty string`);
  }
  return value;
}

function requireBoolean(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") {
    throw new Error(`${path} must be a boolean`);
  }
  return value;
}

function requireInteger(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isInteger(value)) {
    throw new Error(`${path} must be an integer`);
  }
  return value;
}

function requirePositiveInteger(value: unknown, path: string): number {
  const parsed = requireInteger(value, path);
  if (parsed <= 0) {
    throw new Error(`${path} must be positive`);
  }
  return parsed;
}
function requireNonNegativeInteger(value: unknown, path: string): number {
  const parsed = requireInteger(value, path);
  if (parsed < 0) {
    throw new Error(`${path} must be non-negative`);
  }
  return parsed;
}
