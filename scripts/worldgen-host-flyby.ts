import { performance } from "node:perf_hooks";
import process from "node:process";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { createGeneratedWorldHostForRequest } from "../src/runtime/host/generated-world-host-factory";
import { getGeneratedWorldViewChunkRadius, type GeneratedWorldHost } from "../src/runtime/host/generated-world-host";
import { createNodeLightingService } from "../src/runtime/lighting/node-lighting-worker-client";
import type {
  OpenWorldPreset,
  OpenWorldRequest,
  PlayerStateMessage,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldEngineLightingMode,
  WorldEngineLiquidSimulationMode,
  WorldHostMessage,
  WorldPerformanceSnapshot,
  WorldProgressMessage,
  WorldgenPerformanceCounters,
} from "../src/runtime/protocol/world-messages";

type CommandName = "open_world" | "set_chunk_view" | "set_player_input" | "poll_world_updates";

interface CliOptions {
  readonly seed: bigint;
  readonly preset: OpenWorldPreset;
  readonly lightingMode: WorldEngineLightingMode;
  readonly liquidSimulationMode: WorldEngineLiquidSimulationMode;
  readonly radius: number;
  readonly startChunkX: number;
  readonly startChunkZ: number;
  readonly steps: number;
  readonly speedBlocksPerStep: number;
  readonly pollMs: number;
  readonly maxMessages: number;
  readonly cooldownMs: number;
  readonly timeoutMs: number;
  readonly outputPath?: string;
  readonly json: boolean;
}

interface TimedCommandRecord {
  readonly command: CommandName;
  readonly durationMs: number;
  readonly messageCount: number;
  readonly chunkSnapshots: number;
  readonly playerStates: number;
}

interface NumericSummary {
  readonly count: number;
  readonly min: number;
  readonly p50: number;
  readonly p95: number;
  readonly p99: number;
  readonly max: number;
  readonly average: number;
}

interface FlybyReport {
  readonly seed: string;
  readonly preset: OpenWorldPreset;
  readonly lightingMode: WorldEngineLightingMode;
  readonly liquidSimulationMode: WorldEngineLiquidSimulationMode;
  readonly radius: number;
  readonly route: {
    readonly startChunkX: number;
    readonly startChunkZ: number;
    readonly finalChunkX: number;
    readonly finalChunkZ: number;
    readonly steps: number;
    readonly speedBlocksPerStep: number;
    readonly crossedChunkBoundaries: number;
  };
  readonly durationMs: number;
  readonly warmupMs: number;
  readonly cooldownMs: number;
  readonly commands: Readonly<Record<CommandName, NumericSummary>>;
  readonly chunkSnapshotsPerPoll: NumericSummary;
  readonly playerTickGapMs: NumericSummary;
  readonly totals: {
    readonly polls: number;
    readonly setChunkViews: number;
    readonly setPlayerInputs: number;
    readonly chunkSnapshots: number;
    readonly playerStates: number;
    readonly progressMessages: number;
  };
  readonly latestProgress: readonly WorldProgressMessage[];
  readonly worldgenStart?: WorldgenPerformanceCounters;
  readonly worldgenEnd?: WorldgenPerformanceCounters;
  readonly worldgenDelta?: WorldgenPerformanceCounters;
}

function parseInteger(name: string, value: string): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed)) {
    throw new Error(`Expected ${name} to be a safe integer, got ${value}`);
  }

  return parsed;
}

function parseFloatOption(name: string, value: string): number {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed)) {
    throw new Error(`Expected ${name} to be finite, got ${value}`);
  }

  return parsed;
}

function parsePreset(value: string): OpenWorldPreset {
  if (value === "default" || value === "browser_smoke" || value === "flat_grass" || value === "small_island") {
    return value;
  }

  throw new Error(`Unsupported preset ${value}`);
}

