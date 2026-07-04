import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import type {
  TextureFrozenAsset,
  TextureFrozenMetadata,
  TextureFrozenSourceContext,
  TexturePackAsset,
  TextureSpec,
} from "../dsl";
import { repoRoot } from "../output-root";
import { decodePng, type RgbaImage } from "../png";
import { textureSourcePolicyErrors } from "../source-policy";
import type { RenderedTexture } from "../compositor";
import {
  TEXTURE_FREEZE_REQUEST_SCHEMA_VERSION,
  type FreezeRequestFileRef,
  type TextureFreezeRequestManifest,
} from "./freeze-request";

export const TEXTURE_FROZEN_CURATION_SCHEMA_VERSION = 1;
export const TEXTURE_FROZEN_CURATION_FILENAME = "curation.v1.json";
export const TEXTURE_FROZEN_ASSET_RELATIVE_DIR = "frozen";

const execFileAsync = promisify(execFile);

export class TextureFrozenCurationError extends Error {
  constructor(
    message: string,
    readonly statusCode = 400,
  ) {
    super(message);
    this.name = "TextureFrozenCurationError";
  }
}

export interface TextureFrozenCurationManifest {
  schemaVersion: typeof TEXTURE_FROZEN_CURATION_SCHEMA_VERSION;
  updatedAt: string | null;
  textures: Record<string, TextureFrozenCurationManifestEntry>;
}

export interface TextureFrozenCurationManifestEntry extends TextureFrozenMetadata {
  asset: string;
  sha256: string;
}

export interface PromoteFrozenTextureResult {
  manifestPath: string;
  textureName: string;
  assetPath: string;
  entry: TextureFrozenCurationManifestEntry;
}

export function frozenCurationManifestPathForInput(inputPath: string): string {
  return path.join(path.dirname(path.resolve(inputPath)), TEXTURE_FROZEN_CURATION_FILENAME);
}

export async function applyFrozenTextureCurationToPack(
  inputPath: string,
  pack: TexturePackAsset,
): Promise<TexturePackAsset> {
  const packDir = path.dirname(path.resolve(inputPath));
  const manifestPath = frozenCurationManifestPathForInput(inputPath);
  const manifest = await readFrozenTextureCurationManifest(packDir);
  const frozenTextures: Record<string, TextureFrozenAsset> = {};

  for (const [textureName, entry] of Object.entries(manifest.textures)) {
    const texture = pack.textures[textureName];
    if (!texture) {
      throw new TextureFrozenCurationError(
        `Frozen curation '${manifestPath}' references unknown texture '${textureName}'`,
      );
    }
    const assetPath = resolveFrozenAssetPath(packDir, entry.asset);
    const bytes = await fs.readFile(assetPath);
    const sha256 = sha256Hex(bytes);
    if (sha256 !== entry.sha256) {
      throw new TextureFrozenCurationError(
        `Frozen asset '${entry.asset}' hash mismatch: expected ${entry.sha256}, got ${sha256}`,
      );
    }
    const image = decodePng(bytes);
    validateFrozenImageShape(textureName, texture, pack, image);
    frozenTextures[textureName] = {
      asset: entry.asset,
      path: assetPath,
      sha256,
      width: image.width,
      height: image.height,
      data: image.data,
      metadata: metadataFromManifestEntry(entry),
    };
  }

  return {
    ...pack,
    frozenCurationPath: manifestPath,
    frozenTextures,
  };
}

