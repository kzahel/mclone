export interface TexturePackAsset {
  schemaVersion: 1;
  name: string;
  defaultSize: number;
  tints: Record<string, TintSpec>;
  palettes: Record<string, PaletteSpec>;
  textures: Record<string, TextureSpec>;
  blocks: Record<string, BlockSpec>;
}

export type PaletteSpec = Record<string, string>;
export type TextureSourceCategory = "final-color" | "tintable";

export interface TintSpec {
  normal: string;
  alternates?: string[];
}

export interface TextureSpec {
  size?: number;
  source?: TextureSourceCategory;
  tintRole?: string;
  palette: string;
  base: string;
  exportPath: string;
  layers?: TextureLayerSpec[];
  preview?: TexturePreviewSpec;
}

export interface TexturePreviewSpec {
  checkerboard?: boolean;
  cube?: boolean;
  rotation?: boolean;
  tiling?: "xy" | "x" | "none";
}

export interface BlockSpec {
  kind: "cube";
  faces: CubeFaceTextures;
}

export interface CubeFaceTextures {
  all?: string;
  top?: string;
  bottom?: string;
  side?: string;
  north?: string;
  south?: string;
  east?: string;
  west?: string;
  overlay?: string;
}

export type TextureLayerSpec = SpecklesLayerSpec | AsciiLayerSpec;

export interface SpecklesLayerSpec {
  kind: "speckles";
  seed: string;
  density: number;
  colors: string[];
  opacity?: number;
  radius?: number;
}

export interface AsciiLayerSpec {
  kind: "ascii";
  pixels: string[];
  colors: Record<string, string>;
  skip?: string;
  opacity?: number;
}

export interface TextureLabApi {
  tint(name: string, tint: TintSpec): void;
  palette(name: string, colors: PaletteSpec): void;
  texture(name: string, texture: TextureSpec): void;
  block(name: string, block: BlockSpec): void;
  ascii(layer: Omit<AsciiLayerSpec, "kind">): AsciiLayerSpec;
  speckles(layer: Omit<SpecklesLayerSpec, "kind">): SpecklesLayerSpec;
}

export function texturePack(
  name: string,
  build: (api: TextureLabApi) => void,
  options: { defaultSize?: number } = {},
): TexturePackAsset {
  const builder = new TexturePackBuilder(name, options.defaultSize ?? 32);
  build(builder.api());
  const asset = builder.build();
  assertValidTexturePack(asset);
  return asset;
}

export function assertValidTexturePack(asset: TexturePackAsset): void {
  const errors: string[] = [];
  if (asset.schemaVersion !== 1) {
    errors.push(`unsupported texture pack schema version ${asset.schemaVersion}`);
  }
  if (!asset.name) {
    errors.push("texture pack name is required");
  }
  if (!Number.isInteger(asset.defaultSize) || asset.defaultSize <= 0) {
    errors.push(`default size must be a positive integer, got ${asset.defaultSize}`);
  }
  if (!asset.tints || typeof asset.tints !== "object") {
    errors.push("texture pack tints must be an object");
  } else {
    for (const [tintName, tint] of Object.entries(asset.tints)) {
      validateName("tint", tintName, errors);
      if (!isHexColor(tint.normal)) {
        errors.push(`tint '${tintName}' normal color has invalid color '${tint.normal}'`);
      }
      for (const [alternateIndex, alternate] of (tint.alternates ?? []).entries()) {
        if (!isHexColor(alternate)) {
          errors.push(`tint '${tintName}' alternate ${alternateIndex} has invalid color '${alternate}'`);
        }
      }
    }
  }
  for (const [paletteName, palette] of Object.entries(asset.palettes)) {
    validateName("palette", paletteName, errors);
    for (const [colorName, color] of Object.entries(palette)) {
      validateName(`palette '${paletteName}' color`, colorName, errors);
      if (!isHexColor(color)) {
        errors.push(`palette '${paletteName}' color '${colorName}' has invalid color '${color}'`);
      }
    }
  }
  for (const [textureName, texture] of Object.entries(asset.textures)) {
    validateName("texture", textureName, errors);
    const source = texture.source ?? "final-color";
    if (texture.source !== undefined && texture.source !== "final-color" && texture.source !== "tintable") {
      errors.push(`texture '${textureName}' source must be 'final-color' or 'tintable'`);
    }
    if (source === "tintable") {
      if (!texture.tintRole) {
        errors.push(`texture '${textureName}' is tintable and must declare tintRole`);
      } else if (!asset.tints?.[texture.tintRole]) {
        errors.push(`texture '${textureName}' references missing tint role '${texture.tintRole}'`);
      }
    } else if (texture.tintRole) {
      errors.push(`texture '${textureName}' declares tintRole but source is '${source}'`);
    }
    const size = texture.size ?? asset.defaultSize;
    if (!Number.isInteger(size) || size <= 0) {
      errors.push(`texture '${textureName}' size must be a positive integer, got ${size}`);
    }
    const palette = asset.palettes[texture.palette];
    if (!palette) {
      errors.push(`texture '${textureName}' references missing palette '${texture.palette}'`);
      continue;
    }
    if (!palette[texture.base]) {
      errors.push(`texture '${textureName}' references missing base color '${texture.base}'`);
    }
    if (!texture.exportPath.startsWith("assets/") || !texture.exportPath.endsWith(".png")) {
      errors.push(
        `texture '${textureName}' exportPath must be an assets/**/*.png path, got '${texture.exportPath}'`,
      );
    }
    const preview = texture.preview as (TexturePreviewSpec & { tint?: unknown }) | undefined;
    if (preview?.tint !== undefined) {
      errors.push(`texture '${textureName}' preview.tint has been replaced by source/tintRole metadata`);
    }
    if (
      texture.preview?.tiling !== undefined &&
      texture.preview.tiling !== "xy" &&
      texture.preview.tiling !== "x" &&
      texture.preview.tiling !== "none"
    ) {
      errors.push(`texture '${textureName}' preview tiling must be 'xy', 'x', or 'none'`);
    }
    for (const [layerIndex, layer] of (texture.layers ?? []).entries()) {
      validateLayer(textureName, layerIndex, layer, palette, size, errors);
    }
  }
  if (Object.keys(asset.textures).length === 0) {
    errors.push("texture pack has no textures");
  }
  for (const [blockName, block] of Object.entries(asset.blocks)) {
    validateName("block", blockName, errors);
    validateBlock(blockName, block, asset.textures, errors);
  }
  if (errors.length > 0) {
    throw new Error(`Invalid texture pack '${asset.name}':\n${errors.map((error) => `- ${error}`).join("\n")}`);
  }
}

