import { Direction } from "../../core/direction";
import { equal, lerp, positiveModulo } from "../../util/mth";
import { Matrix4f } from "../math/matrix4f";
import { Matrix3f } from "../math/matrix3f";
import { Vector3f } from "../math/vector3f";
import { Vector4f } from "../math/vector4f";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { BakedQuad } from "./baked-quad";
import { BlockMath } from "./block-math";
import { BlockElementFace } from "./block-element-face";
import { type BlockElementRotation } from "./block-element-rotation";
import { BlockFaceUV } from "./block-face-uv";
import { FaceInfo } from "./face-info";
import { type ModelState } from "./model-state";
import { Transformation } from "./transformation";
import { ResourceLocation } from "../../core/resource-location";

function transformVector(matrix: Matrix4f, vector: Vector3f, w: number): Vector3f {
  return new Vector3f(
    (((matrix.m00 * vector.x()) + (matrix.m01 * vector.y())) + (matrix.m02 * vector.z())) + (matrix.m03 * w),
    (((matrix.m10 * vector.x()) + (matrix.m11 * vector.y())) + (matrix.m12 * vector.z())) + (matrix.m13 * w),
    (((matrix.m20 * vector.x()) + (matrix.m21 * vector.y())) + (matrix.m22 * vector.z())) + (matrix.m23 * w),
  );
}

export function rotateDirection(matrix: Matrix4f, direction: Direction): Direction {
  const normal = direction.getNormal();
  const rotated = transformVector(matrix, new Vector3f(normal.getX(), normal.getY(), normal.getZ()), 0.0);
  rotated.normalize();
  let nearestDirection = Direction.UP;
  let largestDot = Number.NEGATIVE_INFINITY;
  for (const candidate of Direction.values()) {
    const candidateNormal = candidate.getNormal();
    const dot = rotated.dot(new Vector3f(candidateNormal.getX(), candidateNormal.getY(), candidateNormal.getZ()));
    if (dot > largestDot) {
      largestDot = dot;
      nearestDirection = candidate;
    }
  }

  return nearestDirection;
}

export class FaceBakery {
  public static readonly VERTEX_INT_SIZE = 8;
  private static readonly RESCALE_22_5 = (1.0 / Math.cos(Math.PI / 8)) - 1.0;
  private static readonly RESCALE_45 = (1.0 / Math.cos(Math.PI / 4)) - 1.0;
  public static readonly VERTEX_COUNT = 4;
  public static readonly UV_INDEX = 4;

  public bakeQuad(
    from: Vector3f,
    to: Vector3f,
    face: BlockElementFace,
    sprite: TextureAtlasSprite,
    direction: Direction,
    modelState: ModelState,
    elementRotation: BlockElementRotation | undefined,
    shade: boolean,
    location: ResourceLocation,
  ): BakedQuad {
    let uv = face.uv;
    if (modelState.isUvLocked()) {
      uv = FaceBakery.recomputeUVs(face.uv, direction, modelState.getRotation(), location);
    }

    const originalUvs = uv.uvs ? [...uv.uvs] : undefined;
    const shrinkRatio = sprite.uvShrinkRatio();
    if (uv.uvs !== undefined) {
      const averageU = (uv.uvs[0]! + uv.uvs[0]! + uv.uvs[2]! + uv.uvs[2]!) / 4.0;
      const averageV = (uv.uvs[1]! + uv.uvs[1]! + uv.uvs[3]! + uv.uvs[3]!) / 4.0;
      uv.uvs[0] = lerp(shrinkRatio, uv.uvs[0]!, averageU);
      uv.uvs[2] = lerp(shrinkRatio, uv.uvs[2]!, averageU);
      uv.uvs[1] = lerp(shrinkRatio, uv.uvs[1]!, averageV);
      uv.uvs[3] = lerp(shrinkRatio, uv.uvs[3]!, averageV);
    }

    const vertices = this.makeVertices(uv, sprite, direction, this.setupShape(from, to), modelState, elementRotation, shade);
    const quadDirection = FaceBakery.calculateFacing(vertices);
    if (originalUvs !== undefined) {
      uv.uvs = originalUvs;
    }

    if (elementRotation === undefined) {
      this.recalculateWinding(vertices, quadDirection);
    }

    return new BakedQuad(vertices, face.tintIndex, quadDirection, sprite, shade);
  }

