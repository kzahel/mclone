import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import type { Biome } from "../../worldgen/biome/biome";
import { getBlockPositionBiome } from "../../worldgen/biome/biome-zoom";
import { ChunkBiomeContainer } from "../../worldgen/biome/chunk-biome-container";
import type { NoiseBiomeSource } from "../../worldgen/biome/noise-biome-source";
import type { ColorResolver } from "./color-resolver";
import type { BlockState } from "./block/state/block-state";
import { type BlockStateResolver, type ChunkLightSnapshot, type ChunkSnapshot } from "./chunk-snapshot";
import { StaticRenderLevel } from "./static-render-level";
import { type LevelChunk } from "./chunk/level-chunk";
import { Vec3 } from "../phys/vec3";
import { SnapshotLevelChunk } from "./chunk/snapshot-level-chunk";
import type { BlockStateIdMap } from "./block/state/block-state-id";
import { unpackChunkSnapshot, type PackedChunkLightDelta, type PackedLightSectionUpdate, type PackedChunkSnapshot } from "./packed-chunk-snapshot";
import { DataLayer } from "./chunk/data-layer";
import { LightLayer, getLightLayerSurrounding } from "./light-layer";

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

function lightSectionKey(chunkX: number, sectionY: number, chunkZ: number): string {
  return `${chunkX},${sectionY},${chunkZ}`;
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
  readonly blockStateIds: BlockStateIdMap;
  readonly skyLight?: number;
  readonly blockLight?: number;
  readonly skyColor?: Vec3;
  readonly clearColorScale?: number;
}

export class ClientChunkCache extends StaticRenderLevel implements NoiseBiomeSource {
  private readonly biomeContainers = new Map<string, ChunkBiomeContainer>();
  private readonly snapshots = new Map<string, ChunkSnapshot>();
  private readonly chunksWithLight = new Set<string>();
  private readonly skyLightSections = new Map<string, DataLayer>();
  private readonly blockLightSections = new Map<string, DataLayer>();
  private readonly biomeSource: NoiseBiomeSource;
  private readonly biomeZoomSeed: bigint;
  private readonly blockStateResolver: BlockStateResolver;
  private readonly blockStateIds: BlockStateIdMap;

  public constructor(options: ClientChunkCacheOptions) {
    super(
      options.airState,
      options.skyLight,
      options.blockLight,
      options.minBuildHeight,
      options.height,
      options.skyColor,
      options.clearColorScale,
    );
    this.biomeSource = options.biomeSource;
    this.biomeZoomSeed = options.biomeZoomSeed;
    this.blockStateResolver = options.blockStateResolver;
    this.blockStateIds = options.blockStateIds;
  }

  public override getChunk(chunkX: number, chunkZ: number, _create = true): LevelChunk | null {
    return super.getChunk(chunkX, chunkZ, false);
  }

  public override setBlock(_pos: BlockPos, _state: BlockState, _flags = 3): boolean {
    return false;
  }

