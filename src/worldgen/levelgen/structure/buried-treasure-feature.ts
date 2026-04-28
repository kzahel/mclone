import { BlockPos } from "../../../core/block-pos";
import { ChunkPos } from "../../../core/chunk-pos";
import type { Biome } from "../../biome/biome";
import { WorldgenRandom } from "../../prng/worldgen-random";
import { ProbabilityFeatureConfiguration } from "../feature/configurations/probability-feature-configuration";
import type { WorldGenerator } from "../world-generator";
import { StructureFeature } from "./structure-feature";
import { BuriedTreasurePiece } from "./buried-treasure-pieces";
import { GenerationStep } from "../generation-step";
import { StructureStart } from "../../../world/level/levelgen/structure/structure-start";

const RANDOM_SALT = 10_387_320;

class BuriedTreasureStart extends StructureStart<ProbabilityFeatureConfiguration> {
  public override generatePieces(
    _generator: WorldGenerator,
    chunkPos: ChunkPos,
    _biome: Biome,
    _config: ProbabilityFeatureConfiguration,
  ): void {
    this.addPiece(new BuriedTreasurePiece(new BlockPos(chunkPos.getBlockX(9), 90, chunkPos.getBlockZ(9))));
  }

  public override getLocatePos(): BlockPos {
    const chunkPos = this.getChunkPos();
    return new BlockPos(chunkPos.getBlockX(9), 0, chunkPos.getBlockZ(9));
  }
}

export class BuriedTreasureFeature extends StructureFeature<ProbabilityFeatureConfiguration> {
  public constructor() {
    super(GenerationStep.Decoration.UNDERGROUND_STRUCTURES);
  }

  protected override isFeatureChunk(
    _generator: WorldGenerator,
    seed: bigint,
    random: WorldgenRandom,
    chunkPos: ChunkPos,
    _biome: Biome,
    _potentialPos: ChunkPos,
    config: ProbabilityFeatureConfiguration,
  ): boolean {
    random.setLargeFeatureWithSalt(seed, chunkPos.x, chunkPos.z, RANDOM_SALT);
    return random.nextFloat() < config.probability;
  }

  protected override createStart(
    chunkPos: ChunkPos,
    references: number,
    seed: bigint,
  ): StructureStart<ProbabilityFeatureConfiguration> {
    return new BuriedTreasureStart(this, chunkPos, references, seed);
  }
}
