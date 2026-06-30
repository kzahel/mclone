import {
  isHexColor,
  type AsciiLayerSpec,
  type BlockSpec,
  type MacroNoiseLayerSpec,
  type MaskLayerSpec,
  type PaletteSpec,
  type TextureSourceCategory,
  type TexturePackAsset,
  type TextureSpec,
  type TintSpec,
} from "./dsl";
import type { RgbaImage } from "./png";

export interface RenderedTexture extends RgbaImage {
  name: string;
  exportPath: string;
  source: TextureSourceCategory;
  tintRole?: string;
  tint?: TintSpec;
  preview: TextureSpec["preview"];
}

type Rgba = [number, number, number, number];
type TilingMode = "xy" | "x" | "none";

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
    } else if (layer.kind === "ascii") {
      renderAsciiLayer(data, size, palette, layer);
    } else if (layer.kind === "mask") {
      renderMaskLayer(data, size, palette, layer);
    } else {
      renderMacroNoiseLayer(data, size, palette, layer);
    }
  }

  const rendered: RenderedTexture = {
    name,
    exportPath: texture.exportPath,
    source: texture.source ?? "final-color",
    width: size,
    height: size,
    data,
    preview: texture.preview,
  };
  if (texture.tintRole) {
    rendered.tintRole = texture.tintRole;
    rendered.tint = pack.tints[texture.tintRole]!;
  }

  return rendered;
}

export interface ReviewSheetOptions {
  reference?: RgbaImage;
}

export function makeReviewSheet(texture: RenderedTexture, options: ReviewSheetOptions = {}): RgbaImage {
  const background: Rgba = [32, 34, 34, 255];
  const panel: Rgba = [52, 54, 54, 255];
  const sheetWidth = 1040;
  const sheetHeight = 672;
  const sheet = solidImage(sheetWidth, sheetHeight, background);
  const rawTexture = texture;
  const displayTexture = previewTexture(texture);
  const rawCheckerboard = texture.preview?.checkerboard ?? hasTransparency(rawTexture);
  const displayCheckerboard = texture.preview?.checkerboard ?? hasTransparency(displayTexture);
  const showCube = texture.preview?.cube ?? !hasTransparency(displayTexture);
  const showRotation = texture.preview?.rotation ?? !hasTransparency(displayTexture);
  const tiling = texture.preview?.tiling ?? "xy";

  drawRect(sheet, 16, 16, 264, 264, panel);
  if (rawCheckerboard) {
    drawCheckerboard(sheet, 20, 20, rawTexture.width * 8, rawTexture.height * 8, 8);
  }
  drawScaled(sheet, rawTexture, 20, 20, 8);
  drawGrid(sheet, 20, 20, rawTexture.width, rawTexture.height, 8, [86, 89, 88, 255]);

  drawRect(sheet, 304, 16, 296, 296, panel);
  if (displayCheckerboard && tiling !== "none") {
    drawCheckerboard(sheet, 308, 20, displayTexture.width * 3 * 3, displayTexture.height * 3 * 3, 12);
  }
  if (tiling === "xy") {
    drawTiledScaled(sheet, displayTexture, 308, 20, 3, 3, 3);
  } else if (tiling === "x") {
    if (displayCheckerboard) {
      drawCheckerboard(sheet, 308, 20, displayTexture.width * 3 * 3, displayTexture.height * 3, 12);
    }
    drawTiledScaled(sheet, displayTexture, 308, 20, 3, 1, 3);
  } else {
    if (displayCheckerboard) {
      drawCheckerboard(sheet, 308, 20, displayTexture.width * 3, displayTexture.height * 3, 12);
    }
    drawScaled(sheet, displayTexture, 308, 20, 3);
  }

  const downsampled = downsampleNearest(displayTexture, 16, 16);
  drawRect(sheet, 624, 16, 200, 200, panel);
  if (displayCheckerboard) {
    drawCheckerboard(sheet, 628, 20, downsampled.width * 12, downsampled.height * 12, 12);
  }
  drawScaled(sheet, downsampled, 628, 20, 12);
  drawGrid(sheet, 628, 20, downsampled.width, downsampled.height, 12, [86, 89, 88, 255]);

  drawRect(sheet, 624, 232, 200, 80, panel);
  drawMipStrip(sheet, displayTexture, 632, 240, displayCheckerboard);

  if (showCube) {
    drawRect(sheet, 840, 16, 184, 296, panel);
    drawIsometricCube(sheet, displayTexture, 868, 48);
  }

  if (showRotation) {
    drawRect(sheet, 16, 328, 328, 328, panel);
    drawRotatedTiledScaled(sheet, displayTexture, 20, 332, 5, 5, 2, `${texture.name}:rotation-preview`);
  }

  if (tiling !== "none") {
    drawRect(sheet, 360, 328, 328, 328, panel);
    drawSeamDiagnostic(sheet, displayTexture, 360, 328, 328, 328, tiling, displayCheckerboard);
  }

  if (options.reference) {
    drawReferencePanel(sheet, options.reference, 704, 328, 320, 328);
  }

  return sheet;
}

