import { bottom, top } from "../../carver/carver-config";
import { NoneFeatureConfiguration } from "./configurations/none-feature-configuration";
import { OVERWORLD_FOSSIL_CONFIGURATION } from "./fossil-feature-defaults";
import { Features } from "./features";

export class UndergroundFeatures {
  public static get MONSTER_ROOM() {
    return Features.MONSTER_ROOM.configured(NoneFeatureConfiguration.INSTANCE)
      .rangeUniform(bottom(), top())
      .squared()
      .count(8);
  }

  public static get FOSSIL() {
    return Features.FOSSIL.configured(OVERWORLD_FOSSIL_CONFIGURATION).rarity(64);
  }
}
