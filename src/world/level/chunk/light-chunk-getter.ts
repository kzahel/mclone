import type { SectionPos } from "../../../core/section-pos";
import type { BlockGetter } from "../block-getter";
import type { LightLayer } from "../light-layer";
import type { LevelHeightAccessorLike } from "../light-section";

export interface LightChunkGetter {
  getChunkForLighting(chunkX: number, chunkZ: number): BlockGetter | null;

  onLightUpdate?(layer: LightLayer, section: SectionPos): void;

  getLevel(): BlockGetter & LevelHeightAccessorLike;
}
