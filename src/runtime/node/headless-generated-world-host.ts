import { readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";
import { createGeneratedWorldHostForRequest } from "../host/generated-world-host-factory";
import { createNodeLightingService } from "../lighting/node-lighting-worker-client";
import type {
  OpenWorldPreset,
  SetChunkViewRequest,
  WorldHostMessage,
  WorldOpenedMessage,
} from "../protocol/world-messages";
import { FileWorldStorage, getFileWorldSaveDirectory } from "../storage/file-world-storage";
import type { WorldStorage } from "../storage/world-storage";

const DEFAULT_CHUNK_VIEW = {
  centerChunkX: 0,
  centerChunkZ: 0,
  radius: 1,
} as const;

export interface HeadlessChunkViewConfig {
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly radius: number;
}

export interface HeadlessGeneratedWorldHostConfig {
  readonly seed: string;
  readonly preset: OpenWorldPreset;
  readonly saveRoot: string;
  readonly chunkViews: readonly HeadlessChunkViewConfig[];
}

export interface HeadlessGeneratedWorldHostViewResult {
  readonly request: SetChunkViewRequest;
  readonly chunkChanged: boolean;
  readonly snapshotCount: number;
  readonly unloadCount: number;
}

export interface HeadlessGeneratedWorldHostResult {
  readonly saveId: string;
  readonly saveDirectory: string;
  readonly opened: WorldOpenedMessage;
  readonly viewResults: readonly HeadlessGeneratedWorldHostViewResult[];
}

export interface RunHeadlessGeneratedWorldHostOptions {
  readonly worldStorage?: WorldStorage;
}

interface MutableHeadlessGeneratedWorldHostConfig {
  seed?: string;
  preset?: OpenWorldPreset;
  saveRoot?: string;
  chunkViews?: readonly HeadlessChunkViewConfig[];
}

interface ParsedCliOverrides {
  configPath?: string;
  seed?: string;
  preset?: OpenWorldPreset;
  saveRoot?: string;
  centerChunkX?: number;
  centerChunkZ?: number;
  radius?: number;
}

function formatUnknownError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function parseIntegerOption(name: string, value: string): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed)) {
    throw new Error(`Expected ${name} to be a safe integer, got ${value}`);
  }

  return parsed;
}

function parseSeed(seed: string): string {
  try {
    BigInt(seed);
    return seed;
  } catch {
    throw new Error(`Expected seed to be a bigint-compatible integer string, got ${seed}`);
  }
}

function parsePreset(preset: string): OpenWorldPreset {
  if (preset === "default" || preset === "browser_smoke" || preset === "flat_grass" || preset === "small_island") {
    return preset;
  }

  throw new Error(`Unsupported preset ${preset}`);
}

function isSafeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value);
}

function parseChunkView(value: unknown, index: number): HeadlessChunkViewConfig {
  if (typeof value !== "object" || value === null) {
    throw new Error(`chunkViews[${index.toString()}] must be an object`);
  }

  const record = value as Record<string, unknown>;
  const centerChunkX = record.centerChunkX;
  const centerChunkZ = record.centerChunkZ;
  const radius = record.radius;

  if (!isSafeInteger(centerChunkX) || !isSafeInteger(centerChunkZ) || !isSafeInteger(radius)) {
    throw new Error(`chunkViews[${index.toString()}] must contain integer centerChunkX, centerChunkZ, and radius values`);
  }

  if (radius < 0) {
    throw new Error(`chunkViews[${index.toString()}].radius must be non-negative`);
  }

  return {
    centerChunkX,
    centerChunkZ,
    radius,
  };
}

async function readConfigFile(configPath: string): Promise<MutableHeadlessGeneratedWorldHostConfig> {
  const filePath = path.resolve(configPath);
  const value = JSON.parse(await readFile(filePath, "utf8")) as unknown;
  if (typeof value !== "object" || value === null) {
    throw new Error(`Headless host config ${filePath} must contain an object`);
  }

  const record = value as Record<string, unknown>;
  const parsed: MutableHeadlessGeneratedWorldHostConfig = {};

  if (record.seed !== undefined) {
    if (typeof record.seed !== "string") {
      throw new Error(`Headless host config ${filePath} field seed must be a string`);
    }

    parsed.seed = parseSeed(record.seed);
  }

  if (record.preset !== undefined) {
    if (typeof record.preset !== "string") {
      throw new Error(`Headless host config ${filePath} field preset must be a string`);
    }

    parsed.preset = parsePreset(record.preset);
  }

  if (record.saveRoot !== undefined) {
    if (typeof record.saveRoot !== "string") {
      throw new Error(`Headless host config ${filePath} field saveRoot must be a string`);
    }

    parsed.saveRoot = path.isAbsolute(record.saveRoot)
      ? record.saveRoot
      : path.resolve(path.dirname(filePath), record.saveRoot);
  }

  if (record.chunkViews !== undefined) {
    if (!Array.isArray(record.chunkViews)) {
      throw new Error(`Headless host config ${filePath} field chunkViews must be an array`);
    }

    parsed.chunkViews = record.chunkViews.map((chunkView, index) => parseChunkView(chunkView, index));
  }

  return parsed;
}

