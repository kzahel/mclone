import process from "node:process";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { createGeneratedWorldHostForRequest } from "../src/runtime/host/generated-world-host-factory";
import {
  getGeneratedWorldFeaturesChunkRadius,
  getGeneratedWorldFullChunkRadius,
  getGeneratedWorldViewChunkRadius,
  type GeneratedChunkLifecycleRecord,
  type GeneratedChunkLifecycleSnapshot,
  type GeneratedWorldHost,
} from "../src/runtime/host/generated-world-host";
import { createNodeLightingService } from "../src/runtime/lighting/node-lighting-worker-client";
import type {
  OpenWorldPreset,
  OpenWorldRequest,
  WorldEngineLightingMode,
  WorldEngineLiquidSimulationMode,
  WorldHostMessage,
  WorldProgressMessage,
} from "../src/runtime/protocol/world-messages";
import { FEATURES_CHUNK_DEPENDENCY_RADIUS } from "../src/world/level/generated-decoration-region";
import {
  GeneratedChunkStatus,
  isGeneratedChunkStatusAtLeast,
  type GeneratedChunkStatus as GeneratedChunkStatusName,
} from "../src/world/level/generated-chunk-status";

interface CliOptions {
  readonly seed: bigint;
  readonly preset: OpenWorldPreset;
  readonly lightingMode: WorldEngineLightingMode;
  readonly liquidSimulationMode: WorldEngineLiquidSimulationMode;
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly route?: readonly ChunkCoordinate[];
  readonly radius: number;
  readonly waitForPublish: boolean;
  readonly pollMs: number;
  readonly maxMessages: number;
  readonly timeoutMs: number;
  readonly outputPath?: string;
  readonly json: boolean;
}

export type ChunkCoordinate = readonly [number, number];

interface ExpectedClosureCounts {
  readonly publishRadius: number;
  readonly fullRadius: number;
  readonly featuresRadius: number;
  readonly materializedTerrainRadius: number;
  readonly metadataRadius: number;
  readonly publishedChunks: number;
  readonly fullChunks: number;
  readonly featureChunks: number;
  readonly materializedTerrainChunks: number;
  readonly metadataChunks: number;
}

interface RouteBlockedChunk {
  readonly chunk: ChunkCoordinate;
  readonly blocker: GeneratedChunkLifecycleRecord["publicationBlocker"];
}

export interface ChunkLifecycleRouteDelta {
  readonly newlyPublished: readonly ChunkCoordinate[];
  readonly unpublished: readonly ChunkCoordinate[];
  readonly newlyFull: readonly ChunkCoordinate[];
  readonly newlyFeatures: readonly ChunkCoordinate[];
  readonly newlyMaterialized: readonly ChunkCoordinate[];
  readonly stillBlocked: readonly RouteBlockedChunk[];
  readonly readyToPublish: readonly ChunkCoordinate[];
  readonly asciiMap: string;
}

interface ChunkLifecycleRouteStepReport {
  readonly step: number;
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly snapshot: GeneratedChunkLifecycleSnapshot;
  readonly delta?: ChunkLifecycleRouteDelta;
}

interface ChunkLifecycleSingleObservatoryReport {
  readonly mode: "single";
  readonly seed: string;
  readonly preset: OpenWorldPreset;
  readonly lightingMode: WorldEngineLightingMode;
  readonly liquidSimulationMode: WorldEngineLiquidSimulationMode;
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly radius: number;
  readonly waitedForPublish: boolean;
  readonly expected: ExpectedClosureCounts;
  readonly latestProgress: readonly WorldProgressMessage[];
  readonly publishView: {
    readonly expected: number;
    readonly published: number;
    readonly missing: readonly GeneratedChunkLifecycleRecord[];
    readonly ready: readonly GeneratedChunkLifecycleRecord[];
  };
  readonly snapshot: GeneratedChunkLifecycleSnapshot;
}

interface ChunkLifecycleRouteObservatoryReport {
  readonly mode: "route";
  readonly seed: string;
  readonly preset: OpenWorldPreset;
  readonly lightingMode: WorldEngineLightingMode;
  readonly liquidSimulationMode: WorldEngineLiquidSimulationMode;
  readonly radius: number;
  readonly waitedForPublish: boolean;
  readonly expected: ExpectedClosureCounts;
  readonly latestProgress: readonly WorldProgressMessage[];
  readonly route: readonly ChunkCoordinate[];
  readonly steps: readonly ChunkLifecycleRouteStepReport[];
}

