import { NativeImage } from "./texture/native-image";
import { GameRenderer } from "./game-renderer";
import { StaticRenderLevel } from "../world/level/static-render-level";

function fillBrightnessRamp(ambientLight: number): number[] {
  const ramp = new Array<number>(16);
  for (let index = 0; index <= 15; index++) {
    const ratio = index / 15.0;
    const value = ratio / (4.0 - (3.0 * ratio));
    ramp[index] = value + ((1.0 - value) * ambientLight);
  }

  return ramp;
}

function notGamma(value: number): number {
  const inverse = 1.0 - value;
  return 1.0 - (((inverse * inverse) * inverse) * inverse);
}

export class LightTexture {
  public static readonly FULL_BRIGHT = 15_728_880;
  public static readonly FULL_SKY = 15_728_640;
  public static readonly FULL_BLOCK = 240;

  private readonly lightPixels = new NativeImage(16, 16, false);
  private readonly texture: GPUTexture;
  private readonly textureView: GPUTextureView;
  private updateLightTextureValue = true;
  private blockLightRedFlicker = 0;
  private readonly brightnessRamp: readonly number[];

  public constructor(
    private readonly renderer: GameRenderer,
    private readonly level: StaticRenderLevel,
    private readonly device: GPUDevice,
  ) {
    this.texture = this.device.createTexture({
      size: { width: 16, height: 16 },
      format: "rgba8unorm",
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    this.textureView = this.texture.createView();
    this.brightnessRamp = fillBrightnessRamp(this.level.getAmbientLight());
    for (let y = 0; y < 16; y++) {
      for (let x = 0; x < 16; x++) {
        this.lightPixels.setPixelRGBA(x, y, 0xffffffff);
      }
    }

    this.lightPixels.upload(this.device.queue, this.texture, 0, 0, 0, 0, 0, 16, 16);
  }

  public close(): void {
    this.lightPixels.close();
    this.texture.destroy();
  }

  public tick(): void {
    this.blockLightRedFlicker += (Math.random() - Math.random()) * Math.random() * Math.random() * 0.1;
    this.blockLightRedFlicker *= 0.9;
    this.updateLightTextureValue = true;
  }

  public turnOnLightLayer(): GPUTextureView {
    return this.textureView;
  }

  public getTextureView(): GPUTextureView {
    return this.textureView;
  }

  // WebGPU: static smoke levels omit player-effect and weather-driven lightmap branches until the client simulation exists.
  public updateLightTexture(partialTick: number): void {
    if (!this.updateLightTextureValue) {
      return;
    }

    this.updateLightTextureValue = false;
    const skyDarken = 0.0;
    const skyBrightnessScale = (skyDarken * 0.95) + 0.05;
    const blockBrightnessScale = this.blockLightRedFlicker + 1.5;
    const darkenWorldAmount = this.renderer.getDarkenWorldAmount(partialTick);
    const skyRed = skyDarken + ((1.0 - skyDarken) * 0.35);
    const skyGreen = skyDarken + ((1.0 - skyDarken) * 0.35);
    const skyBlue = 1.0;

    for (let sky = 0; sky < 16; sky++) {
      for (let block = 0; block < 16; block++) {
        const skyBrightness = this.getBrightness(sky) * skyBrightnessScale;
        const blockBrightness = this.getBrightness(block) * blockBrightnessScale;
        let red = blockBrightness;
        let green = blockBrightness * (((blockBrightness * 0.6) + 0.4) * 0.6 + 0.4);
        let blue = blockBrightness * (((blockBrightness * blockBrightness) * 0.6) + 0.4);
        red += skyRed * skyBrightness;
        green += skyGreen * skyBrightness;
        blue += skyBlue * skyBrightness;
        red += (0.75 - red) * 0.04;
        green += (0.75 - green) * 0.04;
        blue += (0.75 - blue) * 0.04;
        if (darkenWorldAmount > 0.0) {
          red += ((red * 0.7) - red) * darkenWorldAmount;
          green += ((green * 0.6) - green) * darkenWorldAmount;
          blue += ((blue * 0.6) - blue) * darkenWorldAmount;
        }

        red = Math.max(0.0, Math.min(1.0, red));
        green = Math.max(0.0, Math.min(1.0, green));
        blue = Math.max(0.0, Math.min(1.0, blue));

        this.lightPixels.setPixelRGBA(
          block,
          sky,
          NativeImage.combine(
            255,
            Math.trunc(notGamma(blue) * 255.0),
            Math.trunc(notGamma(green) * 255.0),
            Math.trunc(notGamma(red) * 255.0),
          ),
        );
      }
    }

    this.lightPixels.upload(this.device.queue, this.texture, 0, 0, 0, 0, 0, 16, 16);
  }

  private getBrightness(lightLevel: number): number {
    return this.brightnessRamp[lightLevel]!;
  }

  public getPixelRGBA(x: number, y: number): number {
    return this.lightPixels.getPixelRGBA(x, y);
  }

  public samplePacked(light: number): number {
    return this.lightPixels.getPixelRGBA(LightTexture.block(light), LightTexture.sky(light));
  }

  public static pack(blockLight: number, skyLight: number): number {
    return (blockLight << 4) | (skyLight << 20);
  }

  public static block(value: number): number {
    return (value >> 4) & 0xffff;
  }

  public static sky(value: number): number {
    return (value >> 20) & 0xffff;
  }
}
