import { ProbabilityFeatureConfiguration } from "../feature/configurations/probability-feature-configuration";
import { BuriedTreasureFeature } from "./buried-treasure-feature";
import {
  registerStructureFeature,
  StructureFeatureConfiguration,
  StructureSettings,
} from "./structure-feature";

export const StructureFeatures = {
  BURIED_TREASURE: registerStructureFeature("buried_treasure", new BuriedTreasureFeature()),
} as const;

export const ConfiguredStructureFeatures = {
  BURIED_TREASURE: StructureFeatures.BURIED_TREASURE.configured(new ProbabilityFeatureConfiguration(0.01)),
} as const;

export const OVERWORLD_STRUCTURE_SETTINGS = new StructureSettings(
  new Map([
    [StructureFeatures.BURIED_TREASURE, new StructureFeatureConfiguration(1, 0, 0)],
  ]),
);
