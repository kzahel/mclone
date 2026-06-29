import * as THREE from "three";
import type { ClipSpec, FigureAsset, PartSpec, Vec3 } from "./dsl";

export interface FigureScene {
  root: THREE.Group;
  update(timeSeconds: number): void;
}

interface PartObject {
  part: PartSpec;
  group: THREE.Group;
  basePosition: THREE.Vector3;
  baseRotation: THREE.Euler;
}

export function createFigureScene(asset: FigureAsset, clipName?: string): FigureScene {
  const root = new THREE.Group();
  root.name = asset.name;

  const textureMap = new Map<string, THREE.Texture>();
  const materialMap = new Map<string, THREE.Material>();

  for (const [name, texture] of Object.entries(asset.textures)) {
    textureMap.set(name, asciiTextureToCanvasTexture(texture.palette, texture.pixels));
  }

  for (const [name, material] of Object.entries(asset.materials)) {
    materialMap.set(
      name,
      new THREE.MeshStandardMaterial({
        color: new THREE.Color(material.color),
        roughness: material.roughness ?? 0.85,
        metalness: material.metalness ?? 0,
      }),
    );
  }

  const parts = new Map<string, PartObject>();

  for (const part of asset.parts) {
    const group = new THREE.Group();
    group.name = part.name;
    group.position.copy(toVector(part.at ?? [0, 0, 0]));
    group.rotation.copy(toEuler(part.rot ?? [0, 0, 0]));

    const mesh = new THREE.Mesh(createGeometry(part), createMaterial(part, materialMap, textureMap));
    mesh.name = `${part.name}_mesh`;
    group.add(mesh);

    if (part.joint?.pivot) {
      group.add(createPivotMarker(part.joint.pivot));
    }

    parts.set(part.name, {
      part,
      group,
      basePosition: group.position.clone(),
      baseRotation: group.rotation.clone(),
    });
  }

  for (const item of parts.values()) {
    const parent = item.part.parent ? parts.get(item.part.parent) : undefined;
    if (parent) {
      parent.group.add(item.group);
    } else {
      root.add(item.group);
    }
  }

  const clip = clipName ? asset.clips[clipName] : undefined;

  return {
    root,
    update(timeSeconds: number) {
      for (const part of parts.values()) {
        part.group.position.copy(part.basePosition);
        part.group.rotation.copy(part.baseRotation);
      }
      if (clip) {
        applyClip(parts, clip, timeSeconds);
      }
    },
  };
}

function createGeometry(part: PartSpec): THREE.BufferGeometry {
  const primitive = part.primitive;
  if (primitive.kind === "box") {
    return new THREE.BoxGeometry(primitive.size[0], primitive.size[1], primitive.size[2]);
  }
  if (primitive.kind === "sphere") {
    return new THREE.SphereGeometry(
      primitive.radius,
      primitive.widthSegments ?? 16,
      primitive.heightSegments ?? 8,
    );
  }
  if (primitive.kind === "capsule") {
    return new THREE.CapsuleGeometry(
      primitive.radius,
      primitive.length,
      primitive.capSegments ?? 4,
      primitive.radialSegments ?? 10,
    );
  }
  return new THREE.CylinderGeometry(
    primitive.radiusTop,
    primitive.radiusBottom,
    primitive.length,
    primitive.radialSegments ?? 12,
  );
}

function createMaterial(
  part: PartSpec,
  materialMap: Map<string, THREE.Material>,
  textureMap: Map<string, THREE.Texture>,
): THREE.Material {
  const base = part.material ? materialMap.get(part.material) : undefined;
  const texture = part.texture ? textureMap.get(part.texture) : undefined;

  if (texture) {
    const cloned = base?.clone() as THREE.MeshStandardMaterial | undefined;
    const material =
      cloned ??
      new THREE.MeshStandardMaterial({
        color: new THREE.Color("#ffffff"),
        roughness: 0.85,
      });
    material.map = texture;
    material.needsUpdate = true;
    return material;
  }

  return base ?? new THREE.MeshStandardMaterial({ color: new THREE.Color("#d7dde2"), roughness: 0.85 });
}

