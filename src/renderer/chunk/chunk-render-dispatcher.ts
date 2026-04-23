import { BlockPos } from "../../core/block-pos";
import { Direction } from "../../core/direction";
import { SectionPos } from "../../core/section-pos";
import { ClientChunkCache } from "../../world/level/client-chunk-cache";
import { StaticRenderLevel } from "../../world/level/static-render-level";
import { AABB } from "../../world/phys/aabb";
import { Vec3 } from "../../world/phys/vec3";
import { BlockRenderDispatcher } from "../block/block-render-dispatcher";
import { ChunkBufferBuilderPack } from "../chunk-buffer-builder-pack";
import { LevelRenderer } from "../level-renderer";
import { RenderType } from "../render-type";
import { BufferBuilderSortState } from "../vertex/buffer-builder";
import { VertexBuffer } from "../vertex/vertex-buffer";
import { buildSectionMeshInput, decodeSectionMeshResult, withSectionMeshCamera, type SectionMeshInput } from "./chunk-mesh-protocol";
import { ChunkMeshWorkerClient } from "./mesh-worker-client";
import { RenderChunkRegion } from "./render-chunk-region";
import { beginChunkRenderLayer, buildSectionMesh } from "./section-mesh-compiler";
import { VisibilitySet } from "./visibility-set";

type Executor = (task: () => void) => void;

function sortTasks(left: ChunkRenderDispatcher.ChunkCompileTask, right: ChunkRenderDispatcher.ChunkCompileTask): number {
  return left.compareTo(right);
}

type RunnableTask = {
  readonly task: ChunkRenderDispatcher.ChunkCompileTask;
  readonly buffers: ChunkBufferBuilderPack | undefined;
};

export class ChunkRenderDispatcher {
  private readonly toBatch: ChunkRenderDispatcher.ChunkCompileTask[] = [];
  private readonly freeBuffers: ChunkBufferBuilderPack[];
  private readonly activeTasks = new Set<Promise<void>>();
  private readonly idleWaiters: Array<{
    resolve: () => void;
    reject: (error: unknown) => void;
  }> = [];
  private toBatchCount = 0;
  private freeBufferCount = 0;
  private freeMeshWorkerSlots: number;
  private taskFailure: unknown;
  private camera = Vec3.ZERO;
  private hasPendingUploads = false;

  public constructor(
    public level: StaticRenderLevel,
    public readonly renderer: LevelRenderer,
    public readonly blockRenderer: BlockRenderDispatcher,
    public readonly device: GPUDevice,
    private readonly executor: Executor = (task) => queueMicrotask(task),
    useAllCores = false,
    public readonly fixedBuffers = new ChunkBufferBuilderPack(),
    private readonly meshWorker?: ChunkMeshWorkerClient,
  ) {
    // WebGPU: Promise scheduling replaces ProcessorMailbox worker orchestration.
    const workerBufferCount = useAllCores ? 2 : 1;
    this.freeBuffers = new Array<ChunkBufferBuilderPack>(workerBufferCount);
    for (let index = 0; index < workerBufferCount; index++) {
      this.freeBuffers[index] = new ChunkBufferBuilderPack();
    }

    this.freeBufferCount = this.freeBuffers.length;
    this.freeMeshWorkerSlots = meshWorker?.concurrency ?? 0;
  }

  public setLevel(level: StaticRenderLevel): void {
    this.level = level;
  }

  public createRenderChunk(index: number): ChunkRenderDispatcher.RenderChunk {
    return new ChunkRenderDispatcher.RenderChunk(this, index);
  }

