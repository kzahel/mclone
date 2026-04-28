import { Registry } from "../../../core/registry";
import { ChunkPos } from "../../../core/chunk-pos";
import { GenerationStep } from "../generation-step";
import { WorldgenRandom } from "../../prng/worldgen-random";
import { intFloorDiv } from "../../../util/mth";
import type { FeatureConfiguration } from "../feature/configurations/feature-configuration";
import type { Biome } from "../../biome/biome";
import type { WorldGenerator } from "../world-generator";
import type { StructureStart } from "../../../world/level/levelgen/structure/structure-start";

export class StructureFeatureConfiguration {
  public constructor(
    public readonly spacing: number,
    public readonly separation: number,
    public readonly salt: number,
  ) {}
}

export class ConfiguredStructureFeature<C extends FeatureConfiguration, F extends StructureFeature<C>> {
  public constructor(
    public readonly feature: F,
    public readonly config: C,
  ) {}

  public step(): GenerationStep.Decoration {
    return this.feature.step();
  }

  public generate(
    generator: WorldGenerator,
    chunkPos: ChunkPos,
    biome: Biome,
    references: number,
    structureConfiguration: StructureFeatureConfiguration,
  ): StructureStart<C> | undefined {
    return this.feature.generate(generator, chunkPos, biome, references, structureConfiguration, this.config);
  }
}

export abstract class StructureFeature<C extends FeatureConfiguration> {
  public static readonly MAX_STRUCTURE_RANGE = 8;

  protected constructor(
    private readonly stepValue: GenerationStep.Decoration,
  ) {}

  public configured(config: C): ConfiguredStructureFeature<C, StructureFeature<C>> {
    return new ConfiguredStructureFeature(this, config);
  }

  public step(): GenerationStep.Decoration {
    return this.stepValue;
  }

  protected linearSeparation(): boolean {
    return true;
  }

  public getPotentialFeatureChunk(
    separationSettings: StructureFeatureConfiguration,
    seed: bigint,
    random: WorldgenRandom,
    x: number,
    z: number,
  ): ChunkPos {
    const spacing = separationSettings.spacing;
    const separation = separationSettings.separation;
    const regionX = intFloorDiv(x, spacing);
    const regionZ = intFloorDiv(z, spacing);
    random.setLargeFeatureWithSalt(seed, regionX, regionZ, separationSettings.salt);
    const maxOffset = spacing - separation;
    const offsetX = this.linearSeparation()
      ? random.nextInt(maxOffset)
      : Math.floor((random.nextInt(maxOffset) + random.nextInt(maxOffset)) / 2);
    const offsetZ = this.linearSeparation()
      ? random.nextInt(maxOffset)
      : Math.floor((random.nextInt(maxOffset) + random.nextInt(maxOffset)) / 2);
    return new ChunkPos((regionX * spacing) + offsetX, (regionZ * spacing) + offsetZ);
  }

  protected isFeatureChunk(
    _generator: WorldGenerator,
    _seed: bigint,
    _random: WorldgenRandom,
    _chunkPos: ChunkPos,
    _biome: Biome,
    _potentialPos: ChunkPos,
    _config: C,
  ): boolean {
    return true;
  }

  public generate(
    generator: WorldGenerator,
    chunkPos: ChunkPos,
    biome: Biome,
    references: number,
    structureConfiguration: StructureFeatureConfiguration,
    featureConfiguration: C,
  ): StructureStart<C> | undefined {
    const random = new WorldgenRandom();
    const potentialPos = this.getPotentialFeatureChunk(structureConfiguration, generator.getSeed(), random, chunkPos.x, chunkPos.z);
    if (
      chunkPos.x !== potentialPos.x
      || chunkPos.z !== potentialPos.z
      || !this.isFeatureChunk(generator, generator.getSeed(), random, chunkPos, biome, potentialPos, featureConfiguration)
    ) {
      return undefined;
    }

    const start = this.createStart(chunkPos, references, generator.getSeed());
    start.generatePieces(generator, chunkPos, biome, featureConfiguration);
    return start.isValid() ? start : undefined;
  }

  protected abstract createStart(chunkPos: ChunkPos, references: number, seed: bigint): StructureStart<C>;
}

export class StructureSettings {
  public constructor(private readonly configs = new Map<StructureFeature<any>, StructureFeatureConfiguration>()) {}

  public getConfig(structure: StructureFeature<any>): StructureFeatureConfiguration | undefined {
    return this.configs.get(structure);
  }
}

export function registerStructureFeature<F extends StructureFeature<any>>(name: string, feature: F): F {
  return Registry.register(Registry.STRUCTURE_FEATURE, name, feature) as F;
}
