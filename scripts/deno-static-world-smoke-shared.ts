import { Direction } from "../src/core/direction.ts";
import { ResourceLocation } from "../src/core/resource-location.ts";
import { BlockColors } from "../src/renderer/block/block-colors.ts";
import { BlockRenderDispatcher } from "../src/renderer/block/block-render-dispatcher.ts";
import { BlockElementFace } from "../src/renderer/model/block-element-face.ts";
import { BlockFaceUV } from "../src/renderer/model/block-face-uv.ts";
import { BlockModelShaper } from "../src/renderer/model/block-model-shaper.ts";
import { FaceBakery } from "../src/renderer/model/face-bakery.ts";
import { ItemOverrides } from "../src/renderer/model/item-overrides.ts";
import { ItemTransforms } from "../src/renderer/model/item-transforms.ts";
import { type BakedModel } from "../src/renderer/model/baked-model.ts";
import { ModelManager } from "../src/renderer/model/model-manager.ts";
import { IDENTITY_MODEL_STATE } from "../src/renderer/model/model-state.ts";
import { SimpleBakedModel } from "../src/renderer/model/simple-baked-model.ts";
import { Vector3f } from "../src/renderer/math/vector3f.ts";
import { AnimationMetadataSection } from "../src/renderer/texture/animation-metadata-section.ts";
import { NativeImage } from "../src/renderer/texture/native-image.ts";
import { type TextureAtlasSource } from "../src/renderer/texture/texture-atlas.ts";
import { TextureAtlasSprite, TextureAtlasSpriteInfo, type TextureAtlasUploadTarget } from "../src/renderer/texture/texture-atlas-sprite.ts";
import type { BlockState } from "../src/world/level/block/state/block-state.ts";

export const DENO_STATIC_WORLD_SEED = 12345n;
export const DENO_STATIC_WORLD_MIN_BUILD_HEIGHT = 0;
export const DENO_STATIC_WORLD_HEIGHT = 16;
export const DENO_STATIC_WORLD_VIEW_DISTANCE = 2;
export const DENO_STATIC_WORLD_TEXTURE_MIP_LEVEL = 0;
export const DENO_STATIC_WORLD_STONE_TEXTURE = new ResourceLocation("minecraft:block/deno_static_stone");
export const DENO_STATIC_WORLD_CUBE_MODEL = new ResourceLocation("minecraft:block/deno_static_cube");
export const DENO_STATIC_WORLD_STONE_PIXEL = new Uint8Array([176, 176, 176, 255]);

const DUMMY_ATLAS: TextureAtlasUploadTarget = {
  upload(): void {},
};

export class DenoStaticWorldTextureAtlasSource implements TextureAtlasSource {
  private readonly image = createSolidNativeImage(16, 16, nativeColor(DENO_STATIC_WORLD_STONE_PIXEL));

  public async getBasicSpriteInfos(spriteNames: readonly ResourceLocation[]): Promise<readonly TextureAtlasSpriteInfo[]> {
    return spriteNames.map((location) => {
      if (!location.equals(DENO_STATIC_WORLD_STONE_TEXTURE)) {
        throw new Error(`Missing Deno static-world sprite ${location.toString()}`);
      }

      return new TextureAtlasSpriteInfo(location, this.image.getWidth(), this.image.getHeight(), AnimationMetadataSection.EMPTY);
    });
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
    if (!info.name().equals(DENO_STATIC_WORLD_STONE_TEXTURE)) {
      throw new Error(`Missing Deno static-world sprite ${info.name().toString()}`);
    }

    const image = new NativeImage(this.image.getWidth(), this.image.getHeight(), false);
    image.copyFrom(this.image);
    return new TextureAtlasSprite(atlas, info, mipLevel, atlasWidth, atlasHeight, x, y, image);
  }

  public close(): void {
    this.image.close();
  }
}

class SingleModelBlockModelShaper extends BlockModelShaper {
  public constructor(private readonly model: BakedModel) {
    super(new ModelManager(model));
  }

  public override getParticleIcon(_state: BlockState): TextureAtlasSprite {
    return this.model.getParticleIcon();
  }

  public override getBlockModel(_state: BlockState): BakedModel {
    return this.model;
  }

  public override rebuildCache(): void {}
}

export function createDenoStaticWorldBlockRenderer(
  sprite: TextureAtlasSprite,
  waterState: BlockState,
  lavaState: BlockState,
): BlockRenderDispatcher {
  const model = createStoneCubeModel(sprite);
  const shaper = new SingleModelBlockModelShaper(model);
  return new BlockRenderDispatcher(shaper, BlockColors.createDefault(), () => sprite, waterState, lavaState);
}

export function createDenoStaticWorldWorkerStoneSprite(): TextureAtlasSprite {
  const info = new TextureAtlasSpriteInfo(DENO_STATIC_WORLD_STONE_TEXTURE, 16, 16, AnimationMetadataSection.EMPTY);
  const image = createSolidNativeImage(16, 16, nativeColor(DENO_STATIC_WORLD_STONE_PIXEL));
  return new TextureAtlasSprite(DUMMY_ATLAS, info, 0, 32, 16, 0, 0, image);
}

function createStoneCubeModel(sprite: TextureAtlasSprite): BakedModel {
  const bakery = new FaceBakery();
  const culledFaces = new Map<Direction, ReturnType<FaceBakery["bakeQuad"]>[]>();
  for (const direction of Direction.values()) {
    const face = new BlockElementFace(direction, BlockElementFace.NO_TINT, "#all", new BlockFaceUV([0, 0, 16, 16], 0));
    const quad = bakery.bakeQuad(
      new Vector3f(0, 0, 0),
      new Vector3f(16, 16, 16),
      face,
      sprite,
      direction,
      IDENTITY_MODEL_STATE,
      undefined,
      true,
      DENO_STATIC_WORLD_CUBE_MODEL,
    );
    culledFaces.set(direction, [quad]);
  }

  return new SimpleBakedModel(
    [],
    culledFaces,
    false,
    true,
    true,
    sprite,
    ItemTransforms.NO_TRANSFORMS,
    ItemOverrides.EMPTY,
  );
}

function createSolidNativeImage(width: number, height: number, color: number): NativeImage {
  const image = new NativeImage(width, height, false);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      image.setPixelRGBA(x, y, color);
    }
  }

  return image;
}

function nativeColor(pixel: Uint8Array): number {
  return NativeImage.combine(pixel[3]!, pixel[2]!, pixel[1]!, pixel[0]!);
}
