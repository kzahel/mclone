import { BlockPos } from "../../core/block-pos";
import type { OpenWorldRequest } from "../protocol/world-messages";
import { connectWorldWorkerSession, type WorldWorkerHostEndpoint } from "../transport/worker-world-transport";
import { IndexedDbWorldStorage } from "../storage/indexeddb-world-storage";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import { registerGeneratedRenderBlocks } from "../../world/level/generated-render-blocks";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import { GeneratedWorldHost } from "./generated-world-host";

function applySmokeWorldMutations(level: WorldGenLevel, waterState: BlockState): void {
  for (let z = 35; z <= 39; z++) {
    for (let x = 42; x <= 47; x++) {
      level.setBlock(new BlockPos(x, 84, z), waterState);
    }
  }
}

function createGeneratedWorldHost(request: OpenWorldRequest): GeneratedWorldHost {
  const generatedBlocks = registerGeneratedRenderBlocks();

  return new GeneratedWorldHost({
    seed: request.seed,
    airState: generatedBlocks.airState,
    blockStateById: generatedBlocks.blockStateById,
    worldStorage: typeof indexedDB === "undefined" ? undefined : new IndexedDbWorldStorage(indexedDB),
    mutateWorld:
      request.preset === "browser_smoke"
        ? (level) => applySmokeWorldMutations(level, generatedBlocks.blockStateById[ChunkBlockId.WATER]!)
        : undefined,
  });
}

connectWorldWorkerSession(
  globalThis as unknown as WorldWorkerHostEndpoint,
  createGeneratedWorldHost,
);
