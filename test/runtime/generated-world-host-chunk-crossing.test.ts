import { afterEach, describe, expect, test } from "vitest";
import { Registry } from "../../src/core/registry";
import { createGeneratedWorldSaveId, GeneratedWorldHost } from "../../src/runtime/host/generated-world-host";
import type {
  ChunkSnapshotMessage,
  WorldgenPerformanceCounters,
  WorldHostMessage,
  WorldPerformanceMessage,
} from "../../src/runtime/protocol/world-messages";
import {
  createWorldSaveMetadata,
  type ChunkStorage,
  type OpenWorldStorageRequest,
  type WorldSaveMetadata,
  type WorldStorage,
  type WorldStorageSession,
} from "../../src/runtime/storage/world-storage";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import {
  GeneratedChunkStatus,
  type GeneratedChunkStatus as GeneratedChunkStatusName,
} from "../../src/world/level/generated-chunk-status";
import {
  cloneGeneratedChunkStorageRecord,
  type GeneratedChunkAccessDebugRecord,
  type GeneratedChunkStorageRecord,
} from "../../src/world/level/generated-proto-chunk";
import type { GeneratedChunkStatusDebugRecord } from "../../src/world/level/generated-render-level";
import { MemoryWorldStorage } from "../../src/runtime/storage/memory-world-storage";
import { clonePackedChunkSnapshot, type PackedChunkSnapshot } from "../../src/world/level/packed-chunk-snapshot";
import { FlatGrassWorldGenerator } from "../../src/worldgen/levelgen/demo-world-generators";
import { GenerationStep } from "../../src/worldgen/levelgen/generation-step";
import type { WorldGenerator } from "../../src/worldgen/levelgen/world-generator";

const OPEN_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "flat_grass",
  storageMode: "none",
  config: {
    lightingMode: "none",
    liquidSimulationMode: "none",
  },
} as const;

const TRACKED_WORLDGEN_COUNTS = [
  "storage_preload_already_loaded",
  "storage_preload_missing",
  "terrain_chunks_generated",
  "feature_chunks_decorated",
  "chunks_marked_full_without_lighting",
  "chunk_snapshots_published",
  "chunk_snapshots_built",
  "chunk_unloads_sent",
] as const;

type TrackedWorldgenCount = typeof TRACKED_WORLDGEN_COUNTS[number];

interface CountingWorldGeneratorSnapshot {
  readonly fillFromNoise: number;
  readonly uniqueFillFromNoise: number;
  readonly surfaceAndBedrock: number;
  readonly airCarvers: number;
  readonly liquidCarvers: number;
  readonly decorations: number;
  readonly uniqueDecorations: number;
  readonly mobSpawns: number;
  readonly uniqueMobSpawns: number;
}

interface CountingWorldGeneratorOptions {
  readonly slowFirstTerrainPhaseMs?: number;
}

class CountingWorldGenerator implements WorldGenerator {
  private readonly delegate: WorldGenerator;
  private readonly fillFromNoiseCalls: string[] = [];
  private readonly surfaceAndBedrockCalls: string[] = [];
  private readonly airCarverCalls: string[] = [];
  private readonly liquidCarverCalls: string[] = [];
  private readonly decorationCalls: string[] = [];
  private readonly mobSpawnCalls: string[] = [];
  private slowFillFromNoise = true;
  private slowSurfaceAndBedrock = true;
  private slowAirCarver = true;
  private slowLiquidCarver = true;

  public constructor(seed: bigint, private readonly generatorOptions: CountingWorldGeneratorOptions = {}) {
    this.delegate = new FlatGrassWorldGenerator(seed);
  }

  public getSeed(): bigint {
    return this.delegate.getSeed();
  }

  public getBiomeSource(): ReturnType<WorldGenerator["getBiomeSource"]> {
    return this.delegate.getBiomeSource();
  }

  public getBaseStoneSource(): ReturnType<WorldGenerator["getBaseStoneSource"]> {
    return this.delegate.getBaseStoneSource();
  }

  public getSeaLevel(): ReturnType<WorldGenerator["getSeaLevel"]> {
    return this.delegate.getSeaLevel();
  }

  public getPrimaryBiome(...args: Parameters<WorldGenerator["getPrimaryBiome"]>): ReturnType<WorldGenerator["getPrimaryBiome"]> {
    return this.delegate.getPrimaryBiome(...args);
  }

