import { Direction } from "../../../core/direction";
import { Matrix3f } from "../../math/matrix3f";
import { Matrix4f } from "../../math/matrix4f";
import { Vector3f } from "../../math/vector3f";
import { Vector4f } from "../../math/vector4f";
import { PoseStack, type PoseStackPose } from "../../vertex/pose-stack";
import type { VertexConsumer } from "../../vertex/vertex-consumer";
import { PartPose } from "./part-pose";

export class ModelPart {
  public x = 0;
  public y = 0;
  public z = 0;
  public xRot = 0;
  public yRot = 0;
  public zRot = 0;
  public visible = true;

  public constructor(
    private readonly cubes: readonly ModelPart.Cube[],
    private readonly children: ReadonlyMap<string, ModelPart>,
  ) {}

  public storePose(): PartPose {
    return PartPose.offsetAndRotation(this.x, this.y, this.z, this.xRot, this.yRot, this.zRot);
  }

  public loadPose(partPose: PartPose): void {
    this.x = partPose.x;
    this.y = partPose.y;
    this.z = partPose.z;
    this.xRot = partPose.xRot;
    this.yRot = partPose.yRot;
    this.zRot = partPose.zRot;
  }

  public copyFrom(modelPart: ModelPart): void {
    this.xRot = modelPart.xRot;
    this.yRot = modelPart.yRot;
    this.zRot = modelPart.zRot;
    this.x = modelPart.x;
    this.y = modelPart.y;
    this.z = modelPart.z;
  }

  public getChild(name: string): ModelPart {
    const child = this.children.get(name);
    if (child === undefined) {
      throw new Error(`Can't find part ${name}`);
    }

    return child;
  }

  public setPos(x: number, y: number, z: number): void {
    this.x = x;
    this.y = y;
    this.z = z;
  }

  public setRotation(xRot: number, yRot: number, zRot: number): void {
    this.xRot = xRot;
    this.yRot = yRot;
    this.zRot = zRot;
  }

  public render(poseStack: PoseStack, vertexConsumer: VertexConsumer, packedLight: number, packedOverlay: number): void;
  public render(
    poseStack: PoseStack,
    vertexConsumer: VertexConsumer,
    packedLight: number,
    packedOverlay: number,
    red: number,
    green: number,
    blue: number,
    alpha: number,
  ): void;
  public render(
    poseStack: PoseStack,
    vertexConsumer: VertexConsumer,
    packedLight: number,
    packedOverlay: number,
    red = 1.0,
    green = 1.0,
    blue = 1.0,
    alpha = 1.0,
  ): void {
    if (!this.visible) {
      return;
    }

    if (this.cubes.length === 0 && this.children.size === 0) {
      return;
    }

    poseStack.pushPose();
    this.translateAndRotate(poseStack);
    this.compile(poseStack.last(), vertexConsumer, packedLight, packedOverlay, red, green, blue, alpha);

    for (const child of this.children.values()) {
      child.render(poseStack, vertexConsumer, packedLight, packedOverlay, red, green, blue, alpha);
    }

    poseStack.popPose();
  }

  public visit(poseStack: PoseStack, visitor: ModelPart.Visitor): void {
    this.visitWithPath(poseStack, visitor, "");
  }

  private visitWithPath(poseStack: PoseStack, visitor: ModelPart.Visitor, path: string): void {
    if (this.cubes.length === 0 && this.children.size === 0) {
      return;
    }

    poseStack.pushPose();
    this.translateAndRotate(poseStack);
    const pose = poseStack.last();

    for (let index = 0; index < this.cubes.length; index++) {
      visitor.visit(pose, path, index, this.cubes[index]!);
    }

    const childPrefix = `${path}/`;
    for (const [childName, child] of this.children) {
      child.visitWithPath(poseStack, visitor, `${childPrefix}${childName}`);
    }

    poseStack.popPose();
  }

  public translateAndRotate(poseStack: PoseStack): void {
    poseStack.translate(this.x / 16.0, this.y / 16.0, this.z / 16.0);
    if (this.zRot !== 0.0) {
      poseStack.mulPose(Vector3f.ZP.rotation(this.zRot));
    }

    if (this.yRot !== 0.0) {
      poseStack.mulPose(Vector3f.YP.rotation(this.yRot));
    }

    if (this.xRot !== 0.0) {
      poseStack.mulPose(Vector3f.XP.rotation(this.xRot));
    }
  }

