import { BlockPos } from "../core/block-pos";
import { Direction } from "../core/direction";
import { SectionPos } from "../core/section-pos";
import { floor } from "../util/mth";
import type { BlockAndTintGetter } from "../world/level/block-and-tint-getter";
import { LightLayer } from "../world/level/light-layer";
import type { BlockState } from "../world/level/block/state/block-state";
import { StaticRenderLevel } from "../world/level/static-render-level";
import { Vec3 } from "../world/phys/vec3";
import { Camera } from "./camera";
import { ChunkRenderDispatcher } from "./chunk/chunk-render-dispatcher";
import { Frustum } from "./culling/frustum";
import { collectPresentationEntityRenderBatches, type EntityRenderBatch } from "./entity/entity-batch-renderer";
import { EntityRenderDispatcher } from "./entity/entity-render-dispatcher";
import { FogMode, FogRenderer } from "./fog-renderer";
import { GameRenderer, type RenderLevelOptions } from "./game-renderer";
import { LightTexture } from "./light-texture";
import { Matrix4f } from "./math/matrix4f";
import { RenderType } from "./render-type";
import { VertexBuffer } from "./vertex/vertex-buffer";
import { PoseStack } from "./vertex/pose-stack";
import { ViewArea } from "./view-area";

const DIRECTIONS = Direction.values();

export type ChunkLayerDraw = {
  readonly vertexBuffer: VertexBuffer;
  readonly chunkOffset: readonly [number, number, number];
};

export type LevelRenderFrame = {
  readonly frameId: number;
  readonly modelViewMatrix: Float32Array;
  readonly projectionMatrix: Float32Array;
  readonly cameraPosition: readonly [number, number, number];
  readonly fogStart: number;
  readonly fogEnd: number;
  readonly fogColor: readonly [number, number, number, number];
  readonly lightTexture: GPUTextureView;
  readonly layerDraws: ReadonlyMap<RenderType, readonly ChunkLayerDraw[]>;
  readonly entityBatches: readonly EntityRenderBatch[];
};

export class LevelRenderer {
  private level: StaticRenderLevel | undefined;
  private chunkRenderDispatcher: ChunkRenderDispatcher | undefined;
  private viewArea: ViewArea | undefined;
  private chunksToCompile = new Set<ChunkRenderDispatcher.RenderChunk>();
  private readonly renderChunks: LevelRenderer.RenderChunkInfo[] = [];
  private renderInfoMap: LevelRenderer.RenderInfoMap | undefined;
  private lastCameraX = Number.MIN_VALUE;
  private lastCameraY = Number.MIN_VALUE;
  private lastCameraZ = Number.MIN_VALUE;
  private lastCameraChunkX = Number.MIN_SAFE_INTEGER;
  private lastCameraChunkY = Number.MIN_SAFE_INTEGER;
  private lastCameraChunkZ = Number.MIN_SAFE_INTEGER;
  private prevCamX = Number.MIN_VALUE;
  private prevCamY = Number.MIN_VALUE;
  private prevCamZ = Number.MIN_VALUE;
  private prevCamRotX = Number.MIN_VALUE;
  private prevCamRotY = Number.MIN_VALUE;
  private lastViewDistance = -1;
  private cullingFrustum: Frustum | undefined;
  private needsUpdate = true;
  private frameId = 0;
  private readonly entityRenderDispatcher = new EntityRenderDispatcher();

  public setLevel(level: StaticRenderLevel, chunkRenderDispatcher: ChunkRenderDispatcher, viewArea: ViewArea, viewDistance: number): void {
    this.level = level;
    this.chunkRenderDispatcher = chunkRenderDispatcher;
    this.viewArea = viewArea;
    this.lastViewDistance = viewDistance;
    this.renderInfoMap = new LevelRenderer.RenderInfoMap(viewArea.chunks.length);
    this.allChanged();
  }

  public allChanged(): void {
    this.needsUpdate = true;
    this.chunksToCompile.clear();
  }

  public requestUpdate(): void {
    this.needsUpdate = true;
  }

  public countRenderedChunks(): number {
    let count = 0;
    for (const renderChunkInfo of this.renderChunks) {
      if (!renderChunkInfo.chunk.getCompiledChunk().hasNoRenderableLayers()) {
        count++;
      }
    }

    return count;
  }

