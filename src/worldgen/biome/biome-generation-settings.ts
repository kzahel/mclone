import { GenerationStep } from "../levelgen/generation-step";
import type { ConfiguredWorldCarver } from "../carver/configured-world-carver.ts";
import type { ConfiguredFeature } from "../levelgen/feature/configured-feature";

export type ConfiguredFeatureSupplier = () => ConfiguredFeature<any, any>;
export type ConfiguredCarverSupplier = () => ConfiguredWorldCarver<any>;

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

export class BiomeGenerationSettings {
  public static readonly EMPTY = new BiomeGenerationSettings([], new Map());

  public constructor(
    private readonly featuresValue: readonly (readonly ConfiguredFeatureSupplier[])[],
    private readonly carversValue: ReadonlyMap<GenerationStep.Carving, readonly ConfiguredCarverSupplier[]>,
  ) {}

  public features(): readonly (readonly ConfiguredFeatureSupplier[])[] {
    return this.featuresValue;
  }

  public carvers(step: GenerationStep.Carving): readonly ConfiguredCarverSupplier[] {
    return this.carversValue.get(step) ?? [];
  }
}

export namespace BiomeGenerationSettings {
  export class Builder {
    private readonly featuresValue: ConfiguredFeatureSupplier[][] = [];
    private readonly carversValue = new Map<GenerationStep.Carving, ConfiguredCarverSupplier[]>();

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
        this.featuresValue.map((features) => [...features]),
        new Map([...this.carversValue.entries()].map(([step, suppliers]) => [step, [...suppliers]] as const)),
      );
    }
  }
}
