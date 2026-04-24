import { BlockPos } from "../../../core/block-pos";
import { SectionPos } from "../../../core/section-pos";
import { LightLayer } from "../light-layer";
import { DataLayer } from "../chunk/data-layer";
import type { LightChunkGetter } from "../chunk/light-chunk-getter";
import { DataLayerStorageMap } from "./data-layer-storage-map";
import { LayerLightSectionStorage } from "./layer-light-section-storage";

export class BlockDataLayerStorageMap extends DataLayerStorageMap<BlockDataLayerStorageMap> {
  public constructor(map = new Map<bigint, DataLayer>()) {
    super(map);
  }

  public copy(): BlockDataLayerStorageMap {
    return new BlockDataLayerStorageMap(this.cloneLayerMap());
  }
}

export class BlockLightSectionStorage extends LayerLightSectionStorage<BlockDataLayerStorageMap> {
  public constructor(chunkSource: LightChunkGetter) {
    super(LightLayer.BLOCK, chunkSource, new BlockDataLayerStorageMap());
  }

  public override getLightValue(pos: bigint): number {
    const section = SectionPos.blockToSection(pos);
    const dataLayer = this.getDataLayer(section, false);
    return dataLayer === undefined
      ? 0
      : dataLayer.get(
        SectionPos.sectionRelative(BlockPos.getX(pos)),
        SectionPos.sectionRelative(BlockPos.getY(pos)),
        SectionPos.sectionRelative(BlockPos.getZ(pos)),
      );
  }
}
