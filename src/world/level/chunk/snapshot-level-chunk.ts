import { SectionPos } from "../../../core/section-pos";
import { BLOCKS_PER_SECTION, CHUNK_WIDTH, SECTION_HEIGHT } from "../../../worldgen/chunk/chunk-block-buffer";
import type { BlockStateResolver, ChunkSnapshot } from "../chunk-snapshot";
import type { BlockState } from "../block/state/block-state";
import type { ScheduledTickSnapshot } from "../scheduled-tick";
import { LevelChunk } from "./level-chunk";

type SnapshotSectionData = {
  readonly palette: readonly BlockState[];
  readonly blocks: readonly number[];
  readonly occupiedYMask: number;
};

function blockIndex(localX: number, localY: number, localZ: number): number {
  return (localY * CHUNK_WIDTH * CHUNK_WIDTH) + (localZ * CHUNK_WIDTH) + localX;
}

function rangeMask(localMinY: number, localMaxY: number): number {
  const bitCount = localMaxY - localMinY + 1;
  return ((1 << bitCount) - 1) << localMinY;
}

function buildOccupiedYMask(palette: readonly BlockState[], blocks: readonly number[]): number {
  let mask = 0;
  for (let localY = 0; localY < SECTION_HEIGHT; localY++) {
    const rowStart = localY * CHUNK_WIDTH * CHUNK_WIDTH;
    for (let localIndex = 0; localIndex < CHUNK_WIDTH * CHUNK_WIDTH; localIndex++) {
      if (!palette[blocks[rowStart + localIndex]!]!.isAir()) {
        mask |= 1 << localY;
        break;
      }
    }
  }

  return mask;
}

export class SnapshotLevelChunk extends LevelChunk {
  private readonly sections = new Map<number, SnapshotSectionData>();

  public constructor(
    snapshot: ChunkSnapshot,
    private readonly snapshotAirState: BlockState,
    resolveState: BlockStateResolver,
    private readonly scheduledBlockTicks: readonly ScheduledTickSnapshot[],
    private readonly scheduledLiquidTicks: readonly ScheduledTickSnapshot[],
  ) {
    super(snapshot.chunkX, snapshot.chunkZ, snapshotAirState);

    for (const section of snapshot.sections) {
      if (section.blocks.length !== BLOCKS_PER_SECTION) {
        throw new Error(
          `Chunk snapshot section (${snapshot.chunkX}, ${snapshot.chunkZ}, ${section.y}) had ${section.blocks.length} blocks instead of ${BLOCKS_PER_SECTION}`,
        );
      }

      const palette = section.palette.map(resolveState);
      this.sections.set(section.y, {
        palette,
        blocks: section.blocks,
        occupiedYMask: buildOccupiedYMask(palette, section.blocks),
      });
    }
  }

  public static fromSnapshot(
    snapshot: ChunkSnapshot,
    airState: BlockState,
    resolveState: BlockStateResolver,
  ): SnapshotLevelChunk {
    return new SnapshotLevelChunk(
      snapshot,
      airState,
      resolveState,
      [...snapshot.blockTicks],
      [...snapshot.liquidTicks],
    );
  }

  public override getBlockState(pos: import("../../../core/block-pos").BlockPos): BlockState {
    const sectionY = SectionPos.blockToSectionCoord(pos.getY());
    const section = this.sections.get(sectionY);
    if (section === undefined) {
      return this.snapshotAirState;
    }

    const sectionMinY = SectionPos.sectionToBlockCoord(sectionY);
    const localX = pos.getX() & 15;
    const localY = pos.getY() - sectionMinY;
    const localZ = pos.getZ() & 15;
    return section.palette[section.blocks[blockIndex(localX, localY, localZ)]!] ?? this.snapshotAirState;
  }

  public override isYSpaceEmpty(minY: number, maxY: number): boolean {
    const minSectionY = SectionPos.blockToSectionCoord(minY);
    const maxSectionY = SectionPos.blockToSectionCoord(maxY);

    for (let sectionY = minSectionY; sectionY <= maxSectionY; sectionY++) {
      const section = this.sections.get(sectionY);
      if (section === undefined || section.occupiedYMask === 0) {
        continue;
      }

      const sectionMinY = SectionPos.sectionToBlockCoord(sectionY);
      const localMinY = Math.max(0, minY - sectionMinY);
      const localMaxY = Math.min(SECTION_HEIGHT - 1, maxY - sectionMinY);
      if ((section.occupiedYMask & rangeMask(localMinY, localMaxY)) !== 0) {
        return false;
      }
    }

    return true;
  }

  public override getScheduledBlockTicks(): readonly ScheduledTickSnapshot[] {
    return this.scheduledBlockTicks;
  }

  public override getScheduledLiquidTicks(): readonly ScheduledTickSnapshot[] {
    return this.scheduledLiquidTicks;
  }
}