  private takeNextRunnableTask(): RunnableTask | undefined {
    for (let index = 0; index < this.toBatch.length; index++) {
      const task = this.toBatch[index]!;
      if (task.usesBuilderPack() && this.freeBuffers.length === 0) {
        continue;
      }

      if (task.usesMeshWorker() && this.freeMeshWorkerSlots === 0) {
        continue;
      }

      this.toBatch.splice(index, 1);
      this.toBatchCount = this.toBatch.length;

      let buffers: ChunkBufferBuilderPack | undefined;
      if (task.usesBuilderPack()) {
        buffers = this.freeBuffers.shift()!;
        this.freeBufferCount = this.freeBuffers.length;
      }

      if (task.usesMeshWorker()) {
        this.freeMeshWorkerSlots--;
      }

      return { task, buffers };
    }

    return undefined;
  }

  private runTask(): void {
    while (true) {
      const runnable = this.takeNextRunnableTask();
      if (runnable === undefined) {
        break;
      }

      let taskPromise: Promise<void>;
      taskPromise = this.executorTurn()
        .then(() => runnable.task.doTask(runnable.buffers))
        .then((result) => {
          if (runnable.buffers === undefined) {
            return;
          }

          if (result === ChunkRenderDispatcher.ChunkTaskResult.SUCCESSFUL) {
            runnable.buffers.clearAll();
          } else {
            runnable.buffers.discardAll();
          }
        })
        .catch((error) => {
          this.taskFailure = error;
        })
        .finally(() => {
          if (runnable.buffers !== undefined) {
            this.freeBuffers.push(runnable.buffers);
            this.freeBufferCount = this.freeBuffers.length;
          }

          if (runnable.task.usesMeshWorker()) {
            this.freeMeshWorkerSlots++;
          }

          this.activeTasks.delete(taskPromise);
          this.runTask();
          this.resolveIdleWaiters();
        });

      this.activeTasks.add(taskPromise);
    }

    this.resolveIdleWaiters();
  }

  private executorTurn(): Promise<void> {
    return new Promise((resolve) => {
      this.executor(resolve);
    });
  }

  private resolveIdleWaiters(): void {
    if (this.taskFailure !== undefined) {
      const error = this.taskFailure;
      this.taskFailure = undefined;
      while (this.idleWaiters.length > 0) {
        this.idleWaiters.shift()!.reject(error);
      }

      return;
    }

    if (!this.isQueueEmpty()) {
      return;
    }

    while (this.idleWaiters.length > 0) {
      this.idleWaiters.shift()!.resolve();
    }
  }

  public async awaitAllTasks(): Promise<void> {
    if (this.taskFailure !== undefined) {
      throw this.taskFailure;
    }

    if (this.isQueueEmpty()) {
      return;
    }

    await new Promise<void>((resolve, reject) => {
      this.idleWaiters.push({ resolve, reject });
    });
  }

  public getStats(): string {
    return `pC: ${this.toBatchCount.toString().padStart(3, "0")}, pU: 00, aB: ${this.freeBufferCount.toString().padStart(2, "0")}`;
  }

  public getToBatchCount(): number {
    return this.toBatchCount;
  }

  public getToUpload(): number {
    return 0;
  }

  public getFreeBufferCount(): number {
    return this.freeBufferCount;
  }

  public setCamera(camera: Vec3): void {
    this.camera = camera;
  }

  public getCameraPosition(): Vec3 {
    return this.camera;
  }

  public uploadAllPendingUploads(): boolean {
    // WebGPU: immediate uploads replace the GL upload queue with a one-bit "chunk data changed" signal.
    const hadPendingUploads = this.hasPendingUploads;
    this.hasPendingUploads = false;
    return hadPendingUploads;
  }

  public notePendingUploads(): void {
    this.hasPendingUploads = true;
  }

  public rebuildChunkSync(renderChunk: ChunkRenderDispatcher.RenderChunk): Promise<void> {
    return renderChunk.compileSync();
  }

  public blockUntilClear(): void {
    this.clearBatchQueue();
  }

  public schedule(task: ChunkRenderDispatcher.ChunkCompileTask): void {
    this.toBatch.push(task);
    this.toBatch.sort(sortTasks);
    this.toBatchCount = this.toBatch.length;
    this.runTask();
  }

