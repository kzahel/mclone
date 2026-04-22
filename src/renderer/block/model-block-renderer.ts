import { BlockPos } from "../../core/block-pos";
import { Direction } from "../../core/direction";
import type { BlockAndTintGetter } from "../../world/level/block-and-tint-getter";
import { Block } from "../../world/level/block/block";
import type { BlockState } from "../../world/level/block/state/block-state";
import { BakedQuad } from "../model/baked-quad";
import type { BakedModel } from "../model/baked-model";
import { BlockColors } from "./block-colors";
import { LevelRenderer } from "../level-renderer";
import { PoseStack, type PoseStackPose } from "../vertex/pose-stack";
import type { VertexConsumer } from "../vertex/vertex-consumer";
import { JavaRandom } from "../../util/java-random";

const DIRECTIONS = Direction.values();
const CACHE_SIZE = 100;

type ShapeFlags = boolean[];

type SizeInfo = { readonly shape: number };

const SIZE_INFO = {
  DOWN: { shape: Direction.DOWN.get3DDataValue() },
  UP: { shape: Direction.UP.get3DDataValue() },
  NORTH: { shape: Direction.NORTH.get3DDataValue() },
  SOUTH: { shape: Direction.SOUTH.get3DDataValue() },
  WEST: { shape: Direction.WEST.get3DDataValue() },
  EAST: { shape: Direction.EAST.get3DDataValue() },
  FLIP_DOWN: { shape: Direction.DOWN.get3DDataValue() + DIRECTIONS.length },
  FLIP_UP: { shape: Direction.UP.get3DDataValue() + DIRECTIONS.length },
  FLIP_NORTH: { shape: Direction.NORTH.get3DDataValue() + DIRECTIONS.length },
  FLIP_SOUTH: { shape: Direction.SOUTH.get3DDataValue() + DIRECTIONS.length },
  FLIP_WEST: { shape: Direction.WEST.get3DDataValue() + DIRECTIONS.length },
  FLIP_EAST: { shape: Direction.EAST.get3DDataValue() + DIRECTIONS.length },
} as const;

type AdjacencyInfo = {
  readonly corners: readonly Direction[];
  readonly doNonCubicWeight: boolean;
  readonly vert0Weights: readonly SizeInfo[];
  readonly vert1Weights: readonly SizeInfo[];
  readonly vert2Weights: readonly SizeInfo[];
  readonly vert3Weights: readonly SizeInfo[];
};

type AmbientVertexRemap = {
  readonly vert0: number;
  readonly vert1: number;
  readonly vert2: number;
  readonly vert3: number;
};

const AMBIENT_VERTEX_REMAP: ReadonlyMap<Direction, AmbientVertexRemap> = new Map([
  [Direction.DOWN, { vert0: 0, vert1: 1, vert2: 2, vert3: 3 }],
  [Direction.UP, { vert0: 2, vert1: 3, vert2: 0, vert3: 1 }],
  [Direction.NORTH, { vert0: 3, vert1: 0, vert2: 1, vert3: 2 }],
  [Direction.SOUTH, { vert0: 0, vert1: 1, vert2: 2, vert3: 3 }],
  [Direction.WEST, { vert0: 3, vert1: 0, vert2: 1, vert3: 2 }],
  [Direction.EAST, { vert0: 1, vert1: 2, vert2: 3, vert3: 0 }],
]);