  public getPendingVisibleChunkCompileCount(): number {
    return this.chunksToCompile.size;
  }

  public static getLightColor(level: BlockAndTintGetter, pos: BlockPos): number {
    return LevelRenderer.getLightColorFromState(level, level.getBlockState(pos), pos);
  }

  public static getLightColorFromState(level: BlockAndTintGetter, state: BlockState, pos: BlockPos): number {
    if (state.emissiveRendering(level, pos)) {
      return 15_728_880;
    }

    const sky = level.getBrightness(LightLayer.SKY, pos);
    let block = level.getBrightness(LightLayer.BLOCK, pos);
    const lightEmission = state.getLightEmission();
    if (block < lightEmission) {
      block = lightEmission;
    }

    return (sky << 20) | (block << 4);
  }

  public updateGlobalBlockEntities(_removed: ReadonlySet<unknown>, _added: ReadonlySet<unknown>): void {}

  public prepareCullFrustum(poseStack: PoseStack, position: Vec3, projectionMatrix: Matrix4f): void {
    const modelViewMatrix = poseStack.last().pose();
    this.cullingFrustum = new Frustum(modelViewMatrix, projectionMatrix);
    this.cullingFrustum.prepare(position.x, position.y, position.z);
  }

  public async renderLevel(
    poseStack: PoseStack,
    partialTick: number,
    finishTimeNano: number,
    _renderBlockOutline: boolean,
    camera: Camera,
    gameRenderer: GameRenderer,
    lightTexture: LightTexture,
    projectionMatrix: Matrix4f,
    options: RenderLevelOptions = {},
  ): Promise<LevelRenderFrame> {
    const level = this.level!;
    const frustum = this.cullingFrustum!;
    const frameId = this.frameId++;
    FogRenderer.setupColor(camera, partialTick, level, this.lastViewDistance * 16, gameRenderer.getDarkenWorldAmount(partialTick));
    // WebGPU: browser option can disable terrain fog independently of render distance.
    if (gameRenderer.isFogEnabled()) {
      FogRenderer.setupFog(camera, FogMode.FOG_TERRAIN, Math.max(gameRenderer.getRenderDistance() - 16.0, 32.0), false);
    } else {
      FogRenderer.setupNoFog();
    }
    this.setupRender(camera, frustum, false, frameId, false);
    await this.compileChunksUntil(finishTimeNano);
    if (options.waitForChunkTasks && this.chunkRenderDispatcher !== undefined && !this.chunkRenderDispatcher.isQueueEmpty()) {
      await this.chunkRenderDispatcher.awaitAllTasks();
    }

    const cameraPosition = camera.getPosition();
    const layerDraws = new Map<RenderType, readonly ChunkLayerDraw[]>();
    for (const renderType of RenderType.chunkBufferLayers()) {
      const draws = this.collectChunkLayer(renderType, cameraPosition.x, cameraPosition.y, cameraPosition.z);
      if (draws.length > 0) {
        layerDraws.set(renderType, draws);
      }
    }
    const entityBatches = collectPresentationEntityRenderBatches({
      level,
      presentation: options.entityPresentation ?? [],
      cameraPosition,
      partialTick,
      dispatcher: this.entityRenderDispatcher,
    });

    return {
      frameId,
      modelViewMatrix: poseStack.last().pose().toFloat32Array(),
      projectionMatrix: projectionMatrix.toFloat32Array(),
      cameraPosition: [cameraPosition.x, cameraPosition.y, cameraPosition.z],
      fogStart: FogRenderer.getShaderFogStart(),
      fogEnd: FogRenderer.getShaderFogEnd(),
      fogColor: FogRenderer.getShaderFogColor(),
      lightTexture: lightTexture.turnOnLightLayer(),
      layerDraws,
      entityBatches,
    };
  }