function parseCliOptions(argv: readonly string[]): CliOptions {
  let seed = 12345n;
  let preset: OpenWorldPreset = "default";
  let lightingMode: WorldEngineLightingMode = "none";
  let liquidSimulationMode: WorldEngineLiquidSimulationMode = "none";
  let radius = 1;
  let startChunkX = 0;
  let startChunkZ = 0;
  let steps = 96;
  let speedBlocksPerStep = 4;
  let pollMs = 10;
  let maxMessages = 512;
  let cooldownMs = 2_000;
  let timeoutMs = 120_000;
  let outputPath: string | undefined;
  let json = false;

  for (let index = 0; index < argv.length; index++) {
    const option = argv[index];
    if (option === undefined || option === "--") {
      continue;
    }
    if (option === "--json") {
      json = true;
      continue;
    }

    const value = argv[index + 1];
    if (value === undefined) {
      throw new Error(`Missing value for ${option}`);
    }

    switch (option) {
      case "--seed":
        seed = BigInt(value);
        break;
      case "--preset":
        preset = parsePreset(value);
        break;
      case "--lighting-mode":
        if (value !== "vanilla17" && value !== "none") {
          throw new Error(`Unsupported lighting mode ${value}`);
        }
        lightingMode = value;
        break;
      case "--liquid-simulation-mode":
        if (value !== "vanilla17" && value !== "none") {
          throw new Error(`Unsupported liquid simulation mode ${value}`);
        }
        liquidSimulationMode = value;
        break;
      case "--radius":
        radius = parseInteger("radius", value);
        break;
      case "--start-chunk-x":
        startChunkX = parseInteger("startChunkX", value);
        break;
      case "--start-chunk-z":
        startChunkZ = parseInteger("startChunkZ", value);
        break;
      case "--steps":
        steps = parseInteger("steps", value);
        break;
      case "--speed-blocks-per-step":
        speedBlocksPerStep = parseFloatOption("speedBlocksPerStep", value);
        break;
      case "--poll-ms":
        pollMs = parseFloatOption("pollMs", value);
        break;
      case "--max-messages":
        maxMessages = parseInteger("maxMessages", value);
        break;
      case "--cooldown-ms":
        cooldownMs = parseFloatOption("cooldownMs", value);
        break;
      case "--timeout-ms":
        timeoutMs = parseFloatOption("timeoutMs", value);
        break;
      case "--output":
        outputPath = value;
        break;
      default:
        throw new Error(`Unknown option ${option}`);
    }

    index++;
  }

  if (radius < 0 || steps < 0 || speedBlocksPerStep < 0 || pollMs < 0 || maxMessages <= 0 || cooldownMs < 0 || timeoutMs <= 0) {
    throw new Error("radius/steps/speed/poll/maxMessages/cooldown/timeout options must be non-negative, with maxMessages and timeout positive");
  }

  return {
    seed,
    preset,
    lightingMode,
    liquidSimulationMode,
    radius,
    startChunkX,
    startChunkZ,
    steps,
    speedBlocksPerStep,
    pollMs,
    maxMessages,
    cooldownMs,
    timeoutMs,
    ...(outputPath === undefined ? {} : { outputPath }),
    json,
  };
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function summarizeNumbers(values: readonly number[]): NumericSummary {
  if (values.length === 0) {
    return { count: 0, min: 0, p50: 0, p95: 0, p99: 0, max: 0, average: 0 };
  }

  const sorted = [...values].sort((a, b) => a - b);
  const percentile = (fraction: number): number => {
    const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil((sorted.length * fraction) - 1)));
    return sorted[index]!;
  };

  return {
    count: values.length,
    min: sorted[0]!,
    p50: percentile(0.50),
    p95: percentile(0.95),
    p99: percentile(0.99),
    max: sorted[sorted.length - 1]!,
    average: values.reduce((sum, value) => sum + value, 0) / values.length,
  };
}

function createOpenWorldRequest(options: CliOptions): OpenWorldRequest {
  return {
    type: "open_world",
    seed: options.seed,
    preset: options.preset,
    config: {
      lightingMode: options.lightingMode,
      liquidSimulationMode: options.liquidSimulationMode,
    },
  };
}

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

function finalRouteChunk(options: CliOptions): { readonly chunkX: number; readonly chunkZ: number } {
  const startBlockX = (options.startChunkX * 16) + 8;
  const finalBlockX = startBlockX + (options.steps * options.speedBlocksPerStep);
  return {
    chunkX: Math.floor(finalBlockX / 16),
    chunkZ: options.startChunkZ,
  };
}

