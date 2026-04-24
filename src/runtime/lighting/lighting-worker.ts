import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import { DataLayer } from "../../world/level/chunk/data-layer";
import { LevelChunk } from "../../world/level/chunk/level-chunk";
import type { LightChunkGetter } from "../../world/level/chunk/light-chunk-getter";
import type { BlockGetter } from "../../world/level/block-getter";
import type { ChunkLightSectionSnapshot } from "../../world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../../world/level/generated-render-blocks";
import { LightLayer } from "../../world/level/light-layer";
import type { LevelHeightAccessorLike } from "../../world/level/light-section";
import { LevelLightEngine } from "../../world/level/lighting/level-light-engine";
import type { PackedChunkLight, PackedLightSectionUpdate } from "../../world/level/packed-chunk-snapshot";
import { StaticRenderLevel } from "../../world/level/static-render-level";
import { BLOCKS_PER_SECTION, CHUNK_WIDTH, SECTION_HEIGHT } from "../../worldgen/chunk/chunk-block-buffer";
import {
  type ChunkLightDeltaResult,
  type ChunkLightReadyResult,
  type ConfigureLightingWorldRequest,
  type LightBlockChangeBatchRequest,
  type LightErrorResult,
  type LightProgressResult,
  type LightingCommandRequest,
  type LightingChunkRevision,
  type LightingResult,
  type LightingWorkerRequest,
  type LightingWorkerResponse,
  type RemoveLightChunkRequest,
  type RequestInitialLightRequest,
  type SetLightingViewRequest,
  type UpsertLightChunkRequest,
} from "./lighting-protocol";
import { connectLightingWorkerSession, type LightingWorkerHostEndpoint } from "./lighting-worker-client";

const DEFAULT_MAX_QUEUED_COMMANDS = 1024;
const DEFAULT_MAX_QUEUED_RESULTS = 1024;

export interface LightingWorkerHandlerOptions {
  readonly maxQueuedCommands?: number;
  readonly maxQueuedResults?: number;
}

interface LightingWorkerState {
  configuredWorld?: ConfigureLightingWorldRequest;
  view?: SetLightingViewRequest;
  world?: LightingWorkerWorld;
  readonly chunkRevisions: Map<string, number>;
}

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

function yieldMailboxTurn(): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, 0);
  });
}

function lightError(message: string, request?: RequestInitialLightRequest | RemoveLightChunkRequest): LightErrorResult {
  return request === undefined
    ? { type: "light_error", message }
    : {
        type: "light_error",
        message,
        chunkViewRevision: request.chunkViewRevision,
        chunkX: request.chunkX,
        chunkZ: request.chunkZ,
        chunkRevision: request.chunkRevision,
      };
}

function lightProgress(
  stage: string,
  current: number,
  total: number,
  request?: RequestInitialLightRequest,
): LightProgressResult {
  return request === undefined
    ? { type: "light_progress", stage, current, total }
    : {
        type: "light_progress",
        stage,
        current,
        total,
        chunkViewRevision: request.chunkViewRevision,
        chunkX: request.chunkX,
        chunkZ: request.chunkZ,
        chunkRevision: request.chunkRevision,
      };
}

class WorkerLightChunkGetter implements LightChunkGetter {
  public constructor(
    private readonly level: StaticRenderLevel,
    private readonly onLightUpdateCallback: (layer: LightLayer, section: SectionPos) => void,
  ) {}

  public getChunkForLighting(chunkX: number, chunkZ: number): BlockGetter | null {
    return this.level.getChunk(chunkX, chunkZ, false);
  }

  public onLightUpdate(layer: LightLayer, section: SectionPos): void {
    this.onLightUpdateCallback(layer, section);
  }

  public getLevel(): BlockGetter & LevelHeightAccessorLike {
    return this.level;
  }
}

class LightingWorkerWorld {
  private readonly blocks = registerGeneratedRenderBlocks();
  private readonly level: StaticRenderLevel;
  private readonly lightEngine: LevelLightEngine;
  private readonly initializedChunks = new Set<string>();
  private readonly lightCorrectChunks = new Set<string>();
  private readonly dirtyLightSections = new Map<string, { readonly layer: LightLayer; readonly section: SectionPos }>();