function parseCliOverrides(argv: readonly string[]): ParsedCliOverrides {
  const parsed: ParsedCliOverrides = {};

  for (let index = 0; index < argv.length; index++) {
    const option = argv[index];
    if (option === undefined) {
      continue;
    }

    const value = argv[index + 1];
    if (!option.startsWith("--")) {
      throw new Error(`Unexpected argument ${option}`);
    }

    if (value === undefined) {
      throw new Error(`Missing value for ${option}`);
    }

    switch (option) {
      case "--config":
        parsed.configPath = value;
        break;
      case "--seed":
        parsed.seed = parseSeed(value);
        break;
      case "--preset":
        parsed.preset = parsePreset(value);
        break;
      case "--save-root":
        parsed.saveRoot = path.resolve(value);
        break;
      case "--center-chunk-x":
        parsed.centerChunkX = parseIntegerOption("centerChunkX", value);
        break;
      case "--center-chunk-z":
        parsed.centerChunkZ = parseIntegerOption("centerChunkZ", value);
        break;
      case "--radius":
        parsed.radius = parseIntegerOption("radius", value);
        if (parsed.radius < 0) {
          throw new Error("radius must be non-negative");
        }

        break;
      default:
        throw new Error(`Unknown option ${option}`);
    }

    index++;
  }

  return parsed;
}

async function loadHeadlessHostConfig(argv: readonly string[]): Promise<HeadlessGeneratedWorldHostConfig> {
  const overrides = parseCliOverrides(argv);
  const fileConfig = overrides.configPath === undefined ? {} : await readConfigFile(overrides.configPath);
  const defaultSaveRoot = path.resolve(tmpdir(), "mclone-node-worlds");
  const baseView = fileConfig.chunkViews?.[0] ?? DEFAULT_CHUNK_VIEW;
  const hasCliViewOverride = overrides.centerChunkX !== undefined || overrides.centerChunkZ !== undefined || overrides.radius !== undefined;

  return {
    seed: overrides.seed ?? fileConfig.seed ?? "12345",
    preset: overrides.preset ?? fileConfig.preset ?? "default",
    saveRoot: overrides.saveRoot ?? fileConfig.saveRoot ?? defaultSaveRoot,
    chunkViews: hasCliViewOverride
      ? [{
          centerChunkX: overrides.centerChunkX ?? baseView.centerChunkX,
          centerChunkZ: overrides.centerChunkZ ?? baseView.centerChunkZ,
          radius: overrides.radius ?? baseView.radius,
        }]
      : (fileConfig.chunkViews ?? [DEFAULT_CHUNK_VIEW]),
  };
}

function expectWorldOpened(messages: readonly WorldHostMessage[]): WorldOpenedMessage {
  let opened: WorldOpenedMessage | undefined;

  for (const message of messages) {
    switch (message.type) {
      case "world_opened":
        opened = message;
        break;
      case "world_error":
        throw new Error(message.message);
      case "chunk_snapshot":
      case "chunk_light_delta":
      case "chunk_unload":
      case "session_state":
      case "player_state":
      case "world_progress":
      case "world_perf":
        break;
    }
  }

  if (opened === undefined) {
    throw new Error("Headless host did not return world_opened");
  }

  return opened;
}

function summarizeChunkViewMessages(
  request: SetChunkViewRequest,
  messages: readonly WorldHostMessage[],
): HeadlessGeneratedWorldHostViewResult {
  let snapshotCount = 0;
  let unloadCount = 0;

  for (const message of messages) {
    switch (message.type) {
      case "chunk_snapshot":
        snapshotCount++;
        break;
      case "chunk_light_delta":
        break;
      case "chunk_unload":
        unloadCount++;
        break;
      case "world_error":
        throw new Error(message.message);
      case "world_opened":
      case "session_state":
      case "player_state":
      case "world_progress":
      case "world_perf":
        break;
    }
  }

  return {
    request,
    chunkChanged: snapshotCount > 0 || unloadCount > 0,
    snapshotCount,
    unloadCount,
  };
}

export async function runHeadlessGeneratedWorldHost(
  config: HeadlessGeneratedWorldHostConfig,
  options: RunHeadlessGeneratedWorldHostOptions = {},
): Promise<HeadlessGeneratedWorldHostResult> {
  const openWorldRequest = {
    type: "open_world",
    seed: BigInt(config.seed),
    preset: config.preset,
  } as const;
  const worldStorage = options.worldStorage ?? new FileWorldStorage(config.saveRoot);
  const host = createGeneratedWorldHostForRequest(openWorldRequest, {
    worldStorage,
    lightingService: createNodeLightingService(),
  });
  try {
    const opened = expectWorldOpened(await host.openWorld(openWorldRequest));
    const viewResults: HeadlessGeneratedWorldHostViewResult[] = [];

    for (const chunkView of config.chunkViews) {
      const request = {
        type: "set_chunk_view",
        centerChunkX: chunkView.centerChunkX,
        centerChunkZ: chunkView.centerChunkZ,
        radius: chunkView.radius,
      } as const;
      viewResults.push(summarizeChunkViewMessages(request, await host.setChunkView(request)));
    }

    return {
      saveId: opened.saveMetadata.saveId,
      saveDirectory: getFileWorldSaveDirectory(config.saveRoot, opened.saveMetadata.saveId),
      opened,
      viewResults,
    };
  } finally {
    host.close();
  }
}

export async function main(argv: readonly string[] = process.argv.slice(2)): Promise<void> {
  const config = await loadHeadlessHostConfig(argv);
  const result = await runHeadlessGeneratedWorldHost(config);
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
}

function isDirectExecution(): boolean {
  return process.argv[1] !== undefined && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
}

if (isDirectExecution()) {
  void main().catch((error) => {
    process.stderr.write(`${formatUnknownError(error)}\n`);
    process.exitCode = 1;
  });
}
