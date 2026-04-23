import type { NoiseBiome } from "./noise-biome";
import { clamp } from "../../util/mth";
import { FoliageColor } from "../../world/level/foliage-color";
import { GrassColor } from "../../world/level/grass-color";
import { PerlinSimplexNoise } from "../noise/perlin-simplex-noise";
import { WorldgenRandom } from "../prng/worldgen-random";

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
}

export enum GrassColorModifier {
  NONE = "none",
  DARK_FOREST = "dark_forest",
  SWAMP = "swamp",
}

export class Biome implements NoiseBiome {
  public static readonly BIOME_INFO_NOISE = new PerlinSimplexNoise(new WorldgenRandom(2345n), [0]);

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
}
