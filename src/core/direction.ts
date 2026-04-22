import { type StringRepresentable, type StringRepresentableClass } from "./string-representable";
import { Vec3i } from "./vec3i";

class DirectionAxisValue implements StringRepresentable {
  public constructor(private readonly name: string) {}

  public getName(): string {
    return this.name;
  }

  public isVertical(): boolean {
    return this.name === "y";
  }

  public isHorizontal(): boolean {
    return this.name === "x" || this.name === "z";
  }

  public getPlane(): DirectionPlaneValue {
    switch (this) {
      case Direction.Axis.X:
      case Direction.Axis.Z:
        return Direction.Plane.HORIZONTAL;
      case Direction.Axis.Y:
        return Direction.Plane.VERTICAL;
      default:
        throw new Error("Someone's been tampering with the universe!");
    }
  }

  public getSerializedName(): string {
    return this.name;
  }

  public toString(): string {
    return this.name;
  }
}

class DirectionAxisDirectionValue {
  public constructor(
    private readonly step: number,
    private readonly name: string,
  ) {}

  public getStep(): number {
    return this.step;
  }

  public getName(): string {
    return this.name;
  }

  public opposite(): DirectionAxisDirectionValue {
    return this.step > 0 ? AXIS_DIRECTION_NEGATIVE : AXIS_DIRECTION_POSITIVE;
  }

  public toString(): string {
    return this.name;
  }
}

class DirectionPlaneValue implements Iterable<Direction> {
  public constructor(
    private readonly faces: readonly Direction[],
    private readonly axes: readonly DirectionAxisValue[],
  ) {}

  public test(direction: Direction | undefined): boolean {
    return direction !== undefined && direction.getAxis().getPlane() === this;
  }

  public getAxes(): readonly DirectionAxisValue[] {
    return this.axes;
  }

  public [Symbol.iterator](): Iterator<Direction> {
    return this.faces[Symbol.iterator]();
  }

  public stream(): readonly Direction[] {
    return this.faces;
  }
}

const AXIS_X = new DirectionAxisValue("x");
const AXIS_Y = new DirectionAxisValue("y");
const AXIS_Z = new DirectionAxisValue("z");
const AXIS_VALUES = [AXIS_X, AXIS_Y, AXIS_Z] as const;
const AXIS_BY_NAME = new Map(AXIS_VALUES.map((axis) => [axis.getName(), axis] as const));

const AXIS_DIRECTION_POSITIVE = new DirectionAxisDirectionValue(1, "Towards positive");
const AXIS_DIRECTION_NEGATIVE = new DirectionAxisDirectionValue(-1, "Towards negative");

export class Direction implements StringRepresentable {
  public static readonly Axis = {
    X: AXIS_X,
    Y: AXIS_Y,
    Z: AXIS_Z,
    values(): readonly DirectionAxisValue[] {
      return AXIS_VALUES;
    },
    byName(name: string): DirectionAxisValue | undefined {
      return AXIS_BY_NAME.get(name.toLowerCase());
    },
  } satisfies StringRepresentableClass<DirectionAxisValue> & {
    X: DirectionAxisValue;
    Y: DirectionAxisValue;
    Z: DirectionAxisValue;
    byName(name: string): DirectionAxisValue | undefined;
  };

  public static readonly AxisDirection = {
    POSITIVE: AXIS_DIRECTION_POSITIVE,
    NEGATIVE: AXIS_DIRECTION_NEGATIVE,
  };

  public static readonly DOWN = new Direction(0, 1, -1, "down", Direction.AxisDirection.NEGATIVE, Direction.Axis.Y, new Vec3i(0, -1, 0));
  public static readonly UP = new Direction(1, 0, -1, "up", Direction.AxisDirection.POSITIVE, Direction.Axis.Y, new Vec3i(0, 1, 0));
  public static readonly NORTH = new Direction(2, 3, 2, "north", Direction.AxisDirection.NEGATIVE, Direction.Axis.Z, new Vec3i(0, 0, -1));
  public static readonly SOUTH = new Direction(3, 2, 0, "south", Direction.AxisDirection.POSITIVE, Direction.Axis.Z, new Vec3i(0, 0, 1));
  public static readonly WEST = new Direction(4, 5, 1, "west", Direction.AxisDirection.NEGATIVE, Direction.Axis.X, new Vec3i(-1, 0, 0));
  public static readonly EAST = new Direction(5, 4, 3, "east", Direction.AxisDirection.POSITIVE, Direction.Axis.X, new Vec3i(1, 0, 0));

