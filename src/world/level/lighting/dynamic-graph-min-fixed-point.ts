import { clamp } from "../../../util/mth";

const NO_COMPUTED_LEVEL = 255;

export abstract class DynamicGraphMinFixedPoint {
  private readonly queues: Array<Set<bigint>>;
  private readonly computedLevels = new Map<bigint, number>();
  private firstQueuedLevel: number;
  private workQueued = false;

  protected constructor(private readonly levelCount: number) {
    if (levelCount >= 254) {
      throw new Error("Level count must be < 254.");
    }

    this.queues = Array.from({ length: levelCount }, () => new Set<bigint>());
    this.firstQueuedLevel = levelCount;
  }

  private getKey(level: number, computedLevel: number): number {
    return Math.min(Math.min(level, computedLevel), this.levelCount - 1);
  }

  private getComputedLevelOrDefault(pos: bigint): number {
    return this.computedLevels.get(pos) ?? NO_COMPUTED_LEVEL;
  }

  private checkFirstQueuedLevel(nextLevel: number): void {
    const previousLevel = this.firstQueuedLevel;
    this.firstQueuedLevel = nextLevel;

    for (let level = previousLevel + 1; level < nextLevel; level++) {
      if (this.queues[level]!.size > 0) {
        this.firstQueuedLevel = level;
        break;
      }
    }
  }

  public removeFromQueue(pos: bigint): void {
    const computedLevel = this.getComputedLevelOrDefault(pos);
    if (computedLevel !== NO_COMPUTED_LEVEL) {
      const level = this.getLevel(pos);
      const queueKey = this.getKey(level, computedLevel);
      this.dequeue(pos, queueKey, this.levelCount, true);
      this.workQueued = this.firstQueuedLevel < this.levelCount;
    }
  }

  public removeIf(predicate: (pos: bigint) => boolean): void {
    const keysToRemove: bigint[] = [];
    for (const key of this.computedLevels.keys()) {
      if (predicate(key)) {
        keysToRemove.push(key);
      }
    }

    for (const key of keysToRemove) {
      this.removeFromQueue(key);
    }
  }

  private dequeue(pos: bigint, queueKey: number, nextLevel: number, removeComputedLevel: boolean): void {
    if (removeComputedLevel) {
      this.computedLevels.delete(pos);
    }

    this.queues[queueKey]!.delete(pos);
    if (this.queues[queueKey]!.size === 0 && this.firstQueuedLevel === queueKey) {
      this.checkFirstQueuedLevel(nextLevel);
    }
  }

  private enqueue(pos: bigint, computedLevel: number, queueKey: number): void {
    this.computedLevels.set(pos, computedLevel);
    this.queues[queueKey]!.add(pos);
    if (this.firstQueuedLevel > queueKey) {
      this.firstQueuedLevel = queueKey;
    }
  }

  protected checkNode(pos: bigint): void {
    this.checkEdge(pos, pos, this.levelCount - 1, false);
  }

  protected checkEdge(source: bigint, target: bigint, candidateLevel: number, decrease: boolean): void {
    this.checkEdgeWithLevels(
      source,
      target,
      candidateLevel,
      this.getLevel(target),
      this.getComputedLevelOrDefault(target),
      decrease,
    );
    this.workQueued = this.firstQueuedLevel < this.levelCount;
  }

  private checkEdgeWithLevels(
    source: bigint,
    target: bigint,
    candidateLevel: number,
    level: number,
    queuedLevel: number,
    decrease: boolean,
  ): void {
    if (this.isSource(target)) {
      return;
    }

    candidateLevel = clamp(candidateLevel, 0, this.levelCount - 1);
    level = clamp(level, 0, this.levelCount - 1);
    let firstQueue = false;
    if (queuedLevel === NO_COMPUTED_LEVEL) {
      firstQueue = true;
      queuedLevel = level;
    }

    const computedLevel = decrease
      ? Math.min(queuedLevel, candidateLevel)
      : clamp(this.getComputedLevel(target, source, candidateLevel), 0, this.levelCount - 1);
    const oldQueueKey = this.getKey(level, queuedLevel);
    if (level !== computedLevel) {
      const newQueueKey = this.getKey(level, computedLevel);
      if (oldQueueKey !== newQueueKey && !firstQueue) {
        this.dequeue(target, oldQueueKey, newQueueKey, false);
      }

      this.enqueue(target, computedLevel, newQueueKey);
    } else if (!firstQueue) {
      this.dequeue(target, oldQueueKey, this.levelCount, true);
    }
  }

  protected checkNeighbor(source: bigint, target: bigint, sourceLevel: number, decrease: boolean): void {
    const queuedLevel = this.getComputedLevelOrDefault(target);
    const computedFromNeighbor = clamp(this.computeLevelFromNeighbor(source, target, sourceLevel), 0, this.levelCount - 1);
    if (decrease) {
      this.checkEdgeWithLevels(source, target, computedFromNeighbor, this.getLevel(target), queuedLevel, true);
      return;
    }

    let level: number;
    let firstQueue: boolean;
    if (queuedLevel === NO_COMPUTED_LEVEL) {
      firstQueue = true;
      level = clamp(this.getLevel(target), 0, this.levelCount - 1);
    } else {
      level = queuedLevel;
      firstQueue = false;
    }

    if (computedFromNeighbor === level) {
      this.checkEdgeWithLevels(
        source,
        target,
        this.levelCount - 1,
        firstQueue ? level : this.getLevel(target),
        queuedLevel,
        false,
      );
    }
  }

  protected hasWork(): boolean {
    return this.workQueued;
  }

  protected runUpdatesForGraph(budget: number): number {
    if (this.firstQueuedLevel >= this.levelCount) {
      return budget;
    }

    while (this.firstQueuedLevel < this.levelCount && budget > 0) {
      budget--;
      const queue = this.queues[this.firstQueuedLevel]!;
      const pos = queue.values().next().value as bigint | undefined;
      if (pos === undefined) {
        this.checkFirstQueuedLevel(this.levelCount);
        continue;
      }

      queue.delete(pos);
      const level = clamp(this.getLevel(pos), 0, this.levelCount - 1);
      if (queue.size === 0) {
        this.checkFirstQueuedLevel(this.levelCount);
      }

      const computedLevel = this.getComputedLevelOrDefault(pos);
      this.computedLevels.delete(pos);
      if (computedLevel < level) {
        this.setLevel(pos, computedLevel);
        this.checkNeighborsAfterUpdate(pos, computedLevel, true);
      } else if (computedLevel > level) {
        this.enqueue(pos, computedLevel, this.getKey(this.levelCount - 1, computedLevel));
        this.setLevel(pos, this.levelCount - 1);
        this.checkNeighborsAfterUpdate(pos, level, false);
      }
    }

    this.workQueued = this.firstQueuedLevel < this.levelCount;
    return budget;
  }

  public getQueueSize(): number {
    return this.computedLevels.size;
  }

  protected abstract isSource(pos: bigint): boolean;

  protected abstract getComputedLevel(pos: bigint, source: bigint, candidateLevel: number): number;

  protected abstract checkNeighborsAfterUpdate(pos: bigint, level: number, decrease: boolean): void;

  protected abstract getLevel(pos: bigint): number;

  protected abstract setLevel(pos: bigint, level: number): void;

  protected abstract computeLevelFromNeighbor(source: bigint, target: bigint, sourceLevel: number): number;
}
