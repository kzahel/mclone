import { Direction } from "../../core/direction";
import type { BlockState } from "../../world/level/block/state/block-state";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { BakedQuad } from "./baked-quad";
import { ItemOverrides } from "./item-overrides";
import { ItemTransforms } from "./item-transforms";

export interface BakedModel {
  getQuads(state?: BlockState, direction?: Direction, random?: unknown): readonly BakedQuad[];

  useAmbientOcclusion(): boolean;

  isGui3d(): boolean;

  usesBlockLight(): boolean;

  isCustomRenderer(): boolean;

  getParticleIcon(): TextureAtlasSprite;

  getTransforms(): ItemTransforms;

  getOverrides(): ItemOverrides;
}