const ADJACENCY_INFO: ReadonlyMap<Direction, AdjacencyInfo> = new Map([
  [
    Direction.DOWN,
    {
      corners: [Direction.WEST, Direction.EAST, Direction.NORTH, Direction.SOUTH],
      doNonCubicWeight: true,
      vert0Weights: [SIZE_INFO.FLIP_WEST, SIZE_INFO.SOUTH, SIZE_INFO.FLIP_WEST, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.WEST, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.WEST, SIZE_INFO.SOUTH],
      vert1Weights: [SIZE_INFO.FLIP_WEST, SIZE_INFO.NORTH, SIZE_INFO.FLIP_WEST, SIZE_INFO.FLIP_NORTH, SIZE_INFO.WEST, SIZE_INFO.FLIP_NORTH, SIZE_INFO.WEST, SIZE_INFO.NORTH],
      vert2Weights: [SIZE_INFO.FLIP_EAST, SIZE_INFO.NORTH, SIZE_INFO.FLIP_EAST, SIZE_INFO.FLIP_NORTH, SIZE_INFO.EAST, SIZE_INFO.FLIP_NORTH, SIZE_INFO.EAST, SIZE_INFO.NORTH],
      vert3Weights: [SIZE_INFO.FLIP_EAST, SIZE_INFO.SOUTH, SIZE_INFO.FLIP_EAST, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.EAST, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.EAST, SIZE_INFO.SOUTH],
    },
  ],
  [
    Direction.UP,
    {
      corners: [Direction.EAST, Direction.WEST, Direction.NORTH, Direction.SOUTH],
      doNonCubicWeight: true,
      vert0Weights: [SIZE_INFO.EAST, SIZE_INFO.SOUTH, SIZE_INFO.EAST, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.FLIP_EAST, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.FLIP_EAST, SIZE_INFO.SOUTH],
      vert1Weights: [SIZE_INFO.EAST, SIZE_INFO.NORTH, SIZE_INFO.EAST, SIZE_INFO.FLIP_NORTH, SIZE_INFO.FLIP_EAST, SIZE_INFO.FLIP_NORTH, SIZE_INFO.FLIP_EAST, SIZE_INFO.NORTH],
      vert2Weights: [SIZE_INFO.WEST, SIZE_INFO.NORTH, SIZE_INFO.WEST, SIZE_INFO.FLIP_NORTH, SIZE_INFO.FLIP_WEST, SIZE_INFO.FLIP_NORTH, SIZE_INFO.FLIP_WEST, SIZE_INFO.NORTH],
      vert3Weights: [SIZE_INFO.WEST, SIZE_INFO.SOUTH, SIZE_INFO.WEST, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.FLIP_WEST, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.FLIP_WEST, SIZE_INFO.SOUTH],
    },
  ],
  [
    Direction.NORTH,
    {
      corners: [Direction.UP, Direction.DOWN, Direction.EAST, Direction.WEST],
      doNonCubicWeight: true,
      vert0Weights: [SIZE_INFO.UP, SIZE_INFO.FLIP_WEST, SIZE_INFO.UP, SIZE_INFO.WEST, SIZE_INFO.FLIP_UP, SIZE_INFO.WEST, SIZE_INFO.FLIP_UP, SIZE_INFO.FLIP_WEST],
      vert1Weights: [SIZE_INFO.UP, SIZE_INFO.FLIP_EAST, SIZE_INFO.UP, SIZE_INFO.EAST, SIZE_INFO.FLIP_UP, SIZE_INFO.EAST, SIZE_INFO.FLIP_UP, SIZE_INFO.FLIP_EAST],
      vert2Weights: [SIZE_INFO.DOWN, SIZE_INFO.FLIP_EAST, SIZE_INFO.DOWN, SIZE_INFO.EAST, SIZE_INFO.FLIP_DOWN, SIZE_INFO.EAST, SIZE_INFO.FLIP_DOWN, SIZE_INFO.FLIP_EAST],
      vert3Weights: [SIZE_INFO.DOWN, SIZE_INFO.FLIP_WEST, SIZE_INFO.DOWN, SIZE_INFO.WEST, SIZE_INFO.FLIP_DOWN, SIZE_INFO.WEST, SIZE_INFO.FLIP_DOWN, SIZE_INFO.FLIP_WEST],
    },
  ],
  [
    Direction.SOUTH,
    {
      corners: [Direction.WEST, Direction.EAST, Direction.DOWN, Direction.UP],
      doNonCubicWeight: true,
      vert0Weights: [SIZE_INFO.UP, SIZE_INFO.FLIP_WEST, SIZE_INFO.FLIP_UP, SIZE_INFO.FLIP_WEST, SIZE_INFO.FLIP_UP, SIZE_INFO.WEST, SIZE_INFO.UP, SIZE_INFO.WEST],
      vert1Weights: [SIZE_INFO.DOWN, SIZE_INFO.FLIP_WEST, SIZE_INFO.FLIP_DOWN, SIZE_INFO.FLIP_WEST, SIZE_INFO.FLIP_DOWN, SIZE_INFO.WEST, SIZE_INFO.DOWN, SIZE_INFO.WEST],
      vert2Weights: [SIZE_INFO.DOWN, SIZE_INFO.FLIP_EAST, SIZE_INFO.FLIP_DOWN, SIZE_INFO.FLIP_EAST, SIZE_INFO.FLIP_DOWN, SIZE_INFO.EAST, SIZE_INFO.DOWN, SIZE_INFO.EAST],
      vert3Weights: [SIZE_INFO.UP, SIZE_INFO.FLIP_EAST, SIZE_INFO.FLIP_UP, SIZE_INFO.FLIP_EAST, SIZE_INFO.FLIP_UP, SIZE_INFO.EAST, SIZE_INFO.UP, SIZE_INFO.EAST],
    },
  ],
  [
    Direction.WEST,
    {
      corners: [Direction.UP, Direction.DOWN, Direction.NORTH, Direction.SOUTH],
      doNonCubicWeight: true,
      vert0Weights: [SIZE_INFO.UP, SIZE_INFO.SOUTH, SIZE_INFO.UP, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.FLIP_UP, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.FLIP_UP, SIZE_INFO.SOUTH],
      vert1Weights: [SIZE_INFO.UP, SIZE_INFO.NORTH, SIZE_INFO.UP, SIZE_INFO.FLIP_NORTH, SIZE_INFO.FLIP_UP, SIZE_INFO.FLIP_NORTH, SIZE_INFO.FLIP_UP, SIZE_INFO.NORTH],
      vert2Weights: [SIZE_INFO.DOWN, SIZE_INFO.NORTH, SIZE_INFO.DOWN, SIZE_INFO.FLIP_NORTH, SIZE_INFO.FLIP_DOWN, SIZE_INFO.FLIP_NORTH, SIZE_INFO.FLIP_DOWN, SIZE_INFO.NORTH],
      vert3Weights: [SIZE_INFO.DOWN, SIZE_INFO.SOUTH, SIZE_INFO.DOWN, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.FLIP_DOWN, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.FLIP_DOWN, SIZE_INFO.SOUTH],
    },
  ],
  [
    Direction.EAST,
    {
      corners: [Direction.DOWN, Direction.UP, Direction.NORTH, Direction.SOUTH],
      doNonCubicWeight: true,
      vert0Weights: [SIZE_INFO.FLIP_DOWN, SIZE_INFO.SOUTH, SIZE_INFO.FLIP_DOWN, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.DOWN, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.DOWN, SIZE_INFO.SOUTH],
      vert1Weights: [SIZE_INFO.FLIP_DOWN, SIZE_INFO.NORTH, SIZE_INFO.FLIP_DOWN, SIZE_INFO.FLIP_NORTH, SIZE_INFO.DOWN, SIZE_INFO.FLIP_NORTH, SIZE_INFO.DOWN, SIZE_INFO.NORTH],
      vert2Weights: [SIZE_INFO.FLIP_UP, SIZE_INFO.NORTH, SIZE_INFO.FLIP_UP, SIZE_INFO.FLIP_NORTH, SIZE_INFO.UP, SIZE_INFO.FLIP_NORTH, SIZE_INFO.UP, SIZE_INFO.NORTH],
      vert3Weights: [SIZE_INFO.FLIP_UP, SIZE_INFO.SOUTH, SIZE_INFO.FLIP_UP, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.UP, SIZE_INFO.FLIP_SOUTH, SIZE_INFO.UP, SIZE_INFO.SOUTH],
    },
  ],
]);

