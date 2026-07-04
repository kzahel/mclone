import fs from "node:fs/promises";
import path from "node:path";
import { drawScaled, solidImage } from "./image";
import { decodePng, encodePng, type RgbaImage } from "./png";

export interface ReferenceOptions {
  include?: boolean | undefined;
  referenceRoot?: string | undefined;
}

export type ReferenceCounterpart = SingleReferenceCounterpart | CompositeReferenceCounterpart;

export interface SingleReferenceCounterpart {
  kind: "single";
  path: string;
}

export interface CompositeReferenceCounterpart {
  kind: "composite";
  id: string;
  columns: number;
  parts: CompositeReferencePart[];
}

export interface CompositeReferencePart {
  label: string;
  path: string;
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

// Vanilla blocks whose material is not a single block texture. These entries
// encode the blockstate/model texture fanout as a compact counterpart grid so
// the lab can compare against the real set instead of pretending the reference
// is missing or choosing one arbitrary face.
const SPECIAL_REFERENCE_COUNTERPARTS = new Map<string, ReferenceCounterpart>([
  [
    "pointed_dripstone",
    {
      kind: "composite",
      id: "pointed_dripstone-up-down-thickness",
      columns: 2,
      parts: [
        referencePart("down/base", "pointed_dripstone_down_base"),
        referencePart("up/base", "pointed_dripstone_up_base"),
        referencePart("down/frustum", "pointed_dripstone_down_frustum"),
        referencePart("up/frustum", "pointed_dripstone_up_frustum"),
        referencePart("down/middle", "pointed_dripstone_down_middle"),
        referencePart("up/middle", "pointed_dripstone_up_middle"),
        referencePart("down/tip", "pointed_dripstone_down_tip"),
        referencePart("up/tip", "pointed_dripstone_up_tip"),
        referencePart("down/tip_merge", "pointed_dripstone_down_tip_merge"),
        referencePart("up/tip_merge", "pointed_dripstone_up_tip_merge"),
      ],
    },
  ],
]);

// The vanilla counterpart used for analysis and review-sheet comparison.
// Broader than the runtime-compat mapping: far-LOD material tiles install under
// assets/mclone/lod/... and are NOT runtime replacements for vanilla textures,
// but they target a vanilla material's look, so they still have a counterpart
// to measure against. The _lod authoring suffix is stripped; vanilla log side
// textures carry no _side suffix (oak_log.png is the side face).
export function referenceTexturePath(exportPath: string): string | null {
  const counterpart = referenceCounterpart(exportPath);
  return counterpart?.kind === "single" ? counterpart.path : null;
}

export function referenceCounterpart(exportPath: string): ReferenceCounterpart | null {
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
  return SPECIAL_REFERENCE_COUNTERPARTS.get(name) ?? {
    kind: "single",
    path: `assets/minecraft/textures/block/${name}.png`,
  };
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
  const counterpart = referenceCounterpart(exportPath);
  if (!counterpart || counterpart.kind !== "single") {
    return null;
  }
  const counterpartPath = counterpart.path;
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

export async function findOrCreateReferencePreviewFile(
  exportPath: string,
  outputRoot: string,
  options: Pick<ReferenceOptions, "referenceRoot"> = {},
): Promise<string | null> {
  const counterpart = referenceCounterpart(exportPath);
  if (!counterpart) {
    return null;
  }
  if (counterpart.kind === "single") {
    return findReferenceTextureFile(exportPath, options);
  }
  const reference = await loadReferenceTexture(exportPath, options);
  if (!reference) {
    return null;
  }
  const outputPath = path.join(path.resolve(outputRoot), "reference-composites", `${sanitizeFileStem(counterpart.id)}.png`);
  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await fs.writeFile(outputPath, encodePng(reference));
  return outputPath;
}

export async function loadReferenceTexture(
  exportPath: string,
  options: ReferenceOptions = {},
): Promise<RgbaImage | undefined> {
  if (options.include === false) {
    return undefined;
  }
  const counterpart = referenceCounterpart(exportPath);
  if (!counterpart) {
    return undefined;
  }
  if (counterpart.kind === "composite") {
    return loadCompositeReferenceTexture(counterpart, options);
  }
  const counterpartPath = counterpart.path;
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

async function loadCompositeReferenceTexture(
  counterpart: CompositeReferenceCounterpart,
  options: ReferenceOptions,
): Promise<RgbaImage | undefined> {
  for (const root of referenceRoots(options.referenceRoot)) {
    const parts: RgbaImage[] = [];
    let complete = true;
    for (const part of counterpart.parts) {
      try {
        parts.push(decodePng(await fs.readFile(path.join(root, part.path))));
      } catch (error) {
        const code = (error as NodeJS.ErrnoException).code;
        if (code !== "ENOENT") {
          throw new Error(`Failed to load reference texture '${part.path}': ${(error as Error).message}`);
        }
        complete = false;
        break;
      }
    }
    if (complete && parts.length === counterpart.parts.length) {
      return composeReferenceParts(counterpart, parts);
    }
  }
  return undefined;
}

function composeReferenceParts(counterpart: CompositeReferenceCounterpart, parts: RgbaImage[]): RgbaImage {
  const tileWidth = Math.max(...parts.map((part) => part.width));
  const tileHeight = Math.max(...parts.map((part) => part.height));
  const columns = Math.max(1, counterpart.columns);
  const rows = Math.ceil(parts.length / columns);
  const image = solidImage(tileWidth * columns, tileHeight * rows, [0, 0, 0, 0]);
  for (const [index, part] of parts.entries()) {
    const x = (index % columns) * tileWidth;
    const y = Math.floor(index / columns) * tileHeight;
    drawScaled(image, part, x, y, 1);
  }
  return image;
}

function referencePart(label: string, name: string): CompositeReferencePart {
  return {
    label,
    path: `assets/minecraft/textures/block/${name}.png`,
  };
}

function sanitizeFileStem(value: string): string {
  return value.replace(/[^a-zA-Z0-9._-]+/g, "-");
}
