import { ResourceLocation } from "../../core/resource-location";
import { ModelLayers } from "../model/geom/model-layers";
import { WolfModel } from "../model/wolf-model";
import type { EntityRendererProvider } from "./entity-renderer-provider";
import { LivingEntityRenderer } from "./living-entity-renderer";
import type { RenderableWolf } from "./renderable-entity";

export class WolfRenderer extends LivingEntityRenderer<RenderableWolf, WolfModel<RenderableWolf>> {
  public constructor(context: EntityRendererProvider.Context) {
    super(context, new WolfModel(context.bakeLayer(ModelLayers.WOLF)), 0.5);
  }

  public getTextureLocation(entity: RenderableWolf): ResourceLocation {
    return entity.getTextureLocation();
  }

  protected override getBob(livingBase: RenderableWolf, _partialTicks: number): number {
    return livingBase.getTailAngle();
  }
}
