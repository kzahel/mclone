import type { BlockPos } from "../../../../../core/block-pos";
import type { WorldGenLevel } from "../../../world-gen-level";
import type { StructurePlaceSettings } from "./structure-place-settings";
import type { StructureBlockInfo } from "./structure-template";

export abstract class StructureProcessor {
  public abstract processBlock(
    level: WorldGenLevel,
    structureOrigin: BlockPos,
    placementOrigin: BlockPos,
    originalBlockInfo: StructureBlockInfo,
    currentBlockInfo: StructureBlockInfo,
    settings: StructurePlaceSettings,
  ): StructureBlockInfo | undefined;
}
