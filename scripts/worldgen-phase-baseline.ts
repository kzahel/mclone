import { performance } from "node:perf_hooks";
import process from "node:process";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { OverworldBiomeSource } from "../src/worldgen/biome/overworld-biome-source";
import { ChunkBiomeContainer } from "../src/worldgen/biome/chunk-biome-container";
import { NoiseBasedChunkGenerator } from "../src/worldgen/levelgen/noise-based-chunk-generator";
import { GeneratedRenderLevel } from "../src/world/level/generated-render-level";
import {
  FEATURES_CHUNK_DEPENDENCY_RADIUS,
  FEATURES_WRITE_RADIUS_CUTOFF,
} from "../src/world/level/generated-decoration-region";
import { registerGeneratedRenderBlocks } from "../src/world/level/generated-render-blocks";
import {
  buildChunkSnapshot,
  createBlockStateResolver,
  type BlockStateResolver,
} from "../src/world/level/chunk-snapshot";
import {
  packChunkSnapshot,
  type PackedChunkSnapshot,
} from "../src/world/level/packed-chunk-snapshot";
import {
  GeneratedWorldHost,
  getGeneratedWorldFeaturesChunkRadius,
  getGeneratedWorldFullChunkRadius,
  getGeneratedWorldViewChunkRadius,
} from "../src/runtime/host/generated-world-host";
import { createGeneratedWorldHostForRequest } from "../src/runtime/host/generated-world-host-factory";
import type {
  OpenWorldPreset,
  OpenWorldRequest,
  WorldHostMessage,
  WorldProgressMessage,
} from "../src/runtime/protocol/world-messages";
import type { BlockState } from "../src/world/level/block/state/block-state";
import type { BlockStateIdMap } from "../src/world/level/block/state/block-state-id";

type DirectMode = "sync" | "cooperative-noop" | "cooperative-budget";
type HostMode = "sync" | "cooperative";

interface CliOptions {
  readonly seed: bigint;
  readonly preset: OpenWorldPreset;
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly radius: number;
  readonly directModes: readonly DirectMode[];
  readonly hostModes: readonly HostMode[];
  readonly yieldBudgetMs: number;
  readonly pollMs: number;
  readonly timeoutMs: number;
  readonly json: boolean;
}

interface ChunkWindow {
  readonly publishRadius: number;
  readonly fullRadius: number;
  readonly featuresRadius: number;
  readonly authorityRadius: number;
  readonly publishChunkCount: number;
  readonly fullChunkCount: number;
  readonly featuresChunkCount: number;
  readonly authorityChunkCount: number;
}

interface PhaseResult {
  readonly name: string;
  readonly chunks?: number;
  readonly ms: number;
  readonly chunksPerSecond?: number;
  readonly detail?: string;
}

interface YieldCounters {
  readonly calls: number;
  readonly actualYields: number;
  readonly waitMs: number;
}

interface DirectBenchmarkResult {
  readonly kind: "direct";
  readonly mode: DirectMode;
  readonly phases: readonly PhaseResult[];
  readonly yieldCounters?: YieldCounters;
}

interface HostBenchmarkResult {
  readonly kind: "host";
  readonly mode: HostMode;
  readonly phases: readonly PhaseResult[];
  readonly progress: readonly WorldProgressMessage[];
}

interface BenchmarkReport {
  readonly seed: string;
  readonly preset: OpenWorldPreset;
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly radius: number;
  readonly window: ChunkWindow;
  readonly direct: readonly DirectBenchmarkResult[];
  readonly host: readonly HostBenchmarkResult[];
}

interface GeneratedPalette {
  readonly airState: BlockState;
  readonly blockStateById: readonly BlockState[];
  readonly blockStateIds: BlockStateIdMap;
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
  if (value === "default" || value === "browser_smoke") {
    return value;
  }

  throw new Error(`Unsupported preset ${value}`);
}

function parseDirectModes(value: string): readonly DirectMode[] {
  if (value === "none") {
    return [];
  }

  return value.split(",").map((mode) => {
    if (mode === "sync" || mode === "cooperative-noop" || mode === "cooperative-budget") {
      return mode;
    }

    throw new Error(`Unsupported direct mode ${mode}`);
  });
}

function parseHostModes(value: string): readonly HostMode[] {
  if (value === "none") {
    return [];
  }

  return value.split(",").map((mode) => {
    if (mode === "sync" || mode === "cooperative") {
      return mode;
    }

    throw new Error(`Unsupported host mode ${mode}`);
  });
}

