import { BlockPos } from "../../core/block-pos";
import { registerGeneratedRenderBlocks } from "../../world/level/generated-render-blocks";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import type { OpenWorldRequest, WorldEngineLightingMode, WorldEngineLiquidSimulationMode } from "../protocol/world-messages";
import type { WorldStorage } from "../storage/world-storage";
import { GeneratedWorldHost } from "./generated-world-host";

function applySmokeWorldMutations(level: WorldGenLevel, waterState: BlockState): void {
  for (let z = 35; z <= 39; z++) {
    for (let x = 42; x <= 47; x++) {
      level.setBlock(new BlockPos(x, 84, z), waterState);
    }
  }
}

export interface CreateGeneratedWorldHostOptions {
  readonly lightingMode?: WorldEngineLightingMode;
  readonly liquidSimulationMode?: WorldEngineLiquidSimulationMode;
  readonly chunkViewScheduling?: "synchronous" | "cooperative";
  readonly worldStorage?: WorldStorage;
}

export function createGeneratedWorldHostForRequest(
  request: OpenWorldRequest,
  options: CreateGeneratedWorldHostOptions = {},
): GeneratedWorldHost {
  const generatedBlocks = registerGeneratedRenderBlocks();

  return new GeneratedWorldHost({
    seed: request.seed,
    airState: generatedBlocks.airState,
    blockStateById: generatedBlocks.blockStateById,
    blockStateIds: generatedBlocks.blockStateIds,
    lightingMode: options.lightingMode ?? request.config?.lightingMode,
    liquidSimulationMode: options.liquidSimulationMode ?? request.config?.liquidSimulationMode,
    chunkViewScheduling: options.chunkViewScheduling,
    worldStorage: options.worldStorage,
    mutateWorld:
      request.preset === "browser_smoke"
        ? (level) => applySmokeWorldMutations(level, generatedBlocks.blockStateById[ChunkBlockId.WATER]!)
        : undefined,
  });
}
