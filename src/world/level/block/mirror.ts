import { Direction } from "../../../core/direction";
import { Rotation } from "./rotation";

export enum Mirror {
  NONE = "none",
  LEFT_RIGHT = "left_right",
  FRONT_BACK = "front_back",
}

export function mirrorIndex(mirror: Mirror, value: number, maxValue: number): number {
  const half = Math.trunc(maxValue / 2);
  const wrapped = value > half ? value - maxValue : value;
  switch (mirror) {
    case Mirror.FRONT_BACK:
      return (maxValue - wrapped) % maxValue;
    case Mirror.LEFT_RIGHT:
      return ((half - wrapped) + maxValue) % maxValue;
    case Mirror.NONE:
    default:
      return value;
  }
}

export function getRotation(mirror: Mirror, direction: Direction): Rotation {
  const axis = direction.getAxis();
  return (mirror !== Mirror.LEFT_RIGHT || axis !== Direction.Axis.Z) && (mirror !== Mirror.FRONT_BACK || axis !== Direction.Axis.X)
    ? Rotation.NONE
    : Rotation.CLOCKWISE_180;
}

export function mirrorDirection(mirror: Mirror, direction: Direction): Direction {
  if (mirror === Mirror.FRONT_BACK && direction.getAxis() === Direction.Axis.X) {
    return direction.getOpposite();
  }

  if (mirror === Mirror.LEFT_RIGHT && direction.getAxis() === Direction.Axis.Z) {
    return direction.getOpposite();
  }

  return direction;
}