function parseCliOptions(argv: readonly string[]): CliOptions {
  let seed = 12345n;
  let preset: OpenWorldPreset = "default";
  let centerChunkX = 0;
  let centerChunkZ = 0;
  let radius = 1;
  let directModes: readonly DirectMode[] = ["sync", "cooperative-budget"];
  let hostModes: readonly HostMode[] = ["sync", "cooperative"];
  let yieldBudgetMs = 8;
  let pollMs = 10;
  let timeoutMs = 180_000;
  let json = false;

  for (let index = 0; index < argv.length; index++) {
    const option = argv[index];
    if (option === undefined) {
      continue;
    }

    if (option === "--") {
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
      case "--center-chunk-x":
        centerChunkX = parseInteger("centerChunkX", value);
        break;
      case "--center-chunk-z":
        centerChunkZ = parseInteger("centerChunkZ", value);
        break;
      case "--radius":
        radius = parseInteger("radius", value);
        if (radius < 0) {
          throw new Error("radius must be non-negative");
        }
        break;
      case "--direct-modes":
        directModes = parseDirectModes(value);
        break;
      case "--host-modes":
        hostModes = parseHostModes(value);
        break;
      case "--yield-budget-ms":
        yieldBudgetMs = parseFloatOption("yieldBudgetMs", value);
        if (yieldBudgetMs < 0) {
          throw new Error("yieldBudgetMs must be non-negative");
        }
        break;
      case "--poll-ms":
        pollMs = parseFloatOption("pollMs", value);
        if (pollMs < 0) {
          throw new Error("pollMs must be non-negative");
        }
        break;
      case "--timeout-ms":
        timeoutMs = parseFloatOption("timeoutMs", value);
        if (timeoutMs <= 0) {
          throw new Error("timeoutMs must be positive");
        }
        break;
      default:
        throw new Error(`Unknown option ${option}`);
    }

    index++;
  }

  return {
    seed,
    preset,
    centerChunkX,
    centerChunkZ,
    radius,
    directModes,
    hostModes,
    yieldBudgetMs,
    pollMs,
    timeoutMs,
    json,
  };
}

function squareCount(radius: number): number {
  return (radius * 2 + 1) ** 2;
}

function createChunkWindow(radius: number): ChunkWindow {
  const publishRadius = getGeneratedWorldViewChunkRadius(radius);
  const fullRadius = getGeneratedWorldFullChunkRadius(radius);
  const featuresRadius = getGeneratedWorldFeaturesChunkRadius(radius);
  const authorityRadius = featuresRadius + FEATURES_CHUNK_DEPENDENCY_RADIUS;
  return {
    publishRadius,
    fullRadius,
    featuresRadius,
    authorityRadius,
    publishChunkCount: squareCount(publishRadius),
    fullChunkCount: squareCount(fullRadius),
    featuresChunkCount: squareCount(featuresRadius),
    authorityChunkCount: squareCount(authorityRadius),
  };
}

function chunkCoordinates(centerChunkX: number, centerChunkZ: number, radius: number): readonly (readonly [number, number])[] {
  const chunks: Array<readonly [number, number]> = [];
  for (let chunkZ = centerChunkZ - radius; chunkZ <= centerChunkZ + radius; chunkZ++) {
    for (let chunkX = centerChunkX - radius; chunkX <= centerChunkX + radius; chunkX++) {
      chunks.push([chunkX, chunkZ]);
    }
  }

  return chunks;
}

