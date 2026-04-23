import { ResourceLocation } from "../../core/resource-location";
import { BlockPos } from "../../core/block-pos";
import { Direction } from "../../core/direction";
import { BiomeColors } from "../biome-colors";
import { LevelRenderer } from "../level-renderer";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import type { VertexConsumer } from "../vertex/vertex-consumer";
import type { BlockAndTintGetter } from "../../world/level/block-and-tint-getter";
import type { BlockGetter } from "../../world/level/block-getter";
import type { BlockState } from "../../world/level/block/state/block-state";
import { Fluids } from "../../world/level/material/fluids";
import type { Fluid } from "../../world/level/material/fluid";
import type { FluidState } from "../../world/level/material/fluid-state";
import type { BlockModelShaper } from "../model/block-model-shaper";
import { lerp } from "../../util/mth";

const WATER_FLOW = new ResourceLocation("minecraft:block/water_flow");
const LAVA_FLOW = new ResourceLocation("minecraft:block/lava_flow");
const WATER_OVERLAY = new ResourceLocation("minecraft:block/water_overlay");

function isNeighborSameFluid(level: BlockGetter, pos: BlockPos, direction: Direction, fluidState: FluidState): boolean {
  return level.getFluidState(pos.relative(direction)).getType().isSame(fluidState.getType());
}

function isFaceOccludedByState(state: BlockState): boolean {
  return state.canOcclude();
}

function isFaceOccludedByNeighbor(level: BlockGetter, pos: BlockPos, direction: Direction): boolean {
  return isFaceOccludedByState(level.getBlockState(pos.relative(direction)));
}

function isFaceOccludedBySelf(state: BlockState): boolean {
  return isFaceOccludedByState(state);
}

export class LiquidBlockRenderer {
  private readonly lavaIcons: TextureAtlasSprite[] = new Array<TextureAtlasSprite>(2);
  private readonly waterIcons: TextureAtlasSprite[] = new Array<TextureAtlasSprite>(2);
  private waterOverlay: TextureAtlasSprite | undefined;

  // WebGPU: atlas sprite lookup is injected instead of being pulled from Minecraft singleton globals.
  public initializeSprites(
    blockModelShaper: BlockModelShaper,
    stillWaterState: BlockState,
    stillLavaState: BlockState,
    spriteLookup: (location: ResourceLocation) => TextureAtlasSprite,
  ): void {
    this.lavaIcons[0] = blockModelShaper.getBlockModel(stillLavaState).getParticleIcon();
    this.lavaIcons[1] = spriteLookup(LAVA_FLOW);
    this.waterIcons[0] = blockModelShaper.getBlockModel(stillWaterState).getParticleIcon();
    this.waterIcons[1] = spriteLookup(WATER_FLOW);
    this.waterOverlay = spriteLookup(WATER_OVERLAY);
  }

  public static shouldRenderFace(level: BlockAndTintGetter, pos: BlockPos, fluidState: FluidState, blockState: BlockState, direction: Direction): boolean {
    return !isFaceOccludedBySelf(blockState) && !isNeighborSameFluid(level, pos, direction, fluidState);
  }

