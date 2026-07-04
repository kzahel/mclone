import {
  type AuthoringLayerRole,
  type AsciiLayerSpec,
  type MacroNoiseLayerSpec,
  type MaskLayerSpec,
  type PaletteSpec,
  type TextureLayerSpec,
  type TexturePackAsset,
  type TextureSourceCategory,
  type TextureSpec,
  type TintSpec,
} from "./dsl";
import {
  blendPixel,
  clamp01,
  cubicPremul,
  fill,
  lerp,
  lerpPremul,
  lerpRgba,
  premultiply,
  random01,
  resolveColor,
  smoothStep,
  unpremultiply,
  wrap,
  writePixel,
  type PremultipliedRgba,
  type Rgba,
  type RgbaImage,
} from "./image";
import { assertTextureSourcePolicies } from "./source-policy";

export interface RenderedTexture extends RgbaImage {
  name: string;
  exportPath: string;
  source: TextureSourceCategory;
  tintRole?: string;
  tint?: TintSpec;
  preview: TextureSpec["preview"];
  catalog: TextureSpec["catalog"];
  authoring: RenderedAuthoringPreview[];
}

export interface RenderedAuthoringPreview extends RgbaImage {
  role: AuthoringLayerRole;
  label: string;
  entries: AuthoringPreviewEntry[];
}

export interface AuthoringPreviewEntry {
  symbol: string;
  name: string;
  color: Rgba;
}

export function renderAllTextures(pack: TexturePackAsset): RenderedTexture[] {
  const textures = Object.entries(pack.textures).map(([name, texture]) => renderTexture(pack, name, texture));
  assertTextureSourcePolicies(pack, textures);
  return textures;
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
    catalog: texture.catalog,
    authoring: renderAuthoringPreviews(palette, texture.layers ?? []),
  };
  if (texture.tintRole) {
    rendered.tintRole = texture.tintRole;
    rendered.tint = pack.tints[texture.tintRole]!;
  }

  return rendered;
}

function renderAuthoringPreviews(
  palette: PaletteSpec,
  layers: TextureLayerSpec[],
): RenderedAuthoringPreview[] {
  const previews: RenderedAuthoringPreview[] = [];
  for (const layer of layers) {
    if ((layer.kind === "ascii" || layer.kind === "mask") && layer.authoring) {
      previews.push(renderAuthoringLayerPreview(palette, layer));
    }
  }
  return previews;
}

function renderAuthoringLayerPreview(
  palette: PaletteSpec,
  layer: AsciiLayerSpec | MaskLayerSpec,
): RenderedAuthoringPreview {
  const width = layer.pixels[0]?.length ?? 0;
  const height = layer.pixels.length;
  const data = new Uint8Array(width * height * 4);
  const skip = layer.skip ?? ".";

  for (let y = 0; y < height; y += 1) {
    const row = layer.pixels[y] ?? "";
    for (let x = 0; x < width; x += 1) {
      const symbol = row[x] ?? skip;
      if (symbol === skip) {
        writePixel(data, width, x, y, [0, 0, 0, 0]);
        continue;
      }
      const colorName = layer.colors[symbol];
      writePixel(data, width, x, y, colorName ? resolveColor(palette, colorName) : [0, 0, 0, 0]);
    }
  }

  const entries = Object.entries(layer.colors).map(([symbol, colorName]) => ({
    symbol,
    name: colorName,
    color: resolveColor(palette, colorName),
  }));
  return {
    role: layer.authoring!.role,
    label: layer.authoring!.label ?? "AUTHOR STRUCTURE",
    entries,
    width,
    height,
    data,
  };
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