  public static recomputeUVs(uv: BlockFaceUV, direction: Direction, transformation: Transformation, location: ResourceLocation): BlockFaceUV {
    const matrix = BlockMath.getUVLockTransform(transformation, direction, () => `Unable to resolve UVLock for model: ${location}`).getMatrix();
    const u0 = uv.getU(uv.getReverseIndex(0));
    const v0 = uv.getV(uv.getReverseIndex(0));
    const corner0 = new Vector4f(u0 / 16.0, v0 / 16.0, 0.0, 1.0);
    corner0.transform(matrix);
    const u1 = uv.getU(uv.getReverseIndex(2));
    const v1 = uv.getV(uv.getReverseIndex(2));
    const corner1 = new Vector4f(u1 / 16.0, v1 / 16.0, 0.0, 1.0);
    corner1.transform(matrix);
    const transformedU0 = 16.0 * corner0.x();
    const transformedV0 = 16.0 * corner0.y();
    const transformedU1 = 16.0 * corner1.x();
    const transformedV1 = 16.0 * corner1.y();
    const minU = Math.sign(u1 - u0) === Math.sign(transformedU1 - transformedU0) ? transformedU0 : transformedU1;
    const maxU = Math.sign(u1 - u0) === Math.sign(transformedU1 - transformedU0) ? transformedU1 : transformedU0;
    const minV = Math.sign(v1 - v0) === Math.sign(transformedV1 - transformedV0) ? transformedV0 : transformedV1;
    const maxV = Math.sign(v1 - v0) === Math.sign(transformedV1 - transformedV0) ? transformedV1 : transformedV0;
    const radians = (uv.rotation * Math.PI) / 180.0;
    const directionVector = new Vector3f(Math.cos(radians), Math.sin(radians), 0.0);
    directionVector.transform(new Matrix3f(matrix));
    const rotation = positiveModulo(-Math.round((Math.atan2(directionVector.y(), directionVector.x()) * 180.0) / Math.PI / 90.0) * 90, 360);
    return new BlockFaceUV([minU, minV, maxU, maxV], rotation);
  }

  private makeVertices(
    uv: BlockFaceUV,
    sprite: TextureAtlasSprite,
    direction: Direction,
    shape: readonly number[],
    modelState: ModelState,
    elementRotation: BlockElementRotation | undefined,
    shade: boolean,
  ): number[] {
    const vertices = new Array<number>(32).fill(0);
    for (let index = 0; index < 4; index++) {
      this.bakeVertex(vertices, index, direction, uv, shape, sprite, modelState, elementRotation, shade);
    }

    return vertices;
  }

  private setupShape(from: Vector3f, to: Vector3f): number[] {
    const shape = new Array<number>(Direction.values().length).fill(0);
    shape[FaceInfo.Constants.MIN_X] = from.x() / 16.0;
    shape[FaceInfo.Constants.MIN_Y] = from.y() / 16.0;
    shape[FaceInfo.Constants.MIN_Z] = from.z() / 16.0;
    shape[FaceInfo.Constants.MAX_X] = to.x() / 16.0;
    shape[FaceInfo.Constants.MAX_Y] = to.y() / 16.0;
    shape[FaceInfo.Constants.MAX_Z] = to.z() / 16.0;
    return shape;
  }

  private bakeVertex(
    vertices: number[],
    index: number,
    direction: Direction,
    uv: BlockFaceUV,
    shape: readonly number[],
    sprite: TextureAtlasSprite,
    modelState: ModelState,
    elementRotation: BlockElementRotation | undefined,
    _shade: boolean,
  ): void {
    const vertexInfo = FaceInfo.fromFacing(direction).getVertexInfo(index);
    const vertex = new Vector3f(shape[vertexInfo.xFace]!, shape[vertexInfo.yFace]!, shape[vertexInfo.zFace]!);
    this.applyElementRotation(vertex, elementRotation);
    this.applyModelRotation(vertex, modelState);
    this.fillVertex(vertices, index, vertex, sprite, uv);
  }

