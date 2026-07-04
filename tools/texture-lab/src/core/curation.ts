import fs from "node:fs/promises";
import path from "node:path";
import type { TexturePackAsset, TextureSpec } from "../dsl";
import { encodePng, decodePng, type RgbaImage } from "../png";
import { runtimeCompatTexturePath } from "../reference";
import { renderAllTextures, type RenderedTexture } from "../compositor";
import { makeLodMaterialsJson } from "../lod-materials";
import { assertTextureSourcePolicies } from "../source-policy";
import type {
  TextureCandidateEntry,
  TextureCurationSelectionEntry,
  TextureCurationStaleSelectionEntry,
  TextureCurationState,
  TextureImageRef,
} from "./index-model";

export const TEXTURE_CURATION_SCHEMA_VERSION = 1;
export const TEXTURE_CURATION_MANIFEST_RELATIVE_PATH = "curation/selections.v1.json";

export class TextureCurationError extends Error {
  constructor(
    message: string,
    readonly statusCode = 400,
  ) {
    super(message);
    this.name = "TextureCurationError";
  }
}

interface TextureCurationManifest {
  schemaVersion: typeof TEXTURE_CURATION_SCHEMA_VERSION;
  updatedAt: string | null;
  selections: Record<string, TextureCurationManifestSelection>;
}

interface TextureCurationManifestSelection {
  candidateId: string;
  selectedAt: string;
  codename?: string;
  source?: string;
  imagePath?: string | null;
  manifestPath?: string | null;
  projectionReportPath?: string | null;
}

export interface ApplyTextureCurationResult {
  manifestPath: string;
  outputRoot: string;
  selectedCount: number;
  applied: AppliedTextureSelection[];
}

export interface AppliedTextureSelection {
  textureName: string;
  candidateId: string;
  codename: string;
  imagePath: string;
  exportPath: string;
  runtimeCompatPath: string | null;
}

export function textureCurationManifestPath(outputRoot: string): string {
  return path.join(outputRoot, TEXTURE_CURATION_MANIFEST_RELATIVE_PATH);
}

export async function buildTextureCurationState(
  outputRoot: string,
  candidates: TextureCandidateEntry[],
): Promise<TextureCurationState> {
  const manifest = await readTextureCurationManifest(outputRoot);
  return textureCurationStateFromManifest(outputRoot, manifest, candidates);
}

export async function selectTextureCandidateForCuration(
  outputRoot: string,
  candidates: TextureCandidateEntry[],
  textureName: string,
  candidateId: string,
): Promise<TextureCurationState> {
  const candidate = candidates.find((entry) => entry.id === candidateId);
  if (!candidate) {
    throw new TextureCurationError(`Candidate '${candidateId}' not found`, 404);
  }
  if (candidate.textureName !== textureName) {
    throw new TextureCurationError(`Candidate '${candidateId}' is not linked to texture '${textureName}'`);
  }
  const image = promotableCandidateImage(candidate);
  if (!image?.path || !image.exists) {
    throw new TextureCurationError(`Candidate '${candidate.codename}' has no projected image that can be promoted`);
  }

  const now = new Date().toISOString();
  const manifest = await readTextureCurationManifest(outputRoot);
  manifest.updatedAt = now;
  manifest.selections[textureName] = {
    candidateId: candidate.id,
    selectedAt: now,
    codename: candidate.codename,
    source: candidate.source,
    imagePath: image.path,
    manifestPath: candidate.manifestPath,
    projectionReportPath: candidate.projectionReportPath,
  };
  await writeTextureCurationManifest(outputRoot, manifest);
  return textureCurationStateFromManifest(outputRoot, manifest, candidates);
}

export async function clearTextureCandidateCuration(
  outputRoot: string,
  candidates: TextureCandidateEntry[],
  textureName: string,
): Promise<TextureCurationState> {
  const manifest = await readTextureCurationManifest(outputRoot);
  if (manifest.selections[textureName]) {
    delete manifest.selections[textureName];
    manifest.updatedAt = new Date().toISOString();
    await writeTextureCurationManifest(outputRoot, manifest);
  }
  return textureCurationStateFromManifest(outputRoot, manifest, candidates);
}

export async function applyTextureCuration(options: {
  outputRoot: string;
  pack: TexturePackAsset;
  candidates: TextureCandidateEntry[];
}): Promise<ApplyTextureCurationResult> {
  const outputRoot = path.resolve(options.outputRoot);
  const state = await buildTextureCurationState(outputRoot, options.candidates);
  const baseTextures = renderAllTextures(options.pack);
  const activeTextures = new Map(baseTextures.map((texture) => [texture.name, texture]));
  const applied: AppliedTextureSelection[] = [];

  for (const selection of state.selections) {
    const texture = options.pack.textures[selection.textureName];
    const baseTexture = activeTextures.get(selection.textureName);
    if (!texture || !baseTexture) {
      throw new TextureCurationError(`Selected texture '${selection.textureName}' is no longer authored`);
    }
    if (!selection.image.path || !selection.image.exists) {
      throw new TextureCurationError(`Selected candidate '${selection.codename}' has no promotable projected image`);
    }

    const image = decodePng(await fs.readFile(selection.image.path));
    validatePromotedImage(selection.textureName, texture, baseTexture, image);
    const promotedTexture: RenderedTexture = {
      ...baseTexture,
      width: image.width,
      height: image.height,
      data: image.data,
    };
    activeTextures.set(selection.textureName, promotedTexture);
    applied.push({
      textureName: selection.textureName,
      candidateId: selection.candidateId,
      codename: selection.codename,
      imagePath: selection.image.path,
      exportPath: texture.exportPath,
      runtimeCompatPath: runtimeCompatTexturePath(texture.exportPath),
    });
  }

  const textures = [...activeTextures.values()];
  assertTextureSourcePolicies(options.pack, textures);
  await writeActiveTexturePack(outputRoot, textures, options.pack);
  return {
    manifestPath: textureCurationManifestPath(outputRoot),
    outputRoot,
    selectedCount: state.selectedCount,
    applied,
  };
}

