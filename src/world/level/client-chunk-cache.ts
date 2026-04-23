import { BlockPos } from "../../core/block-pos";
import type { Biome } from "../../worldgen/biome/biome";
import { getBlockPositionBiome } from "../../worldgen/biome/biome-zoom";
import { ChunkBiomeContainer } from "../../worldgen/biome/chunk-biome-container";
import type { NoiseBiomeSource } from "../../worldgen/biome/noise-biome-source";
import type { ColorResolver } from "./color-resolver";
import type { BlockState } from "./block/state/block-state";
import { hydrateChunkFromSnapshot, type BlockStateResolver, type ChunkSnapshot } from "./chunk-snapshot";
import { StaticRenderLevel } from "./static-render-level";
import { type LevelChunk } from "./chunk/level-chunk";

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

function quartToChunkCoord(value: number): number {
  return Math.floor(value / 4);
}

export interface ClientChunkCacheOptions {
  readonly airState: BlockState;
  readonly minBuildHeight: number;
  readonly height: number;
  readonly biomeSource: NoiseBiomeSource;
  readonly biomeZoomSeed: bigint;
  readonly blockStateResolver: BlockStateResolver;
  readonly skyLight?: number;
  readonly blockLight?: number;
}

export class ClientChunkCache extends StaticRenderLevel implements NoiseBiomeSource {
  private readonly biomeContainers = new Map<string, ChunkBiomeContainer>();
  private readonly biomeSource: NoiseBiomeSource;
  private readonly biomeZoomSeed: bigint;
  private readonly blockStateResolver: BlockStateResolver;

  public constructor(options: ClientChunkCacheOptions) {
    super(options.airState, options.skyLight, options.blockLight, options.minBuildHeight, options.height);
    this.biomeSource = options.biomeSource;
    this.biomeZoomSeed = options.biomeZoomSeed;
    this.blockStateResolver = options.blockStateResolver;
  }

  public override getChunk(chunkX: number, chunkZ: number, _create = true): LevelChunk | null {
    return super.getChunk(chunkX, chunkZ, false);
  }

  public override setBlock(_pos: BlockPos, _state: BlockState, _flags = 3): boolean {
    return false;
  }

  public applyChunkSnapshot(snapshot: ChunkSnapshot): void {
    super.setChunk(hydrateChunkFromSnapshot(snapshot, this.airState, this.blockStateResolver));
    this.biomeContainers.set(
      chunkKey(snapshot.chunkX, snapshot.chunkZ),
      new ChunkBiomeContainer(
        this.getMinBuildHeight(),
        this.getHeight(),
        snapshot.chunkX,
        snapshot.chunkZ,
        this.biomeSource,
        snapshot.biomes,
      ),
    );
  }

  public applyChunkUnload(chunkX: number, chunkZ: number): boolean {
    this.biomeContainers.delete(chunkKey(chunkX, chunkZ));
    return super.removeChunk(chunkX, chunkZ) !== undefined;
  }

  public override getBlockTint(pos: BlockPos, resolver?: ColorResolver): number {
    if (resolver === undefined) {
      return -1;
    }

    const biome = getBlockPositionBiome(this.biomeZoomSeed, pos.getX(), pos.getZ(), this) as Biome;
    return resolver.getColor(biome, pos.getX(), pos.getZ());
  }

  public override getBiome(pos: BlockPos): Biome {
    return getBlockPositionBiome(this.biomeZoomSeed, pos.getX(), pos.getZ(), this) as Biome;
  }

  public getNoiseBiome(x: number, y: number, z: number): Biome {
    const container = this.biomeContainers.get(chunkKey(quartToChunkCoord(x), quartToChunkCoord(z)));
    return (container?.getNoiseBiome(x, y, z) ?? this.biomeSource.getNoiseBiome(x, y, z)) as Biome;
  }
}