  public constructor(config: ConfigureLightingWorldRequest) {
    this.level = new StaticRenderLevel(
      this.blocks.airState,
      15,
      15,
      config.minBuildHeight,
      config.height,
    );
    this.lightEngine = new LevelLightEngine(
      new WorkerLightChunkGetter(this.level, (layer, section) => this.markLightSectionDirty(layer, section)),
      true,
      true,
    );
  }

  public upsertChunk(request: UpsertLightChunkRequest): void {
    const key = chunkKey(request.chunkX, request.chunkZ);
    this.discardChunkLighting(request.chunkX, request.chunkZ);
    this.level.setChunk(this.hydrateChunk(request));
    this.initializedChunks.delete(key);
    this.lightCorrectChunks.delete(key);
  }

  public removeChunk(request: RemoveLightChunkRequest): void {
    this.level.removeChunk(request.chunkX, request.chunkZ);
    this.discardChunkLighting(request.chunkX, request.chunkZ);
  }

  public hasChunkRevision(request: { readonly chunkX: number; readonly chunkZ: number; readonly chunkRevision: number }, revisions: ReadonlyMap<string, number>): boolean {
    return revisions.get(chunkKey(request.chunkX, request.chunkZ)) === request.chunkRevision
      && this.level.getChunk(request.chunkX, request.chunkZ, false) !== null;
  }

  public async computeInitialLight(request: RequestInitialLightRequest, yieldStep: () => Promise<void>): Promise<ChunkLightReadyResult> {
    const dependencies = [
      { chunkX: request.chunkX, chunkZ: request.chunkZ },
      ...request.neighbors.map((neighbor) => ({ chunkX: neighbor.chunkX, chunkZ: neighbor.chunkZ })),
    ];
    const chunks: LevelChunk[] = [];
    for (const dependency of dependencies) {
      const chunk = this.level.getChunk(dependency.chunkX, dependency.chunkZ, false);
      if (chunk === null) {
        throw new Error(`Missing light input chunk (${dependency.chunkX.toString()}, ${dependency.chunkZ.toString()})`);
      }

      chunks.push(chunk);
    }

    let queuedWork = false;
    for (const chunk of chunks) {
      await yieldStep();
      const key = chunkKey(chunk.chunkX, chunk.chunkZ);
      if (this.initializedChunks.has(key)) {
        continue;
      }

      this.initializeChunkLighting(chunk);
      this.initializedChunks.add(key);
      queuedWork = true;
    }

    if (queuedWork || this.lightEngine.hasLightWork()) {
      await this.runLightingUntilIdle(yieldStep);
    }

    this.lightCorrectChunks.add(chunkKey(request.chunkX, request.chunkZ));
    const center = this.level.getChunk(request.chunkX, request.chunkZ, false);
    if (center === null) {
      throw new Error(`Missing lit output chunk (${request.chunkX.toString()}, ${request.chunkZ.toString()})`);
    }

    return {
      type: "chunk_light_ready",
      chunkViewRevision: request.chunkViewRevision,
      chunkX: request.chunkX,
      chunkZ: request.chunkZ,
      chunkRevision: request.chunkRevision,
      light: this.buildChunkLightSnapshot(center),
    };
  }

  public async applyBlockChanges(request: LightBlockChangeBatchRequest, yieldStep: () => Promise<void>): Promise<readonly ChunkLightDeltaResult[]> {
    for (const change of request.changes) {
      const pos = new BlockPos(change.x, change.y, change.z);
      const chunkX = SectionPos.blockToSectionCoord(change.x);
      const chunkZ = SectionPos.blockToSectionCoord(change.z);
      const chunk = this.level.getChunk(chunkX, chunkZ, false);
      if (chunk === null) {
        continue;
      }

      const oldState = this.blocks.blockStateById[change.oldBlockStateId] ?? this.blocks.airState;
      const newState = this.blocks.blockStateById[change.newBlockStateId] ?? this.blocks.airState;
      chunk.setBlockState(pos, newState);
      this.queueBlockLightUpdate(pos, oldState.getLightEmission(), newState.getLightEmission());
      this.lightCorrectChunks.delete(chunkKey(chunkX, chunkZ));
    }

    if (this.lightEngine.hasLightWork()) {
      await this.runLightingUntilIdle(yieldStep);
    }

    for (const revision of request.chunkRevisions) {
      if (this.level.getChunk(revision.chunkX, revision.chunkZ, false) !== null) {
        this.lightCorrectChunks.add(chunkKey(revision.chunkX, revision.chunkZ));
      }
    }

    return this.drainDirtyLightDeltas(request.chunkRevisions);
  }

