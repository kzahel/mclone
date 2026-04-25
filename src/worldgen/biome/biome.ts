import type { NoiseBiome } from "./noise-biome";
import { BiomeGenerationSettings } from "./biome-generation-settings";
import { clamp } from "../../util/mth";
import { Registry } from "../../core/registry";
import { ResourceLocation } from "../../core/resource-location";
import { FoliageColor } from "../../world/level/foliage-color";
import { GrassColor } from "../../world/level/grass-color";
import { BlockPos } from "../../core/block-pos";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import { LightLayer } from "../../world/level/light-layer";
import { GenerationStep } from "../levelgen/generation-step";
import type { NoiseBasedChunkGenerator } from "../levelgen/noise-based-chunk-generator";
import type { CooperativeGenerationYield } from "../levelgen/cooperative-generation";
import {
  describeConfiguredFeature,
  monotonicDecorationNowMs,
  type BiomeDecorationProfiler,
} from "../levelgen/decoration-profiler";
import { PerlinSimplexNoise } from "../noise/perlin-simplex-noise";
import { WorldgenRandom } from "../prng/worldgen-random";
import { Fluids } from "../../world/level/material/fluids";
import { LiquidBlock } from "../../world/level/block/liquid-block";
import type { Block } from "../../world/level/block/block";

const SNOW_LOCATION = new ResourceLocation("minecraft:snow");

export interface BiomeDefinition {
  id: number;
  key: string;
  depth: number;
  scale: number;
  temperature: number;
  downfall: number;
  waterColor: number;
  foliageColorOverride?: number;
  grassColorOverride?: number;
  grassColorModifier?: GrassColorModifier;
  temperatureModifier?: TemperatureModifier;
}

export enum GrassColorModifier {
  NONE = "none",
  DARK_FOREST = "dark_forest",
  SWAMP = "swamp",
}

export enum TemperatureModifier {
  NONE = "none",
  FROZEN = "frozen",
}

export class Biome implements NoiseBiome {
  private static readonly TEMPERATURE_NOISE = new PerlinSimplexNoise(new WorldgenRandom(1234n), [0]);
  private static readonly FROZEN_TEMPERATURE_NOISE = new PerlinSimplexNoise(new WorldgenRandom(3456n), [-2, -1, 0]);
  public static readonly BIOME_INFO_NOISE = new PerlinSimplexNoise(new WorldgenRandom(2345n), [0]);
  private readonly temperatureCache = new Map<bigint, number>();

  public constructor(
    private readonly id: number,
    private readonly key: string,
    private readonly depth: number,
    private readonly scale: number,
    private readonly temperature: number,
    private readonly downfall: number,
    private readonly waterColor: number,
    private readonly foliageColorOverride?: number,
    private readonly grassColorOverride?: number,
    private readonly grassColorModifier: GrassColorModifier = GrassColorModifier.NONE,
    private readonly temperatureModifier: TemperatureModifier = TemperatureModifier.NONE,
    private readonly generationSettings: BiomeGenerationSettings = BiomeGenerationSettings.EMPTY,
  ) {}

  public getId(): number {
    return this.id;
  }

  public getKey(): string {
    return this.key;
  }

  public getDepth(): number {
    return this.depth;
  }

  public getScale(): number {
    return this.scale;
  }

  public getBaseTemperature(): number {
    return this.temperature;
  }

  public getTemperature(pos: BlockPos): number {
    const key = pos.asLong();
    const cached = this.temperatureCache.get(key);
    if (cached !== undefined) {
      return cached;
    }

    let temperature = this.modifyTemperature(pos, this.temperature);
    if (pos.getY() > 64) {
      const noise = Biome.TEMPERATURE_NOISE.getValue(pos.getX() / 8.0, pos.getZ() / 8.0, false) * 4.0;
      temperature -= ((noise + pos.getY()) - 64.0) * (0.05 / 30.0);
    }

    if (this.temperatureCache.size >= 1024) {
      const oldest = this.temperatureCache.keys().next();
      if (!oldest.done) {
        this.temperatureCache.delete(oldest.value);
      }
    }

    this.temperatureCache.set(key, temperature);
    return temperature;
  }

  public getDownfall(): number {
    return this.downfall;
  }

  public getGrassColor(x: number, z: number): number {
    const color = this.grassColorOverride ?? this.getGrassColorFromTexture();
    switch (this.grassColorModifier) {
      case GrassColorModifier.DARK_FOREST:
        return (((color & 0xfefefe) + 0x28340a) >> 1);
      case GrassColorModifier.SWAMP: {
        const noise = Biome.BIOME_INFO_NOISE.getValue(x * 0.0225, z * 0.0225, false);
        return noise < -0.1 ? 5_011_004 : 6_975_545;
      }
      case GrassColorModifier.NONE:
      default:
        return color;
    }
  }

  public getFoliageColor(): number {
    return this.foliageColorOverride ?? this.getFoliageColorFromTexture();
  }

  public getWaterColor(): number {
    return this.waterColor;
  }

  public shouldFreeze(level: WorldGenLevel, pos: BlockPos, mustBeAtEdge = true): boolean {
    if (this.getTemperature(pos) >= 0.15) {
      return false;
    }

    if (pos.getY() < level.getMinBuildHeight() || pos.getY() >= level.getMaxBuildHeight() || level.getBrightness(LightLayer.BLOCK, pos) >= 10) {
      return false;
    }

    const state = level.getBlockState(pos);
    const fluidState = level.getFluidState(pos);
    if (!fluidState.getType().isSame(Fluids.WATER) || !(state.getBlock() instanceof LiquidBlock)) {
      return false;
    }

    if (!mustBeAtEdge) {
      return true;
    }

    const fullySurroundedByWater =
      Biome.isWaterAt(level, pos.west())
      && Biome.isWaterAt(level, pos.east())
      && Biome.isWaterAt(level, pos.north())
      && Biome.isWaterAt(level, pos.south());
    return !fullySurroundedByWater;
  }

