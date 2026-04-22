import type { Biome } from "../biome/biome.ts";
import { longPartsToBigInt, type Int64Parts, SimpleRandomSource } from "../prng/simple-random-source.ts";
import { cos, f32, sin } from "./carver-math.ts";
import type { CarverContext, CaveCarverConfiguration } from "./carver-config.ts";
import type { ChunkPosLike, CarveSkipChecker } from "./world-carver.ts";
import { WorldCarver } from "./world-carver.ts";
import { MutableChunkBlockBuffer } from "../chunk/chunk-block-buffer.ts";

const HALF_PI = Math.fround(Math.PI / 2.0);
const PI = Math.fround(Math.PI);
const TWO_PI = Math.fround(Math.PI * 2.0);

export class CaveWorldCarver extends WorldCarver<CaveCarverConfiguration> {
  public override isStartChunk(config: CaveCarverConfiguration, random: SimpleRandomSource): boolean {
    return random.nextFloat() <= config.probability;
  }

  public override carve(
    context: CarverContext,
    config: CaveCarverConfiguration,
    chunk: MutableChunkBlockBuffer,
    biomeAccessor: (worldX: number, worldY: number, worldZ: number) => Biome,
    random: SimpleRandomSource,
    chunkPos: ChunkPosLike,
    carvingMask: Uint8Array,
  ): boolean {
    const range = (this.getRange() * 2 - 1) * 16;
    const caveCount = random.nextInt(random.nextInt(random.nextInt(this.getCaveBound()) + 1) + 1);

    for (let caveIndex = 0; caveIndex < caveCount; caveIndex++) {
      const x = (chunkPos.chunkX << 4) + random.nextInt(16);
      const y = config.y.sample(random, context);
      const z = (chunkPos.chunkZ << 4) + random.nextInt(16);
      const horizontalRadiusMultiplier = config.horizontalRadiusMultiplier.sample(random);
      const verticalRadiusMultiplier = config.verticalRadiusMultiplier.sample(random);
      const floorLevel = config.floorLevel.sample(random);
      const skipChecker: CarveSkipChecker = (_context, relativeX, relativeY, relativeZ) =>
        CaveWorldCarver.shouldSkip(relativeX, relativeY, relativeZ, floorLevel);
      let tunnelCount = 1;

      if (random.nextInt(4) === 0) {
        const yScale = config.yScale.sample(random);
        const radius = f32(1.0 + (random.nextFloat() * 6.0));
        this.createRoom(
          context,
          config,
          chunk,
          biomeAccessor,
          nextSeed(random.nextLong()),
          x,
          y,
          z,
          radius,
          yScale,
          carvingMask,
          skipChecker,
        );
        tunnelCount += random.nextInt(4);
      }

      for (let tunnelIndex = 0; tunnelIndex < tunnelCount; tunnelIndex++) {
        const yaw = f32(random.nextFloat() * TWO_PI);
        const pitch = f32((random.nextFloat() - 0.5) / 4.0);
        const thickness = this.getThickness(random);
        const branchCount = range - random.nextInt(Math.trunc(range / 4));
        this.createTunnel(
          context,
          config,
          chunk,
          biomeAccessor,
          nextSeed(random.nextLong()),
          x,
          y,
          z,
          horizontalRadiusMultiplier,
          verticalRadiusMultiplier,
          thickness,
          yaw,
          pitch,
          0,
          branchCount,
          this.getYScale(),
          carvingMask,
          skipChecker,
        );
      }
    }

    return true;
  }

  protected getCaveBound(): number {
    return 15;
  }

  protected getThickness(random: SimpleRandomSource): number {
    let thickness = f32((random.nextFloat() * 2.0) + random.nextFloat());
    if (random.nextInt(10) === 0) {
      thickness = f32(thickness * f32((random.nextFloat() * random.nextFloat() * 3.0) + 1.0));
    }

    return thickness;
  }

  protected getYScale(): number {
    return 1.0;
  }

