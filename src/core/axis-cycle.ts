import { Direction } from "./direction";

type DirectionAxis = (typeof Direction.Axis)["X"];

function axisIndex(axis: DirectionAxis): number {
  switch (axis) {
    case Direction.Axis.X:
      return 0;
    case Direction.Axis.Y:
      return 1;
    case Direction.Axis.Z:
      return 2;
    default:
      throw new Error(`Unknown axis ${axis}`);
  }
}

function floorMod(value: number, divisor: number): number {
  return ((value % divisor) + divisor) % divisor;
}

class AxisCycleValue {
  public constructor(
    private readonly cycleIntValue: (x: number, y: number, z: number, axis: DirectionAxis) => number,
    private readonly cycleAxisValue: (axis: DirectionAxis) => DirectionAxis,
    private inverseValue?: AxisCycleValue,
  ) {}

  public cycle(x: number, y: number, z: number, axis: DirectionAxis): number {
    return this.cycleIntValue(x, y, z, axis);
  }

  public cycleAxis(axis: DirectionAxis): DirectionAxis {
    return this.cycleAxisValue(axis);
  }

  public inverse(): AxisCycleValue {
    return this.inverseValue ?? this;
  }

  public setInverse(value: AxisCycleValue): void {
    this.inverseValue = value;
  }
}

const AXIS_VALUES = Direction.Axis.values();

const AXIS_CYCLE_NONE = new AxisCycleValue(
  (x, y, z, axis) => axis.choose(x, y, z),
  (axis) => axis,
);
const AXIS_CYCLE_FORWARD = new AxisCycleValue(
  (x, y, z, axis) => axis.choose(z, x, y),
  (axis) => AXIS_VALUES[floorMod(axisIndex(axis) + 1, 3)]!,
);
const AXIS_CYCLE_BACKWARD = new AxisCycleValue(
  (x, y, z, axis) => axis.choose(y, z, x),
  (axis) => AXIS_VALUES[floorMod(axisIndex(axis) - 1, 3)]!,
);

AXIS_CYCLE_NONE.setInverse(AXIS_CYCLE_NONE);
AXIS_CYCLE_FORWARD.setInverse(AXIS_CYCLE_BACKWARD);
AXIS_CYCLE_BACKWARD.setInverse(AXIS_CYCLE_FORWARD);

export class AxisCycle {
  public static readonly NONE = AXIS_CYCLE_NONE;
  public static readonly FORWARD = AXIS_CYCLE_FORWARD;
  public static readonly BACKWARD = AXIS_CYCLE_BACKWARD;

  private static readonly VALUES = [AxisCycle.NONE, AxisCycle.FORWARD, AxisCycle.BACKWARD] as const;

  public static between(from: DirectionAxis, to: DirectionAxis): AxisCycleValue {
    return AxisCycle.VALUES[floorMod(axisIndex(to) - axisIndex(from), 3)]!;
  }
}