class ModelBlockRendererCache {
  private enabled = false;
  private readonly colorCache = new Map<bigint, number>();
  private readonly brightnessCache = new Map<bigint, number>();

  public enable(): void {
    this.enabled = true;
  }

  public disable(): void {
    this.enabled = false;
    this.colorCache.clear();
    this.brightnessCache.clear();
  }

  public getLightColor(state: BlockState, level: BlockAndTintGetter, pos: BlockPos): number {
    const key = pos.asLong();
    if (this.enabled && this.colorCache.has(key)) {
      return this.colorCache.get(key)!;
    }

    const lightColor = LevelRenderer.getLightColorFromState(level, state, pos);
    if (this.enabled) {
      if (this.colorCache.size === CACHE_SIZE) {
        this.colorCache.delete(this.colorCache.keys().next().value!);
      }

      this.colorCache.set(key, lightColor);
    }

    return lightColor;
  }

  public getShadeBrightness(state: BlockState, level: BlockAndTintGetter, pos: BlockPos): number {
    const key = pos.asLong();
    if (this.enabled && this.brightnessCache.has(key)) {
      return this.brightnessCache.get(key)!;
    }

    const brightness = state.getShadeBrightness(level, pos);
    if (this.enabled) {
      if (this.brightnessCache.size === CACHE_SIZE) {
        this.brightnessCache.delete(this.brightnessCache.keys().next().value!);
      }

      this.brightnessCache.set(key, brightness);
    }

    return brightness;
  }
}

class AmbientOcclusionFace {
  public readonly brightness = [0, 0, 0, 0];
  public readonly lightmap = [0, 0, 0, 0];

