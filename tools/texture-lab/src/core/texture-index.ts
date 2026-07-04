import fs from "node:fs/promises";
import path from "node:path";
import { discoverTextureCandidates } from "./candidate-index";
import type { BlockSpec, TexturePackAsset, TextureSpec } from "../dsl";
import { loadTexturePack } from "../load";
import { textureLabOutputRoot } from "../output-root";
import { runtimeCompatTexturePath } from "../reference";
import {
  TEXTURE_LAB_INDEX_SCHEMA_VERSION,
  type BlockFaceIndexEntry,
  type BlockIndexEntry,
  type TextureBlockUsage,
  type TextureImageRef,
  type TextureIndexEntry,
  type TextureLabIndex,
  type TextureRole,
} from "./index-model";

export interface BuildTextureLabIndexOptions {
  inputPath: string;
  outputRoot?: string;
}

export async function buildTextureLabIndex(options: BuildTextureLabIndexOptions): Promise<TextureLabIndex> {
  const inputPath = path.resolve(options.inputPath);
  const outputRoot = path.resolve(options.outputRoot ?? textureLabOutputRoot());
  const pack = await loadTexturePack(inputPath);
  const candidateDiscovery = await discoverTextureCandidates(outputRoot, { textureNames: Object.keys(pack.textures) });
  const blockUsagesByTexture = collectBlockUsages(pack);
  const textures = await Promise.all(
    Object.entries(pack.textures)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([name, texture]) => textureEntryFrom(pack, name, texture, outputRoot, blockUsagesByTexture.get(name) ?? [])),
  );
  const blocks = await Promise.all(
    Object.entries(pack.blocks)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([name, block]) => blockEntryFrom(name, block, outputRoot)),
  );

  return {
    schemaVersion: TEXTURE_LAB_INDEX_SCHEMA_VERSION,
    generatedAt: new Date().toISOString(),
    outputRoot,
    pack: {
      name: pack.name,
      inputPath,
      defaultSize: pack.defaultSize,
      textureCount: Object.keys(pack.textures).length,
      blockCount: Object.keys(pack.blocks).length,
    },
    summary: {
      authoredTextures: textures.length,
      tintableTextures: textures.filter((texture) => texture.source === "tintable").length,
      currentExportsPresent: textures.filter((texture) => texture.images.currentExport.exists).length,
      sheetsPresent: textures.filter((texture) => texture.images.sheet.exists).length,
      runtimeExportsPresent: textures.filter((texture) => texture.images.runtimeExport.exists).length,
      candidateCount: candidateDiscovery.candidates.length,
      associatedCandidateCount: candidateDiscovery.candidates.filter((candidate) => candidate.textureName !== null).length,
      archivedCandidateCount: candidateDiscovery.candidates.filter((candidate) => candidate.archived).length,
    },
    textures,
    candidates: candidateDiscovery.candidates,
    blocks,
    warnings: [...buildWarnings(textures), ...buildCandidateWarnings(candidateDiscovery.candidates), ...candidateDiscovery.warnings],
  };
}

function collectBlockUsages(pack: TexturePackAsset): Map<string, TextureBlockUsage[]> {
  const usages = new Map<string, TextureBlockUsage[]>();
  for (const [blockName, block] of Object.entries(pack.blocks)) {
    for (const face of blockFaces(block)) {
      const textureUsages = usages.get(face.textureName) ?? [];
      textureUsages.push({
        blockName,
        face: face.face,
        role: face.role,
      });
      usages.set(face.textureName, textureUsages);
    }
  }
  return usages;
}

async function textureEntryFrom(
  pack: TexturePackAsset,
  name: string,
  texture: TextureSpec,
  outputRoot: string,
  blockUsages: TextureBlockUsage[],
): Promise<TextureIndexEntry> {
  const runtimeCompatPath = runtimeCompatTexturePath(texture.exportPath);
  const currentExportPath = path.join(outputRoot, "pack", texture.exportPath);
  const runtimeExportPath = runtimeCompatPath ? path.join(outputRoot, "runtime-pack", runtimeCompatPath) : null;
  const sheetPath = path.join(outputRoot, `${name}-sheet.png`);
  const catalog = texture.catalog;
  const tags = catalog?.tags ?? [];
  const notes = catalog?.notes ?? [];
  const source = texture.source ?? "final-color";

  return {
    name,
    displayName: displayNameFrom(name),
    source,
    size: texture.size ?? pack.defaultSize,
    palette: texture.palette,
    base: texture.base,
    tintRole: texture.tintRole ?? null,
    materialFamily: inferMaterialFamily(name, texture, blockUsages),
    exportPath: texture.exportPath,
    runtimeCompatPath,
    status: catalog?.status ?? "draft",
    tiling: catalog?.tiling ?? texture.preview?.tiling ?? "unknown",
    rotation: catalog?.rotation ?? "unknown",
    tags,
    notes,
    authoringRoles: authoringRolesFrom(texture),
    blockUsages: blockUsages.sort(compareBlockUsages),
    images: {
      currentExport: await imageRef("Current export", currentExportPath, "pnpm texture-lab:export"),
      runtimeExport: await imageRef(
        "Runtime export",
        runtimeExportPath,
        runtimeCompatPath ? "pnpm texture-lab:runtime-compat" : null,
      ),
      sheet: await imageRef("Review sheet", sheetPath, "pnpm texture-lab:sheet"),
    },
  };
}

