import { BlockPos } from "../../core/block-pos";
import { Direction } from "../../core/direction";
import type { LevelSimulatedReader } from "../../world/level/level-simulated-reader";
import type { BlockState } from "../../world/level/block/state/block-state";

export abstract class Column {
  public static around(floor: number, ceiling: number): Column.Range {
    return new Column.Range(floor - 1, ceiling + 1);
  }

  public static inside(floor: number, ceiling: number): Column.Range {
    return new Column.Range(floor, ceiling);
  }

  public static below(ceiling: number): Column.Ray {
    return new Column.Ray(ceiling, false);
  }

  public static fromHighest(floor: number): Column.Ray {
    return new Column.Ray(floor + 1, false);
  }

  public static above(floor: number): Column.Ray {
    return new Column.Ray(floor, true);
  }

  public static fromLowest(ceiling: number): Column.Ray {
    return new Column.Ray(ceiling - 1, true);
  }

  public static line(): Column.Line {
    return Column.Line.INSTANCE;
  }

  public static create(floor: number | undefined, ceiling: number | undefined): Column {
    if (floor !== undefined && ceiling !== undefined) {
      return Column.inside(floor, ceiling);
    }

    if (floor !== undefined) {
      return Column.above(floor);
    }

    return ceiling !== undefined ? Column.below(ceiling) : Column.line();
  }

  public abstract getCeiling(): number | undefined;

  public abstract getFloor(): number | undefined;

  public abstract getHeight(): number | undefined;

  public withFloor(floor: number | undefined): Column {
    return Column.create(floor, this.getCeiling());
  }

  public withCeiling(ceiling: number | undefined): Column {
    return Column.create(this.getFloor(), ceiling);
  }

  public static scan(
    level: LevelSimulatedReader,
    pos: BlockPos,
    range: number,
    predicate: (state: BlockState) => boolean,
    edgePredicate: (state: BlockState) => boolean,
  ): Column | undefined {
    const mutable = pos.mutable();
    if (!level.isStateAtPosition(pos, predicate)) {
      return undefined;
    }

    const y = pos.getY();
    const ceiling = Column.scanDirection(level, range, predicate, edgePredicate, mutable, y, Direction.UP);
    const floor = Column.scanDirection(level, range, predicate, edgePredicate, mutable, y, Direction.DOWN);
    return Column.create(floor, ceiling);
  }

  private static scanDirection(
    level: LevelSimulatedReader,
    range: number,
    predicate: (state: BlockState) => boolean,
    edgePredicate: (state: BlockState) => boolean,
    mutable: BlockPos.MutableBlockPos,
    startY: number,
    direction: Direction,
  ): number | undefined {
    mutable.set(mutable.getX(), startY, mutable.getZ());

    for (let index = 1; index < range && level.isStateAtPosition(mutable, predicate); index++) {
      mutable.move(direction);
    }

    return level.isStateAtPosition(mutable, edgePredicate) ? mutable.getY() : undefined;
  }
}

export namespace Column {
  export class Line extends Column {
    public static readonly INSTANCE = new Line();

    private constructor() {
      super();
    }

    public override getCeiling(): number | undefined {
      return undefined;
    }

    public override getFloor(): number | undefined {
      return undefined;
    }

    public override getHeight(): number | undefined {
      return undefined;
    }
  }

  export class Range extends Column {
    public constructor(
      private readonly floor: number,
      private readonly ceiling: number,
    ) {
      super();
      if (this.height() < 0) {
        throw new Error(`Column of negative height: ${this}`);
      }
    }

    public override getCeiling(): number {
      return this.ceiling;
    }

    public override getFloor(): number {
      return this.floor;
    }

    public override getHeight(): number {
      return this.height();
    }

    public height(): number {
      return this.ceiling - this.floor - 1;
    }

    public override toString(): string {
      return `C(${this.ceiling}-${this.floor})`;
    }
  }

  export class Ray extends Column {
    public constructor(
      private readonly edge: number,
      private readonly pointingUp: boolean,
    ) {
      super();
    }

    public override getCeiling(): number | undefined {
      return this.pointingUp ? undefined : this.edge;
    }

    public override getFloor(): number | undefined {
      return this.pointingUp ? this.edge : undefined;
    }

    public override getHeight(): number | undefined {
      return undefined;
    }

    public override toString(): string {
      return this.pointingUp ? `C(${this.edge}-)` : `C(-${this.edge})`;
    }
  }
}
