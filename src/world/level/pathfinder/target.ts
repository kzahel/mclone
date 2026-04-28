import { Node } from "./node";

export class Target extends Node {
  private bestHeuristic = Number.MAX_VALUE;
  private bestNode: Node | undefined;
  private reached = false;

  public constructor(node: Node);
  public constructor(x: number, y: number, z: number);
  public constructor(first: Node | number, y?: number, z?: number) {
    if (first instanceof Node) {
      super(first.x, first.y, first.z);
      return;
    }

    super(first, y!, z!);
  }

  public updateBest(heuristic: number, node: Node): void {
    if (heuristic < this.bestHeuristic) {
      this.bestHeuristic = heuristic;
      this.bestNode = node;
    }
  }

  public getBestNode(): Node | undefined {
    return this.bestNode;
  }

  public setReached(): void {
    this.reached = true;
  }

  public isReached(): boolean {
    return this.reached;
  }
}