  private static readonly VALUES = [Direction.DOWN, Direction.UP, Direction.NORTH, Direction.SOUTH, Direction.WEST, Direction.EAST] as const;
  private static readonly BY_NAME = new Map(Direction.VALUES.map((direction) => [direction.getName(), direction] as const));
  private static readonly BY_3D_DATA = [...Direction.VALUES].sort((left, right) => left.data3d - right.data3d);
  private static readonly BY_2D_DATA = Direction.VALUES.filter((direction) => direction.getAxis().isHorizontal()).sort(
    (left, right) => left.data2d - right.data2d,
  );

  public static readonly Plane = {
    HORIZONTAL: new DirectionPlaneValue([Direction.NORTH, Direction.EAST, Direction.SOUTH, Direction.WEST], [Direction.Axis.X, Direction.Axis.Z]),
    VERTICAL: new DirectionPlaneValue([Direction.UP, Direction.DOWN], [Direction.Axis.Y]),
  };

  private constructor(
    private readonly data3d: number,
    private readonly oppositeIndex: number,
    private readonly data2d: number,
    private readonly name: string,
    private readonly axisDirection: DirectionAxisDirectionValue,
    private readonly axis: DirectionAxisValue,
    private readonly normal: Vec3i,
  ) {}

  public static values(): readonly Direction[] {
    return Direction.VALUES;
  }

  public static byName(name: string): Direction | undefined {
    return Direction.BY_NAME.get(name);
  }

  public static from3DDataValue(value: number): Direction {
    return Direction.BY_3D_DATA[Math.abs(value % Direction.BY_3D_DATA.length)]!;
  }

  public static from2DDataValue(value: number): Direction {
    return Direction.BY_2D_DATA[Math.abs(value % Direction.BY_2D_DATA.length)]!;
  }

  public static fromAxisAndDirection(axis: DirectionAxisValue, axisDirection: DirectionAxisDirectionValue): Direction {
    switch (axis) {
      case Direction.Axis.X:
        return axisDirection === Direction.AxisDirection.POSITIVE ? Direction.EAST : Direction.WEST;
      case Direction.Axis.Y:
        return axisDirection === Direction.AxisDirection.POSITIVE ? Direction.UP : Direction.DOWN;
      case Direction.Axis.Z:
      default:
        return axisDirection === Direction.AxisDirection.POSITIVE ? Direction.SOUTH : Direction.NORTH;
    }
  }

  public static get(axisDirection: DirectionAxisDirectionValue, axis: DirectionAxisValue): Direction {
    for (const direction of Direction.VALUES) {
      if (direction.getAxisDirection() === axisDirection && direction.getAxis() === axis) {
        return direction;
      }
    }

    throw new Error(`No such direction: ${axisDirection} ${axis}`);
  }

  public get3DDataValue(): number {
    return this.data3d;
  }

  public get2DDataValue(): number {
    return this.data2d;
  }

  public getAxisDirection(): DirectionAxisDirectionValue {
    return this.axisDirection;
  }

  public getAxis(): DirectionAxisValue {
    return this.axis;
  }

  public getName(): string {
    return this.name;
  }

  public getOpposite(): Direction {
    return Direction.from3DDataValue(this.oppositeIndex);
  }

  public getClockWise(): Direction {
    switch (this) {
      case Direction.NORTH:
        return Direction.EAST;
      case Direction.SOUTH:
        return Direction.WEST;
      case Direction.WEST:
        return Direction.NORTH;
      case Direction.EAST:
        return Direction.SOUTH;
      default:
        throw new Error(`Unable to get Y-rotated facing of ${this}`);
    }
  }

  public getCounterClockWise(): Direction {
    switch (this) {
      case Direction.NORTH:
        return Direction.WEST;
      case Direction.SOUTH:
        return Direction.EAST;
      case Direction.WEST:
        return Direction.SOUTH;
      case Direction.EAST:
        return Direction.NORTH;
      default:
        throw new Error(`Unable to get CCW facing of ${this}`);
    }
  }

  public getNormal(): Vec3i {
    return this.normal;
  }

  public getSerializedName(): string {
    return this.name;
  }

  public toString(): string {
    return this.name;
  }
}

export namespace Direction {
  export type Axis = DirectionAxisValue;
  export type AxisDirection = DirectionAxisDirectionValue;
  export type Plane = DirectionPlaneValue;
}