  private setupRender(camera: Camera, frustum: Frustum, isCapturedFrustum: boolean, frameId: number, spectator: boolean): void {
    const chunkRenderDispatcher = this.chunkRenderDispatcher!;
    const viewArea = this.viewArea!;
    const cameraPosition = camera.getPosition();
    const cameraX = cameraPosition.x;
    const cameraY = cameraPosition.y;
    const cameraZ = cameraPosition.z;
    const deltaX = cameraX - this.lastCameraX;
    const deltaY = cameraY - this.lastCameraY;
    const deltaZ = cameraZ - this.lastCameraZ;
    const chunkX = SectionPos.posToSectionCoord(cameraX);
    const chunkY = SectionPos.posToSectionCoord(cameraY);
    const chunkZ = SectionPos.posToSectionCoord(cameraZ);
    if (
      this.lastCameraChunkX !== chunkX ||
      this.lastCameraChunkY !== chunkY ||
      this.lastCameraChunkZ !== chunkZ ||
      ((deltaX * deltaX) + (deltaY * deltaY) + (deltaZ * deltaZ)) > 16.0
    ) {
      this.lastCameraX = cameraX;
      this.lastCameraY = cameraY;
      this.lastCameraZ = cameraZ;
      this.lastCameraChunkX = chunkX;
      this.lastCameraChunkY = chunkY;
      this.lastCameraChunkZ = chunkZ;
      viewArea.repositionCamera(cameraX, cameraZ);
    }

    chunkRenderDispatcher.setCamera(cameraPosition);
    const cameraBlockPos = camera.getBlockPosition();
    const cameraRenderChunk = viewArea.getRenderChunkAt(cameraBlockPos);
    const cameraSectionOrigin = new BlockPos.MutableBlockPos(
      floor(cameraPosition.x / 16.0) * 16,
      floor(cameraPosition.y / 16.0) * 16,
      floor(cameraPosition.z / 16.0) * 16,
    );
    const xRot = camera.getXRot();
    const yRot = camera.getYRot();
    this.needsUpdate =
      this.needsUpdate ||
      this.chunksToCompile.size > 0 ||
      cameraPosition.x !== this.prevCamX ||
      cameraPosition.y !== this.prevCamY ||
      cameraPosition.z !== this.prevCamZ ||
      xRot !== this.prevCamRotX ||
      yRot !== this.prevCamRotY;
    this.prevCamX = cameraPosition.x;
    this.prevCamY = cameraPosition.y;
    this.prevCamZ = cameraPosition.z;
    this.prevCamRotX = xRot;
    this.prevCamRotY = yRot;
    if (!isCapturedFrustum && this.needsUpdate) {
      this.needsUpdate = false;
      this.updateRenderChunks(frustum, frameId, spectator, cameraPosition, cameraBlockPos, cameraRenderChunk, 16, cameraSectionOrigin);
    }

    const oldChunksToCompile = this.chunksToCompile;
    this.chunksToCompile = new Set<ChunkRenderDispatcher.RenderChunk>();
    for (const renderChunkInfo of this.renderChunks) {
      const renderChunk = renderChunkInfo.chunk;
      if (renderChunk.isDirty() || oldChunksToCompile.has(renderChunk)) {
        this.needsUpdate = true;
        this.chunksToCompile.add(renderChunk);
      }
    }

    for (const renderChunk of oldChunksToCompile) {
      this.chunksToCompile.add(renderChunk);
    }
  }

