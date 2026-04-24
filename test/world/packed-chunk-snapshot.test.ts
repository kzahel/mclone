import { describe, expect, test } from "vitest";
import committedIntegration from "../fixtures/integration/overworld-seed-12345-chunks-0-0.json" with { type: "json" };
import terrainOnly from "../fixtures/integration/overworld-seed-12345-chunks-0-0-terrain-only.json" with { type: "json" };
import { Registry } from "../../src/core/registry";
import { BlockPos } from "../../src/core/block-pos";
import { Block } from "../../src/world/level/block/block";
import { LevelChunk } from "../../src/world/level/chunk/level-chunk";
import {
  CHUNK_SNAPSHOT_BLOCK_ORDER,
  buildChunkSnapshot,
  createBlockStateResolver,
  hydrateChunkFromSnapshot,
  serializeBlockStateSnapshot,
  type BlockStateSnapshot,
  type ChunkSectionSnapshot,
  type ChunkSnapshot,
} from "../../src/world/level/chunk-snapshot";
import {
  bitsForLocalPalette,
  clonePackedChunkSnapshot,
  collectPackedChunkSnapshotTransferables,
  packChunkSection,
  packChunkSnapshot,
  unpackChunkSection,
  unpackChunkSnapshot,
  type PackedChunkLight,
  type PackedChunkSection,
} from "../../src/world/level/packed-chunk-snapshot";
import { buildBlockStateIdMap } from "../../src/world/level/block/state/block-state-id";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import type { BlockState } from "../../src/world/level/block/state/block-state";
import { BLOCKS_PER_SECTION } from "../../src/worldgen/chunk/chunk-block-buffer";
import { BitStorage } from "../../src/util/bit-storage";
import {
  compareChunkLight,
  decodeChunkLightFixture,
  type ChunkLightFixture,
} from "../../src/oracle/integration/light-fixture";

interface TerrainOnlyFixture {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly minY: number;
  readonly height: number;
  readonly blockOrder: string;
  readonly palette: readonly string[];
  readonly blocks: readonly number[];
}

interface IntegrationFixtureWithLight {
  readonly chunks: readonly {
    readonly isLightOn: boolean;
    readonly light: ChunkLightFixture;
  }[];
}

function registeredBlocks(): Iterable<Block> {
  return Registry.BLOCK as Iterable<Block>;
}

function createCodecContext() {
  const { airState } = registerGeneratedRenderBlocks();
  return {
    airState,
    stateIds: buildBlockStateIdMap(registeredBlocks()),
    resolveState: createBlockStateResolver(airState),
  };
}

function sectionFromStates(states: readonly BlockState[], fill: (index: number) => number, y = 0): ChunkSectionSnapshot {
  return {
    y,
    palette: states.map(serializeBlockStateSnapshot),
    blockOrder: CHUNK_SNAPSHOT_BLOCK_ORDER,
    blocks: Array.from({ length: BLOCKS_PER_SECTION }, (_, index) => fill(index)),
  };
}

function nonAirStates(states: readonly BlockState[]): readonly BlockState[] {
  return states.filter((state) => !state.isAir());
}