export function makeBlockReviewSheet(
  blockName: string,
  block: BlockSpec,
  texturesByName: Map<string, RenderedTexture>,
): RgbaImage {
  const background: Rgba = [32, 34, 34, 255];
  const panel: Rgba = [52, 54, 54, 255];
  const sheet = solidImage(1040, 672, background);
  const tintColors = tintColorsForBlock(block, texturesByName);
  const primaryTint = tintColors[0]!;

  drawRect(sheet, 16, 16, 300, 300, panel);
  drawIsometricCubeFaces(sheet, cubeFaces(block, texturesByName, primaryTint), 82, 58);

  const top = textureForFace(block, texturesByName, "top");
  drawRect(sheet, 340, 16, 328, 328, panel);
  drawRotatedTiledScaled(sheet, applyRoleTint(top, primaryTint), 344, 20, 5, 5, 2, `${blockName}:top-rotation`);

  const side = compositeSideTexture(block, texturesByName, primaryTint);
  drawRect(sheet, 692, 16, 328, 328, panel);
  drawTiledScaled(sheet, side, 696, 20, 3, 3, 3);

  drawRect(sheet, 16, 360, 1008, 280, panel);
  drawTerrainPatch(sheet, block, texturesByName, primaryTint, 32, 372);
  drawIsometricCubeFaces(sheet, cubeFaces(block, texturesByName, primaryTint), 760, 410);
  for (const [index, tint] of tintColors.slice(0, 4).entries()) {
    drawTintSwatch(sheet, 890, 412 + index * 34, tint);
  }

  return sheet;
}

