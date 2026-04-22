import { Direction } from "../../core/direction";
import { Vector3f } from "../math/vector3f";

export class BlockElementRotation {
  public constructor(
    public readonly origin: Vector3f,
    public readonly axis: Direction.Axis,
    public readonly angle: number,
    public readonly rescale: boolean,
  ) {}
}