function terrainFixtureSnapshot(): ChunkSnapshot {
  const fixture = terrainOnly as TerrainOnlyFixture;
  const sectionCount = fixture.height / 16;
  const sections: ChunkSectionSnapshot[] = [];
  for (let sectionOffset = 0; sectionOffset < sectionCount; sectionOffset++) {
    const start = sectionOffset * BLOCKS_PER_SECTION;
    const sectionBlocks = fixture.blocks.slice(start, start + BLOCKS_PER_SECTION);
    const usedPaletteIds = [...new Set(sectionBlocks)].sort((left, right) => left - right);
    const hasNonAir = usedPaletteIds.some((id) => fixture.palette[id] !== "minecraft:air");
    if (!hasNonAir) {
      continue;
    }

    const localIndexByGlobal = new Map<number, number>();
    const palette: BlockStateSnapshot[] = [];
    for (const globalPaletteId of usedPaletteIds) {
      localIndexByGlobal.set(globalPaletteId, palette.length);
      palette.push({ name: fixture.palette[globalPaletteId]! });
    }

    sections.push({
      y: Math.floor(fixture.minY / 16) + sectionOffset,
      palette,
      blockOrder: CHUNK_SNAPSHOT_BLOCK_ORDER,
      blocks: sectionBlocks.map((globalPaletteId) => localIndexByGlobal.get(globalPaletteId)!),
    });
  }

  return {
    chunkX: fixture.chunkX,
    chunkZ: fixture.chunkZ,
    biomes: [],
    sections,
    blockTicks: [],
    liquidTicks: [],
  };
}

function committedFixtureLight(): PackedChunkLight {
  const fixture = committedIntegration as unknown as IntegrationFixtureWithLight;
  return {
    ...decodeChunkLightFixture(fixture.chunks[0]!.light),
    lightCorrect: fixture.chunks[0]!.isLightOn,
  };
}

describe("packed chunk section codecs", () => {
  test.each([1, 2, 16, 17])("round-trips palette size %i", (paletteSize) => {
    const { stateIds, resolveState } = createCodecContext();
    const states = paletteSize === 1
      ? [stateIds.getStates()[0]!]
      : stateIds.getStates().slice(0, paletteSize);
    const section = sectionFromStates(states, (index) => index % paletteSize);
    const packed = packChunkSection(section, stateIds, resolveState);
    const unpacked = unpackChunkSection(packed, stateIds);

    expect(packed.paletteStateIds).toHaveLength(paletteSize);
    expect(packed.bitsPerBlock).toBe(bitsForLocalPalette(paletteSize));
    expect(packed.packedBlockIndices).toHaveLength(Math.ceil(BLOCKS_PER_SECTION / Math.floor(64 / packed.bitsPerBlock)));
    expect(unpacked).toEqual(section);
  });

  test("round-trips air-plus-one and no-air sections", () => {
    const { stateIds, resolveState } = createCodecContext();
    const air = stateIds.getStates().find((state) => state.isAir())!;
    const solidStates = nonAirStates(stateIds.getStates());
    const airAndStone = sectionFromStates([air, solidStates[0]!], (index) => index === 0 ? 1 : 0);
    const noAir = sectionFromStates(solidStates.slice(0, 4), (index) => index % 4, 2);

    expect(unpackChunkSection(packChunkSection(airAndStone, stateIds, resolveState), stateIds)).toEqual(airAndStone);
    expect(unpackChunkSection(packChunkSection(noAir, stateIds, resolveState), stateIds)).toEqual(noAir);
  });

  test("rejects invalid section palettes, indices, ids, and bit widths", () => {
    const { stateIds, resolveState } = createCodecContext();
    const air = stateIds.getStates().find((state) => state.isAir())!;
    const invalidSection = sectionFromStates([air], (index) => index === 0 ? 1 : 0);

    expect(() => packChunkSection({ ...invalidSection, palette: [] }, stateIds, resolveState)).toThrow(/empty palette/);
    expect(() => packChunkSection(invalidSection, stateIds, resolveState)).toThrow(/referenced palette entry 1/);

    const storage = new BitStorage(4, BLOCKS_PER_SECTION);
    storage.set(0, 1);
    const packed: PackedChunkSection = {
      y: 0,
      paletteStateIds: new Uint32Array([stateIds.idFor(air)]),
      bitsPerBlock: 4,
      packedBlockIndices: storage.getRaw(),
    };
    expect(() => unpackChunkSection(packed, stateIds)).toThrow(/referenced palette entry 1/);
    expect(() => unpackChunkSection({ ...packed, paletteStateIds: new Uint32Array([stateIds.size + 1]) }, stateIds)).toThrow(/out of bounds/);
    expect(() => unpackChunkSection({ ...packed, bitsPerBlock: 5 }, stateIds)).toThrow(/expected 4/);
  });
});

