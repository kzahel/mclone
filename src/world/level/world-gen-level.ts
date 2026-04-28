import { BlockPos } from "../../core/block-pos";
import { LightLayer } from "./light-layer";
import { Heightmap } from "../../worldgen/levelgen/heightmap";
import type { Biome } from "../../worldgen/biome/biome";
import type { Block } from "./block/block";
import type { BlockGetter } from "./block-getter";
import type { BlockState } from "./block/state/block-state";
import type { Fluid } from "./material/fluid";
import type { LevelSimulatedReader } from "./level-simulated-reader";
import type { TickAccess } from "./tick-access";
import { GenerationStep } from "../../worldgen/levelgen/generation-step";
import type { StructureFeatureManager } from "./structure-feature-manager";

export interface WorldGenLevel extends BlockGetter, LevelSimulatedReader {
  setBlock(pos: BlockPos, state: BlockState, flags?: number): boolean;

  isEmptyBlock(pos: BlockPos): boolean;

  getHeight(type: Heightmap.Types, x: number, z: number): number;

  getMinBuildHeight(): number;

  getMaxBuildHeight(): number;

  getBiome(pos: BlockPos): Biome;

  getBrightness(layer: LightLayer, pos: BlockPos): number;

  getBlockTicks(): TickAccess<Block>;

  getLiquidTicks(): TickAccess<Fluid>;

  getStructureFeatureManager?(): StructureFeatureManager;

  getCarvingMask?(step: GenerationStep.Carving, chunkX: number, chunkZ: number): Uint8Array | undefined;
}
