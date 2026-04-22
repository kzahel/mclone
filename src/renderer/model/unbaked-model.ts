import { ResourceLocation } from "../../core/resource-location";

export interface UnbakedModel {
  getDependencies(): readonly ResourceLocation[];
}
