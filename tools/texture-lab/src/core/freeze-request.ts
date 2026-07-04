import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import type { TexturePackAsset } from "../dsl";
import { renderAllTextures, type RenderedTexture } from "../compositor";
import { decodePng } from "../png";
import { runtimeCompatTexturePath } from "../reference";
import { sourceNeutralityStats, textureSourcePolicyErrors, type SourceNeutralityStats } from "../source-policy";
import { buildTextureCurationState } from "./curation";
import type { TextureCandidateEntry, TextureCandidateSource, TextureImageRef } from "./index-model";

export const TEXTURE_FREEZE_REQUEST_SCHEMA_VERSION = 1;
export const TEXTURE_FREEZE_REQUESTS_RELATIVE_DIR = "freeze-requests";

export class TextureFreezeRequestError extends Error {
  constructor(
    message: string,
    readonly statusCode = 400,
  ) {
    super(message);
    this.name = "TextureFreezeRequestError";
  }
}

export interface TextureFreezeRequestManifest {
  schemaVersion: typeof TEXTURE_FREEZE_REQUEST_SCHEMA_VERSION;
  createdAt: string;
  path: string;
  texture: {
    name: string;
    exportPath: string;
    runtimeCompatPath: string | null;
    size: number;
    source: string;
    tintRole: string | null;
    palette: string;
    base: string;
  };
  candidate: {
    id: string;
    candidateId: string;
    codename: string;
    source: TextureCandidateSource;
    freezeReadiness: "archived" | "archivable";
    archived: boolean;
    artifactRoot: string;
    archivePath: string | null;
    manifestPath: string | null;
    projectionReportPath: string | null;
    promptPreset: string | null;
    prompt: string | null;
    negativePrompt: string | null;
    modelId: string | null;
    scheduler: string | null;
    steps: number | null;
    seed: number | null;
    strength: number | null;
    resolution: number | null;
    resolutions: number[];
    paletteColors: string[];
    status: string | null;
    score: number | null;
    reasons: string[];
  };
  images: {
    projected: FreezeRequestFileRef;
    raw: FreezeRequestFileRef | null;
    rawTile: FreezeRequestFileRef | null;
    reviewSheet: FreezeRequestFileRef | null;
    contactSheet: FreezeRequestFileRef | null;
  };
  sourcePolicy: {
    source: string;
    tintRole: string | null;
    sourceNeutrality: unknown;
    stats: SourceNeutralityStats | null;
    errors: string[];
  };
  sourcePatch: {
    mode: "agent-or-cli-mediated";
    packInputPath: string;
    sourceFileHint: string | null;
    textureName: string;
    intendedSourceShape: string;
  };
}

export interface FreezeRequestFileRef {
  path: string;
  sha256: string;
}

