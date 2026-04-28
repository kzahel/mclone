import { lerp } from "../../util/mth";
import { RenderType } from "../render-type";
import type { MultiBufferSource } from "../multi-buffer-source";
import { EntityModel } from "../model/entity-model";
import { OverlayTexture } from "../texture/overlay-texture";
import { PoseStack } from "../vertex/pose-stack";
import { Vector3f } from "../math/vector3f";
import { EntityRenderer } from "./entity-renderer";
import type { EntityRendererProvider } from "./entity-renderer-provider";
import type { RenderableEntity } from "./renderable-entity";

export abstract class LivingEntityRenderer<T extends RenderableEntity, M extends EntityModel<T>> extends EntityRenderer<T> {
  protected readonly layers: unknown[] = [];

  public constructor(context: EntityRendererProvider.Context, protected readonly model: M, shadowRadius: number) {
    super(context);
    this.shadowRadius = shadowRadius;
  }

  public getModel(): M {
    return this.model;
  }

  public override render(entity: T, entityYaw: number, partialTicks: number, matrixStack: PoseStack, buffer: MultiBufferSource, packedLight: number): void {
    // EntityRender0: neutral-pose MVP omits render layers, invisibility variants, sleeping, death, and swim/fly animation state.
    matrixStack.pushPose();
    this.model.attackTime = this.getAttackAnim(entity, partialTicks);
    this.model.riding = entity.isPassenger();
    this.model.young = entity.isBaby();
    const bodyRot = rotLerp(partialTicks, entity.yBodyRotO, entity.yBodyRot);
    const headRot = rotLerp(partialTicks, entity.yHeadRotO, entity.yHeadRot);
    const netHeadYaw = headRot - bodyRot;
    const headPitch = lerp(partialTicks, entity.xRotO, entity.getXRot());
    const ageInTicks = this.getBob(entity, partialTicks);
    this.setupRotations(entity, matrixStack, ageInTicks, bodyRot, partialTicks);
    matrixStack.scale(-1.0, -1.0, 1.0);
    this.scale(entity, matrixStack, partialTicks);
    matrixStack.translate(0.0, -1.501, 0.0);
    this.model.prepareMobModel(entity, 0.0, 0.0, partialTicks);
    this.model.setupAnim(entity, 0.0, 0.0, ageInTicks, netHeadYaw, headPitch);
    const renderType = this.getRenderType(entity, this.isBodyVisible(entity), false, false);
    if (renderType !== undefined) {
      const vertexConsumer = buffer.getBuffer(renderType);
      const overlay = LivingEntityRenderer.getOverlayCoords(entity, this.getWhiteOverlayProgress(entity, partialTicks));
      this.model.renderToBuffer(matrixStack, vertexConsumer, packedLight, overlay, 1.0, 1.0, 1.0, 1.0);
    }

    this.renderLayers(entity, entityYaw, partialTicks, matrixStack, buffer, packedLight, ageInTicks, netHeadYaw, headPitch);
    matrixStack.popPose();
    super.render(entity, entityYaw, partialTicks, matrixStack, buffer, packedLight);
  }

  protected renderLayers(
    _entity: T,
    _entityYaw: number,
    _partialTicks: number,
    _matrixStack: PoseStack,
    _buffer: MultiBufferSource,
    _packedLight: number,
    _ageInTicks: number,
    _netHeadYaw: number,
    _headPitch: number,
  ): void {}

  protected getRenderType(entity: T, bodyVisible: boolean, translucent: boolean, glowing: boolean): RenderType | undefined {
    const textureLocation = this.getTextureLocation(entity);
    if (translucent) {
      return RenderType.entityTranslucent(textureLocation);
    }
    if (bodyVisible) {
      return this.model.renderType(textureLocation);
    }
    return glowing ? RenderType.entityTranslucent(textureLocation) : undefined;
  }

  public static getOverlayCoords(_livingEntity: RenderableEntity, u: number): number {
    return OverlayTexture.pack(OverlayTexture.u(u), false);
  }

  protected isBodyVisible(entity: T): boolean {
    return !entity.isInvisible();
  }

  protected setupRotations(entityLiving: T, matrixStack: PoseStack, _ageInTicks: number, rotationYaw: number, _partialTicks: number): void {
    // EntityRender0: only standing yaw rotation is ported for neutral remote players.
    if (!entityLiving.isSpectator()) {
      matrixStack.mulPose(Vector3f.YP.rotationDegrees(180.0 - rotationYaw));
    }
  }

  protected getAttackAnim(_livingBase: T, _partialTickTime: number): number {
    return 0.0;
  }

  protected getBob(livingBase: T, partialTicks: number): number {
    return livingBase.tickCount + partialTicks;
  }

  protected getWhiteOverlayProgress(_livingEntity: T, _partialTicks: number): number {
    return 0.0;
  }

  protected scale(_livingEntity: T, _matrixStack: PoseStack, _partialTickTime: number): void {}
}

function rotLerp(partialTick: number, oldRot: number, rot: number): number {
  return oldRot + (partialTick * wrapDegrees(rot - oldRot));
}

function wrapDegrees(value: number): number {
  let wrapped = value % 360.0;
  if (wrapped >= 180.0) {
    wrapped -= 360.0;
  }
  if (wrapped < -180.0) {
    wrapped += 360.0;
  }
  return wrapped;
}
