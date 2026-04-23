import { ResourceLocation } from "../../core/resource-location";
import { log2 } from "../../util/mth";
import { MissingTextureAtlasSprite } from "./missing-texture-atlas-sprite";
import { NativeImage } from "./native-image";
import { Stitcher } from "./stitcher";
import { TextureAtlasSprite, TextureAtlasSpriteInfo, type Tickable, type TextureAtlasUploadTarget } from "./texture-atlas-sprite";

function lowestOneBit(value: number): number {
  return value & -value;
}

export const DEFAULT_BLOCK_ATLAS_MIP_LEVEL = 4;

export interface TextureAtlasSource {
  getBasicSpriteInfos(spriteNames: readonly ResourceLocation[]): Promise<readonly TextureAtlasSpriteInfo[]>;
  loadSprite(
    atlas: TextureAtlasUploadTarget,
    info: TextureAtlasSpriteInfo,
    atlasWidth: number,
    atlasHeight: number,
    mipLevel: number,
    x: number,
    y: number,
  ): Promise<TextureAtlasSprite | undefined>;
}

export class TextureAtlasPreparations {
  public constructor(
    public readonly sprites: readonly ResourceLocation[],
    public readonly width: number,
    public readonly height: number,
    public readonly mipLevel: number,
    public readonly regions: readonly TextureAtlasSprite[],
  ) {}
}

export class TextureAtlas implements TextureAtlasUploadTarget {
  public static readonly LOCATION_BLOCKS = new ResourceLocation("textures/atlas/blocks.png");

  private readonly animatedTextures: Tickable[] = [];
  private readonly sprites = new Set<string>();
  private readonly texturesByName = new Map<string, TextureAtlasSprite>();
  private texture: GPUTexture | undefined;
  private textureView: GPUTextureView | undefined;
  private device: GPUDevice | undefined;

  public constructor(
    private readonly locationValue: ResourceLocation,
    private readonly maxSupportedTextureSize: number = 16_384,
  ) {}

  public async prepareToStitch(source: TextureAtlasSource, spriteNames: Iterable<ResourceLocation>, mipLevel: number): Promise<TextureAtlasPreparations> {
    const spriteList = [...spriteNames];
    const stitcher = new Stitcher(this.maxSupportedTextureSize, this.maxSupportedTextureSize, mipLevel);
    let minimumSpriteSize = Number.MAX_SAFE_INTEGER;
    let minimumMipSize = 1 << mipLevel;

    for (const info of await source.getBasicSpriteInfos(spriteList)) {
      minimumSpriteSize = Math.min(minimumSpriteSize, Math.min(info.width(), info.height()));
      const lowestOneBitSize = Math.min(lowestOneBit(info.width()), lowestOneBit(info.height()));
      if (lowestOneBitSize < minimumMipSize) {
        minimumMipSize = lowestOneBitSize;
      }

      stitcher.registerSprite(info);
    }

    const minimumMipmapDimension = Math.min(minimumSpriteSize, minimumMipSize);
    const allowedMipLevel = log2(minimumMipmapDimension);
    const resolvedMipLevel = allowedMipLevel < mipLevel ? allowedMipLevel : mipLevel;

    stitcher.registerSprite(MissingTextureAtlasSprite.info());
    stitcher.stitch();
    const sprites = await this.getLoadedSprites(source, stitcher, resolvedMipLevel);
    return new TextureAtlasPreparations(spriteList, stitcher.getWidth(), stitcher.getHeight(), resolvedMipLevel, sprites);
  }

  private async getLoadedSprites(source: TextureAtlasSource, stitcher: Stitcher, mipLevel: number): Promise<TextureAtlasSprite[]> {
    const sprites: TextureAtlasSprite[] = [];
    const loads: Promise<void>[] = [];
    stitcher.gatherSprites((info, atlasWidth, atlasHeight, x, y) => {
      if (info.name().equals(MissingTextureAtlasSprite.info().name())) {
        sprites.push(MissingTextureAtlasSprite.newInstance(this, mipLevel, atlasWidth, atlasHeight, x, y));
        return;
      }

      loads.push(
        source.loadSprite(this, info, atlasWidth, atlasHeight, mipLevel, x, y).then((sprite) => {
          if (sprite) {
            sprites.push(sprite);
          }
        }),
      );
    });
    await Promise.all(loads);
    return sprites;
  }

  // WebGPU: atlas upload targets an explicit GPUTexture instead of a GL texture bound on the render thread.
  public reload(device: GPUDevice, preparations: TextureAtlasPreparations): void {
    this.sprites.clear();
    for (const sprite of preparations.sprites) {
      this.sprites.add(sprite.toString());
    }

    this.clearTextureData();
    this.device = device;
    this.texture = device.createTexture({
      size: { width: preparations.width, height: preparations.height },
      format: "rgba8unorm",
      mipLevelCount: preparations.mipLevel + 1,
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    this.textureView = this.texture.createView();

    for (const sprite of preparations.regions) {
      this.texturesByName.set(sprite.getName().toString(), sprite);
      sprite.uploadFirstFrame();
      const ticker = sprite.getAnimationTicker();
      if (ticker) {
        this.animatedTextures.push(ticker);
      }
    }
  }

  public upload(image: NativeImage, mipLevel: number, xOffset: number, yOffset: number, x: number, y: number, width: number, height: number): void {
    if (!this.device || !this.texture) {
      throw new Error("TextureAtlas.reload() must be called before uploading sprite data");
    }

    image.upload(this.device.queue, this.texture, mipLevel, xOffset, yOffset, x, y, width, height);
  }

  public cycleAnimationFrames(): void {
    for (const texture of this.animatedTextures) {
      texture.tick();
    }
  }

  public getSprite(location: ResourceLocation): TextureAtlasSprite {
    return this.texturesByName.get(location.toString()) ?? this.texturesByName.get(MissingTextureAtlasSprite.getLocation().toString())!;
  }

  public getTextureView(): GPUTextureView {
    if (!this.textureView) {
      throw new Error("TextureAtlas.reload() must be called before reading the atlas texture view");
    }

    return this.textureView;
  }

  public clearTextureData(): void {
    for (const sprite of this.texturesByName.values()) {
      sprite.close();
    }

    this.texturesByName.clear();
    this.animatedTextures.length = 0;
    this.texture?.destroy();
    this.texture = undefined;
    this.textureView = undefined;
  }

  public getResourceLocation(location: ResourceLocation): ResourceLocation {
    return new ResourceLocation(location.getNamespace(), `textures/${location.getPath()}.png`);
  }

  public location(): ResourceLocation {
    return this.locationValue;
  }
}
