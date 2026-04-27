import { describe, expect, test } from "vitest";
import { HumanoidModel } from "../../../src/renderer/model/humanoid-model";
import { PlayerModel } from "../../../src/renderer/model/player-model";
import { ModelPart } from "../../../src/renderer/model/geom/model-part";
import { PartPose } from "../../../src/renderer/model/geom/part-pose";
import { CubeDeformation } from "../../../src/renderer/model/geom/builders/cube-deformation";
import { CubeListBuilder } from "../../../src/renderer/model/geom/builders/cube-list-builder";
import { LayerDefinition } from "../../../src/renderer/model/geom/builders/layer-definition";
import { MeshDefinition } from "../../../src/renderer/model/geom/builders/mesh-definition";
import { BufferBuilder } from "../../../src/renderer/vertex/buffer-builder";
import { BufferVertexConsumer } from "../../../src/renderer/vertex/buffer-vertex-consumer";
import { DefaultVertexFormat } from "../../../src/renderer/vertex/default-vertex-format";
import { PoseStack, type PoseStackPose } from "../../../src/renderer/vertex/pose-stack";
import { VertexFormat } from "../../../src/renderer/vertex/vertex-format";

const LITTLE_ENDIAN = true;
const FULL_BRIGHT = 0x00f000f0;
const NO_OVERLAY = 0;

function bakePlayer(slim: boolean): ModelPart {
  return LayerDefinition.create(PlayerModel.createMesh(CubeDeformation.NONE, slim), 64, 64).bakeRoot();
}

function visitedCubes(root: ModelPart): Array<{ path: string; index: number; cube: ModelPart.Cube; pose: PoseStackPose }> {
  const visited: Array<{ path: string; index: number; cube: ModelPart.Cube; pose: PoseStackPose }> = [];
  root.visit(new PoseStack(), {
    visit(pose, path, index, cube): void {
      visited.push({ path, index, cube, pose });
    },
  });
  return visited;
}

function findCube(root: ModelPart, path: string, index = 0): ModelPart.Cube {
  const found = visitedCubes(root).find((entry) => entry.path === path && entry.index === index);
  if (found === undefined) {
    throw new Error(`Missing cube ${path}#${index}`);
  }

  return found.cube;
}

function expectClose(value: number, expected: number): void {
  expect(value).toBeCloseTo(expected, 6);
}

function readVertex(buffer: Uint8Array, index: number): {
  x: number;
  y: number;
  z: number;
  r: number;
  g: number;
  b: number;
  a: number;
  u: number;
  v: number;
  overlayU: number;
  overlayV: number;
  lightU: number;
  lightV: number;
  normalX: number;
  normalY: number;
  normalZ: number;
} {
  const stride = DefaultVertexFormat.NEW_ENTITY.getVertexSize();
  const offset = index * stride;
  const view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
  return {
    x: view.getFloat32(offset + 0, LITTLE_ENDIAN),
    y: view.getFloat32(offset + 4, LITTLE_ENDIAN),
    z: view.getFloat32(offset + 8, LITTLE_ENDIAN),
    r: buffer[offset + 12]!,
    g: buffer[offset + 13]!,
    b: buffer[offset + 14]!,
    a: buffer[offset + 15]!,
    u: view.getFloat32(offset + 16, LITTLE_ENDIAN),
    v: view.getFloat32(offset + 20, LITTLE_ENDIAN),
    overlayU: view.getInt16(offset + 24, LITTLE_ENDIAN),
    overlayV: view.getInt16(offset + 26, LITTLE_ENDIAN),
    lightU: view.getInt16(offset + 28, LITTLE_ENDIAN),
    lightV: view.getInt16(offset + 30, LITTLE_ENDIAN),
    normalX: view.getInt8(offset + 32),
    normalY: view.getInt8(offset + 33),
    normalZ: view.getInt8(offset + 34),
  };
}

