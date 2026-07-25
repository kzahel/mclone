import fs from "node:fs/promises";
import path from "node:path";
import type {
  TextureFrozenMetadata,
  TextureLifecycleBinding,
  TextureLifecycleState,
  TexturePackAsset,
  TexturePromotedAsset,
  TextureSpec,
} from "../dsl";
import type { RenderedTexture } from "../compositor";
import { decodePng } from "../png";
import { textureSourcePolicyErrors } from "../source-policy";
import {
  defaultFrozenAssetForTexture,
  promoteFrozenTexture,
  sha256Hex,
  TextureFrozenCurationError,
} from "./frozen-curation";

export const TEXTURE_LIFECYCLE_SCHEMA_VERSION = 1;
export const TEXTURE_LIFECYCLE_FILENAME = "lifecycle.v1.json";
export const TEXTURE_PROVISIONAL_ASSET_RELATIVE_DIR = "provisional";

export class TextureLifecycleError extends Error {
  constructor(
    message: string,
    readonly statusCode = 400,
  ) {
    super(message);
    this.name = "TextureLifecycleError";
  }
}

export interface TextureLifecycleManifest {
  schemaVersion: typeof TEXTURE_LIFECYCLE_SCHEMA_VERSION;
  updatedAt: string | null;
  textures: Record<string, TextureLifecycleManifestEntry>;
}

export interface TextureLifecycleManifestEntry extends TextureLifecycleBinding {
  codename?: string;
  candidateId?: string;
}

export interface PromoteTextureLifecycleResult {
  manifestPath: string;
  textureName: string;
  assetPath: string;
  entry: TextureLifecycleManifestEntry;
}

export function lifecycleManifestPathForInput(inputPath: string): string {
  return path.join(path.dirname(path.resolve(inputPath)), TEXTURE_LIFECYCLE_FILENAME);
}

export async function applyTextureLifecycleToPack(
  inputPath: string,
  pack: TexturePackAsset,
): Promise<TexturePackAsset> {
  const packDir = path.dirname(path.resolve(inputPath));
  const manifestPath = lifecycleManifestPathForInput(inputPath);
  const manifest = await readTextureLifecycleManifest(packDir);
  const lifecycleTextures: Record<string, TexturePromotedAsset> = {};
  const textureLifecycle: Record<string, TextureLifecycleBinding> = {};

  for (const [textureName, entry] of Object.entries(manifest.textures)) {
    const texture = pack.textures[textureName];
    if (!texture) {
      throw new TextureLifecycleError(
        `Texture lifecycle '${manifestPath}' references unknown texture '${textureName}'`,
      );
    }
    validateRuntimeMaterials(textureName, entry.runtimeMaterials);
    const assetPath = resolveLifecycleAssetPath(packDir, entry.asset);
    const bytes = await fs.readFile(assetPath);
    const sha256 = sha256Hex(bytes);
    if (sha256 !== entry.sha256) {
      throw new TextureLifecycleError(
        `Lifecycle asset '${entry.asset}' hash mismatch: expected ${entry.sha256}, got ${sha256}`,
      );
    }
    const image = decodePng(bytes);
    validateLifecycleImageShape(textureName, texture, pack, image);
    if (entry.state === "curated") {
      const frozen = pack.frozenTextures?.[textureName];
      if (!frozen || frozen.asset !== entry.asset || frozen.sha256 !== entry.sha256) {
        throw new TextureLifecycleError(
          `Curated lifecycle texture '${textureName}' must match its frozen curation entry`,
        );
      }
    }

    textureLifecycle[textureName] = bindingFromEntry(entry);
    lifecycleTextures[textureName] = {
      asset: entry.asset,
      path: assetPath,
      sha256,
      width: image.width,
      height: image.height,
      data: image.data,
      metadata: metadataFromEntry(entry),
      state: entry.state,
      runtimeMaterials: [...entry.runtimeMaterials],
    };
  }

  return {
    ...pack,
    lifecycleManifestPath: manifestPath,
    textureLifecycle,
    lifecycleTextures,
  };
}