  private hydrateChunk(request: UpsertLightChunkRequest): LevelChunk {
    const chunk = new LevelChunk(request.chunkX, request.chunkZ, this.blocks.airState);
    const chunkBlockX = SectionPos.sectionToBlockCoord(request.chunkX);
    const chunkBlockZ = SectionPos.sectionToBlockCoord(request.chunkZ);
    const pos = new BlockPos.MutableBlockPos();

    for (const section of request.sections) {
      if (section.blockStateIds.length !== BLOCKS_PER_SECTION) {
        throw new Error(
          `Light input section (${request.chunkX.toString()}, ${section.y.toString()}, ${request.chunkZ.toString()}) had ${section.blockStateIds.length.toString()} block ids instead of ${BLOCKS_PER_SECTION.toString()}`,
        );
      }

      let index = 0;
      const sectionMinY = SectionPos.sectionToBlockCoord(section.y);
      for (let localY = 0; localY < SECTION_HEIGHT; localY++) {
        for (let localZ = 0; localZ < CHUNK_WIDTH; localZ++) {
          for (let localX = 0; localX < CHUNK_WIDTH; localX++) {
            const state = this.blocks.blockStateById[section.blockStateIds[index++]!] ?? this.blocks.airState;
            if (state.isAir()) {
              continue;
            }

            pos.set(chunkBlockX + localX, sectionMinY + localY, chunkBlockZ + localZ);
            chunk.setBlockState(pos, state);
          }
        }
      }
    }

    return chunk;
  }

  private initializeChunkLighting(chunk: LevelChunk): void {
    const nonEmptySections = new Set<number>();
    const lightEmitters: Array<{ readonly pos: BlockPos; readonly lightEmission: number }> = [];
    for (const entry of chunk.getBlockEntries()) {
      nonEmptySections.add(SectionPos.blockToSectionCoord(entry.pos.getY()));
      const lightEmission = entry.state.getLightEmission();
      if (lightEmission > 0) {
        lightEmitters.push({ pos: entry.pos, lightEmission });
      }
    }

    const minSection = this.level.getMinSection();
    const sectionCount = this.level.getSectionsCount();
    for (let sectionOffset = 0; sectionOffset < sectionCount; sectionOffset++) {
      const sectionY = minSection + sectionOffset;
      this.lightEngine.updateSectionStatus(
        SectionPos.of(chunk.chunkX, sectionY, chunk.chunkZ),
        !nonEmptySections.has(sectionY),
      );
    }

    this.lightEngine.enableLightSources({ x: chunk.chunkX, z: chunk.chunkZ }, true);
    for (const emitter of lightEmitters) {
      this.lightEngine.onBlockEmissionIncrease(emitter.pos, emitter.lightEmission);
    }
  }

  private queueBlockLightUpdate(pos: BlockPos, oldEmission: number, newEmission: number): void {
    const section = SectionPos.fromBlockPos(pos);
    const chunk = this.level.getChunk(section.x(), section.z(), false);
    if (chunk !== null) {
      const sectionMinY = SectionPos.sectionToBlockCoord(section.y());
      this.lightEngine.updateSectionStatus(section, chunk.isYSpaceEmpty(sectionMinY, sectionMinY + 15));
    }

    this.lightEngine.checkBlock(pos);
    if (newEmission > oldEmission) {
      this.lightEngine.onBlockEmissionIncrease(pos, newEmission);
    }
  }