  public fillFromNoise(...args: Parameters<WorldGenerator["fillFromNoise"]>): ReturnType<WorldGenerator["fillFromNoise"]> {
    const [chunkX, chunkZ] = args;
    this.fillFromNoiseCalls.push(chunkKey(chunkX, chunkZ));
    const generated = this.delegate.fillFromNoise(...args);
    if (this.slowFillFromNoise) {
      this.slowFillFromNoise = false;
      busyWait(this.generatorOptions.slowFirstTerrainPhaseMs ?? 0);
    }
    return generated;
  }

  public buildSurfaceAndBedrock(...args: Parameters<WorldGenerator["buildSurfaceAndBedrock"]>): void {
    const [chunk] = args;
    this.surfaceAndBedrockCalls.push(chunkKey(chunk.chunkX, chunk.chunkZ));
    this.delegate.buildSurfaceAndBedrock(...args);
    if (this.slowSurfaceAndBedrock) {
      this.slowSurfaceAndBedrock = false;
      busyWait(this.generatorOptions.slowFirstTerrainPhaseMs ?? 0);
    }
  }

  public applyCarvers(...args: Parameters<WorldGenerator["applyCarvers"]>): void {
    const [chunk, step] = args;
    if (step === GenerationStep.Carving.LIQUID) {
      this.liquidCarverCalls.push(chunkKey(chunk.chunkX, chunk.chunkZ));
      this.delegate.applyCarvers(...args);
      if (this.slowLiquidCarver) {
        this.slowLiquidCarver = false;
        busyWait(this.generatorOptions.slowFirstTerrainPhaseMs ?? 0);
      }
    } else {
      this.airCarverCalls.push(chunkKey(chunk.chunkX, chunk.chunkZ));
      this.delegate.applyCarvers(...args);
      if (this.slowAirCarver) {
        this.slowAirCarver = false;
        busyWait(this.generatorOptions.slowFirstTerrainPhaseMs ?? 0);
      }
    }
  }

  public async applyCarversCooperative(...args: Parameters<WorldGenerator["applyCarversCooperative"]>): Promise<void> {
    const [chunk] = args;
    this.airCarverCalls.push(chunkKey(chunk.chunkX, chunk.chunkZ));
    this.liquidCarverCalls.push(chunkKey(chunk.chunkX, chunk.chunkZ));
    await this.delegate.applyCarversCooperative(...args);
  }

  public createStructures(...args: Parameters<WorldGenerator["createStructures"]>): void {
    this.delegate.createStructures(...args);
  }

  public createReferences(...args: Parameters<WorldGenerator["createReferences"]>): void {
    this.delegate.createReferences(...args);
  }

  public applyBiomeDecoration(...args: Parameters<WorldGenerator["applyBiomeDecoration"]>): void {
    const [, chunkX, chunkZ] = args;
    this.decorationCalls.push(chunkKey(chunkX, chunkZ));
    this.delegate.applyBiomeDecoration(...args);
  }

  public async applyBiomeDecorationCooperative(...args: Parameters<WorldGenerator["applyBiomeDecorationCooperative"]>): Promise<void> {
    const [, chunkX, chunkZ] = args;
    this.decorationCalls.push(chunkKey(chunkX, chunkZ));
    await this.delegate.applyBiomeDecorationCooperative(...args);
  }

  public spawnOriginalMobs(...args: Parameters<WorldGenerator["spawnOriginalMobs"]>): void {
    const [, chunkX, chunkZ] = args;
    this.mobSpawnCalls.push(chunkKey(chunkX, chunkZ));
    this.delegate.spawnOriginalMobs(...args);
  }

  public snapshot(): CountingWorldGeneratorSnapshot {
    return {
      fillFromNoise: this.fillFromNoiseCalls.length,
      uniqueFillFromNoise: new Set(this.fillFromNoiseCalls).size,
      surfaceAndBedrock: this.surfaceAndBedrockCalls.length,
      airCarvers: this.airCarverCalls.length,
      liquidCarvers: this.liquidCarverCalls.length,
      decorations: this.decorationCalls.length,
      uniqueDecorations: new Set(this.decorationCalls).size,
      mobSpawns: this.mobSpawnCalls.length,
      uniqueMobSpawns: new Set(this.mobSpawnCalls).size,
    };
  }

