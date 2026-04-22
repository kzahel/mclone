import type { Biome } from "../biome/biome.ts";
import { longPartsToBigInt, type Int64Parts, SimpleRandomSource } from "../prng/simple-random-source.ts";
import { cos, f32, randomBetween, sin } from "./carver-math.ts";
import type { CanyonCarverConfiguration, CarverContext } from "./carver-config.ts";
import type { ChunkPosLike } from "./world-carver.ts";
import { WorldCarver } from "./world-carver.ts";
import { MutableChunkBlockBuffer } from "../chunk/chunk-block-buffer.ts";

const PI = Math.fround(Math.PI);
const TWO_PI = Math.fround(Math.PI * 2.0);

export class CanyonWorldCarver extends WorldCarver<CanyonCarverConfiguration> {
  public override isStartChunk(config: CanyonCarverConfiguration, random: SimpleRandomSource): boolean {
    return random.nextFloat() <= config.probability;
  }

  public override carve(
    context: CarverContext,
    config: CanyonCarverConfiguration,
    chunk: MutableChunkBlockBuffer,
    biomeAccessor: (worldX: number, worldY: number, worldZ: number) => Biome,
    random: SimpleRandomSource,
    chunkPos: ChunkPosLike,
    carvingMask: Uint8Array,
  ): boolean {
    const range = (this.getRange() * 2 - 1) * 16;
    const x = (chunkPos.chunkX << 4) + random.nextInt(16);
    const y = config.y.sample(random, context);
    const z = (chunkPos.chunkZ << 4) + random.nextInt(16);
    const yaw = f32(random.nextFloat() * TWO_PI);
    const pitch = config.verticalRotation.sample(random);
    const yScale = config.yScale.sample(random);
    const thickness = config.shape.thickness.sample(random);
    const branchCount = Math.trunc(range * config.shape.distanceFactor.sample(random));

    this.doCarve(
      context,
      config,
      chunk,
      biomeAccessor,
      nextSeed(random.nextLong()),
      x,
      y,
      z,
      thickness,
      yaw,
      pitch,
      0,
      branchCount,
      yScale,
      carvingMask,
    );
    return true;
  }

  private doCarve(
    context: CarverContext,
    config: CanyonCarverConfiguration,
    chunk: MutableChunkBlockBuffer,
    biomeAccessor: (worldX: number, worldY: number, worldZ: number) => Biome,
    seed: bigint,
    x: number,
    y: number,
    z: number,
    thickness: number,
    yaw: number,
    pitch: number,
    branchIndex: number,
    branchCount: number,
    horizontalVerticalRatio: number,
    carvingMask: Uint8Array,
  ): void {
    const random = new SimpleRandomSource(seed);
    const widthFactors = this.initWidthFactors(context, config, random);
    let yawVelocity = f32(0.0);
    let pitchVelocity = f32(0.0);

    for (let currentBranch = branchIndex; currentBranch < branchCount; currentBranch++) {
      const angle = f32((currentBranch * PI) / branchCount);
      let horizontalRadius = 1.5 + f32(sin(angle) * thickness);
      let verticalRadius = horizontalRadius * horizontalVerticalRatio;
      horizontalRadius *= config.shape.horizontalRadiusFactor.sample(random);
      verticalRadius = this.updateVerticalRadius(config, random, verticalRadius, branchCount, currentBranch);

      const pitchCos = cos(pitch);
      const pitchSin = sin(pitch);
      x += f32(cos(yaw) * pitchCos);
      y += pitchSin;
      z += f32(sin(yaw) * pitchCos);
      pitch = f32(pitch * 0.7);
      pitch = f32(pitch + f32(pitchVelocity * 0.05));
      yaw = f32(yaw + f32(yawVelocity * 0.05));
      pitchVelocity = f32(pitchVelocity * 0.8);
      yawVelocity = f32(yawVelocity * 0.5);
      pitchVelocity = f32(
        pitchVelocity + f32(f32(random.nextFloat() - random.nextFloat()) * random.nextFloat() * 2.0),
      );
      yawVelocity = f32(
        yawVelocity + f32(f32(random.nextFloat() - random.nextFloat()) * random.nextFloat() * 4.0),
      );

      if (random.nextInt(4) !== 0) {
        if (!WorldCarver.canReach(chunk, x, z, currentBranch, branchCount, thickness)) {
          return;
        }

        this.carveEllipsoid(
          context,
          config,
          chunk,
          biomeAccessor,
          seed,
          x,
          y,
          z,
          horizontalRadius,
          verticalRadius,
          carvingMask,
          (skipContext, relativeX, relativeY, relativeZ, worldY) =>
            this.shouldSkip(skipContext, widthFactors, relativeX, relativeY, relativeZ, worldY),
        );
      }
    }
  }

  private initWidthFactors(
    context: CarverContext,
    config: CanyonCarverConfiguration,
    random: SimpleRandomSource,
  ): Float32Array {
    const widthFactors = new Float32Array(context.genDepth);
    let width = f32(1.0);

    for (let index = 0; index < context.genDepth; index++) {
      if (index === 0 || random.nextInt(config.shape.widthSmoothness) === 0) {
        width = f32(1.0 + (random.nextFloat() * random.nextFloat()));
      }

      widthFactors[index] = f32(width * width);
    }

    return widthFactors;
  }

  private updateVerticalRadius(
    config: CanyonCarverConfiguration,
    random: SimpleRandomSource,
    verticalRadius: number,
    branchCount: number,
    branchIndex: number,
  ): number {
    const centeredProgress = f32(1.0 - (Math.abs(0.5 - (branchIndex / branchCount)) * 2.0));
    const factor = f32(
      config.shape.verticalRadiusDefaultFactor + (config.shape.verticalRadiusCenterFactor * centeredProgress),
    );
    return factor * verticalRadius * randomBetween(random, 0.75, 1.0);
  }

  private shouldSkip(
    context: CarverContext,
    widthFactors: Float32Array,
    relativeX: number,
    relativeY: number,
    relativeZ: number,
    worldY: number,
  ): boolean {
    const relativeIndex = worldY - context.minY;
    return ((((relativeX * relativeX) + (relativeZ * relativeZ)) * widthFactors[relativeIndex - 1]!) +
      ((relativeY * relativeY) / 6.0)) >= 1.0;
  }
}

function nextSeed(parts: Int64Parts): bigint {
  return longPartsToBigInt(parts);
}
