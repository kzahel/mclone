import { ResourceLocation } from "../../core/resource-location";
import { AnimationMetadataSection } from "./animation-metadata-section";
import { NativeImage } from "./native-image";
import { TextureAtlas } from "./texture-atlas";
import { TextureAtlasSprite, TextureAtlasSpriteInfo } from "./texture-atlas-sprite";

function createScratchCanvas(width: number, height: number): OffscreenCanvas | HTMLCanvasElement {
  if (typeof OffscreenCanvas !== "undefined") {
    return new OffscreenCanvas(width, height);
  }

  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  return canvas;
}

export async function loadNativeImageFromUrl(url: string): Promise<NativeImage> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Unable to load image ${url}: ${response.status} ${response.statusText}`);
  }

  const blob = await response.blob();
  const bitmap = await createImageBitmap(blob);
  try {
    const canvas = createScratchCanvas(bitmap.width, bitmap.height);
    const context = canvas.getContext("2d") as OffscreenCanvasRenderingContext2D | CanvasRenderingContext2D | null;
    if (!context) {
      throw new Error("Unable to acquire a 2d canvas context for image decoding");
    }

    context.drawImage(bitmap, 0, 0);
    return NativeImage.fromImageData(context.getImageData(0, 0, bitmap.width, bitmap.height));
  } finally {
    bitmap.close();
  }
}

export async function loadColorMapPixels(url: string): Promise<readonly number[]> {
  const image = await loadNativeImageFromUrl(url);
  try {
    return image.makePixelArray();
  } finally {
    image.close();
  }
}

function copyImage(image: NativeImage): NativeImage {
  const copy = new NativeImage(image.getWidth(), image.getHeight(), false);
  copy.copyFrom(image);
  return copy;
}

export class BrowserTextureAtlasSource {
  private readonly cache = new Map<string, Promise<NativeImage>>();

  public constructor(private readonly assetRoot: string = "/reference/minecraft-1.17.1/extracted/assets") {}

  public resolveTextureUrl(location: ResourceLocation): string {
    return `${this.assetRoot}/${location.getNamespace()}/textures/${location.getPath()}.png`;
  }

  public loadColorMap(location: ResourceLocation): Promise<readonly number[]> {
    return loadColorMapPixels(this.resolveTextureUrl(location));
  }

  private loadImage(location: ResourceLocation): Promise<NativeImage> {
    const key = location.toString();
    let image = this.cache.get(key);
    if (!image) {
      image = loadNativeImageFromUrl(this.resolveTextureUrl(location));
      this.cache.set(key, image);
    }

    return image;
  }

  public async getBasicSpriteInfos(spriteNames: readonly ResourceLocation[]): Promise<readonly TextureAtlasSpriteInfo[]> {
    const infos = await Promise.all(
      spriteNames.map(async (location) => {
        const image = await this.loadImage(location);
        return new TextureAtlasSpriteInfo(location, image.getWidth(), image.getHeight(), AnimationMetadataSection.EMPTY);
      }),
    );
    return infos;
  }

  public async loadSprite(
    atlas: TextureAtlas,
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
}
