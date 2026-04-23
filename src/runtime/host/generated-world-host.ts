import { SectionPos } from "../../core/section-pos";
import { OverworldBiomeSource } from "../../worldgen/biome/overworld-biome-source";
import { ChunkBiomeContainer } from "../../worldgen/biome/chunk-biome-container";
import { NoiseBasedChunkGenerator } from "../../worldgen/levelgen/noise-based-chunk-generator";
import { buildChunkSnapshot } from "../../world/level/chunk-snapshot";
import { GeneratedRenderLevel } from "../../world/level/generated-render-level";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import type { WorldHost } from "../protocol/world-host";
import type { OpenWorldRequest, SetChunkViewRequest, WorldHostMessage } from "../protocol/world-messages";

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

export interface GeneratedWorldHostOptions {
  readonly seed: bigint;
  readonly airState: BlockState;
  readonly blockStateById: readonly BlockState[];
  readonly mutateWorld?: (level: WorldGenLevel) => void;
}

export class GeneratedWorldHost implements WorldHost {
  private readonly biomeSource;
  private readonly generator;
  private readonly level;
  private opened = false;

  public constructor(private readonly options: GeneratedWorldHostOptions) {
    this.biomeSource = new OverworldBiomeSource(options.seed);
    this.generator = new NoiseBasedChunkGenerator(this.biomeSource, options.seed);
    this.level = new GeneratedRenderLevel(
      options.airState,
      this.generator,
      this.biomeSource,
      options.seed,
      options.blockStateById,
    );
  }

  public async openWorld(_request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    this.opened = true;
    return [
      {
        type: "world_opened",
        minBuildHeight: this.level.getMinBuildHeight(),
        height: this.level.getHeight(),
      },
    ];
  }

  public async setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    if (!this.opened) {
      return [{ type: "world_error", message: "set_chunk_view received before open_world" }];
    }

    const loadedBefore = new Map(this.level.getLoadedChunks().map((chunk) => [chunkKey(chunk.chunkX, chunk.chunkZ), chunk] as const));
    const changed = this.level.ensureChunksForCamera(
      SectionPos.sectionToBlockCoord(request.centerChunkX) + 8,
      SectionPos.sectionToBlockCoord(request.centerChunkZ) + 8,
      request.radius,
    );
    if (!changed) {
      return [];
    }

    this.options.mutateWorld?.(this.level);

    const loadedAfter = this.level.getLoadedChunks();
    const loadedAfterKeys = new Set(loadedAfter.map((chunk) => chunkKey(chunk.chunkX, chunk.chunkZ)));
    const messages: WorldHostMessage[] = [];

    for (const [key, chunk] of loadedBefore) {
      if (loadedAfterKeys.has(key)) {
        continue;
      }

      messages.push({
        type: "chunk_unload",
        chunkX: chunk.chunkX,
        chunkZ: chunk.chunkZ,
      });
    }

    for (const chunk of loadedAfter) {
      messages.push({
        type: "chunk_snapshot",
        snapshot: buildChunkSnapshot(
          chunk,
          new ChunkBiomeContainer(
            this.level.getMinBuildHeight(),
            this.level.getHeight(),
            chunk.chunkX,
            chunk.chunkZ,
            this.biomeSource,
          ).writeBiomes(),
          this.level.getMinBuildHeight(),
          this.level.getHeight(),
        ),
      });
    }

    return messages;
  }
}
