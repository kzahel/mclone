import { NoiseSamplingSettings } from "./noise-sampling-settings.ts";
import { NoiseSettings } from "./noise-settings.ts";
import { NoiseSlideSettings } from "./noise-slide-settings.ts";

const INT_MIN = -2_147_483_648;

export class NoiseGeneratorSettings {
  private constructor(
    private readonly noiseSettingsValue: NoiseSettings,
    private readonly defaultBlockValue: string,
    private readonly defaultFluidValue: string,
    private readonly bedrockRoofPositionValue: number,
    private readonly bedrockFloorPositionValue: number,
    private readonly seaLevelValue: number,
    private readonly minSurfaceLevelValue: number,
    private readonly aquifersEnabledValue: boolean,
    private readonly noiseCavesEnabledValue: boolean,
    private readonly deepslateEnabledValue: boolean,
    private readonly oreVeinsEnabledValue: boolean,
    private readonly noodleCavesEnabledValue: boolean,
  ) {}

  public noiseSettings(): NoiseSettings {
    return this.noiseSettingsValue;
  }

  public getDefaultBlock(): string {
    return this.defaultBlockValue;
  }

  public getDefaultFluid(): string {
    return this.defaultFluidValue;
  }

  public getBedrockRoofPosition(): number {
    return this.bedrockRoofPositionValue;
  }

  public getBedrockFloorPosition(): number {
    return this.bedrockFloorPositionValue;
  }

  public seaLevel(): number {
    return this.seaLevelValue;
  }

  public getMinSurfaceLevel(): number {
    return this.minSurfaceLevelValue;
  }

  public isAquifersEnabled(): boolean {
    return this.aquifersEnabledValue;
  }

  public isNoiseCavesEnabled(): boolean {
    return this.noiseCavesEnabledValue;
  }

  public isDeepslateEnabled(): boolean {
    return this.deepslateEnabledValue;
  }

  public isOreVeinsEnabled(): boolean {
    return this.oreVeinsEnabledValue;
  }

  public isNoodleCavesEnabled(): boolean {
    return this.noodleCavesEnabledValue;
  }

  public static overworld(isAmplified = false): NoiseGeneratorSettings {
    return new NoiseGeneratorSettings(
      NoiseSettings.create(
        0,
        256,
        new NoiseSamplingSettings(0.9999999814507745, 0.9999999814507745, 80, 160),
        new NoiseSlideSettings(-10, 3, 0),
        new NoiseSlideSettings(15, 3, 0),
        1,
        2,
        1,
        -0.46875,
        true,
        true,
        false,
        isAmplified,
      ),
      "minecraft:stone",
      "minecraft:water",
      INT_MIN,
      0,
      63,
      0,
      false,
      false,
      false,
      false,
      false,
    );
  }
}