  public async uploadChunkLayer(bufferBuilder: import("../vertex/buffer-builder").BufferBuilder, vertexBuffer: VertexBuffer): Promise<void> {
    // WebGPU: section uploads happen immediately on the main thread instead of through the deferred GL upload queue.
    vertexBuffer.upload(bufferBuilder);
    this.notePendingUploads();
  }

  public createSectionMeshInput(origin: BlockPos): SectionMeshInput | undefined {
    if (this.meshWorker === undefined || !(this.level instanceof ClientChunkCache)) {
      return undefined;
    }

    try {
      return buildSectionMeshInput(this.level, origin);
    } catch (error) {
      console.warn("Mesh worker section input build failed; falling back to main-thread compilation", error);
      return undefined;
    }
  }

  public async runMeshWorkerBuild(meshInput: SectionMeshInput, camera: Vec3): Promise<import("./chunk-mesh-protocol").SectionMeshResult> {
    if (this.meshWorker === undefined) {
      throw new Error("Mesh worker was not configured for this chunk render dispatcher");
    }

    return this.meshWorker.buildSectionMesh(withSectionMeshCamera(meshInput, camera));
  }

  private clearBatchQueue(): void {
    while (this.toBatch.length > 0) {
      this.toBatch.shift()!.cancel();
    }

    this.toBatchCount = 0;
  }

  public isQueueEmpty(): boolean {
    return this.toBatchCount === 0 && this.activeTasks.size === 0;
  }

  public dispose(): void {
    this.clearBatchQueue();
    this.meshWorker?.close();
    this.freeBuffers.length = 0;
  }
}

export namespace ChunkRenderDispatcher {
  export enum ChunkTaskResult {
    SUCCESSFUL = "successful",
    CANCELLED = "cancelled",
  }

  export class CompiledChunk {
    public static readonly UNCOMPILED = new (class extends CompiledChunk {
      public override facesCanSeeEachother(_from: Direction, _to: Direction): boolean {
        return false;
      }
    })();

    public readonly hasBlocks = new Set<RenderType>();
    public readonly hasLayer = new Set<RenderType>();
    public isCompletelyEmpty = true;
    public readonly renderableBlockEntities: readonly unknown[] = [];
    public visibilitySet = new VisibilitySet();
    public transparencyState: BufferBuilderSortState | undefined;

    public hasNoRenderableLayers(): boolean {
      return this.isCompletelyEmpty;
    }

    public isEmpty(renderType: RenderType): boolean {
      return !this.hasBlocks.has(renderType);
    }

    public getRenderableBlockEntities(): readonly unknown[] {
      return this.renderableBlockEntities;
    }

    public facesCanSeeEachother(from: Direction, to: Direction): boolean {
      return this.visibilitySet.visibilityBetween(from, to);
    }
  }

  export abstract class ChunkCompileTask {
    protected isCancelled = false;

    public constructor(
      protected readonly renderChunk: RenderChunk,
      protected readonly distAtCreation: number,
    ) {}

    public abstract doTask(buffers?: ChunkBufferBuilderPack): Promise<ChunkTaskResult>;

    public usesBuilderPack(): boolean {
      return true;
    }

    public usesMeshWorker(): boolean {
      return false;
    }

    public abstract cancel(): void;

    public compareTo(other: ChunkCompileTask): number {
      return this.distAtCreation - other.distAtCreation;
    }
  }

  export class RenderChunk {
    public static readonly SIZE = 16;

    public readonly compiled = { current: CompiledChunk.UNCOMPILED as CompiledChunk };
    private lastRebuildTask: RebuildTask | undefined;
    private lastResortTransparencyTask: ResortTransparencyTask | undefined;
    private readonly buffers: Map<RenderType, VertexBuffer>;
    private lastFrame = -1;
    private dirty = true;
    private readonly origin = new BlockPos.MutableBlockPos(-1, -1, -1);
    private readonly relativeOrigins = Direction.values().map(() => new BlockPos.MutableBlockPos());
    private playerChanged = false;
    public bb = new AABB(0, 0, 0, 16, 16, 16);

