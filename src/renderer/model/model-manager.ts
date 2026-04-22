import { ResourceLocation } from "../../core/resource-location";
import { type BakedModel } from "./baked-model";

export class ModelManager {
  private readonly bakedRegistry = new Map<string, BakedModel>();

  public constructor(private missingModel: BakedModel) {}

  public getModel(location: ResourceLocation): BakedModel {
    return this.bakedRegistry.get(location.toString()) ?? this.missingModel;
  }

  public getMissingModel(): BakedModel {
    return this.missingModel;
  }

  public setMissingModel(model: BakedModel): void {
    this.missingModel = model;
  }

  public setModel(location: ResourceLocation, model: BakedModel): void {
    this.bakedRegistry.set(location.toString(), model);
  }
}