  public isColdEnoughToSnow(pos: BlockPos): boolean {
    return this.getTemperature(pos) < 0.15;
  }

  public shouldSnow(level: WorldGenLevel, pos: BlockPos): boolean {
    if (!this.isColdEnoughToSnow(pos)) {
      return false;
    }

    if (pos.getY() < level.getMinBuildHeight() || pos.getY() >= level.getMaxBuildHeight() || level.getBrightness(LightLayer.BLOCK, pos) >= 10) {
      return false;
    }

    const snowBlock = Registry.BLOCK.get(SNOW_LOCATION) as Block | undefined;
    if (snowBlock === undefined) {
      throw new Error(`Missing registered block ${SNOW_LOCATION}`);
    }

    const state = level.getBlockState(pos);
    return state.isAir() && snowBlock.defaultBlockState().canSurvive(level, pos);
  }

  public getGenerationSettings(): BiomeGenerationSettings {
    return this.generationSettings;
  }

  public generate(
    chunkGenerator: NoiseBasedChunkGenerator,
    level: WorldGenLevel,
    decorationSeed: bigint,
    random: WorldgenRandom,
    origin: BlockPos,
    profiler?: BiomeDecorationProfiler,
  ): void {
    const features = this.generationSettings.features();
    // TypeScript: structure placement stays deferred here; only configured features run during biome decoration.
    for (let stepIndex = 0; stepIndex < GenerationStep.DECORATION_VALUES.length; stepIndex++) {
      if (features.length <= stepIndex) {
        continue;
      }

      let featureIndex = 0;
      for (const featureSupplier of features[stepIndex]!) {
        const feature = featureSupplier();
        random.setFeatureSeed(decorationSeed, featureIndex, stepIndex);
        if (profiler === undefined) {
          feature.place(level, chunkGenerator, random, origin);
        } else {
          const info = describeConfiguredFeature(stepIndex, featureIndex, feature);
          profiler.beginFeature(info);
          const startedAtMs = monotonicDecorationNowMs();
          const placed = feature.place(level, chunkGenerator, random, origin);
          profiler.endFeature(info, placed, monotonicDecorationNowMs() - startedAtMs);
        }
        featureIndex++;
      }
    }
  }

  public async generateCooperative(
    chunkGenerator: NoiseBasedChunkGenerator,
    level: WorldGenLevel,
    decorationSeed: bigint,
    random: WorldgenRandom,
    origin: BlockPos,
    yieldStep: CooperativeGenerationYield,
    profiler?: BiomeDecorationProfiler,
  ): Promise<void> {
    const features = this.generationSettings.features();
    // Host scheduling: same feature order as generate(), but yields between configured placement units.
    for (let stepIndex = 0; stepIndex < GenerationStep.DECORATION_VALUES.length; stepIndex++) {
      if (features.length <= stepIndex) {
        continue;
      }

      let featureIndex = 0;
      for (const featureSupplier of features[stepIndex]!) {
        const feature = featureSupplier();
        random.setFeatureSeed(decorationSeed, featureIndex, stepIndex);
        if (profiler === undefined) {
          await feature.placeCooperative(level, chunkGenerator, random, origin, yieldStep);
        } else {
          const info = describeConfiguredFeature(stepIndex, featureIndex, feature);
          profiler.beginFeature(info);
          const startedAtMs = monotonicDecorationNowMs();
          const placed = await feature.placeCooperative(level, chunkGenerator, random, origin, yieldStep);
          profiler.endFeature(info, placed, monotonicDecorationNowMs() - startedAtMs);
        }
        featureIndex++;
      }
    }
  }

  private getGrassColorFromTexture(): number {
    const temperature = clamp(this.temperature, 0.0, 1.0);
    const downfall = clamp(this.downfall, 0.0, 1.0);
    return GrassColor.get(temperature, downfall);
  }

  private getFoliageColorFromTexture(): number {
    const temperature = clamp(this.temperature, 0.0, 1.0);
    const downfall = clamp(this.downfall, 0.0, 1.0);
    return FoliageColor.get(temperature, downfall);
  }

  private modifyTemperature(pos: BlockPos, baseTemperature: number): number {
    switch (this.temperatureModifier) {
      case TemperatureModifier.FROZEN: {
        const frozenNoise = Biome.FROZEN_TEMPERATURE_NOISE.getValue(pos.getX() * 0.05, pos.getZ() * 0.05, false) * 7.0;
        const biomeNoise = Biome.BIOME_INFO_NOISE.getValue(pos.getX() * 0.2, pos.getZ() * 0.2, false);
        if ((frozenNoise + biomeNoise) < 0.3) {
          const detailNoise = Biome.BIOME_INFO_NOISE.getValue(pos.getX() * 0.09, pos.getZ() * 0.09, false);
          if (detailNoise < 0.8) {
            return 0.2;
          }
        }

        return baseTemperature;
      }
      case TemperatureModifier.NONE:
      default:
        return baseTemperature;
    }
  }

  private static isWaterAt(level: WorldGenLevel, pos: BlockPos): boolean {
    const state = level.getBlockState(pos);
    const fluidState = level.getFluidState(pos);
    return fluidState.getType().isSame(Fluids.WATER) && state.getBlock() instanceof LiquidBlock;
  }
}