class TexturePackBuilder {
  private readonly tints: Record<string, TintSpec> = {};
  private readonly palettes: Record<string, PaletteSpec> = {};
  private readonly textures: Record<string, TextureSpec> = {};
  private readonly blocks: Record<string, BlockSpec> = {};

  constructor(
    private readonly name: string,
    private readonly defaultSize: number,
  ) {}

  api(): TextureLabApi {
    return {
      tint: (name, tint) => {
        this.tints[name] = tint;
      },
      palette: (name, colors) => {
        this.palettes[name] = colors;
      },
      texture: (name, texture) => {
        this.textures[name] = texture;
      },
      block: (name, block) => {
        this.blocks[name] = block;
      },
      ascii: (layer) => ({ kind: "ascii", ...layer }),
      speckles: (layer) => ({ kind: "speckles", ...layer }),
    };
  }

  build(): TexturePackAsset {
    return {
      schemaVersion: 1,
      name: this.name,
      defaultSize: this.defaultSize,
      tints: this.tints,
      palettes: this.palettes,
      textures: this.textures,
      blocks: this.blocks,
    };
  }
}

function validateBlock(
  blockName: string,
  block: BlockSpec,
  textures: Record<string, TextureSpec>,
  errors: string[],
): void {
  if (block.kind !== "cube") {
    errors.push(`block '${blockName}' has unsupported kind '${block.kind}'`);
  }
  const textureNames = Object.values(block.faces).flatMap((value) => (Array.isArray(value) ? value : [value]));
  if (textureNames.length === 0) {
    errors.push(`block '${blockName}' has no face textures`);
  }
  for (const textureName of textureNames) {
    if (textureName && !textures[textureName]) {
      errors.push(`block '${blockName}' references missing texture '${textureName}'`);
    }
  }
}

function validateLayer(
  textureName: string,
  layerIndex: number,
  layer: TextureLayerSpec,
  palette: PaletteSpec,
  size: number,
  errors: string[],
): void {
  if (layer.kind === "speckles") {
    if (!Number.isFinite(layer.density) || layer.density < 0 || layer.density > 1) {
      errors.push(`texture '${textureName}' layer ${layerIndex} density must be 0..1`);
    }
    if (layer.opacity !== undefined && (!Number.isFinite(layer.opacity) || layer.opacity < 0 || layer.opacity > 1)) {
      errors.push(`texture '${textureName}' layer ${layerIndex} opacity must be 0..1`);
    }
    if (layer.radius !== undefined && (!Number.isInteger(layer.radius) || layer.radius < 0)) {
      errors.push(`texture '${textureName}' layer ${layerIndex} radius must be a non-negative integer`);
    }
    if (layer.colors.length === 0) {
      errors.push(`texture '${textureName}' layer ${layerIndex} has no speckle colors`);
    }
    for (const color of layer.colors) {
      if (!palette[color] && !isHexColor(color)) {
        errors.push(`texture '${textureName}' layer ${layerIndex} references missing color '${color}'`);
      }
    }
    return;
  }

  if (layer.pixels.length !== size) {
    errors.push(`texture '${textureName}' layer ${layerIndex} has ${layer.pixels.length} rows, expected ${size}`);
  }
  for (const [rowIndex, row] of layer.pixels.entries()) {
    if (row.length !== size) {
      errors.push(`texture '${textureName}' layer ${layerIndex} row ${rowIndex} has width ${row.length}, expected ${size}`);
    }
    for (const symbol of row) {
      if (symbol === (layer.skip ?? ".")) {
        continue;
      }
      const color = layer.colors[symbol];
      if (!color) {
        errors.push(`texture '${textureName}' layer ${layerIndex} uses unmapped symbol '${symbol}'`);
      } else if (!palette[color] && !isHexColor(color)) {
        errors.push(`texture '${textureName}' layer ${layerIndex} symbol '${symbol}' references missing color '${color}'`);
      }
    }
  }
  if (layer.opacity !== undefined && (!Number.isFinite(layer.opacity) || layer.opacity < 0 || layer.opacity > 1)) {
    errors.push(`texture '${textureName}' layer ${layerIndex} opacity must be 0..1`);
  }
}

function validateName(kind: string, name: string, errors: string[]): void {
  if (!/^[a-z0-9_.-]+$/.test(name)) {
    errors.push(`${kind} name '${name}' must use lowercase letters, digits, _, ., or -`);
  }
}

export function isHexColor(value: string): boolean {
  return /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(value);
}
