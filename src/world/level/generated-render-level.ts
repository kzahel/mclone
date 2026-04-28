import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import type { ColorResolver } from "./color-resolver";
import type { Biome } from "../../worldgen/biome/biome";
import { getBlockPositionBiome } from "../../worldgen/biome/biome-zoom";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import { MutableChunkBlockBuffer } from "../../worldgen/chunk/chunk-block-buffer";
import { GenerationStep } from "../../worldgen/levelgen/generation-step";
import { Heightmap } from "../../worldgen/levelgen/heightmap";
import type { CooperativeGenerationYield } from "../../worldgen/levelgen/cooperative-generation";
import type { BiomeDecorationProfiler } from "../../worldgen/levelgen/decoration-profiler";
import type { WorldGenerator } from "../../worldgen/levelgen/world-generator";
import { BlockStateProperties } from "./block/state/properties/block-state-properties";
import { type BlockState } from "./block/state/block-state";
import { LevelChunk } from "./chunk/level-chunk";
import type { FluidState } from "./material/fluid-state";
import {
  GeneratedChunkStatus,
  getGeneratedChunkDependencyStatus,
  isGeneratedChunkStatusAtLeast,
  type GeneratedChunkStatus as GeneratedChunkStatusName,
} from "./generated-chunk-status";
import {
  FEATURES_CHUNK_DEPENDENCY_RADIUS,
  GeneratedDecorationRegion,
  type GeneratedDecorationMetrics,
  type GeneratedDecorationOptions,
} from "./generated-decoration-region";
import { StaticRenderLevel } from "./static-render-level";

function generatedChunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

interface GeneratedChunkRecord {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly status: GeneratedChunkStatusName;
  readonly hasBlockSections: boolean;
}

export interface GeneratedChunkStatusDebugRecord {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly status: GeneratedChunkStatusName;
  readonly hasBlockSections: boolean;
}

export type GeneratedLevelPhaseRecorder = (phase: string, elapsedMs: number) => void;