describe("packed chunk snapshot codecs", () => {
  test("round-trips a chunk snapshot and preserves hydration", () => {
    const { airState, stateIds, resolveState } = createCodecContext();
    const states = stateIds.getStates();
    const stone = states.find((state) => serializeBlockStateSnapshot(state).name === "minecraft:stone")!;
    const water = states.find((state) => serializeBlockStateSnapshot(state).name === "minecraft:water")!;
    const chunk = new LevelChunk(0, 0, airState);
    chunk.setBlockState(new BlockPos(1, 2, 3), stone);
    chunk.setBlockState(new BlockPos(4, 5, 6), water);
    chunk.recordBlockTick(new BlockPos(1, 2, 3), "minecraft:stone", 2);
    chunk.recordLiquidTick(new BlockPos(4, 5, 6), "minecraft:water", 3);

    const snapshot = buildChunkSnapshot(chunk, [1, 2, 3], 0, 16);
    const unpacked = unpackChunkSnapshot(packChunkSnapshot(snapshot, stateIds, resolveState), stateIds);
    const hydrated = hydrateChunkFromSnapshot(unpacked, airState, resolveState);

    expect(unpacked).toEqual(snapshot);
    expect(hydrated.getBlockState(new BlockPos(1, 2, 3))).toBe(stone);
    expect(hydrated.getBlockState(new BlockPos(4, 5, 6))).toBe(water);
    expect(hydrated.getScheduledBlockTicks()).toEqual(snapshot.blockTicks);
    expect(hydrated.getScheduledLiquidTicks()).toEqual(snapshot.liquidTicks);
  });

  test("packs and unpacks a committed terrain fixture without palette-index drift", () => {
    const { stateIds, resolveState } = createCodecContext();
    const snapshot = terrainFixtureSnapshot();
    const packed = packChunkSnapshot(snapshot, stateIds, resolveState);
    const unpacked = unpackChunkSnapshot(packed, stateIds);

    expect(packed.sections.length).toBe(snapshot.sections.length);
    for (const section of packed.sections) {
      expect(section.bitsPerBlock).toBe(bitsForLocalPalette(section.paletteStateIds.length));
    }
    expect(unpacked).toEqual(snapshot);
  });

  test("carries L0 vanilla light bytes through packed snapshots", () => {
    const { stateIds, resolveState } = createCodecContext();
    const light = committedFixtureLight();
    const snapshot: ChunkSnapshot = {
      ...terrainFixtureSnapshot(),
      light,
    };

    const packed = packChunkSnapshot(snapshot, stateIds, resolveState);
    const unpacked = unpackChunkSnapshot(packed, stateIds);

    expect(packed.light?.lightCorrect).toBe(true);
    expect(compareChunkLight(light, packed.light!)).toEqual([]);
    expect(compareChunkLight(light, unpacked.light!)).toEqual([]);
  });

  test("clones and collects transferables for packed light bytes", () => {
    const { stateIds, resolveState } = createCodecContext();
    const light = committedFixtureLight();
    const snapshot = packChunkSnapshot({ ...terrainFixtureSnapshot(), light }, stateIds, resolveState);
    const cloned = clonePackedChunkSnapshot(snapshot);

    const clonedSkyLight = cloned.light!.sky[0]!.data;
    clonedSkyLight[0] = clonedSkyLight[0]! ^ 0xFF;
    expect(cloned.light!.sky[0]!.data[0]).not.toBe(snapshot.light!.sky[0]!.data[0]);

    const transferables = collectPackedChunkSnapshotTransferables(snapshot);
    for (const section of [...snapshot.light!.sky, ...snapshot.light!.block]) {
      expect(transferables).toContain(section.data.buffer);
    }
  });
});
