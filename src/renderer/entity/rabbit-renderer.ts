import { ResourceLocation } from "../../core/resource-location";
import { ModelLayers } from "../model/geom/model-layers";
import { RabbitModel } from "../model/rabbit-model";
import type { EntityRendererProvider } from "./entity-renderer-provider";
import { LivingEntityRenderer } from "./living-entity-renderer";
import type { RenderableRabbit } from "./renderable-entity";

export class RabbitRenderer extends LivingEntityRenderer<RenderableRabbit, RabbitModel<RenderableRabbit>> {
  public constructor(context: EntityRendererProvider.Context) {
    super(context, new RabbitModel(context.bakeLayer(ModelLayers.RABBIT)), 0.3);
  }

  public getTextureLocation(entity: RenderableRabbit): ResourceLocation {
    return entity.getTextureLocation();
  }
}
