import { Direction } from "../../../core/direction";

export enum Rotation {
  NONE = "none",
  CLOCKWISE_90 = "clockwise_90",
  CLOCKWISE_180 = "clockwise_180",
  COUNTERCLOCKWISE_90 = "counterclockwise_90",
}

export function getRotated(rotation: Rotation, other: Rotation): Rotation {
  switch (other) {
    case Rotation.CLOCKWISE_180:
      switch (rotation) {
        case Rotation.NONE:
          return Rotation.CLOCKWISE_180;
        case Rotation.CLOCKWISE_90:
          return Rotation.COUNTERCLOCKWISE_90;
        case Rotation.CLOCKWISE_180:
          return Rotation.NONE;
        case Rotation.COUNTERCLOCKWISE_90:
          return Rotation.CLOCKWISE_90;
      }
    case Rotation.COUNTERCLOCKWISE_90:
      switch (rotation) {
        case Rotation.NONE:
          return Rotation.COUNTERCLOCKWISE_90;
        case Rotation.CLOCKWISE_90:
          return Rotation.NONE;
        case Rotation.CLOCKWISE_180:
          return Rotation.CLOCKWISE_90;
        case Rotation.COUNTERCLOCKWISE_90:
          return Rotation.CLOCKWISE_180;
      }
    case Rotation.CLOCKWISE_90:
      switch (rotation) {
        case Rotation.NONE:
          return Rotation.CLOCKWISE_90;
        case Rotation.CLOCKWISE_90:
          return Rotation.CLOCKWISE_180;
        case Rotation.CLOCKWISE_180:
          return Rotation.COUNTERCLOCKWISE_90;
        case Rotation.COUNTERCLOCKWISE_90:
          return Rotation.NONE;
      }
    default:
      return rotation;
  }
}

export function rotateDirection(rotation: Rotation, direction: Direction): Direction {
  if (direction.getAxis() === Direction.Axis.Y) {
    return direction;
  }

  switch (rotation) {
    case Rotation.CLOCKWISE_90:
      return direction.getClockWise();
    case Rotation.CLOCKWISE_180:
      return direction.getOpposite();
    case Rotation.COUNTERCLOCKWISE_90:
      return direction.getCounterClockWise();
    case Rotation.NONE:
    default:
      return direction;
  }
}
