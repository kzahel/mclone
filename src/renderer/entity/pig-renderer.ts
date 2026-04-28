import { ResourceLocation } from "../../core/resource-location";
import { ModelLayers } from "../model/geom/model-layers";
import { PigModel } from "../model/pig-model";
import type { EntityRendererProvider } from "./entity-renderer-provider";
import { LivingEntityRenderer } from "./living-entity-renderer";
import type { RenderablePig } from "./renderable-entity";

const PIG_LOCATION = new ResourceLocation("minecraft", "textures/entity/pig/pig.png");

export class PigRenderer extends LivingEntityRenderer<RenderablePig, PigModel<RenderablePig>> {
  public constructor(context: EntityRendererProvider.Context) {
    super(context, new PigModel(context.bakeLayer(ModelLayers.PIG)), 0.7);
    // Runtime: saddle layer waits for pig saddle data/interactions.
  }

  public getTextureLocation(entity: RenderablePig): ResourceLocation {
    return entity.getTextureLocation();
  }
}

export { PIG_LOCATION };
