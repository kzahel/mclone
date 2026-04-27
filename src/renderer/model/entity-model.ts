import { RenderType } from "../render-type";
import type { ResourceLocation } from "../../core/resource-location";
import { Model } from "./model";

export abstract class EntityModel<T> extends Model {
  public attackTime = 0;
  public riding = false;
  public young = true;

  protected constructor(renderType: (location: ResourceLocation) => RenderType = RenderType.entityCutoutNoCull) {
    super(renderType);
  }

  public abstract setupAnim(entity: T, limbSwing: number, limbSwingAmount: number, ageInTicks: number, netHeadYaw: number, headPitch: number): void;

  public prepareMobModel(_entity: T, _limbSwing: number, _limbSwingAmount: number, _partialTick: number): void {}

  public copyPropertiesTo(otherModel: EntityModel<T>): void {
    otherModel.attackTime = this.attackTime;
    otherModel.riding = this.riding;
    otherModel.young = this.young;
  }
}