  public hasFillFromNoiseCall(chunkX: number, chunkZ: number): boolean {
    return this.fillFromNoiseCalls.includes(chunkKey(chunkX, chunkZ));
  }

  public fillFromNoiseCallCount(chunkX: number, chunkZ: number): number {
    return this.fillFromNoiseCalls.filter((key) => key === chunkKey(chunkX, chunkZ)).length;
  }

  public hasDecorationCall(chunkX: number, chunkZ: number): boolean {
    return this.decorationCalls.includes(chunkKey(chunkX, chunkZ));
  }
}

class BlockingGeneratedChunkStorage implements ChunkStorage {
  public readonly snapshots = new Map<string, PackedChunkSnapshot>();
  public readonly generatedRecords = new Map<string, GeneratedChunkStorageRecord>();
  public generatedSaveStarted = 0;
  private readonly pendingGeneratedSaveResolvers: Array<() => void> = [];
  private generatedSavesReleased = false;

  public async loadChunk(chunkX: number, chunkZ: number): Promise<PackedChunkSnapshot | undefined> {
    const snapshot = this.snapshots.get(chunkKey(chunkX, chunkZ));
    return snapshot === undefined ? undefined : clonePackedChunkSnapshot(snapshot);
  }

  public async saveChunk(snapshot: PackedChunkSnapshot): Promise<void> {
    this.snapshots.set(chunkKey(snapshot.chunkX, snapshot.chunkZ), clonePackedChunkSnapshot(snapshot));
  }

  public async loadGeneratedChunk(chunkX: number, chunkZ: number): Promise<GeneratedChunkStorageRecord | undefined> {
    const record = this.generatedRecords.get(chunkKey(chunkX, chunkZ));
    return record === undefined ? undefined : cloneGeneratedChunkStorageRecord(record);
  }

  public async saveGeneratedChunk(record: GeneratedChunkStorageRecord): Promise<void> {
    this.generatedSaveStarted++;
    if (this.generatedSavesReleased) {
      this.generatedRecords.set(chunkKey(record.chunkX, record.chunkZ), cloneGeneratedChunkStorageRecord(record));
      return;
    }

    await new Promise<void>((resolve) => {
      this.pendingGeneratedSaveResolvers.push(() => {
        this.generatedRecords.set(chunkKey(record.chunkX, record.chunkZ), cloneGeneratedChunkStorageRecord(record));
        resolve();
      });
    });
  }

  public async evictChunk(_chunkX: number, _chunkZ: number): Promise<void> {}

  public releaseAllGeneratedSaves(): void {
    this.generatedSavesReleased = true;
    while (this.pendingGeneratedSaveResolvers.length > 0) {
      this.pendingGeneratedSaveResolvers.shift()?.();
    }
  }
}

class BlockingGeneratedWorldStorageSession implements WorldStorageSession {
  public constructor(
    public readonly metadata: WorldSaveMetadata,
    public readonly chunks: BlockingGeneratedChunkStorage,
  ) {}

  public async close(): Promise<void> {}
}

class BlockingGeneratedWorldStorage implements WorldStorage {
  public readonly chunks = new BlockingGeneratedChunkStorage();
  public metadata: WorldSaveMetadata | undefined;

  public async openWorld(request: OpenWorldStorageRequest): Promise<WorldStorageSession> {
    this.metadata = createWorldSaveMetadata(request);
    return new BlockingGeneratedWorldStorageSession(this.metadata, this.chunks);
  }
}

class BlockingPreloadChunkStorage implements ChunkStorage {
  public generatedLoadStarted = 0;
  public chunkLoadStarted = 0;
  private firstGeneratedLoadBlocked = false;
  private firstGeneratedLoadResolver: (() => void) | undefined;

  public async loadChunk(_chunkX: number, _chunkZ: number): Promise<PackedChunkSnapshot | undefined> {
    this.chunkLoadStarted++;
    return undefined;
  }

  public async saveChunk(_snapshot: PackedChunkSnapshot): Promise<void> {}

  public async loadGeneratedChunk(_chunkX: number, _chunkZ: number): Promise<GeneratedChunkStorageRecord | undefined> {
    this.generatedLoadStarted++;
    if (!this.firstGeneratedLoadBlocked) {
      this.firstGeneratedLoadBlocked = true;
      await new Promise<void>((resolve) => {
        this.firstGeneratedLoadResolver = resolve;
      });
    }

    return undefined;
  }