  private updateRenderChunks(
    frustum: Frustum,
    frameId: number,
    spectator: boolean,
    cameraPosition: Vec3,
    cameraBlockPos: BlockPos,
    cameraRenderChunk: ChunkRenderDispatcher.RenderChunk | null,
    sectionSize: number,
    cameraSectionOrigin: BlockPos,
  ): void {
    const level = this.level!;
    const viewArea = this.viewArea!;
    this.renderChunks.length = 0;
    this.renderInfoMap!.clear();
    const queue: LevelRenderer.RenderChunkInfo[] = [];
    let smartCull = true;
    if (cameraRenderChunk === null) {
      const maxBuildY = cameraBlockPos.getY() > level.getMinBuildHeight() ? level.getMaxBuildHeight() - 8 : level.getMinBuildHeight() + 8;
      const originX = floor(cameraPosition.x / sectionSize) * sectionSize;
      const originZ = floor(cameraPosition.z / sectionSize) * sectionSize;
      const candidates: LevelRenderer.RenderChunkInfo[] = [];
      for (let x = -this.lastViewDistance; x <= this.lastViewDistance; x++) {
        for (let z = -this.lastViewDistance; z <= this.lastViewDistance; z++) {
          const renderChunk = viewArea.getRenderChunkAt(
            new BlockPos.MutableBlockPos(
              originX + SectionPos.sectionToBlockCoordWithOffset(x, 8),
              maxBuildY,
              originZ + SectionPos.sectionToBlockCoordWithOffset(z, 8),
            ),
          );
          if (renderChunk !== null && frustum.isVisible(renderChunk.bb)) {
            renderChunk.setFrame(frameId);
            candidates.push(new LevelRenderer.RenderChunkInfo(renderChunk, undefined, 0));
          }
        }
      }

      candidates.sort((left, right) => {
        const leftCenter = left.chunk.getOrigin().offset(8, 8, 8);
        const rightCenter = right.chunk.getOrigin().offset(8, 8, 8);
        const leftDeltaX = cameraBlockPos.getX() - leftCenter.getX();
        const leftDeltaY = cameraBlockPos.getY() - leftCenter.getY();
        const leftDeltaZ = cameraBlockPos.getZ() - leftCenter.getZ();
        const rightDeltaX = cameraBlockPos.getX() - rightCenter.getX();
        const rightDeltaY = cameraBlockPos.getY() - rightCenter.getY();
        const rightDeltaZ = cameraBlockPos.getZ() - rightCenter.getZ();
        return ((leftDeltaX * leftDeltaX) + (leftDeltaY * leftDeltaY) + (leftDeltaZ * leftDeltaZ))
          - ((rightDeltaX * rightDeltaX) + (rightDeltaY * rightDeltaY) + (rightDeltaZ * rightDeltaZ));
      });
      queue.push(...candidates);
    } else {
      if (spectator && level.getBlockState(cameraBlockPos).isSolidRender(level, cameraBlockPos)) {
        smartCull = false;
      }

      cameraRenderChunk.setFrame(frameId);
      const info = new LevelRenderer.RenderChunkInfo(cameraRenderChunk, undefined, 0);
      queue.push(info);
      this.renderInfoMap!.put(cameraRenderChunk, info);
    }
    while (queue.length > 0) {
      const renderChunkInfo = queue.shift()!;
      const renderChunk = renderChunkInfo.chunk;
      this.renderChunks.push(renderChunkInfo);
      for (const direction of DIRECTIONS) {
        const relativeChunk = this.getRelativeFrom(cameraSectionOrigin, renderChunk, direction);
        if (!smartCull || !renderChunkInfo.hasDirection(direction.getOpposite())) {
          if (smartCull && renderChunkInfo.hasSourceDirections()) {
            const compiledChunk = renderChunk.getCompiledChunk();
            let canSee = false;
            for (let index = 0; index < DIRECTIONS.length; index++) {
              if (
                renderChunkInfo.hasSourceDirection(index) &&
                compiledChunk.facesCanSeeEachother(DIRECTIONS[index]!.getOpposite(), direction)
              ) {
                canSee = true;
                break;
              }
            }

            if (!canSee) {
              continue;
            }
          }

          if (relativeChunk !== null && relativeChunk.hasAllNeighbors()) {
            if (!relativeChunk.setFrame(frameId)) {
              this.renderInfoMap!.get(relativeChunk)?.addSourceDirection(direction);
            } else if (frustum.isVisible(relativeChunk.bb)) {
              const info = new LevelRenderer.RenderChunkInfo(relativeChunk, direction, renderChunkInfo.step + 1);
              info.setDirections(renderChunkInfo.directions, direction);
              queue.push(info);
              this.renderInfoMap!.put(relativeChunk, info);
            }
          }
        }
      }
    }
  }

  private getRelativeFrom(
    cameraSectionOrigin: BlockPos,
    renderChunk: ChunkRenderDispatcher.RenderChunk,
    direction: Direction,
  ): ChunkRenderDispatcher.RenderChunk | null {
    const relativeOrigin = renderChunk.getRelativeOrigin(direction);
    if (Math.abs(cameraSectionOrigin.getX() - relativeOrigin.getX()) > this.lastViewDistance * 16) {
      return null;
    }

    if (relativeOrigin.getY() < this.level!.getMinBuildHeight() || relativeOrigin.getY() >= this.level!.getMaxBuildHeight()) {
      return null;
    }

    if (Math.abs(cameraSectionOrigin.getZ() - relativeOrigin.getZ()) > this.lastViewDistance * 16) {
      return null;
    }

    return this.viewArea!.getRenderChunkAt(relativeOrigin);
  }

