import { ResourceLocation } from "../../core/resource-location";
import type { MultiBufferSource } from "../multi-buffer-source";
import { ModelLayers } from "../model/geom/model-layers";
import { PlayerModel } from "../model/player-model";
import type { PoseStack } from "../vertex/pose-stack";
import type { EntityRendererProvider } from "./entity-renderer-provider";
import { LivingEntityRenderer } from "./living-entity-renderer";
import type { RenderablePlayer } from "./renderable-entity";

export class PlayerRenderer extends LivingEntityRenderer<RenderablePlayer, PlayerModel<RenderablePlayer>> {
  public constructor(context: EntityRendererProvider.Context, slim: boolean) {
    super(context, new PlayerModel(context.bakeLayer(slim ? ModelLayers.PLAYER_SLIM : ModelLayers.PLAYER), slim), 0.5);
  }

  public override render(entity: RenderablePlayer, entityYaw: number, partialTicks: number, matrixStack: PoseStack, buffer: MultiBufferSource, packedLight: number): void {
    this.setModelProperties(entity);
    super.render(entity, entityYaw, partialTicks, matrixStack, buffer, packedLight);
  }

  private setModelProperties(clientPlayer: RenderablePlayer): void {
    // EntityRender0: skin-part toggles and arm poses are deferred until player metadata carries them.
    const model = this.getModel();
    if (clientPlayer.isSpectator()) {
      model.setAllVisible(false);
      model.head.visible = true;
      model.hat.visible = true;
      return;
    }

    model.setAllVisible(true);
    model.crouching = clientPlayer.isCrouching();
  }

  public getTextureLocation(entity: RenderablePlayer): ResourceLocation {
    return entity.getSkinTextureLocation();
  }

  protected override scale(_livingEntity: RenderablePlayer, matrixStack: PoseStack, _partialTickTime: number): void {
    matrixStack.scale(0.9375, 0.9375, 0.9375);
  }
}
