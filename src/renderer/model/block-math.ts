import { Direction } from "../../core/direction";
import { Matrix4f } from "../math/matrix4f";
import { Vector3f } from "../math/vector3f";
import { Transformation } from "./transformation";

function rotateDirection(matrix: Matrix4f, direction: Direction): Direction {
  const normal = direction.getNormal();
  const rotated = new Vector3f(normal.getX(), normal.getY(), normal.getZ());
  const transformed = new Vector3f(
    ((matrix.m00 * rotated.x()) + (matrix.m01 * rotated.y())) + (matrix.m02 * rotated.z()),
    ((matrix.m10 * rotated.x()) + (matrix.m11 * rotated.y())) + (matrix.m12 * rotated.z()),
    ((matrix.m20 * rotated.x()) + (matrix.m21 * rotated.y())) + (matrix.m22 * rotated.z()),
  );
  transformed.normalize();
  let result = Direction.UP;
  let largestDot = Number.NEGATIVE_INFINITY;
  for (const candidate of Direction.values()) {
    const candidateNormal = candidate.getNormal();
    const dot = transformed.dot(new Vector3f(candidateNormal.getX(), candidateNormal.getY(), candidateNormal.getZ()));
    if (dot > largestDot) {
      largestDot = dot;
      result = candidate;
    }
  }

  return result;
}

export class BlockMath {
  public static readonly VANILLA_UV_TRANSFORM_LOCAL_TO_GLOBAL = new Map<Direction, Transformation>([
    [Direction.SOUTH, Transformation.identity()],
    [Direction.EAST, new Transformation(null, Vector3f.YP.rotationDegrees(90.0), null, null)],
    [Direction.WEST, new Transformation(null, Vector3f.YP.rotationDegrees(-90.0), null, null)],
    [Direction.NORTH, new Transformation(null, Vector3f.YP.rotationDegrees(180.0), null, null)],
    [Direction.UP, new Transformation(null, Vector3f.XP.rotationDegrees(-90.0), null, null)],
    [Direction.DOWN, new Transformation(null, Vector3f.XP.rotationDegrees(90.0), null, null)],
  ]);

  public static readonly VANILLA_UV_TRANSFORM_GLOBAL_TO_LOCAL = new Map<Direction, Transformation>(
    Direction.values().map((direction) => {
      const inverse = BlockMath.VANILLA_UV_TRANSFORM_LOCAL_TO_GLOBAL.get(direction)!.inverse();
      if (inverse === undefined) {
        throw new Error(`Unable to invert vanilla UV transform for ${direction}`);
      }

      return [direction, inverse] as const;
    }),
  );

  public static blockCenterToCorner(transformation: Transformation): Transformation {
    const matrix = Matrix4f.createTranslateMatrix(0.5, 0.5, 0.5);
    matrix.multiply(transformation.getMatrix());
    matrix.multiply(Matrix4f.createTranslateMatrix(-0.5, -0.5, -0.5));
    return new Transformation(matrix);
  }

  public static blockCornerToCenter(transformation: Transformation): Transformation {
    const matrix = Matrix4f.createTranslateMatrix(-0.5, -0.5, -0.5);
    matrix.multiply(transformation.getMatrix());
    matrix.multiply(Matrix4f.createTranslateMatrix(0.5, 0.5, 0.5));
    return new Transformation(matrix);
  }

  public static getUVLockTransform(transformation: Transformation, direction: Direction, warning: () => string): Transformation {
    const rotatedDirection = rotateDirection(transformation.getMatrix(), direction);
    const inverse = transformation.inverse();
    if (inverse === undefined) {
      console.warn(warning());
      return new Transformation(null, null, new Vector3f(0.0, 0.0, 0.0), null);
    }

    const result = BlockMath.VANILLA_UV_TRANSFORM_GLOBAL_TO_LOCAL.get(direction)!
      .compose(inverse)
      .compose(BlockMath.VANILLA_UV_TRANSFORM_LOCAL_TO_GLOBAL.get(rotatedDirection)!);
    return BlockMath.blockCenterToCorner(result);
  }
}
