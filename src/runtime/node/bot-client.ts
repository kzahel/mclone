import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";
import { BotRuntime, GoodViewBotController, IdleBotController, WalkToPointBotController, WanderBotController, createBotClientRuntime, type BotController, type BotLogger } from "../bot";
import type { OpenWorldPreset } from "../protocol/world-messages";
import { RemoteWorldWebSocketTransport } from "../transport/remote-world-transport";

type BotClientGoal = "idle" | "wander" | "good-view" | "walk-to-point";

interface BotClientConfig {
  readonly url: string;
  readonly name: string;
  readonly seed: string;
  readonly preset: OpenWorldPreset;
  readonly radius: number;
  readonly tickRate: number;
  readonly goal: BotClientGoal;
  readonly maxTicks?: number;
}

interface MutableBotClientConfig {
  url?: string;
  name?: string;
  seed?: string;
  preset?: OpenWorldPreset;
  radius?: number;
  tickRate?: number;
  goal?: BotClientGoal;
  maxTicks?: number;
}

const DEFAULT_CONFIG: BotClientConfig = {
  url: "ws://127.0.0.1:4173/api/world/socket",
  name: "Bot",
  seed: "12345",
  preset: "default",
  radius: 2,
  tickRate: 20,
  goal: "wander",
};

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

function parsePositiveIntegerOption(name: string, value: string): number {
  const parsed = parseIntegerOption(name, value);
  if (parsed <= 0) {
    throw new Error(`Expected ${name} to be positive, got ${value}`);
  }
  return parsed;
}

function parseNonNegativeIntegerOption(name: string, value: string): number {
  const parsed = parseIntegerOption(name, value);
  if (parsed < 0) {
    throw new Error(`Expected ${name} to be non-negative, got ${value}`);
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

function parseGoal(value: string): BotClientGoal {
  if (value === "idle" || value === "wander" || value === "good-view" || value === "walk-to-point") {
    return value;
  }

  throw new Error(`Unsupported bot goal ${value}`);
}

function parseControllerAlias(value: string): BotClientGoal {
  if (value === "idle" || value === "wander") {
    return value;
  }

  throw new Error(`Unsupported bot controller ${value}; use --goal good-view for goal policies`);
}

export function parseBotClientConfig(argv: readonly string[]): BotClientConfig {
  const parsed: MutableBotClientConfig = {};

  for (let index = 0; index < argv.length; index++) {
    const option = argv[index];
    if (option === undefined) {
      continue;
    }

    if (option === "--") {
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
      case "--url":
        parsed.url = value;
        break;
      case "--name":
        parsed.name = value.trim();
        break;
      case "--seed":
        parsed.seed = parseSeed(value);
        break;
      case "--preset":
        parsed.preset = parsePreset(value);
        break;
      case "--radius":
        parsed.radius = parseNonNegativeIntegerOption("radius", value);
        break;
      case "--tick-rate":
        parsed.tickRate = parsePositiveIntegerOption("tick-rate", value);
        break;
      case "--goal":
        parsed.goal = parseGoal(value);
        break;
      case "--controller":
        parsed.goal = parseControllerAlias(value);
        break;
      case "--max-ticks":
        parsed.maxTicks = parseNonNegativeIntegerOption("max-ticks", value);
        break;
      default:
        throw new Error(`Unknown option ${option}`);
    }

    index++;
  }

  return {
    ...DEFAULT_CONFIG,
    ...parsed,
    name: parsed.name === undefined || parsed.name.length === 0 ? DEFAULT_CONFIG.name : parsed.name,
  };
}

function createController(name: BotClientConfig["goal"]): BotController {
  switch (name) {
    case "idle":
      return new IdleBotController();
    case "wander":
      return new WanderBotController();
    case "good-view":
      return new GoodViewBotController();
    case "walk-to-point":
      return new WalkToPointBotController();
  }
}

export async function runBotClient(config: BotClientConfig): Promise<void> {
  const logger: BotLogger = {
    info(message) {
      process.stdout.write(`${message}\n`);
    },
    error(message) {
      process.stderr.write(`${message}\n`);
    },
  };
  const transport = new RemoteWorldWebSocketTransport(config.url);
  const clientRuntime = createBotClientRuntime({
    transport,
    seed: BigInt(config.seed),
    pollUpdateMaxMessages: 16,
    worldProgressSink(progress) {
      if (progress.total > 0 && progress.current >= progress.total) {
        logger.info(`bot ${config.name} ${progress.stage} ${progress.current.toString()}/${progress.total.toString()}`);
      }
    },
  });
  const bot = new BotRuntime({
    clientRuntime,
    openWorldRequest: {
      type: "open_world",
      seed: BigInt(config.seed),
      preset: config.preset,
      playerProfile: { name: config.name },
    },
    viewRadius: config.radius,
    controller: createController(config.goal),
    logger,
  });
  const abortController = new AbortController();
  const stop = (): void => {
    abortController.abort();
  };
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);

  try {
    await bot.open();
    logger.info(`bot ${config.name} running goal=${config.goal} radius=${config.radius.toString()} tickRate=${config.tickRate.toString()}`);
    await bot.runUntilStopped({
      tickIntervalMs: Math.max(1, Math.round(1000 / config.tickRate)),
      maxTicks: config.maxTicks,
      signal: abortController.signal,
    });
  } finally {
    process.off("SIGINT", stop);
    process.off("SIGTERM", stop);
    await bot.close();
  }
}

export async function main(argv: readonly string[] = process.argv.slice(2)): Promise<void> {
  await runBotClient(parseBotClientConfig(argv));
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
