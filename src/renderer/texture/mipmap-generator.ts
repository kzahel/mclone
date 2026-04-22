import { NativeImage } from "./native-image";

const ALPHA_CUTOUT_CUTOFF = 96;
const POW22 = new Float32Array(256);

for (let index = 0; index < POW22.length; index++) {
  POW22[index] = Math.pow(index / 255.0, 2.2);
}

function getPow22(value: number): number {
  return POW22[value & 0xff]!;
}

function gammaBlend(a: number, b: number, c: number, d: number, shift: number): number {
  const powA = getPow22(a >>> shift);
  const powB = getPow22(b >>> shift);
  const powC = getPow22(c >>> shift);
  const powD = getPow22(d >>> shift);
  const average = Math.pow((powA + powB + powC + powD) * 0.25, 0.45454545454545453);
  return Math.trunc(average * 255.0);
}

function alphaBlend(a: number, b: number, c: number, d: number, hasTransparentPixel: boolean): number {
  if (hasTransparentPixel) {
    let alpha = 0.0;
    let blue = 0.0;
    let green = 0.0;
    let red = 0.0;

    if (NativeImage.getA(a) !== 0) {
      alpha += getPow22(a >>> 24);
      blue += getPow22(a >>> 16);
      green += getPow22(a >>> 8);
      red += getPow22(a >>> 0);
    }

    if (NativeImage.getA(b) !== 0) {
      alpha += getPow22(b >>> 24);
      blue += getPow22(b >>> 16);
      green += getPow22(b >>> 8);
      red += getPow22(b >>> 0);
    }

    if (NativeImage.getA(c) !== 0) {
      alpha += getPow22(c >>> 24);
      blue += getPow22(c >>> 16);
      green += getPow22(c >>> 8);
      red += getPow22(c >>> 0);
    }

    if (NativeImage.getA(d) !== 0) {
      alpha += getPow22(d >>> 24);
      blue += getPow22(d >>> 16);
      green += getPow22(d >>> 8);
      red += getPow22(d >>> 0);
    }

    alpha /= 4.0;
    blue /= 4.0;
    green /= 4.0;
    red /= 4.0;

    let blendedAlpha = Math.trunc(Math.pow(alpha, 0.45454545454545453) * 255.0);
    const blendedBlue = Math.trunc(Math.pow(blue, 0.45454545454545453) * 255.0);
    const blendedGreen = Math.trunc(Math.pow(green, 0.45454545454545453) * 255.0);
    const blendedRed = Math.trunc(Math.pow(red, 0.45454545454545453) * 255.0);
    if (blendedAlpha < ALPHA_CUTOUT_CUTOFF) {
      blendedAlpha = 0;
    }

    return NativeImage.combine(blendedAlpha, blendedBlue, blendedGreen, blendedRed);
  }

  const alpha = gammaBlend(a, b, c, d, 24);
  const blue = gammaBlend(a, b, c, d, 16);
  const green = gammaBlend(a, b, c, d, 8);
  const red = gammaBlend(a, b, c, d, 0);
  return NativeImage.combine(alpha, blue, green, red);
}

export class MipmapGenerator {
  public static generateMipLevels(image: NativeImage, mipLevel: number): NativeImage[] {
    const mipLevels = new Array<NativeImage>(mipLevel + 1);
    mipLevels[0] = image;

    if (mipLevel <= 0) {
      return mipLevels;
    }

    let hasTransparentPixel = false;
    transparentLoop:
    for (let x = 0; x < image.getWidth(); x++) {
      for (let y = 0; y < image.getHeight(); y++) {
        if (NativeImage.getA(image.getPixelRGBA(x, y)) === 0) {
          hasTransparentPixel = true;
          break transparentLoop;
        }
      }
    }

    for (let level = 1; level <= mipLevel; level++) {
      const previous = mipLevels[level - 1]!;
      const current = new NativeImage(previous.getWidth() >> 1, previous.getHeight() >> 1, false);
      for (let x = 0; x < current.getWidth(); x++) {
        for (let y = 0; y < current.getHeight(); y++) {
          current.setPixelRGBA(
            x,
            y,
            alphaBlend(
              previous.getPixelRGBA((x * 2) + 0, (y * 2) + 0),
              previous.getPixelRGBA((x * 2) + 1, (y * 2) + 0),
              previous.getPixelRGBA((x * 2) + 0, (y * 2) + 1),
              previous.getPixelRGBA((x * 2) + 1, (y * 2) + 1),
              hasTransparentPixel,
            ),
          );
        }
      }

      mipLevels[level] = current;
    }

    return mipLevels;
  }
}
