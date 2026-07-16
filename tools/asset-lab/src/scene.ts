import * as THREE from "three";
import type { BoxFaceName, ClipSpec, FaceSpec, FigureAsset, PartSpec, Vec3 } from "./dsl";

export interface FigureScene {
  root: THREE.Group;
  update(timeSeconds: number): void;
}

export interface FigureSceneOptions {
  debug?: boolean;
  jointMarkers?: boolean;
  labels?: boolean;
}

interface PartObject {
  part: PartSpec;
  group: THREE.Group;
  content: THREE.Group;
  basePosition: THREE.Vector3;
  baseRotation: THREE.Euler;
  baseScale: THREE.Vector3;
}

export function createFigureScene(asset: FigureAsset, clipName?: string, options: FigureSceneOptions = {}): FigureScene {
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
    const pivot = partPivot(part);
    const group = new THREE.Group();
    group.name = part.name;
    group.position.copy(toVector(part.at ?? [0, 0, 0]).add(pivot));
    group.rotation.copy(toEuler(part.rot ?? [0, 0, 0]));

    const content = new THREE.Group();
    content.name = `${part.name}_content`;
    content.position.copy(pivot.clone().multiplyScalar(-1));
    group.add(content);

    const mesh = new THREE.Mesh(createGeometry(part), createMaterials(part, materialMap, textureMap));
    mesh.name = `${part.name}_mesh`;
    content.add(mesh);

    if (options.debug) {
      content.add(createWireframe(mesh.geometry));
      if (part.joint) {
        group.add(createPivotMarker());
        group.add(createAxisMarker(0.18));
      }
      if (options.labels) {
        content.add(createLabel(part.name));
      }
    } else if (options.jointMarkers !== false && part.joint?.pivot) {
      group.add(createPivotMarker());
    }

    parts.set(part.name, {
      part,
      group,
      content,
      basePosition: group.position.clone(),
      baseRotation: group.rotation.clone(),
      baseScale: group.scale.clone(),
    });
  }

  for (const item of parts.values()) {
    const parent = item.part.parent ? parts.get(item.part.parent) : undefined;
    if (parent) {
      parent.content.add(item.group);
    } else {
      root.add(item.group);
    }
  }

  const clip = clipName ? asset.clips[clipName] : undefined;

  if (options.debug) {
    root.add(createAxisMarker(0.35));
  }

  return {
    root,
    update(timeSeconds: number) {
      for (const part of parts.values()) {
        part.group.position.copy(part.basePosition);
        part.group.rotation.copy(part.baseRotation);
        part.group.scale.copy(part.baseScale);
      }
      if (clip) {
        applyClip(parts, clip, timeSeconds);
      }
    },
  };
}

