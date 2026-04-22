import { ResourceLocation } from "../../core/resource-location";
import { AnimationMetadataSection } from "./animation-metadata-section";
import { MipmapGenerator } from "./mipmap-generator";
import { NativeImage } from "./native-image";

export interface Tickable {
  tick(): void;
}

export interface TextureAtlasUploadTarget {
  upload(image: NativeImage, mipLevel: number, xOffset: number, yOffset: number, x: number, y: number, width: number, height: number): void;
}

class TextureAtlasSpriteFrameInfo {
  public constructor(
    public readonly index: number,
    public readonly time: number,
  ) {}
}

class TextureAtlasSpriteInterpolationData {
  private readonly activeFrame: NativeImage[];

  public constructor(
    private readonly sprite: TextureAtlasSprite,
    info: TextureAtlasSpriteInfo,
    mipLevel: number,
  ) {
    this.activeFrame = new Array<NativeImage>(mipLevel + 1);
    for (let level = 0; level < this.activeFrame.length; level++) {
      const width = info.width() >> level;
      const height = info.height() >> level;
      this.activeFrame[level] = new NativeImage(width, height, false);
    }
  }

  private getPixel(animation: TextureAtlasSpriteAnimatedTexture, frame: number, mipLevel: number, x: number, y: number): number {
    return this.sprite.mainImage[mipLevel]!.getPixelRGBA(
      x + ((animation.getFrameX(frame) * this.sprite.getWidth()) >> mipLevel),
      y + ((animation.getFrameY(frame) * this.sprite.getHeight()) >> mipLevel),
    );
  }

  private mix(weight: number, from: number, to: number): number {
    return Math.trunc((weight * from) + ((1.0 - weight) * to));
  }

  public uploadInterpolatedFrame(animation: TextureAtlasSpriteAnimatedTexture): void {
    const currentFrame = animation.frames[animation.frame]!;
    const interpolation = 1.0 - (animation.subFrame / currentFrame.time);
    const currentIndex = currentFrame.index;
    const nextIndex = animation.frames[(animation.frame + 1) % animation.frames.length]!.index;
    if (currentIndex === nextIndex) {
      return;
    }

    for (let level = 0; level < this.activeFrame.length; level++) {
      const width = this.sprite.getWidth() >> level;
      const height = this.sprite.getHeight() >> level;
      for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
          const from = this.getPixel(animation, currentIndex, level, x, y);
          const to = this.getPixel(animation, nextIndex, level, x, y);
          const blue = this.mix(interpolation, (from >>> 16) & 0xff, (to >>> 16) & 0xff);
          const green = this.mix(interpolation, (from >>> 8) & 0xff, (to >>> 8) & 0xff);
          const red = this.mix(interpolation, from & 0xff, to & 0xff);
          this.activeFrame[level]!.setPixelRGBA(x, y, ((from & 0xff000000) | (blue << 16) | (green << 8) | red) >>> 0);
        }
      }
    }

    this.sprite.upload(0, 0, this.activeFrame);
  }

  public close(): void {
    for (const frame of this.activeFrame) {
      frame?.close();
    }
  }
}

class TextureAtlasSpriteAnimatedTexture implements Tickable {
  public frame = 0;
  public subFrame = 0;

  public constructor(
    private readonly sprite: TextureAtlasSprite,
    public readonly frames: readonly TextureAtlasSpriteFrameInfo[],
    private readonly frameRowSize: number,
    private readonly interpolationData: TextureAtlasSpriteInterpolationData | undefined,
  ) {}

  public getFrameX(frame: number): number {
    return frame % this.frameRowSize;
  }

  public getFrameY(frame: number): number {
    return Math.trunc(frame / this.frameRowSize);
  }

  private uploadFrame(frame: number): void {
    const x = this.getFrameX(frame) * this.sprite.getWidth();
    const y = this.getFrameY(frame) * this.sprite.getHeight();
    this.sprite.upload(x, y, this.sprite.mainImage);
  }

  public tick(): void {
    this.subFrame++;
    const currentFrame = this.frames[this.frame]!;
    if (this.subFrame >= currentFrame.time) {
      const previousIndex = currentFrame.index;
      this.frame = (this.frame + 1) % this.frames.length;
      this.subFrame = 0;
      const nextIndex = this.frames[this.frame]!.index;
      if (previousIndex !== nextIndex) {
        this.uploadFrame(nextIndex);
      }
    } else {
      this.interpolationData?.uploadInterpolatedFrame(this);
    }
  }

  public uploadFirstFrame(): void {
    this.uploadFrame(this.frames[0]!.index);
  }

  public getUniqueFrames(): number[] {
    return [...new Set(this.frames.map((frame) => frame.index))];
  }

  public close(): void {
    this.interpolationData?.close();
  }
}

export class TextureAtlasSpriteInfo {
  public constructor(
    private readonly nameValue: ResourceLocation,
    private readonly widthValue: number,
    private readonly heightValue: number,
    public readonly metadata: AnimationMetadataSection,
  ) {}

  public name(): ResourceLocation {
    return this.nameValue;
  }

  public width(): number {
    return this.widthValue;
  }

  public height(): number {
    return this.heightValue;
  }
}

export class TextureAtlasSprite {
  public readonly width: number;
  public readonly height: number;
  public readonly mainImage: NativeImage[];
  private readonly animatedTexture: TextureAtlasSpriteAnimatedTexture | undefined;
  private readonly name: ResourceLocation;
  private readonly x: number;
  private readonly y: number;
  private readonly u0: number;
  private readonly u1: number;
  private readonly v0: number;
  private readonly v1: number;