  public async saveGeneratedChunk(_record: GeneratedChunkStorageRecord): Promise<void> {}

  public async evictChunk(_chunkX: number, _chunkZ: number): Promise<void> {}

  public releaseFirstGeneratedLoad(): void {
    this.firstGeneratedLoadResolver?.();
    this.firstGeneratedLoadResolver = undefined;
  }
}

class BlockingPreloadWorldStorageSession implements WorldStorageSession {
  public constructor(
    public readonly metadata: WorldSaveMetadata,
    public readonly chunks: BlockingPreloadChunkStorage,
  ) {}

  public async close(): Promise<void> {}
}

class BlockingPreloadWorldStorage implements WorldStorage {
  public readonly chunks = new BlockingPreloadChunkStorage();
  public metadata: WorldSaveMetadata | undefined;

  public async openWorld(request: OpenWorldStorageRequest): Promise<WorldStorageSession> {
    this.metadata = createWorldSaveMetadata(request);
    return new BlockingPreloadWorldStorageSession(this.metadata, this.chunks);
  }
}

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

function busyWait(durationMs: number): void {
  if (durationMs <= 0) {
    return;
  }

  const start = performance.now();
  while (performance.now() - start < durationMs) {
    // Test-only delay to create an overlapping in-flight cooperative status job.
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function chunkSnapshots(messages: readonly WorldHostMessage[]): ChunkSnapshotMessage[] {
  return messages.filter((message): message is ChunkSnapshotMessage => message.type === "chunk_snapshot");
}

function chunkUnloadCount(messages: readonly WorldHostMessage[]): number {
  return messages.filter((message) => message.type === "chunk_unload").length;
}

function worldPerformance(messages: readonly WorldHostMessage[]): WorldPerformanceMessage {
  const performance = messages.find((message): message is WorldPerformanceMessage => message.type === "world_perf");
  if (performance === undefined) {
    throw new Error("Expected world_perf message");
  }

  return performance;
}

async function readWorldgenPerformance(host: GeneratedWorldHost): Promise<WorldgenPerformanceCounters> {
  const performance = worldPerformance(await host.pollUpdates({ type: "poll_world_updates" })).performance.worldgen;
  if (performance === undefined) {
    throw new Error("Expected worldgen performance counters");
  }

  return performance;
}

function selectedCounts(counters: WorldgenPerformanceCounters): Record<TrackedWorldgenCount, number> {
  return Object.fromEntries(
    TRACKED_WORLDGEN_COUNTS.map((name) => [name, counters.counts[name] ?? 0]),
  ) as Record<TrackedWorldgenCount, number>;
}

function selectedCountDelta(
  before: WorldgenPerformanceCounters,
  after: WorldgenPerformanceCounters,
): Record<TrackedWorldgenCount, number> {
  const beforeCounts = selectedCounts(before);
  const afterCounts = selectedCounts(after);
  return Object.fromEntries(
    TRACKED_WORLDGEN_COUNTS.map((name) => [name, afterCounts[name] - beforeCounts[name]]),
  ) as Record<TrackedWorldgenCount, number>;
}

function countByExactStatus(
  records: readonly GeneratedChunkStatusDebugRecord[],
): Partial<Record<GeneratedChunkStatusName, number>> {
  const counts: Partial<Record<GeneratedChunkStatusName, number>> = {};
  for (const record of records) {
    counts[record.status] = (counts[record.status] ?? 0) + 1;
  }

  return counts;
}

function countChunkAccessByExactStatus(
  records: readonly GeneratedChunkAccessDebugRecord[],
): Partial<Record<GeneratedChunkStatusName, number>> {
  const counts: Partial<Record<GeneratedChunkStatusName, number>> = {};
  for (const record of records) {
    counts[record.status] = (counts[record.status] ?? 0) + 1;
  }

  return counts;
}

function countMaterialized(records: readonly GeneratedChunkStatusDebugRecord[]): number {
  return records.filter((record) => record.hasBlockSections).length;
}

function countMaterializedChunkAccess(records: readonly GeneratedChunkAccessDebugRecord[]): number {
  return records.filter((record) => record.hasBlockSections).length;
}

function createHost(options: {
  readonly chunkViewScheduling?: "synchronous" | "cooperative";
  readonly generatorOptions?: CountingWorldGeneratorOptions;
  readonly worldStorage?: WorldStorage;
  readonly nowMs?: () => number;
} = {}): { readonly host: GeneratedWorldHost; readonly generator: CountingWorldGenerator } {
  const blocks = registerGeneratedRenderBlocks();
  const generator = new CountingWorldGenerator(12345n, options.generatorOptions);
  const host = new GeneratedWorldHost({
    seed: 12345n,
    generator,
    airState: blocks.airState,
    blockStateById: blocks.blockStateById,
    blockStateIds: blocks.blockStateIds,
    chunkViewScheduling: options.chunkViewScheduling,
    worldStorage: options.worldStorage,
    nowMs: options.nowMs,
    lightingMode: "none",
    liquidSimulationMode: "none",
  });

  return { host, generator };
}

async function waitForFillFromNoiseCalls(
  generator: CountingWorldGenerator,
  expectedCalls: number,
  timeoutMs = 10_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (generator.snapshot().fillFromNoise >= expectedCalls) {
      return;
    }

    await sleep(1);
  }

  throw new Error(`Expected at least ${expectedCalls.toString()} fillFromNoise calls`);
}

async function waitForGeneratedSaveStart(
  storage: BlockingGeneratedChunkStorage,
  timeoutMs = 10_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (storage.generatedSaveStarted > 0) {
      return;
    }

    await sleep(1);
  }

  throw new Error("Expected a generated chunk save to start");
}

