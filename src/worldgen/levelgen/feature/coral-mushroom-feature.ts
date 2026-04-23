import { BlockPos } from "../../../core/block-pos";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { CoralFeature } from "./coral-feature";

export class CoralMushroomFeature extends CoralFeature {
  protected override placeFeature(level: WorldGenLevel, random: SimpleRandomSource, pos: BlockPos, state: BlockState): boolean {
    const width = random.nextInt(3) + 3;
    const height = random.nextInt(3) + 3;
    const depth = random.nextInt(3) + 3;
    const downOffset = random.nextInt(3) + 1;
    const mutablePos = pos.mutable();

    for (let x = 0; x <= height; x++) {
      for (let y = 0; y <= width; y++) {
        for (let z = 0; z <= depth; z++) {
          mutablePos.set(x + pos.getX(), y + pos.getY(), z + pos.getZ());
          mutablePos.move(0, -downOffset, 0);
          if (
            ((x !== 0 && x !== height) || (y !== 0 && y !== width)) &&
            ((z !== 0 && z !== depth) || (y !== 0 && y !== width)) &&
            ((x !== 0 && x !== height) || (z !== 0 && z !== depth)) &&
            (x === 0 || x === height || y === 0 || y === width || z === 0 || z === depth) &&
            !(random.nextFloat() < 0.1)
          ) {
            this.placeCoralBlock(level, random, mutablePos, state);
          }
        }
      }
    }

    return true;
  }
}
