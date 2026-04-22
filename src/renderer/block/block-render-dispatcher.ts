import { BlockPos } from "../../core/block-pos";
import type { BlockAndTintGetter } from "../../world/level/block-and-tint-getter";
import { RenderShape } from "../../world/level/block/render-shape";
import type { BlockState } from "../../world/level/block/state/block-state";
import { JavaRandom } from "../../util/java-random";
import type { BakedModel } from "../model/baked-model";
import { BlockModelShaper } from "../model/block-model-shaper";
import { PoseStack } from "../vertex/pose-stack";
import type { VertexConsumer } from "../vertex/vertex-consumer";
import { BlockColors } from "./block-colors";
import { ModelBlockRenderer } from "./model-block-renderer";

export class BlockRenderDispatcher {
  private readonly modelRenderer: ModelBlockRenderer;
  private readonly random = new JavaRandom();

  public constructor(
    private readonly blockModelShaper: BlockModelShaper,
    private readonly blockColors: BlockColors,
  ) {
    this.modelRenderer = new ModelBlockRenderer(this.blockColors);
  }

  public getBlockModelShaper(): BlockModelShaper {
    return this.blockModelShaper;
  }

  public renderBatched(
    state: BlockState,
    pos: BlockPos,
    level: BlockAndTintGetter,
    poseStack: PoseStack,
    consumer: VertexConsumer,
    checkSides: boolean,
    random: JavaRandom = this.random,
  ): boolean {
    return state.getRenderShape() !== RenderShape.MODEL
      ? false
      : this.modelRenderer.tesselateBlock(level, this.getBlockModel(state), state, pos, poseStack, consumer, checkSides, random, state.getSeed(pos), 0);
  }

  public getModelRenderer(): ModelBlockRenderer {
    return this.modelRenderer;
  }

  public getBlockModel(state: BlockState): BakedModel {
    return this.blockModelShaper.getBlockModel(state);
  }
}
