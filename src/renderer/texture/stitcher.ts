import { smallestEncompassingPowerOfTwo } from "../../util/mth";
import { StitcherException } from "./stitcher-exception";
import type { TextureAtlasSpriteInfo } from "./texture-atlas-sprite";

function holderComparator(left: StitcherHolder, right: StitcherHolder): number {
  if (left.height !== right.height) {
    return right.height - left.height;
  }

  if (left.width !== right.width) {
    return right.width - left.width;
  }

  return left.spriteInfo.name().compareTo(right.spriteInfo.name());
}

export type StitcherSpriteLoader = (info: TextureAtlasSpriteInfo, atlasWidth: number, atlasHeight: number, x: number, y: number) => void;

export class StitcherHolder {
  public readonly width: number;
  public readonly height: number;

  public constructor(
    public readonly spriteInfo: TextureAtlasSpriteInfo,
    mipLevel: number,
  ) {
    this.width = Stitcher.smallestFittingMinTexel(spriteInfo.width(), mipLevel);
    this.height = Stitcher.smallestFittingMinTexel(spriteInfo.height(), mipLevel);
  }

  public toString(): string {
    return `Holder{width=${this.width}, height=${this.height}}`;
  }
}

export class StitcherRegion {
  private subSlots: StitcherRegion[] | undefined;
  private holder: StitcherHolder | undefined;

  public constructor(
    private readonly originX: number,
    private readonly originY: number,
    private readonly width: number,
    private readonly height: number,
  ) {}

  public getHolder(): StitcherHolder | undefined {
    return this.holder;
  }

  public getX(): number {
    return this.originX;
  }

  public getY(): number {
    return this.originY;
  }

  public add(holder: StitcherHolder): boolean {
    if (this.holder !== undefined) {
      return false;
    }

    const width = holder.width;
    const height = holder.height;
    if (width > this.width || height > this.height) {
      return false;
    }

    if (width === this.width && height === this.height) {
      this.holder = holder;
      return true;
    }

    if (this.subSlots === undefined) {
      this.subSlots = [new StitcherRegion(this.originX, this.originY, width, height)];
      const remainingWidth = this.width - width;
      const remainingHeight = this.height - height;
      if (remainingHeight > 0 && remainingWidth > 0) {
        const maxHeight = Math.max(this.height, remainingWidth);
        const maxWidth = Math.max(this.width, remainingHeight);
        if (maxHeight >= maxWidth) {
          this.subSlots.push(new StitcherRegion(this.originX, this.originY + height, width, remainingHeight));
          this.subSlots.push(new StitcherRegion(this.originX + width, this.originY, remainingWidth, this.height));
        } else {
          this.subSlots.push(new StitcherRegion(this.originX + width, this.originY, remainingWidth, height));
          this.subSlots.push(new StitcherRegion(this.originX, this.originY + height, this.width, remainingHeight));
        }
      } else if (remainingWidth === 0) {
        this.subSlots.push(new StitcherRegion(this.originX, this.originY + height, width, remainingHeight));
      } else if (remainingHeight === 0) {
        this.subSlots.push(new StitcherRegion(this.originX + width, this.originY, remainingWidth, height));
      }
    }

    for (const slot of this.subSlots) {
      if (slot.add(holder)) {
        return true;
      }
    }

    return false;
  }

  public walk(consumer: (region: StitcherRegion) => void): void {
    if (this.holder !== undefined) {
      consumer(this);
    } else if (this.subSlots !== undefined) {
      for (const slot of this.subSlots) {
        slot.walk(consumer);
      }
    }
  }

  public toString(): string {
    return `Slot{originX=${this.originX}, originY=${this.originY}, width=${this.width}, height=${this.height}, texture=${this.holder}, subSlots=${this.subSlots}}`;
  }
}

export class Stitcher {
  private readonly texturesToBeStitched = new Set<StitcherHolder>();
  private readonly storage: StitcherRegion[] = [];
  private storageX = 0;
  private storageY = 0;

  public constructor(
    private readonly maxWidth: number,
    private readonly maxHeight: number,
    private readonly mipLevel: number,
  ) {}

  public getWidth(): number {
    return this.storageX;
  }

  public getHeight(): number {
    return this.storageY;
  }

  public registerSprite(info: TextureAtlasSpriteInfo): void {
    this.texturesToBeStitched.add(new StitcherHolder(info, this.mipLevel));
  }

  public stitch(): void {
    const holders = [...this.texturesToBeStitched];
    holders.sort(holderComparator);

    for (const holder of holders) {
      if (!this.addToStorage(holder)) {
        throw new StitcherException(holder.spriteInfo, holders.map((entry) => entry.spriteInfo));
      }
    }

    this.storageX = smallestEncompassingPowerOfTwo(this.storageX);
    this.storageY = smallestEncompassingPowerOfTwo(this.storageY);
  }

  public gatherSprites(loader: StitcherSpriteLoader): void {
    for (const slot of this.storage) {
      slot.walk((region) => {
        const holder = region.getHolder();
        if (!holder) {
          return;
        }

        loader(holder.spriteInfo, this.storageX, this.storageY, region.getX(), region.getY());
      });
    }
  }

  public static smallestFittingMinTexel(value: number, mipLevel: number): number {
    return (((value >> mipLevel) + ((value & ((1 << mipLevel) - 1)) === 0 ? 0 : 1)) << mipLevel) >>> 0;
  }

  private addToStorage(holder: StitcherHolder): boolean {
    for (const region of this.storage) {
      if (region.add(holder)) {
        return true;
      }
    }

    return this.expand(holder);
  }

  private expand(holder: StitcherHolder): boolean {
    const storagePowerX = smallestEncompassingPowerOfTwo(this.storageX);
    const storagePowerY = smallestEncompassingPowerOfTwo(this.storageY);
    const expandedPowerX = smallestEncompassingPowerOfTwo(this.storageX + holder.width);
    const expandedPowerY = smallestEncompassingPowerOfTwo(this.storageY + holder.height);
    const canExpandX = expandedPowerX <= this.maxWidth;
    const canExpandY = expandedPowerY <= this.maxHeight;
    if (!canExpandX && !canExpandY) {
      return false;
    }

    const growX = canExpandX && storagePowerX !== expandedPowerX;
    const growY = canExpandY && storagePowerY !== expandedPowerY;
    let expandHorizontally: boolean;
    if (growX !== growY) {
      expandHorizontally = growX;
    } else {
      expandHorizontally = canExpandX && storagePowerX <= storagePowerY;
    }

    let region: StitcherRegion;
    if (expandHorizontally) {
      if (this.storageY === 0) {
        this.storageY = holder.height;
      }

      region = new StitcherRegion(this.storageX, 0, holder.width, this.storageY);
      this.storageX += holder.width;
    } else {
      region = new StitcherRegion(0, this.storageY, this.storageX, holder.height);
      this.storageY += holder.height;
    }

    region.add(holder);
    this.storage.push(region);
    return true;
  }
}
