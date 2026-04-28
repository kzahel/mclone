import { BlockPos } from "../../../../core/block-pos";
import { ChunkPos } from "../../../../core/chunk-pos";
import { BoundingBox } from "./bounding-box";
import { WorldgenRandom } from "../../../../worldgen/prng/worldgen-random";
import type { StructurePiece } from "./structure-piece";
import type { StructureFeature } from "../../../../worldgen/levelgen/structure/structure-feature";
import type { WorldGenerator } from "../../../../worldgen/levelgen/world-generator";
import type { WorldGenLevel } from "../../world-gen-level";
import type { StructureFeatureManager } from "../../structure-feature-manager";
import type { FeatureConfiguration } from "../../../../worldgen/levelgen/feature/configurations/feature-configuration";

export abstract class StructureStart<C extends FeatureConfiguration> {
  protected readonly pieces: StructurePiece[] = [];
  protected readonly random = new WorldgenRandom();
  private cachedBoundingBox: BoundingBox | undefined;

  public constructor(
    private readonly feature: StructureFeature<C>,
    private readonly chunkPos: ChunkPos,
    private references: number,
    seed: bigint,
  ) {
    this.random.setLargeFeatureSeed(seed, chunkPos.x, chunkPos.z);
  }

  public abstract generatePieces(
    generator: WorldGenerator,
    chunkPos: ChunkPos,
    biome: import("../../../../worldgen/biome/biome").Biome,
    config: C,
  ): void;

  public getBoundingBox(): BoundingBox {
    if (this.cachedBoundingBox === undefined) {
      if (this.pieces.length === 0) {
        throw new Error("Unable to calculate bounding box without pieces");
      }

      const first = this.pieces[0]!.getBoundingBox();
      const box = new BoundingBox(first.minX(), first.minY(), first.minZ(), first.maxX(), first.maxY(), first.maxZ());
      for (let index = 1; index < this.pieces.length; index++) {
        const pieceBox = this.pieces[index]!.getBoundingBox();
        box.encapsulate(new BlockPos(pieceBox.minX(), pieceBox.minY(), pieceBox.minZ()));
        box.encapsulate(new BlockPos(pieceBox.maxX(), pieceBox.maxY(), pieceBox.maxZ()));
      }
      this.cachedBoundingBox = box;
    }

    return this.cachedBoundingBox;
  }

  public getPieces(): readonly StructurePiece[] {
    return this.pieces;
  }

  public placeInChunk(
    level: WorldGenLevel,
    structureManager: StructureFeatureManager,
    chunkGenerator: WorldGenerator,
    random: WorldgenRandom,
    box: BoundingBox,
    chunkPos: ChunkPos,
  ): void {
    if (this.pieces.length === 0) {
      return;
    }

    const firstBox = this.pieces[0]!.getBoundingBox();
    const center = firstBox.getCenter();
    const pivot = new BlockPos(center.getX(), firstBox.minY(), center.getZ());
    for (const piece of this.pieces) {
      if (!piece.getBoundingBox().intersects(box)) {
        continue;
      }

      piece.postProcess(level, structureManager, chunkGenerator, random, box, chunkPos, pivot);
    }
  }

  public isValid(): boolean {
    return this.pieces.length > 0;
  }

  public getChunkPos(): ChunkPos {
    return this.chunkPos;
  }

  public getLocatePos(): BlockPos {
    return new BlockPos(this.chunkPos.getMinBlockX(), 0, this.chunkPos.getMinBlockZ());
  }

  public canBeReferenced(): boolean {
    return this.references < this.getMaxReferences();
  }

  public addReference(): void {
    this.references++;
  }

  public getReferences(): number {
    return this.references;
  }

  protected getMaxReferences(): number {
    return 1;
  }

  public getFeature(): StructureFeature<C> {
    return this.feature;
  }

  public addPiece(piece: StructurePiece): void {
    this.pieces.push(piece);
    this.cachedBoundingBox = undefined;
  }
}