function publishKeys(centerChunkX: number, centerChunkZ: number, radius: number): Set<string> {
  const viewRadius = getGeneratedWorldViewChunkRadius(radius);
  const keys = new Set<string>();
  for (let chunkZ = centerChunkZ - viewRadius; chunkZ <= centerChunkZ + viewRadius; chunkZ++) {
    for (let chunkX = centerChunkX - viewRadius; chunkX <= centerChunkX + viewRadius; chunkX++) {
      keys.add(chunkKey(chunkX, chunkZ));
    }
  }
  return keys;
}

function countSnapshots(messages: readonly WorldHostMessage[], published: Set<string>): number {
  let snapshots = 0;
  for (const message of messages) {
    if (message.type !== "chunk_snapshot") {
      continue;
    }
    snapshots++;
    published.add(chunkKey(message.snapshot.chunkX, message.snapshot.chunkZ));
  }

  return snapshots;
}

function countPlayerStates(messages: readonly WorldHostMessage[]): number {
  return messages.filter((message): message is PlayerStateMessage => message.type === "player_state").length;
}

function throwOnWorldError(messages: readonly WorldHostMessage[]): void {
  const error = messages.find((message) => message.type === "world_error");
  if (error !== undefined) {
    throw new Error(error.message);
  }
}

function collectLatestProgress(messages: readonly WorldHostMessage[], latestProgress: Map<string, WorldProgressMessage>): number {
  let count = 0;
  for (const message of messages) {
    if (message.type === "world_progress") {
      latestProgress.set(message.stage, message);
      count++;
    }
  }
  return count;
}

function collectLatestPerformance(
  messages: readonly WorldHostMessage[],
  current: WorldPerformanceSnapshot | undefined,
): WorldPerformanceSnapshot | undefined {
  let next = current;
  for (const message of messages) {
    if (message.type === "world_perf") {
      next = message.performance;
    }
  }
  return next;
}

function collectPlayerTickTimes(messages: readonly WorldHostMessage[], samples: Array<readonly [number, number]>): void {
  const nowMs = performance.now();
  for (const message of messages) {
    if (message.type === "player_state") {
      samples.push([nowMs, message.state.tick]);
    }
  }
}

function diffWorldgenCounters(
  start: WorldgenPerformanceCounters | undefined,
  end: WorldgenPerformanceCounters | undefined,
): WorldgenPerformanceCounters | undefined {
  if (end === undefined) {
    return undefined;
  }
  if (start === undefined) {
    return end;
  }

  const phaseNames = new Set([...Object.keys(start.phases), ...Object.keys(end.phases)]);
  const phases: Record<string, { count: number; totalMs: number; maxMs: number }> = {};
  for (const name of phaseNames) {
    const before = start.phases[name] ?? { count: 0, totalMs: 0, maxMs: 0 };
    const after = end.phases[name] ?? { count: 0, totalMs: 0, maxMs: 0 };
    phases[name] = {
      count: after.count - before.count,
      totalMs: after.totalMs - before.totalMs,
      maxMs: after.maxMs,
    };
  }

  const countNames = new Set([...Object.keys(start.counts), ...Object.keys(end.counts)]);
  const counts: Record<string, number> = {};
  for (const name of countNames) {
    counts[name] = (end.counts[name] ?? 0) - (start.counts[name] ?? 0);
  }

  return {
    phases,
    counts,
    maxPendingMessages: end.maxPendingMessages,
    currentPendingMessages: end.currentPendingMessages,
    ...(end.activeChunkViewJobRevision === undefined ? {} : { activeChunkViewJobRevision: end.activeChunkViewJobRevision }),
  };
}

async function timedCommand(
  records: TimedCommandRecord[],
  command: CommandName,
  published: Set<string>,
  run: () => Promise<readonly WorldHostMessage[]>,
): Promise<readonly WorldHostMessage[]> {
  const startedAtMs = performance.now();
  const messages = await run();
  const durationMs = performance.now() - startedAtMs;
  throwOnWorldError(messages);
  const chunkSnapshots = countSnapshots(messages, published);
  const playerStates = countPlayerStates(messages);
  records.push({
    command,
    durationMs,
    messageCount: messages.length,
    chunkSnapshots,
    playerStates,
  });
  return messages;
}