    public constructor(
      private readonly dispatcher: ChunkRenderDispatcher,
      public readonly index: number,
    ) {
      this.buffers = new Map<RenderType, VertexBuffer>(
        RenderType.chunkBufferLayers().map((renderType) => [renderType, new VertexBuffer(this.dispatcher.device)] as const),
      );
    }

    public getDispatcher(): ChunkRenderDispatcher {
      return this.dispatcher;
    }

    private doesChunkExistAt(pos: BlockPos): boolean {
      return this.dispatcher.level.getChunk(
        SectionPos.blockToSectionCoord(pos.getX()),
        SectionPos.blockToSectionCoord(pos.getZ()),
        false,
      ) !== null;
    }

    public hasAllNeighbors(): boolean {
      if (this.getDistToPlayerSqr() <= 576.0) {
        return true;
      }

      return this.doesChunkExistAt(this.relativeOrigins[Direction.WEST.get3DDataValue()]!)
        && this.doesChunkExistAt(this.relativeOrigins[Direction.NORTH.get3DDataValue()]!)
        && this.doesChunkExistAt(this.relativeOrigins[Direction.EAST.get3DDataValue()]!)
        && this.doesChunkExistAt(this.relativeOrigins[Direction.SOUTH.get3DDataValue()]!);
    }

    public setFrame(frame: number): boolean {
      if (this.lastFrame === frame) {
        return false;
      }

      this.lastFrame = frame;
      return true;
    }

    public getBuffer(renderType: RenderType): VertexBuffer {
      return this.buffers.get(renderType)!;
    }

    public setOrigin(x: number, y: number, z: number): void {
      if (x === this.origin.getX() && y === this.origin.getY() && z === this.origin.getZ()) {
        return;
      }

      this.reset();
      this.origin.set(x, y, z);
      this.bb = new AABB(x, y, z, x + 16, y + 16, z + 16);
      for (const direction of Direction.values()) {
        this.relativeOrigins[direction.get3DDataValue()]!
          .set(x, y, z)
          .move(direction.getStepX() * 16, direction.getStepY() * 16, direction.getStepZ() * 16);
      }
    }

    protected getDistToPlayerSqr(): number {
      const camera = this.dispatcher.getCameraPosition();
      const deltaX = (this.origin.getX() + 8) - camera.x;
      const deltaY = (this.origin.getY() + 8) - camera.y;
      const deltaZ = (this.origin.getZ() + 8) - camera.z;
      return (deltaX * deltaX) + (deltaY * deltaY) + (deltaZ * deltaZ);
    }

    public beginLayer(builder: import("../vertex/buffer-builder").BufferBuilder): void {
      beginChunkRenderLayer(builder);
    }

    public getCompiledChunk(): CompiledChunk {
      return this.compiled.current;
    }

    private reset(): void {
      this.cancelTasks();
      this.compiled.current = CompiledChunk.UNCOMPILED;
      this.dirty = true;
    }

    public releaseBuffers(): void {
      this.reset();
      for (const buffer of this.buffers.values()) {
        buffer.close();
      }
    }

    public getOrigin(): BlockPos {
      return this.origin;
    }

    public setDirty(playerChanged: boolean): void {
      const wasDirty = this.dirty;
      this.dirty = true;
      this.playerChanged = playerChanged || (wasDirty && this.playerChanged);
    }

    public setNotDirty(): void {
      this.dirty = false;
      this.playerChanged = false;
    }

    public isDirty(): boolean {
      return this.dirty;
    }

    public isDirtyFromPlayer(): boolean {
      return this.dirty && this.playerChanged;
    }

    public getRelativeOrigin(direction: Direction): BlockPos {
      return this.relativeOrigins[direction.get3DDataValue()]!;
    }

