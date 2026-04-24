import { BlockPos } from "../../core/block-pos";
import type { TickAccess } from "./tick-access";

export enum TickPriority {
  EXTREMELY_HIGH = -3,
  VERY_HIGH = -2,
  HIGH = -1,
  NORMAL = 0,
  LOW = 1,
  VERY_LOW = 2,
  EXTREMELY_LOW = 3,
}

export class TickNextTickData<T extends object> {
  private static counter = 0;
  private readonly order: number;

  public constructor(
    public readonly pos: BlockPos,
    private readonly type: T,
    public readonly triggerTick = 0,
    public readonly priority = TickPriority.NORMAL,
  ) {
    this.order = TickNextTickData.counter++;
  }

  public getType(): T {
    return this.type;
  }

  public identityKey(typeIds: WeakMap<T, number>, allocateTypeId: () => number): string {
    let typeId = typeIds.get(this.type);
    if (typeId === undefined) {
      typeId = allocateTypeId();
      typeIds.set(this.type, typeId);
    }
    return `${this.pos.asLong()}:${typeId}`;
  }

  public compareTo(other: TickNextTickData<T>): number {
    return (this.triggerTick - other.triggerTick)
      || (this.priority - other.priority)
      || (this.order - other.order);
  }
}

export class ServerTickList<T extends object> implements TickAccess<T> {
  public static readonly MAX_TICK_BLOCKS_PER_TICK = 65536;

  private readonly typeIds = new WeakMap<T, number>();
  private nextTypeId = 1;
  private readonly tickNextTickSet = new Map<string, TickNextTickData<T>>();
  private readonly tickNextTickList: TickNextTickData<T>[] = [];
  private readonly currentlyTicking: TickNextTickData<T>[] = [];
  private readonly alreadyTicked: TickNextTickData<T>[] = [];

  public constructor(
    private readonly ignore: (target: T) => boolean,
    private readonly getGameTime: () => number,
    private readonly ticker: (tick: TickNextTickData<T>) => void,
    private readonly isPositionTicking: (pos: BlockPos) => boolean = () => true,
  ) {}

  public tick(): void {
    this.tickNextTickList.sort((left, right) => left.compareTo(right));
    let limit = Math.min(this.tickNextTickList.length, ServerTickList.MAX_TICK_BLOCKS_PER_TICK);

    for (let index = 0; index < this.tickNextTickList.length && limit > 0;) {
      const tick = this.tickNextTickList[index]!;
      if (tick.triggerTick > this.getGameTime()) {
        break;
      }

      if (this.isPositionTicking(tick.pos)) {
        this.tickNextTickList.splice(index, 1);
        this.tickNextTickSet.delete(this.identityKey(tick));
        this.currentlyTicking.push(tick);
        limit--;
      } else {
        index++;
      }
    }

    let tick: TickNextTickData<T> | undefined;
    while ((tick = this.currentlyTicking.shift()) !== undefined) {
      if (this.isPositionTicking(tick.pos)) {
        this.alreadyTicked.push(tick);
        this.ticker(tick);
      } else {
        this.scheduleTick(tick.pos, tick.getType(), 0);
      }
    }

    this.alreadyTicked.length = 0;
    this.currentlyTicking.length = 0;
  }

  public scheduleTick(pos: BlockPos, target: T, delay: number, priority = TickPriority.NORMAL): void {
    if (!this.ignore(target)) {
      this.addTickData(new TickNextTickData(pos, target, this.getGameTime() + delay, priority));
    }
  }

  private addTickData(tick: TickNextTickData<T>): void {
    const key = this.identityKey(tick);
    if (!this.tickNextTickSet.has(key)) {
      this.tickNextTickSet.set(key, tick);
      this.tickNextTickList.push(tick);
    }
  }

  public hasScheduledTick(pos: BlockPos, target: T): boolean {
    return this.tickNextTickSet.has(this.identityKey(new TickNextTickData(pos, target)));
  }

  public size(): number {
    return this.tickNextTickSet.size;
  }

  public getPendingTicks(): readonly TickNextTickData<T>[] {
    return [...this.tickNextTickList].sort((left, right) => left.compareTo(right));
  }

  private identityKey(tick: TickNextTickData<T>): string {
    return tick.identityKey(this.typeIds, () => this.nextTypeId++);
  }
}
