import type { BlockSpec, CubeFaceTextures, TexturePackAsset } from "./dsl";
import { clampByte, type Rgba } from "./image";
import type { RenderedTexture } from "./compositor";
import { runtimeCompatTexturePath } from "./reference";

type LodFaceName = "all" | "top" | "bottom" | "side" | "north" | "south" | "east" | "west";

interface LodMaterialsAsset {
  schemaVersion: 1;
  pack: {
    name: string;
    defaultSize: number;
  };
  textures: Record<string, LodTextureMaterial>;
  blocks: Record<string, LodBlockMaterial>;
  aliases: Record<string, string>;
}

interface LodTextureMaterial {
  exportPath: string;
  runtimePath?: string;
  source: RenderedTexture["source"];
  tintRole?: string;
  tintSrgb?: Rgba;
  baseAverageSrgb: Rgba;
  averageSrgb: Rgba;
}

interface LodBlockMaterial {
  sourceBlockName: string;
  kind: BlockSpec["kind"];
  faces: Partial<Record<LodFaceName, LodBlockFaceMaterial>>;
}

interface LodBlockFaceMaterial {
  texture: string;
  overlay?: string;
  averageSrgb: Rgba;
}

export function makeLodMaterialsJson(pack: TexturePackAsset, textures: RenderedTexture[]): string {
  return `${JSON.stringify(makeLodMaterials(pack, textures), null, 2)}\n`;
}

export function makeLodMaterials(pack: TexturePackAsset, textures: RenderedTexture[]): LodMaterialsAsset {
  const texturesByName = new Map(textures.map((texture) => [texture.name, texture]));
  const textureMaterials: Record<string, LodTextureMaterial> = {};
  for (const texture of textures) {
    textureMaterials[texture.name] = textureMaterial(texture);
  }

  const blocks: Record<string, LodBlockMaterial> = {};
  const aliases: Record<string, string> = {};
  for (const [blockName, block] of Object.entries(pack.blocks)) {
    const materialName = canonicalBlockName(blockName);
    if (materialName !== blockName) {
      aliases[blockName] = materialName;
    }
    blocks[materialName] = blockMaterial(blockName, block, texturesByName);
  }

  return {
    schemaVersion: 1,
    pack: {
      name: pack.name,
      defaultSize: pack.defaultSize,
    },
    textures: textureMaterials,
    blocks,
    aliases,
  };
}

function textureMaterial(texture: RenderedTexture): LodTextureMaterial {
  const runtimePath = runtimeCompatTexturePath(texture.exportPath);
  const material: LodTextureMaterial = {
    exportPath: texture.exportPath,
    source: texture.source,
    baseAverageSrgb: averageTexture(texture, { tint: false }),
    averageSrgb: averageTexture(texture, { tint: true }),
  };
  if (runtimePath) {
    material.runtimePath = runtimePath;
  }
  if (texture.tintRole) {
    material.tintRole = texture.tintRole;
  }
  if (texture.tint) {
    material.tintSrgb = parseHexColor(texture.tint.normal);
  }
  return material;
}

function blockMaterial(
  sourceBlockName: string,
  block: BlockSpec,
  texturesByName: Map<string, RenderedTexture>,
): LodBlockMaterial {
  const faces: Partial<Record<LodFaceName, LodBlockFaceMaterial>> = {};
  for (const face of ["top", "bottom", "side", "north", "south", "east", "west"] as const) {
    const material = blockFaceMaterial(block.faces, face, texturesByName);
    if (material) {
      faces[face] = material;
    }
  }
  return {
    sourceBlockName,
    kind: block.kind,
    faces,
  };
}

function blockFaceMaterial(
  faces: CubeFaceTextures,
  face: Exclude<LodFaceName, "all">,
  texturesByName: Map<string, RenderedTexture>,
): LodBlockFaceMaterial | undefined {
  const textureName = faceTextureName(faces, face);
  if (!textureName) {
    return undefined;
  }
  const texture = texturesByName.get(textureName);
  if (!texture) {
    return undefined;
  }

  const overlayName = lateralFace(face) ? faces.overlay : undefined;
  const overlay = overlayName ? texturesByName.get(overlayName) : undefined;
  const material: LodBlockFaceMaterial = {
    texture: textureName,
    averageSrgb: overlay ? averageComposite(texture, overlay) : averageTexture(texture, { tint: true }),
  };
  if (overlayName && overlay) {
    material.overlay = overlayName;
  }
  return material;
}