export function clipDuration(clip: ClipSpec | undefined): number {
  if (!clip) {
    return 0;
  }
  return Math.max(0, ...clip.keys.map(([, time]) => time));
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

const BOX_FACE_MATERIAL_ORDER: BoxFaceName[] = ["east", "west", "up", "down", "south", "north"];

function createMaterials(
  part: PartSpec,
  materialMap: Map<string, THREE.Material>,
  textureMap: Map<string, THREE.Texture>,
): THREE.Material | THREE.Material[] {
  if (part.primitive.kind === "box" && part.primitive.faces) {
    const faces = part.primitive.faces;
    return BOX_FACE_MATERIAL_ORDER.map((faceName) =>
      createMaterial(part, faces[faceName], materialMap, textureMap),
    );
  }
  return createMaterial(part, undefined, materialMap, textureMap);
}

function partPivot(part: PartSpec): THREE.Vector3 {
  return toVector(part.joint?.pivot ?? part.pivot ?? [0, 0, 0]);
}

function createMaterial(
  part: PartSpec,
  face: FaceSpec | undefined,
  materialMap: Map<string, THREE.Material>,
  textureMap: Map<string, THREE.Texture>,
): THREE.Material {
  const materialName = face?.material ?? part.material;
  const textureName = face?.texture ?? part.texture;
  const base = materialName ? materialMap.get(materialName) : undefined;
  const texture = textureName ? textureMap.get(textureName) : undefined;

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
  texture.flipY = true;
  return texture;
}

function createPivotMarker(): THREE.Object3D {
  const marker = new THREE.Mesh(
    new THREE.SphereGeometry(0.035, 8, 4),
    new THREE.MeshBasicMaterial({ color: new THREE.Color("#1d4ed8") }),
  );
  marker.name = "pivot";
  return marker;
}

function createAxisMarker(size: number): THREE.Object3D {
  const axes = new THREE.AxesHelper(size);
  axes.name = "debug_axes";
  return axes;
}

function createWireframe(geometry: THREE.BufferGeometry): THREE.Object3D {
  const wireframe = new THREE.LineSegments(
    new THREE.WireframeGeometry(geometry),
    new THREE.LineBasicMaterial({
      color: new THREE.Color("#31506b"),
      transparent: true,
      opacity: 0.38,
      depthTest: false,
    }),
  );
  wireframe.name = "debug_wireframe";
  wireframe.renderOrder = 20;
  return wireframe;
}

function createLabel(text: string): THREE.Object3D {
  const canvas = document.createElement("canvas");
  canvas.width = 256;
  canvas.height = 64;
  const context = canvas.getContext("2d");
  if (!context) {
    throw new Error("Could not create label canvas context");
  }
  context.font = "28px system-ui, sans-serif";
  context.textAlign = "center";
  context.textBaseline = "middle";
  context.fillStyle = "rgba(255, 255, 255, 0.82)";
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.fillStyle = "#17202a";
  context.fillText(text, canvas.width / 2, canvas.height / 2);

  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  const sprite = new THREE.Sprite(
    new THREE.SpriteMaterial({
      map: texture,
      transparent: true,
      depthTest: false,
    }),
  );
  sprite.name = `label_${text}`;
  sprite.position.set(0, 0.28, 0);
  sprite.scale.set(0.34, 0.085, 1);
  sprite.renderOrder = 30;
  return sprite;
}

function applyClip(parts: Map<string, PartObject>, clip: ClipSpec, timeSeconds: number): void {
  if (!Number.isFinite(timeSeconds)) {
    throw new Error("Animation presentation time must be finite");
  }
  const duration = clipDuration(clip);
  const localTime = clip.loop && duration > 0
    ? ((timeSeconds % duration) + duration) % duration
    : THREE.MathUtils.clamp(timeSeconds, 0, duration);
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
    const translation = sampleChannel(sorted, localTime, (key) => key[2].at);
    if (translation) {
      part.group.position.copy(part.basePosition).add(toVector(translation));
    }

    const rotation = channelSpan(sorted, localTime, (key) => key[2].rot);
    if (rotation) {
      const left = additiveRotation(part.baseRotation, rotation.left);
      const right = additiveRotation(part.baseRotation, rotation.right);
      part.group.quaternion.slerpQuaternions(left, right, rotation.alpha).normalize();
    }

    const scale = sampleChannel(sorted, localTime, (key) => key[2].scale);
    if (scale) {
      part.group.scale.fromArray(scale);
    }
  }
}

interface ChannelSpan {
  left: Vec3;
  right: Vec3;
  alpha: number;
}

function sampleChannel(
  keys: ClipSpec["keys"],
  time: number,
  channel: (key: ClipSpec["keys"][number]) => Vec3 | undefined,
): Vec3 | undefined {
  const span = channelSpan(keys, time, channel);
  return span ? mixVec(span.left, span.right, span.alpha) : undefined;
}

function channelSpan(
  keys: ClipSpec["keys"],
  time: number,
  channel: (key: ClipSpec["keys"][number]) => Vec3 | undefined,
): ChannelSpan | undefined {
  const keyed = keys.flatMap((key) => {
    const value = channel(key);
    return value ? [{ time: key[1], value }] : [];
  });
  const first = keyed[0];
  if (!first) {
    return undefined;
  }

  let left = first;
  let right = first;

  for (const current of keyed.slice(1)) {
    if (time < current.time) {
      right = current;
      break;
    }
    left = current;
    right = current;
  }

  const alpha = right.time === left.time
    ? 0
    : THREE.MathUtils.clamp((time - left.time) / (right.time - left.time), 0, 1);
  return { left: left.value, right: right.value, alpha };
}

function mixVec(left: Vec3, right: Vec3, alpha: number): Vec3 {
  return [
    lerp(left[0], right[0], alpha),
    lerp(left[1], right[1], alpha),
    lerp(left[2], right[2], alpha),
  ];
}

function additiveRotation(base: THREE.Euler, delta: Vec3): THREE.Quaternion {
  return new THREE.Quaternion().setFromEuler(new THREE.Euler(
    base.x + degToRad(delta[0]),
    base.y + degToRad(delta[1]),
    base.z + degToRad(delta[2]),
    "XYZ",
  ));
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