  private fillVertex(vertices: number[], index: number, position: Vector3f, sprite: TextureAtlasSprite, uv: BlockFaceUV): void {
    const vertexIndex = index * FaceBakery.VERTEX_INT_SIZE;
    vertices[vertexIndex] = floatToRawIntBits(position.x());
    vertices[vertexIndex + 1] = floatToRawIntBits(position.y());
    vertices[vertexIndex + 2] = floatToRawIntBits(position.z());
    vertices[vertexIndex + 3] = -1;
    vertices[vertexIndex + 4] = floatToRawIntBits(sprite.getU(uv.getU(index)));
    vertices[vertexIndex + 5] = floatToRawIntBits(sprite.getV(uv.getV(index)));
  }

  private applyElementRotation(vertex: Vector3f, rotation: BlockElementRotation | undefined): void {
    if (rotation === undefined) {
      return;
    }

    let axis: Vector3f;
    let scale: Vector3f;
    switch (rotation.axis) {
      case Direction.Axis.X:
        axis = Vector3f.XP;
        scale = new Vector3f(0.0, 1.0, 1.0);
        break;
      case Direction.Axis.Y:
        axis = Vector3f.YP;
        scale = new Vector3f(1.0, 0.0, 1.0);
        break;
      case Direction.Axis.Z:
        axis = Vector3f.ZP;
        scale = new Vector3f(1.0, 1.0, 0.0);
        break;
      default:
        throw new Error("There are only 3 axes");
    }

    const quaternion = axis.rotationDegrees(rotation.angle);
    if (rotation.rescale) {
      scale.mul(Math.abs(rotation.angle) === 22.5 ? FaceBakery.RESCALE_22_5 : FaceBakery.RESCALE_45);
      scale.add(1.0, 1.0, 1.0);
    } else {
      scale.set(1.0, 1.0, 1.0);
    }

    this.rotateVertexBy(vertex, rotation.origin.copy(), new Matrix4f(quaternion), scale);
  }

  public applyModelRotation(vertex: Vector3f, modelState: ModelState): void {
    const transformation = modelState.getRotation();
    if (transformation !== Transformation.identity()) {
      this.rotateVertexBy(vertex, new Vector3f(0.5, 0.5, 0.5), transformation.getMatrix(), new Vector3f(1.0, 1.0, 1.0));
    }
  }

  private rotateVertexBy(vertex: Vector3f, origin: Vector3f, matrix: Matrix4f, scale: Vector3f): void {
    const transformed = transformVector(matrix, new Vector3f(vertex.x() - origin.x(), vertex.y() - origin.y(), vertex.z() - origin.z()), 1.0);
    transformed.set(transformed.x() * scale.x(), transformed.y() * scale.y(), transformed.z() * scale.z());
    vertex.set(transformed.x() + origin.x(), transformed.y() + origin.y(), transformed.z() + origin.z());
  }

  public static calculateFacing(vertices: readonly number[]): Direction {
    const vertex0 = new Vector3f(intBitsToFloat(vertices[0]!), intBitsToFloat(vertices[1]!), intBitsToFloat(vertices[2]!));
    const vertex1 = new Vector3f(intBitsToFloat(vertices[8]!), intBitsToFloat(vertices[9]!), intBitsToFloat(vertices[10]!));
    const vertex2 = new Vector3f(intBitsToFloat(vertices[16]!), intBitsToFloat(vertices[17]!), intBitsToFloat(vertices[18]!));
    const left = vertex0.copy();
    left.sub(vertex1);
    const right = vertex2.copy();
    right.sub(vertex1);
    const normal = right.copy();
    normal.cross(left);
    normal.normalize();
    let result: Direction | undefined;
    let largestDot = 0.0;
    for (const direction of Direction.values()) {
      const normalVector = direction.getNormal();
      const dot = normal.dot(new Vector3f(normalVector.getX(), normalVector.getY(), normalVector.getZ()));
      if (dot >= 0.0 && dot > largestDot) {
        largestDot = dot;
        result = direction;
      }
    }

    return result ?? Direction.UP;
  }

