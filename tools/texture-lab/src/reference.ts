import fs from "node:fs/promises";
import path from "node:path";
import { decodePng, type RgbaImage } from "./png";

export interface ReferenceOptions {
  include?: boolean | undefined;
  referenceRoot?: string | undefined;
}

// Authored mclone textures export under assets/mclone/...; the local vanilla
// extraction stores the same block under assets/minecraft/.... This maps one to
// the other so a candidate texture can find its private vanilla reference.
export function runtimeCompatTexturePath(exportPath: string): string | null {
  const authoredPrefix = "assets/mclone/textures/block/";
  if (!exportPath.startsWith(authoredPrefix)) {
    return null;
  }
  return `assets/minecraft/textures/block/${exportPath.slice(authoredPrefix.length)}`;
}

export function referenceRoots(referenceRoot?: string): string[] {
  if (referenceRoot) {
    return [path.resolve(referenceRoot)];
  }
  return [
    path.resolve("..", "..", "reference", "minecraft-1.17.1", "extracted"),
    path.resolve("..", "..", "reference", "minecraft-1.17.1", "src"),
  ];
}

export async function loadReferenceTexture(
  exportPath: string,
  options: ReferenceOptions = {},
): Promise<RgbaImage | undefined> {
  if (options.include === false) {
    return undefined;
  }
  const compatPath = runtimeCompatTexturePath(exportPath);
  if (!compatPath) {
    return undefined;
  }
  for (const root of referenceRoots(options.referenceRoot)) {
    const referencePath = path.join(root, compatPath);
    try {
      return decodePng(await fs.readFile(referencePath));
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code;
      if (code !== "ENOENT") {
        throw new Error(`Failed to load reference texture '${referencePath}': ${(error as Error).message}`);
      }
    }
  }
  return undefined;
}