export async function promoteTextureLifecycle(options: {
  pack: TexturePackAsset;
  packInputPath: string;
  textureName: string;
  imagePath: string;
  state: TextureLifecycleState;
  runtimeMaterials?: string[];
  metadata?: TextureFrozenMetadata;
}): Promise<PromoteTextureLifecycleResult> {
  const packInputPath = path.resolve(options.packInputPath);
  const packDir = path.dirname(packInputPath);
  const texture = options.pack.textures[options.textureName];
  if (!texture) {
    throw new TextureLifecycleError(`Texture '${options.textureName}' is not authored`);
  }
  if (isLegacyDerivedTexture(texture)) {
    throw new TextureLifecycleError(
      `Texture '${options.textureName}' is a legacy Far LOD tile; Far LOD is derived from resolved runtime materials`,
    );
  }

  const manifest = await readTextureLifecycleManifest(packDir);
  const existing = manifest.textures[options.textureName];
  const runtimeMaterials = options.runtimeMaterials ?? existing?.runtimeMaterials ?? [];
  validateRuntimeMaterials(options.textureName, runtimeMaterials);

  let asset: string;
  let assetPath: string;
  let sha256: string;
  if (options.state === "curated") {
    let frozen;
    try {
      const promoteOptions: Parameters<typeof promoteFrozenTexture>[0] = {
        pack: options.pack,
        packInputPath,
        textureName: options.textureName,
        imagePath: options.imagePath,
      };
      if (options.metadata) {
        promoteOptions.metadata = options.metadata;
      }
      frozen = await promoteFrozenTexture(promoteOptions);
    } catch (error) {
      if (error instanceof TextureFrozenCurationError) {
        throw new TextureLifecycleError(error.message, error.statusCode);
      }
      throw error;
    }
    asset = frozen.entry.asset;
    assetPath = frozen.assetPath;
    sha256 = frozen.entry.sha256;
  } else {
    const sourceBytes = await fs.readFile(options.imagePath);
    const image = decodePng(sourceBytes);
    validateLifecycleImageShape(options.textureName, texture, options.pack, image);
    validateLifecycleImageSourcePolicy(options.textureName, texture, options.pack, image);
    asset = defaultProvisionalAssetForTexture(options.textureName, texture);
    assetPath = resolveLifecycleAssetPath(packDir, asset);
    sha256 = sha256Hex(sourceBytes);
    await fs.mkdir(path.dirname(assetPath), { recursive: true });
    await fs.writeFile(assetPath, sourceBytes);
  }

  const entry = compactEntry({
    state: options.state,
    runtimeMaterials: [...runtimeMaterials].sort(),
    asset,
    sha256,
    codename: options.metadata?.codename,
    candidateId: options.metadata?.candidateId,
  }) as TextureLifecycleManifestEntry;
  manifest.updatedAt = new Date().toISOString();
  manifest.textures[options.textureName] = entry;
  await writeTextureLifecycleManifest(packDir, manifest);
  return {
    manifestPath: lifecycleManifestPathForInput(packInputPath),
    textureName: options.textureName,
    assetPath,
    entry,
  };
}

export async function returnTextureLifecycleToCandidate(
  packInputPath: string,
  textureName: string,
): Promise<void> {
  const packDir = path.dirname(path.resolve(packInputPath));
  const manifest = await readTextureLifecycleManifest(packDir);
  if (!manifest.textures[textureName]) {
    return;
  }
  delete manifest.textures[textureName];
  manifest.updatedAt = new Date().toISOString();
  await writeTextureLifecycleManifest(packDir, manifest);
}

export async function readTextureLifecycleManifest(packDir: string): Promise<TextureLifecycleManifest> {
  const manifestPath = path.join(path.resolve(packDir), TEXTURE_LIFECYCLE_FILENAME);
  try {
    const parsed = JSON.parse(await fs.readFile(manifestPath, "utf8")) as Partial<TextureLifecycleManifest>;
    if (
      parsed.schemaVersion !== TEXTURE_LIFECYCLE_SCHEMA_VERSION ||
      !parsed.textures ||
      typeof parsed.textures !== "object"
    ) {
      throw new TextureLifecycleError(`Unsupported texture lifecycle manifest '${manifestPath}'`);
    }
    const textures = parsed.textures as Record<string, TextureLifecycleManifestEntry>;
    for (const [textureName, entry] of Object.entries(textures)) {
      if (entry.state !== "provisional" && entry.state !== "curated") {
        throw new TextureLifecycleError(
          `Texture lifecycle '${manifestPath}' has invalid state '${String(entry.state)}' for '${textureName}'`,
        );
      }
      if (typeof entry.asset !== "string" || typeof entry.sha256 !== "string") {
        throw new TextureLifecycleError(
          `Texture lifecycle '${manifestPath}' has an invalid asset for '${textureName}'`,
        );
      }
      validateRuntimeMaterials(textureName, entry.runtimeMaterials);
    }
    return {
      schemaVersion: TEXTURE_LIFECYCLE_SCHEMA_VERSION,
      updatedAt: typeof parsed.updatedAt === "string" ? parsed.updatedAt : null,
      textures,
    };
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return {
        schemaVersion: TEXTURE_LIFECYCLE_SCHEMA_VERSION,
        updatedAt: null,
        textures: {},
      };
    }
    throw error;
  }
}

