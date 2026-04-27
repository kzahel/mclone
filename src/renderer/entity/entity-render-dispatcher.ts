import { PLAYER_ENTITY_TYPE_ID } from "../../runtime/protocol/world-messages";
import { Vec3 } from "../../world/phys/vec3";
import { LightTexture } from "../light-texture";
import type { MultiBufferSource } from "../multi-buffer-source";
import { EntityModelSet } from "../model/geom/entity-model-set";
import { PoseStack } from "../vertex/pose-stack";
import { EntityRendererProvider } from "./entity-renderer-provider";
import type { EntityRenderer } from "./entity-renderer";
import { PlayerRenderer } from "./player-renderer";
import type { RenderableEntity, RenderablePlayer } from "./renderable-entity";

export class EntityRenderDispatcher {
  private readonly playerRenderers: ReadonlyMap<string, EntityRenderer<RenderablePlayer>>;

  public constructor(modelSet = EntityModelSet.createDefault()) {
    // EntityRender0: register only player renderers until other entity types are ported.
    const context = new EntityRendererProvider.Context(this, modelSet);
    this.playerRenderers = new Map([
      ["default", new PlayerRenderer(context, false)],
      ["slim", new PlayerRenderer(context, true)],
    ]);
  }

  public getRenderer<T extends RenderableEntity>(entity: T): EntityRenderer<T> {
    if (isRenderablePlayer(entity)) {
      const renderer = this.playerRenderers.get(entity.getModelName()) ?? this.playerRenderers.get("default");
      if (renderer === undefined) {
        throw new Error("Default player renderer is not registered");
      }

      return renderer as unknown as EntityRenderer<T>;
    }

    throw new Error(`No entity renderer for ${entity.typeId}`);
  }

  public render<T extends RenderableEntity>(
    entity: T,
    x: number,
    y: number,
    z: number,
    rotationYaw: number,
    partialTicks: number,
    matrixStack: PoseStack,
    buffer: MultiBufferSource,
    packedLight: number,
  ): void {
    // EntityRender0: fire, shadows, and hitbox debug rendering are deferred.
    const renderer = this.getRenderer(entity);
    const renderOffset = renderer.getRenderOffset(entity, partialTicks);
    matrixStack.pushPose();
    matrixStack.translate(x + renderOffset.x, y + renderOffset.y, z + renderOffset.z);
    renderer.render(entity, rotationYaw, partialTicks, matrixStack, buffer, packedLight);
    matrixStack.popPose();
  }

  public renderEntity<T extends RenderableEntity>(
    entity: T,
    cameraPosition: Vec3,
    matrixStack: PoseStack,
    buffer: MultiBufferSource,
    packedLight = LightTexture.pack(15, 15),
    partialTicks = 0,
  ): void {
    this.render(
      entity,
      entity.getX() - cameraPosition.x,
      entity.getY() - cameraPosition.y,
      entity.getZ() - cameraPosition.z,
      entity.getYRot(),
      partialTicks,
      matrixStack,
      buffer,
      packedLight,
    );
  }
}

function isRenderablePlayer(entity: RenderableEntity): entity is RenderablePlayer {
  return entity.typeId === PLAYER_ENTITY_TYPE_ID && "getModelName" in entity && typeof entity.getModelName === "function";
}
