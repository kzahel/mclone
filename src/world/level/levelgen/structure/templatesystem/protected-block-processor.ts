import type { BlockPos } from "../../../../../core/block-pos";
import type { BlockTag } from "../../../../../tags/block-tags";
import { Feature } from "../../../../../worldgen/levelgen/feature/feature";
import type { WorldGenLevel } from "../../../world-gen-level";
import type { StructurePlaceSettings } from "./structure-place-settings";
import { StructureProcessor } from "./structure-processor";
import type { StructureBlockInfo } from "./structure-template";

export class ProtectedBlockProcessor extends StructureProcessor {
  public constructor(private readonly cannotReplace: BlockTag) {
    super();
  }

  public override processBlock(
    level: WorldGenLevel,
    _structureOrigin: BlockPos,
    _placementOrigin: BlockPos,
    _originalBlockInfo: StructureBlockInfo,
    currentBlockInfo: StructureBlockInfo,
    _settings: StructurePlaceSettings,
  ): StructureBlockInfo | undefined {
    return Feature.isReplaceable(this.cannotReplace)(level.getBlockState(currentBlockInfo.pos)) ? currentBlockInfo : undefined;
  }
}
