import { Direction } from "../../core/direction";
import type { BlockState } from "../../world/level/block/state/block-state";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { type BakedModel } from "./baked-model";
import { BakedQuad } from "./baked-quad";
import { ItemOverrides } from "./item-overrides";
import { ItemTransforms } from "./item-transforms";

type SelectorEntry = {
  readonly predicate: (state: BlockState) => boolean;
  readonly model: BakedModel;
};

type RandomLike = {
  nextLong(): number | bigint;
};

class FixedRandom implements RandomLike {
  public constructor(private readonly value: number | bigint) {}

  public nextLong(): number | bigint {
    return this.value;
  }
}

function nextSeed(random: unknown): number | bigint {
  if (random && typeof random === "object" && "nextLong" in random && typeof (random as RandomLike).nextLong === "function") {
    return (random as RandomLike).nextLong();
  }

  return 0;
}

export class MultiPartBakedModel implements BakedModel {
  private readonly hasAmbientOcclusionValue: boolean;
  private readonly isGui3dValue: boolean;
  private readonly usesBlockLightValue: boolean;
  private readonly particleIconValue: TextureAtlasSprite;
  private readonly transformsValue: ItemTransforms;
  private readonly overridesValue: ItemOverrides;
  private readonly selectorCache = new Map<BlockState, boolean[]>();

  public constructor(private readonly selectors: readonly SelectorEntry[]) {
    const model = selectors[0]!.model;
    this.hasAmbientOcclusionValue = model.useAmbientOcclusion();
    this.isGui3dValue = model.isGui3d();
    this.usesBlockLightValue = model.usesBlockLight();
    this.particleIconValue = model.getParticleIcon();
    this.transformsValue = model.getTransforms();
    this.overridesValue = model.getOverrides();
  }

  public getQuads(state?: BlockState, direction?: Direction, random?: unknown): readonly BakedQuad[] {
    if (state === undefined) {
      return [];
    }

    let matches = this.selectorCache.get(state);
    if (matches === undefined) {
      matches = this.selectors.map((selector) => selector.predicate(state));
      this.selectorCache.set(state, matches);
    }

    const quads: BakedQuad[] = [];
    const seed = nextSeed(random);
    for (let index = 0; index < matches.length; index++) {
      if (matches[index]!) {
        quads.push(...this.selectors[index]!.model.getQuads(state, direction, new FixedRandom(seed)));
      }
    }

    return quads;
  }

  public useAmbientOcclusion(): boolean {
    return this.hasAmbientOcclusionValue;
  }

  public isGui3d(): boolean {
    return this.isGui3dValue;
  }

  public usesBlockLight(): boolean {
    return this.usesBlockLightValue;
  }

  public isCustomRenderer(): boolean {
    return false;
  }

  public getParticleIcon(): TextureAtlasSprite {
    return this.particleIconValue;
  }

  public getTransforms(): ItemTransforms {
    return this.transformsValue;
  }

  public getOverrides(): ItemOverrides {
    return this.overridesValue;
  }
}

export namespace MultiPartBakedModel {
  export class Builder {
    private readonly selectors: SelectorEntry[] = [];

    public add(predicate: (state: BlockState) => boolean, model: BakedModel): void {
      this.selectors.push({ predicate, model });
    }

    public build(): BakedModel {
      return new MultiPartBakedModel(this.selectors);
    }
  }
}