  private compile(
    pose: PoseStackPose,
    vertexConsumer: VertexConsumer,
    packedLight: number,
    packedOverlay: number,
    red: number,
    green: number,
    blue: number,
    alpha: number,
  ): void {
    for (const cube of this.cubes) {
      cube.compile(pose, vertexConsumer, packedLight, packedOverlay, red, green, blue, alpha);
    }
  }

  public getRandomCube(random: { nextInt(bound: number): number }): ModelPart.Cube {
    return this.cubes[random.nextInt(this.cubes.length)]!;
  }

  public isEmpty(): boolean {
    return this.cubes.length === 0;
  }

  public getAllParts(): ModelPart[] {
    const parts: ModelPart[] = [this];
    for (const child of this.children.values()) {
      parts.push(...child.getAllParts());
    }
    return parts;
  }
}

export namespace ModelPart {
  export class Cube {
    private readonly polygons: readonly Polygon[];
    public readonly minX: number;
    public readonly minY: number;
    public readonly minZ: number;
    public readonly maxX: number;
    public readonly maxY: number;
    public readonly maxZ: number;

    public constructor(
      texCoordU: number,
      texCoordV: number,
      originX: number,
      originY: number,
      originZ: number,
      dimensionX: number,
      dimensionY: number,
      dimensionZ: number,
      growX: number,
      growY: number,
      growZ: number,
      mirror: boolean,
      texWidth: number,
      texHeight: number,
    ) {
      this.minX = originX;
      this.minY = originY;
      this.minZ = originZ;
      this.maxX = originX + dimensionX;
      this.maxY = originY + dimensionY;
      this.maxZ = originZ + dimensionZ;
      const polygons = new Array<Polygon>(6);
      let maxX = originX + dimensionX;
      const maxY = originY + dimensionY;
      const maxZ = originZ + dimensionZ;
      originX -= growX;
      originY -= growY;
      originZ -= growZ;
      maxX += growX;
      const grownMaxY = maxY + growY;
      const grownMaxZ = maxZ + growZ;
      if (mirror) {
        const oldMaxX = maxX;
        maxX = originX;
        originX = oldMaxX;
      }

      const vertex000 = new Vertex(originX, originY, originZ, 0.0, 0.0);
      const vertex100 = new Vertex(maxX, originY, originZ, 0.0, 8.0);
      const vertex110 = new Vertex(maxX, grownMaxY, originZ, 8.0, 8.0);
      const vertex010 = new Vertex(originX, grownMaxY, originZ, 8.0, 0.0);
      const vertex001 = new Vertex(originX, originY, grownMaxZ, 0.0, 0.0);
      const vertex101 = new Vertex(maxX, originY, grownMaxZ, 0.0, 8.0);
      const vertex111 = new Vertex(maxX, grownMaxY, grownMaxZ, 8.0, 8.0);
      const vertex011 = new Vertex(originX, grownMaxY, grownMaxZ, 8.0, 0.0);
      const u0 = texCoordU;
      const u1 = texCoordU + dimensionZ;
      const u2 = texCoordU + dimensionZ + dimensionX;
      const u3 = texCoordU + dimensionZ + dimensionX + dimensionX;
      const u4 = texCoordU + dimensionZ + dimensionX + dimensionZ;
      const u5 = texCoordU + dimensionZ + dimensionX + dimensionZ + dimensionX;
      const v0 = texCoordV;
      const v1 = texCoordV + dimensionZ;
      const v2 = texCoordV + dimensionZ + dimensionY;
      polygons[2] = new Polygon([vertex101, vertex001, vertex000, vertex100], u1, v0, u2, v1, texWidth, texHeight, mirror, Direction.DOWN);
      polygons[3] = new Polygon([vertex110, vertex010, vertex011, vertex111], u2, v1, u3, v0, texWidth, texHeight, mirror, Direction.UP);
      polygons[1] = new Polygon([vertex000, vertex001, vertex011, vertex010], u0, v1, u1, v2, texWidth, texHeight, mirror, Direction.WEST);
      polygons[4] = new Polygon([vertex100, vertex000, vertex010, vertex110], u1, v1, u2, v2, texWidth, texHeight, mirror, Direction.NORTH);
      polygons[0] = new Polygon([vertex101, vertex100, vertex110, vertex111], u2, v1, u4, v2, texWidth, texHeight, mirror, Direction.EAST);
      polygons[5] = new Polygon([vertex001, vertex101, vertex111, vertex011], u4, v1, u5, v2, texWidth, texHeight, mirror, Direction.SOUTH);
      this.polygons = polygons;
    }

