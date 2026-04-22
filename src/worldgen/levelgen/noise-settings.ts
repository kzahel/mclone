import { NoiseSamplingSettings } from "./noise-sampling-settings";
import { NoiseSlideSettings } from "./noise-slide-settings";

const BITS_FOR_Y = 12;
const Y_SIZE = (1 << BITS_FOR_Y) - 32;
const MAX_Y = (Y_SIZE >> 1) - 1;

function validateNoiseSettings(settings: NoiseSettings): void {
  if ((settings.minY() + settings.height()) > (MAX_Y + 1)) {
    throw new Error(`min_y + height cannot be higher than: ${MAX_Y + 1}`);
  }

  if ((settings.height() % 16) !== 0) {
    throw new Error("height has to be a multiple of 16");
  }

  if ((settings.minY() % 16) !== 0) {
    throw new Error("min_y has to be a multiple of 16");
  }
}

export class NoiseSettings {
  private constructor(
    private readonly minYValue: number,
    private readonly heightValue: number,
    private readonly noiseSamplingSettingsValue: NoiseSamplingSettings,
    private readonly topSlideSettingsValue: NoiseSlideSettings,
    private readonly bottomSlideSettingsValue: NoiseSlideSettings,
    private readonly noiseSizeHorizontalValue: number,
    private readonly noiseSizeVerticalValue: number,
    private readonly densityFactorValue: number,
    private readonly densityOffsetValue: number,
    private readonly useSimplexSurfaceNoiseValue: boolean,
    private readonly randomDensityOffsetValue: boolean,
    private readonly islandNoiseOverrideValue: boolean,
    private readonly amplifiedValue: boolean,
  ) {}

  public minY(): number {
    return this.minYValue;
  }

  public height(): number {
    return this.heightValue;
  }

  public noiseSamplingSettings(): NoiseSamplingSettings {
    return this.noiseSamplingSettingsValue;
  }

  public topSlideSettings(): NoiseSlideSettings {
    return this.topSlideSettingsValue;
  }

  public bottomSlideSettings(): NoiseSlideSettings {
    return this.bottomSlideSettingsValue;
  }

  public noiseSizeHorizontal(): number {
    return this.noiseSizeHorizontalValue;
  }

  public noiseSizeVertical(): number {
    return this.noiseSizeVerticalValue;
  }

  public densityFactor(): number {
    return this.densityFactorValue;
  }

  public densityOffset(): number {
    return this.densityOffsetValue;
  }

  public useSimplexSurfaceNoise(): boolean {
    return this.useSimplexSurfaceNoiseValue;
  }

  public randomDensityOffset(): boolean {
    return this.randomDensityOffsetValue;
  }

  public islandNoiseOverride(): boolean {
    return this.islandNoiseOverrideValue;
  }

  public isAmplified(): boolean {
    return this.amplifiedValue;
  }

  public static create(
    minY: number,
    height: number,
    noiseSamplingSettings: NoiseSamplingSettings,
    topSlideSettings: NoiseSlideSettings,
    bottomSlideSettings: NoiseSlideSettings,
    noiseSizeHorizontal: number,
    noiseSizeVertical: number,
    densityFactor: number,
    densityOffset: number,
    useSimplexSurfaceNoise: boolean,
    randomDensityOffset: boolean,
    islandNoiseOverride: boolean,
    isAmplified: boolean,
  ): NoiseSettings {
    const settings = new NoiseSettings(
      minY,
      height,
      noiseSamplingSettings,
      topSlideSettings,
      bottomSlideSettings,
      noiseSizeHorizontal,
      noiseSizeVertical,
      densityFactor,
      densityOffset,
      useSimplexSurfaceNoise,
      randomDensityOffset,
      islandNoiseOverride,
      isAmplified,
    );
    validateNoiseSettings(settings);
    return settings;
  }
}
