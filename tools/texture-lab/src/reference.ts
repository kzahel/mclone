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
// This is also the path a runtime-compat export installs the texture at, so it
// must only match textures that genuinely replace their vanilla counterpart.
export function runtimeCompatTexturePath(exportPath: string): string | null {
  const authoredPrefix = "assets/mclone/textures/block/";
  if (!exportPath.startsWith(authoredPrefix)) {
    return null;
  }
  return `assets/minecraft/textures/block/${exportPath.slice(authoredPrefix.length)}`;
}

// Textures whose vanilla counterpart lives under a different file name. Only
// static equivalents belong here — animated strips (water_still, lava_still,
// magma) would be squashed by the shared-grid downsample and measured as
// garbage, so they are deliberately left without a reference.
const REFERENCE_NAME_ALIASES = new Map<string, string>([
  ["grass_block_bottom", "dirt"],
  ["snow_block", "snow"],
]);

// The vanilla counterpart used for analysis and review-sheet comparison.
// Broader than the runtime-compat mapping: far-LOD material tiles install under
// assets/mclone/lod/... and are NOT runtime replacements for vanilla textures,
// but they target a vanilla material's look, so they still have a counterpart
// to measure against. The _lod authoring suffix is stripped; vanilla log side
// textures carry no _side suffix (oak_log.png is the side face).
export function referenceTexturePath(exportPath: string): string | null {
  const authoredPrefix = "assets/mclone/textures/block/";
  const lodPrefix = "assets/mclone/lod/textures/block/";
  let name: string;
  if (exportPath.startsWith(authoredPrefix)) {
    name = exportPath.slice(authoredPrefix.length).replace(/\.png$/i, "");
  } else if (exportPath.startsWith(lodPrefix)) {
    name = exportPath
      .slice(lodPrefix.length)
      .replace(/\.png$/i, "")
      .replace(/_lod$/, "")
      .replace(/_log_side$/, "_log");
  } else {
    return null;
  }
  name = REFERENCE_NAME_ALIASES.get(name) ?? name;
  return `assets/minecraft/textures/block/${name}.png`;
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

export async function findReferenceTextureFile(
  exportPath: string,
  options: Pick<ReferenceOptions, "referenceRoot"> = {},
): Promise<string | null> {
  const counterpartPath = referenceTexturePath(exportPath);
  if (!counterpartPath) {
    return null;
  }
  for (const root of referenceRoots(options.referenceRoot)) {
    const referencePath = path.join(root, counterpartPath);
    try {
      const stat = await fs.stat(referencePath);
      if (stat.isFile()) {
        return referencePath;
      }
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code;
      if (code !== "ENOENT") {
        throw new Error(`Failed to inspect reference texture '${referencePath}': ${(error as Error).message}`);
      }
    }
  }
  return null;
}

export async function loadReferenceTexture(
  exportPath: string,
  options: ReferenceOptions = {},
): Promise<RgbaImage | undefined> {
  if (options.include === false) {
    return undefined;
  }
  const counterpartPath = referenceTexturePath(exportPath);
  if (!counterpartPath) {
    return undefined;
  }
  for (const root of referenceRoots(options.referenceRoot)) {
    const referencePath = path.join(root, counterpartPath);
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
