import { Matrix3f } from "../math/matrix3f";
import { Matrix4f } from "../math/matrix4f";
import { Vector3f } from "../math/vector3f";
import { Vector4f } from "../math/vector4f";
import type { BakedQuad } from "../model/baked-quad";
import type { PoseStackPose } from "./pose-stack";

export abstract class VertexConsumer {
  public vertex(x: number, y: number, z: number): this;
  public vertex(
    x: number,
    y: number,
    z: number,
    r: number,
    g: number,
    b: number,
    a: number,
    u: number,
    v: number,
    overlay: number,
    light: number,
    normalX: number,
    normalY: number,
    normalZ: number,
  ): void;
  public vertex(...args: number[]): this | void {
    if (args.length === 3) {
      return this.vertexPosition(args[0]!, args[1]!, args[2]!);
    }

    this.vertexPosition(args[0]!, args[1]!, args[2]!);
    this.colorFloat(args[3]!, args[4]!, args[5]!, args[6]!);
    this.uv(args[7]!, args[8]!);
    this.overlayCoords(args[9]!);
    this.uv2(args[10]!);
    this.normal(args[11]!, args[12]!, args[13]!);
    this.endVertex();
  }

  protected abstract vertexPosition(x: number, y: number, z: number): this;

  public abstract color(r: number, g: number, b: number, a: number): this;

  public abstract uv(u: number, v: number): this;

  public overlayCoords(overlay: number): this;
  public overlayCoords(u: number, v: number): this;
  public overlayCoords(uOrOverlay: number, v?: number): this {
    if (v === undefined) {
      return this.overlayCoordsPair(uOrOverlay & 0xffff, (uOrOverlay >> 16) & 0xffff);
    }

    return this.overlayCoordsPair(uOrOverlay, v);
  }

  protected abstract overlayCoordsPair(u: number, v: number): this;

  public uv2(light: number): this;
  public uv2(u: number, v: number): this;
  public uv2(uOrLight: number, v?: number): this {
    if (v === undefined) {
      return this.uv2Pair(uOrLight & 0xffff, (uOrLight >> 16) & 0xffff);
    }

    return this.uv2Pair(uOrLight, v);
  }

  protected abstract uv2Pair(u: number, v: number): this;

  public abstract normal(x: number, y: number, z: number): this;

  public abstract endVertex(): void;

  public abstract defaultColor(r: number, g: number, b: number, a: number): void;

  public abstract unsetDefaultColor(): void;

  public colorFloat(r: number, g: number, b: number, a: number): this {
    return this.color(Math.trunc(r * 255), Math.trunc(g * 255), Math.trunc(b * 255), Math.trunc(a * 255));
  }

  public putBulkData(pose: PoseStackPose, quad: BakedQuad, red: number, green: number, blue: number, light: number, overlay: number): void;
  public putBulkData(
    pose: PoseStackPose,
    quad: BakedQuad,
    brightness: readonly number[],
    red: number,
    green: number,
    blue: number,
    lightmap: readonly number[],
    overlay: number,
    useQuadColorData: boolean,
  ): void;
  public putBulkData(
    pose: PoseStackPose,
    quad: BakedQuad,
    brightnessOrRed: readonly number[] | number,
    redOrGreen: number,
    greenOrBlue: number,
    blueOrLight: number,
    lightmapOrOverlay: readonly number[] | number,
    overlayOrUseQuadColorData?: number,
    useQuadColorData = false,
  ): void {
    if (typeof brightnessOrRed === "number") {
      this.putBulkData(
        pose,
        quad,
        [1, 1, 1, 1],
        brightnessOrRed,
        redOrGreen,
        greenOrBlue,
        [blueOrLight, blueOrLight, blueOrLight, blueOrLight],
        lightmapOrOverlay as number,
        false,
      );
      return;
    }

    const brightness = [...brightnessOrRed];
    const lightmap = [...(lightmapOrOverlay as readonly number[])];
    const vertices = quad.getVertices();
    const normal = quad.getDirection().getNormal();
    const transformedNormal = new Vector3f(normal.getX(), normal.getY(), normal.getZ());
    transformedNormal.transform(pose.normal());

    for (let vertexIndex = 0; vertexIndex < vertices.length / 8; vertexIndex++) {
      const base = vertexIndex * 8;
      const x = intBitsToFloat(vertices[base]!);
      const y = intBitsToFloat(vertices[base + 1]!);
      const z = intBitsToFloat(vertices[base + 2]!);
      let vertexRed: number;
      let vertexGreen: number;
      let vertexBlue: number;
      if (useQuadColorData) {
        vertexRed = ((vertices[base + 3]! >> 0) & 0xff) / 255.0;
        vertexGreen = ((vertices[base + 3]! >> 8) & 0xff) / 255.0;
        vertexBlue = ((vertices[base + 3]! >> 16) & 0xff) / 255.0;
        vertexRed *= brightness[vertexIndex]! * redOrGreen;
        vertexGreen *= brightness[vertexIndex]! * greenOrBlue;
        vertexBlue *= brightness[vertexIndex]! * blueOrLight;
      } else {
        vertexRed = brightness[vertexIndex]! * redOrGreen;
        vertexGreen = brightness[vertexIndex]! * greenOrBlue;
        vertexBlue = brightness[vertexIndex]! * blueOrLight;
      }

      const u = intBitsToFloat(vertices[base + 4]!);
      const v = intBitsToFloat(vertices[base + 5]!);
      const transformed = new Vector4f(x, y, z, 1.0);
      transformed.transform(pose.pose());
      this.vertex(
        transformed.x(),
        transformed.y(),
        transformed.z(),
        vertexRed,
        vertexGreen,
        vertexBlue,
        1.0,
        u,
        v,
        overlayOrUseQuadColorData!,
        lightmap[vertexIndex]!,
        transformedNormal.x(),
        transformedNormal.y(),
        transformedNormal.z(),
      );
    }
  }

  public vertexMatrix(matrix: Matrix4f, x: number, y: number, z: number): this {
    const transformed = new Vector4f(x, y, z, 1.0);
    transformed.transform(matrix);
    return this.vertex(transformed.x(), transformed.y(), transformed.z()) as this;
  }

  public normalMatrix(matrix: Matrix3f, x: number, y: number, z: number): this {
    const transformed = new Vector3f(x, y, z);
    transformed.transform(matrix);
    return this.normal(transformed.x(), transformed.y(), transformed.z());
  }
}

function intBitsToFloat(value: number): number {
  const array = new ArrayBuffer(4);
  const view = new DataView(array);
  view.setInt32(0, value, true);
  return view.getFloat32(0, true);
}
