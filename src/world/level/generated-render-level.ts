import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import type { ColorResolver } from "./color-resolver";
import type { Biome } from "../../worldgen/biome/biome";
import { getBlockPositionBiome } from "../../worldgen/biome/biome-zoom";
import type { NoiseBiomeSource } from "../../worldgen/biome/noise-biome-source";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import { MutableChunkBlockBuffer } from "../../worldgen/chunk/chunk-block-buffer";
import { NoiseBasedChunkGenerator } from "../../worldgen/levelgen/noise-based-chunk-generator";
import type { CooperativeGenerationYield } from "../../worldgen/levelgen/cooperative-generation";
import { BlockStateProperties } from "./block/state/properties/block-state-properties";
import { type BlockState } from "./block/state/block-state";
import { LevelChunk } from "./chunk/level-chunk";
import {
  FEATURES_CHUNK_DEPENDENCY_RADIUS,
  GeneratedDecorationRegion,
} from "./generated-decoration-region";
import { StaticRenderLevel } from "./static-render-level";

function decoratedChunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

export interface GeneratedChunkViewUpdate {
  readonly changed: boolean;
  readonly unloadedChunks: readonly LevelChunk[];
  readonly removedChunks: readonly LevelChunk[];
  readonly missingChunks: readonly (readonly [number, number])[];
}

export class GeneratedRenderLevel extends StaticRenderLevel {
  private viewCenterX = Number.MIN_SAFE_INTEGER;
  private viewCenterZ = Number.MIN_SAFE_INTEGER;
  // Runtime: keep a hidden authority window for FEATURES dependency reads while only publishing the view radius.
  private publishChunkRadius = -1;
  private authorityChunkRadius = -1;
  private readonly decoratedChunks = new Set<string>();
  private readonly decoratingChunks = new Set<string>();
  private automaticDecorationEnabled = true;

  public constructor(
    airState: BlockState,
    private readonly generator: NoiseBasedChunkGenerator,
    private readonly biomeSource: NoiseBiomeSource,
    private readonly biomeZoomSeed: bigint,
    private readonly blockStateById: readonly BlockState[],
    skyLight = 15,
    blockLight = 15,
    minBuildHeight = 0,
    height = 256,
  ) {
    super(airState, skyLight, blockLight, minBuildHeight, height);
  }

  public override getChunk(chunkX: number, chunkZ: number, create = true): LevelChunk | null {
    const existing = super.getChunk(chunkX, chunkZ, false);
    if (existing !== null) {
      if (
        create
        && this.automaticDecorationEnabled
        && this.inPublishRange(chunkX, chunkZ)
        && !this.isChunkDecorated(chunkX, chunkZ)
        && !this.decoratingChunks.has(decoratedChunkKey(chunkX, chunkZ))
      ) {
        this.decorateChunk(chunkX, chunkZ);
      }
      return existing;
    }

    if (!create || !this.inAuthorityRange(chunkX, chunkZ)) {
      return null;
    }

    const generated = this.generateChunkTerrain(chunkX, chunkZ);
    if (generated === null) {
      return null;
    }
    if (!this.automaticDecorationEnabled || !this.inPublishRange(chunkX, chunkZ)) {
      return generated;
    }
    this.decorateChunk(chunkX, chunkZ);
    return generated;
  }

  public override getLoadedChunks(): readonly LevelChunk[] {
    return super.getLoadedChunks().filter((chunk) => this.inPublishRange(chunk.chunkX, chunk.chunkZ));
  }

  public override getLoadedChunkCount(): number {
    return this.getLoadedChunks().length;
  }

  public override setChunk(chunk: LevelChunk, decorated = false): void {
    super.setChunk(chunk);
    const key = decoratedChunkKey(chunk.chunkX, chunk.chunkZ);
    if (decorated) {
      this.decoratedChunks.add(key);
    } else {
      this.decoratedChunks.delete(key);
    }
    this.decoratingChunks.delete(key);
  }

  public ensureChunksForCamera(cameraX: number, cameraZ: number, viewDistance: number): boolean {
    const update = this.updateChunkView(
      SectionPos.posToSectionCoord(cameraX),
      SectionPos.posToSectionCoord(cameraZ),
      viewDistance,
    );
    for (const [chunkX, chunkZ] of update.missingChunks) {
      this.getChunk(chunkX, chunkZ, true);
    }

    return update.changed;
  }

