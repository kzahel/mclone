import { BlockPos } from "../../../core/block-pos";
import type { PathfinderMob } from "../../entity/ai/pathfinder-mob";
import { BinaryHeap } from "./binary-heap";
import { Node } from "./node";
import type { NodeEvaluator } from "./node-evaluator";
import { Path } from "./path";
import type { PathNavigationRegion } from "./path-navigation-region";
import { Target } from "./target";

const FUDGING = 1.5;

export class PathFinder {
  private readonly neighbors: Node[] = new Array<Node>(32);
  private readonly openSet = new BinaryHeap();

  public constructor(
    private readonly nodeEvaluator: NodeEvaluator,
    private readonly maxVisitedNodes: number,
  ) {}

  public findPath(
    region: PathNavigationRegion,
    mob: PathfinderMob,
    targetPositions: ReadonlySet<BlockPos>,
    maxRange: number,
    accuracy: number,
    searchDepthMultiplier: number,
  ): Path | undefined {
    if (targetPositions.size === 0) {
      return undefined;
    }

    this.openSet.clear();
    this.nodeEvaluator.prepare(region, mob);
    const start = this.nodeEvaluator.getStart();
    const targets = new Map<Target, BlockPos>();
    for (const targetPosition of targetPositions) {
      targets.set(this.nodeEvaluator.getGoal(targetPosition.getX(), targetPosition.getY(), targetPosition.getZ()), targetPosition);
    }

    const path = this.findPathToTargets(start, targets, maxRange, accuracy, searchDepthMultiplier);
    this.nodeEvaluator.done();
    return path;
  }

  private findPathToTargets(
    start: Node,
    targetPositions: ReadonlyMap<Target, BlockPos>,
    maxRange: number,
    accuracy: number,
    searchDepthMultiplier: number,
  ): Path | undefined {
    const targets = new Set(targetPositions.keys());
    start.g = 0.0;
    start.h = this.getBestH(start, targets);
    start.f = start.h;
    this.openSet.clear();
    this.openSet.insert(start);
    let visitedNodes = 0;
    const reachedTargets = new Set<Target>();
    const maxVisitedNodes = Math.trunc(this.maxVisitedNodes * searchDepthMultiplier);

    while (!this.openSet.isEmpty()) {
      if (++visitedNodes >= maxVisitedNodes) {
        break;
      }

      const node = this.openSet.pop();
      node.closed = true;

      for (const target of targets) {
        if (node.distanceManhattan(target) <= accuracy) {
          target.setReached();
          reachedTargets.add(target);
        }
      }

      if (reachedTargets.size > 0) {
        break;
      }

      if (!(node.distanceTo(start) >= maxRange)) {
        const neighborCount = this.nodeEvaluator.getNeighbors(this.neighbors, node);

        for (let i = 0; i < neighborCount; i++) {
          const neighbor = this.neighbors[i]!;
          const distance = node.distanceTo(neighbor);
          neighbor.walkedDistance = node.walkedDistance + distance;
          const nextCost = node.g + distance + neighbor.costMalus;
          if (neighbor.walkedDistance < maxRange && (!neighbor.inOpenSet() || nextCost < neighbor.g)) {
            neighbor.cameFrom = node;
            neighbor.g = nextCost;
            neighbor.h = this.getBestH(neighbor, targets) * FUDGING;
            if (neighbor.inOpenSet()) {
              this.openSet.changeCost(neighbor, neighbor.g + neighbor.h);
            } else {
              neighbor.f = neighbor.g + neighbor.h;
              this.openSet.insert(neighbor);
            }
          }
        }
      }
    }

    const candidates = reachedTargets.size > 0
      ? [...reachedTargets]
        .map((target) => this.reconstructPath(target.getBestNode()!, targetPositions.get(target)!, true))
        .sort((left, right) => left.getNodeCount() - right.getNodeCount())
      : [...targets]
        .map((target) => this.reconstructPath(target.getBestNode()!, targetPositions.get(target)!, false))
        .sort((left, right) => {
          const dist = left.getDistToTarget() - right.getDistToTarget();
          return dist !== 0 ? dist : left.getNodeCount() - right.getNodeCount();
        });

    return candidates[0];
  }

  private getBestH(node: Node, targets: ReadonlySet<Target>): number {
    let best = Number.MAX_VALUE;

    for (const target of targets) {
      const heuristic = node.distanceTo(target);
      target.updateBest(heuristic, node);
      best = Math.min(heuristic, best);
    }

    return best;
  }

  private reconstructPath(point: Node, targetPos: BlockPos, reachesTarget: boolean): Path {
    const nodes: Node[] = [];
    let node: Node | undefined = point;
    nodes.unshift(point);

    while (node.cameFrom !== undefined) {
      node = node.cameFrom;
      nodes.unshift(node);
    }

    return new Path(nodes, targetPos, reachesTarget);
  }
}
