import { isHexColor, type AsciiLayerSpec, type PaletteSpec, type TexturePackAsset, type TextureSpec } from "./dsl";
import type { RgbaImage } from "./png";

export interface RenderedTexture extends RgbaImage {
  name: string;
  exportPath: string;
}

type Rgba = [number, number, number, number];

export function renderAllTextures(pack: TexturePackAsset): RenderedTexture[] {
  return Object.entries(pack.textures).map(([name, texture]) => renderTexture(pack, name, texture));
}

export function renderTexture(pack: TexturePackAsset, name: string, texture: TextureSpec): RenderedTexture {
  const palette = pack.palettes[texture.palette];
  if (!palette) {
    throw new Error(`Texture '${name}' references missing palette '${texture.palette}'`);
  }
  const size = texture.size ?? pack.defaultSize;
  const data = new Uint8Array(size * size * 4);
  const baseColor = resolveColor(palette, texture.base);
  fill(data, baseColor);

  for (const layer of texture.layers ?? []) {
    if (layer.kind === "speckles") {
      renderSpeckles(data, size, palette, layer.seed, layer.density, layer.colors, layer.opacity ?? 1, layer.radius ?? 0);
    } else {
      renderAsciiLayer(data, size, palette, layer);
    }
  }

  return {
    name,
    exportPath: texture.exportPath,
    width: size,
    height: size,
    data,
  };
}

export function makeReviewSheet(texture: RenderedTexture): RgbaImage {
  const background: Rgba = [32, 34, 34, 255];
  const panel: Rgba = [52, 54, 54, 255];
  const sheetWidth = 1040;
  const sheetHeight = 672;
  const sheet = solidImage(sheetWidth, sheetHeight, background);

  drawRect(sheet, 16, 16, 264, 264, panel);
  drawScaled(sheet, texture, 20, 20, 8);
  drawGrid(sheet, 20, 20, texture.width, texture.height, 8, [86, 89, 88, 255]);

  drawRect(sheet, 304, 16, 296, 296, panel);
  drawTiledScaled(sheet, texture, 308, 20, 3, 3, 3);

  const downsampled = downsampleNearest(texture, 16, 16);
  drawRect(sheet, 624, 16, 200, 200, panel);
  drawScaled(sheet, downsampled, 628, 20, 12);
  drawGrid(sheet, 628, 20, downsampled.width, downsampled.height, 12, [86, 89, 88, 255]);

  drawRect(sheet, 624, 232, 200, 80, panel);
  drawMipStrip(sheet, texture, 632, 240);

  drawRect(sheet, 840, 16, 184, 296, panel);
  drawIsometricCube(sheet, texture, 868, 48);

  drawRect(sheet, 16, 328, 328, 328, panel);
  drawRotatedTiledScaled(sheet, texture, 20, 332, 5, 5, 2, "vanilla-dirt-rotation-preview");

  return sheet;
}

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

function renderSpeckles(
  data: Uint8Array,
  size: number,
  palette: PaletteSpec,
  seed: string,
  density: number,
  colors: string[],
  opacity: number,
  radius: number,
): void {
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const value = random01(seed, x, y);
      if (value >= density) {
        continue;
      }
      const colorName = colors[Math.floor(random01(`${seed}:color`, x, y) * colors.length)] ?? colors[0]!;
      const color = resolveColor(palette, colorName);
      for (let dy = -radius; dy <= radius; dy += 1) {
        for (let dx = -radius; dx <= radius; dx += 1) {
          if (Math.abs(dx) + Math.abs(dy) > radius) {
            continue;
          }
          blendPixel(data, size, wrap(x + dx, size), wrap(y + dy, size), color, opacity);
        }
      }
    }
  }
}

