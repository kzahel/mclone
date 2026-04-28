import { Node } from "./node";

export class BinaryHeap {
  private heap: Array<Node | undefined> = new Array<Node | undefined>(128);
  private heapSize = 0;

  public insert(point: Node): Node {
    if (point.heapIdx >= 0) {
      throw new Error("OW KNOWS!");
    }

    if (this.heapSize === this.heap.length) {
      this.heap.length = this.heapSize << 1;
    }

    this.heap[this.heapSize] = point;
    point.heapIdx = this.heapSize;
    this.upHeap(this.heapSize++);
    return point;
  }

  public clear(): void {
    this.heapSize = 0;
  }

  public peek(): Node {
    return this.heap[0]!;
  }

  public pop(): Node {
    const node = this.heap[0]!;
    this.heap[0] = this.heap[--this.heapSize];
    this.heap[this.heapSize] = undefined;
    if (this.heapSize > 0) {
      this.downHeap(0);
    }

    node.heapIdx = -1;
    return node;
  }

  public remove(node: Node): void {
    const index = node.heapIdx;
    this.heap[index] = this.heap[--this.heapSize];
    this.heap[this.heapSize] = undefined;
    if (this.heapSize > index) {
      const moved = this.heap[index]!;
      if (moved.f < node.f) {
        this.upHeap(index);
      } else {
        this.downHeap(index);
      }
    }

    node.heapIdx = -1;
  }

  public changeCost(point: Node, cost: number): void {
    const oldCost = point.f;
    point.f = cost;
    if (cost < oldCost) {
      this.upHeap(point.heapIdx);
    } else {
      this.downHeap(point.heapIdx);
    }
  }

  public size(): number {
    return this.heapSize;
  }

  private upHeap(index: number): void {
    const node = this.heap[index]!;
    const cost = node.f;

    while (index > 0) {
      const parentIndex = (index - 1) >> 1;
      const parent = this.heap[parentIndex]!;
      if (!(cost < parent.f)) {
        break;
      }

      this.heap[index] = parent;
      parent.heapIdx = index;
      index = parentIndex;
    }

    this.heap[index] = node;
    node.heapIdx = index;
  }

  private downHeap(index: number): void {
    const node = this.heap[index]!;
    const cost = node.f;

    while (true) {
      const leftIndex = 1 + (index << 1);
      const rightIndex = leftIndex + 1;
      if (leftIndex >= this.heapSize) {
        break;
      }

      const left = this.heap[leftIndex]!;
      const leftCost = left.f;
      let right: Node | undefined;
      let rightCost: number;
      if (rightIndex >= this.heapSize) {
        right = undefined;
        rightCost = Number.POSITIVE_INFINITY;
      } else {
        right = this.heap[rightIndex]!;
        rightCost = right.f;
      }

      if (leftCost < rightCost) {
        if (!(leftCost < cost)) {
          break;
        }

        this.heap[index] = left;
        left.heapIdx = index;
        index = leftIndex;
      } else {
        if (!(rightCost < cost)) {
          break;
        }

        this.heap[index] = right;
        right!.heapIdx = index;
        index = rightIndex;
      }
    }

    this.heap[index] = node;
    node.heapIdx = index;
  }

  public isEmpty(): boolean {
    return this.heapSize === 0;
  }

  public getHeap(): readonly Node[] {
    return this.heap.slice(0, this.heapSize) as Node[];
  }
}