export async function promoteFrozenTexture(options: {
  pack: TexturePackAsset;
  packInputPath: string;
  textureName: string;
  imagePath: string;
  asset?: string;
  metadata?: TextureFrozenMetadata;
}): Promise<PromoteFrozenTextureResult> {
  const packInputPath = path.resolve(options.packInputPath);
  const packDir = path.dirname(packInputPath);
  const texture = options.pack.textures[options.textureName];
  if (!texture) {
    throw new TextureFrozenCurationError(`Texture '${options.textureName}' is not authored`);
  }

  const sourceBytes = await fs.readFile(options.imagePath);
  const image = decodePng(sourceBytes);
  validateFrozenImageShape(options.textureName, texture, options.pack, image);
  validateFrozenImageSourcePolicy(options.textureName, texture, options.pack, image);

  const asset = options.asset ?? defaultFrozenAssetForTexture(options.textureName, texture);
  const assetPath = resolveFrozenAssetPath(packDir, asset);
  const sha256 = sha256Hex(sourceBytes);
  await fs.mkdir(path.dirname(assetPath), { recursive: true });
  await fs.writeFile(assetPath, sourceBytes);

  const manifest = await readFrozenTextureCurationManifest(packDir);
  const sourceContext = await mergeSourceContext(packInputPath, options.imagePath, sha256, options.metadata?.sourceContext);
  const entry: TextureFrozenCurationManifestEntry = compactEntry({
    asset,
    sha256,
    ...options.metadata,
    sourceContext,
  });
  manifest.updatedAt = new Date().toISOString();
  manifest.textures[options.textureName] = entry;
  await writeFrozenTextureCurationManifest(packDir, manifest);

  return {
    manifestPath: frozenCurationManifestPathForInput(packInputPath),
    textureName: options.textureName,
    assetPath,
    entry,
  };
}

export async function promoteFrozenTextureFromFreezeRequest(options: {
  pack: TexturePackAsset;
  packInputPath: string;
  requestPath: string;
  asset?: string;
}): Promise<PromoteFrozenTextureResult> {
  const requestPath = path.resolve(options.requestPath);
  const requestBytes = await fs.readFile(requestPath);
  const request = JSON.parse(Buffer.from(requestBytes).toString("utf8")) as TextureFreezeRequestManifest;
  validateFreezeRequestManifest(request, requestPath, options.pack);
  await verifyFreezeRequestFiles(request, requestPath);

  const metadata = metadataFromFreezeRequest(request, requestPath, sha256Hex(requestBytes));
  const promoteOptions: Parameters<typeof promoteFrozenTexture>[0] = {
    pack: options.pack,
    packInputPath: options.packInputPath,
    textureName: request.texture.name,
    imagePath: request.images.projected.path,
    metadata,
  };
  if (options.asset) {
    promoteOptions.asset = options.asset;
  }
  return promoteFrozenTexture(promoteOptions);
}

export async function readFrozenTextureCurationManifest(packDir: string): Promise<TextureFrozenCurationManifest> {
  const manifestPath = path.join(path.resolve(packDir), TEXTURE_FROZEN_CURATION_FILENAME);
  try {
    const parsed = JSON.parse(await fs.readFile(manifestPath, "utf8")) as Partial<TextureFrozenCurationManifest>;
    if (parsed.schemaVersion !== TEXTURE_FROZEN_CURATION_SCHEMA_VERSION || !parsed.textures || typeof parsed.textures !== "object") {
      throw new TextureFrozenCurationError(`Unsupported frozen texture curation manifest '${manifestPath}'`);
    }
    return {
      schemaVersion: TEXTURE_FROZEN_CURATION_SCHEMA_VERSION,
      updatedAt: typeof parsed.updatedAt === "string" ? parsed.updatedAt : null,
      textures: parsed.textures as Record<string, TextureFrozenCurationManifestEntry>,
    };
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return {
        schemaVersion: TEXTURE_FROZEN_CURATION_SCHEMA_VERSION,
        updatedAt: null,
        textures: {},
      };
    }
    throw error;
  }
}

export async function writeFrozenTextureCurationManifest(
  packDir: string,
  manifest: TextureFrozenCurationManifest,
): Promise<void> {
  const manifestPath = path.join(path.resolve(packDir), TEXTURE_FROZEN_CURATION_FILENAME);
  const sorted: TextureFrozenCurationManifest = {
    schemaVersion: TEXTURE_FROZEN_CURATION_SCHEMA_VERSION,
    updatedAt: manifest.updatedAt,
    textures: Object.fromEntries(
      Object.entries(manifest.textures).sort(([left], [right]) => left.localeCompare(right)),
    ),
  };
  await fs.writeFile(manifestPath, `${JSON.stringify(sorted, null, 2)}\n`);
}