  public updateChunkView(centerChunkX: number, centerChunkZ: number, viewDistance: number): GeneratedChunkViewUpdate {
    const previousCenterX = this.viewCenterX;
    const previousCenterZ = this.viewCenterZ;
    const previousPublishRadius = this.publishChunkRadius;
    const nextPublishRadius = Math.max(1, viewDistance) + 1;
    // Runtime: widen authoritative retention to cover the vanilla FEATURES read dependency window.
    const nextAuthorityRadius = nextPublishRadius + FEATURES_CHUNK_DEPENDENCY_RADIUS;
    let changed =
      centerChunkX !== previousCenterX ||
      centerChunkZ !== previousCenterZ ||
      nextPublishRadius !== previousPublishRadius ||
      nextAuthorityRadius !== this.authorityChunkRadius;

    this.viewCenterX = centerChunkX;
    this.viewCenterZ = centerChunkZ;
    this.publishChunkRadius = nextPublishRadius;
    this.authorityChunkRadius = nextAuthorityRadius;

    const unloadedChunks: LevelChunk[] = [];
    const removedChunks: LevelChunk[] = [];
    for (const chunk of super.getLoadedChunks()) {
      const wasPublished = this.isWithinRadius(chunk.chunkX, chunk.chunkZ, previousCenterX, previousCenterZ, previousPublishRadius);
      if (!this.inAuthorityRange(chunk.chunkX, chunk.chunkZ)) {
        const removed = super.removeChunk(chunk.chunkX, chunk.chunkZ);
        if (removed !== undefined) {
          removedChunks.push(removed);
          if (wasPublished) {
            unloadedChunks.push(removed);
          }
        }
        this.decoratedChunks.delete(decoratedChunkKey(chunk.chunkX, chunk.chunkZ));
        this.decoratingChunks.delete(decoratedChunkKey(chunk.chunkX, chunk.chunkZ));
        changed = true;
        continue;
      }

      if (wasPublished && !this.inPublishRange(chunk.chunkX, chunk.chunkZ)) {
        unloadedChunks.push(chunk);
        changed = true;
      }
    }

    const missingChunks: Array<readonly [number, number]> = [];
    for (let chunkZ = this.viewCenterZ - this.authorityChunkRadius; chunkZ <= this.viewCenterZ + this.authorityChunkRadius; chunkZ++) {
      for (let chunkX = this.viewCenterX - this.authorityChunkRadius; chunkX <= this.viewCenterX + this.authorityChunkRadius; chunkX++) {
        const chunk = super.getChunk(chunkX, chunkZ, false);
        if (chunk === null || (this.inPublishRange(chunkX, chunkZ) && !this.isChunkDecorated(chunkX, chunkZ))) {
          missingChunks.push([chunkX, chunkZ]);
          changed = true;
        }
      }
    }

    return { changed, unloadedChunks, removedChunks, missingChunks };
  }

  public override getBlockTint(pos: BlockPos, resolver?: ColorResolver): number {
    if (resolver === undefined) {
      return -1;
    }

    const biome = getBlockPositionBiome(this.biomeZoomSeed, pos.getX(), pos.getZ(), this.biomeSource) as Biome;
    return resolver.getColor(biome, pos.getX(), pos.getZ());
  }

  public override getBiome(pos: BlockPos): Biome {
    return getBlockPositionBiome(this.biomeZoomSeed, pos.getX(), pos.getZ(), this.biomeSource) as Biome;
  }

  public getAuthorityChunk(chunkX: number, chunkZ: number): LevelChunk | null {
    return super.getChunk(chunkX, chunkZ, false);
  }

  private inPublishRange(chunkX: number, chunkZ: number): boolean {
    return this.isWithinRadius(chunkX, chunkZ, this.viewCenterX, this.viewCenterZ, this.publishChunkRadius);
  }

  private inAuthorityRange(chunkX: number, chunkZ: number): boolean {
    return this.isWithinRadius(chunkX, chunkZ, this.viewCenterX, this.viewCenterZ, this.authorityChunkRadius);
  }

  private isWithinRadius(
    chunkX: number,
    chunkZ: number,
    centerChunkX: number,
    centerChunkZ: number,
    radius: number,
  ): boolean {
    if (radius < 0) {
      return false;
    }

    return Math.abs(chunkX - centerChunkX) <= radius && Math.abs(chunkZ - centerChunkZ) <= radius;
  }

  public isChunkDecorated(chunkX: number, chunkZ: number): boolean {
    return this.decoratedChunks.has(decoratedChunkKey(chunkX, chunkZ));
  }

  public generateChunkTerrain(chunkX: number, chunkZ: number): LevelChunk | null {
    const existing = super.getChunk(chunkX, chunkZ, false);
    if (existing !== null) {
      return existing;
    }

    if (!this.inAuthorityRange(chunkX, chunkZ)) {
      return null;
    }

    const generated = this.generator.fillFromNoise(chunkX, chunkZ);
    this.generator.buildSurfaceAndBedrock(generated);
    this.generator.applyCarvers(generated);
    const chunk = this.copyGeneratedChunk(chunkX, chunkZ, generated);
    super.setChunk(chunk);
    this.decoratedChunks.delete(decoratedChunkKey(chunkX, chunkZ));
    return chunk;
  }