  public calculate(
    level: BlockAndTintGetter,
    state: BlockState,
    pos: BlockPos,
    direction: Direction,
    shape: readonly number[],
    flags: ShapeFlags,
    shade: boolean,
    cache: ModelBlockRendererCache,
  ): void {
    const basePos = flags[0] ? pos.relative(direction) : pos;
    const adjacency = ADJACENCY_INFO.get(direction)!;
    const mutable = new BlockPos.MutableBlockPos();

    mutable.setWithOffset(basePos, adjacency.corners[0]!);
    const state0 = level.getBlockState(mutable);
    const light0 = cache.getLightColor(state0, level, mutable);
    const bright0 = cache.getShadeBrightness(state0, level, mutable);

    mutable.setWithOffset(basePos, adjacency.corners[1]!);
    const state1 = level.getBlockState(mutable);
    const light1 = cache.getLightColor(state1, level, mutable);
    const bright1 = cache.getShadeBrightness(state1, level, mutable);

    mutable.setWithOffset(basePos, adjacency.corners[2]!);
    const state2 = level.getBlockState(mutable);
    const light2 = cache.getLightColor(state2, level, mutable);
    const bright2 = cache.getShadeBrightness(state2, level, mutable);

    mutable.setWithOffset(basePos, adjacency.corners[3]!);
    const state3 = level.getBlockState(mutable);
    const light3 = cache.getLightColor(state3, level, mutable);
    const bright3 = cache.getShadeBrightness(state3, level, mutable);

    const state4 = level.getBlockState(mutable.setWithOffset(basePos, adjacency.corners[0]!).move(direction));
    const open4 = !state4.isViewBlocking(level, mutable) || state4.getLightBlock(level, mutable) === 0;
    const state5 = level.getBlockState(mutable.setWithOffset(basePos, adjacency.corners[1]!).move(direction));
    const open5 = !state5.isViewBlocking(level, mutable) || state5.getLightBlock(level, mutable) === 0;
    const state6 = level.getBlockState(mutable.setWithOffset(basePos, adjacency.corners[2]!).move(direction));
    const open6 = !state6.isViewBlocking(level, mutable) || state6.getLightBlock(level, mutable) === 0;
    const state7 = level.getBlockState(mutable.setWithOffset(basePos, adjacency.corners[3]!).move(direction));
    const open7 = !state7.isViewBlocking(level, mutable) || state7.getLightBlock(level, mutable) === 0;

    let bright8: number;
    let light8: number;
    if (!open6 && !open4) {
      bright8 = bright0;
      light8 = light0;
    } else {
      mutable.setWithOffset(basePos, adjacency.corners[0]!).move(adjacency.corners[2]!);
      const state8 = level.getBlockState(mutable);
      bright8 = cache.getShadeBrightness(state8, level, mutable);
      light8 = cache.getLightColor(state8, level, mutable);
    }

    let bright9: number;
    let light9: number;
    if (!open7 && !open4) {
      bright9 = bright0;
      light9 = light0;
    } else {
      mutable.setWithOffset(basePos, adjacency.corners[0]!).move(adjacency.corners[3]!);
      const state9 = level.getBlockState(mutable);
      bright9 = cache.getShadeBrightness(state9, level, mutable);
      light9 = cache.getLightColor(state9, level, mutable);
    }

    let bright10: number;
    let light10: number;
    if (!open6 && !open5) {
      bright10 = bright0;
      light10 = light0;
    } else {
      mutable.setWithOffset(basePos, adjacency.corners[1]!).move(adjacency.corners[2]!);
      const state10 = level.getBlockState(mutable);
      bright10 = cache.getShadeBrightness(state10, level, mutable);
      light10 = cache.getLightColor(state10, level, mutable);
    }

    let bright11: number;
    let light11: number;
    if (!open7 && !open5) {
      bright11 = bright0;
      light11 = light0;
    } else {
      mutable.setWithOffset(basePos, adjacency.corners[1]!).move(adjacency.corners[3]!);
      const state11 = level.getBlockState(mutable);
      bright11 = cache.getShadeBrightness(state11, level, mutable);
      light11 = cache.getLightColor(state11, level, mutable);
    }

    let centerLight = cache.getLightColor(state, level, pos);
    mutable.setWithOffset(pos, direction);
    const forwardState = level.getBlockState(mutable);
    if (flags[0] || !forwardState.isSolidRender(level, mutable)) {
      centerLight = cache.getLightColor(forwardState, level, mutable);
    }

    const centerBrightness = flags[0]
      ? cache.getShadeBrightness(level.getBlockState(basePos), level, basePos)
      : cache.getShadeBrightness(level.getBlockState(pos), level, pos);
    const remap = AMBIENT_VERTEX_REMAP.get(direction)!;

    if (flags[1] && adjacency.doNonCubicWeight) {
      const br0 = (bright3 + bright0 + bright9 + centerBrightness) * 0.25;
      const br1 = (bright2 + bright0 + bright8 + centerBrightness) * 0.25;
      const br2 = (bright2 + bright1 + bright10 + centerBrightness) * 0.25;
      const br3 = (bright3 + bright1 + bright11 + centerBrightness) * 0.25;

      const w0 = shape[adjacency.vert0Weights[0]!.shape]! * shape[adjacency.vert0Weights[1]!.shape]!;
      const w1 = shape[adjacency.vert0Weights[2]!.shape]! * shape[adjacency.vert0Weights[3]!.shape]!;
      const w2 = shape[adjacency.vert0Weights[4]!.shape]! * shape[adjacency.vert0Weights[5]!.shape]!;
      const w3 = shape[adjacency.vert0Weights[6]!.shape]! * shape[adjacency.vert0Weights[7]!.shape]!;
      const w4 = shape[adjacency.vert1Weights[0]!.shape]! * shape[adjacency.vert1Weights[1]!.shape]!;
      const w5 = shape[adjacency.vert1Weights[2]!.shape]! * shape[adjacency.vert1Weights[3]!.shape]!;
      const w6 = shape[adjacency.vert1Weights[4]!.shape]! * shape[adjacency.vert1Weights[5]!.shape]!;
      const w7 = shape[adjacency.vert1Weights[6]!.shape]! * shape[adjacency.vert1Weights[7]!.shape]!;
      const w8 = shape[adjacency.vert2Weights[0]!.shape]! * shape[adjacency.vert2Weights[1]!.shape]!;
      const w9 = shape[adjacency.vert2Weights[2]!.shape]! * shape[adjacency.vert2Weights[3]!.shape]!;
      const w10 = shape[adjacency.vert2Weights[4]!.shape]! * shape[adjacency.vert2Weights[5]!.shape]!;
      const w11 = shape[adjacency.vert2Weights[6]!.shape]! * shape[adjacency.vert2Weights[7]!.shape]!;
      const w12 = shape[adjacency.vert3Weights[0]!.shape]! * shape[adjacency.vert3Weights[1]!.shape]!;
      const w13 = shape[adjacency.vert3Weights[2]!.shape]! * shape[adjacency.vert3Weights[3]!.shape]!;
      const w14 = shape[adjacency.vert3Weights[4]!.shape]! * shape[adjacency.vert3Weights[5]!.shape]!;
      const w15 = shape[adjacency.vert3Weights[6]!.shape]! * shape[adjacency.vert3Weights[7]!.shape]!;

      this.brightness[remap.vert0] = (((br0 * w0) + (br1 * w1)) + (br2 * w2)) + (br3 * w3);
      this.brightness[remap.vert1] = (((br0 * w4) + (br1 * w5)) + (br2 * w6)) + (br3 * w7);
      this.brightness[remap.vert2] = (((br0 * w8) + (br1 * w9)) + (br2 * w10)) + (br3 * w11);
      this.brightness[remap.vert3] = (((br0 * w12) + (br1 * w13)) + (br2 * w14)) + (br3 * w15);

      const lm0 = this.blend(light3, light0, light9, centerLight);
      const lm1 = this.blend(light2, light0, light8, centerLight);
      const lm2 = this.blend(light2, light1, light10, centerLight);
      const lm3 = this.blend(light3, light1, light11, centerLight);
      this.lightmap[remap.vert0] = this.blendWeighted(lm0, lm1, lm2, lm3, w0, w1, w2, w3);
      this.lightmap[remap.vert1] = this.blendWeighted(lm0, lm1, lm2, lm3, w4, w5, w6, w7);
      this.lightmap[remap.vert2] = this.blendWeighted(lm0, lm1, lm2, lm3, w8, w9, w10, w11);
      this.lightmap[remap.vert3] = this.blendWeighted(lm0, lm1, lm2, lm3, w12, w13, w14, w15);
    } else {
      this.brightness[remap.vert0] = (bright3 + bright0 + bright9 + centerBrightness) * 0.25;
      this.brightness[remap.vert1] = (bright2 + bright0 + bright8 + centerBrightness) * 0.25;
      this.brightness[remap.vert2] = (bright2 + bright1 + bright10 + centerBrightness) * 0.25;
      this.brightness[remap.vert3] = (bright3 + bright1 + bright11 + centerBrightness) * 0.25;
      this.lightmap[remap.vert0] = this.blend(light3, light0, light9, centerLight);
      this.lightmap[remap.vert1] = this.blend(light2, light0, light8, centerLight);
      this.lightmap[remap.vert2] = this.blend(light2, light1, light10, centerLight);
      this.lightmap[remap.vert3] = this.blend(light3, light1, light11, centerLight);
    }

    const faceShade = level.getShade(direction, shade);
    for (let index = 0; index < this.brightness.length; index++) {
      this.brightness[index] = this.brightness[index]! * faceShade;
    }
  }

