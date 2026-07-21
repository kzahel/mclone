import assert from "node:assert/strict";
import test from "node:test";
import * as THREE from "three";
import type { ClipKey, FigureAsset } from "../src/dsl";
import { createFigureScene } from "../src/scene";

test("samples sparse animation channels independently", () => {
  const scene = createFigureScene(figureWithKeys([
    ["body", 0, { rot: [0, 0, 0] }],
    ["body", 0.25, { at: [0, 0, 0] }],
    ["body", 0.75, { at: [0, 2, 0] }],
    ["body", 1, { rot: [90, 0, 0] }],
  ]), "motion", { jointMarkers: false });

  scene.update(0.5);
  const body = requiredPart(scene.root, "body");

  assert.ok(Math.abs(body.position.y - 1) < 1e-6);
  const expected = new THREE.Quaternion().setFromEuler(new THREE.Euler(Math.PI / 4, 0, 0, "XYZ"));
  assert.ok(1 - Math.abs(body.quaternion.dot(expected)) < 1e-6);
});

test("rotation interpolation follows the shortest quaternion path", () => {
  const scene = createFigureScene(figureWithKeys([
    ["body", 0, { rot: [170, 0, 0] }],
    ["body", 1, { rot: [-170, 0, 0] }],
  ]), "motion", { jointMarkers: false });

  scene.update(0.5);
  const body = requiredPart(scene.root, "body");
  const expected = new THREE.Quaternion().setFromEuler(new THREE.Euler(Math.PI, 0, 0, "XYZ"));

  assert.ok(1 - Math.abs(body.quaternion.dot(expected)) < 1e-6);
});

test("repeated updates reset base scale and retain arbitrary-time motion", () => {
  const scene = createFigureScene(figureWithKeys([
    ["body", 0, { scale: [1, 1, 1], rot: [0, 0, 0] }],
    ["body", 1, { scale: [2, 2, 2], rot: [90, 0, 0] }],
  ]), "motion", { jointMarkers: false });
  const body = requiredPart(scene.root, "body");

  scene.update(0.75);
  assert.ok(Math.abs(body.scale.x - 1.75) < 1e-6);
  scene.update(0);
  assert.deepEqual(body.scale.toArray(), [1, 1, 1]);
  scene.update(0.123);
  const first = body.quaternion.clone();
  scene.update(0.125);
  assert.ok(1 - Math.abs(first.dot(body.quaternion)) > 1e-9);
});

test("switches clips on one shared semantic scene and disposes cleanly", () => {
  const asset = figureWithKeys([
    ["body", 0, { at: [0, 0, 0] }],
    ["body", 1, { at: [0, 1, 0] }],
  ]);
  asset.clips.reverse = {
    loop: false,
    keys: [
      ["body", 0, { at: [0, 1, 0] }],
      ["body", 1, { at: [0, 0, 0] }],
    ],
  };
  const scene = createFigureScene(asset, "motion", { jointMarkers: false });
  const body = requiredPart(scene.root, "body");

  scene.update(0.25);
  assert.ok(Math.abs(body.position.y - 0.25) < 1e-6);
  scene.setClip("reverse");
  scene.update(0.25);
  assert.ok(Math.abs(body.position.y - 0.75) < 1e-6);
  assert.throws(() => scene.setClip("missing"), /has no clip 'missing'/);
  assert.doesNotThrow(() => scene.dispose());
});

function requiredPart(root: THREE.Object3D, name: string): THREE.Object3D {
  const part = root.getObjectByName(name);
  assert.ok(part, `missing part '${name}'`);
  return part;
}

function figureWithKeys(keys: ClipKey[]): FigureAsset {
  return {
    schemaVersion: 1,
    name: "animation-test",
    materials: { white: { color: "#ffffff" } },
    textures: {},
    parts: [
      {
        name: "body",
        material: "white",
        primitive: { kind: "box", size: [1, 1, 1] },
      },
    ],
    clips: {
      motion: { loop: false, keys },
    },
  };
}