export function makeBlockSideReviewSheet(
  blockName: string,
  block: BlockSpec,
  texturesByName: Map<string, RenderedTexture>,
): RgbaImage | undefined {
  if (!block.faces.side || !block.faces.overlay) {
    return undefined;
  }

  const background: Rgba = [32, 34, 34, 255];
  const panel: Rgba = [52, 54, 54, 255];
  const sheet = solidImage(1040, 432, background);
  const tintColors = tintColorsForBlock(block, texturesByName);
  const primaryTint = tintColors[0]!;
  const side = textureForFace(block, texturesByName, "side");
  const overlay = texturesByName.get(block.faces.overlay);
  if (!overlay) {
    throw new Error(`Block '${blockName}' overlay references missing rendered texture '${block.faces.overlay}'`);
  }
  const composedSide = compositeImages(cloneImage(side), applyRoleTint(overlay, primaryTint));
  const top = applyRoleTint(textureForFace(block, texturesByName, "top"), primaryTint);

  drawRect(sheet, 16, 16, 264, 264, panel);
  drawScaled(sheet, side, 20, 20, 8);
  drawGrid(sheet, 20, 20, side.width, side.height, 8, [86, 89, 88, 255]);

  drawRect(sheet, 304, 16, 264, 264, panel);
  drawCheckerboard(sheet, 308, 20, overlay.width * 8, overlay.height * 8, 8);
  drawScaled(sheet, overlay, 308, 20, 8);
  drawGrid(sheet, 308, 20, overlay.width, overlay.height, 8, [86, 89, 88, 255]);

  drawRect(sheet, 592, 16, 432, 128, panel);
  drawTiledScaled(sheet, composedSide, 608, 32, 4, 1, 3);

  drawRect(sheet, 592, 160, 432, 248, panel);
  drawScaled(sheet, top, 608, 176, 3);
  drawGrid(sheet, 608, 176, top.width, top.height, 3, [86, 89, 88, 255]);
  drawScaled(sheet, composedSide, 728, 176, 3);
  drawGrid(sheet, 728, 176, composedSide.width, composedSide.height, 3, [86, 89, 88, 255]);
  drawIsometricCubeFaces(sheet, cubeFaces(block, texturesByName, primaryTint), 864, 216);
  for (const [index, tint] of tintColors.slice(0, 4).entries()) {
    drawTintSwatch(sheet, 608, 296 + index * 26, tint);
  }

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

function renderMaskLayer(data: Uint8Array, size: number, palette: PaletteSpec, layer: MaskLayerSpec): void {
  const opacity = layer.opacity ?? 1;
  const upscale = layer.upscale ?? "nearest";
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const color = sampleMaskLayer(palette, layer, x, y, size, upscale);
      if (color[3] === 0) {
        continue;
      }
      blendPixel(data, size, x, y, color, opacity);
    }
  }
}

function renderMacroNoiseLayer(
  data: Uint8Array,
  size: number,
  palette: PaletteSpec,
  layer: MacroNoiseLayerSpec,
): void {
  const opacity = layer.opacity ?? 1;
  const octaves = layer.octaves ?? 1;
  const contrast = layer.contrast ?? 1;
  const bias = layer.bias ?? 0;
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const noise = periodicValueNoise(layer.seed, x, y, size, layer.frequency, octaves);
      const value = clamp01((noise - 0.5) * contrast + 0.5 + bias);
      blendPixel(data, size, x, y, colorRamp(palette, layer.colors, value), opacity);
    }
  }
}

function sampleMaskLayer(
  palette: PaletteSpec,
  layer: MaskLayerSpec,
  x: number,
  y: number,
  size: number,
  upscale: NonNullable<MaskLayerSpec["upscale"]>,
): Rgba {
  const maskHeight = layer.pixels.length;
  const maskWidth = layer.pixels[0]?.length ?? 0;
  if (maskWidth === 0 || maskHeight === 0) {
    return [0, 0, 0, 0];
  }

  if (upscale === "nearest") {
    const maskX = Math.min(maskWidth - 1, Math.floor(x * maskWidth / size));
    const maskY = Math.min(maskHeight - 1, Math.floor(y * maskHeight / size));
    return maskCellColor(palette, layer, maskX, maskY);
  }

  const u = x * maskWidth / size;
  const v = y * maskHeight / size;
  if (upscale === "bicubic") {
    return unpremultiply(cubicMaskSample(palette, layer, u, v));
  }

  const x0 = Math.floor(u);
  const y0 = Math.floor(v);
  const tx = upscale === "smooth" ? smoothStep(u - x0) : u - x0;
  const ty = upscale === "smooth" ? smoothStep(v - y0) : v - y0;
  const sample = lerpPremul(
    lerpPremul(
      premultiply(maskCellColor(palette, layer, x0, y0)),
      premultiply(maskCellColor(palette, layer, x0 + 1, y0)),
      tx,
    ),
    lerpPremul(
      premultiply(maskCellColor(palette, layer, x0, y0 + 1)),
      premultiply(maskCellColor(palette, layer, x0 + 1, y0 + 1)),
      tx,
    ),
    ty,
  );
  if (upscale === "smooth") {
    sample[3] = smoothStep(sample[3]);
  }
  return unpremultiply(sample);
}