  private blend(a: number, b: number, c: number, d: number): number {
    if (a === 0) {
      a = d;
    }

    if (b === 0) {
      b = d;
    }

    if (c === 0) {
      c = d;
    }

    return (((a + b + c + d) >> 2) & 0xff00ff);
  }

  private blendWeighted(a: number, b: number, c: number, d: number, wa: number, wb: number, wc: number, wd: number): number {
    const high = Math.trunc((((a >> 16) & 0xff) * wa) + (((b >> 16) & 0xff) * wb) + (((c >> 16) & 0xff) * wc) + (((d >> 16) & 0xff) * wd)) & 0xff;
    const low = Math.trunc(((a & 0xff) * wa) + ((b & 0xff) * wb) + ((c & 0xff) * wc) + ((d & 0xff) * wd)) & 0xff;
    return (high << 16) | low;
  }
}

export class ModelBlockRenderer {
  private static readonly CACHE = new ModelBlockRendererCache();

  public constructor(private readonly blockColors: BlockColors) {}

  public static enableCaching(): void {
    ModelBlockRenderer.CACHE.enable();
  }

  public static clearCache(): void {
    ModelBlockRenderer.CACHE.disable();
  }

  public tesselateBlock(
    level: BlockAndTintGetter,
    model: BakedModel,
    state: BlockState,
    pos: BlockPos,
    poseStack: PoseStack,
    consumer: VertexConsumer,
    checkSides: boolean,
    random: JavaRandom,
    seed: bigint,
    overlay: number,
  ): boolean {
    const useAmbientOcclusion = state.getLightEmission() === 0 && model.useAmbientOcclusion();
    const offset = state.getOffset(level, pos);
    poseStack.translate(offset.x, offset.y, offset.z);
    return useAmbientOcclusion
      ? this.tesselateWithAO(level, model, state, pos, poseStack, consumer, checkSides, random, seed, overlay)
      : this.tesselateWithoutAO(level, model, state, pos, poseStack, consumer, checkSides, random, seed, overlay);
  }