function isWithinChunkRadius(
  chunkX: number,
  chunkZ: number,
  centerChunkX: number,
  centerChunkZ: number,
  radius: number,
): boolean {
  return Math.abs(chunkX - centerChunkX) <= radius && Math.abs(chunkZ - centerChunkZ) <= radius;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function createYieldMeter(mode: DirectMode, budgetMs: number): {
  readonly yieldStep: () => Promise<void>;
  readonly getCounters: () => YieldCounters;
} {
  let calls = 0;
  let actualYields = 0;
  let waitMs = 0;
  let lastYieldAtMs = performance.now();

  return {
    yieldStep: async () => {
      calls++;
      if (mode === "cooperative-noop") {
        return;
      }

      const nowMs = performance.now();
      if (nowMs - lastYieldAtMs < budgetMs) {
        return;
      }

      actualYields++;
      const waitStartedAtMs = performance.now();
      await sleep(1);
      waitMs += performance.now() - waitStartedAtMs;
      lastYieldAtMs = performance.now();
    },
    getCounters: () => ({ calls, actualYields, waitMs }),
  };
}

async function timePhase(
  name: string,
  chunks: number | undefined,
  run: () => void | Promise<void>,
  detail?: () => string | undefined,
): Promise<PhaseResult> {
  const startedAtMs = performance.now();
  await run();
  const ms = performance.now() - startedAtMs;
  return {
    name,
    chunks,
    ms,
    chunksPerSecond: chunks === undefined || ms <= 0 ? undefined : chunks / (ms / 1000),
    detail: detail?.(),
  };
}

function createPalette(): GeneratedPalette {
  const generatedBlocks = registerGeneratedRenderBlocks();
  return {
    airState: generatedBlocks.airState,
    blockStateById: generatedBlocks.blockStateById,
    blockStateIds: generatedBlocks.blockStateIds,
  };
}

function createLevel(seed: bigint, centerChunkX: number, centerChunkZ: number, radius: number, palette: GeneratedPalette): {
  readonly level: GeneratedRenderLevel;
  readonly biomeSource: OverworldBiomeSource;
} {
  const biomeSource = new OverworldBiomeSource(seed);
  const generator = new NoiseBasedChunkGenerator(biomeSource, seed);
  const level = new GeneratedRenderLevel(
    palette.airState,
    generator,
    biomeSource,
    seed,
    palette.blockStateById,
  );
  level.updateChunkView(centerChunkX, centerChunkZ, radius);
  return { level, biomeSource };
}

function packedChunkBytes(snapshot: PackedChunkSnapshot): number {
  let bytes = snapshot.biomes.length * 8;
  for (const section of snapshot.sections) {
    bytes += section.paletteStateIds.byteLength + section.packedBlockIndices.byteLength;
  }
  bytes += snapshot.blockTicks.length * 24;
  bytes += snapshot.liquidTicks.length * 24;
  return bytes;
}

function packSnapshotForChunk(
  level: GeneratedRenderLevel,
  biomeSource: OverworldBiomeSource,
  palette: GeneratedPalette,
  resolveBlockState: BlockStateResolver,
  chunkX: number,
  chunkZ: number,
): PackedChunkSnapshot {
  const chunk = level.getAuthorityChunk(chunkX, chunkZ);
  if (chunk === null) {
    throw new Error(`Missing generated chunk (${chunkX.toString()}, ${chunkZ.toString()})`);
  }

  return packChunkSnapshot(
    buildChunkSnapshot(
      chunk,
      new ChunkBiomeContainer(
        level.getMinBuildHeight(),
        level.getHeight(),
        chunkX,
        chunkZ,
        biomeSource,
      ).writeBiomes(),
      level.getMinBuildHeight(),
      level.getHeight(),
    ),
    palette.blockStateIds,
    resolveBlockState,
  );
}

async function runDirectBenchmark(options: CliOptions, mode: DirectMode, window: ChunkWindow): Promise<DirectBenchmarkResult> {
  const palette = createPalette();
  const { level, biomeSource } = createLevel(options.seed, options.centerChunkX, options.centerChunkZ, options.radius, palette);
  const terrainHaloChunks = chunkCoordinates(
    options.centerChunkX,
    options.centerChunkZ,
    window.featuresRadius + FEATURES_WRITE_RADIUS_CUTOFF,
  );
  const publishTerrainChunks = terrainHaloChunks.filter(([chunkX, chunkZ]) =>
    isWithinChunkRadius(chunkX, chunkZ, options.centerChunkX, options.centerChunkZ, window.publishRadius)
  );
  const remainingTerrainHaloChunks = terrainHaloChunks.filter(([chunkX, chunkZ]) =>
    !isWithinChunkRadius(chunkX, chunkZ, options.centerChunkX, options.centerChunkZ, window.publishRadius)
  );
  const featuresChunks = chunkCoordinates(options.centerChunkX, options.centerChunkZ, window.featuresRadius);
  const fullChunks = chunkCoordinates(options.centerChunkX, options.centerChunkZ, window.fullRadius);
  const publishChunks = chunkCoordinates(options.centerChunkX, options.centerChunkZ, window.publishRadius);
  const phases: PhaseResult[] = [];
  const yieldMeter = mode === "sync" ? undefined : createYieldMeter(mode, options.yieldBudgetMs);
  const resolveBlockState = createBlockStateResolver(palette.airState);

  phases.push(await timePhase("terrain:publish", publishTerrainChunks.length, async () => {
    for (const [chunkX, chunkZ] of publishTerrainChunks) {
      if (mode === "sync") {
        level.generateChunkTerrain(chunkX, chunkZ);
      } else {
        await level.generateChunkTerrainCooperative(chunkX, chunkZ, yieldMeter!.yieldStep);
      }
    }
  }));

  phases.push(await timePhase("terrain:features-halo", remainingTerrainHaloChunks.length, async () => {
    for (const [chunkX, chunkZ] of remainingTerrainHaloChunks) {
      if (mode === "sync") {
        level.generateChunkTerrain(chunkX, chunkZ);
      } else {
        await level.generateChunkTerrainCooperative(chunkX, chunkZ, yieldMeter!.yieldStep);
      }
    }
  }));

  phases.push(await timePhase("features:decorate", featuresChunks.length, async () => {
    for (const [chunkX, chunkZ] of featuresChunks) {
      if (mode === "sync") {
        level.decorateChunk(chunkX, chunkZ);
      } else {
        await level.decorateChunkCooperative(chunkX, chunkZ, yieldMeter!.yieldStep);
      }
    }
  }));

  phases.push(await timePhase("full:no-light", fullChunks.length, () => {
    for (const [chunkX, chunkZ] of fullChunks) {
      const chunk = level.getAuthorityChunk(chunkX, chunkZ);
      if (chunk === null) {
        throw new Error(`Missing full chunk (${chunkX.toString()}, ${chunkZ.toString()})`);
      }
      level.markChunkLighted(chunkX, chunkZ);
      level.markChunkFull(chunkX, chunkZ);
    }
  }));

  let packedBytes = 0;
  let packedSections = 0;
  phases.push(await timePhase("snapshot:pack-publish", publishChunks.length, () => {
    for (const [chunkX, chunkZ] of publishChunks) {
      const snapshot = packSnapshotForChunk(level, biomeSource, palette, resolveBlockState, chunkX, chunkZ);
      packedBytes += packedChunkBytes(snapshot);
      packedSections += snapshot.sections.length;
    }
  }, () => `${packedSections.toString()} sections, ${(packedBytes / (1024 * 1024)).toFixed(2)} MiB packed payload`));

  return {
    kind: "direct",
    mode,
    phases,
    yieldCounters: yieldMeter?.getCounters(),
  };
}

function createOpenWorldRequest(options: CliOptions): OpenWorldRequest {
  return {
    type: "open_world",
    seed: options.seed,
    preset: options.preset,
    config: {
      lightingMode: "none",
      liquidSimulationMode: "none",
    },
  };
}

function countSnapshots(messages: readonly WorldHostMessage[]): number {
  return messages.filter((message) => message.type === "chunk_snapshot").length;
}

function throwOnWorldError(messages: readonly WorldHostMessage[]): void {
  const error = messages.find((message) => message.type === "world_error");
  if (error !== undefined) {
    throw new Error(error.message);
  }
}

function collectProgress(messages: readonly WorldHostMessage[], progress: WorldProgressMessage[]): void {
  for (const message of messages) {
    if (message.type === "world_progress") {
      progress.push(message);
    }
  }
}

async function runHostBenchmark(options: CliOptions, mode: HostMode, expectedPublishChunks: number): Promise<HostBenchmarkResult> {
  const request = createOpenWorldRequest(options);
  const host = createGeneratedWorldHostForRequest(request, {
    lightingMode: "none",
    liquidSimulationMode: "none",
    chunkViewScheduling: mode === "cooperative" ? "cooperative" : "synchronous",
  });
  const phases: PhaseResult[] = [];
  const progress: WorldProgressMessage[] = [];

  try {
    phases.push(await timePhase("open_world", undefined, async () => {
      const messages = await host.openWorld(request);
      throwOnWorldError(messages);
    }));

    const chunkViewRequest = {
      type: "set_chunk_view",
      centerChunkX: options.centerChunkX,
      centerChunkZ: options.centerChunkZ,
      radius: options.radius,
    } as const;
    let publishedChunks = 0;

    if (mode === "sync") {
      phases.push(await timePhase("set_chunk_view:complete", expectedPublishChunks, async () => {
        const messages = await host.setChunkView(chunkViewRequest);
        throwOnWorldError(messages);
        collectProgress(messages, progress);
        publishedChunks += countSnapshots(messages);
      }, () => `${publishedChunks.toString()} chunk snapshots returned`));
    } else {
      phases.push(await timePhase("set_chunk_view:ack", undefined, async () => {
        const messages = await host.setChunkView(chunkViewRequest);
        throwOnWorldError(messages);
        collectProgress(messages, progress);
        publishedChunks += countSnapshots(messages);
      }, () => `${publishedChunks.toString()} chunk snapshots returned in ack`));

      phases.push(await timePhase("poll_until_publish", expectedPublishChunks, async () => {
        const deadlineMs = performance.now() + options.timeoutMs;
        while (publishedChunks < expectedPublishChunks) {
          if (performance.now() > deadlineMs) {
            throw new Error(
              `Timed out after ${options.timeoutMs.toFixed(0)}ms waiting for ${expectedPublishChunks.toString()} snapshots; got ${publishedChunks.toString()}`,
            );
          }

          await sleep(options.pollMs);
          const messages = await host.pollUpdates({
            type: "poll_world_updates",
            maxMessages: 512,
          });
          throwOnWorldError(messages);
          collectProgress(messages, progress);
          publishedChunks += countSnapshots(messages);
        }
      }, () => `${publishedChunks.toString()} chunk snapshots returned`));
    }
  } finally {
    if (host instanceof GeneratedWorldHost) {
      host.close();
    }
  }

  return {
    kind: "host",
    mode,
    phases,
    progress,
  };
}

async function runBenchmark(options: CliOptions): Promise<BenchmarkReport> {
  const window = createChunkWindow(options.radius);
  const direct: DirectBenchmarkResult[] = [];
  for (const mode of options.directModes) {
    direct.push(await runDirectBenchmark(options, mode, window));
  }

  const host: HostBenchmarkResult[] = [];
  for (const mode of options.hostModes) {
    host.push(await runHostBenchmark(options, mode, window.publishChunkCount));
  }

  return {
    seed: options.seed.toString(),
    preset: options.preset,
    centerChunkX: options.centerChunkX,
    centerChunkZ: options.centerChunkZ,
    radius: options.radius,
    window,
    direct,
    host,
  };
}

function formatMs(ms: number): string {
  return `${ms.toFixed(1)}ms`;
}

function formatRate(rate: number | undefined): string {
  return rate === undefined ? "-" : `${rate.toFixed(1)}/s`;
}

function printPhaseRows(prefix: string, phases: readonly PhaseResult[]): void {
  for (const phase of phases) {
    const chunks = phase.chunks === undefined ? "-" : phase.chunks.toString();
    const detail = phase.detail === undefined ? "" : `  ${phase.detail}`;
    process.stdout.write(
      `${prefix.padEnd(26)} ${phase.name.padEnd(24)} ${chunks.padStart(6)} chunks  ${formatMs(phase.ms).padStart(10)}  ${formatRate(phase.chunksPerSecond).padStart(10)}${detail}\n`,
    );
  }
}

function summarizeProgress(progress: readonly WorldProgressMessage[]): string {
  if (progress.length === 0) {
    return "no progress messages";
  }

  const latestByStage = new Map<string, WorldProgressMessage>();
  for (const message of progress) {
    latestByStage.set(message.stage, message);
  }

  return [...latestByStage.values()]
    .map((message) => `${message.stage} ${message.current.toString()}/${message.total.toString()}${message.detail === undefined ? "" : ` (${message.detail})`}`)
    .join("; ");
}

function printReport(report: BenchmarkReport): void {
  process.stdout.write(`worldgen phase baseline seed=${report.seed} preset=${report.preset} center=(${report.centerChunkX.toString()},${report.centerChunkZ.toString()}) radius=${report.radius.toString()}\n`);
  process.stdout.write(
    `windows: publish r=${report.window.publishRadius.toString()} (${report.window.publishChunkCount.toString()} chunks), `
    + `full r=${report.window.fullRadius.toString()} (${report.window.fullChunkCount.toString()}), `
    + `features r=${report.window.featuresRadius.toString()} (${report.window.featuresChunkCount.toString()}), `
    + `authority r=${report.window.authorityRadius.toString()} (${report.window.authorityChunkCount.toString()})\n\n`,
  );
  process.stdout.write("runner                     phase                    count        time        rate\n");
  process.stdout.write("--------------------------------------------------------------------------------\n");
  for (const result of report.direct) {
    printPhaseRows(`direct:${result.mode}`, result.phases);
    if (result.yieldCounters !== undefined) {
      process.stdout.write(
        `${"".padEnd(26)} yield calls=${result.yieldCounters.calls.toString()}, actual=${result.yieldCounters.actualYields.toString()}, wait=${formatMs(result.yieldCounters.waitMs)}\n`,
      );
    }
  }
  for (const result of report.host) {
    printPhaseRows(`host:${result.mode}`, result.phases);
    process.stdout.write(`${"".padEnd(26)} progress: ${summarizeProgress(result.progress)}\n`);
  }
}

function isDirectExecution(): boolean {
  return process.argv[1] !== undefined && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
}

export async function main(argv: readonly string[] = process.argv.slice(2)): Promise<void> {
  const options = parseCliOptions(argv);
  const report = await runBenchmark(options);
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
