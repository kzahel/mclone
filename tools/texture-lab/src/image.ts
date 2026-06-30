import { isHexColor, type PaletteSpec } from "./dsl";
import type { RgbaImage } from "./png";

export type { RgbaImage } from "./png";

export type Rgba = [number, number, number, number];

export type PremultipliedRgba = [number, number, number, number];

export function downsampleNearest(image: RgbaImage, width: number, height: number): RgbaImage {
  const out = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const sourceX = Math.min(image.width - 1, Math.floor((x + 0.5) * image.width / width));
      const sourceY = Math.min(image.height - 1, Math.floor((y + 0.5) * image.height / height));
      copyPixel(image.data, image.width, sourceX, sourceY, out, width, x, y);
    }
  }
  return { width, height, data: out };
}

export function premultiply(color: Rgba): PremultipliedRgba {
  const alpha = color[3] / 255;
  return [color[0] * alpha, color[1] * alpha, color[2] * alpha, alpha];
}

export function unpremultiply(color: PremultipliedRgba): Rgba {
  const alpha = clamp01(color[3]);
  if (alpha <= 0) {
    return [0, 0, 0, 0];
  }
  return [
    clampByte(color[0] / alpha),
    clampByte(color[1] / alpha),
    clampByte(color[2] / alpha),
    clampByte(alpha * 255),
  ];
}

export function lerpPremul(a: PremultipliedRgba, b: PremultipliedRgba, alpha: number): PremultipliedRgba {
  return [
    lerp(a[0], b[0], alpha),
    lerp(a[1], b[1], alpha),
    lerp(a[2], b[2], alpha),
    lerp(a[3], b[3], alpha),
  ];
}

export function cubicPremul(
  p0: PremultipliedRgba,
  p1: PremultipliedRgba,
  p2: PremultipliedRgba,
  p3: PremultipliedRgba,
  alpha: number,
): PremultipliedRgba {
  return [
    cubic(p0[0], p1[0], p2[0], p3[0], alpha),
    cubic(p0[1], p1[1], p2[1], p3[1], alpha),
    cubic(p0[2], p1[2], p2[2], p3[2], alpha),
    cubic(p0[3], p1[3], p2[3], p3[3], alpha),
  ];
}

export function lerpRgba(a: Rgba, b: Rgba, alpha: number): Rgba {
  return [
    clampByte(lerp(a[0], b[0], alpha)),
    clampByte(lerp(a[1], b[1], alpha)),
    clampByte(lerp(a[2], b[2], alpha)),
    clampByte(lerp(a[3], b[3], alpha)),
  ];
}

export function fill(data: Uint8Array, color: Rgba): void {
  for (let index = 0; index < data.length; index += 4) {
    data[index] = color[0];
    data[index + 1] = color[1];
    data[index + 2] = color[2];
    data[index + 3] = color[3];
  }
}

export function solidImage(width: number, height: number, color: Rgba): RgbaImage {
  const data = new Uint8Array(width * height * 4);
  fill(data, color);
  return { width, height, data };
}

export function drawScaled(target: RgbaImage, source: RgbaImage, targetX: number, targetY: number, scale: number): void {
  for (let y = 0; y < source.height; y += 1) {
    for (let x = 0; x < source.width; x += 1) {
      const color = readPixel(source, x, y);
      drawRect(target, targetX + x * scale, targetY + y * scale, scale, scale, color);
    }
  }
}

export function drawTiledScaled(
  target: RgbaImage,
  source: RgbaImage,
  targetX: number,
  targetY: number,
  tilesX: number,
  tilesY: number,
  scale: number,
): void {
  for (let tileY = 0; tileY < tilesY; tileY += 1) {
    for (let tileX = 0; tileX < tilesX; tileX += 1) {
      drawScaled(target, source, targetX + tileX * source.width * scale, targetY + tileY * source.height * scale, scale);
    }
  }
}

export function cubic(p0: number, p1: number, p2: number, p3: number, alpha: number): number {
  return p1 + 0.5 * alpha * (
    p2 - p0 +
    alpha * (2 * p0 - 5 * p1 + 4 * p2 - p3 + alpha * (3 * (p1 - p2) + p3 - p0))
  );
}

export function lerp(a: number, b: number, alpha: number): number {
  return a + (b - a) * alpha;
}

export function smoothStep(value: number): number {
  const alpha = clamp01(value);
  return alpha * alpha * (3 - 2 * alpha);
}

export function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value));
}

export function clampByte(value: number): number {
  return Math.max(0, Math.min(255, Math.round(value)));
}

export function drawGrid(target: RgbaImage, targetX: number, targetY: number, width: number, height: number, scale: number, color: Rgba): void {
  for (let x = 0; x <= width; x += 1) {
    drawRect(target, targetX + x * scale, targetY, 1, height * scale, color);
  }
  for (let y = 0; y <= height; y += 1) {
    drawRect(target, targetX, targetY + y * scale, width * scale, 1, color);
  }
}

export function drawCheckerboard(target: RgbaImage, x: number, y: number, width: number, height: number, cellSize: number): void {
  const light: Rgba = [74, 77, 75, 255];
  const dark: Rgba = [45, 47, 46, 255];
  for (let py = 0; py < height; py += 1) {
    for (let px = 0; px < width; px += 1) {
      const color = (Math.floor(px / cellSize) + Math.floor(py / cellSize)) % 2 === 0 ? light : dark;
      writePixel(target.data, target.width, x + px, y + py, color);
    }
  }
}