async function blockEntryFrom(name: string, block: BlockSpec, outputRoot: string): Promise<BlockIndexEntry> {
  const sheetName = `${name}-sheet.png`;
  return {
    name,
    kind: block.kind,
    faces: blockFaces(block),
    sheet: await imageRef("Block sheet", path.join(outputRoot, sheetName), "pnpm texture-lab:sheet"),
  };
}

function blockFaces(block: BlockSpec): BlockFaceIndexEntry[] {
  const faces: BlockFaceIndexEntry[] = [];
  for (const [face, textureName] of Object.entries(block.faces)) {
    if (!textureName) {
      continue;
    }
    faces.push({
      face,
      role: textureRoleFromFace(face),
      textureName,
    });
  }
  return faces.sort((left, right) => left.face.localeCompare(right.face));
}

function textureRoleFromFace(face: string): TextureRole {
  if (
    face === "all" ||
    face === "top" ||
    face === "bottom" ||
    face === "side" ||
    face === "overlay" ||
    face === "particle"
  ) {
    return face;
  }
  if (face === "north" || face === "south" || face === "east" || face === "west") {
    return "face";
  }
  return "unknown";
}

async function imageRef(label: string, imagePath: string | null, missingCommand: string | null): Promise<TextureImageRef> {
  return {
    label,
    path: imagePath,
    exists: imagePath ? await fileExists(imagePath) : false,
    missingCommand,
  };
}

async function fileExists(filePath: string): Promise<boolean> {
  try {
    const stat = await fs.stat(filePath);
    return stat.isFile();
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return false;
    }
    throw error;
  }
}

function displayNameFrom(name: string): string {
  return name
    .split("_")
    .map((part) => (part.length > 0 ? `${part[0]!.toUpperCase()}${part.slice(1)}` : part))
    .join(" ");
}

function inferMaterialFamily(name: string, texture: TextureSpec, usages: TextureBlockUsage[]): string {
  const tags = texture.catalog?.tags ?? [];
  const materialTag = tags.find((tag) => tag.startsWith("material:"));
  if (materialTag) {
    return materialTag.slice("material:".length);
  }
  if (name.includes("grass")) {
    return "grass";
  }
  if (name.includes("dirt")) {
    return "dirt";
  }
  if (name.includes("ore")) {
    return "ore";
  }
  if (name.includes("stone") || usages.some((usage) => usage.blockName.includes("stone"))) {
    return "stone";
  }
  if (texture.exportPath.includes("/lod/")) {
    return "far-lod";
  }
  return "misc";
}

function authoringRolesFrom(texture: TextureSpec): string[] {
  const roles = new Set<string>();
  for (const layer of texture.layers ?? []) {
    if ((layer.kind === "ascii" || layer.kind === "mask") && layer.authoring) {
      roles.add(layer.authoring.role);
    }
  }
  return [...roles].sort();
}

function compareBlockUsages(left: TextureBlockUsage, right: TextureBlockUsage): number {
  return left.blockName.localeCompare(right.blockName) || left.face.localeCompare(right.face);
}

function buildWarnings(textures: TextureIndexEntry[]): string[] {
  const warnings: string[] = [];
  const missingExports = textures.filter((texture) => !texture.images.currentExport.exists);
  if (missingExports.length > 0) {
    warnings.push(`${missingExports.length} current texture exports are missing. Run pnpm texture-lab:export.`);
  }
  const missingSheets = textures.filter((texture) => !texture.images.sheet.exists);
  if (missingSheets.length > 0) {
    warnings.push(`${missingSheets.length} review sheets are missing. Run pnpm texture-lab:sheet.`);
  }
  return warnings;
}

function buildCandidateWarnings(candidates: { textureName: string | null }[]): string[] {
  const unassociatedCount = candidates.filter((candidate) => candidate.textureName === null).length;
  return unassociatedCount > 0
    ? [`${unassociatedCount} generated candidate artifact(s) are not associated with an authored texture.`]
    : [];
}