    public compile(
      pose: PoseStackPose,
      vertexConsumer: VertexConsumer,
      packedLight: number,
      packedOverlay: number,
      red: number,
      green: number,
      blue: number,
      alpha: number,
    ): void {
      const poseMatrix = pose.pose();
      const normalMatrix = pose.normal();

      for (const polygon of this.polygons) {
        polygon.compile(poseMatrix, normalMatrix, vertexConsumer, packedLight, packedOverlay, red, green, blue, alpha);
      }
    }
  }

  export class Polygon {
    public readonly normal: Vector3f;

    public constructor(
      public readonly vertices: Vertex[],
      minU: number,
      minV: number,
      maxU: number,
      maxV: number,
      texWidth: number,
      texHeight: number,
      mirror: boolean,
      direction: Direction,
    ) {
      const shrinkU = 0.0 / texWidth;
      const shrinkV = 0.0 / texHeight;
      vertices[0] = vertices[0]!.remap(maxU / texWidth - shrinkU, minV / texHeight + shrinkV);
      vertices[1] = vertices[1]!.remap(minU / texWidth + shrinkU, minV / texHeight + shrinkV);
      vertices[2] = vertices[2]!.remap(minU / texWidth + shrinkU, maxV / texHeight - shrinkV);
      vertices[3] = vertices[3]!.remap(maxU / texWidth - shrinkU, maxV / texHeight - shrinkV);
      if (mirror) {
        const vertexCount = vertices.length;
        for (let index = 0; index < vertexCount / 2; index++) {
          const oldVertex = vertices[index]!;
          vertices[index] = vertices[vertexCount - 1 - index]!;
          vertices[vertexCount - 1 - index] = oldVertex;
        }
      }

      this.normal = new Vector3f(direction.getStepX(), direction.getStepY(), direction.getStepZ());
      if (mirror) {
        this.normal.mul(-1.0, 1.0, 1.0);
      }
    }

    public compile(
      pose: Matrix4f,
      normal: Matrix3f,
      vertexConsumer: VertexConsumer,
      packedLight: number,
      packedOverlay: number,
      red: number,
      green: number,
      blue: number,
      alpha: number,
    ): void {
      const transformedNormal = this.normal.copy();
      transformedNormal.transform(normal);
      const normalX = transformedNormal.x();
      const normalY = transformedNormal.y();
      const normalZ = transformedNormal.z();

      for (const vertex of this.vertices) {
        const x = vertex.pos.x() / 16.0;
        const y = vertex.pos.y() / 16.0;
        const z = vertex.pos.z() / 16.0;
        const transformed = new Vector4f(x, y, z, 1.0);
        transformed.transform(pose);
        vertexConsumer.vertex(
          transformed.x(),
          transformed.y(),
          transformed.z(),
          red,
          green,
          blue,
          alpha,
          vertex.u,
          vertex.v,
          packedOverlay,
          packedLight,
          normalX,
          normalY,
          normalZ,
        );
      }
    }
  }

  export class Vertex {
    public constructor(pos: Vector3f, u: number, v: number);
    public constructor(x: number, y: number, z: number, u: number, v: number);
    public constructor(posOrX: Vector3f | number, yOrU: number, zOrV: number, u?: number, v?: number) {
      if (posOrX instanceof Vector3f) {
        this.pos = posOrX;
        this.u = yOrU;
        this.v = zOrV;
      } else {
        this.pos = new Vector3f(posOrX, yOrU, zOrV);
        this.u = u!;
        this.v = v!;
      }
    }

    public readonly pos: Vector3f;
    public readonly u: number;
    public readonly v: number;

    public remap(u: number, v: number): Vertex {
      return new Vertex(this.pos, u, v);
    }
  }

  export interface Visitor {
    visit(pose: PoseStackPose, path: string, index: number, cube: Cube): void;
  }
}