function createPlayerInput(sequence: number): SetPlayerInputRequest {
  return {
    type: "set_player_input",
    input: {
      sequence,
      moveX: 0,
      moveY: 0,
      moveZ: 1,
      yaw: 0,
      pitch: 0,
      commandQuantumUs: 50_000,
      stepCount: 1,
    },
  };
}

async function waitForPublishedKeys(
  host: GeneratedWorldHost,
  options: CliOptions,
  targetKeys: Set<string>,
  published: Set<string>,
  records: TimedCommandRecord[],
  latestProgress: Map<string, WorldProgressMessage>,
  playerTickSamples: Array<readonly [number, number]>,
): Promise<{ readonly elapsedMs: number; readonly polls: number; readonly progressMessages: number; readonly performance?: WorldPerformanceSnapshot }> {
  const startedAtMs = performance.now();
  const deadlineMs = startedAtMs + options.timeoutMs;
  let polls = 0;
  let progressMessages = 0;
  let latestPerformance: WorldPerformanceSnapshot | undefined;

  while ([...targetKeys].some((key) => !published.has(key))) {
    if (performance.now() > deadlineMs) {
      throw new Error(`Timed out waiting for ${targetKeys.size.toString()} publish keys; got ${[...targetKeys].filter((key) => published.has(key)).length.toString()}`);
    }

    await sleep(options.pollMs);
    const messages = await timedCommand(records, "poll_world_updates", published, () =>
      host.pollUpdates({ type: "poll_world_updates", maxMessages: options.maxMessages })
    );
    polls++;
    progressMessages += collectLatestProgress(messages, latestProgress);
    latestPerformance = collectLatestPerformance(messages, latestPerformance);
    collectPlayerTickTimes(messages, playerTickSamples);
  }

  return {
    elapsedMs: performance.now() - startedAtMs,
    polls,
    progressMessages,
    ...(latestPerformance === undefined ? {} : { performance: latestPerformance }),
  };
}

