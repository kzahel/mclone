import { ResourceLocation } from "../../core/resource-location";
import type { AssetPack } from "../assets/asset-pack";
import { AnimationFrame } from "./animation-frame";
import { AnimationMetadataSection } from "./animation-metadata-section";
import { NativeImage } from "./native-image";
import { loadNativeImageFromBlobWithDecoder, loadNativeImageFromUrlWithDecoder, type NativeImageDecoder } from "./native-image-decoder";
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

export class BrowserNativeImageDecoder implements NativeImageDecoder {
  public async decode(blob: Blob): Promise<NativeImage> {
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
}

const BROWSER_NATIVE_IMAGE_DECODER = new BrowserNativeImageDecoder();

export async function loadNativeImageFromUrl(
  url: string,
  decoder: NativeImageDecoder = BROWSER_NATIVE_IMAGE_DECODER,
): Promise<NativeImage> {
  return loadNativeImageFromUrlWithDecoder(url, decoder);
}

export async function loadNativeImageFromBlob(
  blob: Blob,
  decoder: NativeImageDecoder = BROWSER_NATIVE_IMAGE_DECODER,
): Promise<NativeImage> {
  return loadNativeImageFromBlobWithDecoder(blob, decoder);
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

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function looksLikeHtmlDocument(value: string): boolean {
  const trimmed = value.trimStart().toLowerCase();
  return trimmed.startsWith("<!doctype html") || trimmed.startsWith("<html");
}

function getInteger(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? Math.trunc(value) : undefined;
}

function parseAnimationFrame(value: unknown): AnimationFrame | undefined {
  if (typeof value === "number" && Number.isFinite(value)) {
    return new AnimationFrame(Math.trunc(value));
  }

  if (!isObject(value)) {
    return undefined;
  }

  const index = getInteger(value.index);
  if (index === undefined) {
    return undefined;
  }

  const time = getInteger(value.time);
  return time === undefined ? new AnimationFrame(index) : new AnimationFrame(index, time);
}

export function parseAnimationMetadataSection(json: unknown): AnimationMetadataSection {
  if (!isObject(json) || !isObject(json.animation)) {
    return AnimationMetadataSection.EMPTY;
  }

  const animation = json.animation;
  const frames: AnimationFrame[] = [];
  if (Array.isArray(animation.frames)) {
    for (const frame of animation.frames) {
      const parsed = parseAnimationFrame(frame);
      if (parsed !== undefined) {
        frames.push(parsed);
      }
    }
  }

  return new AnimationMetadataSection(
    frames,
    getInteger(animation.width) ?? AnimationMetadataSection.UNKNOWN_SIZE,
    getInteger(animation.height) ?? AnimationMetadataSection.UNKNOWN_SIZE,
    getInteger(animation.frametime) ?? AnimationMetadataSection.DEFAULT_FRAME_TIME,
    animation.interpolate === true,
  );
}

export function parseAnimationMetadataResponseText(
  value: string,
  contentType: string | null,
): AnimationMetadataSection {
  if (looksLikeHtmlDocument(value) || contentType?.toLowerCase().includes("text/html") === true) {
    return AnimationMetadataSection.EMPTY;
  }

  return parseAnimationMetadataSection(JSON.parse(value) as unknown);
}

export function createSpriteInfo(
  location: ResourceLocation,
  image: NativeImage,
  metadata: AnimationMetadataSection,
): TextureAtlasSpriteInfo {
  const [frameWidth, frameHeight] = metadata.getFrameSize(image.getWidth(), image.getHeight());
  return new TextureAtlasSpriteInfo(location, frameWidth, frameHeight, metadata);
}

export class BrowserTextureAtlasSource {
  private readonly cache = new Map<string, Promise<NativeImage>>();
  private readonly metadataCache = new Map<string, Promise<AnimationMetadataSection>>();

  public constructor(
    private readonly assetPack: AssetPack,
    private readonly imageDecoder: NativeImageDecoder = BROWSER_NATIVE_IMAGE_DECODER,
  ) {}

  public resolveTexturePath(location: ResourceLocation): string {
    return `assets/${location.getNamespace()}/textures/${location.getPath()}.png`;
  }

  public resolveAnimationMetadataPath(location: ResourceLocation): string {
    return `${this.resolveTexturePath(location)}.mcmeta`;
  }

  private async loadImageFromPack(location: ResourceLocation): Promise<NativeImage> {
    const path = this.resolveTexturePath(location);
    const blob = await this.assetPack.readBlob(path, "image/png");
    if (blob === undefined) {
      throw new Error(`Unable to load image asset ${path}`);
    }

    return loadNativeImageFromBlob(blob, this.imageDecoder);
  }

  public async loadColorMap(location: ResourceLocation): Promise<readonly number[]> {
    const image = await this.loadImageFromPack(location);
    try {
      return image.makePixelArray();
    } finally {
      image.close();
    }
  }

  private loadImage(location: ResourceLocation): Promise<NativeImage> {
    const key = location.toString();
    let image = this.cache.get(key);
    if (!image) {
      image = this.loadImageFromPack(location);
      this.cache.set(key, image);
    }

    return image;
  }

  private loadAnimationMetadata(location: ResourceLocation): Promise<AnimationMetadataSection> {
    const key = location.toString();
    let metadata = this.metadataCache.get(key);
    if (!metadata) {
      metadata = (async () => {
        const text = await this.assetPack.readText(this.resolveAnimationMetadataPath(location));
        if (text === undefined) {
          return AnimationMetadataSection.EMPTY;
        }

        return parseAnimationMetadataResponseText(
          text,
          "application/json",
        );
      })();
      this.metadataCache.set(key, metadata);
    }

    return metadata;
  }

  public async getBasicSpriteInfos(spriteNames: readonly ResourceLocation[]): Promise<readonly TextureAtlasSpriteInfo[]> {
    const infos = await Promise.all(
      spriteNames.map(async (location) => {
        const [image, metadata] = await Promise.all([this.loadImage(location), this.loadAnimationMetadata(location)]);
        return createSpriteInfo(location, image, metadata);
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
