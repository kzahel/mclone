import { ResourceLocation } from "../../core/resource-location";
import { lerp } from "../../util/mth";
import { ChickenModel } from "../model/chicken-model";
import { ModelLayers } from "../model/geom/model-layers";
import type { EntityRendererProvider } from "./entity-renderer-provider";
import { LivingEntityRenderer } from "./living-entity-renderer";
import type { RenderableChicken } from "./renderable-entity";

const CHICKEN_LOCATION = new ResourceLocation("minecraft", "textures/entity/chicken.png");

export class ChickenRenderer extends LivingEntityRenderer<RenderableChicken, ChickenModel<RenderableChicken>> {
  public constructor(context: EntityRendererProvider.Context) {
    super(context, new ChickenModel(context.bakeLayer(ModelLayers.CHICKEN)), 0.3);
  }

  public getTextureLocation(entity: RenderableChicken): ResourceLocation {
    return entity.getTextureLocation();
  }

  protected override getBob(entity: RenderableChicken, partialTicks: number): number {
    const flap = lerp(partialTicks, entity.getOFlap(), entity.getFlap());
    const flapSpeed = lerp(partialTicks, entity.getOFlapSpeed(), entity.getFlapSpeed());
    return (Math.sin(flap) + 1.0) * flapSpeed;
  }
}

export { CHICKEN_LOCATION };
