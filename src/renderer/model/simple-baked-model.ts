import { Direction } from "../../core/direction";
import type { BlockState } from "../../world/level/block/state/block-state";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { BakedQuad } from "./baked-quad";
import { type BakedModel } from "./baked-model";
import { guiLightLikeBlock, type BlockModel } from "./block-model";
import { ItemOverrides } from "./item-overrides";
import { ItemTransforms } from "./item-transforms";

export class SimpleBakedModel implements BakedModel {
  public constructor(
    protected readonly unculledFaces: readonly BakedQuad[],
    protected readonly culledFaces: ReadonlyMap<Direction, readonly BakedQuad[]>,
    protected readonly hasAmbientOcclusionValue: boolean,
    protected readonly usesBlockLightValue: boolean,
    protected readonly isGui3dValue: boolean,
    protected readonly particleIcon: TextureAtlasSprite,
    protected readonly transforms: ItemTransforms,
    protected readonly overrides: ItemOverrides,
  ) {}

  public getQuads(_state?: BlockState, direction?: Direction, _random?: unknown): readonly BakedQuad[] {
    return direction === undefined ? this.unculledFaces : (this.culledFaces.get(direction) ?? []);
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
    return this.particleIcon;
  }

  public getTransforms(): ItemTransforms {
    return this.transforms;
  }

  public getOverrides(): ItemOverrides {
    return this.overrides;
  }
}

export namespace SimpleBakedModel {
  export class Builder {
    private readonly unculledFaces: BakedQuad[] = [];
    private readonly culledFaces = new Map<Direction, BakedQuad[]>();
    private particleIcon: TextureAtlasSprite | undefined;
    private readonly overrides: ItemOverrides;
    private readonly hasAmbientOcclusion: boolean;
    private readonly usesBlockLight: boolean;
    private readonly isGui3d: boolean;
    private readonly transforms: ItemTransforms;

    public constructor(model: BlockModel, overrides: ItemOverrides, isGui3d: boolean) {
      for (const direction of Direction.values()) {
        this.culledFaces.set(direction, []);
      }

      this.overrides = overrides;
      this.hasAmbientOcclusion = model.hasAmbientOcclusion();
      this.usesBlockLight = guiLightLikeBlock(model.getGuiLight());
      this.isGui3d = isGui3d;
      this.transforms = model.getTransforms();
    }

    public addCulledFace(direction: Direction, quad: BakedQuad): Builder {
      this.culledFaces.get(direction)!.push(quad);
      return this;
    }

    public addUnculledFace(quad: BakedQuad): Builder {
      this.unculledFaces.push(quad);
      return this;
    }

    public particle(sprite: TextureAtlasSprite): Builder {
      this.particleIcon = sprite;
      return this;
    }

    public item(): Builder {
      return this;
    }

    public build(): BakedModel {
      if (this.particleIcon === undefined) {
        throw new Error("Missing particle!");
      }

      const culledFaces = new Map<Direction, readonly BakedQuad[]>();
      for (const [direction, quads] of this.culledFaces.entries()) {
        culledFaces.set(direction, quads);
      }

      return new SimpleBakedModel(
        this.unculledFaces,
        culledFaces,
        this.hasAmbientOcclusion,
        this.usesBlockLight,
        this.isGui3d,
        this.particleIcon,
        this.transforms,
        this.overrides,
      );
    }
  }
}
