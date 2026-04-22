import { BlockPos } from "../../core/block-pos";
import { Direction } from "../../core/direction";
import { VisibilitySet } from "./visibility-set";

const SIZE = 4096;
const MASK = 15;
const DX = 1;
const DZ = 16;
const DY = 256;
const DIRECTIONS = Direction.values();

function createIndexOfEdges(): number[] {
  const result = new Array<number>(1352);
  let next = 0;
  for (let x = 0; x < 16; x++) {
    for (let y = 0; y < 16; y++) {
      for (let z = 0; z < 16; z++) {
        if (x === 0 || x === 15 || y === 0 || y === 15 || z === 0 || z === 15) {
          result[next++] = getIndex(x, y, z);
        }
      }
    }
  }

  return result;
}

function getIndex(x: number, y: number, z: number): number {
  return (x << 0) | (y << 8) | (z << 4);
}

export class VisGraph {
  private static readonly INDEX_OF_EDGES = createIndexOfEdges();
  private readonly bitSet = new Uint8Array(SIZE);
  private empty = SIZE;

  public setOpaque(pos: BlockPos): void {
    const index = getIndex(pos.getX() & MASK, pos.getY() & MASK, pos.getZ() & MASK);
    if (this.bitSet[index] === 0) {
      this.bitSet[index] = 1;
      this.empty--;
    }
  }

  public resolve(): VisibilitySet {
    const result = new VisibilitySet();
    if ((SIZE - this.empty) < 256) {
      result.setAll(true);
    } else if (this.empty === 0) {
      result.setAll(false);
    } else {
      for (const index of VisGraph.INDEX_OF_EDGES) {
        if (this.bitSet[index] === 0) {
          result.add(this.floodFill(index));
        }
      }
    }

    return result;
  }

  private floodFill(start: number): Set<Direction> {
    const result = new Set<Direction>();
    const queue: number[] = [start];
    let readIndex = 0;
    this.bitSet[start] = 1;

    while (readIndex < queue.length) {
      const current = queue[readIndex++]!;
      this.addEdges(current, result);
      for (const direction of DIRECTIONS) {
        const neighborIndex = this.getNeighborIndexAtFace(current, direction);
        if (neighborIndex >= 0 && this.bitSet[neighborIndex] === 0) {
          this.bitSet[neighborIndex] = 1;
          queue.push(neighborIndex);
        }
      }
    }

    return result;
  }

  private addEdges(index: number, edges: Set<Direction>): void {
    const x = (index >> 0) & MASK;
    if (x === 0) {
      edges.add(Direction.WEST);
    } else if (x === MASK) {
      edges.add(Direction.EAST);
    }

    const y = (index >> 8) & MASK;
    if (y === 0) {
      edges.add(Direction.DOWN);
    } else if (y === MASK) {
      edges.add(Direction.UP);
    }

    const z = (index >> 4) & MASK;
    if (z === 0) {
      edges.add(Direction.NORTH);
    } else if (z === MASK) {
      edges.add(Direction.SOUTH);
    }
  }

  private getNeighborIndexAtFace(index: number, direction: Direction): number {
    switch (direction) {
      case Direction.DOWN:
        return ((index >> 8) & MASK) === 0 ? -1 : index - DY;
      case Direction.UP:
        return ((index >> 8) & MASK) === MASK ? -1 : index + DY;
      case Direction.NORTH:
        return ((index >> 4) & MASK) === 0 ? -1 : index - DZ;
      case Direction.SOUTH:
        return ((index >> 4) & MASK) === MASK ? -1 : index + DZ;
      case Direction.WEST:
        return ((index >> 0) & MASK) === 0 ? -1 : index - DX;
      case Direction.EAST:
        return ((index >> 0) & MASK) === MASK ? -1 : index + DX;
      default:
        return -1;
    }
  }
}
