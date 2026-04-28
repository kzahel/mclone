import type { BlockPos } from "../../../../../core/block-pos";
import type { WorldGenLevel } from "../../../world-gen-level";
import type { StructurePlaceSettings } from "./structure-place-settings";
import { StructureProcessor } from "./structure-processor";
import type { StructureBlockInfo } from "./structure-template";

export class BlockRotProcessor extends StructureProcessor {
  public constructor(private readonly integrity: number) {
    super();
  }

  public override processBlock(
    _level: WorldGenLevel,
    _structureOrigin: BlockPos,
    _placementOrigin: BlockPos,
    _originalBlockInfo: StructureBlockInfo,
    currentBlockInfo: StructureBlockInfo,
    settings: StructurePlaceSettings,
  ): StructureBlockInfo | undefined {
    const random = settings.getRandom(currentBlockInfo.pos);
    return this.integrity < 1.0 && random.nextFloat() > this.integrity ? undefined : currentBlockInfo;
  }
}