function faceTextureName(faces: CubeFaceTextures, face: Exclude<LodFaceName, "all">): string | undefined {
  switch (face) {
    case "top":
      return faces.top ?? faces.all;
    case "bottom":
      return faces.bottom ?? faces.all;
    case "side":
      return faces.side ?? faces.all;
    case "north":
      return faces.north ?? faces.side ?? faces.all;
    case "south":
      return faces.south ?? faces.side ?? faces.all;
    case "east":
      return faces.east ?? faces.side ?? faces.all;
    case "west":
      return faces.west ?? faces.side ?? faces.all;
  }
}

function lateralFace(face: Exclude<LodFaceName, "all">): boolean {
  return face !== "top" && face !== "bottom";
}

function averageTexture(texture: RenderedTexture, options: { tint: boolean }): Rgba {
  return averagePixels(texture.width * texture.height, (pixelIndex) => pixelColor(texture, pixelIndex, options.tint));
}

function averageComposite(base: RenderedTexture, overlay: RenderedTexture): Rgba {
  const width = Math.min(base.width, overlay.width);
  const height = Math.min(base.height, overlay.height);
  return averagePixels(width * height, (pixelIndex) => {
    const x = pixelIndex % width;
    const y = Math.floor(pixelIndex / width);
    const baseIndex = y * base.width + x;
    const overlayIndex = y * overlay.width + x;
    return compositeOver(pixelColor(base, baseIndex, true), pixelColor(overlay, overlayIndex, true));
  });
}

function averagePixels(pixelCount: number, pixel: (pixelIndex: number) => Rgba): Rgba {
  let sumR = 0;
  let sumG = 0;
  let sumB = 0;
  let sumA = 0;
  for (let pixelIndex = 0; pixelIndex < pixelCount; pixelIndex += 1) {
    const [r, g, b, a] = pixel(pixelIndex);
    const alpha = a / 255;
    sumR += r * alpha;
    sumG += g * alpha;
    sumB += b * alpha;
    sumA += alpha;
  }
  if (sumA <= 0) {
    return [0, 0, 0, 0];
  }
  return [
    clampByte(sumR / sumA),
    clampByte(sumG / sumA),
    clampByte(sumB / sumA),
    clampByte(sumA / pixelCount * 255),
  ];
}

function pixelColor(texture: RenderedTexture, pixelIndex: number, tint: boolean): Rgba {
  const index = pixelIndex * 4;
  let r = texture.data[index] ?? 0;
  let g = texture.data[index + 1] ?? 0;
  let b = texture.data[index + 2] ?? 0;
  let a = texture.data[index + 3] ?? 0;
  if (tint && texture.tint) {
    const tintColor = parseHexColor(texture.tint.normal);
    r = clampByte(r * tintColor[0] / 255);
    g = clampByte(g * tintColor[1] / 255);
    b = clampByte(b * tintColor[2] / 255);
    a = clampByte(a * tintColor[3] / 255);
  }
  return [r, g, b, a];
}

function compositeOver(base: Rgba, overlay: Rgba): Rgba {
  const baseAlpha = base[3] / 255;
  const overlayAlpha = overlay[3] / 255;
  const outAlpha = overlayAlpha + baseAlpha * (1 - overlayAlpha);
  if (outAlpha <= 0) {
    return [0, 0, 0, 0];
  }
  return [
    clampByte((overlay[0] * overlayAlpha + base[0] * baseAlpha * (1 - overlayAlpha)) / outAlpha),
    clampByte((overlay[1] * overlayAlpha + base[1] * baseAlpha * (1 - overlayAlpha)) / outAlpha),
    clampByte((overlay[2] * overlayAlpha + base[2] * baseAlpha * (1 - overlayAlpha)) / outAlpha),
    clampByte(outAlpha * 255),
  ];
}

function parseHexColor(hex: string): Rgba {
  const normalized = hex.startsWith("#") ? hex.slice(1) : hex;
  const r = Number.parseInt(normalized.slice(0, 2), 16);
  const g = Number.parseInt(normalized.slice(2, 4), 16);
  const b = Number.parseInt(normalized.slice(4, 6), 16);
  const a = normalized.length >= 8 ? Number.parseInt(normalized.slice(6, 8), 16) : 255;
  return [r, g, b, a];
}

function canonicalBlockName(blockName: string): string {
  if (blockName.includes(":")) {
    return blockName.replaceAll("-", "_");
  }
  return `minecraft:${blockName.replaceAll("-", "_")}`;
}