describe("Vanilla model geometry foundation", () => {
  test("PartDefinition replacement preserves old child subtree", () => {
    const mesh = new MeshDefinition();
    const root = mesh.getRoot();
    const parent = root.addOrReplaceChild("parent", CubeListBuilder.create(), PartPose.ZERO);
    parent.addOrReplaceChild("child", CubeListBuilder.create().texOffs(0, 0).addBox(0.0, 0.0, 0.0, 1.0, 1.0, 1.0), PartPose.ZERO);

    root.addOrReplaceChild("parent", CubeListBuilder.create().texOffs(0, 0).addBox(0.0, 0.0, 0.0, 2.0, 2.0, 2.0), PartPose.ZERO);

    const bakedParent = LayerDefinition.create(mesh, 16, 16).bakeRoot().getChild("parent");
    expect(bakedParent.getChild("child").isEmpty()).toBe(false);
    expect(findCube(bakedParent, "", 0).maxX).toBe(2);
  });

  test("HumanoidModel.createMesh bakes vanilla base limbs", () => {
    const root = LayerDefinition.create(HumanoidModel.createMesh(CubeDeformation.NONE, 0.0), 64, 64).bakeRoot();

    expect(findCube(root, "/head").minY).toBe(-8);
    expect(findCube(root, "/body").maxY).toBe(12);
    expect(root.getChild("right_arm").x).toBe(-5);
    expect(root.getChild("left_arm").x).toBe(5);
    expect(root.getChild("right_leg").x).toBe(-1.9);
    expect(root.getChild("left_leg").x).toBe(1.9);
  });

  test("PlayerModel.createMesh bakes default and slim player layer roots", () => {
    const defaultRoot = bakePlayer(false);
    const slimRoot = bakePlayer(true);

    expect(defaultRoot.getAllParts().filter((part) => !part.isEmpty())).toHaveLength(14);
    expect(slimRoot.getAllParts().filter((part) => !part.isEmpty())).toHaveLength(14);
    expect(defaultRoot.getChild("right_arm").y).toBe(2);
    expect(slimRoot.getChild("right_arm").y).toBe(2.5);
    expect(findCube(defaultRoot, "/right_arm").maxX - findCube(defaultRoot, "/right_arm").minX).toBe(4);
    expect(findCube(slimRoot, "/right_arm").maxX - findCube(slimRoot, "/right_arm").minX).toBe(3);
    expect(findCube(defaultRoot, "/jacket").minX).toBe(-4);
    expect(findCube(slimRoot, "/left_sleeve").maxX - findCube(slimRoot, "/left_sleeve").minX).toBe(3);
  });

  test("ModelPart.render emits NEW_ENTITY cube vertices with vanilla UVs and normals", () => {
    const part = new ModelPart([new ModelPart.Cube(0, 0, 0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0, false, 64, 64)], new Map());
    const builder = new BufferBuilder(256);
    builder.begin(VertexFormat.Mode.QUADS, DefaultVertexFormat.NEW_ENTITY);

    part.render(new PoseStack(), builder, FULL_BRIGHT, NO_OVERLAY, 1.0, 0.5, 0.25, 1.0);
    builder.end();

    const { drawState, buffer } = builder.popNextBuffer();
    expect(drawState.format()).toBe(DefaultVertexFormat.NEW_ENTITY);
    expect(drawState.vertexCount()).toBe(24);
    expect(drawState.indexCount()).toBe(36);
    expect(drawState.sequentialIndex()).toBe(true);

    const vertex = readVertex(buffer, 0);
    expectClose(vertex.x, 2.0 / 16.0);
    expectClose(vertex.y, 0.0);
    expectClose(vertex.z, 2.0 / 16.0);
    expect(vertex.r).toBe(255);
    expect(vertex.g).toBe(127);
    expect(vertex.b).toBe(63);
    expect(vertex.a).toBe(255);
    expectClose(vertex.u, 6.0 / 64.0);
    expectClose(vertex.v, 2.0 / 64.0);
    expect(vertex.overlayU).toBe(0);
    expect(vertex.overlayV).toBe(0);
    expect(vertex.lightU).toBe(0x00f0);
    expect(vertex.lightV).toBe(0x00f0);
    expect(vertex.normalX).toBe(BufferVertexConsumer.normalIntValue(1));
    expect(vertex.normalY).toBe(BufferVertexConsumer.normalIntValue(0));
    expect(vertex.normalZ).toBe(BufferVertexConsumer.normalIntValue(0));
  });
});
