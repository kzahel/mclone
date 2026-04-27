import type { ResourceLocation } from "../../core/resource-location";
import { RenderType } from "../render-type";
import type { PoseStack } from "../vertex/pose-stack";
import type { VertexConsumer } from "../vertex/vertex-consumer";
import { EntityModel } from "./entity-model";
import type { ModelPart } from "./geom/model-part";

export abstract class AgeableListModel<T> extends EntityModel<T> {
  protected constructor(
    renderType: (location: ResourceLocation) => RenderType = RenderType.entityCutoutNoCull,
    private readonly scaleHead = false,
    private readonly babyYHeadOffset = 5.0,
    private readonly babyZHeadOffset = 2.0,
    private readonly babyHeadScale = 2.0,
    private readonly babyBodyScale = 2.0,
    private readonly bodyYOffset = 24.0,
  ) {
    super(renderType);
  }

  public override renderToBuffer(
    poseStack: PoseStack,
    vertexConsumer: VertexConsumer,
    packedLight: number,
    packedOverlay: number,
    red: number,
    green: number,
    blue: number,
    alpha: number,
  ): void {
    if (this.young) {
      poseStack.pushPose();
      if (this.scaleHead) {
        const scale = 1.5 / this.babyHeadScale;
        poseStack.scale(scale, scale, scale);
      }

      poseStack.translate(0.0, this.babyYHeadOffset / 16.0, this.babyZHeadOffset / 16.0);
      for (const part of this.headParts()) {
        part.render(poseStack, vertexConsumer, packedLight, packedOverlay, red, green, blue, alpha);
      }
      poseStack.popPose();
      poseStack.pushPose();
      const bodyScale = 1.0 / this.babyBodyScale;
      poseStack.scale(bodyScale, bodyScale, bodyScale);
      poseStack.translate(0.0, this.bodyYOffset / 16.0, 0.0);
      for (const part of this.bodyParts()) {
        part.render(poseStack, vertexConsumer, packedLight, packedOverlay, red, green, blue, alpha);
      }
      poseStack.popPose();
      return;
    }

    for (const part of this.headParts()) {
      part.render(poseStack, vertexConsumer, packedLight, packedOverlay, red, green, blue, alpha);
    }
    for (const part of this.bodyParts()) {
      part.render(poseStack, vertexConsumer, packedLight, packedOverlay, red, green, blue, alpha);
    }
  }

  protected abstract headParts(): Iterable<ModelPart>;

  protected abstract bodyParts(): Iterable<ModelPart>;
}