  private discardChunkLighting(chunkX: number, chunkZ: number): void {
    const key = chunkKey(chunkX, chunkZ);
    this.initializedChunks.delete(key);
    this.lightCorrectChunks.delete(key);
    this.clearDirtyLightSectionsForChunk(chunkX, chunkZ);

    this.lightEngine.retainData({ x: chunkX, z: chunkZ }, false);
    this.lightEngine.enableLightSources({ x: chunkX, z: chunkZ }, false);

    for (let sectionY = this.lightEngine.getMinLightSection(); sectionY < this.lightEngine.getMaxLightSection(); sectionY++) {
      const section = SectionPos.of(chunkX, sectionY, chunkZ);
      this.lightEngine.queueSectionData(LightLayer.BLOCK, section, undefined, true);
      this.lightEngine.queueSectionData(LightLayer.SKY, section, undefined, true);
    }

    const minSection = this.level.getMinSection();
    const sectionCount = this.level.getSectionsCount();
    for (let sectionOffset = 0; sectionOffset < sectionCount; sectionOffset++) {
      this.lightEngine.updateSectionStatus(SectionPos.of(chunkX, minSection + sectionOffset, chunkZ), true);
    }
  }

  private async runLightingUntilIdle(yieldStep: () => Promise<void>): Promise<void> {
    for (let iteration = 0; iteration < 10_000; iteration++) {
      if (!this.lightEngine.hasLightWork()) {
        return;
      }

      this.lightEngine.runUpdates(262_144, true, true);
      await yieldStep();
    }

    throw new Error("Lighting worker engine did not become idle");
  }

  private buildChunkLightSnapshot(chunk: LevelChunk): PackedChunkLight {
    return {
      sky: this.collectChunkLightSections(LightLayer.SKY, chunk),
      block: this.collectChunkLightSections(LightLayer.BLOCK, chunk),
      lightCorrect: this.lightCorrectChunks.has(chunkKey(chunk.chunkX, chunk.chunkZ)),
    };
  }

  private collectChunkLightSections(layer: LightLayer, chunk: LevelChunk): ChunkLightSectionSnapshot[] {
    const listener = this.lightEngine.getLayerListener(layer);
    const sections: ChunkLightSectionSnapshot[] = [];
    for (let sectionY = this.lightEngine.getMinLightSection(); sectionY < this.lightEngine.getMaxLightSection(); sectionY++) {
      const dataLayer = listener.getDataLayerData(SectionPos.of(chunk.chunkX, sectionY, chunk.chunkZ));
      if (dataLayer === undefined) {
        continue;
      }

      sections.push({
        y: sectionY,
        data: this.copyDataLayerBytes(dataLayer),
      });
    }

    return sections;
  }

  private drainDirtyLightDeltas(revisions: readonly LightingChunkRevision[]): readonly ChunkLightDeltaResult[] {
    const revisionByChunk = new Map(revisions.map((revision) => [chunkKey(revision.chunkX, revision.chunkZ), revision] as const));
    const grouped = new Map<string, {
      readonly revision: LightingChunkRevision;
      readonly sky: PackedLightSectionUpdate[];
      readonly block: PackedLightSectionUpdate[];
    }>();
    const dirty = [...this.dirtyLightSections.values()];
    this.dirtyLightSections.clear();

    for (const record of dirty) {
      const chunkX = record.section.x();
      const chunkZ = record.section.z();
      const key = chunkKey(chunkX, chunkZ);
      const revision = revisionByChunk.get(key);
      if (revision === undefined) {
        continue;
      }

      const dataLayer = this.lightEngine.getLayerListener(record.layer).getDataLayerData(record.section);
      if (dataLayer === undefined) {
        continue;
      }

      let group = grouped.get(key);
      if (group === undefined) {
        group = { revision, sky: [], block: [] };
        grouped.set(key, group);
      }

      const update = dataLayer.isEmpty()
        ? { y: record.section.y() }
        : { y: record.section.y(), data: this.copyDataLayerBytes(dataLayer) };
      if (record.layer === LightLayer.SKY) {
        group.sky.push(update);
      } else {
        group.block.push(update);
      }
    }

    return [...grouped.values()]
      .sort((left, right) => left.revision.chunkZ - right.revision.chunkZ || left.revision.chunkX - right.revision.chunkX)
      .map((group) => ({
        type: "chunk_light_delta",
        chunkViewRevision: group.revision.chunkViewRevision,
        chunkX: group.revision.chunkX,
        chunkZ: group.revision.chunkZ,
        chunkRevision: group.revision.chunkRevision,
        light: {
          sky: group.sky.length === 0 ? undefined : group.sky.sort((left, right) => left.y - right.y),
          block: group.block.length === 0 ? undefined : group.block.sort((left, right) => left.y - right.y),
        },
      }));
  }

