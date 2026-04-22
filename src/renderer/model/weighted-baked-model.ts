import { Direction } from "../../core/direction";
import type { BlockState } from "../../world/level/block/state/block-state";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { type BakedModel } from "./baked-model";
import { BakedQuad } from "./baked-quad";
import { ItemOverrides } from "./item-overrides";
import { ItemTransforms } from "./item-transforms";

type WeightedEntry<T> = {
  readonly data: T;
  readonly weight: number;
};

type RandomLike = {
  nextLong(): number | bigint;
};

function getWeightedIndex(totalWeight: number, random: unknown): number {
  if (totalWeight <= 0) {
    return 0;
  }

  if (random && typeof random === "object" && "nextLong" in random && typeof (random as RandomLike).nextLong === "function") {
    const value = (random as RandomLike).nextLong();
    if (typeof value === "bigint") {
      return Number((value < 0n ? -value : value) % BigInt(totalWeight));
    }

    return Math.abs(Math.trunc(value)) % totalWeight;
  }

  return 0;
}

export class WeightedBakedModel implements BakedModel {
  private readonly totalWeight: number;
  private readonly wrapped: BakedModel;

  public constructor(private readonly list: readonly WeightedEntry<BakedModel>[]) {
    this.totalWeight = list.reduce((sum, entry) => sum + entry.weight, 0);
    this.wrapped = list[0]!.data;
  }

  public getQuads(state?: BlockState, direction?: Direction, random?: unknown): readonly BakedQuad[] {
    let target = getWeightedIndex(this.totalWeight, random);
    for (const entry of this.list) {
      target -= entry.weight;
      if (target < 0) {
        return entry.data.getQuads(state, direction, random);
      }
    }

    return [];
  }

  public useAmbientOcclusion(): boolean {
    return this.wrapped.useAmbientOcclusion();
  }

  public isGui3d(): boolean {
    return this.wrapped.isGui3d();
  }

  public usesBlockLight(): boolean {
    return this.wrapped.usesBlockLight();
  }

  public isCustomRenderer(): boolean {
    return this.wrapped.isCustomRenderer();
  }

  public getParticleIcon(): TextureAtlasSprite {
    return this.wrapped.getParticleIcon();
  }

  public getTransforms(): ItemTransforms {
    return this.wrapped.getTransforms();
  }

  public getOverrides(): ItemOverrides {
    return this.wrapped.getOverrides();
  }
}

export namespace WeightedBakedModel {
  export class Builder {
    private readonly list: WeightedEntry<BakedModel>[] = [];

    public add(model: BakedModel | null | undefined, weight: number): Builder {
      if (model !== undefined && model !== null) {
        this.list.push({ data: model, weight });
      }

      return this;
    }

    public build(): BakedModel | undefined {
      if (this.list.length === 0) {
        return undefined;
      }

      if (this.list.length === 1) {
        return this.list[0]!.data;
      }

      return new WeightedBakedModel(this.list);
    }
  }
}