const FULL_PUBLICATION_NEIGHBOR_RADIUS = 1;
const LIGHT_FEATURES_NEIGHBOR_RADIUS = 1;

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
  private readonly chunkRecords = new Map<string, GeneratedChunkRecord>();
  private readonly advancingFeatureChunks = new Set<string>();
  private automaticDecorationEnabled = true;
  private phaseRecorder: GeneratedLevelPhaseRecorder | undefined;

  public constructor(
    airState: BlockState,
    private readonly generator: WorldGenerator,
    private readonly blockStateById: readonly BlockState[],
    skyLight = 15,
    blockLight = 15,
    minBuildHeight = 0,
    height = 256,
  ) {
    super(airState, skyLight, blockLight, minBuildHeight, height);
  }

  public setPhaseRecorder(recorder: GeneratedLevelPhaseRecorder | undefined): void {
    this.phaseRecorder = recorder;
  }

  public override getChunk(chunkX: number, chunkZ: number, create = true): LevelChunk | null {
    const existing = super.getChunk(chunkX, chunkZ, false);
    if (existing !== null) {
      if (
        create
        && this.automaticDecorationEnabled
        && this.inPublishRange(chunkX, chunkZ)
        && !this.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES)
        && !this.advancingFeatureChunks.has(generatedChunkKey(chunkX, chunkZ))
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

  public override setChunk(
    chunk: LevelChunk,
    status: GeneratedChunkStatusName | boolean = GeneratedChunkStatus.LIQUID_CARVERS,
  ): void {
    super.setChunk(chunk);
    const nextStatus = typeof status === "boolean"
      ? (status ? GeneratedChunkStatus.FULL : GeneratedChunkStatus.LIQUID_CARVERS)
      : status;
    this.setChunkStatus(chunk.chunkX, chunk.chunkZ, nextStatus);
    this.advancingFeatureChunks.delete(generatedChunkKey(chunk.chunkX, chunk.chunkZ));
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
    // Runtime: retain status records far enough for publishable 3x3 FULL, LIGHT's 3x3 FEATURES input, and FEATURES' dependency window.
    const nextAuthorityRadius =
      nextPublishRadius
      + FULL_PUBLICATION_NEIGHBOR_RADIUS
      + LIGHT_FEATURES_NEIGHBOR_RADIUS
      + FEATURES_CHUNK_DEPENDENCY_RADIUS;
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
        this.chunkRecords.delete(generatedChunkKey(chunk.chunkX, chunk.chunkZ));
        this.advancingFeatureChunks.delete(generatedChunkKey(chunk.chunkX, chunk.chunkZ));
        changed = true;
        continue;
      }

      if (wasPublished && !this.inPublishRange(chunk.chunkX, chunk.chunkZ)) {
        unloadedChunks.push(chunk);
        changed = true;
      }
    }

    for (const [key, record] of this.chunkRecords) {
      if (!this.inAuthorityRange(record.chunkX, record.chunkZ)) {
        this.chunkRecords.delete(key);
        this.advancingFeatureChunks.delete(key);
        changed = true;
      }
    }

    const missingChunks: Array<readonly [number, number]> = [];
    for (let chunkZ = this.viewCenterZ - this.authorityChunkRadius; chunkZ <= this.viewCenterZ + this.authorityChunkRadius; chunkZ++) {
      for (let chunkX = this.viewCenterX - this.authorityChunkRadius; chunkX <= this.viewCenterX + this.authorityChunkRadius; chunkX++) {
        const chunk = super.getChunk(chunkX, chunkZ, false);
        if (
          this.inPublishRange(chunkX, chunkZ)
          && (
            chunk === null
            || !this.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES)
          )
        ) {
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

    const biome = getBlockPositionBiome(this.generator.getSeed(), pos.getX(), pos.getZ(), this.generator.getBiomeSource()) as Biome;
    return resolver.getColor(biome, pos.getX(), pos.getZ());
  }

  public override getBiome(pos: BlockPos): Biome {
    return getBlockPositionBiome(this.generator.getSeed(), pos.getX(), pos.getZ(), this.generator.getBiomeSource()) as Biome;
  }

  public getAuthorityChunk(chunkX: number, chunkZ: number): LevelChunk | null {
    return super.getChunk(chunkX, chunkZ, false);
  }

  public getAuthorityBlockState(pos: BlockPos, metrics?: GeneratedDecorationMetrics): BlockState {
    const chunkX = SectionPos.blockToSectionCoord(pos.getX());
    const chunkZ = SectionPos.blockToSectionCoord(pos.getZ());
    const chunk = super.getChunk(chunkX, chunkZ, false);
    if (chunk !== null) {
      return chunk.getBlockState(pos);
    }

    if (this.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_STARTS)) {
      if (metrics !== undefined) {
        metrics.metadataOnlyBlockReads++;
      }
      return this.airState;
    }

    throw new Error(`Missing authority status for block read at chunk (${chunkX.toString()}, ${chunkZ.toString()})`);
  }

  public getAuthorityFluidState(pos: BlockPos, metrics?: GeneratedDecorationMetrics): FluidState {
    return this.getAuthorityBlockState(pos, metrics).getFluidState();
  }

  public getAuthorityHeight(
    type: Heightmap.Types,
    x: number,
    z: number,
    _metrics?: GeneratedDecorationMetrics,
  ): number {
    const chunkX = SectionPos.blockToSectionCoord(x);
    const chunkZ = SectionPos.blockToSectionCoord(z);
    const chunk = super.getChunk(chunkX, chunkZ, false);
    if (chunk !== null) {
      return chunk.getHeight(type, x, z) + 1;
    }

    if (this.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_STARTS)) {
      return this.getMinBuildHeight();
    }

    throw new Error(`Missing authority status for height read at chunk (${chunkX.toString()}, ${chunkZ.toString()})`);
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
    return this.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES);
  }

  public getChunkStatus(chunkX: number, chunkZ: number): GeneratedChunkStatusName {
    return this.getChunkRecord(chunkX, chunkZ)?.status ?? GeneratedChunkStatus.EMPTY;
  }

  public hasMaterializedChunk(chunkX: number, chunkZ: number): boolean {
    return this.getChunkRecord(chunkX, chunkZ)?.hasBlockSections ?? false;
  }

  public getDebugChunkStatusRecords(): readonly GeneratedChunkStatusDebugRecord[] {
    return [...this.chunkRecords.values()]
      .map((record) => ({
        chunkX: record.chunkX,
        chunkZ: record.chunkZ,
        status: record.status,
        hasBlockSections: record.hasBlockSections,
      }))
      .sort((left, right) => left.chunkZ - right.chunkZ || left.chunkX - right.chunkX);
  }

  public hasChunkStatus(chunkX: number, chunkZ: number, status: GeneratedChunkStatusName): boolean {
    return isGeneratedChunkStatusAtLeast(this.getChunkStatus(chunkX, chunkZ), status);
  }

  public markMetadataStatusAtLeast(chunkX: number, chunkZ: number, status: GeneratedChunkStatusName): boolean {
    if (!this.inAuthorityRange(chunkX, chunkZ)) {
      return false;
    }

    switch (status) {
      case GeneratedChunkStatus.EMPTY:
        return true;
      case GeneratedChunkStatus.STRUCTURE_STARTS:
        this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_STARTS);
        return true;
      case GeneratedChunkStatus.STRUCTURE_REFERENCES:
        this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_STARTS);
        this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_REFERENCES);
        return true;
      case GeneratedChunkStatus.BIOMES:
        this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_STARTS);
        this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_REFERENCES);
        this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.BIOMES);
        return true;
      default:
        return this.hasChunkStatus(chunkX, chunkZ, status);
    }
  }

  public hasStatusWindow(
    centerChunkX: number,
    centerChunkZ: number,
    radius: number,
    status: GeneratedChunkStatusName,
  ): boolean {
    for (let chunkZ = centerChunkZ - radius; chunkZ <= centerChunkZ + radius; chunkZ++) {
      for (let chunkX = centerChunkX - radius; chunkX <= centerChunkX + radius; chunkX++) {
        if (!this.hasChunkStatus(chunkX, chunkZ, status)) {
          return false;
        }
      }
    }

    return true;
  }

  public isChunkFeaturesStable(chunkX: number, chunkZ: number): boolean {
    return this.hasStatusWindow(chunkX, chunkZ, 1, GeneratedChunkStatus.FEATURES);
  }

  public isChunkFull(chunkX: number, chunkZ: number): boolean {
    return this.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FULL);
  }

  public isChunkPublishable(chunkX: number, chunkZ: number): boolean {
    return this.hasStatusWindow(chunkX, chunkZ, 1, GeneratedChunkStatus.FULL);
  }

  public markChunkLighted(chunkX: number, chunkZ: number): boolean {
    if (!this.isChunkFeaturesStable(chunkX, chunkZ)) {
      return false;
    }

    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.LIGHT);
    return true;
  }

  public markChunkFull(chunkX: number, chunkZ: number): boolean {
    if (!this.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.LIGHT)) {
      return false;
    }

    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.SPAWN);
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.HEIGHTMAPS);
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.FULL);
    return true;
  }

  public generateChunkTerrain(chunkX: number, chunkZ: number): LevelChunk | null {
    const existing = super.getChunk(chunkX, chunkZ, false);
    if (existing !== null) {
      this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.LIQUID_CARVERS);
      return existing;
    }

    if (!this.inAuthorityRange(chunkX, chunkZ)) {
      return null;
    }

    this.advanceNoopPreNoiseStatuses(chunkX, chunkZ);
    const generated = this.recordPhase("terrain.fill_from_noise", () => this.generator.fillFromNoise(chunkX, chunkZ));
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.NOISE);
    this.recordPhase("terrain.surface_bedrock", () => this.generator.buildSurfaceAndBedrock(generated));
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.SURFACE);
    this.recordPhase("terrain.carvers_air", () => this.generator.applyCarvers(generated, GenerationStep.Carving.AIR));
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.CARVERS);
    this.recordPhase("terrain.carvers_liquid", () => this.generator.applyCarvers(generated, GenerationStep.Carving.LIQUID));
    const chunk = this.recordPhase("terrain.copy_to_level_chunk", () => this.copyGeneratedChunk(chunkX, chunkZ, generated));
    super.setChunk(chunk);
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.LIQUID_CARVERS);
    return chunk;
  }

  public async generateChunkTerrainCooperative(
    chunkX: number,
    chunkZ: number,
    yieldStep: CooperativeGenerationYield,
  ): Promise<LevelChunk | null> {
    const existing = super.getChunk(chunkX, chunkZ, false);
    if (existing !== null) {
      this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.LIQUID_CARVERS);
      return existing;
    }

    if (!this.inAuthorityRange(chunkX, chunkZ)) {
      return null;
    }

    this.advanceNoopPreNoiseStatuses(chunkX, chunkZ);
    const generated = this.recordPhase("terrain.fill_from_noise", () => this.generator.fillFromNoise(chunkX, chunkZ));
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.NOISE);
    await yieldStep();
    this.recordPhase("terrain.surface_bedrock", () => this.generator.buildSurfaceAndBedrock(generated));
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.SURFACE);
    await yieldStep();
    this.recordPhase("terrain.carvers_air", () => this.generator.applyCarvers(generated, GenerationStep.Carving.AIR));
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.CARVERS);
    await yieldStep();
    this.recordPhase("terrain.carvers_liquid", () => this.generator.applyCarvers(generated, GenerationStep.Carving.LIQUID));
    const chunk = this.recordPhase("terrain.copy_to_level_chunk", () => this.copyGeneratedChunk(chunkX, chunkZ, generated));
    super.setChunk(chunk);
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.LIQUID_CARVERS);
    await yieldStep();
    return chunk;
  }

  public decorateChunk(
    chunkX: number,
    chunkZ: number,
    options: GeneratedDecorationOptions & { readonly profiler?: BiomeDecorationProfiler } = {},
  ): void {
    const key = generatedChunkKey(chunkX, chunkZ);
    if (this.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES)) {
      return;
    }

    if (this.advancingFeatureChunks.has(key)) {
      return;
    }

    this.advancingFeatureChunks.add(key);
    try {
      this.recordPhase("features.ensure_status_window", () => this.ensureDecorationStatusWindow(chunkX, chunkZ));
      this.recordPhase("features.prime_heightmaps", () => this.primeFeatureHeightmaps(chunkX, chunkZ));
      this.recordPhase("features.apply_biome_decoration", () => this.generator.applyBiomeDecoration(
        new GeneratedDecorationRegion(this, chunkX, chunkZ, undefined, undefined, options),
        chunkX,
        chunkZ,
        options.profiler,
      ));
      this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.FEATURES);
    } finally {
      this.advancingFeatureChunks.delete(key);
    }
  }

  public async decorateChunkCooperative(
    chunkX: number,
    chunkZ: number,
    yieldStep: CooperativeGenerationYield,
    options: GeneratedDecorationOptions & { readonly profiler?: BiomeDecorationProfiler } = {},
  ): Promise<void> {
    const key = generatedChunkKey(chunkX, chunkZ);
    if (this.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES)) {
      return;
    }

    if (this.advancingFeatureChunks.has(key)) {
      return;
    }

    this.advancingFeatureChunks.add(key);
    try {
      await this.recordPhaseAsync("features.ensure_status_window", () =>
        this.ensureDecorationStatusWindowCooperative(chunkX, chunkZ, yieldStep)
      );
      this.recordPhase("features.prime_heightmaps", () => this.primeFeatureHeightmaps(chunkX, chunkZ));
      await this.recordPhaseAsync("features.apply_biome_decoration", () => this.generator.applyBiomeDecorationCooperative(
        new GeneratedDecorationRegion(this, chunkX, chunkZ, undefined, undefined, options),
        chunkX,
        chunkZ,
        yieldStep,
        options.profiler,
      ));
      this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.FEATURES);
    } finally {
      this.advancingFeatureChunks.delete(key);
    }
  }

  private primeFeatureHeightmaps(chunkX: number, chunkZ: number): void {
    const chunk = this.getAuthorityChunk(chunkX, chunkZ);
    if (chunk === null) {
      throw new Error(`Missing center chunk (${chunkX.toString()}, ${chunkZ.toString()}) while priming FEATURES heightmaps`);
    }

    chunk.primeHeightmaps([
      Heightmap.Types.MOTION_BLOCKING,
      Heightmap.Types.MOTION_BLOCKING_NO_LEAVES,
      Heightmap.Types.OCEAN_FLOOR,
      Heightmap.Types.WORLD_SURFACE,
    ]);
  }

  private ensureDecorationStatusWindow(chunkX: number, chunkZ: number): void {
    for (let neighborChunkZ = chunkZ - FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkZ <= chunkZ + FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkZ++) {
      for (let neighborChunkX = chunkX - FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkX <= chunkX + FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkX++) {
        const dependencyRadius = Math.max(Math.abs(neighborChunkX - chunkX), Math.abs(neighborChunkZ - chunkZ));
        const requiredStatus = getGeneratedChunkDependencyStatus(GeneratedChunkStatus.FEATURES, dependencyRadius);
        if (this.hasChunkStatus(neighborChunkX, neighborChunkZ, requiredStatus)) {
          continue;
        }

        if (!this.ensureChunkStatus(neighborChunkX, neighborChunkZ, requiredStatus)) {
          throw new Error(
            `Missing authority ${requiredStatus} chunk for decoration window at (${neighborChunkX.toString()}, ${neighborChunkZ.toString()}) around (${chunkX.toString()}, ${chunkZ.toString()})`,
          );
        }
      }
    }
  }

  private async ensureDecorationStatusWindowCooperative(
    chunkX: number,
    chunkZ: number,
    yieldStep: CooperativeGenerationYield,
  ): Promise<void> {
    for (let neighborChunkZ = chunkZ - FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkZ <= chunkZ + FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkZ++) {
      for (let neighborChunkX = chunkX - FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkX <= chunkX + FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkX++) {
        const dependencyRadius = Math.max(Math.abs(neighborChunkX - chunkX), Math.abs(neighborChunkZ - chunkZ));
        const requiredStatus = getGeneratedChunkDependencyStatus(GeneratedChunkStatus.FEATURES, dependencyRadius);
        if (this.hasChunkStatus(neighborChunkX, neighborChunkZ, requiredStatus)) {
          continue;
        }

        if (!await this.ensureChunkStatusCooperative(neighborChunkX, neighborChunkZ, requiredStatus, yieldStep)) {
          throw new Error(
            `Missing authority ${requiredStatus} chunk for decoration window at (${neighborChunkX.toString()}, ${neighborChunkZ.toString()}) around (${chunkX.toString()}, ${chunkZ.toString()})`,
          );
        }
      }
    }
  }

  private ensureChunkStatus(chunkX: number, chunkZ: number, status: GeneratedChunkStatusName): boolean {
    if (this.hasChunkStatus(chunkX, chunkZ, status)) {
      return true;
    }

    if (!this.inAuthorityRange(chunkX, chunkZ)) {
      return false;
    }

    switch (status) {
      case GeneratedChunkStatus.EMPTY:
        return true;
      case GeneratedChunkStatus.STRUCTURE_STARTS:
        this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_STARTS);
        return true;
      case GeneratedChunkStatus.STRUCTURE_REFERENCES:
        this.ensureChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_STARTS);
        this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_REFERENCES);
        return true;
      case GeneratedChunkStatus.BIOMES:
        this.ensureChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_REFERENCES);
        this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.BIOMES);
        return true;
      case GeneratedChunkStatus.NOISE:
      case GeneratedChunkStatus.SURFACE:
      case GeneratedChunkStatus.CARVERS:
      case GeneratedChunkStatus.LIQUID_CARVERS:
        return this.generateChunkTerrain(chunkX, chunkZ) !== null && this.hasChunkStatus(chunkX, chunkZ, status);
      case GeneratedChunkStatus.FEATURES:
        this.decorateChunk(chunkX, chunkZ);
        return this.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES);
      case GeneratedChunkStatus.LIGHT:
        return this.markChunkLighted(chunkX, chunkZ);
      case GeneratedChunkStatus.SPAWN:
      case GeneratedChunkStatus.HEIGHTMAPS:
      case GeneratedChunkStatus.FULL:
        return this.markChunkFull(chunkX, chunkZ);
    }
  }

  private async ensureChunkStatusCooperative(
    chunkX: number,
    chunkZ: number,
    status: GeneratedChunkStatusName,
    yieldStep: CooperativeGenerationYield,
  ): Promise<boolean> {
    if (this.hasChunkStatus(chunkX, chunkZ, status)) {
      return true;
    }

    if (!this.inAuthorityRange(chunkX, chunkZ)) {
      return false;
    }

    switch (status) {
      case GeneratedChunkStatus.EMPTY:
        return true;
      case GeneratedChunkStatus.STRUCTURE_STARTS:
        this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_STARTS);
        return true;
      case GeneratedChunkStatus.STRUCTURE_REFERENCES:
        await this.ensureChunkStatusCooperative(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_STARTS, yieldStep);
        this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_REFERENCES);
        return true;
      case GeneratedChunkStatus.BIOMES:
        await this.ensureChunkStatusCooperative(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_REFERENCES, yieldStep);
        this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.BIOMES);
        return true;
      case GeneratedChunkStatus.NOISE:
      case GeneratedChunkStatus.SURFACE:
      case GeneratedChunkStatus.CARVERS:
      case GeneratedChunkStatus.LIQUID_CARVERS:
        return await this.generateChunkTerrainCooperative(chunkX, chunkZ, yieldStep) !== null
          && this.hasChunkStatus(chunkX, chunkZ, status);
      case GeneratedChunkStatus.FEATURES:
        await this.decorateChunkCooperative(chunkX, chunkZ, yieldStep);
        return this.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES);
      case GeneratedChunkStatus.LIGHT:
        return this.markChunkLighted(chunkX, chunkZ);
      case GeneratedChunkStatus.SPAWN:
      case GeneratedChunkStatus.HEIGHTMAPS:
      case GeneratedChunkStatus.FULL:
        return this.markChunkFull(chunkX, chunkZ);
    }
  }

  private copyGeneratedChunk(chunkX: number, chunkZ: number, generated: MutableChunkBlockBuffer): LevelChunk {
    const chunk = new LevelChunk(chunkX, chunkZ, this.blockStateById[0]!, this.getMinBuildHeight(), this.getHeight());
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

    chunk.primeHeightmaps([
      Heightmap.Types.OCEAN_FLOOR_WG,
      Heightmap.Types.WORLD_SURFACE_WG,
    ]);
    for (const step of GenerationStep.CARVING_VALUES) {
      const carvingMask = generated.getCarvingMask(step);
      if (carvingMask !== undefined) {
        chunk.setCarvingMask(step, carvingMask);
      }
    }
    chunk.appendBlockTicks(generated.getScheduledBlockTicks());
    chunk.appendLiquidTicks(generated.getScheduledLiquidTicks());

    return chunk;
  }

  private advanceNoopPreNoiseStatuses(chunkX: number, chunkZ: number): void {
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_STARTS);
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.STRUCTURE_REFERENCES);
    this.setChunkStatusAtLeast(chunkX, chunkZ, GeneratedChunkStatus.BIOMES);
  }

  private setChunkStatusAtLeast(chunkX: number, chunkZ: number, status: GeneratedChunkStatusName): void {
    if (this.hasChunkStatus(chunkX, chunkZ, status)) {
      return;
    }

    this.setChunkStatus(chunkX, chunkZ, status);
  }

  private setChunkStatus(chunkX: number, chunkZ: number, status: GeneratedChunkStatusName): void {
    const key = generatedChunkKey(chunkX, chunkZ);
    const existing = this.chunkRecords.get(key);
    this.chunkRecords.set(key, {
      chunkX,
      chunkZ,
      status,
      hasBlockSections: (existing?.hasBlockSections ?? false) || super.getChunk(chunkX, chunkZ, false) !== null,
    });
  }

  private getChunkRecord(chunkX: number, chunkZ: number): GeneratedChunkRecord | undefined {
    const key = generatedChunkKey(chunkX, chunkZ);
    const existing = this.chunkRecords.get(key);
    if (existing !== undefined) {
      return existing;
    }

    if (super.getChunk(chunkX, chunkZ, false) === null) {
      return undefined;
    }

    const record: GeneratedChunkRecord = {
      chunkX,
      chunkZ,
      status: GeneratedChunkStatus.LIQUID_CARVERS,
      hasBlockSections: true,
    };
    this.chunkRecords.set(key, record);
    return record;
  }

  private recordPhase<T>(phase: string, run: () => T): T {
    const startedAtMs = performance.now();
    try {
      return run();
    } finally {
      this.phaseRecorder?.(phase, performance.now() - startedAtMs);
    }
  }

  private async recordPhaseAsync<T>(phase: string, run: () => Promise<T>): Promise<T> {
    const startedAtMs = performance.now();
    try {
      return await run();
    } finally {
      this.phaseRecorder?.(phase, performance.now() - startedAtMs);
    }
  }
}
