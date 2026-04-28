import { afterEach, beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { ChunkPos } from "../../../../src/core/chunk-pos";
import { Registry } from "../../../../src/core/registry";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import { GeneratedChunkStatus } from "../../../../src/world/level/generated-chunk-status";
import { GeneratedRenderLevel } from "../../../../src/world/level/generated-render-level";
import { StructureFeatureManager } from "../../../../src/world/level/structure-feature-manager";
import { getOverworldBiomeGenerationSettings } from "../../../../src/worldgen/biome/overworld-biome-generation-settings";
import { OverworldBiomeSource } from "../../../../src/worldgen/biome/overworld-biome-source";
import { NoiseBasedChunkGenerator } from "../../../../src/worldgen/levelgen/noise-based-chunk-generator";
import { StructureFeatures } from "../../../../src/worldgen/levelgen/structure/structure-features";
import { findBuriedTreasureTestSite } from "../../../support/buried-treasure-test-site";

function createGeneratedLevel(seed: bigint): GeneratedRenderLevel {
  const blocks = registerGeneratedRenderBlocks();
  const generator = new NoiseBasedChunkGenerator(new OverworldBiomeSource(seed), seed);
  return new GeneratedRenderLevel(blocks.airState, generator, blocks.blockStateById);
}

describe("Buried treasure structure feature", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("beaches and snowy beaches include buried treasure starts while stone shores do not", () => {
    registerGeneratedRenderBlocks();

    const beachStructures = getOverworldBiomeGenerationSettings("minecraft:beach").structures().map((supplier) => supplier().feature);
    const snowyBeachStructures = getOverworldBiomeGenerationSettings("minecraft:snowy_beach").structures().map((supplier) => supplier().feature);
    const stoneShoreStructures = getOverworldBiomeGenerationSettings("minecraft:stone_shore").structures().map((supplier) => supplier().feature);

    expect(beachStructures).toContain(StructureFeatures.BURIED_TREASURE);
    expect(snowyBeachStructures).toContain(StructureFeatures.BURIED_TREASURE);
    expect(stoneShoreStructures).not.toContain(StructureFeatures.BURIED_TREASURE);
  });

  test("generated metadata produces buried treasure starts, references, and the final chest block", () => {
    registerGeneratedRenderBlocks();
    const site = findBuriedTreasureTestSite();
    const level = createGeneratedLevel(site.seed);
    const structureManager = new StructureFeatureManager(level);

    level.updateChunkView(site.chunkX, site.chunkZ, 1);
    expect(level.markMetadataStatusAtLeast(site.chunkX, site.chunkZ, GeneratedChunkStatus.STRUCTURE_REFERENCES)).toBe(true);

    const start = level.getStartForFeature(site.chunkX, site.chunkZ, StructureFeatures.BURIED_TREASURE);
    expect(start).toBeDefined();
    expect(level.hasChunkStatus(site.chunkX, site.chunkZ, GeneratedChunkStatus.STRUCTURE_STARTS)).toBe(true);
    expect(level.hasChunkStatus(site.chunkX, site.chunkZ, GeneratedChunkStatus.STRUCTURE_REFERENCES)).toBe(true);
    expect(level.getReferencesForFeature(site.chunkX, site.chunkZ, StructureFeatures.BURIED_TREASURE)).toContain(
      ChunkPos.asLong(site.chunkX, site.chunkZ),
    );
    expect(structureManager.startsForFeature(site.chunkX, site.chunkZ, StructureFeatures.BURIED_TREASURE)).toHaveLength(1);

    expect(level.generateChunkTerrain(site.chunkX, site.chunkZ)).not.toBeNull();
    level.decorateChunk(site.chunkX, site.chunkZ);

    const placedStart = level.getStartForFeature(site.chunkX, site.chunkZ, StructureFeatures.BURIED_TREASURE);
    expect(placedStart).toBeDefined();
    const pieceBox = placedStart!.getPieces()[0]!.getBoundingBox();
    const chestPos = new BlockPos(pieceBox.minX(), pieceBox.minY(), pieceBox.minZ());
    const chestBlockKey = Registry.BLOCK.getKey(level.getBlockState(chestPos).getBlock() as unknown as object)?.toString();
    expect(chestBlockKey).toBe("minecraft:chest");
  });
});
