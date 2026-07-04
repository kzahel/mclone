import fs from "node:fs/promises";
import path from "node:path";
import type { BlockKind, BlockSpec, CubeFaceTextures, TexturePackAsset } from "../dsl";
import { encodePng } from "../png";
import { makeBlockReviewSheet, renderAllTextures } from "../render";
import type { TextureIndexEntry, TextureVanillaUseSummary } from "./index-model";

interface VanillaPreviewBlockContext {
  texture: TextureIndexEntry;
  use: TextureVanillaUseSummary;
}

export interface VanillaPreviewBlock {
  name: string;
  block: BlockSpec;
}

export function deriveVanillaPreviewBlocks(
  pack: TexturePackAsset,
  textures: TextureIndexEntry[],
): VanillaPreviewBlock[] {
  const authoredBlockNames = new Set(Object.keys(pack.blocks));
  const targetBlocks = new Set<string>();
  for (const texture of textures) {
    if (texture.blockUsages.length > 0) {
      continue;
    }
    const primaryBlock = primaryVanillaBlock(texture);
    if (primaryBlock && !authoredBlockNames.has(blockNameFromResource(primaryBlock))) {
      targetBlocks.add(primaryBlock);
    }
  }

  const contextsByBlock = new Map<string, VanillaPreviewBlockContext[]>();
  for (const texture of textures) {
    for (const use of texture.vanillaUsage?.uses ?? []) {
      if (!targetBlocks.has(use.block)) {
        continue;
      }
      const contexts = contextsByBlock.get(use.block) ?? [];
      contexts.push({ texture, use });
      contextsByBlock.set(use.block, contexts);
    }
  }

  const blocks: VanillaPreviewBlock[] = [];
  for (const [vanillaBlock, contexts] of contextsByBlock) {
    const kind = blockKindForContexts(vanillaBlock, contexts);
    if (!kind) {
      continue;
    }
    const faces = facesForContexts(kind, contexts);
    if (!faces) {
      continue;
    }
    blocks.push({
      name: blockNameFromResource(vanillaBlock),
      block: { kind, faces },
    });
  }

  return blocks.sort((left, right) => left.name.localeCompare(right.name));
}

export async function writeVanillaPreviewBlockSheets(
  pack: TexturePackAsset,
  blocks: VanillaPreviewBlock[],
  outputRoot: string,
): Promise<void> {
  if (blocks.length === 0) {
    return;
  }
  const texturesByName = new Map(renderAllTextures(pack).map((texture) => [texture.name, texture]));
  await fs.mkdir(outputRoot, { recursive: true });
  for (const { name, block } of blocks) {
    await fs.writeFile(path.join(outputRoot, blockSheetName(pack, name)), encodePng(makeBlockReviewSheet(name, block, texturesByName)));
  }
}

export function blockSheetName(pack: TexturePackAsset, blockName: string): string {
  return pack.textures[blockName] ? `${blockName}-block-sheet.png` : `${blockName}-sheet.png`;
}

function primaryVanillaBlock(texture: TextureIndexEntry): string | null {
  const usage = texture.vanillaUsage;
  if (
    !usage ||
    usage.previewHint === "unknown" ||
    usage.previewHint === "fluid" ||
    usage.previewHint === "cube" ||
    usage.uses.length === 0
  ) {
    return null;
  }
  const textureStem = usage.texture.split("/").pop() ?? texture.name;
  return [...usage.uses]
    .sort((left, right) => scorePrimaryUse(right, textureStem) - scorePrimaryUse(left, textureStem) || left.block.localeCompare(right.block))[0]
    ?.block ?? null;
}

function scorePrimaryUse(use: TextureVanillaUseSummary, textureStem: string): number {
  const blockStem = use.block.split(":")[1] ?? use.block;
  let score = use.role === "face" ? 20 : 0;
  if (blockStem === textureStem) {
    score += 120;
  } else if (textureStem.startsWith(`${blockStem}_`)) {
    score += 90;
  } else if (blockStem.startsWith(`${textureStem}_`)) {
    score += 70;
  } else if (textureStem.includes(blockStem) || blockStem.includes(textureStem)) {
    score += 40;
  }
  return score;
}