export function drawRect(target: RgbaImage, x: number, y: number, width: number, height: number, color: Rgba): void {
  const x0 = Math.max(0, x);
  const y0 = Math.max(0, y);
  const x1 = Math.min(target.width, x + width);
  const y1 = Math.min(target.height, y + height);
  for (let py = y0; py < y1; py += 1) {
    for (let px = x0; px < x1; px += 1) {
      blendPixel(target.data, target.width, px, py, color, 1);
    }
  }
}

export function readPixel(image: RgbaImage, x: number, y: number): Rgba {
  const index = (y * image.width + x) * 4;
  return [image.data[index]!, image.data[index + 1]!, image.data[index + 2]!, image.data[index + 3]!];
}

export function writePixel(data: Uint8Array, width: number, x: number, y: number, color: Rgba): void {
  const index = (y * width + x) * 4;
  data[index] = color[0];
  data[index + 1] = color[1];
  data[index + 2] = color[2];
  data[index + 3] = color[3];
}

export function hasTransparency(image: RgbaImage): boolean {
  for (let index = 3; index < image.data.length; index += 4) {
    if (image.data[index] !== 255) {
      return true;
    }
  }
  return false;
}

export function cloneImage(image: RgbaImage): RgbaImage {
  return {
    width: image.width,
    height: image.height,
    data: new Uint8Array(image.data),
  };
}

export function tintTexture(image: RgbaImage, tint: Rgba): RgbaImage {
  const out = cloneImage(image);
  for (let index = 0; index < out.data.length; index += 4) {
    out.data[index] = Math.round(out.data[index]! * tint[0] / 255);
    out.data[index + 1] = Math.round(out.data[index + 1]! * tint[1] / 255);
    out.data[index + 2] = Math.round(out.data[index + 2]! * tint[2] / 255);
  }
  return out;
}

export function compositeImages(base: RgbaImage, overlay: RgbaImage): RgbaImage {
  if (base.width !== overlay.width || base.height !== overlay.height) {
    throw new Error(`Cannot composite ${base.width}x${base.height} with ${overlay.width}x${overlay.height}`);
  }
  const out = cloneImage(base);
  for (let y = 0; y < overlay.height; y += 1) {
    for (let x = 0; x < overlay.width; x += 1) {
      blendPixel(out.data, out.width, x, y, readPixel(overlay, x, y), 1);
    }
  }
  return out;
}

function copyPixel(
  source: Uint8Array,
  sourceWidth: number,
  sourceX: number,
  sourceY: number,
  target: Uint8Array,
  targetWidth: number,
  targetX: number,
  targetY: number,
): void {
  const sourceIndex = (sourceY * sourceWidth + sourceX) * 4;
  const targetIndex = (targetY * targetWidth + targetX) * 4;
  target[targetIndex] = source[sourceIndex]!;
  target[targetIndex + 1] = source[sourceIndex + 1]!;
  target[targetIndex + 2] = source[sourceIndex + 2]!;
  target[targetIndex + 3] = source[sourceIndex + 3]!;
}

export function blendPixel(data: Uint8Array, width: number, x: number, y: number, color: Rgba, opacity: number): void {
  const index = (y * width + x) * 4;
  const sourceAlpha = (color[3] / 255) * opacity;
  const destAlpha = data[index + 3]! / 255;
  const outAlpha = sourceAlpha + destAlpha * (1 - sourceAlpha);
  if (outAlpha <= 0) {
    data[index] = 0;
    data[index + 1] = 0;
    data[index + 2] = 0;
    data[index + 3] = 0;
    return;
  }
  data[index] = Math.round((color[0] * sourceAlpha + data[index]! * destAlpha * (1 - sourceAlpha)) / outAlpha);
  data[index + 1] = Math.round((color[1] * sourceAlpha + data[index + 1]! * destAlpha * (1 - sourceAlpha)) / outAlpha);
  data[index + 2] = Math.round((color[2] * sourceAlpha + data[index + 2]! * destAlpha * (1 - sourceAlpha)) / outAlpha);
  data[index + 3] = clampByte(outAlpha * 255);
}

export function resolveColor(palette: PaletteSpec, color: string): Rgba {
  const value = palette[color] ?? color;
  if (!isHexColor(value)) {
    throw new Error(`Unknown palette color '${color}'`);
  }
  return parseHexColor(value);
}

export function parseHexColor(value: string): Rgba {
  return [
    Number.parseInt(value.slice(1, 3), 16),
    Number.parseInt(value.slice(3, 5), 16),
    Number.parseInt(value.slice(5, 7), 16),
    value.length === 9 ? Number.parseInt(value.slice(7, 9), 16) : 255,
  ];
}

export function random01(seed: string, x: number, y: number): number {
  let hash = 2166136261;
  for (let index = 0; index < seed.length; index += 1) {
    hash ^= seed.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  hash ^= Math.imul(x + 0x9e3779b9, 374761393);
  hash ^= Math.imul(y + 0x85ebca6b, 668265263);
  hash ^= hash >>> 13;
  hash = Math.imul(hash, 1274126177);
  hash ^= hash >>> 16;
  return (hash >>> 0) / 0x100000000;
}

export function wrap(value: number, size: number): number {
  return ((value % size) + size) % size;
}

export function shadeColor(color: Rgba, shade: number): Rgba {
  return [
    clampByte(color[0] * shade),
    clampByte(color[1] * shade),
    clampByte(color[2] * shade),
    color[3],
  ];
}

export function colorDistance(a: Rgba, b: Rgba): number {
  const dr = a[0] - b[0];
  const dg = a[1] - b[1];
  const db = a[2] - b[2];
  const da = a[3] - b[3];
  return Math.sqrt(dr * dr + dg * dg + db * db + da * da) / 510;
}