export async function createTextureFreezeRequest(options: {
  outputRoot: string;
  pack: TexturePackAsset;
  packInputPath: string;
  candidates: TextureCandidateEntry[];
  textureName: string;
}): Promise<TextureFreezeRequestManifest> {
  const outputRoot = path.resolve(options.outputRoot);
  const curation = await buildTextureCurationState(outputRoot, options.candidates);
  const selection = curation.selections.find((entry) => entry.textureName === options.textureName);
  if (!selection) {
    throw new TextureFreezeRequestError(`Texture '${options.textureName}' has no selected pack candidate`);
  }

  const candidate = options.candidates.find((entry) => entry.id === selection.candidateId);
  if (!candidate) {
    throw new TextureFreezeRequestError(`Selected candidate '${selection.candidateId}' is no longer indexed`, 404);
  }
  const readiness = freezeReadiness(candidate);
  const texture = options.pack.textures[options.textureName];
  if (!texture) {
    throw new TextureFreezeRequestError(`Texture '${options.textureName}' is no longer authored`, 404);
  }
  if (!candidate.images.projected.path || !candidate.images.projected.exists) {
    throw new TextureFreezeRequestError(`Candidate '${candidate.codename}' has no projected PNG to freeze`);
  }

  const rendered = renderAllTextures(options.pack).find((entry) => entry.name === options.textureName);
  if (!rendered) {
    throw new TextureFreezeRequestError(`Texture '${options.textureName}' did not render`);
  }
  const projectedImage = decodePng(await fs.readFile(candidate.images.projected.path));
  if (projectedImage.width !== rendered.width || projectedImage.height !== rendered.height) {
    throw new TextureFreezeRequestError(
      `Candidate '${candidate.codename}' is ${projectedImage.width}x${projectedImage.height}, expected ${rendered.width}x${rendered.height}`,
    );
  }
  const promotedTexture: RenderedTexture = {
    ...rendered,
    width: projectedImage.width,
    height: projectedImage.height,
    data: projectedImage.data,
  };
  const sourcePolicyErrors = textureSourcePolicyErrors(options.pack, [promotedTexture]);
  if (sourcePolicyErrors.length > 0) {
    throw new TextureFreezeRequestError(sourcePolicyErrors.join("; "));
  }

  const createdAt = new Date().toISOString();
  const requestPath = freezeRequestPath(outputRoot, options.textureName, candidate.codename, createdAt);
  const manifest: TextureFreezeRequestManifest = {
    schemaVersion: TEXTURE_FREEZE_REQUEST_SCHEMA_VERSION,
    createdAt,
    path: requestPath,
    texture: {
      name: options.textureName,
      exportPath: texture.exportPath,
      runtimeCompatPath: runtimeCompatTexturePath(texture.exportPath),
      size: texture.size ?? options.pack.defaultSize,
      source: texture.source ?? "final-color",
      tintRole: texture.tintRole ?? null,
      palette: texture.palette,
      base: texture.base,
    },
    candidate: {
      id: candidate.id,
      candidateId: candidate.candidateId,
      codename: candidate.codename,
      source: candidate.source,
      freezeReadiness: readiness,
      archived: candidate.archived,
      artifactRoot: candidate.artifactRoot,
      archivePath: candidate.archivePath,
      manifestPath: candidate.manifestPath,
      projectionReportPath: candidate.projectionReportPath,
      promptPreset: candidate.promptPreset,
      prompt: candidate.prompt,
      negativePrompt: candidate.negativePrompt,
      modelId: candidate.modelId,
      scheduler: candidate.scheduler,
      steps: candidate.steps,
      seed: candidate.seed,
      strength: candidate.strength,
      resolution: candidate.resolution,
      resolutions: candidate.resolutions,
      paletteColors: candidate.paletteColors,
      status: candidate.status,
      score: candidate.score,
      reasons: candidate.reasons,
    },
    images: {
      projected: await freezeFileRef(candidate.images.projected),
      raw: await optionalFreezeFileRef(candidate.images.raw),
      rawTile: await optionalFreezeFileRef(candidate.images.rawTile),
      reviewSheet: await optionalFreezeFileRef(candidate.images.reviewSheet),
      contactSheet: await optionalFreezeFileRef(candidate.images.contactSheet),
    },
    sourcePolicy: {
      source: promotedTexture.source,
      tintRole: promotedTexture.tintRole ?? null,
      sourceNeutrality: texture.tintRole ? options.pack.tints[texture.tintRole]?.sourceNeutrality ?? null : null,
      stats: promotedTexture.source === "tintable" ? sourceNeutralityStats(promotedTexture) : null,
      errors: sourcePolicyErrors,
    },
    sourcePatch: {
      mode: "agent-or-cli-mediated",
      packInputPath: path.resolve(options.packInputPath),
      sourceFileHint: sourceFileHintForTexture(options.textureName),
      textureName: options.textureName,
      intendedSourceShape: "Promote into the canonical curation.v1.json plus a committed frozen PNG asset; do not patch TypeScript texture definitions by default.",
    },
  };

  await fs.mkdir(path.dirname(requestPath), { recursive: true });
  await fs.writeFile(requestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  return manifest;
}

export async function listTextureFreezeRequests(outputRoot: string, textureName?: string): Promise<TextureFreezeRequestManifest[]> {
  const root = path.join(path.resolve(outputRoot), TEXTURE_FREEZE_REQUESTS_RELATIVE_DIR);
  let entries: string[];
  try {
    entries = await fs.readdir(root);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return [];
    }
    throw error;
  }
  const requests: TextureFreezeRequestManifest[] = [];
  for (const entry of entries.sort((left, right) => left.localeCompare(right))) {
    if (!entry.endsWith(".json")) {
      continue;
    }
    const filePath = path.join(root, entry);
    const request = JSON.parse(await fs.readFile(filePath, "utf8")) as TextureFreezeRequestManifest;
    if (request.schemaVersion === TEXTURE_FREEZE_REQUEST_SCHEMA_VERSION && (!textureName || request.texture.name === textureName)) {
      requests.push(request);
    }
  }
  return requests.sort((left, right) => right.createdAt.localeCompare(left.createdAt));
}

function freezeReadiness(candidate: TextureCandidateEntry): "archived" | "archivable" {
  if (candidate.archived && candidate.archivePath) {
    return "archived";
  }
  if (candidate.source === "projection" && candidate.manifestPath && candidate.projectionReportPath) {
    return "archivable";
  }
  throw new TextureFreezeRequestError(
    `Candidate '${candidate.codename}' must be archived or have projection artifacts before a freeze request can be written`,
  );
}

async function freezeFileRef(image: TextureImageRef): Promise<FreezeRequestFileRef> {
  if (!image.path || !image.exists) {
    throw new TextureFreezeRequestError(`Missing required ${image.label} image for freeze request`);
  }
  return {
    path: image.path,
    sha256: sha256Hex(await fs.readFile(image.path)),
  };
}

async function optionalFreezeFileRef(image: TextureImageRef): Promise<FreezeRequestFileRef | null> {
  return image.path && image.exists ? await freezeFileRef(image) : null;
}

function freezeRequestPath(outputRoot: string, textureName: string, codename: string, createdAt: string): string {
  const stamp = createdAt.replace(/[^0-9A-Za-z]+/g, "-").replace(/-$/g, "");
  return path.join(
    outputRoot,
    TEXTURE_FREEZE_REQUESTS_RELATIVE_DIR,
    `${sanitizeFileStem(textureName)}-${sanitizeFileStem(codename)}-${stamp}.freeze-request.v1.json`,
  );
}

function sanitizeFileStem(value: string): string {
  return value.replace(/[^a-zA-Z0-9._-]+/g, "-");
}

function sha256Hex(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function sourceFileHintForTexture(textureName: string): string | null {
  if (textureName.startsWith("grass_block_")) {
    return "tools/texture-lab/packs/mclone-default/block/grass-block.ts";
  }
  if (textureName === "dirt" || textureName.includes("dirt")) {
    return "tools/texture-lab/packs/mclone-default/block/dirt.ts";
  }
  if (textureName.includes("stone") || textureName.includes("_ore") || textureName === "coal_ore" || textureName === "iron_ore") {
    return "tools/texture-lab/packs/mclone-default/block/stone.ts";
  }
  if (textureName.endsWith("_lod")) {
    return "tools/texture-lab/packs/mclone-default/block/far-lod-materials.ts";
  }
  return null;
}
