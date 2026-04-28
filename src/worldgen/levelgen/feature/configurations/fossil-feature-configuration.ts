import type { ResourceLocation } from "../../../../core/resource-location";
import type { StructureProcessorList } from "../../../../world/level/levelgen/structure/templatesystem/structure-processor-list";
import type { FeatureConfiguration } from "./feature-configuration";

export class FossilFeatureConfiguration implements FeatureConfiguration {
  public constructor(
    public readonly fossilStructures: readonly ResourceLocation[],
    public readonly overlayStructures: readonly ResourceLocation[],
    public readonly fossilProcessors: () => StructureProcessorList,
    public readonly overlayProcessors: () => StructureProcessorList,
    public readonly maxEmptyCornersAllowed: number,
  ) {
    if (this.fossilStructures.length === 0) {
      throw new Error("Fossil structures must not be empty");
    }

    if (this.fossilStructures.length !== this.overlayStructures.length) {
      throw new Error("Fossil and overlay structure lists must have the same length");
    }
  }
}