  public tesselateWithAO(
    level: BlockAndTintGetter,
    model: BakedModel,
    state: BlockState,
    pos: BlockPos,
    poseStack: PoseStack,
    consumer: VertexConsumer,
    checkSides: boolean,
    random: JavaRandom,
    seed: bigint,
    overlay: number,
  ): boolean {
    let rendered = false;
    const shape = new Array<number>(DIRECTIONS.length * 2).fill(0);
    const flags = [false, false, false];
    const aoFace = new AmbientOcclusionFace();
    const mutable = pos.mutable();

    for (const direction of DIRECTIONS) {
      random.setSeed(seed);
      const quads = model.getQuads(state, direction, random);
      if (quads.length === 0) {
        continue;
      }

      mutable.setWithOffset(pos, direction);
      if (!checkSides || Block.shouldRenderFace(state, level, pos, direction, mutable)) {
        this.renderModelFaceAO(level, state, pos, poseStack.last(), consumer, quads, shape, flags, aoFace, overlay);
        rendered = true;
      }
    }

    random.setSeed(seed);
    const quads = model.getQuads(state, undefined, random);
    if (quads.length !== 0) {
      this.renderModelFaceAO(level, state, pos, poseStack.last(), consumer, quads, shape, flags, aoFace, overlay);
      rendered = true;
    }

    return rendered;
  }

  public tesselateWithoutAO(
    level: BlockAndTintGetter,
    model: BakedModel,
    state: BlockState,
    pos: BlockPos,
    poseStack: PoseStack,
    consumer: VertexConsumer,
    checkSides: boolean,
    random: JavaRandom,
    seed: bigint,
    overlay: number,
  ): boolean {
    let rendered = false;
    const flags = [false, false, false];
    const mutable = pos.mutable();

    for (const direction of DIRECTIONS) {
      random.setSeed(seed);
      const quads = model.getQuads(state, direction, random);
      if (quads.length === 0) {
        continue;
      }

      mutable.setWithOffset(pos, direction);
      if (!checkSides || Block.shouldRenderFace(state, level, pos, direction, mutable)) {
        const light = LevelRenderer.getLightColorFromState(level, state, mutable);
        this.renderModelFaceFlat(level, state, pos, light, overlay, false, poseStack.last(), consumer, quads, flags);
        rendered = true;
      }
    }

    random.setSeed(seed);
    const quads = model.getQuads(state, undefined, random);
    if (quads.length !== 0) {
      this.renderModelFaceFlat(level, state, pos, -1, overlay, true, poseStack.last(), consumer, quads, flags);
      rendered = true;
    }

    return rendered;
  }