function textureCurationStateFromManifest(
  outputRoot: string,
  manifest: TextureCurationManifest,
  candidates: TextureCandidateEntry[],
): TextureCurationState {
  const byId = new Map(candidates.map((candidate) => [candidate.id, candidate]));
  const selections: TextureCurationSelectionEntry[] = [];
  const staleSelections: TextureCurationStaleSelectionEntry[] = [];
  for (const [textureName, selection] of Object.entries(manifest.selections).sort(([left], [right]) => left.localeCompare(right))) {
    const candidate = byId.get(selection.candidateId);
    if (!candidate) {
      staleSelections.push({
        textureName,
        candidateId: selection.candidateId,
        selectedAt: selection.selectedAt,
        reason: "candidate artifact is missing from the current index",
      });
      continue;
    }
    if (candidate.textureName !== textureName) {
      staleSelections.push({
        textureName,
        candidateId: selection.candidateId,
        selectedAt: selection.selectedAt,
        reason: `candidate is linked to '${candidate.textureName ?? "none"}'`,
      });
      continue;
    }
    const image = promotableCandidateImage(candidate);
    selections.push({
      textureName,
      candidateId: candidate.id,
      codename: candidate.codename,
      source: candidate.source,
      selectedAt: selection.selectedAt,
      image: image ?? missingPromotableImage(),
    });
  }
  return {
    schemaVersion: TEXTURE_CURATION_SCHEMA_VERSION,
    manifestPath: textureCurationManifestPath(outputRoot),
    selectedCount: selections.length,
    selections,
    staleSelections,
  };
}

function promotableCandidateImage(candidate: TextureCandidateEntry): TextureImageRef | null {
  return candidate.images.projected.exists ? candidate.images.projected : null;
}

function missingPromotableImage(): TextureImageRef {
  return {
    label: "Projected",
    path: null,
    exists: false,
    missingCommand: "pnpm --dir tools/texture-lab project-diffusion",
  };
}

async function readTextureCurationManifest(outputRoot: string): Promise<TextureCurationManifest> {
  const manifestPath = textureCurationManifestPath(outputRoot);
  try {
    const parsed = JSON.parse(await fs.readFile(manifestPath, "utf8")) as Partial<TextureCurationManifest>;
    if (parsed.schemaVersion !== TEXTURE_CURATION_SCHEMA_VERSION || !parsed.selections || typeof parsed.selections !== "object") {
      throw new TextureCurationError(`Unsupported texture curation manifest '${manifestPath}'`);
    }
    return {
      schemaVersion: TEXTURE_CURATION_SCHEMA_VERSION,
      updatedAt: typeof parsed.updatedAt === "string" ? parsed.updatedAt : null,
      selections: parsed.selections as Record<string, TextureCurationManifestSelection>,
    };
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return {
        schemaVersion: TEXTURE_CURATION_SCHEMA_VERSION,
        updatedAt: null,
        selections: {},
      };
    }
    throw error;
  }
}

async function writeTextureCurationManifest(outputRoot: string, manifest: TextureCurationManifest): Promise<void> {
  const manifestPath = textureCurationManifestPath(outputRoot);
  await fs.mkdir(path.dirname(manifestPath), { recursive: true });
  await fs.writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
}

function validatePromotedImage(
  textureName: string,
  texture: TextureSpec,
  baseTexture: RenderedTexture,
  image: RgbaImage,
): void {
  const expectedSize = texture.size ?? baseTexture.width;
  if (image.width !== expectedSize || image.height !== expectedSize) {
    throw new TextureCurationError(
      `Candidate for '${textureName}' is ${image.width}x${image.height}, expected ${expectedSize}x${expectedSize}`,
    );
  }
}

async function writeActiveTexturePack(
  outputRoot: string,
  textures: RenderedTexture[],
  pack: TexturePackAsset,
): Promise<void> {
  for (const texture of textures) {
    const outputPath = path.join(outputRoot, "pack", texture.exportPath);
    await fs.mkdir(path.dirname(outputPath), { recursive: true });
    await fs.writeFile(outputPath, encodePng(texture));

    const compatPath = runtimeCompatTexturePath(texture.exportPath);
    if (compatPath) {
      const compatOutputPath = path.join(outputRoot, "runtime-pack", compatPath);
      await fs.mkdir(path.dirname(compatOutputPath), { recursive: true });
      await fs.writeFile(compatOutputPath, encodePng(texture));
    }
  }

  const lodMaterialsJson = makeLodMaterialsJson(pack, textures);
  const lodMaterialsReportPath = path.join(outputRoot, `${pack.name}-lod-materials.v1.json`);
  await fs.writeFile(lodMaterialsReportPath, lodMaterialsJson);
  const lodMaterialsPackPath = path.join(outputRoot, "runtime-pack", "assets/mclone/lod/materials.v1.json");
  await fs.mkdir(path.dirname(lodMaterialsPackPath), { recursive: true });
  await fs.writeFile(lodMaterialsPackPath, lodMaterialsJson);
}