function asciiTextureToCanvasTexture(palette: Record<string, string>, pixels: string[]): THREE.Texture {
  const width = pixels[0]?.length ?? 1;
  const height = pixels.length;
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) {
    throw new Error("Could not create texture canvas context");
  }

  context.imageSmoothingEnabled = false;
  for (let y = 0; y < height; y += 1) {
    const row = pixels[y] ?? "";
    for (let x = 0; x < width; x += 1) {
      context.fillStyle = palette[row[x] ?? "."] ?? "#ff00ff";
      context.fillRect(x, y, 1, 1);
    }
  }

  const texture = new THREE.CanvasTexture(canvas);
  texture.magFilter = THREE.NearestFilter;
  texture.minFilter = THREE.NearestFilter;
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.flipY = false;
  return texture;
}

function createPivotMarker(pivot: Vec3): THREE.Object3D {
  const marker = new THREE.Mesh(
    new THREE.SphereGeometry(0.035, 8, 4),
    new THREE.MeshBasicMaterial({ color: new THREE.Color("#1d4ed8") }),
  );
  marker.name = "pivot";
  marker.position.copy(toVector(pivot));
  return marker;
}

function applyClip(parts: Map<string, PartObject>, clip: ClipSpec, timeSeconds: number): void {
  const duration = Math.max(0, ...clip.keys.map(([, time]) => time));
  const localTime = clip.loop && duration > 0 ? timeSeconds % duration : Math.min(timeSeconds, duration);
  const byPart = new Map<string, ClipSpec["keys"]>();

  for (const key of clip.keys) {
    const keys = byPart.get(key[0]) ?? [];
    keys.push(key);
    byPart.set(key[0], keys);
  }

  for (const [partName, keys] of byPart.entries()) {
    const part = parts.get(partName);
    if (!part) {
      continue;
    }
    const sorted = [...keys].sort((left, right) => left[1] - right[1]);
    const [left, right] = surroundingKeys(sorted, localTime);
    const alpha = right[1] === left[1] ? 0 : (localTime - left[1]) / (right[1] - left[1]);
    const transform = mixTransform(left[2], right[2], alpha);

    if (transform.at) {
      part.group.position.copy(part.basePosition.clone().add(toVector(transform.at)));
    }
    if (transform.rot) {
      part.group.rotation.set(
        part.baseRotation.x + degToRad(transform.rot[0]),
        part.baseRotation.y + degToRad(transform.rot[1]),
        part.baseRotation.z + degToRad(transform.rot[2]),
      );
    }
    if (transform.scale) {
      part.group.scale.set(transform.scale[0], transform.scale[1], transform.scale[2]);
    }
  }
}

function surroundingKeys(keys: ClipSpec["keys"], time: number): [ClipSpec["keys"][number], ClipSpec["keys"][number]] {
  const first = keys[0];
  const last = keys[keys.length - 1];
  if (!first || !last) {
    throw new Error("Animation clip has no keys for part");
  }

  let left = first;
  let right = last;

  for (let index = 0; index < keys.length; index += 1) {
    const current = keys[index];
    const next = keys[index + 1];
    if (!current) {
      continue;
    }
    if (!next || time < next[1]) {
      left = current;
      right = next ?? current;
      break;
    }
  }

  return [left, right];
}

function mixTransform(left: ClipSpec["keys"][number][2], right: ClipSpec["keys"][number][2], alpha: number) {
  return {
    at: mixVec(left.at, right.at, alpha),
    rot: mixVec(left.rot, right.rot, alpha),
    scale: mixVec(left.scale, right.scale, alpha),
  };
}

function mixVec(left: Vec3 | undefined, right: Vec3 | undefined, alpha: number): Vec3 | undefined {
  if (!left && !right) {
    return undefined;
  }
  const a = left ?? right ?? [0, 0, 0];
  const b = right ?? left ?? [0, 0, 0];
  return [lerp(a[0], b[0], alpha), lerp(a[1], b[1], alpha), lerp(a[2], b[2], alpha)];
}

function lerp(left: number, right: number, alpha: number): number {
  return left + (right - left) * alpha;
}

function toVector(value: Vec3): THREE.Vector3 {
  return new THREE.Vector3(value[0], value[1], value[2]);
}

function toEuler(value: Vec3): THREE.Euler {
  return new THREE.Euler(degToRad(value[0]), degToRad(value[1]), degToRad(value[2]), "XYZ");
}

function degToRad(value: number): number {
  return (value * Math.PI) / 180;
}