    public resortTransparency(renderType: RenderType, dispatcher: ChunkRenderDispatcher): boolean {
      const compiledChunk = this.getCompiledChunk();
      this.lastResortTransparencyTask?.cancel();
      if (!compiledChunk.hasLayer.has(renderType)) {
        return false;
      }

      this.lastResortTransparencyTask = new ResortTransparencyTask(this, this.getDistToPlayerSqr(), compiledChunk);
      dispatcher.schedule(this.lastResortTransparencyTask);
      return true;
    }

    protected cancelTasks(): void {
      this.lastRebuildTask?.cancel();
      this.lastRebuildTask = undefined;
      this.lastResortTransparencyTask?.cancel();
      this.lastResortTransparencyTask = undefined;
    }

    public createCompileTask(): ChunkCompileTask {
      this.cancelTasks();
      const origin = new BlockPos(this.origin.getX(), this.origin.getY(), this.origin.getZ());
      const meshInput = this.dispatcher.createSectionMeshInput(origin);
      const region = meshInput === undefined
        ? RenderChunkRegion.createIfNotEmpty(this.dispatcher.level, origin.offset(-1, -1, -1), origin.offset(16, 16, 16), 1)
        : null;
      this.lastRebuildTask = new RebuildTask(this, this.getDistToPlayerSqr(), region, meshInput);
      return this.lastRebuildTask;
    }

    public rebuildChunkAsync(dispatcher: ChunkRenderDispatcher): void {
      dispatcher.schedule(this.createCompileTask());
    }

    public updateGlobalBlockEntities(removed: ReadonlySet<unknown>, added: ReadonlySet<unknown>): void {
      this.dispatcher.renderer.updateGlobalBlockEntities(removed, added);
    }

    public async compileSync(): Promise<void> {
      await this.createCompileTask().doTask(this.dispatcher.fixedBuffers);
    }
  }

  export class RebuildTask extends ChunkCompileTask {
    public constructor(
      renderChunk: RenderChunk,
      distAtCreation: number,
      private region: RenderChunkRegion | null,
      private readonly meshInput?: SectionMeshInput,
    ) {
      super(renderChunk, distAtCreation);
    }

    public override usesBuilderPack(): boolean {
      return this.meshInput === undefined;
    }

    public override usesMeshWorker(): boolean {
      return this.meshInput !== undefined;
    }

    public override async doTask(buffers?: ChunkBufferBuilderPack): Promise<ChunkTaskResult> {
      if (this.isCancelled) {
        return ChunkTaskResult.CANCELLED;
      }

      if (!this.renderChunk.hasAllNeighbors()) {
        this.region = null;
        this.renderChunk.setDirty(false);
        this.isCancelled = true;
        return ChunkTaskResult.CANCELLED;
      }

      const camera = this.renderChunk.getDispatcher().getCameraPosition();
      const origin = new BlockPos(
        this.renderChunk.getOrigin().getX(),
        this.renderChunk.getOrigin().getY(),
        this.renderChunk.getOrigin().getZ(),
      );
      const compiledChunk = new CompiledChunk();

      if (this.meshInput !== undefined) {
        const decoded = decodeSectionMeshResult(await this.renderChunk.getDispatcher().runMeshWorkerBuild(this.meshInput, camera));
        this.applyCompiledState(compiledChunk, decoded);
        this.renderChunk.updateGlobalBlockEntities(new Set<unknown>(), new Set<unknown>());
        if (this.isCancelled) {
          return ChunkTaskResult.CANCELLED;
        }

        for (const layer of decoded.layers) {
          this.renderChunk.getBuffer(layer.renderType).uploadRaw(layer.drawState, layer.buffer);
        }
        this.renderChunk.getDispatcher().notePendingUploads();
      } else {
        if (buffers === undefined) {
          throw new Error("Chunk rebuild task requires a ChunkBufferBuilderPack");
        }

        const region = this.region;
        this.region = null;
        const build = buildSectionMesh(origin, camera, region, this.renderChunk.getDispatcher().blockRenderer, buffers);
        this.applyCompiledState(compiledChunk, build);
        this.renderChunk.updateGlobalBlockEntities(new Set<unknown>(), build.globalBlockEntities);
        if (this.isCancelled) {
          return ChunkTaskResult.CANCELLED;
        }

        for (const renderType of compiledChunk.hasLayer) {
          await this.renderChunk.getDispatcher().uploadChunkLayer(buffers.builder(renderType), this.renderChunk.getBuffer(renderType));
        }
      }

      if (this.isCancelled) {
        return ChunkTaskResult.CANCELLED;
      }

      this.renderChunk.compiled.current = compiledChunk;
      // WebGPU: immediate uploads still need the same post-upload visibility invalidation vanilla gets from its GL upload queue.
      this.renderChunk.getDispatcher().renderer.requestUpdate();
      this.renderChunk.setNotDirty();
      return ChunkTaskResult.SUCCESSFUL;
    }