async function runFlyby(options: CliOptions): Promise<FlybyReport> {
  const request = createOpenWorldRequest(options);
  const lightingService = options.lightingMode === "vanilla17" ? createNodeLightingService() : undefined;
  const host = createGeneratedWorldHostForRequest(request, {
    chunkViewScheduling: "cooperative",
    lightingMode: options.lightingMode,
    liquidSimulationMode: options.liquidSimulationMode,
    lightingService,
  });
  const published = new Set<string>();
  const records: TimedCommandRecord[] = [];
  const latestProgress = new Map<string, WorldProgressMessage>();
  const playerTickSamples: Array<readonly [number, number]> = [];
  let latestPerformance: WorldPerformanceSnapshot | undefined;
  let worldgenStart: WorldgenPerformanceCounters | undefined;

  try {
    const startedAtMs = performance.now();
    let messages = await timedCommand(records, "open_world", published, () => host.openWorld(request));
    latestPerformance = collectLatestPerformance(messages, latestPerformance);
    collectPlayerTickTimes(messages, playerTickSamples);

    const initialView: SetChunkViewRequest = {
      type: "set_chunk_view",
      centerChunkX: options.startChunkX,
      centerChunkZ: options.startChunkZ,
      radius: options.radius,
    };
    messages = await timedCommand(records, "set_chunk_view", published, () => host.setChunkView(initialView));
    latestPerformance = collectLatestPerformance(messages, latestPerformance);
    collectPlayerTickTimes(messages, playerTickSamples);
    let progressMessages = collectLatestProgress(messages, latestProgress);
    const warmup = await waitForPublishedKeys(
      host,
      options,
      publishKeys(options.startChunkX, options.startChunkZ, options.radius),
      published,
      records,
      latestProgress,
      playerTickSamples,
    );
    progressMessages += warmup.progressMessages;
    latestPerformance = warmup.performance ?? latestPerformance;
    worldgenStart = latestPerformance?.worldgen;
    playerTickSamples.length = 0;

    let currentChunkX = options.startChunkX;
    let currentChunkZ = options.startChunkZ;
    let setChunkViews = 1;
    let setPlayerInputs = 0;
    let polls = warmup.polls;
    let sequence = 1;
    let crossedChunkBoundaries = 0;
    const startBlockX = (options.startChunkX * 16) + 8;

    for (let step = 1; step <= options.steps; step++) {
      const blockX = startBlockX + (step * options.speedBlocksPerStep);
      const nextChunkX = Math.floor(blockX / 16);
      if (nextChunkX !== currentChunkX) {
        currentChunkX = nextChunkX;
        crossedChunkBoundaries++;
        messages = await timedCommand(records, "set_chunk_view", published, () =>
          host.setChunkView({
            type: "set_chunk_view",
            centerChunkX: currentChunkX,
            centerChunkZ: currentChunkZ,
            radius: options.radius,
          })
        );
        setChunkViews++;
        progressMessages += collectLatestProgress(messages, latestProgress);
        latestPerformance = collectLatestPerformance(messages, latestPerformance);
        collectPlayerTickTimes(messages, playerTickSamples);
      }

      messages = await timedCommand(records, "set_player_input", published, () => host.setPlayerInput(createPlayerInput(sequence++)));
      setPlayerInputs++;
      progressMessages += collectLatestProgress(messages, latestProgress);
      latestPerformance = collectLatestPerformance(messages, latestPerformance);
      collectPlayerTickTimes(messages, playerTickSamples);

      await sleep(options.pollMs);
      messages = await timedCommand(records, "poll_world_updates", published, () =>
        host.pollUpdates({ type: "poll_world_updates", maxMessages: options.maxMessages })
      );
      polls++;
      progressMessages += collectLatestProgress(messages, latestProgress);
      latestPerformance = collectLatestPerformance(messages, latestPerformance);
      collectPlayerTickTimes(messages, playerTickSamples);
    }

    const cooldownStartedAtMs = performance.now();
    const finalChunk = finalRouteChunk(options);
    while (
      performance.now() - cooldownStartedAtMs < options.cooldownMs
      || [...publishKeys(finalChunk.chunkX, finalChunk.chunkZ, options.radius)].some((key) => !published.has(key))
    ) {
      if (performance.now() - cooldownStartedAtMs > options.timeoutMs) {
        throw new Error("Timed out during fly-by cooldown");
      }

      await sleep(options.pollMs);
      messages = await timedCommand(records, "poll_world_updates", published, () =>
        host.pollUpdates({ type: "poll_world_updates", maxMessages: options.maxMessages })
      );
      polls++;
      progressMessages += collectLatestProgress(messages, latestProgress);
      latestPerformance = collectLatestPerformance(messages, latestPerformance);
      collectPlayerTickTimes(messages, playerTickSamples);
    }

    const durationMs = performance.now() - startedAtMs;
    const commandSummaries = Object.fromEntries(
      (["open_world", "set_chunk_view", "set_player_input", "poll_world_updates"] as const).map((command) => [
        command,
        summarizeNumbers(records.filter((record) => record.command === command).map((record) => record.durationMs)),
      ]),
    ) as Readonly<Record<CommandName, NumericSummary>>;
    const tickGaps = playerTickSamples
      .slice(1)
      .map((sample, index) => sample[0] - playerTickSamples[index]![0])
      .filter((gap) => gap >= 0);

    return {
      seed: options.seed.toString(),
      preset: options.preset,
      lightingMode: options.lightingMode,
      liquidSimulationMode: options.liquidSimulationMode,
      radius: options.radius,
      route: {
        startChunkX: options.startChunkX,
        startChunkZ: options.startChunkZ,
        finalChunkX: finalChunk.chunkX,
        finalChunkZ: finalChunk.chunkZ,
        steps: options.steps,
        speedBlocksPerStep: options.speedBlocksPerStep,
        crossedChunkBoundaries,
      },
      durationMs,
      warmupMs: warmup.elapsedMs,
      cooldownMs: performance.now() - cooldownStartedAtMs,
      commands: commandSummaries,
      chunkSnapshotsPerPoll: summarizeNumbers(records
        .filter((record) => record.command === "poll_world_updates")
        .map((record) => record.chunkSnapshots)),
      playerTickGapMs: summarizeNumbers(tickGaps),
      totals: {
        polls,
        setChunkViews,
        setPlayerInputs,
        chunkSnapshots: records.reduce((sum, record) => sum + record.chunkSnapshots, 0),
        playerStates: records.reduce((sum, record) => sum + record.playerStates, 0),
        progressMessages,
      },
      latestProgress: [...latestProgress.values()],
      ...(worldgenStart === undefined ? {} : { worldgenStart }),
      ...(latestPerformance?.worldgen === undefined ? {} : { worldgenEnd: latestPerformance.worldgen }),
      ...(diffWorldgenCounters(worldgenStart, latestPerformance?.worldgen) === undefined
        ? {}
        : { worldgenDelta: diffWorldgenCounters(worldgenStart, latestPerformance?.worldgen)! }),
    };
  } finally {
    host.close();
  }
}