function maskCellColor(palette: PaletteSpec, layer: MaskLayerSpec, x: number, y: number): Rgba {
  const skip = layer.skip ?? ".";
  const maskHeight = layer.pixels.length;
  const maskWidth = layer.pixels[0]?.length ?? 0;
  if (maskWidth === 0 || maskHeight === 0) {
    return [0, 0, 0, 0];
  }
  const symbol = layer.pixels[wrap(y, maskHeight)]?.[wrap(x, maskWidth)] ?? skip;
  if (symbol === skip) {
    return [0, 0, 0, 0];
  }
  const colorName = layer.colors[symbol];
  return colorName ? resolveColor(palette, colorName) : [0, 0, 0, 0];
}

function periodicValueNoise(seed: string, x: number, y: number, size: number, frequency: number, octaves: number): number {
  let value = 0;
  let amplitude = 1;
  let amplitudeTotal = 0;
  for (let octave = 0; octave < octaves; octave += 1) {
    value += samplePeriodicValueNoise(`${seed}:octave-${octave}`, x, y, size, frequency * 2 ** octave) * amplitude;
    amplitudeTotal += amplitude;
    amplitude *= 0.5;
  }
  return amplitudeTotal > 0 ? value / amplitudeTotal : 0;
}

function samplePeriodicValueNoise(seed: string, x: number, y: number, size: number, frequency: number): number {
  const coordinateScale = size <= 1 ? 0 : 1 / (size - 1);
  const u = x * coordinateScale * frequency;
  const v = y * coordinateScale * frequency;
  const x0 = Math.floor(u);
  const y0 = Math.floor(v);
  const tx = smoothStep(u - x0);
  const ty = smoothStep(v - y0);
  const top = lerp(
    periodicGridValue(seed, x0, y0, frequency),
    periodicGridValue(seed, x0 + 1, y0, frequency),
    tx,
  );
  const bottom = lerp(
    periodicGridValue(seed, x0, y0 + 1, frequency),
    periodicGridValue(seed, x0 + 1, y0 + 1, frequency),
    tx,
  );
  return lerp(top, bottom, ty);
}

function periodicGridValue(seed: string, x: number, y: number, frequency: number): number {
  return random01(seed, wrap(x, frequency), wrap(y, frequency));
}

function colorRamp(palette: PaletteSpec, colors: string[], value: number): Rgba {
  if (colors.length === 1) {
    return resolveColor(palette, colors[0]!);
  }
  const position = clamp01(value) * (colors.length - 1);
  const lowIndex = Math.floor(position);
  const highIndex = Math.min(colors.length - 1, lowIndex + 1);
  const alpha = position - lowIndex;
  return lerpRgba(resolveColor(palette, colors[lowIndex]!), resolveColor(palette, colors[highIndex]!), alpha);
}

type PremultipliedRgba = [number, number, number, number];

function cubicMaskSample(palette: PaletteSpec, layer: MaskLayerSpec, u: number, v: number): PremultipliedRgba {
  const x0 = Math.floor(u);
  const y0 = Math.floor(v);
  const tx = u - x0;
  const ty = v - y0;
  const rows: PremultipliedRgba[] = [];
  for (let row = -1; row <= 2; row += 1) {
    const samples: PremultipliedRgba[] = [];
    for (let column = -1; column <= 2; column += 1) {
      samples.push(premultiply(maskCellColor(palette, layer, x0 + column, y0 + row)));
    }
    rows.push(cubicPremul(samples[0]!, samples[1]!, samples[2]!, samples[3]!, tx));
  }
  return cubicPremul(rows[0]!, rows[1]!, rows[2]!, rows[3]!, ty);
}

function premultiply(color: Rgba): PremultipliedRgba {
  const alpha = color[3] / 255;
  return [color[0] * alpha, color[1] * alpha, color[2] * alpha, alpha];
}

