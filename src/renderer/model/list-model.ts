import type { RenderType } from "../render-type";
import type { PoseStack } from "../vertex/pose-stack";
import type { VertexConsumer } from "../vertex/vertex-consumer";
import type { ModelPart } from "./geom/model-part";
import { EntityModel } from "./entity-model";

export abstract class ListModel<T> extends EntityModel<T> {
  public constructor(renderType?: (location: import("../../core/resource-location").ResourceLocation) => RenderType) {
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
    for (const part of this.parts()) {
      part.render(poseStack, vertexConsumer, packedLight, packedOverlay, red, green, blue, alpha);
    }
  }

  public abstract parts(): Iterable<ModelPart>;
}
