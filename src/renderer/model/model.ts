import type { ResourceLocation } from "../../core/resource-location";
import type { RenderType } from "../render-type";
import type { PoseStack } from "../vertex/pose-stack";
import type { VertexConsumer } from "../vertex/vertex-consumer";

export abstract class Model {
  protected constructor(protected readonly renderTypeFunction: (location: ResourceLocation) => RenderType) {}

  public renderType(location: ResourceLocation): RenderType {
    return this.renderTypeFunction(location);
  }

  public abstract renderToBuffer(
    poseStack: PoseStack,
    vertexConsumer: VertexConsumer,
    packedLight: number,
    packedOverlay: number,
    red: number,
    green: number,
    blue: number,
    alpha: number,
  ): void;
}
