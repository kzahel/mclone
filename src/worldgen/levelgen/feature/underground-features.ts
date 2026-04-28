import { bottom, top } from "../../carver/carver-config";
import { NoneFeatureConfiguration } from "./configurations/none-feature-configuration";
import { Features } from "./features";

export class UndergroundFeatures {
  public static get MONSTER_ROOM() {
    return Features.MONSTER_ROOM.configured(NoneFeatureConfiguration.INSTANCE)
      .rangeUniform(bottom(), top())
      .squared()
      .count(8);
  }
}