  private recalculateWinding(vertices: number[], direction: Direction): void {
    const original = [...vertices];
    const bounds = new Array<number>(Direction.values().length).fill(0);
    bounds[FaceInfo.Constants.MIN_X] = 999.0;
    bounds[FaceInfo.Constants.MIN_Y] = 999.0;
    bounds[FaceInfo.Constants.MIN_Z] = 999.0;
    bounds[FaceInfo.Constants.MAX_X] = -999.0;
    bounds[FaceInfo.Constants.MAX_Y] = -999.0;
    bounds[FaceInfo.Constants.MAX_Z] = -999.0;

    for (let index = 0; index < 4; index++) {
      const vertexIndex = FaceBakery.VERTEX_INT_SIZE * index;
      const x = intBitsToFloat(original[vertexIndex]!);
      const y = intBitsToFloat(original[vertexIndex + 1]!);
      const z = intBitsToFloat(original[vertexIndex + 2]!);
      if (x < bounds[FaceInfo.Constants.MIN_X]!) {
        bounds[FaceInfo.Constants.MIN_X] = x;
      }

      if (y < bounds[FaceInfo.Constants.MIN_Y]!) {
        bounds[FaceInfo.Constants.MIN_Y] = y;
      }

      if (z < bounds[FaceInfo.Constants.MIN_Z]!) {
        bounds[FaceInfo.Constants.MIN_Z] = z;
      }

      if (x > bounds[FaceInfo.Constants.MAX_X]!) {
        bounds[FaceInfo.Constants.MAX_X] = x;
      }

      if (y > bounds[FaceInfo.Constants.MAX_Y]!) {
        bounds[FaceInfo.Constants.MAX_Y] = y;
      }

      if (z > bounds[FaceInfo.Constants.MAX_Z]!) {
        bounds[FaceInfo.Constants.MAX_Z] = z;
      }
    }

    const faceInfo = FaceInfo.fromFacing(direction);
    for (let index = 0; index < 4; index++) {
      const vertexIndex = FaceBakery.VERTEX_INT_SIZE * index;
      const vertexInfo = faceInfo.getVertexInfo(index);
      const x = bounds[vertexInfo.xFace]!;
      const y = bounds[vertexInfo.yFace]!;
      const z = bounds[vertexInfo.zFace]!;
      vertices[vertexIndex] = floatToRawIntBits(x);
      vertices[vertexIndex + 1] = floatToRawIntBits(y);
      vertices[vertexIndex + 2] = floatToRawIntBits(z);

      for (let sourceIndex = 0; sourceIndex < 4; sourceIndex++) {
        const sourceVertexIndex = FaceBakery.VERTEX_INT_SIZE * sourceIndex;
        const sourceX = intBitsToFloat(original[sourceVertexIndex]!);
        const sourceY = intBitsToFloat(original[sourceVertexIndex + 1]!);
        const sourceZ = intBitsToFloat(original[sourceVertexIndex + 2]!);
        if (equal(x, sourceX) && equal(y, sourceY) && equal(z, sourceZ)) {
          vertices[vertexIndex + 4] = original[sourceVertexIndex + 4]!;
          vertices[vertexIndex + 5] = original[sourceVertexIndex + 5]!;
        }
      }
    }
  }
}

function floatToRawIntBits(value: number): number {
  const array = new ArrayBuffer(4);
  const view = new DataView(array);
  view.setFloat32(0, value, true);
  return view.getInt32(0, true);
}

function intBitsToFloat(value: number): number {
  const array = new ArrayBuffer(4);
  const view = new DataView(array);
  view.setInt32(0, value, true);
  return view.getFloat32(0, true);
}