export async function writeTextureLifecycleManifest(
  packDir: string,
  manifest: TextureLifecycleManifest,
): Promise<void> {
  const sorted: TextureLifecycleManifest = {
    schemaVersion: TEXTURE_LIFECYCLE_SCHEMA_VERSION,
    updatedAt: manifest.updatedAt,
    textures: Object.fromEntries(
      Object.entries(manifest.textures).sort(([left], [right]) => left.localeCompare(right)),
    ),
  };
  await fs.writeFile(
    path.join(path.resolve(packDir), TEXTURE_LIFECYCLE_FILENAME),
    `${JSON.stringify(sorted, null, 2)}\n`,
  );
}

export function defaultProvisionalAssetForTexture(textureName: string, texture: TextureSpec): string {
  const frozen = defaultFrozenAssetForTexture(textureName, texture);
  return frozen.replace(/^frozen(?:[/\\]|$)/, `${TEXTURE_PROVISIONAL_ASSET_RELATIVE_DIR}/`);
}

export function isLegacyDerivedTexture(texture: TextureSpec): boolean {
  return texture.catalog?.tags?.includes("far-lod-material") ?? false;
}

function validateRuntimeMaterials(textureName: string, runtimeMaterials: unknown): asserts runtimeMaterials is string[] {
  if (
    !Array.isArray(runtimeMaterials) ||
    runtimeMaterials.length === 0 ||
    runtimeMaterials.some(
      (material) =>
        typeof material !== "string" ||
        !/^[a-z0-9_.-]+:[a-z0-9_./-]+$/.test(material),
    )
  ) {
    throw new TextureLifecycleError(
      `Texture '${textureName}' needs at least one explicit canonical runtime material binding`,
    );
  }
  if (new Set(runtimeMaterials).size !== runtimeMaterials.length) {
    throw new TextureLifecycleError(`Texture '${textureName}' repeats a canonical runtime material binding`);
  }
}

function validateLifecycleImageShape(
  textureName: string,
  texture: TextureSpec,
  pack: TexturePackAsset,
  image: { width: number; height: number },
): void {
  const expectedSize = texture.size ?? pack.defaultSize;
  if (image.width !== expectedSize || image.height !== expectedSize) {
    throw new TextureLifecycleError(
      `Lifecycle asset for '${textureName}' is ${image.width}x${image.height}, expected ${expectedSize}x${expectedSize}`,
    );
  }
}

function validateLifecycleImageSourcePolicy(
  textureName: string,
  texture: TextureSpec,
  pack: TexturePackAsset,
  image: { width: number; height: number; data: Uint8Array },
): void {
  const rendered = {
    name: textureName,
    exportPath: texture.exportPath,
    source: texture.source ?? "final-color",
    width: image.width,
    height: image.height,
    data: image.data,
    preview: texture.preview,
    catalog: texture.catalog,
    authoring: [],
  } as RenderedTexture;
  if (texture.tintRole) {
    rendered.tintRole = texture.tintRole;
    const tint = pack.tints[texture.tintRole];
    if (tint) {
      rendered.tint = tint;
    }
  }
  const errors = textureSourcePolicyErrors(pack, [rendered]);
  if (errors.length > 0) {
    throw new TextureLifecycleError(errors.join("; "));
  }
}

function resolveLifecycleAssetPath(packDir: string, asset: string): string {
  if (!asset || path.isAbsolute(asset) || asset.split(/[\\/]/).includes("..")) {
    throw new TextureLifecycleError(
      `Lifecycle asset path must be relative to the pack directory, got '${asset}'`,
    );
  }
  const root = path.resolve(packDir);
  const resolved = path.resolve(root, asset);
  if (resolved !== root && !resolved.startsWith(`${root}${path.sep}`)) {
    throw new TextureLifecycleError(`Lifecycle asset '${asset}' resolves outside the pack directory`);
  }
  return resolved;
}

function bindingFromEntry(entry: TextureLifecycleManifestEntry): TextureLifecycleBinding {
  return {
    state: entry.state,
    runtimeMaterials: [...entry.runtimeMaterials],
    asset: entry.asset,
    sha256: entry.sha256,
  };
}

function metadataFromEntry(entry: TextureLifecycleManifestEntry): TextureFrozenMetadata {
  const metadata: TextureFrozenMetadata = {};
  if (entry.codename) {
    metadata.codename = entry.codename;
  }
  if (entry.candidateId) {
    metadata.candidateId = entry.candidateId;
  }
  return metadata;
}

function compactEntry<T extends Record<string, unknown>>(entry: T): T {
  return Object.fromEntries(
    Object.entries(entry).filter(([, value]) => value !== undefined),
  ) as T;
}
