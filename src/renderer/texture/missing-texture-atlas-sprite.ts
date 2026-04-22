import { ResourceLocation } from "../../core/resource-location";
import { AnimationFrame } from "./animation-frame";
import { AnimationMetadataSection } from "./animation-metadata-section";
import { NativeImage } from "./native-image";
import { TextureAtlasSprite, TextureAtlasSpriteInfo, type TextureAtlasUploadTarget } from "./texture-atlas-sprite";

const MISSING_TEXTURE_LOCATION = new ResourceLocation("missingno");
const MISSING_IMAGE_WIDTH = 16;
const MISSING_IMAGE_HEIGHT = 16;
let missingImageData: NativeImage | undefined;

function getMissingImageData(): NativeImage {
  if (missingImageData) {
    return missingImageData;
  }

  const image = new NativeImage(MISSING_IMAGE_WIDTH, MISSING_IMAGE_HEIGHT, false);
  const black = 0xff000000;
  const magenta = 0xfff800f8;
  for (let y = 0; y < MISSING_IMAGE_HEIGHT; y++) {
    for (let x = 0; x < MISSING_IMAGE_WIDTH; x++) {
      image.setPixelRGBA(x, y, (x < 8) !== (y < 8) ? magenta : black);
    }
  }

  missingImageData = image;
  return image;
}

const INFO = new TextureAtlasSpriteInfo(
  MISSING_TEXTURE_LOCATION,
  MISSING_IMAGE_WIDTH,
  MISSING_IMAGE_HEIGHT,
  new AnimationMetadataSection([new AnimationFrame(0, -1)], 16, 16, 1, false),
);

export class MissingTextureAtlasSprite extends TextureAtlasSprite {
  private constructor(atlas: TextureAtlasUploadTarget, mipLevel: number, atlasWidth: number, atlasHeight: number, x: number, y: number) {
    super(atlas, INFO, mipLevel, atlasWidth, atlasHeight, x, y, getMissingImageData());
  }

  public static newInstance(
    atlas: TextureAtlasUploadTarget,
    mipLevel: number,
    atlasWidth: number,
    atlasHeight: number,
    x: number,
    y: number,
  ): MissingTextureAtlasSprite {
    return new MissingTextureAtlasSprite(atlas, mipLevel, atlasWidth, atlasHeight, x, y);
  }

  public static getLocation(): ResourceLocation {
    return MISSING_TEXTURE_LOCATION;
  }

  public static info(): TextureAtlasSpriteInfo {
    return INFO;
  }

  public override close(): void {
    for (let index = 1; index < this.mainImage.length; index++) {
      this.mainImage[index]?.close();
    }
  }
}
