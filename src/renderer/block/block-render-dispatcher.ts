import { BlockPos } from "../../core/block-pos";
import type { BlockAndTintGetter } from "../../world/level/block-and-tint-getter";
import { ResourceLocation } from "../../core/resource-location";
import { RenderShape } from "../../world/level/block/render-shape";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { FluidState } from "../../world/level/material/fluid-state";
import { JavaRandom } from "../../util/java-random";
import type { BakedModel } from "../model/baked-model";
import { BlockModelShaper } from "../model/block-model-shaper";
import type { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { PoseStack } from "../vertex/pose-stack";
import type { VertexConsumer } from "../vertex/vertex-consumer";
import { BlockColors } from "./block-colors";
import { LiquidBlockRenderer } from "./liquid-block-renderer";
import { ModelBlockRenderer } from "./model-block-renderer";

export class BlockRenderDispatcher {
  private readonly modelRenderer: ModelBlockRenderer;
  private readonly liquidBlockRenderer = new LiquidBlockRenderer();
  private readonly random = new JavaRandom();

  public constructor(
    private readonly blockModelShaper: BlockModelShaper,
    private readonly blockColors: BlockColors,
    spriteLookup: (location: ResourceLocation) => TextureAtlasSprite,
    waterState: BlockState,
    lavaState: BlockState,
  ) {
    this.modelRenderer = new ModelBlockRenderer(this.blockColors);
    this.liquidBlockRenderer.initializeSprites(this.blockModelShaper, waterState, lavaState, spriteLookup);
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

  public renderLiquid(pos: BlockPos, level: BlockAndTintGetter, consumer: VertexConsumer, fluidState: FluidState): boolean {
    return this.liquidBlockRenderer.tesselate(level, pos, consumer, fluidState);
  }
}