  protected createRoom(
    context: CarverContext,
    config: CaveCarverConfiguration,
    chunk: MutableChunkBlockBuffer,
    biomeAccessor: (worldX: number, worldY: number, worldZ: number) => Biome,
    seed: bigint,
    x: number,
    y: number,
    z: number,
    radius: number,
    horizontalVerticalRatio: number,
    carvingMask: Uint8Array,
    skipChecker: CarveSkipChecker,
  ): void {
    const horizontalRadius = 1.5 + f32(sin(HALF_PI) * radius);
    const verticalRadius = horizontalRadius * horizontalVerticalRatio;
    this.carveEllipsoid(
      context,
      config,
      chunk,
      biomeAccessor,
      seed,
      x + 1.0,
      y,
      z,
      horizontalRadius,
      verticalRadius,
      carvingMask,
      skipChecker,
    );
  }

  protected createTunnel(
    context: CarverContext,
    config: CaveCarverConfiguration,
    chunk: MutableChunkBlockBuffer,
    biomeAccessor: (worldX: number, worldY: number, worldZ: number) => Biome,
    seed: bigint,
    x: number,
    y: number,
    z: number,
    horizontalRadiusMultiplier: number,
    verticalRadiusMultiplier: number,
    thickness: number,
    yaw: number,
    pitch: number,
    branchIndex: number,
    branchCount: number,
    horizontalVerticalRatio: number,
    carvingMask: Uint8Array,
    skipChecker: CarveSkipChecker,
  ): void {
    const random = new SimpleRandomSource(seed);
    const splitBranchIndex = random.nextInt(Math.trunc(branchCount / 2)) + Math.trunc(branchCount / 4);
    const widePitch = random.nextInt(6) === 0;
    let yawVelocity = f32(0.0);
    let pitchVelocity = f32(0.0);

    for (let currentBranch = branchIndex; currentBranch < branchCount; currentBranch++) {
      const angle = f32((PI * currentBranch) / branchCount);
      const horizontalRadius = 1.5 + f32(sin(angle) * thickness);
      const verticalRadius = horizontalRadius * horizontalVerticalRatio;
      const pitchCos = cos(pitch);
      x += f32(cos(yaw) * pitchCos);
      y += sin(pitch);
      z += f32(sin(yaw) * pitchCos);
      pitch = f32(pitch * (widePitch ? 0.92 : 0.7));
      pitch = f32(pitch + f32(pitchVelocity * 0.1));
      yaw = f32(yaw + f32(yawVelocity * 0.1));
      pitchVelocity = f32(pitchVelocity * 0.9);
      yawVelocity = f32(yawVelocity * 0.75);
      pitchVelocity = f32(
        pitchVelocity + f32(f32(random.nextFloat() - random.nextFloat()) * random.nextFloat() * 2.0),
      );
      yawVelocity = f32(
        yawVelocity + f32(f32(random.nextFloat() - random.nextFloat()) * random.nextFloat() * 4.0),
      );

      if (currentBranch === splitBranchIndex && thickness > 1.0) {
        this.createTunnel(
          context,
          config,
          chunk,
          biomeAccessor,
          nextSeed(random.nextLong()),
          x,
          y,
          z,
          horizontalRadiusMultiplier,
          verticalRadiusMultiplier,
          f32((random.nextFloat() * 0.5) + 0.5),
          f32(yaw - HALF_PI),
          f32(pitch / 3.0),
          currentBranch,
          branchCount,
          1.0,
          carvingMask,
          skipChecker,
        );
        this.createTunnel(
          context,
          config,
          chunk,
          biomeAccessor,
          nextSeed(random.nextLong()),
          x,
          y,
          z,
          horizontalRadiusMultiplier,
          verticalRadiusMultiplier,
          f32((random.nextFloat() * 0.5) + 0.5),
          f32(yaw + HALF_PI),
          f32(pitch / 3.0),
          currentBranch,
          branchCount,
          1.0,
          carvingMask,
          skipChecker,
        );
        return;
      }

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
          horizontalRadius * horizontalRadiusMultiplier,
          verticalRadius * verticalRadiusMultiplier,
          carvingMask,
          skipChecker,
        );
      }
    }
  }

  private static shouldSkip(relativeX: number, relativeY: number, relativeZ: number, floorLevel: number): boolean {
    return relativeY <= floorLevel ? true : ((relativeX * relativeX) + (relativeY * relativeY) + (relativeZ * relativeZ)) >= 1.0;
  }
}

function nextSeed(parts: Int64Parts): bigint {
  return longPartsToBigInt(parts);
}