  public constructor(
    private readonly atlasValue: TextureAtlasUploadTarget,
    info: TextureAtlasSpriteInfo,
    mipLevel: number,
    atlasWidth: number,
    atlasHeight: number,
    x: number,
    y: number,
    image: NativeImage,
  ) {
    this.width = info.width();
    this.height = info.height();
    this.name = info.name();
    this.x = x;
    this.y = y;
    this.u0 = x / atlasWidth;
    this.u1 = (x + this.width) / atlasWidth;
    this.v0 = y / atlasHeight;
    this.v1 = (y + this.height) / atlasHeight;
    this.animatedTexture = this.createTicker(info, image.getWidth(), image.getHeight(), mipLevel);
    this.mainImage = MipmapGenerator.generateMipLevels(image, mipLevel);
  }

  private getFrameCount(): number {
    return this.animatedTexture !== undefined ? this.animatedTexture.frames.length : 1;
  }

  private createTicker(info: TextureAtlasSpriteInfo, width: number, height: number, mipLevel: number): TextureAtlasSpriteAnimatedTexture | undefined {
    const metadata = info.metadata;
    const frameRowSize = Math.trunc(width / metadata.getFrameWidth(info.width()));
    const frameColumnSize = Math.trunc(height / metadata.getFrameHeight(info.height()));
    const frameCount = frameRowSize * frameColumnSize;
    const frames: TextureAtlasSpriteFrameInfo[] = [];
    metadata.forEachFrame((index, time) => frames.push(new TextureAtlasSpriteFrameInfo(index, time)));
    if (frames.length === 0) {
      for (let index = 0; index < frameCount; index++) {
        frames.push(new TextureAtlasSpriteFrameInfo(index, metadata.getDefaultFrameTime()));
      }
    } else {
      const usedFrames = new Set<number>();
      for (let index = frames.length - 1; index >= 0; index--) {
        const frame = frames[index]!;
        let valid = true;
        if (frame.time <= 0) {
          valid = false;
        }

        if (frame.index < 0 || frame.index >= frameCount) {
          valid = false;
        }

        if (valid) {
          usedFrames.add(frame.index);
        } else {
          frames.splice(index, 1);
        }
      }
    }

    if (frames.length <= 1) {
      return undefined;
    }

    const interpolationData = metadata.isInterpolatedFrames() ? new TextureAtlasSpriteInterpolationData(this, info, mipLevel) : undefined;
    return new TextureAtlasSpriteAnimatedTexture(this, frames, frameRowSize, interpolationData);
  }

  public upload(x: number, y: number, images: readonly NativeImage[]): void {
    for (let level = 0; level < this.mainImage.length; level++) {
      this.atlasValue.upload(
        images[level]!,
        level,
        this.x >> level,
        this.y >> level,
        x >> level,
        y >> level,
        this.width >> level,
        this.height >> level,
      );
    }
  }

  private atlasSize(): number {
    const width = this.width / (this.u1 - this.u0);
    const height = this.height / (this.v1 - this.v0);
    return Math.max(height, width);
  }

  public uvShrinkRatio(): number {
    return 4.0 / this.atlasSize();
  }

  public getAnimationTicker(): Tickable | undefined {
    return this.animatedTexture;
  }

  public getX(): number {
    return this.x;
  }

  public getY(): number {
    return this.y;
  }

  public getWidth(): number {
    return this.width;
  }

  public getHeight(): number {
    return this.height;
  }

  public getU0(): number {
    return this.u0;
  }

  public getU1(): number {
    return this.u1;
  }

  public getU(value: number): number {
    const width = this.u1 - this.u0;
    return this.u0 + ((width * value) / 16.0);
  }

  public getUOffset(value: number): number {
    const width = this.u1 - this.u0;
    return ((value - this.u0) / width) * 16.0;
  }

  public getV0(): number {
    return this.v0;
  }

  public getV1(): number {
    return this.v1;
  }

  public getV(value: number): number {
    const height = this.v1 - this.v0;
    return this.v0 + ((height * value) / 16.0);
  }

  public getVOffset(value: number): number {
    const height = this.v1 - this.v0;
    return ((value - this.v0) / height) * 16.0;
  }

  public getName(): ResourceLocation {
    return this.name;
  }

  public atlas(): TextureAtlasUploadTarget {
    return this.atlasValue;
  }

  public getUniqueFrames(): number[] {
    return this.animatedTexture !== undefined ? this.animatedTexture.getUniqueFrames() : [1];
  }

  public isTransparent(frame: number, x: number, y: number): boolean {
    let pixelX = x;
    let pixelY = y;
    if (this.animatedTexture !== undefined) {
      pixelX = x + (this.animatedTexture.getFrameX(frame) * this.width);
      pixelY = y + (this.animatedTexture.getFrameY(frame) * this.height);
    }

    return NativeImage.getA(this.mainImage[0]!.getPixelRGBA(pixelX, pixelY)) === 0;
  }

  public uploadFirstFrame(): void {
    if (this.animatedTexture !== undefined) {
      this.animatedTexture.uploadFirstFrame();
      return;
    }

    this.upload(0, 0, this.mainImage);
  }

  public close(): void {
    for (const image of this.mainImage) {
      image?.close();
    }

    this.animatedTexture?.close();
  }

  public toString(): string {
    return `TextureAtlasSprite{name='${this.name}', frameCount=${this.getFrameCount()}, x=${this.x}, y=${this.y}, height=${this.height}, width=${this.width}, u0=${this.u0}, u1=${this.u1}, v0=${this.v0}, v1=${this.v1}}`;
  }
}
