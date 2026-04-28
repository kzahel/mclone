import { ResourceLocation } from "../../core/resource-location";
import { ModelLayers } from "../model/geom/model-layers";
import { CowModel } from "../model/cow-model";
import type { EntityRendererProvider } from "./entity-renderer-provider";
import { LivingEntityRenderer } from "./living-entity-renderer";
import type { RenderableMooshroom } from "./renderable-entity";

const BROWN_MOOSHROOM_LOCATION = new ResourceLocation("minecraft", "textures/entity/cow/brown_mooshroom.png");
const RED_MOOSHROOM_LOCATION = new ResourceLocation("minecraft", "textures/entity/cow/red_mooshroom.png");

export class MooshroomRenderer extends LivingEntityRenderer<RenderableMooshroom, CowModel<RenderableMooshroom>> {
  public constructor(context: EntityRendererProvider.Context) {
    super(context, new CowModel(context.bakeLayer(ModelLayers.MOOSHROOM)), 0.7);
  }

  public getTextureLocation(entity: RenderableMooshroom): ResourceLocation {
    return entity.getMushroomType() === "brown" ? BROWN_MOOSHROOM_LOCATION : RED_MOOSHROOM_LOCATION;
  }
}

export { BROWN_MOOSHROOM_LOCATION, RED_MOOSHROOM_LOCATION };