function blockKindForContexts(vanillaBlock: string, contexts: VanillaPreviewBlockContext[]): BlockKind | null {
  const blockName = blockNameFromResource(vanillaBlock);
  const kinds = new Set(contexts.map((context) => context.use.geometryKind));
  const families = contexts.flatMap((context) => context.texture.vanillaUsage?.modelFamilies ?? []);
  if (kinds.has("pane") || blockName.includes("pane")) {
    return "pane";
  }
  if (kinds.has("door") || blockName.includes("door") && !blockName.includes("trapdoor")) {
    return "door";
  }
  if (kinds.has("trapdoor") || blockName.includes("trapdoor")) {
    return "trapdoor";
  }
  if (kinds.has("torch") || blockName.includes("torch")) {
    return "torch";
  }
  if (kinds.has("rail") || families.some((family) => family.includes("rail")) || blockName.includes("rail")) {
    return "rail";
  }
  if (kinds.has("flat-ground")) {
    return "flat";
  }
  if (kinds.has("cross-sprite") || kinds.has("crop-cross")) {
    return "cross";
  }
  if (kinds.has("cube") || kinds.has("partial-model")) {
    return "cube";
  }
  return null;
}

function facesForContexts(kind: BlockKind, contexts: VanillaPreviewBlockContext[]): CubeFaceTextures | null {
  const faces: CubeFaceTextures = {};
  for (const context of contexts) {
    const face = faceForContext(kind, context);
    if (!face || faces[face]) {
      continue;
    }
    faces[face] = context.texture.name;
  }
  normalizeFaces(kind, faces);
  return hasRequiredFaces(kind, faces) ? faces : null;
}

function faceForContext(kind: BlockKind, { texture, use }: VanillaPreviewBlockContext): keyof CubeFaceTextures | null {
  const slot = use.textureSlot ?? "";
  if (kind === "cross") {
    return "all";
  }
  if (kind === "flat" || kind === "rail" || kind === "trapdoor") {
    return "top";
  }
  if (kind === "pane") {
    return slot === "edge" || texture.name.includes("pane_top") ? "top" : "side";
  }
  if (kind === "torch") {
    return "side";
  }
  if (kind === "door") {
    if (slot === "top" || texture.name.endsWith("_top")) {
      return "top";
    }
    if (slot === "bottom" || texture.name.endsWith("_bottom")) {
      return "bottom";
    }
    return "side";
  }
  if (use.face === "up") {
    return "top";
  }
  if (use.face === "down") {
    return "bottom";
  }
  if (use.face === "north" || use.face === "east" || use.face === "south" || use.face === "west") {
    return "side";
  }
  return "all";
}

function normalizeFaces(kind: BlockKind, faces: CubeFaceTextures): void {
  if (kind === "pane") {
    if (!faces.side && faces.top) {
      faces.side = faces.top;
    }
    if (!faces.top && faces.side) {
      faces.top = faces.side;
    }
  } else if (kind === "door") {
    if (!faces.top && faces.bottom) {
      faces.top = faces.bottom;
    }
    if (!faces.bottom && faces.top) {
      faces.bottom = faces.top;
    }
  } else if (kind === "cube") {
    const fallback = faces.side ?? faces.top ?? faces.bottom;
    if (!faces.all && fallback) {
      faces.all = fallback;
    }
  }
}

function hasRequiredFaces(kind: BlockKind, faces: CubeFaceTextures): boolean {
  if (kind === "cross") {
    return Boolean(faces.all);
  }
  if (kind === "door") {
    return Boolean(faces.top && faces.bottom);
  }
  if (kind === "pane") {
    return Boolean(faces.side);
  }
  if (kind === "torch") {
    return Boolean(faces.side || faces.all);
  }
  return Boolean(faces.top || faces.all || faces.side);
}

function blockNameFromResource(resource: string): string {
  return (resource.split(":")[1] ?? resource).replace(/_/gu, "-");
}
