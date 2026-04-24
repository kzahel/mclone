import { SectionPos } from "../../../core/section-pos";
import { DynamicGraphMinFixedPoint } from "./dynamic-graph-min-fixed-point";

export const LIGHT_SELF_SOURCE = 9223372036854775807n;

export abstract class SectionTracker extends DynamicGraphMinFixedPoint {
  protected constructor() {
    super(3);
  }

  protected override isSource(pos: bigint): boolean {
    return pos === LIGHT_SELF_SOURCE;
  }

  protected override checkNeighborsAfterUpdate(pos: bigint, level: number, decrease: boolean): void {
    for (let dx = -1; dx <= 1; dx++) {
      for (let dy = -1; dy <= 1; dy++) {
        for (let dz = -1; dz <= 1; dz++) {
          const neighbor = SectionPos.offset(pos, dx, dy, dz);
          if (neighbor !== pos) {
            this.checkNeighbor(pos, neighbor, level, decrease);
          }
        }
      }
    }
  }

  protected override getComputedLevel(pos: bigint, source: bigint, candidateLevel: number): number {
    let level = candidateLevel;

    for (let dx = -1; dx <= 1; dx++) {
      for (let dy = -1; dy <= 1; dy++) {
        for (let dz = -1; dz <= 1; dz++) {
          let neighbor = SectionPos.offset(pos, dx, dy, dz);
          if (neighbor === pos) {
            neighbor = LIGHT_SELF_SOURCE;
          }

          if (neighbor !== source) {
            const computedLevel = this.computeLevelFromNeighbor(neighbor, pos, this.getLevel(neighbor));
            if (level > computedLevel) {
              level = computedLevel;
            }

            if (level === 0) {
              return level;
            }
          }
        }
      }
    }

    return level;
  }

  protected override computeLevelFromNeighbor(source: bigint, target: bigint, sourceLevel: number): number {
    return source === LIGHT_SELF_SOURCE ? this.getLevelFromSource(target) : sourceLevel + 1;
  }

  protected abstract getLevelFromSource(pos: bigint): number;

  public update(pos: bigint, level: number, decrease: boolean): void {
    this.checkEdge(LIGHT_SELF_SOURCE, pos, level, decrease);
  }

  public runUpdates(budget: number): number {
    return this.runUpdatesForGraph(budget);
  }
}
