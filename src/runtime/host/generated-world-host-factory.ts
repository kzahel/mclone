import { BlockPos } from "../../core/block-pos";
import { registerGeneratedRenderBlocks } from "../../world/level/generated-render-blocks";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import { createWorldGeneratorForPreset } from "../../worldgen/levelgen/world-generator-factory";
import type { OpenWorldRequest, WorldEngineLightingMode, WorldEngineLiquidSimulationMode } from "../protocol/world-messages";
import type { LightingService } from "../lighting/lighting-protocol";
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
  readonly lightingService?: LightingService;
}

export function createGeneratedWorldHostForRequest(
  request: OpenWorldRequest,
  options: CreateGeneratedWorldHostOptions = {},
): GeneratedWorldHost {
  const generatedBlocks = registerGeneratedRenderBlocks();
  const generator = createWorldGeneratorForPreset(request.preset, request.seed);

  return new GeneratedWorldHost({
    seed: request.seed,
    generator,
    airState: generatedBlocks.airState,
    blockStateById: generatedBlocks.blockStateById,
    blockStateIds: generatedBlocks.blockStateIds,
    lightingMode: options.lightingMode ?? request.config?.lightingMode,
    liquidSimulationMode: options.liquidSimulationMode ?? request.config?.liquidSimulationMode,
    chunkViewScheduling: options.chunkViewScheduling,
    worldStorage: options.worldStorage,
    lightingService: options.lightingService,
    mutateWorld:
      request.preset === "browser_smoke"
        ? (level) => applySmokeWorldMutations(level, generatedBlocks.blockStateById[ChunkBlockId.WATER]!)
        : undefined,
  });
}