function formatMs(ms: number): string {
  return `${ms.toFixed(1)}ms`;
}

function formatSummary(summary: NumericSummary): string {
  return `p50=${formatMs(summary.p50)} p95=${formatMs(summary.p95)} p99=${formatMs(summary.p99)} max=${formatMs(summary.max)} n=${summary.count.toString()}`;
}

function formatCountSummary(summary: NumericSummary): string {
  return `p50=${summary.p50.toFixed(1)} p95=${summary.p95.toFixed(1)} p99=${summary.p99.toFixed(1)} max=${summary.max.toFixed(1)} n=${summary.count.toString()}`;
}

function formatTopWorldgenPhases(counters: WorldgenPerformanceCounters | undefined): string {
  if (counters === undefined) {
    return "none";
  }

  return Object.entries(counters.phases)
    .sort((a, b) => b[1].totalMs - a[1].totalMs)
    .slice(0, 8)
    .map(([name, phase]) => `${name} total=${formatMs(phase.totalMs)} max=${formatMs(phase.maxMs)} count=${phase.count.toString()}`)
    .join("; ") || "none";
}

function printReport(report: FlybyReport): void {
  process.stdout.write(
    `worldgen host flyby seed=${report.seed} preset=${report.preset} lighting=${report.lightingMode} `
    + `liquid=${report.liquidSimulationMode} radius=${report.radius.toString()} duration=${formatMs(report.durationMs)}\n`,
  );
  process.stdout.write(
    `route: (${report.route.startChunkX.toString()},${report.route.startChunkZ.toString()}) -> `
    + `(${report.route.finalChunkX.toString()},${report.route.finalChunkZ.toString()}), `
    + `${report.route.crossedChunkBoundaries.toString()} chunk boundaries, steps=${report.route.steps.toString()}\n`,
  );
  process.stdout.write(`warmup=${formatMs(report.warmupMs)} cooldown=${formatMs(report.cooldownMs)}\n`);
  process.stdout.write(`set_chunk_view: ${formatSummary(report.commands.set_chunk_view)}\n`);
  process.stdout.write(`set_player_input: ${formatSummary(report.commands.set_player_input)}\n`);
  process.stdout.write(`poll_world_updates: ${formatSummary(report.commands.poll_world_updates)}\n`);
  process.stdout.write(`player tick gaps: ${formatSummary(report.playerTickGapMs)}\n`);
  process.stdout.write(`snapshots per poll: ${formatCountSummary(report.chunkSnapshotsPerPoll)}\n`);
  process.stdout.write(
    `totals: polls=${report.totals.polls.toString()} setChunkViews=${report.totals.setChunkViews.toString()} `
    + `snapshots=${report.totals.chunkSnapshots.toString()} playerStates=${report.totals.playerStates.toString()}\n`,
  );
  process.stdout.write(`worldgen top phases: ${formatTopWorldgenPhases(report.worldgenDelta)}\n`);
  process.stdout.write(`worldgen counts: ${JSON.stringify(report.worldgenDelta?.counts ?? {})}\n`);
}

function isDirectExecution(): boolean {
  return process.argv[1] !== undefined && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
}

export async function main(argv: readonly string[] = process.argv.slice(2)): Promise<void> {
  const options = parseCliOptions(argv);
  const report = await runFlyby(options);
  if (options.outputPath !== undefined) {
    const { writeFile } = await import("node:fs/promises");
    await writeFile(options.outputPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  if (options.json) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
    return;
  }

  printReport(report);
}

if (isDirectExecution()) {
  void main().catch((error) => {
    const message = error instanceof Error ? error.stack ?? error.message : String(error);
    process.stderr.write(`${message}\n`);
    process.exitCode = 1;
  });
}
