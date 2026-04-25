import { BlockPos } from "../../../core/block-pos";
import { SectionPos } from "../../../core/section-pos";
import type { BlockState } from "../block/state/block-state";
import type { FluidState } from "../material/fluid-state";
import { Heightmap } from "../../../worldgen/levelgen/heightmap";
import { LevelChunkSection } from "./level-chunk-section";
import {
  cloneScheduledTickSnapshot,
  createScheduledTickSnapshot,
  type ScheduledTickSnapshot,
} from "../scheduled-tick";

export type ChunkEntry = {
  readonly pos: BlockPos;
  readonly state: BlockState;
};

export class LevelChunk {
  private readonly chunkSections = new Map<number, LevelChunkSection>();
  private readonly blockTicks: ScheduledTickSnapshot[] = [];
  private readonly liquidTicks: ScheduledTickSnapshot[] = [];

  public constructor(
    public readonly chunkX: number,
    public readonly chunkZ: number,
    private readonly airState: BlockState,
    private readonly minBuildHeight = 0,
    private readonly height = 256,
  ) {}

  private readonly heightmaps = new Map<Heightmap.Types, Heightmap>();

  public getBlockState(pos: BlockPos): BlockState {
    return this.getSection(SectionPos.blockToSectionCoord(pos.getY()), false)
      ?.getBlockState(pos.getX() & 15, pos.getY() & 15, pos.getZ() & 15)
      ?? this.airState;
  }

  public getFluidState(pos: BlockPos): FluidState {
    return this.getBlockState(pos).getFluidState();
  }

  public getMaxLightLevel(): number {
    return 15;
  }

  public setBlockState(pos: BlockPos, state: BlockState): void {
    const sectionY = SectionPos.blockToSectionCoord(pos.getY());
    const section = this.getSection(sectionY, state !== this.airState);
    if (section === undefined) {
      return;
    }

    section.setBlockState(pos.getX() & 15, pos.getY() & 15, pos.getZ() & 15, state);
    if (!section.hasStoredBlocks()) {
      this.chunkSections.delete(sectionY);
    }
    for (const heightmap of this.heightmaps.values()) {
      heightmap.update(pos.getX() & 15, pos.getY(), pos.getZ() & 15, state);
    }
  }

  public getMinBuildHeight(): number {
    return this.minBuildHeight;
  }

  public getHeight(): number;
  public getHeight(type: Heightmap.Types, x: number, z: number): number;
  public getHeight(type?: Heightmap.Types, x?: number, z?: number): number {
    if (type === undefined || x === undefined || z === undefined) {
      return this.height;
    }

    let heightmap = this.heightmaps.get(type);
    if (heightmap === undefined) {
      Heightmap.primeHeightmaps(this, [type]);
      heightmap = this.heightmaps.get(type);
      if (heightmap === undefined) {
        throw new Error(`Failed to prime heightmap ${type.toString()} for chunk (${this.chunkX.toString()}, ${this.chunkZ.toString()})`);
      }
    }

    return heightmap.getHighestTaken(x & 15, z & 15);
  }

  public getMaxBuildHeight(): number {
    return this.minBuildHeight + this.height;
  }

  public getOrCreateHeightmapUnprimed(type: Heightmap.Types): Heightmap {
    let heightmap = this.heightmaps.get(type);
    if (heightmap === undefined) {
      heightmap = new Heightmap(this, type);
      this.heightmaps.set(type, heightmap);
    }

    return heightmap;
  }

  public primeHeightmaps(types: Iterable<Heightmap.Types>): void {
    Heightmap.primeHeightmaps(this, types);
  }

  public isYSpaceEmpty(minY: number, maxY: number): boolean {
    const minSectionY = SectionPos.blockToSectionCoord(minY);
    const maxSectionY = SectionPos.blockToSectionCoord(maxY);
    for (let sectionY = minSectionY; sectionY <= maxSectionY; sectionY++) {
      const section = this.chunkSections.get(sectionY);
      if (section === undefined) {
        continue;
      }

      const sectionMinY = SectionPos.sectionToBlockCoord(sectionY);
      const localMinY = Math.max(0, minY - sectionMinY);
      const localMaxY = Math.min(15, maxY - sectionMinY);
      if (!section.isYSpaceEmpty(localMinY, localMaxY)) {
        return false;
      }
    }

    return true;
  }

  public getBlockEntries(): Iterable<ChunkEntry> {
    return this.iterBlockEntries();
  }

  public recordBlockTick(pos: BlockPos, target: string, delay: number): void {
    this.blockTicks.push(createScheduledTickSnapshot(pos, target, delay));
  }

  public recordLiquidTick(pos: BlockPos, target: string, delay: number): void {
    this.liquidTicks.push(createScheduledTickSnapshot(pos, target, delay));
  }

  public appendBlockTicks(ticks: readonly ScheduledTickSnapshot[]): void {
    for (const tick of ticks) {
      this.blockTicks.push(cloneScheduledTickSnapshot(tick));
    }
  }

  public appendLiquidTicks(ticks: readonly ScheduledTickSnapshot[]): void {
    for (const tick of ticks) {
      this.liquidTicks.push(cloneScheduledTickSnapshot(tick));
    }
  }

  public getScheduledBlockTicks(): readonly ScheduledTickSnapshot[] {
    return this.blockTicks;
  }

  public getScheduledLiquidTicks(): readonly ScheduledTickSnapshot[] {
    return this.liquidTicks;
  }

  public consumeScheduledLiquidTicks(): readonly ScheduledTickSnapshot[] {
    const ticks = this.liquidTicks.map(cloneScheduledTickSnapshot);
    this.liquidTicks.length = 0;
    return ticks;
  }

  private getSection(sectionY: number, create: boolean): LevelChunkSection | undefined {
    let section = this.chunkSections.get(sectionY);
    if (section === undefined && create) {
      section = new LevelChunkSection(sectionY, this.airState);
      this.chunkSections.set(sectionY, section);
    }

    return section;
  }

  private *iterBlockEntries(): Iterable<ChunkEntry> {
    const sectionYs = [...this.chunkSections.keys()].sort((left, right) => left - right);
    for (const sectionY of sectionYs) {
      yield* this.chunkSections.get(sectionY)!.blockEntries(this.chunkX, this.chunkZ);
    }
  }
}
