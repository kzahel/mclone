import { ResourceLocation } from "../../core/resource-location";
import type { AssetPack } from "../assets/asset-pack";
import { AnimationMetadataSection } from "./animation-metadata-section";
import { NativeImage } from "./native-image";
import { decodePngNativeImage } from "./png-native-image-decoder";
import type { TextureAtlasSource } from "./texture-atlas";
import { createSpriteInfo, parseAnimationMetadataResponseText } from "./texture-atlas-source-utils";
import { TextureAtlasSprite, type TextureAtlasSpriteInfo, type TextureAtlasUploadTarget } from "./texture-atlas-sprite";

export type NativeImageBytesDecoder = (bytes: Uint8Array) => Promise<NativeImage>;

function copyImage(image: NativeImage): NativeImage {
  const copy = new NativeImage(image.getWidth(), image.getHeight(), false);
  copy.copyFrom(image);
  return copy;
}

export class AssetPackTextureAtlasSource implements TextureAtlasSource {
  private readonly imageCache = new Map<string, Promise<NativeImage>>();
  private readonly metadataCache = new Map<string, Promise<AnimationMetadataSection>>();

  public constructor(
    private readonly assetPack: AssetPack,
    private readonly decodeImageBytes: NativeImageBytesDecoder = decodePngNativeImage,
  ) {}

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

  public close(): void {
    for (const image of this.imageCache.values()) {
      image.then((loaded) => loaded.close()).catch(() => {});
    }
    this.imageCache.clear();
    this.metadataCache.clear();
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

    return this.decodeImageBytes(bytes);
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
}