function unpremultiply(color: PremultipliedRgba): Rgba {
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

function lerpPremul(a: PremultipliedRgba, b: PremultipliedRgba, alpha: number): PremultipliedRgba {
  return [
    lerp(a[0], b[0], alpha),
    lerp(a[1], b[1], alpha),
    lerp(a[2], b[2], alpha),
    lerp(a[3], b[3], alpha),
  ];
}

function cubicPremul(
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

function lerpRgba(a: Rgba, b: Rgba, alpha: number): Rgba {
  return [
    clampByte(lerp(a[0], b[0], alpha)),
    clampByte(lerp(a[1], b[1], alpha)),
    clampByte(lerp(a[2], b[2], alpha)),
    clampByte(lerp(a[3], b[3], alpha)),
  ];
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

function drawMipStrip(target: RgbaImage, source: RgbaImage, targetX: number, targetY: number, checkerboard: boolean): void {
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
    if (checkerboard) {
      drawCheckerboard(target, x, targetY, level.size * level.scale, level.size * level.scale, level.scale);
    }
    drawScaled(target, image, x, targetY, level.scale);
    x += level.size * level.scale + 12;
  }
}

function drawSeamDiagnostic(
  target: RgbaImage,
  source: RgbaImage,
  panelX: number,
  panelY: number,
  panelWidth: number,
  panelHeight: number,
  tiling: TilingMode,
  checkerboard: boolean,
): void {
  const tilesX = 2;
  const tilesY = tiling === "xy" ? 2 : 1;
  const padding = 28;
  const scale = Math.max(
    1,
    Math.floor(
      Math.min(
        (panelWidth - padding * 2) / (source.width * tilesX),
        (panelHeight - padding * 2) / (source.height * tilesY),
      ),
    ),
  );
  const previewWidth = source.width * tilesX * scale;
  const previewHeight = source.height * tilesY * scale;
  const previewX = panelX + Math.floor((panelWidth - previewWidth) / 2);
  const previewY = panelY + Math.floor((panelHeight - previewHeight) / 2);

  if (checkerboard) {
    drawCheckerboard(target, previewX, previewY, previewWidth, previewHeight, Math.max(4, scale * 2));
  }
  drawTiledScaled(target, source, previewX, previewY, tilesX, tilesY, scale);

  const seamWidth = Math.max(2, Math.floor(scale / 2));
  const verticalSeamX = previewX + source.width * scale - Math.floor(seamWidth / 2);
  drawVerticalSeamError(target, source, verticalSeamX, previewY, tilesY, scale, seamWidth);

  if (tiling === "xy") {
    const horizontalSeamY = previewY + source.height * scale - Math.floor(seamWidth / 2);
    drawHorizontalSeamError(target, source, previewX, horizontalSeamY, tilesX, scale, seamWidth);
  }
}

function drawReferencePanel(
  target: RgbaImage,
  reference: RgbaImage,
  panelX: number,
  panelY: number,
  panelWidth: number,
  panelHeight: number,
): void {
  const panel: Rgba = [52, 54, 54, 255];
  const border: Rgba = [116, 119, 116, 255];
  drawRect(target, panelX, panelY, panelWidth, panelHeight, panel);
  drawRect(target, panelX, panelY, panelWidth, 2, border);
  drawRect(target, panelX, panelY + panelHeight - 2, panelWidth, 2, border);
  drawRect(target, panelX, panelY, 2, panelHeight, border);
  drawRect(target, panelX + panelWidth - 2, panelY, 2, panelHeight, border);
  drawRect(target, panelX + 2, panelY + 2, panelWidth - 4, 22, [42, 44, 44, 255]);
  drawPixelText(target, "MINECRAFT REFERENCE", panelX + 18, panelY + 8, 2, [214, 218, 210, 255]);

  const checkerboard = hasTransparency(reference);
  const enlargedScale = Math.max(1, Math.floor(Math.min(128 / reference.width, 128 / reference.height)));
  const enlargedWidth = reference.width * enlargedScale;
  const enlargedHeight = reference.height * enlargedScale;
  const enlargedX = panelX + 18;
  const enlargedY = panelY + 42;
  if (checkerboard) {
    drawCheckerboard(target, enlargedX, enlargedY, enlargedWidth, enlargedHeight, Math.max(4, enlargedScale));
  }
  drawScaled(target, reference, enlargedX, enlargedY, enlargedScale);
  drawGrid(target, enlargedX, enlargedY, reference.width, reference.height, enlargedScale, [86, 89, 88, 255]);

  const repeatScale = Math.max(1, Math.floor(Math.min(144 / (reference.width * 3), 144 / (reference.height * 3))));
  const repeatWidth = reference.width * 3 * repeatScale;
  const repeatHeight = reference.height * 3 * repeatScale;
  const repeatX = panelX + panelWidth - repeatWidth - 18;
  const repeatY = panelY + 42;
  if (checkerboard) {
    drawCheckerboard(target, repeatX, repeatY, repeatWidth, repeatHeight, Math.max(4, repeatScale * 2));
  }
  drawTiledScaled(target, reference, repeatX, repeatY, 3, 3, repeatScale);

  const mipY = panelY + Math.max(enlargedHeight, repeatHeight) + 40;
  if (mipY + 64 < panelY + panelHeight) {
    drawMipStrip(target, reference, panelX + 18, mipY, checkerboard);
  }
}

function drawPixelText(target: RgbaImage, text: string, x: number, y: number, scale: number, color: Rgba): void {
  let cursorX = x;
  for (const character of text.toUpperCase()) {
    const glyph = PIXEL_FONT[character] ?? PIXEL_FONT[" "];
    if (!glyph) {
      cursorX += 6 * scale;
      continue;
    }
    for (let gy = 0; gy < glyph.length; gy += 1) {
      const row = glyph[gy]!;
      for (let gx = 0; gx < row.length; gx += 1) {
        if (row[gx] === "1") {
          drawRect(target, cursorX + gx * scale, y + gy * scale, scale, scale, color);
        }
      }
    }
    cursorX += 6 * scale;
  }
}

const PIXEL_FONT: Record<string, string[]> = {
  " ": [
    "00000",
    "00000",
    "00000",
    "00000",
    "00000",
    "00000",
    "00000",
  ],
  A: [
    "01110",
    "10001",
    "10001",
    "11111",
    "10001",
    "10001",
    "10001",
  ],
  C: [
    "01111",
    "10000",
    "10000",
    "10000",
    "10000",
    "10000",
    "01111",
  ],
  E: [
    "11111",
    "10000",
    "10000",
    "11110",
    "10000",
    "10000",
    "11111",
  ],
  F: [
    "11111",
    "10000",
    "10000",
    "11110",
    "10000",
    "10000",
    "10000",
  ],
  I: [
    "11111",
    "00100",
    "00100",
    "00100",
    "00100",
    "00100",
    "11111",
  ],
  M: [
    "10001",
    "11011",
    "10101",
    "10101",
    "10001",
    "10001",
    "10001",
  ],
  N: [
    "10001",
    "11001",
    "10101",
    "10011",
    "10001",
    "10001",
    "10001",
  ],
  R: [
    "11110",
    "10001",
    "10001",
    "11110",
    "10100",
    "10010",
    "10001",
  ],
  T: [
    "11111",
    "00100",
    "00100",
    "00100",
    "00100",
    "00100",
    "00100",
  ],
};

function drawVerticalSeamError(
  target: RgbaImage,
  source: RgbaImage,
  targetX: number,
  targetY: number,
  tilesY: number,
  scale: number,
  seamWidth: number,
): void {
  for (let y = 0; y < source.height; y += 1) {
    const color = seamErrorColor(readPixel(source, source.width - 1, y), readPixel(source, 0, y), "x");
    if (color[3] === 0) {
      continue;
    }
    for (let tileY = 0; tileY < tilesY; tileY += 1) {
      drawRect(target, targetX, targetY + (tileY * source.height + y) * scale, seamWidth, scale, color);
    }
  }
}

function drawHorizontalSeamError(
  target: RgbaImage,
  source: RgbaImage,
  targetX: number,
  targetY: number,
  tilesX: number,
  scale: number,
  seamWidth: number,
): void {
  for (let x = 0; x < source.width; x += 1) {
    const color = seamErrorColor(readPixel(source, x, source.height - 1), readPixel(source, x, 0), "y");
    if (color[3] === 0) {
      continue;
    }
    for (let tileX = 0; tileX < tilesX; tileX += 1) {
      drawRect(target, targetX + (tileX * source.width + x) * scale, targetY, scale, seamWidth, color);
    }
  }
}

function seamErrorColor(a: Rgba, b: Rgba, axis: "x" | "y"): Rgba {
  const diff = colorDistance(a, b);
  if (diff < 0.04) {
    return [0, 0, 0, 0];
  }
  const alpha = clampByte(Math.min(0.9, diff * 1.8) * 255);
  return axis === "x" ? [255, 56, 24, alpha] : [24, 148, 255, alpha];
}

function drawIsometricCube(target: RgbaImage, texture: RgbaImage, targetX: number, targetY: number): void {
  drawIsometricCubeFaces(
    target,
    {
      top: texture,
      left: texture,
      right: texture,
    },
    targetX,
    targetY,
  );
}

interface CubeFaceImages {
  top: RgbaImage;
  left: RgbaImage;
  right: RgbaImage;
}

function drawIsometricCubeFaces(target: RgbaImage, faces: CubeFaceImages, targetX: number, targetY: number): void {
  const halfWidth = 64;
  const halfDepth = 32;
  const height = 84;
  const originX = targetX + halfWidth;
  const originY = targetY + height;

  drawCubeFace(target, faces.left, 0.68, (u, v) => projectIso(originX, originY, halfWidth, halfDepth, height, u, 1 - v, 1));
  drawCubeFace(target, faces.right, 0.78, (u, v) => projectIso(originX, originY, halfWidth, halfDepth, height, 1, 1 - v, u));
  drawCubeFace(target, faces.top, 1.08, (u, v) => projectIso(originX, originY, halfWidth, halfDepth, height, u, 1, v));
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

function cubeFaces(block: BlockSpec, texturesByName: Map<string, RenderedTexture>, tint: Rgba): CubeFaceImages {
  return {
    top: applyRoleTint(textureForFace(block, texturesByName, "top"), tint),
    left: compositeSideTexture(block, texturesByName, tint),
    right: compositeSideTexture(block, texturesByName, tint),
  };
}

function tintColorsForBlock(block: BlockSpec, texturesByName: Map<string, RenderedTexture>): Rgba[] {
  const tint = tintSpecForBlock(block, texturesByName);
  if (!tint) {
    return [[255, 255, 255, 255]];
  }
  return [tint.normal, ...(tint.alternates ?? [])].map(parseHexColor);
}

function tintSpecForBlock(block: BlockSpec, texturesByName: Map<string, RenderedTexture>): TintSpec | undefined {
  const textureNames = [
    block.faces.top,
    block.faces.overlay,
    block.faces.side,
    block.faces.bottom,
    block.faces.all,
  ];
  for (const textureName of textureNames) {
    if (!textureName) {
      continue;
    }
    const texture = texturesByName.get(textureName);
    if (texture?.tint) {
      return texture.tint;
    }
  }
  return undefined;
}

function textureForFace(block: BlockSpec, texturesByName: Map<string, RenderedTexture>, face: "top" | "bottom" | "side"): RenderedTexture {
  const textureName = block.faces[face] ?? block.faces.all ?? block.faces.side;
  if (!textureName) {
    throw new Error(`Block face '${face}' has no texture`);
  }
  const texture = texturesByName.get(textureName);
  if (!texture) {
    throw new Error(`Block face '${face}' references missing rendered texture '${textureName}'`);
  }
  return texture;
}

function compositeSideTexture(block: BlockSpec, texturesByName: Map<string, RenderedTexture>, tint: Rgba): RgbaImage {
  const side = cloneImage(textureForFace(block, texturesByName, "side"));
  const overlayName = block.faces.overlay;
  if (!overlayName) {
    return side;
  }
  const overlay = texturesByName.get(overlayName);
  if (!overlay) {
    throw new Error(`Block overlay references missing rendered texture '${overlayName}'`);
  }
  return compositeImages(side, applyRoleTint(overlay, tint));
}

function drawTerrainPatch(
  target: RgbaImage,
  block: BlockSpec,
  texturesByName: Map<string, RenderedTexture>,
  tint: Rgba,
  targetX: number,
  targetY: number,
): void {
  const top = applyRoleTint(textureForFace(block, texturesByName, "top"), tint);
  const dirt = textureForFace(block, texturesByName, "bottom");
  const tileScale = 2;
  const tileSize = top.width * tileScale;
  for (let row = 0; row < 4; row += 1) {
    for (let column = 0; column < 10; column += 1) {
      const useDirt = random01("terrain-patch-dirt", column, row) < 0.12;
      const image = useDirt ? dirt : top;
      const rotation = useDirt ? 0 : Math.floor(random01("terrain-patch-grass-rotation", column, row) * 4) % 4;
      drawRotatedScaled(target, image, targetX + column * tileSize, targetY + row * tileSize, tileScale, rotation);
    }
  }
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
        blendPixel(target.data, target.width, x, y, color, 1);
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

function colorDistance(a: Rgba, b: Rgba): number {
  const dr = a[0] - b[0];
  const dg = a[1] - b[1];
  const db = a[2] - b[2];
  const da = a[3] - b[3];
  return Math.sqrt(dr * dr + dg * dg + db * db + da * da) / 510;
}

function cubic(p0: number, p1: number, p2: number, p3: number, alpha: number): number {
  return p1 + 0.5 * alpha * (
    p2 - p0 +
    alpha * (2 * p0 - 5 * p1 + 4 * p2 - p3 + alpha * (3 * (p1 - p2) + p3 - p0))
  );
}

function lerp(a: number, b: number, alpha: number): number {
  return a + (b - a) * alpha;
}

function smoothStep(value: number): number {
  const alpha = clamp01(value);
  return alpha * alpha * (3 - 2 * alpha);
}

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value));
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

function drawCheckerboard(target: RgbaImage, x: number, y: number, width: number, height: number, cellSize: number): void {
  const light: Rgba = [74, 77, 75, 255];
  const dark: Rgba = [45, 47, 46, 255];
  for (let py = 0; py < height; py += 1) {
    for (let px = 0; px < width; px += 1) {
      const color = (Math.floor(px / cellSize) + Math.floor(py / cellSize)) % 2 === 0 ? light : dark;
      writePixel(target.data, target.width, x + px, y + py, color);
    }
  }
}

function drawRect(target: RgbaImage, x: number, y: number, width: number, height: number, color: Rgba): void {
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

function previewTexture(texture: RenderedTexture): RgbaImage {
  if (!texture.tint) {
    return texture;
  }
  return applyRoleTint(texture, parseHexColor(texture.tint.normal));
}

function applyRoleTint(texture: RenderedTexture, tint: Rgba): RgbaImage {
  if (texture.source !== "tintable") {
    return texture;
  }
  return tintTexture(texture, tint);
}

function hasTransparency(image: RgbaImage): boolean {
  for (let index = 3; index < image.data.length; index += 4) {
    if (image.data[index] !== 255) {
      return true;
    }
  }
  return false;
}

function cloneImage(image: RgbaImage): RgbaImage {
  return {
    width: image.width,
    height: image.height,
    data: new Uint8Array(image.data),
  };
}

function tintTexture(image: RgbaImage, tint: Rgba): RgbaImage {
  const out = cloneImage(image);
  for (let index = 0; index < out.data.length; index += 4) {
    out.data[index] = Math.round(out.data[index]! * tint[0] / 255);
    out.data[index + 1] = Math.round(out.data[index + 1]! * tint[1] / 255);
    out.data[index + 2] = Math.round(out.data[index + 2]! * tint[2] / 255);
  }
  return out;
}

function compositeImages(base: RgbaImage, overlay: RgbaImage): RgbaImage {
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

function drawTintSwatch(target: RgbaImage, x: number, y: number, color: Rgba): void {
  drawRect(target, x, y, 64, 20, [24, 26, 26, 255]);
  drawRect(target, x + 2, y + 2, 60, 16, color);
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