function renderAsciiLayer(data: Uint8Array, size: number, palette: PaletteSpec, layer: AsciiLayerSpec): void {
  const skip = layer.skip ?? ".";
  const opacity = layer.opacity ?? 1;
  for (let y = 0; y < size; y += 1) {
    const row = layer.pixels[y] ?? "";
    for (let x = 0; x < size; x += 1) {
      const symbol = row[x] ?? skip;
      if (symbol === skip) {
        continue;
      }
      const colorName = layer.colors[symbol];
      if (!colorName) {
        continue;
      }
      blendPixel(data, size, x, y, resolveColor(palette, colorName), opacity);
    }
  }
}

function fill(data: Uint8Array, color: Rgba): void {
  for (let index = 0; index < data.length; index += 4) {
    data[index] = color[0];
    data[index + 1] = color[1];
    data[index + 2] = color[2];
    data[index + 3] = color[3];
  }
}

function solidImage(width: number, height: number, color: Rgba): RgbaImage {
  const data = new Uint8Array(width * height * 4);
  fill(data, color);
  return { width, height, data };
}

function drawScaled(target: RgbaImage, source: RgbaImage, targetX: number, targetY: number, scale: number): void {
  for (let y = 0; y < source.height; y += 1) {
    for (let x = 0; x < source.width; x += 1) {
      const color = readPixel(source, x, y);
      drawRect(target, targetX + x * scale, targetY + y * scale, scale, scale, color);
    }
  }
}

