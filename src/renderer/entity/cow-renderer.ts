import { ResourceLocation } from "../../core/resource-location";
import { ModelLayers } from "../model/geom/model-layers";
import { CowModel } from "../model/cow-model";
import type { EntityRendererProvider } from "./entity-renderer-provider";
import { LivingEntityRenderer } from "./living-entity-renderer";
import type { RenderableCow } from "./renderable-entity";

const COW_LOCATION = new ResourceLocation("minecraft", "textures/entity/cow/cow.png");

export class CowRenderer extends LivingEntityRenderer<RenderableCow, CowModel<RenderableCow>> {
  public constructor(context: EntityRendererProvider.Context) {
    super(context, new CowModel(context.bakeLayer(ModelLayers.COW)), 0.7);
  }

  public getTextureLocation(entity: RenderableCow): ResourceLocation {
    return entity.getTextureLocation();
  }
}

export { COW_LOCATION };
