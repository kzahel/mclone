import { Matrix4f } from "../math/matrix4f";
import { Vector4f } from "../math/vector4f";
import { AABB } from "../../world/phys/aabb";

export class Frustum {
  private readonly frustumData = new Array<Vector4f>(6);
  private camX = 0;
  private camY = 0;
  private camZ = 0;

  public constructor(modelViewMatrix: Matrix4f, projectionMatrix: Matrix4f) {
    this.calculateFrustum(modelViewMatrix, projectionMatrix);
  }

  public prepare(camX: number, camY: number, camZ: number): void {
    this.camX = camX;
    this.camY = camY;
    this.camZ = camZ;
  }

  private calculateFrustum(modelViewMatrix: Matrix4f, projectionMatrix: Matrix4f): void {
    const matrix = projectionMatrix.copy();
    matrix.multiply(modelViewMatrix);
    matrix.transpose();
    this.getPlane(matrix, -1, 0, 0, 0);
    this.getPlane(matrix, 1, 0, 0, 1);
    this.getPlane(matrix, 0, -1, 0, 2);
    this.getPlane(matrix, 0, 1, 0, 3);
    this.getPlane(matrix, 0, 0, -1, 4);
    this.getPlane(matrix, 0, 0, 1, 5);
  }

  private getPlane(matrix: Matrix4f, x: number, y: number, z: number, index: number): void {
    const plane = new Vector4f(x, y, z, 1.0);
    plane.transform(matrix);
    plane.normalize();
    this.frustumData[index] = plane;
  }

  public isVisible(bounds: AABB): boolean {
    return this.cubeInFrustum(bounds.minX, bounds.minY, bounds.minZ, bounds.maxX, bounds.maxY, bounds.maxZ);
  }

  private cubeInFrustum(minX: number, minY: number, minZ: number, maxX: number, maxY: number, maxZ: number): boolean {
    const relativeMinX = minX - this.camX;
    const relativeMinY = minY - this.camY;
    const relativeMinZ = minZ - this.camZ;
    const relativeMaxX = maxX - this.camX;
    const relativeMaxY = maxY - this.camY;
    const relativeMaxZ = maxZ - this.camZ;

    for (let index = 0; index < 6; index++) {
      const plane = this.frustumData[index]!;
      if (
        !(plane.dot(new Vector4f(relativeMinX, relativeMinY, relativeMinZ, 1.0)) > 0.0) &&
        !(plane.dot(new Vector4f(relativeMaxX, relativeMinY, relativeMinZ, 1.0)) > 0.0) &&
        !(plane.dot(new Vector4f(relativeMinX, relativeMaxY, relativeMinZ, 1.0)) > 0.0) &&
        !(plane.dot(new Vector4f(relativeMaxX, relativeMaxY, relativeMinZ, 1.0)) > 0.0) &&
        !(plane.dot(new Vector4f(relativeMinX, relativeMinY, relativeMaxZ, 1.0)) > 0.0) &&
        !(plane.dot(new Vector4f(relativeMaxX, relativeMinY, relativeMaxZ, 1.0)) > 0.0) &&
        !(plane.dot(new Vector4f(relativeMinX, relativeMaxY, relativeMaxZ, 1.0)) > 0.0) &&
        !(plane.dot(new Vector4f(relativeMaxX, relativeMaxY, relativeMaxZ, 1.0)) > 0.0)
      ) {
        return false;
      }
    }

    return true;
  }
}
