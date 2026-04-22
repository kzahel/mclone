import { Direction } from "../../core/direction";
import type { BlockState } from "../../world/level/block/state/block-state";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { BakedQuad } from "./baked-quad";
import { type BakedModel } from "./baked-model";
import { ItemOverrides } from "./item-overrides";
import { ItemTransforms } from "./item-transforms";

export class BuiltInModel implements BakedModel {
  public constructor(
    private readonly itemTransforms: ItemTransforms,
    private readonly overrides: ItemOverrides,
    private readonly particleTexture: TextureAtlasSprite,
    private readonly usesBlockLightValue: boolean,
  ) {}

  public getQuads(_state?: BlockState, _direction?: Direction, _random?: unknown): readonly BakedQuad[] {
    return [];
  }

  public useAmbientOcclusion(): boolean {
    return false;
  }

  public isGui3d(): boolean {
    return true;
  }

  public usesBlockLight(): boolean {
    return this.usesBlockLightValue;
  }

  public isCustomRenderer(): boolean {
    return true;
  }

  public getParticleIcon(): TextureAtlasSprite {
    return this.particleTexture;
  }

  public getTransforms(): ItemTransforms {
    return this.itemTransforms;
  }

  public getOverrides(): ItemOverrides {
    return this.overrides;
  }
}
