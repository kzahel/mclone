import { ChunkPos } from "../../src/core/chunk-pos";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { NoiseBasedChunkGenerator } from "../../src/worldgen/levelgen/noise-based-chunk-generator";
import { ConfiguredStructureFeatures, OVERWORLD_STRUCTURE_SETTINGS, StructureFeatures } from "../../src/worldgen/levelgen/structure/structure-features";

export interface BuriedTreasureTestSite {
  readonly seed: bigint;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly biomeKey: string;
}

let cachedSite: BuriedTreasureTestSite | undefined;

export function findBuriedTreasureTestSite(): BuriedTreasureTestSite {
  if (cachedSite !== undefined) {
    return cachedSite;
  }

  registerGeneratedRenderBlocks();
  const structureConfiguration = OVERWORLD_STRUCTURE_SETTINGS.getConfig(StructureFeatures.BURIED_TREASURE);
  if (structureConfiguration === undefined) {
    throw new Error("Missing buried treasure structure spacing configuration");
  }

  for (let seed = 0n; seed < 512n; seed++) {
    const generator = new NoiseBasedChunkGenerator(new OverworldBiomeSource(seed), seed);
    for (let chunkZ = -24; chunkZ <= 24; chunkZ++) {
      for (let chunkX = -24; chunkX <= 24; chunkX++) {
        const biome = generator.getPrimaryBiome(chunkX, chunkZ);
        const biomeKey = biome.getKey();
        if (biomeKey !== "minecraft:beach" && biomeKey !== "minecraft:snowy_beach") {
          continue;
        }

        const start = ConfiguredStructureFeatures.BURIED_TREASURE.generate(
          generator,
          new ChunkPos(chunkX, chunkZ),
          biome,
          0,
          structureConfiguration,
        );
        if (start !== undefined) {
          cachedSite = { seed, chunkX, chunkZ, biomeKey };
          return cachedSite;
        }
      }
    }
  }

  throw new Error("Could not find a deterministic buried treasure test site");
}
