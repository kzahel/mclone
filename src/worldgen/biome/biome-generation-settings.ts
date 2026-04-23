import { GenerationStep } from "../levelgen/generation-step";
import type { ConfiguredFeature } from "../levelgen/feature/configured-feature";

export type ConfiguredFeatureSupplier = () => ConfiguredFeature<any, any>;

function toFeatureSupplier(feature: ConfiguredFeature<any, any> | ConfiguredFeatureSupplier): ConfiguredFeatureSupplier {
  if (typeof feature === "function") {
    return feature;
  }

  return () => feature;
}

export class BiomeGenerationSettings {
  public static readonly EMPTY = new BiomeGenerationSettings([]);

  public constructor(private readonly featuresValue: readonly (readonly ConfiguredFeatureSupplier[])[]) {}

  public features(): readonly (readonly ConfiguredFeatureSupplier[])[] {
    return this.featuresValue;
  }
}

export namespace BiomeGenerationSettings {
  export class Builder {
    private readonly featuresValue: ConfiguredFeatureSupplier[][] = [];

    public addFeature(step: GenerationStep.Decoration, feature: ConfiguredFeature<any, any> | ConfiguredFeatureSupplier): Builder {
      return this.addFeatureAt(step, toFeatureSupplier(feature));
    }

    public addFeatureAt(step: number, feature: ConfiguredFeatureSupplier): Builder {
      this.addFeatureStepsUpTo(step);
      this.featuresValue[step]!.push(feature);
      return this;
    }

    private addFeatureStepsUpTo(step: number): void {
      while (this.featuresValue.length <= step) {
        this.featuresValue.push([]);
      }
    }

    public build(): BiomeGenerationSettings {
      return new BiomeGenerationSettings(this.featuresValue.map((features) => [...features]));
    }
  }
}
