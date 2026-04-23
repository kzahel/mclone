import { describe, expect, test } from "vitest";
import { ResourceLocation } from "../../../src/core/resource-location";
import { AnimationMetadataSection } from "../../../src/renderer/texture/animation-metadata-section";
import {
  createSpriteInfo,
  parseAnimationMetadataResponseText,
  parseAnimationMetadataSection,
} from "../../../src/renderer/texture/browser-native-image-loader";
import { MipmapGenerator } from "../../../src/renderer/texture/mipmap-generator";
import { NativeImage } from "../../../src/renderer/texture/native-image";
import { Stitcher } from "../../../src/renderer/texture/stitcher";
import { TextureAtlas, type TextureAtlasSource } from "../../../src/renderer/texture/texture-atlas";
import { TextureAtlasSprite, TextureAtlasSpriteInfo } from "../../../src/renderer/texture/texture-atlas-sprite";

function solidImage(width: number, height: number, color: number): NativeImage {
  const image = new NativeImage(width, height, false);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      image.setPixelRGBA(x, y, color);
    }
  }

  return image;
}

function rgba8(pixel: number): readonly [number, number, number, number] {
  return [NativeImage.getR(pixel), NativeImage.getG(pixel), NativeImage.getB(pixel), NativeImage.getA(pixel)];
}

class MemoryTextureAtlasSource implements TextureAtlasSource {
  public constructor(
    private readonly images: ReadonlyMap<string, NativeImage>,
    private readonly metadataByName: ReadonlyMap<string, AnimationMetadataSection> = new Map(),
  ) {}

  public async getBasicSpriteInfos(spriteNames: readonly ResourceLocation[]): Promise<readonly TextureAtlasSpriteInfo[]> {
    return spriteNames.map((location) => {
      const image = this.images.get(location.toString());
      if (!image) {
        throw new Error(`Missing test sprite ${location}`);
      }

      const metadata = this.metadataByName.get(location.toString()) ?? AnimationMetadataSection.EMPTY;
      return createSpriteInfo(location, image, metadata);
    });
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
    const source = this.images.get(info.name().toString());
    if (!source) {
      throw new Error(`Missing test sprite ${info.name()}`);
    }

    const image = new NativeImage(source.getWidth(), source.getHeight(), false);
    image.copyFrom(source);
    return new TextureAtlasSprite(atlas, info, mipLevel, atlasWidth, atlasHeight, x, y, image);
  }
}

describe("Texture atlas plumbing", () => {
  test("NativeImage preserves Minecraft's RGBA byte packing", () => {
    const image = new NativeImage(1, 1, false);
    image.setPixelRGBA(0, 0, NativeImage.combine(0x44, 0x33, 0x22, 0x11));
    expect(rgba8(image.getPixelRGBA(0, 0))).toEqual([0x11, 0x22, 0x33, 0x44]);
  });

  test("MipmapGenerator keeps a solid color stable across mip levels", () => {
    const color = NativeImage.combine(0xff, 0x33, 0x22, 0x11);
    const levels = MipmapGenerator.generateMipLevels(solidImage(2, 2, color), 1);
    expect(levels).toHaveLength(2);
    expect(rgba8(levels[1]!.getPixelRGBA(0, 0))).toEqual([0x10, 0x22, 0x33, 0xff]);
  });

  test("Stitcher packs two equal sprites into one 32x16 row", () => {
    const stitcher = new Stitcher(32, 32, 0);
    const left = new TextureAtlasSpriteInfo(new ResourceLocation("minecraft:block/left"), 16, 16, AnimationMetadataSection.EMPTY);
    const right = new TextureAtlasSpriteInfo(new ResourceLocation("minecraft:block/right"), 16, 16, AnimationMetadataSection.EMPTY);
    stitcher.registerSprite(left);
    stitcher.registerSprite(right);
    stitcher.stitch();

    const placements = new Map<string, readonly [number, number]>();
    stitcher.gatherSprites((info, _atlasWidth, _atlasHeight, x, y) => {
      placements.set(info.name().toString(), [x, y]);
    });

    expect(stitcher.getWidth()).toBe(32);
    expect(stitcher.getHeight()).toBe(16);
    expect(placements.get("minecraft:block/left")).toEqual([0, 0]);
    expect(placements.get("minecraft:block/right")).toEqual([16, 0]);
  });

  test("TextureAtlas.prepareToStitch produces non-trivial UVs for a stitched sprite", async () => {
    const spriteLocation = new ResourceLocation("minecraft:block/orange_wool");
    const atlas = new TextureAtlas(new ResourceLocation("minecraft:textures/atlas/blocks.png"), 64);
    const source = new MemoryTextureAtlasSource(
      new Map([[spriteLocation.toString(), solidImage(16, 16, NativeImage.combine(0xff, 0x48, 0x60, 0xe0))]]),
    );

    const preparations = await atlas.prepareToStitch(source, [spriteLocation], 0);
    const sprite = preparations.regions.find((entry) => entry.getName().equals(spriteLocation));

    expect(preparations.width).toBe(32);
    expect(preparations.height).toBe(16);
    expect(sprite).toBeDefined();
    expect(sprite?.getU0()).toBe(0);
    expect(sprite?.getU1()).toBe(0.5);
    expect(sprite?.getV0()).toBe(0);
    expect(sprite?.getV1()).toBe(1);
  });

  test("animated water-strip metadata resolves to frame-sized sprite info", () => {
    const spriteLocation = new ResourceLocation("minecraft:block/water_still");
    const image = solidImage(16, 512, NativeImage.combine(0xff, 0x80, 0x80, 0x80));
    const metadata = parseAnimationMetadataSection({
      animation: {
        frametime: 2,
      },
    });

    const info = createSpriteInfo(spriteLocation, image, metadata);

    expect(info.width()).toBe(16);
    expect(info.height()).toBe(16);
    expect(metadata.getDefaultFrameTime()).toBe(2);
  });

  test("HTML fallback animation metadata is treated as absent metadata", () => {
    const metadata = parseAnimationMetadataResponseText(
      "<!doctype html><html><body>Not JSON</body></html>",
      "text/html; charset=utf-8",
    );

    expect(metadata).toBe(AnimationMetadataSection.EMPTY);
  });

  test("TextureAtlas.prepareToStitch packs animated strips at frame size instead of full image height", async () => {
    const spriteLocation = new ResourceLocation("minecraft:block/water_still");
    const metadata = parseAnimationMetadataSection({
      animation: {
        frametime: 2,
      },
    });
    const atlas = new TextureAtlas(new ResourceLocation("minecraft:textures/atlas/blocks.png"), 64);
    const source = new MemoryTextureAtlasSource(
      new Map([[spriteLocation.toString(), solidImage(16, 512, NativeImage.combine(0xff, 0x80, 0x80, 0x80))]]),
      new Map([[spriteLocation.toString(), metadata]]),
    );

    const preparations = await atlas.prepareToStitch(source, [spriteLocation], 0);
    const sprite = preparations.regions.find((entry) => entry.getName().equals(spriteLocation));

    expect(preparations.width).toBe(32);
    expect(preparations.height).toBe(16);
    expect(sprite?.getWidth()).toBe(16);
    expect(sprite?.getHeight()).toBe(16);
    expect(sprite?.getAnimationTicker()).toBeDefined();
  });
});
