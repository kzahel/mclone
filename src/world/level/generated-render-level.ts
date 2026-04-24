import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import type { ColorResolver } from "./color-resolver";
import type { Biome } from "../../worldgen/biome/biome";
import { getBlockPositionBiome } from "../../worldgen/biome/biome-zoom";
import type { NoiseBiomeSource } from "../../worldgen/biome/noise-biome-source";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import { MutableChunkBlockBuffer } from "../../worldgen/chunk/chunk-block-buffer";
import { NoiseBasedChunkGenerator } from "../../worldgen/levelgen/noise-based-chunk-generator";
import { BlockStateProperties } from "./block/state/properties/block-state-properties";
import { type BlockState } from "./block/state/block-state";
import { LevelChunk } from "./chunk/level-chunk";
import { StaticRenderLevel } from "./static-render-level";

function decoratedChunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

export interface GeneratedChunkViewUpdate {
  readonly changed: boolean;
  readonly unloadedChunks: readonly LevelChunk[];
  readonly missingChunks: readonly (readonly [number, number])[];
}

export class GeneratedRenderLevel extends StaticRenderLevel {
  private viewCenterX = Number.MIN_SAFE_INTEGER;
  private viewCenterZ = Number.MIN_SAFE_INTEGER;
  private chunkRadius = -1;
  private readonly decoratedChunks = new Set<string>();

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
      return existing;
    }

    if (!create || !this.inRange(chunkX, chunkZ)) {
      return null;
    }

    const generated = this.generateChunk(chunkX, chunkZ);
    super.setChunk(generated);
    this.decorateChunk(chunkX, chunkZ);
    return generated;
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
    const nextRadius = Math.max(1, viewDistance) + 1;
    let changed =
      centerChunkX !== this.viewCenterX ||
      centerChunkZ !== this.viewCenterZ ||
      nextRadius !== this.chunkRadius;

    this.viewCenterX = centerChunkX;
    this.viewCenterZ = centerChunkZ;
    this.chunkRadius = nextRadius;

    const unloadedChunks: LevelChunk[] = [];
    for (const chunk of this.getLoadedChunks()) {
      if (!this.inRange(chunk.chunkX, chunk.chunkZ)) {
        const removed = super.removeChunk(chunk.chunkX, chunk.chunkZ);
        if (removed !== undefined) {
          unloadedChunks.push(removed);
        }
        this.decoratedChunks.delete(decoratedChunkKey(chunk.chunkX, chunk.chunkZ));
        changed = true;
      }
    }

    const missingChunks: Array<readonly [number, number]> = [];
    for (let chunkZ = this.viewCenterZ - this.chunkRadius; chunkZ <= this.viewCenterZ + this.chunkRadius; chunkZ++) {
      for (let chunkX = this.viewCenterX - this.chunkRadius; chunkX <= this.viewCenterX + this.chunkRadius; chunkX++) {
        if (super.getChunk(chunkX, chunkZ, false) === null) {
          missingChunks.push([chunkX, chunkZ]);
          changed = true;
        }
      }
    }

    return { changed, unloadedChunks, missingChunks };
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

  private inRange(chunkX: number, chunkZ: number): boolean {
    return Math.abs(chunkX - this.viewCenterX) <= this.chunkRadius && Math.abs(chunkZ - this.viewCenterZ) <= this.chunkRadius;
  }

  private generateChunk(chunkX: number, chunkZ: number): LevelChunk {
    const generated = this.generator.fillFromNoise(chunkX, chunkZ);
    this.generator.buildSurfaceAndBedrock(generated);
    this.generator.applyCarvers(generated);
    return this.copyGeneratedChunk(chunkX, chunkZ, generated);
  }

  private decorateChunk(chunkX: number, chunkZ: number): void {
    const key = decoratedChunkKey(chunkX, chunkZ);
    if (this.decoratedChunks.has(key)) {
      return;
    }

    this.decoratedChunks.add(key);
    this.generator.applyBiomeDecoration(this, chunkX, chunkZ);
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