  private markLightSectionDirty(layer: LightLayer, section: SectionPos): void {
    this.dirtyLightSections.set(
      `${layer},${section.x()},${section.y()},${section.z()}`,
      { layer, section },
    );
  }

  private clearDirtyLightSectionsForChunk(chunkX: number, chunkZ: number): void {
    for (const [key, record] of this.dirtyLightSections) {
      if (record.section.x() === chunkX && record.section.z() === chunkZ) {
        this.dirtyLightSections.delete(key);
      }
    }
  }

  private copyDataLayerBytes(dataLayer: DataLayer): Uint8Array {
    return new Uint8Array(dataLayer.getData());
  }
}

class LightingWorkerMailbox {
  private readonly maxQueuedCommands: number;
  private readonly maxQueuedResults: number;
  private readonly queuedCommands: LightingCommandRequest[] = [];
  private readonly resultQueue: LightingResult[] = [];
  private readonly state: LightingWorkerState = {
    chunkRevisions: new Map(),
  };
  private activeCommandCount = 0;
  private drainingCommands = false;

  public constructor(options: LightingWorkerHandlerOptions = {}) {
    this.maxQueuedCommands = options.maxQueuedCommands ?? DEFAULT_MAX_QUEUED_COMMANDS;
    this.maxQueuedResults = options.maxQueuedResults ?? DEFAULT_MAX_QUEUED_RESULTS;
  }

  public enqueueCommand(message: LightingCommandRequest): void {
    if (this.activeCommandCount >= this.maxQueuedCommands) {
      throw new Error(`lighting mailbox full (${this.activeCommandCount.toString()}/${this.maxQueuedCommands.toString()} commands)`);
    }

    this.activeCommandCount++;
    this.queuedCommands.push(message);
    if (!this.drainingCommands) {
      this.drainingCommands = true;
      queueMicrotask(() => {
        void this.drainCommands();
      });
    }
  }

  public pollResults(maxResults: number | undefined): LightingWorkerResponse {
    const resultLimit = maxResults === undefined ? this.resultQueue.length : Math.max(0, maxResults);
    const results = this.resultQueue.splice(0, resultLimit);
    return {
      type: "lighting_result_batch",
      results,
      pendingResultCount: this.resultQueue.length,
    };
  }

  private async drainCommands(): Promise<void> {
    try {
      while (this.queuedCommands.length > 0) {
        const command = this.queuedCommands.shift()!;
        try {
          await this.processCommand(command);
        } catch (error) {
          this.enqueueResult(lightError(error instanceof Error ? error.message : String(error)));
        } finally {
          this.activeCommandCount--;
        }

        await yieldMailboxTurn();
      }
    } finally {
      this.drainingCommands = false;
      if (this.queuedCommands.length > 0) {
        this.drainingCommands = true;
        queueMicrotask(() => {
          void this.drainCommands();
        });
      }
    }
  }

  private async processCommand(command: LightingCommandRequest): Promise<void> {
    switch (command.type) {
      case "configure_light_world":
        this.state.configuredWorld = command;
        this.state.world = new LightingWorkerWorld(command);
        this.state.chunkRevisions.clear();
        this.enqueueResult(lightProgress("configured", 1, 1));
        break;
      case "set_light_view":
        this.state.view = command;
        this.enqueueResult(lightProgress("view_updated", 1, 1));
        break;
      case "upsert_light_chunk":
        this.processUpsertChunk(command);
        break;
      case "remove_light_chunk":
        this.processRemoveChunk(command);
        break;
      case "request_initial_light":
        await this.processInitialLightRequest(command);
        break;
      case "block_light_update_batch":
        await this.processBlockLightUpdateBatch(command);
        break;
    }
  }

