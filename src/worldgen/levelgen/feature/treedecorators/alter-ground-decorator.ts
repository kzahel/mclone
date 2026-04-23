import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { LevelSimulatedReader } from "../../../../world/level/level-simulated-reader";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { Feature } from "../feature";
import type { BlockStateProvider } from "../stateproviders/block-state-provider";
import { TreeDecorator } from "./tree-decorator";

export class AlterGroundDecorator extends TreeDecorator {
  public constructor(private readonly provider: BlockStateProvider) {
    super();
  }

  public override place(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    trunkPositions: readonly BlockPos[],
    _leafPositions: readonly BlockPos[],
  ): void {
    if (trunkPositions.length === 0) {
      return;
    }

    const baseY = trunkPositions[0]!.getY();
    for (const trunkPos of trunkPositions) {
      if (trunkPos.getY() !== baseY) {
        continue;
      }

      this.placeCircle(level, consumer, random, trunkPos.west().north());
      this.placeCircle(level, consumer, random, trunkPos.east().east().north());
      this.placeCircle(level, consumer, random, trunkPos.west().south().south());
      this.placeCircle(level, consumer, random, trunkPos.east().east().south().south());

      for (let attempt = 0; attempt < 5; attempt++) {
        const offset = random.nextInt(64);
        const offsetX = offset % 8;
        const offsetZ = Math.floor(offset / 8);
        if (offsetX === 0 || offsetX === 7 || offsetZ === 0 || offsetZ === 7) {
          this.placeCircle(level, consumer, random, trunkPos.offset(-3 + offsetX, 0, -3 + offsetZ));
        }
      }
    }
  }

  private placeCircle(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    pos: BlockPos,
  ): void {
    for (let dx = -2; dx <= 2; dx++) {
      for (let dz = -2; dz <= 2; dz++) {
        if (Math.abs(dx) !== 2 || Math.abs(dz) !== 2) {
          this.placeBlockAt(level, consumer, random, pos.offset(dx, 0, dz));
        }
      }
    }
  }

  private placeBlockAt(
    level: LevelSimulatedReader,
    consumer: (pos: BlockPos, state: BlockState) => void,
    random: SimpleRandomSource,
    pos: BlockPos,
  ): void {
    for (let y = 2; y >= -3; y--) {
      const target = pos.above(y);
      if (Feature.isGrassOrDirt(level, target)) {
        consumer(target, this.provider.getState(random, pos));
        return;
      }

      if (!Feature.isAir(level, target) && y < 0) {
        return;
      }
    }
  }
}