export function defaultFrozenAssetForTexture(textureName: string, texture: TextureSpec): string {
  const prefixes = ["assets/mclone/textures/", "assets/mclone/lod/textures/"];
  for (const prefix of prefixes) {
    if (texture.exportPath.startsWith(prefix)) {
      return path.posix.join(TEXTURE_FROZEN_ASSET_RELATIVE_DIR, texture.exportPath.slice(prefix.length));
    }
  }
  return path.posix.join(TEXTURE_FROZEN_ASSET_RELATIVE_DIR, `${textureName}.png`);
}

export function sha256Hex(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function resolveFrozenAssetPath(packDir: string, asset: string): string {
  if (!asset || path.isAbsolute(asset) || asset.split(/[\\/]/).includes("..")) {
    throw new TextureFrozenCurationError(`Frozen asset path must be relative to the pack directory, got '${asset}'`);
  }
  const root = path.resolve(packDir);
  const resolved = path.resolve(root, asset);
  if (resolved !== root && !resolved.startsWith(`${root}${path.sep}`)) {
    throw new TextureFrozenCurationError(`Frozen asset '${asset}' resolves outside the pack directory`);
  }
  return resolved;
}

function validateFrozenImageShape(
  textureName: string,
  texture: TextureSpec,
  pack: TexturePackAsset,
  image: RgbaImage,
): void {
  const expectedSize = texture.size ?? pack.defaultSize;
  if (image.width !== expectedSize || image.height !== expectedSize) {
    throw new TextureFrozenCurationError(
      `Frozen asset for '${textureName}' is ${image.width}x${image.height}, expected ${expectedSize}x${expectedSize}`,
    );
  }
}

function validateFrozenImageSourcePolicy(
  textureName: string,
  texture: TextureSpec,
  pack: TexturePackAsset,
  image: RgbaImage,
): void {
  const source = texture.source ?? "final-color";
  const promotedTexture: RenderedTexture = {
    name: textureName,
    exportPath: texture.exportPath,
    source,
    preview: texture.preview,
    catalog: texture.catalog,
    authoring: [],
    width: image.width,
    height: image.height,
    data: image.data,
  };
  if (texture.tintRole) {
    promotedTexture.tintRole = texture.tintRole;
    promotedTexture.tint = pack.tints[texture.tintRole]!;
  }
  const errors = textureSourcePolicyErrors(pack, [promotedTexture]);
  if (errors.length > 0) {
    throw new TextureFrozenCurationError(errors.join("; "));
  }
}

function validateFreezeRequestManifest(
  request: TextureFreezeRequestManifest,
  requestPath: string,
  pack: TexturePackAsset,
): void {
  if (request.schemaVersion !== TEXTURE_FREEZE_REQUEST_SCHEMA_VERSION) {
    throw new TextureFrozenCurationError(`Unsupported freeze request '${requestPath}'`);
  }
  const texture = pack.textures[request.texture.name];
  if (!texture) {
    throw new TextureFrozenCurationError(`Freeze request '${requestPath}' references unknown texture '${request.texture.name}'`);
  }
  const expectedSize = texture.size ?? pack.defaultSize;
  const expectedSource = texture.source ?? "final-color";
  if (request.texture.exportPath !== texture.exportPath) {
    throw new TextureFrozenCurationError(
      `Freeze request '${requestPath}' exportPath changed: expected ${texture.exportPath}, got ${request.texture.exportPath}`,
    );
  }
  if (request.texture.size !== expectedSize) {
    throw new TextureFrozenCurationError(
      `Freeze request '${requestPath}' texture size changed: expected ${expectedSize}, got ${request.texture.size}`,
    );
  }
  if (request.texture.source !== expectedSource || request.texture.tintRole !== (texture.tintRole ?? null)) {
    throw new TextureFrozenCurationError(`Freeze request '${requestPath}' texture source policy changed`);
  }
  if (request.sourcePolicy.errors.length > 0) {
    throw new TextureFrozenCurationError(
      `Freeze request '${requestPath}' was created with source-policy errors: ${request.sourcePolicy.errors.join("; ")}`,
    );
  }
}

async function verifyFreezeRequestFiles(request: TextureFreezeRequestManifest, requestPath: string): Promise<void> {
  await verifyFreezeFileRef("projected image", request.images.projected);
  await verifyOptionalFreezeFileRef("raw image", request.images.raw);
  await verifyOptionalFreezeFileRef("raw tile image", request.images.rawTile);
  await verifyOptionalFreezeFileRef("review sheet", request.images.reviewSheet);
  await verifyOptionalFreezeFileRef("contact sheet", request.images.contactSheet);

  if (request.candidate.freezeReadiness === "archived" && !request.candidate.archivePath) {
    throw new TextureFrozenCurationError(`Freeze request '${requestPath}' is archived but has no archive manifest path`);
  }
  if (request.candidate.freezeReadiness === "archivable" && (!request.candidate.manifestPath || !request.candidate.projectionReportPath)) {
    throw new TextureFrozenCurationError(`Freeze request '${requestPath}' is archivable but is missing projection artifact paths`);
  }
  await verifyReferencedArtifact("archive manifest", request.candidate.archivePath);
  await verifyReferencedArtifact("diffusion manifest", request.candidate.manifestPath);
  await verifyReferencedArtifact("projection report", request.candidate.projectionReportPath);
}

async function verifyFreezeFileRef(label: string, ref: FreezeRequestFileRef): Promise<void> {
  const bytes = await fs.readFile(ref.path);
  const actual = sha256Hex(bytes);
  if (actual !== ref.sha256) {
    throw new TextureFrozenCurationError(`${label} '${ref.path}' hash mismatch: expected ${ref.sha256}, got ${actual}`);
  }
}

async function verifyOptionalFreezeFileRef(label: string, ref: FreezeRequestFileRef | null): Promise<void> {
  if (ref) {
    await verifyFreezeFileRef(label, ref);
  }
}

async function verifyReferencedArtifact(label: string, artifactPath: string | null): Promise<void> {
  if (!artifactPath) {
    return;
  }
  try {
    const stat = await fs.stat(artifactPath);
    if (!stat.isFile()) {
      throw new TextureFrozenCurationError(`${label} '${artifactPath}' is not a file`);
    }
  } catch (error) {
    if (error instanceof TextureFrozenCurationError) {
      throw error;
    }
    throw new TextureFrozenCurationError(`${label} '${artifactPath}' is missing`);
  }
}

function metadataFromFreezeRequest(
  request: TextureFreezeRequestManifest,
  requestPath: string,
  requestSha256: string,
): TextureFrozenMetadata {
  const metadata: TextureFrozenMetadata = {
    codename: request.candidate.codename,
    candidateId: request.candidate.candidateId,
    sourceContext: {
      projectionAssetSha256: request.images.projected.sha256,
      freezeRequest: requestPath,
      freezeRequestSha256: requestSha256,
      note: `Promoted from freeze request created at ${request.createdAt}.`,
    },
  };
  if (request.candidate.promptPreset) metadata.promptPreset = request.candidate.promptPreset;
  if (request.candidate.prompt) metadata.prompt = request.candidate.prompt;
  if (request.candidate.negativePrompt) metadata.negativePrompt = request.candidate.negativePrompt;
  if (request.candidate.modelId) metadata.modelId = request.candidate.modelId;
  if (request.candidate.scheduler) metadata.scheduler = request.candidate.scheduler;
  if (request.candidate.steps !== null) metadata.steps = request.candidate.steps;
  if (request.candidate.seed !== null) metadata.seed = request.candidate.seed;
  if (request.candidate.strength !== null) metadata.strength = request.candidate.strength;
  if (request.candidate.resolution !== null) metadata.resolution = request.candidate.resolution;
  if (request.candidate.archivePath) metadata.sourceContext!.archiveManifest = request.candidate.archivePath;
  if (request.candidate.manifestPath) metadata.sourceContext!.diffusionManifest = request.candidate.manifestPath;
  if (request.candidate.projectionReportPath) metadata.sourceContext!.projectionReport = request.candidate.projectionReportPath;
  if (request.images.raw) metadata.sourceContext!.rawAssetSha256 = request.images.raw.sha256;
  return metadata;
}

async function mergeSourceContext(
  packInputPath: string,
  sourceAsset: string,
  sourceAssetSha256: string,
  existing: TextureFrozenSourceContext | undefined,
): Promise<TextureFrozenSourceContext> {
  const packRelativePath = repoRelativePath(packInputPath);
  const existingContext = existing
    ? {
        ...existing,
        archiveManifest: normalizeContextPath(existing.archiveManifest),
        diffusionManifest: normalizeContextPath(existing.diffusionManifest),
        projectionReport: normalizeContextPath(existing.projectionReport),
        freezeRequest: normalizeContextPath(existing.freezeRequest),
        sourceAsset: normalizeContextPath(existing.sourceAsset),
      }
    : undefined;
  return compactObject({
    gitCommit: await gitOutput(["rev-parse", "HEAD"]),
    packTreeGitCommit: await gitOutput(["log", "-1", "--format=%H", "--", path.dirname(packRelativePath)]),
    packInputPath: packRelativePath,
    sourceAsset: repoRelativePath(sourceAsset),
    sourceAssetSha256,
    ...existingContext,
  }) as TextureFrozenSourceContext;
}

function metadataFromManifestEntry(entry: TextureFrozenCurationManifestEntry): TextureFrozenMetadata {
  const metadata: TextureFrozenMetadata = {};
  if (entry.codename) metadata.codename = entry.codename;
  if (entry.candidateId) metadata.candidateId = entry.candidateId;
  if (entry.promptPreset) metadata.promptPreset = entry.promptPreset;
  if (entry.prompt) metadata.prompt = entry.prompt;
  if (entry.negativePrompt) metadata.negativePrompt = entry.negativePrompt;
  if (entry.modelId) metadata.modelId = entry.modelId;
  if (entry.scheduler) metadata.scheduler = entry.scheduler;
  if (entry.steps !== undefined) metadata.steps = entry.steps;
  if (entry.seed !== undefined) metadata.seed = entry.seed;
  if (entry.strength !== undefined) metadata.strength = entry.strength;
  if (entry.resolution !== undefined) metadata.resolution = entry.resolution;
  if (entry.sourceContext) metadata.sourceContext = entry.sourceContext;
  return metadata;
}

function compactEntry<T extends Record<string, unknown>>(entry: T): T {
  return compactObject(entry) as T;
}

function compactObject<T extends Record<string, unknown>>(entry: T): Partial<T> {
  const result: Partial<T> = {};
  for (const [key, value] of Object.entries(entry) as [keyof T, T[keyof T]][]) {
    if (value !== undefined && value !== null && value !== "") {
      result[key] = value;
    }
  }
  return result;
}

function repoRelativePath(filePath: string): string {
  const relative = path.relative(repoRoot(), path.resolve(filePath));
  return relative.startsWith("..") ? path.resolve(filePath) : relative;
}

function normalizeContextPath(value: string | undefined): string | undefined {
  if (!value || !path.isAbsolute(value)) {
    return value;
  }
  return repoRelativePath(value);
}

async function gitOutput(args: string[]): Promise<string | undefined> {
  try {
    const { stdout } = await execFileAsync("git", args, { cwd: repoRoot() });
    return stdout.trim() || undefined;
  } catch {
    return undefined;
  }
}