function drawTiledScaled(
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

function drawRotatedTiledScaled(
  target: RgbaImage,
  source: RgbaImage,
  targetX: number,
  targetY: number,
  tilesX: number,
  tilesY: number,
  scale: number,
  seed: string,
): void {
  for (let tileY = 0; tileY < tilesY; tileY += 1) {
    for (let tileX = 0; tileX < tilesX; tileX += 1) {
      const rotation = Math.floor(random01(seed, tileX, tileY) * 4) % 4;
      drawRotatedScaled(
        target,
        source,
        targetX + tileX * source.width * scale,
        targetY + tileY * source.height * scale,
        scale,
        rotation,
      );
    }
  }
}

function drawRotatedScaled(
  target: RgbaImage,
  source: RgbaImage,
  targetX: number,
  targetY: number,
  scale: number,
  rotation: number,
): void {
  for (let y = 0; y < source.height; y += 1) {
    for (let x = 0; x < source.width; x += 1) {
      const [sourceX, sourceY] = rotatedSourceCoordinate(source.width, source.height, x, y, rotation);
      const color = readPixel(source, sourceX, sourceY);
      drawRect(target, targetX + x * scale, targetY + y * scale, scale, scale, color);
    }
  }
}

function rotatedSourceCoordinate(width: number, height: number, x: number, y: number, rotation: number): [number, number] {
  if (rotation === 1) {
    return [y, height - 1 - x];
  }
  if (rotation === 2) {
    return [width - 1 - x, height - 1 - y];
  }
  if (rotation === 3) {
    return [width - 1 - y, x];
  }
  return [x, y];
}

function drawMipStrip(target: RgbaImage, source: RgbaImage, targetX: number, targetY: number): void {
  const levels = [
    { size: 16, scale: 3 },
    { size: 8, scale: 4 },
    { size: 4, scale: 6 },
    { size: 2, scale: 10 },
    { size: 1, scale: 16 },
  ];
  let x = targetX;
  for (const level of levels) {
    const image = downsampleNearest(source, level.size, level.size);
    drawScaled(target, image, x, targetY, level.scale);
    x += level.size * level.scale + 12;
  }
}

function drawIsometricCube(target: RgbaImage, texture: RgbaImage, targetX: number, targetY: number): void {
  const halfWidth = 64;
  const halfDepth = 32;
  const height = 84;
  const originX = targetX + halfWidth;
  const originY = targetY + height;

  drawCubeFace(target, texture, 0.68, (u, v) => projectIso(originX, originY, halfWidth, halfDepth, height, u, 1 - v, 1));
  drawCubeFace(target, texture, 0.78, (u, v) => projectIso(originX, originY, halfWidth, halfDepth, height, 1, 1 - v, u));
  drawCubeFace(target, texture, 1.08, (u, v) => projectIso(originX, originY, halfWidth, halfDepth, height, u, 1, v));
  drawCubeOutline(target, [
    projectIso(originX, originY, halfWidth, halfDepth, height, 0, 1, 0),
    projectIso(originX, originY, halfWidth, halfDepth, height, 1, 1, 0),
    projectIso(originX, originY, halfWidth, halfDepth, height, 1, 1, 1),
    projectIso(originX, originY, halfWidth, halfDepth, height, 0, 1, 1),
    projectIso(originX, originY, halfWidth, halfDepth, height, 0, 0, 1),
    projectIso(originX, originY, halfWidth, halfDepth, height, 1, 0, 1),
    projectIso(originX, originY, halfWidth, halfDepth, height, 1, 0, 0),
  ]);
}

function drawCubeFace(
  target: RgbaImage,
  texture: RgbaImage,
  shade: number,
  map: (u: number, v: number) => Point,
): void {
  for (let y = 0; y < texture.height; y += 1) {
    for (let x = 0; x < texture.width; x += 1) {
      const u0 = x / texture.width;
      const v0 = y / texture.height;
      const u1 = (x + 1) / texture.width;
      const v1 = (y + 1) / texture.height;
      fillPolygon(
        target,
        [map(u0, v0), map(u1, v0), map(u1, v1), map(u0, v1)],
        shadeColor(readPixel(texture, x, y), shade),
      );
    }
  }
}

function projectIso(
  originX: number,
  originY: number,
  halfWidth: number,
  halfDepth: number,
  height: number,
  x: number,
  y: number,
  z: number,
): Point {
  return {
    x: originX + (x - z) * halfWidth,
    y: originY + (x + z) * halfDepth - y * height,
  };
}

function drawCubeOutline(target: RgbaImage, points: Point[]): void {
  const outline: Rgba = [24, 22, 20, 255];
  const [topFront, topRight, topBack, topLeft, bottomLeft, bottomBack, bottomRight] = points;
  drawLine(target, topFront!, topRight!, outline);
  drawLine(target, topRight!, topBack!, outline);
  drawLine(target, topBack!, topLeft!, outline);
  drawLine(target, topLeft!, topFront!, outline);
  drawLine(target, topLeft!, bottomLeft!, outline);
  drawLine(target, topBack!, bottomBack!, outline);
  drawLine(target, topRight!, bottomRight!, outline);
  drawLine(target, bottomLeft!, bottomBack!, outline);
  drawLine(target, bottomBack!, bottomRight!, outline);
}

interface Point {
  x: number;
  y: number;
}

function fillPolygon(target: RgbaImage, polygon: Point[], color: Rgba): void {
  const minX = Math.max(0, Math.floor(Math.min(...polygon.map((point) => point.x))));
  const maxX = Math.min(target.width - 1, Math.ceil(Math.max(...polygon.map((point) => point.x))));
  const minY = Math.max(0, Math.floor(Math.min(...polygon.map((point) => point.y))));
  const maxY = Math.min(target.height - 1, Math.ceil(Math.max(...polygon.map((point) => point.y))));

  for (let y = minY; y <= maxY; y += 1) {
    for (let x = minX; x <= maxX; x += 1) {
      if (pointInPolygon(x + 0.5, y + 0.5, polygon)) {
        writePixel(target.data, target.width, x, y, color);
      }
    }
  }
}

function pointInPolygon(x: number, y: number, polygon: Point[]): boolean {
  let inside = false;
  for (let current = 0, previous = polygon.length - 1; current < polygon.length; previous = current, current += 1) {
    const currentPoint = polygon[current]!;
    const previousPoint = polygon[previous]!;
    const crosses = currentPoint.y > y !== previousPoint.y > y;
    if (!crosses) {
      continue;
    }
    const intersectionX =
      ((previousPoint.x - currentPoint.x) * (y - currentPoint.y)) / (previousPoint.y - currentPoint.y) +
      currentPoint.x;
    if (x < intersectionX) {
      inside = !inside;
    }
  }
  return inside;
}

function drawLine(target: RgbaImage, from: Point, to: Point, color: Rgba): void {
  const steps = Math.max(Math.abs(to.x - from.x), Math.abs(to.y - from.y));
  for (let step = 0; step <= steps; step += 1) {
    const alpha = steps === 0 ? 0 : step / steps;
    const x = Math.round(from.x + (to.x - from.x) * alpha);
    const y = Math.round(from.y + (to.y - from.y) * alpha);
    if (x >= 0 && x < target.width && y >= 0 && y < target.height) {
      writePixel(target.data, target.width, x, y, color);
    }
  }
}

function shadeColor(color: Rgba, shade: number): Rgba {
  return [
    clampByte(color[0] * shade),
    clampByte(color[1] * shade),
    clampByte(color[2] * shade),
    color[3],
  ];
}

function clampByte(value: number): number {
  return Math.max(0, Math.min(255, Math.round(value)));
}

function drawGrid(target: RgbaImage, targetX: number, targetY: number, width: number, height: number, scale: number, color: Rgba): void {
  for (let x = 0; x <= width; x += 1) {
    drawRect(target, targetX + x * scale, targetY, 1, height * scale, color);
  }
  for (let y = 0; y <= height; y += 1) {
    drawRect(target, targetX, targetY + y * scale, width * scale, 1, color);
  }
}

function drawRect(target: RgbaImage, x: number, y: number, width: number, height: number, color: Rgba): void {
  const x0 = Math.max(0, x);
  const y0 = Math.max(0, y);
  const x1 = Math.min(target.width, x + width);
  const y1 = Math.min(target.height, y + height);
  for (let py = y0; py < y1; py += 1) {
    for (let px = x0; px < x1; px += 1) {
      writePixel(target.data, target.width, px, py, color);
    }
  }
}

function readPixel(image: RgbaImage, x: number, y: number): Rgba {
  const index = (y * image.width + x) * 4;
  return [image.data[index]!, image.data[index + 1]!, image.data[index + 2]!, image.data[index + 3]!];
}

function writePixel(data: Uint8Array, width: number, x: number, y: number, color: Rgba): void {
  const index = (y * width + x) * 4;
  data[index] = color[0];
  data[index + 1] = color[1];
  data[index + 2] = color[2];
  data[index + 3] = color[3];
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

function blendPixel(data: Uint8Array, width: number, x: number, y: number, color: Rgba, opacity: number): void {
  const index = (y * width + x) * 4;
  const alpha = (color[3] / 255) * opacity;
  data[index] = Math.round(data[index]! * (1 - alpha) + color[0] * alpha);
  data[index + 1] = Math.round(data[index + 1]! * (1 - alpha) + color[1] * alpha);
  data[index + 2] = Math.round(data[index + 2]! * (1 - alpha) + color[2] * alpha);
  data[index + 3] = 255;
}

function resolveColor(palette: PaletteSpec, color: string): Rgba {
  const value = palette[color] ?? color;
  if (!isHexColor(value)) {
    throw new Error(`Unknown palette color '${color}'`);
  }
  return parseHexColor(value);
}

function parseHexColor(value: string): Rgba {
  return [
    Number.parseInt(value.slice(1, 3), 16),
    Number.parseInt(value.slice(3, 5), 16),
    Number.parseInt(value.slice(5, 7), 16),
    value.length === 9 ? Number.parseInt(value.slice(7, 9), 16) : 255,
  ];
}

function random01(seed: string, x: number, y: number): number {
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

function wrap(value: number, size: number): number {
  return ((value % size) + size) % size;
}
