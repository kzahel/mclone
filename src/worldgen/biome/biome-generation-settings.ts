import { GenerationStep } from "../levelgen/generation-step";
import type { ConfiguredWorldCarver } from "../carver/configured-world-carver.ts";
import type { ConfiguredFeature } from "../levelgen/feature/configured-feature";
import type { ConfiguredStructureFeature } from "../levelgen/structure/structure-feature";
import type { StructureFeature } from "../levelgen/structure/structure-feature";

export type ConfiguredFeatureSupplier = () => ConfiguredFeature<any, any>;
export type ConfiguredCarverSupplier = () => ConfiguredWorldCarver<any>;
export type ConfiguredStructureFeatureSupplier = () => ConfiguredStructureFeature<any, any>;

function toFeatureSupplier(feature: ConfiguredFeature<any, any> | ConfiguredFeatureSupplier): ConfiguredFeatureSupplier {
  if (typeof feature === "function") {
    return feature;
  }

  return () => feature;
}

function toCarverSupplier(carver: ConfiguredWorldCarver<any> | ConfiguredCarverSupplier): ConfiguredCarverSupplier {
  if (typeof carver === "function") {
    return carver;
  }

  return () => carver;
}

function toStructureSupplier(
  structure: ConfiguredStructureFeature<any, any> | ConfiguredStructureFeatureSupplier,
): ConfiguredStructureFeatureSupplier {
  if (typeof structure === "function") {
    return structure;
  }

  return () => structure;
}

export class BiomeGenerationSettings {
  public static readonly EMPTY = new BiomeGenerationSettings([], [], new Map());

  public constructor(
    private readonly structuresValue: readonly ConfiguredStructureFeatureSupplier[],
    private readonly featuresValue: readonly (readonly ConfiguredFeatureSupplier[])[],
    private readonly carversValue: ReadonlyMap<GenerationStep.Carving, readonly ConfiguredCarverSupplier[]>,
  ) {}

  public structures(): readonly ConfiguredStructureFeatureSupplier[] {
    return this.structuresValue;
  }

  public features(): readonly (readonly ConfiguredFeatureSupplier[])[] {
    return this.featuresValue;
  }

  public carvers(step: GenerationStep.Carving): readonly ConfiguredCarverSupplier[] {
    return this.carversValue.get(step) ?? [];
  }

  public isValidStart(structure: StructureFeature<any>): boolean {
    return this.structuresValue.some((supplier) => supplier().feature === structure);
  }
}

export namespace BiomeGenerationSettings {
  export class Builder {
    private readonly structuresValue: ConfiguredStructureFeatureSupplier[] = [];
    private readonly featuresValue: ConfiguredFeatureSupplier[][] = [];
    private readonly carversValue = new Map<GenerationStep.Carving, ConfiguredCarverSupplier[]>();

    public addStructureStart(
      structure: ConfiguredStructureFeature<any, any> | ConfiguredStructureFeatureSupplier,
    ): Builder {
      this.structuresValue.push(toStructureSupplier(structure));
      return this;
    }

    public addFeature(step: GenerationStep.Decoration, feature: ConfiguredFeature<any, any> | ConfiguredFeatureSupplier): Builder {
      return this.addFeatureAt(step, toFeatureSupplier(feature));
    }

    public addFeatureAt(step: number, feature: ConfiguredFeatureSupplier): Builder {
      this.addFeatureStepsUpTo(step);
      this.featuresValue[step]!.push(feature);
      return this;
    }

    public addCarver(step: GenerationStep.Carving, carver: ConfiguredWorldCarver<any> | ConfiguredCarverSupplier): Builder {
      const suppliers = this.carversValue.get(step) ?? [];
      suppliers.push(toCarverSupplier(carver));
      this.carversValue.set(step, suppliers);
      return this;
    }

    private addFeatureStepsUpTo(step: number): void {
      while (this.featuresValue.length <= step) {
        this.featuresValue.push([]);
      }
    }

    public build(): BiomeGenerationSettings {
      return new BiomeGenerationSettings(
        [...this.structuresValue],
        this.featuresValue.map((features) => [...features]),
        new Map([...this.carversValue.entries()].map(([step, suppliers]) => [step, [...suppliers]] as const)),
      );
    }
  }
}