  public tesselate(level: BlockAndTintGetter, pos: BlockPos, consumer: VertexConsumer, fluidState: FluidState): boolean {
    const lava = fluidState.getType().isSame(Fluids.LAVA);
    const sprites = lava ? this.lavaIcons : this.waterIcons;
    const blockState = level.getBlockState(pos);
    const color = lava ? 0xffffff : BiomeColors.getAverageWaterColor(level, pos);
    const red = ((color >> 16) & 0xff) / 255.0;
    const green = ((color >> 8) & 0xff) / 255.0;
    const blue = (color & 0xff) / 255.0;
    const renderTop = !isNeighborSameFluid(level, pos, Direction.UP, fluidState);
    const renderBottom =
      LiquidBlockRenderer.shouldRenderFace(level, pos, fluidState, blockState, Direction.DOWN) && !isFaceOccludedByNeighbor(level, pos, Direction.DOWN);
    const renderNorth = LiquidBlockRenderer.shouldRenderFace(level, pos, fluidState, blockState, Direction.NORTH);
    const renderSouth = LiquidBlockRenderer.shouldRenderFace(level, pos, fluidState, blockState, Direction.SOUTH);
    const renderWest = LiquidBlockRenderer.shouldRenderFace(level, pos, fluidState, blockState, Direction.WEST);
    const renderEast = LiquidBlockRenderer.shouldRenderFace(level, pos, fluidState, blockState, Direction.EAST);
    if (!renderTop && !renderBottom && !renderEast && !renderWest && !renderNorth && !renderSouth) {
      return false;
    }

    let rendered = false;
    const shadeDown = level.getShade(Direction.DOWN, true);
    const shadeUp = level.getShade(Direction.UP, true);
    const shadeNorth = level.getShade(Direction.NORTH, true);
    const shadeWest = level.getShade(Direction.WEST, true);
    let heightNW = this.getWaterHeight(level, pos, fluidState.getType());
    let heightSW = this.getWaterHeight(level, pos.south(), fluidState.getType());
    let heightSE = this.getWaterHeight(level, pos.east().south(), fluidState.getType());
    let heightNE = this.getWaterHeight(level, pos.east(), fluidState.getType());
    const x = pos.getX() & 15;
    const y = pos.getY() & 15;
    const z = pos.getZ() & 15;
    const bottomOffset = renderBottom ? 0.001 : 0.0;
    if (renderTop && !isFaceOccludedByNeighbor(level, pos, Direction.UP)) {
      rendered = true;
      heightNW -= 0.001;
      heightSW -= 0.001;
      heightSE -= 0.001;
      heightNE -= 0.001;
      let u0: number;
      let v0: number;
      let u1: number;
      let v1: number;
      let u2: number;
      let v2: number;
      let u3: number;
      let v3: number;
      const flow = fluidState.getFlow(level, pos);
      if (flow.x === 0.0 && flow.z === 0.0) {
        const sprite = sprites[0]!;
        u0 = sprite.getU(0.0);
        v0 = sprite.getV(0.0);
        u1 = u0;
        v1 = sprite.getV(16.0);
        u2 = sprite.getU(16.0);
        v2 = v1;
        u3 = u2;
        v3 = v0;
      } else {
        const sprite = sprites[1]!;
        const angle = Math.atan2(flow.z, flow.x) - (Math.PI / 2.0);
        const sine = Math.sin(angle) * 0.25;
        const cosine = Math.cos(angle) * 0.25;
        u0 = sprite.getU(8.0 + (-cosine - sine) * 16.0);
        v0 = sprite.getV(8.0 + (-cosine + sine) * 16.0);
        u1 = sprite.getU(8.0 + (-cosine + sine) * 16.0);
        v1 = sprite.getV(8.0 + (cosine + sine) * 16.0);
        u2 = sprite.getU(8.0 + (cosine + sine) * 16.0);
        v2 = sprite.getV(8.0 + (cosine - sine) * 16.0);
        u3 = sprite.getU(8.0 + (cosine - sine) * 16.0);
        v3 = sprite.getV(8.0 + (-cosine - sine) * 16.0);
      }

      const averageU = (u0 + u1 + u2 + u3) / 4.0;
      const averageV = (v0 + v1 + v2 + v3) / 4.0;
      const width = sprites[0]!.getWidth() / (sprites[0]!.getU1() - sprites[0]!.getU0());
      const height = sprites[0]!.getHeight() / (sprites[0]!.getV1() - sprites[0]!.getV0());
      const shrink = 4.0 / Math.max(height, width);
      u0 = lerp(shrink, u0, averageU);
      u1 = lerp(shrink, u1, averageU);
      u2 = lerp(shrink, u2, averageU);
      u3 = lerp(shrink, u3, averageU);
      v0 = lerp(shrink, v0, averageV);
      v1 = lerp(shrink, v1, averageV);
      v2 = lerp(shrink, v2, averageV);
      v3 = lerp(shrink, v3, averageV);
      const light = this.getLightColor(level, pos);
      const shadedRed = shadeUp * red;
      const shadedGreen = shadeUp * green;
      const shadedBlue = shadeUp * blue;
      this.vertex(consumer, x + 0.0, y + heightNW, z + 0.0, shadedRed, shadedGreen, shadedBlue, u0, v0, light);
      this.vertex(consumer, x + 0.0, y + heightSW, z + 1.0, shadedRed, shadedGreen, shadedBlue, u1, v1, light);
      this.vertex(consumer, x + 1.0, y + heightSE, z + 1.0, shadedRed, shadedGreen, shadedBlue, u2, v2, light);
      this.vertex(consumer, x + 1.0, y + heightNE, z + 0.0, shadedRed, shadedGreen, shadedBlue, u3, v3, light);
      if (fluidState.shouldRenderBackwardUpFace(level, pos.above())) {
        this.vertex(consumer, x + 0.0, y + heightNW, z + 0.0, shadedRed, shadedGreen, shadedBlue, u0, v0, light);
        this.vertex(consumer, x + 1.0, y + heightNE, z + 0.0, shadedRed, shadedGreen, shadedBlue, u3, v3, light);
        this.vertex(consumer, x + 1.0, y + heightSE, z + 1.0, shadedRed, shadedGreen, shadedBlue, u2, v2, light);
        this.vertex(consumer, x + 0.0, y + heightSW, z + 1.0, shadedRed, shadedGreen, shadedBlue, u1, v1, light);
      }
    }

    if (renderBottom) {
      const sprite = sprites[0]!;
      const light = this.getLightColor(level, pos.below());
      const shadedRed = shadeDown * red;
      const shadedGreen = shadeDown * green;
      const shadedBlue = shadeDown * blue;
      this.vertex(consumer, x, y + bottomOffset, z + 1.0, shadedRed, shadedGreen, shadedBlue, sprite.getU0(), sprite.getV1(), light);
      this.vertex(consumer, x, y + bottomOffset, z, shadedRed, shadedGreen, shadedBlue, sprite.getU0(), sprite.getV0(), light);
      this.vertex(consumer, x + 1.0, y + bottomOffset, z, shadedRed, shadedGreen, shadedBlue, sprite.getU1(), sprite.getV0(), light);
      this.vertex(consumer, x + 1.0, y + bottomOffset, z + 1.0, shadedRed, shadedGreen, shadedBlue, sprite.getU1(), sprite.getV1(), light);
      rendered = true;
    }

    const light = this.getLightColor(level, pos);
    for (let side = 0; side < 4; side++) {
      let height0: number;
      let height1: number;
      let x0: number;
      let x1: number;
      let z0: number;
      let z1: number;
      let direction: Direction;
      let shouldRender: boolean;
      if (side === 0) {
        height0 = heightNW;
        height1 = heightNE;
        x0 = x;
        x1 = x + 1.0;
        z0 = z + 0.001;
        z1 = z + 0.001;
        direction = Direction.NORTH;
        shouldRender = renderNorth;
      } else if (side === 1) {
        height0 = heightSE;
        height1 = heightSW;
        x0 = x + 1.0;
        x1 = x;
        z0 = z + 1.0 - 0.001;
        z1 = z + 1.0 - 0.001;
        direction = Direction.SOUTH;
        shouldRender = renderSouth;
      } else if (side === 2) {
        height0 = heightSW;
        height1 = heightNW;
        x0 = x + 0.001;
        x1 = x + 0.001;
        z0 = z + 1.0;
        z1 = z;
        direction = Direction.WEST;
        shouldRender = renderWest;
      } else {
        height0 = heightNE;
        height1 = heightSE;
        x0 = x + 1.0 - 0.001;
        x1 = x + 1.0 - 0.001;
        z0 = z;
        z1 = z + 1.0;
        direction = Direction.EAST;
        shouldRender = renderEast;
      }

      if (shouldRender && !isFaceOccludedByNeighbor(level, pos, direction)) {
        rendered = true;
        const sprite = (!lava && this.waterOverlay !== undefined && !level.getBlockState(pos.relative(direction)).getMaterial().isSolid())
          ? this.waterOverlay
          : sprites[1]!;
        const u0 = sprite.getU(0.0);
        const u1 = sprite.getU(8.0);
        const v0 = sprite.getV((1.0 - height0) * 16.0 * 0.5);
        const v1 = sprite.getV((1.0 - height1) * 16.0 * 0.5);
        const v2 = sprite.getV(8.0);
        const sideShade = side < 2 ? shadeNorth : shadeWest;
        const shadedRed = shadeUp * sideShade * red;
        const shadedGreen = shadeUp * sideShade * green;
        const shadedBlue = shadeUp * sideShade * blue;
        this.vertex(consumer, x0, y + height0, z0, shadedRed, shadedGreen, shadedBlue, u0, v0, light);
        this.vertex(consumer, x1, y + height1, z1, shadedRed, shadedGreen, shadedBlue, u1, v1, light);
        this.vertex(consumer, x1, y + bottomOffset, z1, shadedRed, shadedGreen, shadedBlue, u1, v2, light);
        this.vertex(consumer, x0, y + bottomOffset, z0, shadedRed, shadedGreen, shadedBlue, u0, v2, light);
        if (sprite !== this.waterOverlay) {
          this.vertex(consumer, x0, y + bottomOffset, z0, shadedRed, shadedGreen, shadedBlue, u0, v2, light);
          this.vertex(consumer, x1, y + bottomOffset, z1, shadedRed, shadedGreen, shadedBlue, u1, v2, light);
          this.vertex(consumer, x1, y + height1, z1, shadedRed, shadedGreen, shadedBlue, u1, v1, light);
          this.vertex(consumer, x0, y + height0, z0, shadedRed, shadedGreen, shadedBlue, u0, v0, light);
        }
      }
    }

    return rendered;
  }

