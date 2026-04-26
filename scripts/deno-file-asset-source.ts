import { ResourceLocation } from "../src/core/resource-location.ts";
import type { AssetPack } from "../src/renderer/assets/asset-pack.ts";
import { AnimationMetadataSection } from "../src/renderer/texture/animation-metadata-section.ts";
import { NativeImage } from "../src/renderer/texture/native-image.ts";
import { decodePngNativeImage } from "../src/renderer/texture/png-native-image-decoder.ts";
import type { TextureAtlasSource } from "../src/renderer/texture/texture-atlas.ts";
import { parseAnimationMetadataResponseText, createSpriteInfo } from "../src/renderer/texture/texture-atlas-source-utils.ts";
import { TextureAtlasSprite, type TextureAtlasSpriteInfo, type TextureAtlasUploadTarget } from "../src/renderer/texture/texture-atlas-sprite.ts";

export const DEFAULT_DENO_EXTRACTED_ASSETS_ROOT = new URL("../reference/minecraft-1.17.1/extracted/", import.meta.url);

export class DenoFileAssetPack implements AssetPack {
  public constructor(private readonly root: URL = DEFAULT_DENO_EXTRACTED_ASSETS_ROOT) {}

  public has(path: string): boolean {
    try {
      return Deno.statSync(this.resolve(path)).isFile;
    } catch {
      return false;
    }
  }

  public async readText(path: string): Promise<string | undefined> {
    try {
      return await Deno.readTextFile(this.resolve(path));
    } catch (error) {
      if (isNotFoundError(error)) {
        return undefined;
      }
      throw error;
    }
  }

  public async readBytes(path: string): Promise<Uint8Array | undefined> {
    try {
      return await Deno.readFile(this.resolve(path));
    } catch (error) {
      if (isNotFoundError(error)) {
        return undefined;
      }
      throw error;
    }
  }

  public async readBlob(path: string, contentType?: string): Promise<Blob | undefined> {
    const bytes = await this.readBytes(path);
    if (bytes === undefined) {
      return undefined;
    }

    const copy = new Uint8Array(bytes.byteLength);
    copy.set(bytes);
    return new Blob([copy.buffer], { type: contentType });
  }

  private resolve(path: string): URL {
    const normalized = normalizeAssetPath(path);
    return new URL(normalized, this.root);
  }
}

export class DenoFileTextureAtlasSource implements TextureAtlasSource {
  private readonly imageCache = new Map<string, Promise<NativeImage>>();
  private readonly metadataCache = new Map<string, Promise<AnimationMetadataSection>>();

  public constructor(private readonly assetPack: AssetPack) {}

  public resolveTexturePath(location: ResourceLocation): string {
    return `assets/${location.getNamespace()}/textures/${location.getPath()}.png`;
  }

  public resolveAnimationMetadataPath(location: ResourceLocation): string {
    return `${this.resolveTexturePath(location)}.mcmeta`;
  }

  public async loadColorMap(location: ResourceLocation): Promise<readonly number[]> {
    const image = await this.loadImageFromPack(location);
    try {
      return image.makePixelArray();
    } finally {
      image.close();
    }
  }

  public async getBasicSpriteInfos(spriteNames: readonly ResourceLocation[]): Promise<readonly TextureAtlasSpriteInfo[]> {
    return Promise.all(
      spriteNames.map(async (location) => {
        const [image, metadata] = await Promise.all([this.loadImage(location), this.loadAnimationMetadata(location)]);
        return createSpriteInfo(location, image, metadata);
      }),
    );
  }

  public async loadSprite(
    atlas: TextureAtlasUploadTarget,
    info: TextureAtlasSpriteInfo,
    atlasWidth: number,
    atlasHeight: number,
    mipLevel: number,
    x: number,
    y: number,
  ): Promise<TextureAtlasSprite | undefined> {
    const image = await this.loadImage(info.name());
    return new TextureAtlasSprite(atlas, info, mipLevel, atlasWidth, atlasHeight, x, y, copyImage(image));
  }

  private loadImage(location: ResourceLocation): Promise<NativeImage> {
    const key = location.toString();
    let image = this.imageCache.get(key);
    if (image === undefined) {
      image = this.loadImageFromPack(location);
      this.imageCache.set(key, image);
    }

    return image;
  }

  private async loadImageFromPack(location: ResourceLocation): Promise<NativeImage> {
    const path = this.resolveTexturePath(location);
    const bytes = await this.assetPack.readBytes(path);
    if (bytes === undefined) {
      throw new Error(`Unable to load image asset ${path}`);
    }

    return decodePngNativeImage(bytes);
  }

  private loadAnimationMetadata(location: ResourceLocation): Promise<AnimationMetadataSection> {
    const key = location.toString();
    let metadata = this.metadataCache.get(key);
    if (metadata === undefined) {
      metadata = (async () => {
        const text = await this.assetPack.readText(this.resolveAnimationMetadataPath(location));
        if (text === undefined) {
          return AnimationMetadataSection.EMPTY;
        }

        return parseAnimationMetadataResponseText(text, "application/json");
      })();
      this.metadataCache.set(key, metadata);
    }

    return metadata;
  }

  public close(): void {
    for (const image of this.imageCache.values()) {
      image.then((loaded) => loaded.close()).catch(() => {});
    }
    this.imageCache.clear();
    this.metadataCache.clear();
  }
}

function normalizeAssetPath(path: string): string {
  const normalized = path.replace(/^\/+/, "");
  if (normalized.length === 0 || normalized.split("/").includes("..")) {
    throw new Error(`Invalid asset path ${path}`);
  }

  return normalized;
}

function isNotFoundError(error: unknown): boolean {
  return error instanceof Deno.errors.NotFound;
}

function copyImage(image: NativeImage): NativeImage {
  const copy = new NativeImage(image.getWidth(), image.getHeight(), false);
  copy.copyFrom(image);
  return copy;
}