  private renderModelFaceAO(
    level: BlockAndTintGetter,
    state: BlockState,
    pos: BlockPos,
    pose: PoseStackPose,
    consumer: VertexConsumer,
    quads: readonly BakedQuad[],
    shape: number[],
    flags: ShapeFlags,
    aoFace: AmbientOcclusionFace,
    overlay: number,
  ): void {
    for (const quad of quads) {
      this.calculateShape(level, state, pos, quad.getVertices(), quad.getDirection(), shape, flags);
      aoFace.calculate(level, state, pos, quad.getDirection(), shape, flags, quad.isShade(), ModelBlockRenderer.CACHE);
      this.putQuadData(
        level,
        state,
        pos,
        consumer,
        pose,
        quad,
        aoFace.brightness[0]!,
        aoFace.brightness[1]!,
        aoFace.brightness[2]!,
        aoFace.brightness[3]!,
        aoFace.lightmap[0]!,
        aoFace.lightmap[1]!,
        aoFace.lightmap[2]!,
        aoFace.lightmap[3]!,
        overlay,
      );
    }
  }

  private renderModelFaceFlat(
    level: BlockAndTintGetter,
    state: BlockState,
    pos: BlockPos,
    light: number,
    overlay: number,
    recalcLight: boolean,
    pose: PoseStackPose,
    consumer: VertexConsumer,
    quads: readonly BakedQuad[],
    flags: ShapeFlags,
  ): void {
    for (const quad of quads) {
      let quadLight = light;
      if (recalcLight) {
        this.calculateShape(level, state, pos, quad.getVertices(), quad.getDirection(), undefined, flags);
        const lightPos = flags[0] ? pos.relative(quad.getDirection()) : pos;
        quadLight = LevelRenderer.getLightColorFromState(level, state, lightPos);
      }

      const brightness = level.getShade(quad.getDirection(), quad.isShade());
      this.putQuadData(level, state, pos, consumer, pose, quad, brightness, brightness, brightness, brightness, quadLight, quadLight, quadLight, quadLight, overlay);
    }
  }

  private putQuadData(
    level: BlockAndTintGetter,
    state: BlockState,
    pos: BlockPos,
    consumer: VertexConsumer,
    pose: PoseStackPose,
    quad: BakedQuad,
    brightness0: number,
    brightness1: number,
    brightness2: number,
    brightness3: number,
    light0: number,
    light1: number,
    light2: number,
    light3: number,
    overlay: number,
  ): void {
    let red: number;
    let green: number;
    let blue: number;
    if (quad.isTinted()) {
      const color = this.blockColors.getColor(state, level, pos, quad.getTintIndex());
      red = ((color >> 16) & 0xff) / 255.0;
      green = ((color >> 8) & 0xff) / 255.0;
      blue = (color & 0xff) / 255.0;
    } else {
      red = 1.0;
      green = 1.0;
      blue = 1.0;
    }

    consumer.putBulkData(
      pose,
      quad,
      [brightness0, brightness1, brightness2, brightness3],
      red,
      green,
      blue,
      [light0, light1, light2, light3],
      overlay,
      true,
    );
  }

  public renderModel(
    pose: PoseStackPose,
    consumer: VertexConsumer,
    state: BlockState | undefined,
    model: BakedModel,
    red: number,
    green: number,
    blue: number,
    light: number,
    overlay: number,
  ): void {
    const random = new JavaRandom();
    for (const direction of DIRECTIONS) {
      random.setSeed(42n);
      this.renderQuadList(pose, consumer, red, green, blue, model.getQuads(state, direction, random), light, overlay);
    }

    random.setSeed(42n);
    this.renderQuadList(pose, consumer, red, green, blue, model.getQuads(state, undefined, random), light, overlay);
  }