  private processUpsertChunk(command: UpsertLightChunkRequest): void {
    const world = this.requireWorld(command.type);
    world.upsertChunk(command);
    this.state.chunkRevisions.set(chunkKey(command.chunkX, command.chunkZ), command.chunkRevision);
  }

  private processRemoveChunk(command: RemoveLightChunkRequest): void {
    const key = chunkKey(command.chunkX, command.chunkZ);
    if (this.state.chunkRevisions.get(key) === command.chunkRevision) {
      this.requireWorld(command.type).removeChunk(command);
      this.state.chunkRevisions.delete(key);
    }
  }

  private async processInitialLightRequest(command: RequestInitialLightRequest): Promise<void> {
    const world = this.state.world;
    if (world === undefined) {
      this.enqueueResult(lightError("request_initial_light received before configure_light_world", command));
      return;
    }

    const required = [
      {
        chunkX: command.chunkX,
        chunkZ: command.chunkZ,
        chunkRevision: command.chunkRevision,
      },
      ...command.neighbors,
    ];
    let ready = 0;
    for (const dependency of required) {
      if (world.hasChunkRevision(dependency, this.state.chunkRevisions)) {
        ready++;
      }
    }

    this.enqueueResult(lightProgress(
      ready === required.length ? "initial_light_ready_for_solver" : "initial_light_waiting_for_inputs",
      ready,
      required.length,
      command,
    ));
    if (ready !== required.length) {
      return;
    }

    this.enqueueResult(await world.computeInitialLight(command, yieldMailboxTurn));
  }

  private async processBlockLightUpdateBatch(command: LightBlockChangeBatchRequest): Promise<void> {
    const world = this.state.world;
    if (world === undefined) {
      this.enqueueResult(lightError("block_light_update_batch received before configure_light_world"));
      return;
    }

    for (const delta of await world.applyBlockChanges(command, yieldMailboxTurn)) {
      this.enqueueResult(delta);
    }
    for (const revision of command.chunkRevisions) {
      this.state.chunkRevisions.set(chunkKey(revision.chunkX, revision.chunkZ), revision.chunkRevision);
    }
    this.enqueueResult({
      type: "block_light_update_complete",
      batchId: command.batchId,
      chunkViewRevision: command.chunkViewRevision,
      changeCount: command.changes.length,
      chunkRevisions: command.chunkRevisions,
    });
  }

  private requireWorld(messageType: string): LightingWorkerWorld {
    const world = this.state.world;
    if (world === undefined) {
      throw new Error(`${messageType} received before configure_light_world`);
    }

    return world;
  }

  private enqueueResult(result: LightingResult): void {
    if (this.maxQueuedResults <= 0) {
      return;
    }

    while (this.resultQueue.length >= this.maxQueuedResults) {
      const progressIndex = this.resultQueue.findIndex((queued) => queued.type === "light_progress");
      this.resultQueue.splice(progressIndex >= 0 ? progressIndex : 0, 1);
    }

    this.resultQueue.push(result);
  }
}

export function createLightingWorkerHandler(options: LightingWorkerHandlerOptions = {}): (message: LightingWorkerRequest) => Promise<LightingWorkerResponse> {
  const mailbox = new LightingWorkerMailbox(options);

  return async (message) => {
    if (message.type === "poll_light_results") {
      return mailbox.pollResults(message.maxResults);
    }

    mailbox.enqueueCommand(message);
    return { type: "lighting_ack" };
  };
}

const maybeWorkerGlobal = globalThis as Partial<LightingWorkerHostEndpoint>;
if (typeof maybeWorkerGlobal.postMessage === "function" && typeof maybeWorkerGlobal.addEventListener === "function") {
  connectLightingWorkerSession(
    maybeWorkerGlobal as LightingWorkerHostEndpoint,
    createLightingWorkerHandler(),
  );
}