    private applyCompiledState(
      compiledChunk: CompiledChunk,
      state: {
        readonly hasBlocks: ReadonlySet<RenderType>;
        readonly hasLayers: ReadonlySet<RenderType>;
        readonly isCompletelyEmpty: boolean;
        readonly visibilitySet: VisibilitySet;
        readonly transparencyState: BufferBuilderSortState | undefined;
      },
    ): void {
      compiledChunk.isCompletelyEmpty = state.isCompletelyEmpty;
      compiledChunk.visibilitySet = state.visibilitySet;
      compiledChunk.transparencyState = state.transparencyState;
      for (const renderType of state.hasBlocks) {
        compiledChunk.hasBlocks.add(renderType);
      }

      for (const renderType of state.hasLayers) {
        compiledChunk.hasLayer.add(renderType);
      }
    }

    public override cancel(): void {
      this.region = null;
      if (!this.isCancelled) {
        this.isCancelled = true;
        this.renderChunk.setDirty(false);
      }
    }
  }

  export class ResortTransparencyTask extends ChunkCompileTask {
    public constructor(
      renderChunk: RenderChunk,
      distAtCreation: number,
      private readonly compiledChunk: CompiledChunk,
    ) {
      super(renderChunk, distAtCreation);
    }

    public override async doTask(buffers?: ChunkBufferBuilderPack): Promise<ChunkTaskResult> {
      if (this.isCancelled || !this.renderChunk.hasAllNeighbors()) {
        this.isCancelled = true;
        return ChunkTaskResult.CANCELLED;
      }

      if (buffers === undefined) {
        throw new Error("Transparency resort task requires a ChunkBufferBuilderPack");
      }

      const sortState = this.compiledChunk.transparencyState;
      if (sortState === undefined || !this.compiledChunk.hasBlocks.has(RenderType.translucent())) {
        return ChunkTaskResult.CANCELLED;
      }

      const origin = this.renderChunk.getOrigin();
      const camera = this.renderChunk.getDispatcher().getCameraPosition();
      const builder = buffers.builder(RenderType.translucent());
      this.renderChunk.beginLayer(builder);
      builder.restoreSortState(sortState);
      builder.setQuadSortOrigin(camera.x - origin.getX(), camera.y - origin.getY(), camera.z - origin.getZ());
      this.compiledChunk.transparencyState = builder.getSortState();
      builder.end();
      if (this.isCancelled) {
        return ChunkTaskResult.CANCELLED;
      }

      await this.renderChunk.getDispatcher().uploadChunkLayer(builder, this.renderChunk.getBuffer(RenderType.translucent()));
      return this.isCancelled ? ChunkTaskResult.CANCELLED : ChunkTaskResult.SUCCESSFUL;
    }

    public override cancel(): void {
      this.isCancelled = true;
    }
  }
}