  private async compileChunksUntil(_deadlineNano: number): Promise<void> {
    this.needsUpdate = this.needsUpdate || this.chunkRenderDispatcher!.uploadAllPendingUploads();
    if (this.chunksToCompile.size === 0) {
      return;
    }

    // WebGPU: the browser smoke frame compiles the whole visible set deterministically instead of timeslicing against a client frame budget.
    for (const renderChunk of [...this.chunksToCompile]) {
      if (renderChunk.isDirtyFromPlayer()) {
        await this.chunkRenderDispatcher!.rebuildChunkSync(renderChunk);
      } else {
        renderChunk.rebuildChunkAsync(this.chunkRenderDispatcher!);
      }

      renderChunk.setNotDirty();
      this.chunksToCompile.delete(renderChunk);
    }
  }

  private collectChunkLayer(renderType: RenderType, cameraX: number, cameraY: number, cameraZ: number): ChunkLayerDraw[] {
    const draws: ChunkLayerDraw[] = [];
    const opaque = renderType !== RenderType.translucent();
    if (opaque) {
      for (const renderChunkInfo of this.renderChunks) {
        this.maybePushChunkLayer(draws, renderChunkInfo.chunk, renderType, cameraX, cameraY, cameraZ);
      }
      return draws;
    }

    for (let index = this.renderChunks.length - 1; index >= 0; index--) {
      this.maybePushChunkLayer(draws, this.renderChunks[index]!.chunk, renderType, cameraX, cameraY, cameraZ);
    }

    return draws;
  }

  private maybePushChunkLayer(
    draws: ChunkLayerDraw[],
    renderChunk: ChunkRenderDispatcher.RenderChunk,
    renderType: RenderType,
    cameraX: number,
    cameraY: number,
    cameraZ: number,
  ): void {
    if (renderChunk.getCompiledChunk().isEmpty(renderType)) {
      return;
    }

    const origin = renderChunk.getOrigin();
    draws.push({
      vertexBuffer: renderChunk.getBuffer(renderType),
      chunkOffset: [origin.getX() - cameraX, origin.getY() - cameraY, origin.getZ() - cameraZ],
    });
  }
}

export namespace LevelRenderer {
  export class RenderChunkInfo {
    private sourceDirections = 0;
    public directions = 0;

    public constructor(
      public readonly chunk: ChunkRenderDispatcher.RenderChunk,
      direction: Direction | undefined,
      public readonly step: number,
    ) {
      if (direction !== undefined) {
        this.addSourceDirection(direction);
      }
    }

    public setDirections(directions: number, direction: Direction): void {
      this.directions = this.directions | directions | (1 << direction.get3DDataValue());
    }

    public hasDirection(direction: Direction): boolean {
      return (this.directions & (1 << direction.get3DDataValue())) > 0;
    }

    public addSourceDirection(direction: Direction): void {
      this.sourceDirections = this.sourceDirections | this.sourceDirections | (1 << direction.get3DDataValue());
    }

    public hasSourceDirection(direction: number): boolean {
      return (this.sourceDirections & (1 << direction)) > 0;
    }

    public hasSourceDirections(): boolean {
      return this.sourceDirections !== 0;
    }
  }

  export class RenderInfoMap {
    private readonly infos: Array<RenderChunkInfo | undefined>;

    public constructor(size: number) {
      this.infos = new Array<RenderChunkInfo | undefined>(size);
    }

    public clear(): void {
      this.infos.fill(undefined);
    }

    public put(renderChunk: ChunkRenderDispatcher.RenderChunk, info: RenderChunkInfo): void {
      this.infos[renderChunk.index] = info;
    }

    public get(renderChunk: ChunkRenderDispatcher.RenderChunk): RenderChunkInfo | undefined {
      return this.infos[renderChunk.index];
    }
  }
}