async function waitForGeneratedPreloadStart(
  storage: BlockingPreloadChunkStorage,
  timeoutMs = 10_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (storage.generatedLoadStarted > 0) {
      return;
    }

    await sleep(1);
  }

  throw new Error("Expected a generated chunk preload to start");
}

async function drainChunkSnapshots(
  host: GeneratedWorldHost,
  expectedCount: number,
  timeoutMs = 60_000,
): Promise<ChunkSnapshotMessage[]> {
  const snapshots: ChunkSnapshotMessage[] = [];
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    await sleep(10);
    const messages = await host.pollUpdates({ type: "poll_world_updates" });
    snapshots.push(...chunkSnapshots(messages));
    if (snapshots.length >= expectedCount) {
      return snapshots.slice(0, expectedCount);
    }
  }

  throw new Error(`Expected ${expectedCount.toString()} chunk snapshots, got ${snapshots.length.toString()}`);
}

describe("GeneratedWorldHost chunk-boundary generation counts", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("coalesces a flat-grass chunk-view crossing without regenerating overlapping chunks", async () => {
    const { host, generator } = createHost();
    await host.openWorld(OPEN_WORLD_REQUEST);

    const initialMessages = await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });
    const initialPerformance = await readWorldgenPerformance(host);

    expect(chunkSnapshots(initialMessages)).toHaveLength(25);
    expect(chunkUnloadCount(initialMessages)).toBe(0);
    expect(generator.snapshot()).toEqual({
      fillFromNoise: 121,
      uniqueFillFromNoise: 121,
      surfaceAndBedrock: 121,
      airCarvers: 121,
      liquidCarvers: 121,
      decorations: 81,
      uniqueDecorations: 81,
      mobSpawns: 25,
      uniqueMobSpawns: 25,
    });
    expect(selectedCounts(initialPerformance)).toEqual({
      storage_preload_already_loaded: 0,
      storage_preload_missing: 121,
      terrain_chunks_generated: 121,
      feature_chunks_decorated: 81,
      chunks_marked_full_without_lighting: 49,
      chunk_snapshots_published: 25,
      chunk_snapshots_built: 25,
      chunk_unloads_sent: 0,
    });

    const initialStatusRecords = host.getDebugChunkStatusRecords();
    expect(initialStatusRecords).toHaveLength(729);
    expect(countMaterialized(initialStatusRecords)).toBe(121);
    expect(countByExactStatus(initialStatusRecords)).toEqual({
      [GeneratedChunkStatus.STRUCTURE_STARTS]: 608,
      [GeneratedChunkStatus.LIQUID_CARVERS]: 40,
      [GeneratedChunkStatus.FEATURES]: 32,
      [GeneratedChunkStatus.FULL]: 49,
    });

    const initialAccessRecords = host.getDebugGeneratedChunkAccessRecords();
    expect(initialAccessRecords).toHaveLength(625);
    expect(countMaterializedChunkAccess(initialAccessRecords)).toBe(121);
    expect(initialAccessRecords.filter((record) => record.type === "level")).toHaveLength(49);
    expect(initialAccessRecords.every((record) => record.isUnsaved)).toBe(true);
    expect(countChunkAccessByExactStatus(initialAccessRecords)).toEqual({
      [GeneratedChunkStatus.STRUCTURE_STARTS]: 504,
      [GeneratedChunkStatus.LIQUID_CARVERS]: 40,
      [GeneratedChunkStatus.FEATURES]: 32,
      [GeneratedChunkStatus.FULL]: 49,
    });

    const repeatMessages = await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });
    const repeatPerformance = await readWorldgenPerformance(host);

    expect(chunkSnapshots(repeatMessages)).toHaveLength(0);
    expect(chunkUnloadCount(repeatMessages)).toBe(0);
    expect(generator.snapshot()).toEqual({
      fillFromNoise: 121,
      uniqueFillFromNoise: 121,
      surfaceAndBedrock: 121,
      airCarvers: 121,
      liquidCarvers: 121,
      decorations: 81,
      uniqueDecorations: 81,
      mobSpawns: 25,
      uniqueMobSpawns: 25,
    });
    expect(selectedCountDelta(initialPerformance, repeatPerformance)).toEqual({
      storage_preload_already_loaded: 0,
      storage_preload_missing: 0,
      terrain_chunks_generated: 0,
      feature_chunks_decorated: 0,
      chunks_marked_full_without_lighting: 0,
      chunk_snapshots_published: 0,
      chunk_snapshots_built: 0,
      chunk_unloads_sent: 0,
    });

    const movedMessages = await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 1,
      centerChunkZ: 0,
      radius: 0,
    });
    const movedPerformance = await readWorldgenPerformance(host);

    expect(chunkSnapshots(movedMessages)).toHaveLength(5);
    expect(chunkUnloadCount(movedMessages)).toBe(5);
    expect(generator.snapshot()).toEqual({
      fillFromNoise: 132,
      uniqueFillFromNoise: 132,
      surfaceAndBedrock: 132,
      airCarvers: 132,
      liquidCarvers: 132,
      decorations: 90,
      uniqueDecorations: 90,
      mobSpawns: 30,
      uniqueMobSpawns: 30,
    });
    expect(selectedCountDelta(repeatPerformance, movedPerformance)).toEqual({
      storage_preload_already_loaded: 0,
      storage_preload_missing: 11,
      terrain_chunks_generated: 11,
      feature_chunks_decorated: 9,
      chunks_marked_full_without_lighting: 7,
      chunk_snapshots_published: 5,
      chunk_snapshots_built: 5,
      chunk_unloads_sent: 5,
    });

    const movedStatusRecords = host.getDebugChunkStatusRecords();
    expect(movedStatusRecords).toHaveLength(756);
    expect(movedPerformance.counts.chunk_holders_resident_current).toBeLessThanOrEqual(625);
    expect(movedPerformance.counts.chunk_holders_pruned_outside_authority ?? 0).toBeGreaterThan(0);
    expect(countMaterialized(movedStatusRecords)).toBe(132);
    expect(countByExactStatus(movedStatusRecords)).toEqual({
      [GeneratedChunkStatus.STRUCTURE_STARTS]: 624,
      [GeneratedChunkStatus.LIQUID_CARVERS]: 42,
      [GeneratedChunkStatus.FEATURES]: 34,
      [GeneratedChunkStatus.FULL]: 56,
    });

    const movedAccessRecords = host.getDebugGeneratedChunkAccessRecords();
    expect(movedAccessRecords).toHaveLength(625);
    expect(countMaterializedChunkAccess(movedAccessRecords)).toBe(132);
    expect(movedAccessRecords.filter((record) => record.type === "level")).toHaveLength(56);
    expect(movedAccessRecords.every((record) => record.isUnsaved)).toBe(true);
    expect(countChunkAccessByExactStatus(movedAccessRecords)).toEqual({
      [GeneratedChunkStatus.STRUCTURE_STARTS]: 493,
      [GeneratedChunkStatus.LIQUID_CARVERS]: 42,
      [GeneratedChunkStatus.FEATURES]: 34,
      [GeneratedChunkStatus.FULL]: 56,
    });

    await host.flushStorageSideEffects();
    const settledAccessRecords = host.getDebugGeneratedChunkAccessRecords();
    expect(settledAccessRecords).toHaveLength(625);
    expect(countMaterializedChunkAccess(settledAccessRecords)).toBe(132);
    expect(settledAccessRecords.every((record) => !record.isUnsaved)).toBe(true);
    expect(countChunkAccessByExactStatus(settledAccessRecords)).toEqual({
      [GeneratedChunkStatus.STRUCTURE_STARTS]: 493,
      [GeneratedChunkStatus.LIQUID_CARVERS]: 42,
      [GeneratedChunkStatus.FEATURES]: 34,
      [GeneratedChunkStatus.FULL]: 56,
    });
  });

  test("budgets holder unloads and settles chunk access state after flush", async () => {
    const { host } = createHost({ nowMs: () => 0 });
    await host.openWorld(OPEN_WORLD_REQUEST);

    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 64,
      centerChunkZ: 0,
      radius: 0,
    });
    const movedPerformance = await readWorldgenPerformance(host);

    expect(movedPerformance.counts.chunk_holders_unload_queued).toBe(625);
    expect(movedPerformance.counts.chunk_holders_unload_processed).toBe(200);
    expect(movedPerformance.counts.chunk_holders_unload_queue_current).toBe(425);
    expect(movedPerformance.counts.chunk_holders_resident_current).toBe(1050);
    expect(host.getDebugGeneratedChunkAccessRecords()).toHaveLength(1050);

    await host.flushStorageSideEffects();
    const settledPerformance = await readWorldgenPerformance(host);
    const settledAccessRecords = host.getDebugGeneratedChunkAccessRecords();

    expect(settledPerformance.counts.chunk_holders_unload_queue_current).toBe(0);
    expect(settledPerformance.counts.chunk_holders_resident_current).toBe(625);
    expect(settledAccessRecords).toHaveLength(625);
    expect(countMaterializedChunkAccess(settledAccessRecords)).toBe(121);
    expect(settledAccessRecords.every((record) => !record.isUnsaved)).toBe(true);
    expect(countChunkAccessByExactStatus(settledAccessRecords)).toEqual({
      [GeneratedChunkStatus.STRUCTURE_STARTS]: 504,
      [GeneratedChunkStatus.LIQUID_CARVERS]: 40,
      [GeneratedChunkStatus.FEATURES]: 32,
      [GeneratedChunkStatus.FULL]: 49,
    });
  }, 60_000);

  test("drops stale preload results and settles expected chunk state after a walk", async () => {
    const storage = new BlockingPreloadWorldStorage();
    const { host } = createHost({ worldStorage: storage });
    await host.openWorld(OPEN_WORLD_REQUEST);

    const staleViewPromise = host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });
    await waitForGeneratedPreloadStart(storage.chunks);

    const movedMessages = await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 1,
      centerChunkZ: 0,
      radius: 0,
    });
    storage.chunks.releaseFirstGeneratedLoad();
    const staleMessages = await staleViewPromise;
    await host.flushStorageSideEffects();
    const performance = await readWorldgenPerformance(host);

    expect(chunkSnapshots(movedMessages)).toHaveLength(25);
    expect(chunkSnapshots(staleMessages)).toHaveLength(0);
    expect(performance.counts.storage_preload_skipped_stale ?? 0).toBeGreaterThan(0);
    expect(performance.counts.storage_preload_skipped_revision_changed ?? 0).toBeGreaterThan(0);

    const settledStatusRecords = host.getDebugChunkStatusRecords();
    expect(settledStatusRecords).toHaveLength(729);
    expect(countMaterialized(settledStatusRecords)).toBe(121);
    expect(countByExactStatus(settledStatusRecords)).toEqual({
      [GeneratedChunkStatus.STRUCTURE_STARTS]: 608,
      [GeneratedChunkStatus.LIQUID_CARVERS]: 40,
      [GeneratedChunkStatus.FEATURES]: 32,
      [GeneratedChunkStatus.FULL]: 49,
    });

    const settledAccessRecords = host.getDebugGeneratedChunkAccessRecords();
    expect(settledAccessRecords).toHaveLength(625);
    expect(countMaterializedChunkAccess(settledAccessRecords)).toBe(121);
    expect(settledAccessRecords.every((record) => !record.isUnsaved)).toBe(true);
    expect(countChunkAccessByExactStatus(settledAccessRecords)).toEqual({
      [GeneratedChunkStatus.STRUCTURE_STARTS]: 504,
      [GeneratedChunkStatus.LIQUID_CARVERS]: 40,
      [GeneratedChunkStatus.FEATURES]: 32,
      [GeneratedChunkStatus.FULL]: 49,
    });
  });

  test("coalesces overlapping cooperative status work when the same view is requested twice", async () => {
    const { host, generator } = createHost({
      chunkViewScheduling: "cooperative",
      generatorOptions: { slowFirstTerrainPhaseMs: 100 },
    });
    await host.openWorld(OPEN_WORLD_REQUEST);

    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });
    await waitForFillFromNoiseCalls(generator, 1);

    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });

    const snapshots = await drainChunkSnapshots(host, 25);
    const performance = await readWorldgenPerformance(host);
    const generatorSnapshot = generator.snapshot();

    expect(snapshots).toHaveLength(25);
    expect(performance.counts.storage_preload_missing).toBeLessThan(242);
    expect(performance.counts["status_coalesced_pending.features"] ?? 0).toBeGreaterThan(0);
    expect(performance.counts["status_jobs_started.liquid_carvers"]).toBe(performance.counts.terrain_chunks_generated);
    expect(generatorSnapshot.fillFromNoise).toBe(generatorSnapshot.uniqueFillFromNoise);
    expect(generatorSnapshot.decorations).toBe(generatorSnapshot.uniqueDecorations);
  }, 60_000);

  test("reloads saved partial statuses without rerunning completed terrain or features", async () => {
    const storage = new MemoryWorldStorage(() => 10_000);
    const saveId = createGeneratedWorldSaveId(12345n, "flat_grass", OPEN_WORLD_REQUEST.config);
    const first = createHost({ worldStorage: storage });
    await first.host.openWorld(OPEN_WORLD_REQUEST);
    await first.host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });
    await first.host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 32,
      centerChunkZ: 0,
      radius: 0,
    });
    await first.host.flushStorageSideEffects();

    expect(storage.getGeneratedChunkRecord(saveId, 5, 0)?.generatedChunk.status).toBe(GeneratedChunkStatus.LIQUID_CARVERS);
    expect(storage.getGeneratedChunkRecord(saveId, 4, 0)?.generatedChunk.status).toBe(GeneratedChunkStatus.FEATURES);

    const second = createHost({ worldStorage: storage });
    await second.host.openWorld(OPEN_WORLD_REQUEST);
    await second.host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 2,
      centerChunkZ: 0,
      radius: 0,
    });

    expect(second.generator.hasFillFromNoiseCall(5, 0)).toBe(false);
    expect(second.generator.hasDecorationCall(4, 0)).toBe(false);
    expect(second.host.getDebugGeneratedChunkAccessRecords().find((record) => record.chunkX === 5 && record.chunkZ === 0)?.status)
      .toBe(GeneratedChunkStatus.FULL);
    expect(second.host.getDebugGeneratedChunkAccessRecords().find((record) => record.chunkX === 4 && record.chunkZ === 0)?.status)
      .toBe(GeneratedChunkStatus.FULL);
  });

  test("resurrects pending-unload holders instead of reloading stale storage", async () => {
    const storage = new BlockingGeneratedWorldStorage();
    const { host, generator } = createHost({ worldStorage: storage });
    await host.openWorld(OPEN_WORLD_REQUEST);
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });
    expect(generator.fillFromNoiseCallCount(0, 0)).toBe(1);

    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 32,
      centerChunkZ: 0,
      radius: 0,
    });
    await waitForGeneratedSaveStart(storage.chunks);
    expect(storage.chunks.generatedRecords.size).toBe(0);

    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });
    const performance = await readWorldgenPerformance(host);

    expect(generator.fillFromNoiseCallCount(0, 0)).toBe(1);
    expect(performance.counts.chunk_holders_pending_unload_resurrected ?? 0).toBeGreaterThan(0);

    storage.chunks.releaseAllGeneratedSaves();
    await host.flushStorageSideEffects();
  }, 60_000);
});