  private renderQuadList(
    pose: PoseStackPose,
    consumer: VertexConsumer,
    red: number,
    green: number,
    blue: number,
    quads: readonly BakedQuad[],
    light: number,
    overlay: number,
  ): void {
    for (const quad of quads) {
      const quadRed = quad.isTinted() ? clamp(red, 0.0, 1.0) : 1.0;
      const quadGreen = quad.isTinted() ? clamp(green, 0.0, 1.0) : 1.0;
      const quadBlue = quad.isTinted() ? clamp(blue, 0.0, 1.0) : 1.0;
      consumer.putBulkData(pose, quad, quadRed, quadGreen, quadBlue, light, overlay);
    }
  }

  public calculateShape(
    level: BlockAndTintGetter,
    state: BlockState,
    pos: BlockPos,
    vertices: readonly number[],
    direction: Direction,
    shape: number[] | undefined,
    flags: ShapeFlags,
  ): void {
    let minX = 32.0;
    let minY = 32.0;
    let minZ = 32.0;
    let maxX = -32.0;
    let maxY = -32.0;
    let maxZ = -32.0;

    for (let index = 0; index < 4; index++) {
      const x = intBitsToFloat(vertices[index * 8]!);
      const y = intBitsToFloat(vertices[(index * 8) + 1]!);
      const z = intBitsToFloat(vertices[(index * 8) + 2]!);
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      minZ = Math.min(minZ, z);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
      maxZ = Math.max(maxZ, z);
    }

    if (shape !== undefined) {
      shape[Direction.WEST.get3DDataValue()] = minX;
      shape[Direction.EAST.get3DDataValue()] = maxX;
      shape[Direction.DOWN.get3DDataValue()] = minY;
      shape[Direction.UP.get3DDataValue()] = maxY;
      shape[Direction.NORTH.get3DDataValue()] = minZ;
      shape[Direction.SOUTH.get3DDataValue()] = maxZ;
      const offset = DIRECTIONS.length;
      shape[Direction.WEST.get3DDataValue() + offset] = 1.0 - minX;
      shape[Direction.EAST.get3DDataValue() + offset] = 1.0 - maxX;
      shape[Direction.DOWN.get3DDataValue() + offset] = 1.0 - minY;
      shape[Direction.UP.get3DDataValue() + offset] = 1.0 - maxY;
      shape[Direction.NORTH.get3DDataValue() + offset] = 1.0 - minZ;
      shape[Direction.SOUTH.get3DDataValue() + offset] = 1.0 - maxZ;
    }

    switch (direction) {
      case Direction.DOWN:
        flags[1] = minX >= 1.0e-4 || minZ >= 1.0e-4 || maxX <= 0.9999 || maxZ <= 0.9999;
        flags[0] = minY === maxY && (minY < 1.0e-4 || state.isCollisionShapeFullBlock(level, pos));
        break;
      case Direction.UP:
        flags[1] = minX >= 1.0e-4 || minZ >= 1.0e-4 || maxX <= 0.9999 || maxZ <= 0.9999;
        flags[0] = minY === maxY && (maxY > 0.9999 || state.isCollisionShapeFullBlock(level, pos));
        break;
      case Direction.NORTH:
        flags[1] = minX >= 1.0e-4 || minY >= 1.0e-4 || maxX <= 0.9999 || maxY <= 0.9999;
        flags[0] = minZ === maxZ && (minZ < 1.0e-4 || state.isCollisionShapeFullBlock(level, pos));
        break;
      case Direction.SOUTH:
        flags[1] = minX >= 1.0e-4 || minY >= 1.0e-4 || maxX <= 0.9999 || maxY <= 0.9999;
        flags[0] = minZ === maxZ && (maxZ > 0.9999 || state.isCollisionShapeFullBlock(level, pos));
        break;
      case Direction.WEST:
        flags[1] = minY >= 1.0e-4 || minZ >= 1.0e-4 || maxY <= 0.9999 || maxZ <= 0.9999;
        flags[0] = minX === maxX && (minX < 1.0e-4 || state.isCollisionShapeFullBlock(level, pos));
        break;
      case Direction.EAST:
        flags[1] = minY >= 1.0e-4 || minZ >= 1.0e-4 || maxY <= 0.9999 || maxZ <= 0.9999;
        flags[0] = minX === maxX && (maxX > 0.9999 || state.isCollisionShapeFullBlock(level, pos));
        break;
    }
  }
}

function clamp(value: number, minValue: number, maxValue: number): number {
  if (value < minValue) {
    return minValue;
  }

  return value > maxValue ? maxValue : value;
}

function intBitsToFloat(value: number): number {
  const array = new ArrayBuffer(4);
  const view = new DataView(array);
  view.setInt32(0, value, true);
  return view.getFloat32(0, true);
}