  public applyChunkSnapshot(snapshot: ChunkSnapshot): void {
    super.setChunk(SnapshotLevelChunk.fromSnapshot(snapshot, this.airState, this.blockStateResolver));
    this.snapshots.set(chunkKey(snapshot.chunkX, snapshot.chunkZ), snapshot);
    this.applyChunkLightSnapshot(snapshot.chunkX, snapshot.chunkZ, snapshot.light);
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

  public applyPackedChunkSnapshot(snapshot: PackedChunkSnapshot): void {
    this.applyChunkSnapshot(unpackChunkSnapshot(snapshot, this.blockStateIds));
  }

  public applyChunkLightDelta(delta: {
    readonly type?: "chunk_light_delta";
    readonly chunkX: number;
    readonly chunkZ: number;
    readonly light: PackedChunkLightDelta;
  }): boolean {
    const key = chunkKey(delta.chunkX, delta.chunkZ);
    const snapshot = this.snapshots.get(key);
    if (snapshot === undefined) {
      return false;
    }

    const light: ChunkLightSnapshot = {
      sky: this.applyLightSectionUpdates(
        this.skyLightSections,
        delta.chunkX,
        delta.chunkZ,
        snapshot.light?.sky ?? [],
        delta.light.sky ?? [],
      ),
      block: this.applyLightSectionUpdates(
        this.blockLightSections,
        delta.chunkX,
        delta.chunkZ,
        snapshot.light?.block ?? [],
        delta.light.block ?? [],
      ),
      lightCorrect: true,
    };

    this.chunksWithLight.add(key);
    this.snapshots.set(key, { ...snapshot, light });
    return (delta.light.sky?.length ?? 0) > 0 || (delta.light.block?.length ?? 0) > 0;
  }

  public applyChunkUnload(chunkX: number, chunkZ: number): boolean {
    this.snapshots.delete(chunkKey(chunkX, chunkZ));
    this.biomeContainers.delete(chunkKey(chunkX, chunkZ));
    this.clearChunkLight(chunkX, chunkZ);
    return super.removeChunk(chunkX, chunkZ) !== undefined;
  }

  public getChunkSnapshot(chunkX: number, chunkZ: number): ChunkSnapshot | undefined {
    return this.snapshots.get(chunkKey(chunkX, chunkZ));
  }

  public override getBrightness(layer: LightLayer, pos: BlockPos): number {
    const chunkX = SectionPos.blockToSectionCoord(pos.getX());
    const chunkZ = SectionPos.blockToSectionCoord(pos.getZ());
    if (!this.chunksWithLight.has(chunkKey(chunkX, chunkZ))) {
      return super.getBrightness(layer, pos);
    }

    if (layer === LightLayer.BLOCK) {
      return this.getLightSectionValue(this.blockLightSections, chunkX, SectionPos.blockToSectionCoord(pos.getY()), chunkZ, pos)
        ?? getLightLayerSurrounding(layer);
    }

    return this.getSkyLightValue(chunkX, chunkZ, pos);
  }

  public override getRawBrightness(pos: BlockPos, amount: number): number {
    const sky = this.getBrightness(LightLayer.SKY, pos) - amount;
    const block = this.getBrightness(LightLayer.BLOCK, pos);
    return Math.max(block, sky);
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

  private applyChunkLightSnapshot(chunkX: number, chunkZ: number, light: ChunkLightSnapshot | undefined): void {
    this.clearChunkLight(chunkX, chunkZ);
    if (light === undefined) {
      return;
    }

    this.chunksWithLight.add(chunkKey(chunkX, chunkZ));
    for (const section of light.sky) {
      this.skyLightSections.set(lightSectionKey(chunkX, section.y, chunkZ), new DataLayer(new Uint8Array(section.data)));
    }
    for (const section of light.block) {
      this.blockLightSections.set(lightSectionKey(chunkX, section.y, chunkZ), new DataLayer(new Uint8Array(section.data)));
    }
  }

  private clearChunkLight(chunkX: number, chunkZ: number): void {
    this.chunksWithLight.delete(chunkKey(chunkX, chunkZ));
    for (const key of [...this.skyLightSections.keys()]) {
      if (key.startsWith(`${chunkX},`) && key.endsWith(`,${chunkZ}`)) {
        this.skyLightSections.delete(key);
      }
    }
    for (const key of [...this.blockLightSections.keys()]) {
      if (key.startsWith(`${chunkX},`) && key.endsWith(`,${chunkZ}`)) {
        this.blockLightSections.delete(key);
      }
    }
  }

  private applyLightSectionUpdates(
    sections: Map<string, DataLayer>,
    chunkX: number,
    chunkZ: number,
    existing: readonly ChunkLightSnapshot["sky"][number][],
    updates: readonly PackedLightSectionUpdate[],
  ): ChunkLightSnapshot["sky"] {
    const next = new Map(existing.map((section) => [section.y, this.cloneLightData(section.data)] as const));
    for (const update of updates) {
      const layer = update.data === undefined ? new DataLayer() : new DataLayer(this.cloneLightData(update.data));
      sections.set(lightSectionKey(chunkX, update.y, chunkZ), layer);
      next.set(update.y, this.cloneLightData(layer.getData()));
    }

    return [...next.entries()]
      .sort(([leftY], [rightY]) => leftY - rightY)
      .map(([y, data]) => ({ y, data }));
  }

  private cloneLightData(data: Uint8Array): Uint8Array {
    return new Uint8Array(data);
  }

  private getSkyLightValue(chunkX: number, chunkZ: number, pos: BlockPos): number {
    const minSectionY = SectionPos.blockToSectionCoord(pos.getY());
    const maxSectionY = SectionPos.blockToSectionCoord(this.getMaxBuildHeight()) + 1;
    for (let sectionY = minSectionY; sectionY <= maxSectionY; sectionY++) {
      const value = this.getLightSectionValue(this.skyLightSections, chunkX, sectionY, chunkZ, pos);
      if (value !== undefined) {
        return value;
      }
    }

    return getLightLayerSurrounding(LightLayer.SKY);
  }

  private getLightSectionValue(
    sections: ReadonlyMap<string, DataLayer>,
    chunkX: number,
    sectionY: number,
    chunkZ: number,
    pos: BlockPos,
  ): number | undefined {
    const section = sections.get(lightSectionKey(chunkX, sectionY, chunkZ));
    if (section === undefined) {
      return undefined;
    }

    return section.get(
      SectionPos.sectionRelative(pos.getX()),
      SectionPos.sectionRelative(pos.getY()),
      SectionPos.sectionRelative(pos.getZ()),
    );
  }
}
