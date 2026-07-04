import fs from "node:fs/promises";
import path from "node:path";
import { discoverTextureCandidates } from "./candidate-index";
import { buildTextureCurationState } from "./curation";
import type { BlockSpec, TexturePackAsset, TextureSpec } from "../dsl";
import { loadTexturePack } from "../load";
import { textureLabOutputRoot } from "../output-root";
import { findOrCreateReferencePreviewFile, runtimeCompatTexturePath } from "../reference";
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
  const curation = await buildTextureCurationState(outputRoot, candidateDiscovery.candidates);
  const blockUsagesByTexture = collectBlockUsages(pack);
  const textures = await Promise.all(
    Object.entries(pack.textures)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([name, texture]) => textureEntryFrom(pack, name, texture, outputRoot, blockUsagesByTexture.get(name) ?? [])),
  );
  const blocks = await Promise.all(
    Object.entries(pack.blocks)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([name, block]) => blockEntryFrom(pack, name, block, outputRoot)),
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
      proceduralPlaceholderCount: textures.filter((texture) => texture.artSource.kind === "procedural-placeholder").length,
      tintableTextures: textures.filter((texture) => texture.source === "tintable").length,
      currentExportsPresent: textures.filter((texture) => texture.images.currentExport.exists).length,
      sheetsPresent: textures.filter((texture) => texture.images.sheet.exists).length,
      runtimeExportsPresent: textures.filter((texture) => texture.images.runtimeExport.exists).length,
      candidateCount: candidateDiscovery.candidates.length,
      associatedCandidateCount: candidateDiscovery.candidates.filter((candidate) => candidate.textureName !== null).length,
      archivedCandidateCount: candidateDiscovery.candidates.filter((candidate) => candidate.archived).length,
      curatedSelectionCount: curation.selectedCount,
      frozenTextureCount: Object.keys(pack.frozenTextures ?? {}).length,
    },
    curation,
    textures,
    candidates: candidateDiscovery.candidates,
    blocks,
    warnings: [
      ...buildWarnings(textures),
      ...buildCandidateWarnings(candidateDiscovery.candidates),
      ...buildCurationWarnings(curation),
      ...candidateDiscovery.warnings,
    ],
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
  const referencePath = await findOrCreateReferencePreviewFile(texture.exportPath, outputRoot);
  const sheetPath = path.join(outputRoot, `${name}-sheet.png`);
  const catalog = texture.catalog;
  const tags = catalog?.tags ?? [];
  const notes = catalog?.notes ?? [];
  const source = texture.source ?? "final-color";
  const tint = texture.tintRole ? pack.tints[texture.tintRole] : undefined;
  const frozen = frozenRef(pack, name);
  const authoringRoles = authoringRolesFrom(texture);

  return {
    name,
    displayName: displayNameFrom(name),
    source,
    size: texture.size ?? pack.defaultSize,
    palette: texture.palette,
    base: texture.base,
    tintRole: texture.tintRole ?? null,
    tint: tint
      ? {
          role: texture.tintRole!,
          normal: tint.normal,
          alternates: tint.alternates ?? [],
          sourceNeutrality: tint.sourceNeutrality ?? null,
        }
      : null,
    materialFamily: inferMaterialFamily(name, texture, blockUsages),
    exportPath: texture.exportPath,
    runtimeCompatPath,
    status: catalog?.status ?? "draft",
    artSource: artSourceFrom(texture, frozen, tags, authoringRoles),
    tiling: catalog?.tiling ?? texture.preview?.tiling ?? "unknown",
    rotation: catalog?.rotation ?? "unknown",
    tags,
    notes,
    frozen,
    authoringRoles,
    blockUsages: blockUsages.sort(compareBlockUsages),
    images: {
      currentExport: await imageRef("Current export", currentExportPath, "pnpm texture-lab:export"),
      runtimeExport: await imageRef(
        "Runtime export",
        runtimeExportPath,
        runtimeCompatPath ? "pnpm texture-lab:runtime-compat" : null,
      ),
      minecraftReference: await imageRef("Minecraft reference", referencePath, "./scripts/extract-assets.sh"),
      sheet: await imageRef("Review sheet", sheetPath, "pnpm texture-lab:sheet"),
    },
  };
}

function artSourceFrom(
  texture: TextureSpec,
  frozen: TextureIndexEntry["frozen"],
  tags: string[],
  authoringRoles: string[],
): TextureIndexEntry["artSource"] {
  if (frozen) {
    return {
      kind: "frozen",
      label: "frozen asset",
      description: "Committed frozen PNG selected from a reviewed generated candidate.",
    };
  }
  if (isProceduralPlaceholder(texture, tags)) {
    return {
      kind: "procedural-placeholder",
      label: "noise placeholder",
      description: "Seeded macro-noise and speckle coverage art; useful for layout review, not a curated texture.",
    };
  }
  if (authoringRoles.length > 0) {
    return {
      kind: "authored-structure",
      label: "authored structure",
      description: "Author-controlled mask or ASCII structure drives the current texture.",
    };
  }
  return {
    kind: "authored-baseline",
    label: "authored baseline",
    description: "Authored palette and layer source without a frozen generated override.",
  };
}

function isProceduralPlaceholder(texture: TextureSpec, tags: string[]): boolean {
  if (tags.includes("placeholder") || tags.includes("far-lod-material")) {
    return true;
  }
  const layers = texture.layers ?? [];
  return layers.length > 0 && layers.every((layer) => layer.kind === "macroNoise" || layer.kind === "speckles");
}

function frozenRef(pack: TexturePackAsset, textureName: string): TextureIndexEntry["frozen"] {
  const frozen = pack.frozenTextures?.[textureName];
  if (!frozen) {
    return null;
  }
  return {
    asset: frozen.asset,
    path: frozen.path,
    sha256: frozen.sha256,
    codename: frozen.metadata.codename ?? null,
    candidateId: frozen.metadata.candidateId ?? null,
    sourceContextGitCommit: frozen.metadata.sourceContext?.gitCommit ?? null,
  };
}

async function blockEntryFrom(pack: TexturePackAsset, name: string, block: BlockSpec, outputRoot: string): Promise<BlockIndexEntry> {
  const sheetName = pack.textures[name] ? `${name}-block-sheet.png` : `${name}-sheet.png`;
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
  return faces.sort(compareBlockFaces);
}

function compareBlockFaces(left: BlockFaceIndexEntry, right: BlockFaceIndexEntry): number {
  return blockFaceOrder(left.face) - blockFaceOrder(right.face) || left.face.localeCompare(right.face);
}

function blockFaceOrder(face: string): number {
  const order = ["all", "top", "bottom", "north", "east", "south", "west", "side", "overlay", "particle"];
  const index = order.indexOf(face);
  return index === -1 ? order.length : index;
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
  if (name.includes("redstone") || usages.some((usage) => usage.blockName.includes("redstone"))) {
    return "redstone";
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

function buildCurationWarnings(curation: { staleSelections: { textureName: string; reason: string }[] }): string[] {
  return curation.staleSelections.map((selection) => `Curated selection for ${selection.textureName} is stale: ${selection.reason}.`);
}