  private vertex(
    consumer: VertexConsumer,
    x: number,
    y: number,
    z: number,
    red: number,
    green: number,
    blue: number,
    u: number,
    v: number,
    light: number,
  ): void {
    consumer.vertex(x, y, z).color(red, green, blue, 1.0).uv(u, v).uv2(light).normal(0.0, 1.0, 0.0).endVertex();
  }

  private getLightColor(level: BlockAndTintGetter, pos: BlockPos): number {
    const baseLight = LevelRenderer.getLightColor(level, pos);
    const aboveLight = LevelRenderer.getLightColor(level, pos.above());
    const block = baseLight & 0xff;
    const aboveBlock = aboveLight & 0xff;
    const sky = (baseLight >> 16) & 0xff;
    const aboveSky = (aboveLight >> 16) & 0xff;
    return (Math.max(block, aboveBlock)) | (Math.max(sky, aboveSky) << 16);
  }

  private getWaterHeight(level: BlockGetter, pos: BlockPos, fluid: Fluid): number {
    let count = 0;
    let total = 0.0;
    for (let corner = 0; corner < 4; corner++) {
      const samplePos = pos.offset(-(corner & 1), 0, -((corner >> 1) & 1));
      if (level.getFluidState(samplePos.above()).getType().isSame(fluid)) {
        return 1.0;
      }

      const sampleFluid = level.getFluidState(samplePos);
      if (sampleFluid.getType().isSame(fluid)) {
        const height = sampleFluid.getHeight(level, samplePos);
        if (height >= 0.8) {
          total += height * 10.0;
          count += 10;
        } else {
          total += height;
          count++;
        }
      } else if (!level.getBlockState(samplePos).getMaterial().isSolid()) {
        count++;
      }
    }

    return count === 0 ? 0.0 : total / count;
  }
}
