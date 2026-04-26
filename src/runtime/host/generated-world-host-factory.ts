import { BlockPos } from "../../core/block-pos";
import { registerGeneratedRenderBlocks } from "../../world/level/generated-render-blocks";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import { createWorldGeneratorForPreset } from "../../worldgen/levelgen/world-generator-factory";
import type { WorldGenerator } from "../../worldgen/levelgen/world-generator";
import { NoneFeatureConfiguration } from "../../worldgen/levelgen/feature/configurations/none-feature-configuration";
import { Features } from "../../worldgen/levelgen/feature/features";
import { WorldgenRandom } from "../../worldgen/prng/worldgen-random";
import type { OpenWorldRequest, WorldEngineLightingMode, WorldEngineLiquidSimulationMode } from "../protocol/world-messages";
import type { LightingService } from "../lighting/lighting-protocol";
import type { WorldStorage } from "../storage/world-storage";
import { GeneratedWorldHost } from "./generated-world-host";

function applySmokeWorldMutations(
  level: WorldGenLevel,
  waterState: BlockState,
  sandState: BlockState,
  sandstoneState: BlockState,
  generator: WorldGenerator,
  seed: bigint,
): void {
  for (let z = 35; z <= 39; z++) {
    for (let x = 42; x <= 47; x++) {
      level.setBlock(new BlockPos(x, 84, z), waterState);
    }
  }

  const center = new BlockPos(168, 192, 168);
  for (let offsetX = -6; offsetX <= 6; offsetX++) {
    for (let offsetZ = -6; offsetZ <= 6; offsetZ++) {
      level.setBlock(center.offset(offsetX, -2, offsetZ), sandstoneState, 2);
      level.setBlock(center.offset(offsetX, -1, offsetZ), sandstoneState, 2);
      level.setBlock(center.offset(offsetX, 0, offsetZ), sandState, 2);
    }
  }

  Features.DESERT_WELL.configured(NoneFeatureConfiguration.INSTANCE).place(level, generator, new WorldgenRandom(seed), center);
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
        ? (level) =>
            applySmokeWorldMutations(
              level,
              generatedBlocks.blockStateById[ChunkBlockId.WATER]!,
              generatedBlocks.blockStateById[ChunkBlockId.SAND]!,
              generatedBlocks.blockStateById[ChunkBlockId.SANDSTONE]!,
              generator,
              request.seed,
            )
        : undefined,
  });
}