  public async generateChunkTerrainCooperative(
    chunkX: number,
    chunkZ: number,
    yieldStep: CooperativeGenerationYield,
  ): Promise<LevelChunk | null> {
    const existing = super.getChunk(chunkX, chunkZ, false);
    if (existing !== null) {
      return existing;
    }

    if (!this.inAuthorityRange(chunkX, chunkZ)) {
      return null;
    }

    const generated = this.generator.fillFromNoise(chunkX, chunkZ);
    await yieldStep();
    this.generator.buildSurfaceAndBedrock(generated);
    await yieldStep();
    await this.generator.applyCarversCooperative(generated, yieldStep);
    const chunk = this.copyGeneratedChunk(chunkX, chunkZ, generated);
    super.setChunk(chunk);
    this.decoratedChunks.delete(decoratedChunkKey(chunkX, chunkZ));
    await yieldStep();
    return chunk;
  }

  public decorateChunk(chunkX: number, chunkZ: number): void {
    const key = decoratedChunkKey(chunkX, chunkZ);
    if (this.decoratedChunks.has(key)) {
      return;
    }

    if (this.decoratingChunks.has(key)) {
      return;
    }

    this.decoratingChunks.add(key);
    try {
      this.ensureDecorationTerrainWindow(chunkX, chunkZ);
      this.generator.applyBiomeDecoration(new GeneratedDecorationRegion(this, chunkX, chunkZ), chunkX, chunkZ);
      this.decoratedChunks.add(key);
    } finally {
      this.decoratingChunks.delete(key);
    }
  }

  public async decorateChunkCooperative(
    chunkX: number,
    chunkZ: number,
    yieldStep: CooperativeGenerationYield,
  ): Promise<void> {
    const key = decoratedChunkKey(chunkX, chunkZ);
    if (this.decoratedChunks.has(key)) {
      return;
    }

    if (this.decoratingChunks.has(key)) {
      return;
    }

    this.decoratingChunks.add(key);
    try {
      await this.ensureDecorationTerrainWindowCooperative(chunkX, chunkZ, yieldStep);
      await this.generator.applyBiomeDecorationCooperative(new GeneratedDecorationRegion(this, chunkX, chunkZ), chunkX, chunkZ, yieldStep);
      this.decoratedChunks.add(key);
    } finally {
      this.decoratingChunks.delete(key);
    }
  }

  private ensureDecorationTerrainWindow(chunkX: number, chunkZ: number): void {
    for (let neighborChunkZ = chunkZ - FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkZ <= chunkZ + FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkZ++) {
      for (let neighborChunkX = chunkX - FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkX <= chunkX + FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkX++) {
        if (super.getChunk(neighborChunkX, neighborChunkZ, false) !== null) {
          continue;
        }

        if (this.generateChunkTerrain(neighborChunkX, neighborChunkZ) === null) {
          throw new Error(
            `Missing authority terrain for decoration window at (${neighborChunkX.toString()}, ${neighborChunkZ.toString()}) around (${chunkX.toString()}, ${chunkZ.toString()})`,
          );
        }
      }
    }
  }

  private async ensureDecorationTerrainWindowCooperative(
    chunkX: number,
    chunkZ: number,
    yieldStep: CooperativeGenerationYield,
  ): Promise<void> {
    for (let neighborChunkZ = chunkZ - FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkZ <= chunkZ + FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkZ++) {
      for (let neighborChunkX = chunkX - FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkX <= chunkX + FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkX++) {
        if (super.getChunk(neighborChunkX, neighborChunkZ, false) !== null) {
          continue;
        }

        if (await this.generateChunkTerrainCooperative(neighborChunkX, neighborChunkZ, yieldStep) === null) {
          throw new Error(
            `Missing authority terrain for decoration window at (${neighborChunkX.toString()}, ${neighborChunkZ.toString()}) around (${chunkX.toString()}, ${chunkZ.toString()})`,
          );
        }
      }
    }
  }

  private copyGeneratedChunk(chunkX: number, chunkZ: number, generated: MutableChunkBlockBuffer): LevelChunk {
    const chunk = new LevelChunk(chunkX, chunkZ, this.blockStateById[0]!);
    const worldX = SectionPos.sectionToBlockCoord(chunkX);
    const worldZ = SectionPos.sectionToBlockCoord(chunkZ);
    const pos = new BlockPos.MutableBlockPos();
    let index = 0;

    for (let y = generated.minY; y < generated.minY + generated.height; y++) {
      for (let z = 0; z < 16; z++) {
        for (let x = 0; x < 16; x++) {
          const blockId = generated.blocks[index++]!;
          let state = this.blockStateById[blockId] ?? this.blockStateById[0]!;
          if (state.isAir()) {
            continue;
          }

          if (blockId === ChunkBlockId.GRASS_BLOCK && y + 1 < generated.minY + generated.height && generated.getBlockAtY(x, y + 1, z) === ChunkBlockId.SNOW) {
            state = state.setValue(BlockStateProperties.SNOWY, true);
          }

          pos.set(worldX + x, y, worldZ + z);
          chunk.setBlockState(pos, state);
        }
      }
    }

    chunk.appendBlockTicks(generated.getScheduledBlockTicks());
    chunk.appendLiquidTicks(generated.getScheduledLiquidTicks());

    return chunk;
  }
}