type ChunkLifecycleObservatoryReport =
  | ChunkLifecycleSingleObservatoryReport
  | ChunkLifecycleRouteObservatoryReport;

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

export function parseChunkRoute(value: string): readonly ChunkCoordinate[] {
  const parts = value
    .split(/\s*(?:->|;)\s*/u)
    .map((part) => part.trim())
    .filter((part) => part.length > 0);
  if (parts.length === 0) {
    throw new Error("Route must contain at least one chunk coordinate");
  }

  return parts.map((part) => {
    const match = /^(-?\d+)\s*,\s*(-?\d+)$/u.exec(part);
    if (match === null) {
      throw new Error(`Invalid route coordinate "${part}", expected "chunkX,chunkZ"`);
    }

    return [parseInteger("route chunkX", match[1]!), parseInteger("route chunkZ", match[2]!)] as const;
  });
}

function parseCliOptions(argv: readonly string[]): CliOptions {
  let seed = 12345n;
  let preset: OpenWorldPreset = "default";
  let lightingMode: WorldEngineLightingMode = "none";
  let liquidSimulationMode: WorldEngineLiquidSimulationMode = "none";
  let centerChunkX = 0;
  let centerChunkZ = 0;
  let route: readonly ChunkCoordinate[] | undefined;
  let radius = 0;
  let waitForPublish = false;
  let pollMs = 10;
  let maxMessages = 512;
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
    if (option === "--wait-for-publish") {
      waitForPublish = true;
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
      case "--center-chunk-x":
        centerChunkX = parseInteger("centerChunkX", value);
        break;
      case "--center-chunk-z":
        centerChunkZ = parseInteger("centerChunkZ", value);
        break;
      case "--route":
        route = parseChunkRoute(value);
        break;
      case "--radius":
        radius = parseInteger("radius", value);
        break;
      case "--poll-ms":
        pollMs = parseFloatOption("pollMs", value);
        break;
      case "--max-messages":
        maxMessages = parseInteger("maxMessages", value);
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

  if (radius < 0 || pollMs < 0 || maxMessages <= 0 || timeoutMs <= 0) {
    throw new Error("radius/poll/maxMessages/timeout options must be non-negative, with maxMessages and timeout positive");
  }

  return {
    seed,
    preset,
    lightingMode,
    liquidSimulationMode,
    centerChunkX,
    centerChunkZ,
    ...(route === undefined ? {} : { route }),
    radius,
    waitForPublish,
    pollMs,
    maxMessages,
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

function squareCount(radius: number): number {
  return ((radius * 2) + 1) ** 2;
}

function expectedClosureCounts(radius: number): ExpectedClosureCounts {
  const publishRadius = getGeneratedWorldViewChunkRadius(radius);
  const fullRadius = getGeneratedWorldFullChunkRadius(radius);
  const featuresRadius = getGeneratedWorldFeaturesChunkRadius(radius);
  const materializedTerrainRadius = featuresRadius + 1;
  const metadataRadius = featuresRadius + FEATURES_CHUNK_DEPENDENCY_RADIUS;
  return {
    publishRadius,
    fullRadius,
    featuresRadius,
    materializedTerrainRadius,
    metadataRadius,
    publishedChunks: squareCount(publishRadius),
    fullChunks: squareCount(fullRadius),
    featureChunks: squareCount(featuresRadius),
    materializedTerrainChunks: squareCount(materializedTerrainRadius),
    metadataChunks: squareCount(metadataRadius),
  };
}

function throwOnWorldError(messages: readonly WorldHostMessage[]): void {
  const error = messages.find((message) => message.type === "world_error");
  if (error !== undefined) {
    throw new Error(error.message);
  }
}

function collectLatestProgress(messages: readonly WorldHostMessage[], latestProgress: Map<string, WorldProgressMessage>): void {
  for (const message of messages) {
    if (message.type === "world_progress") {
      latestProgress.set(message.stage, message);
    }
  }
}

function publishViewRecords(snapshot: GeneratedChunkLifecycleSnapshot): readonly GeneratedChunkLifecycleRecord[] {
  return snapshot.records.filter((record) => record.inPublishView);
}

function isPublishViewSettled(snapshot: GeneratedChunkLifecycleSnapshot, expectedPublishedChunks: number): boolean {
  const records = publishViewRecords(snapshot);
  return records.length === expectedPublishedChunks && records.every((record) => record.published);
}

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

function coordinateKey(coord: ChunkCoordinate): string {
  return chunkKey(coord[0], coord[1]);
}

function recordKey(record: GeneratedChunkLifecycleRecord): string {
  return chunkKey(record.chunkX, record.chunkZ);
}

function coordinateFromKey(key: string): ChunkCoordinate {
  const [chunkXRaw, chunkZRaw] = key.split(",");
  return [Number(chunkXRaw), Number(chunkZRaw)];
}

function coordFromRecord(record: GeneratedChunkLifecycleRecord): ChunkCoordinate {
  return [record.chunkX, record.chunkZ];
}

function sortCoords(coords: Iterable<ChunkCoordinate>): readonly ChunkCoordinate[] {
  return [...coords].sort((left, right) => left[1] - right[1] || left[0] - right[0]);
}

function lifecycleRecordMap(snapshot: GeneratedChunkLifecycleSnapshot): Map<string, GeneratedChunkLifecycleRecord> {
  return new Map(snapshot.records.map((record) => [recordKey(record), record]));
}

function hasStatusAtLeast(record: GeneratedChunkLifecycleRecord | undefined, status: GeneratedChunkStatusName): boolean {
  return record !== undefined && isGeneratedChunkStatusAtLeast(record.generatedStatus, status);
}

function changedToTrueCoords(
  previous: Map<string, GeneratedChunkLifecycleRecord>,
  current: Map<string, GeneratedChunkLifecycleRecord>,
  predicate: (record: GeneratedChunkLifecycleRecord | undefined) => boolean,
): readonly ChunkCoordinate[] {
  const keys = new Set([...previous.keys(), ...current.keys()]);
  const coords: ChunkCoordinate[] = [];
  for (const key of keys) {
    if (!predicate(previous.get(key)) && predicate(current.get(key))) {
      coords.push(coordinateFromKey(key));
    }
  }

  return sortCoords(coords);
}

function changedToFalseCoords(
  previous: Map<string, GeneratedChunkLifecycleRecord>,
  current: Map<string, GeneratedChunkLifecycleRecord>,
  predicate: (record: GeneratedChunkLifecycleRecord | undefined) => boolean,
): readonly ChunkCoordinate[] {
  const keys = new Set([...previous.keys(), ...current.keys()]);
  const coords: ChunkCoordinate[] = [];
  for (const key of keys) {
    if (predicate(previous.get(key)) && !predicate(current.get(key))) {
      coords.push(coordinateFromKey(key));
    }
  }

  return sortCoords(coords);
}

export function createChunkLifecycleRouteDelta(
  previous: GeneratedChunkLifecycleSnapshot,
  current: GeneratedChunkLifecycleSnapshot,
  center: ChunkCoordinate,
): ChunkLifecycleRouteDelta {
  const previousRecords = lifecycleRecordMap(previous);
  const currentRecords = lifecycleRecordMap(current);
  const stillBlocked = current.records
    .filter((record) => record.inPublishView && !record.published)
    .map((record) => ({
      chunk: coordFromRecord(record),
      blocker: record.publicationBlocker,
    }))
    .sort((left, right) => left.chunk[1] - right.chunk[1] || left.chunk[0] - right.chunk[0]);
  const deltaWithoutMap = {
    newlyPublished: changedToTrueCoords(previousRecords, currentRecords, (record) => record?.published === true),
    unpublished: changedToFalseCoords(previousRecords, currentRecords, (record) => record?.published === true),
    newlyFull: changedToTrueCoords(previousRecords, currentRecords, (record) => hasStatusAtLeast(record, GeneratedChunkStatus.FULL)),
    newlyFeatures: changedToTrueCoords(previousRecords, currentRecords, (record) => hasStatusAtLeast(record, GeneratedChunkStatus.FEATURES)),
    newlyMaterialized: changedToTrueCoords(previousRecords, currentRecords, (record) => record?.hasBlockSections === true),
    stillBlocked,
    readyToPublish: sortCoords(current.records
      .filter((record) => record.publicationBlocker.kind === "ready_to_publish")
      .map(coordFromRecord)),
  };

  return {
    ...deltaWithoutMap,
    asciiMap: renderChunkLifecycleDeltaAsciiMap(current, deltaWithoutMap, center),
  };
}

function coordSet(coords: readonly ChunkCoordinate[]): Set<string> {
  return new Set(coords.map(coordinateKey));
}

type ChunkLifecycleDeltaForMap = Omit<ChunkLifecycleRouteDelta, "asciiMap">;

function renderChunkLifecycleDeltaAsciiMap(
  snapshot: GeneratedChunkLifecycleSnapshot,
  delta: ChunkLifecycleDeltaForMap,
  center: ChunkCoordinate,
): string {
  const currentRecords = lifecycleRecordMap(snapshot);
  const newlyPublished = coordSet(delta.newlyPublished);
  const unpublished = coordSet(delta.unpublished);
  const newlyFull = coordSet(delta.newlyFull);
  const newlyFeatures = coordSet(delta.newlyFeatures);
  const newlyMaterialized = coordSet(delta.newlyMaterialized);
  const blocked = coordSet(delta.stillBlocked.map((entry) => entry.chunk));
  const ready = coordSet(delta.readyToPublish);
  const currentPublish = coordSet(snapshot.records
    .filter((record) => record.inPublishView)
    .map(coordFromRecord));
  const keys = new Set([
    ...newlyPublished,
    ...unpublished,
    ...newlyFull,
    ...newlyFeatures,
    ...newlyMaterialized,
    ...blocked,
    ...ready,
    ...currentPublish,
    coordinateKey(center),
  ]);

  const coords = [...keys].map(coordinateFromKey);
  const minChunkX = Math.min(...coords.map((coord) => coord[0]));
  const maxChunkX = Math.max(...coords.map((coord) => coord[0]));
  const minChunkZ = Math.min(...coords.map((coord) => coord[1]));
  const maxChunkZ = Math.max(...coords.map((coord) => coord[1]));
  const lines: string[] = [];
  for (let chunkZ = minChunkZ; chunkZ <= maxChunkZ; chunkZ++) {
    const cells: string[] = [];
    for (let chunkX = minChunkX; chunkX <= maxChunkX; chunkX++) {
      const key = chunkKey(chunkX, chunkZ);
      const record = currentRecords.get(key);
      let cell = ".";
      if (blocked.has(key)) {
        cell = "B";
      } else if (newlyPublished.has(key)) {
        cell = "N";
      } else if (unpublished.has(key)) {
        cell = "U";
      } else if (record?.published === true) {
        cell = "P";
      } else if (ready.has(key)) {
        cell = "R";
      } else if (newlyFull.has(key)) {
        cell = "L";
      } else if (newlyFeatures.has(key)) {
        cell = "F";
      } else if (newlyMaterialized.has(key)) {
        cell = "T";
      } else if (record !== undefined && record.generatedStatus !== GeneratedChunkStatus.EMPTY && !record.hasBlockSections) {
        cell = "M";
      }
      if (chunkX === center[0] && chunkZ === center[1]) {
        cell = "C";
      }
      cells.push(cell);
    }
    lines.push(`z=${chunkZ.toString().padStart(3, " ")} ${cells.join(" ")}`);
  }

  return [
    `x=${minChunkX.toString()}..${maxChunkX.toString()}`,
    ...lines,
    "legend: C center, P published, N newly published, U unpublished, B blocked, R ready, L newly full, F newly features, T newly materialized, M metadata-only, . unchanged",
  ].join("\n");
}

async function pollUntilPublishSettled(
  host: GeneratedWorldHost,
  options: CliOptions,
  latestProgress: Map<string, WorldProgressMessage>,
): Promise<void> {
  const expected = expectedClosureCounts(options.radius);
  const deadline = Date.now() + options.timeoutMs;
  while (!isPublishViewSettled(host.getDebugChunkLifecycleSnapshot(), expected.publishedChunks)) {
    if (Date.now() > deadline) {
      const snapshot = host.getDebugChunkLifecycleSnapshot();
      const published = publishViewRecords(snapshot).filter((record) => record.published).length;
      throw new Error(
        `Timed out waiting for published view: got ${published.toString()} / ${expected.publishedChunks.toString()} chunks`,
      );
    }

    await sleep(options.pollMs);
    const messages = await host.pollUpdates({ type: "poll_world_updates", maxMessages: options.maxMessages });
    throwOnWorldError(messages);
    collectLatestProgress(messages, latestProgress);
  }
}

async function runObservatory(options: CliOptions): Promise<ChunkLifecycleObservatoryReport> {
  const request = createOpenWorldRequest(options);
  const lightingService = options.lightingMode === "vanilla17" ? createNodeLightingService() : undefined;
  const host = createGeneratedWorldHostForRequest(request, {
    chunkViewScheduling: "cooperative",
    lightingMode: options.lightingMode,
    liquidSimulationMode: options.liquidSimulationMode,
    lightingService,
  });
  const latestProgress = new Map<string, WorldProgressMessage>();

  try {
    let messages = await host.openWorld(request);
    throwOnWorldError(messages);
    collectLatestProgress(messages, latestProgress);

    if (options.route !== undefined) {
      const steps: ChunkLifecycleRouteStepReport[] = [];
      let previousSnapshot: GeneratedChunkLifecycleSnapshot | undefined;
      for (const [step, center] of options.route.entries()) {
        messages = await host.setChunkView({
          type: "set_chunk_view",
          centerChunkX: center[0],
          centerChunkZ: center[1],
          radius: options.radius,
        });
        throwOnWorldError(messages);
        collectLatestProgress(messages, latestProgress);
        if (options.waitForPublish) {
          await pollUntilPublishSettled(host, options, latestProgress);
        }

        const snapshot = host.getDebugChunkLifecycleSnapshot();
        const delta = previousSnapshot === undefined
          ? undefined
          : createChunkLifecycleRouteDelta(previousSnapshot, snapshot, center);
        steps.push({
          step,
          centerChunkX: center[0],
          centerChunkZ: center[1],
          snapshot,
          ...(delta === undefined ? {} : { delta }),
        });
        previousSnapshot = snapshot;
      }

      return {
        mode: "route",
        seed: options.seed.toString(),
        preset: options.preset,
        lightingMode: options.lightingMode,
        liquidSimulationMode: options.liquidSimulationMode,
        radius: options.radius,
        waitedForPublish: options.waitForPublish,
        expected: expectedClosureCounts(options.radius),
        latestProgress: [...latestProgress.values()],
        route: options.route,
        steps,
      };
    }

    messages = await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: options.centerChunkX,
      centerChunkZ: options.centerChunkZ,
      radius: options.radius,
    });
    throwOnWorldError(messages);
    collectLatestProgress(messages, latestProgress);

    if (options.waitForPublish) {
      await pollUntilPublishSettled(host, options, latestProgress);
    }

    const snapshot = host.getDebugChunkLifecycleSnapshot();
    const publishRecords = publishViewRecords(snapshot);
    return {
      mode: "single",
      seed: options.seed.toString(),
      preset: options.preset,
      lightingMode: options.lightingMode,
      liquidSimulationMode: options.liquidSimulationMode,
      centerChunkX: options.centerChunkX,
      centerChunkZ: options.centerChunkZ,
      radius: options.radius,
      waitedForPublish: options.waitForPublish,
      expected: expectedClosureCounts(options.radius),
      latestProgress: [...latestProgress.values()],
      publishView: {
        expected: expectedClosureCounts(options.radius).publishedChunks,
        published: publishRecords.filter((record) => record.published).length,
        missing: publishRecords.filter((record) => !record.published),
        ready: publishRecords.filter((record) => record.publicationBlocker.kind === "ready_to_publish"),
      },
      snapshot,
    };
  } finally {
    host.close();
  }
}

function printRecordSummary(record: GeneratedChunkLifecycleRecord): string {
  const parts = [
    `(${record.chunkX.toString()},${record.chunkZ.toString()})`,
    `status=${record.generatedStatus}`,
    `full=${record.holderFullStatus ?? record.ticketFullStatus}`,
    `loaded=${record.chunkLoaded ? "yes" : "no"}`,
    `published=${record.published ? "yes" : "no"}`,
    `blocker=${record.publicationBlocker.kind}`,
  ];
  if (record.publicationBlocker.chunkX !== undefined && record.publicationBlocker.chunkZ !== undefined) {
    parts.push(`blockedBy=(${record.publicationBlocker.chunkX.toString()},${record.publicationBlocker.chunkZ.toString()})`);
    parts.push(`actual=${record.publicationBlocker.actualStatus ?? "unknown"}`);
  }
  return parts.join(" ");
}

function printSingleReport(report: ChunkLifecycleSingleObservatoryReport): void {
  process.stdout.write(
    `chunk lifecycle seed=${report.seed} preset=${report.preset} center=`
    + `(${report.centerChunkX.toString()},${report.centerChunkZ.toString()}) radius=${report.radius.toString()}\n`,
  );
  process.stdout.write(
    `expected: published=${report.expected.publishedChunks.toString()} full=${report.expected.fullChunks.toString()} `
    + `features=${report.expected.featureChunks.toString()} terrain=${report.expected.materializedTerrainChunks.toString()} `
    + `metadata=${report.expected.metadataChunks.toString()}\n`,
  );
  process.stdout.write(
    `actual: records=${report.snapshot.counts.total.toString()} published=${report.snapshot.counts.published.toString()} `
    + `loaded=${report.snapshot.counts.loaded.toString()} materialized=${report.snapshot.counts.materialized.toString()} `
    + `authority=${report.snapshot.counts.inAuthorityView.toString()}\n`,
  );
  process.stdout.write(`publication blockers: ${JSON.stringify(report.snapshot.counts.byPublicationBlocker)}\n`);
  if (report.latestProgress.length > 0) {
    process.stdout.write(`progress: ${report.latestProgress.map((progress) =>
      `${progress.stage} ${progress.current.toString()}/${progress.total.toString()}`
    ).join("; ")}\n`);
  }
  if (report.publishView.missing.length > 0) {
    process.stdout.write("missing publish chunks:\n");
    for (const record of report.publishView.missing.slice(0, 32)) {
      process.stdout.write(`  ${printRecordSummary(record)}\n`);
    }
    if (report.publishView.missing.length > 32) {
      process.stdout.write(`  ... ${String(report.publishView.missing.length - 32)} more\n`);
    }
  }
}

function printRouteReport(report: ChunkLifecycleRouteObservatoryReport): void {
  process.stdout.write(
    `chunk lifecycle route seed=${report.seed} preset=${report.preset} radius=${report.radius.toString()} `
    + `steps=${report.steps.length.toString()}\n`,
  );
  process.stdout.write(
    `expected per settled view: published=${report.expected.publishedChunks.toString()} `
    + `full=${report.expected.fullChunks.toString()} features=${report.expected.featureChunks.toString()} `
    + `terrain=${report.expected.materializedTerrainChunks.toString()} metadata=${report.expected.metadataChunks.toString()}\n`,
  );
  for (const step of report.steps) {
    const counts = step.snapshot.counts;
    process.stdout.write(
      `step ${step.step.toString()} center=(${step.centerChunkX.toString()},${step.centerChunkZ.toString()}) `
      + `published=${counts.published.toString()} loaded=${counts.loaded.toString()} `
      + `materialized=${counts.materialized.toString()} blockers=${JSON.stringify(counts.byPublicationBlocker)}\n`,
    );
    if (step.delta === undefined) {
      continue;
    }

    process.stdout.write(
      `  delta: newlyPublished=${step.delta.newlyPublished.length.toString()} `
      + `unpublished=${step.delta.unpublished.length.toString()} `
      + `newlyFull=${step.delta.newlyFull.length.toString()} `
      + `newlyFeatures=${step.delta.newlyFeatures.length.toString()} `
      + `newlyMaterialized=${step.delta.newlyMaterialized.length.toString()} `
      + `blocked=${step.delta.stillBlocked.length.toString()}\n`,
    );
    process.stdout.write(`${step.delta.asciiMap}\n`);
  }
}

function printReport(report: ChunkLifecycleObservatoryReport): void {
  if (report.mode === "route") {
    printRouteReport(report);
    return;
  }

  printSingleReport(report);
}

function isDirectExecution(): boolean {
  return process.argv[1] !== undefined && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
}

export async function main(argv: readonly string[] = process.argv.slice(2)): Promise<void> {
  const options = parseCliOptions(argv);
  const report = await runObservatory(options);
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
