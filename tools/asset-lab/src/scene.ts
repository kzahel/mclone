import * as THREE from "three";
import {
  TRANSPARENT_PALETTE_COLOR,
  type BoxFaceName,
  type ClipSpec,
  type FaceSpec,
  type FigureAsset,
  type FigureAlphaMode,
  type MaterialSpec,
  type PartSpec,
  type Vec3,
  asciiTextureHasTransparency,
} from "./dsl";

export interface FigureScene {
  root: THREE.Group;
  setClip(clipName?: string): void;
  update(timeSeconds: number): void;
  dispose(): void;
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

interface PreparedClip {
  source: ClipSpec;
  keysByPart: Map<string, ClipSpec["keys"]>;
}

interface PreparedAsciiTexture {
  hasTransparency: boolean;
  texture: THREE.Texture;
}

interface PreparedThreeMaterial {
  color: THREE.Material;
  depth?: THREE.Material;
  mode: FigureAlphaMode;
}

export function createFigureScene(asset: FigureAsset, clipName?: string, options: FigureSceneOptions = {}): FigureScene {
  const root = new THREE.Group();
  root.name = asset.name;

  const textureMap = new Map<string, PreparedAsciiTexture>();

  for (const [name, texture] of Object.entries(asset.textures)) {
    textureMap.set(name, asciiTextureToCanvasTexture(texture.palette, texture.pixels));
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

    const geometry = createGeometry(part);
    const preparedMaterials = createMaterials(part, asset.materials, textureMap);
    const colorMaterials = mapPreparedMaterials(preparedMaterials, (material) => material.color);
    const mesh = new THREE.Mesh(geometry, colorMaterials);
    mesh.name = `${part.name}_mesh`;
    content.add(mesh);

    if (flattenPreparedMaterials(preparedMaterials).some((material) => material.depth)) {
      const depthMaterials = mapPreparedMaterials(
        preparedMaterials,
        (material) => material.depth ?? hiddenMaterial(),
      );
      const depthMesh = new THREE.Mesh(geometry, depthMaterials);
      depthMesh.name = `${part.name}_blend_depth_mesh`;
      depthMesh.renderOrder = 1;
      content.add(depthMesh);
    }

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

  const clips = new Map(
    Object.entries(asset.clips).map(([name, clip]) => [name, prepareClip(clip)]),
  );
  let activeClip = clipName === undefined ? undefined : requiredClip(clips, clipName, asset.name);

  if (options.debug) {
    root.add(createAxisMarker(0.35));
  }

  return {
    root,
    setClip(nextClipName?: string) {
      activeClip = nextClipName === undefined
        ? undefined
        : requiredClip(clips, nextClipName, asset.name);
    },
    update(timeSeconds: number) {
      for (const part of parts.values()) {
        part.group.position.copy(part.basePosition);
        part.group.rotation.copy(part.baseRotation);
        part.group.scale.copy(part.baseScale);
      }
      if (activeClip) {
        applyClip(parts, activeClip, timeSeconds);
      }
    },
    dispose() {
      disposeObjectResources(
        root,
        [],
        [...textureMap.values()].map((prepared) => prepared.texture),
      );
      root.clear();
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
  materials: Record<string, MaterialSpec>,
  textureMap: Map<string, PreparedAsciiTexture>,
): PreparedThreeMaterial | PreparedThreeMaterial[] {
  if (part.primitive.kind === "box" && part.primitive.faces) {
    const faces = part.primitive.faces;
    return BOX_FACE_MATERIAL_ORDER.map((faceName) =>
      createMaterial(part, faces[faceName], materials, textureMap),
    );
  }
  return createMaterial(part, undefined, materials, textureMap);
}

function partPivot(part: PartSpec): THREE.Vector3 {
  return toVector(part.joint?.pivot ?? part.pivot ?? [0, 0, 0]);
}

function createMaterial(
  part: PartSpec,
  face: FaceSpec | undefined,
  materials: Record<string, MaterialSpec>,
  textureMap: Map<string, PreparedAsciiTexture>,
): PreparedThreeMaterial {
  const materialName = face?.material ?? part.material;
  const textureName = face?.texture ?? part.texture;
  const spec = materialName ? materials[materialName] : undefined;
  const texture = textureName ? textureMap.get(textureName) : undefined;
  const declaredMode = spec?.alphaMode ?? "opaque";
  const mode = declaredMode === "opaque" && texture?.hasTransparency
    ? "mask"
    : declaredMode;
  const opacity = spec?.opacity ?? 1;
  const material = new THREE.MeshStandardMaterial({
    color: new THREE.Color(spec?.color ?? (texture ? "#ffffff" : "#d7dde2")),
    map: texture?.texture ?? null,
    metalness: spec?.metalness ?? 0,
    opacity,
    roughness: spec?.roughness ?? 0.85,
  });

  if (mode === "mask") {
    if (spec?.alphaCoverage === "dither") {
      material.alphaHash = true;
    } else {
      material.alphaTest = spec?.alphaCutoff ?? 0.1;
    }
  } else if (mode === "blend") {
    material.depthFunc = THREE.EqualDepth;
    material.depthWrite = false;
    material.premultipliedAlpha = true;
    material.transparent = true;
  } else if (mode === "additive") {
    material.blending = THREE.AdditiveBlending;
    material.depthWrite = false;
    material.transparent = true;
  }
  material.needsUpdate = true;

  return {
    color: material,
    ...(mode === "blend" ? { depth: blendDepthMaterial(material, texture, opacity) } : {}),
    mode,
  };
}

function blendDepthMaterial(
  color: THREE.MeshStandardMaterial,
  texture: PreparedAsciiTexture | undefined,
  opacity: number,
): THREE.Material {
  return new THREE.MeshBasicMaterial({
    alphaTest: 0.0001,
    colorWrite: false,
    depthTest: true,
    depthWrite: true,
    map: texture?.texture ?? null,
    opacity,
    side: color.side,
  });
}

function hiddenMaterial(): THREE.Material {
  const material = new THREE.MeshBasicMaterial();
  material.visible = false;
  return material;
}

function mapPreparedMaterials(
  materials: PreparedThreeMaterial | PreparedThreeMaterial[],
  select: (material: PreparedThreeMaterial) => THREE.Material,
): THREE.Material | THREE.Material[] {
  return Array.isArray(materials) ? materials.map(select) : select(materials);
}

function flattenPreparedMaterials(
  materials: PreparedThreeMaterial | PreparedThreeMaterial[],
): PreparedThreeMaterial[] {
  return Array.isArray(materials) ? materials : [materials];
}

function asciiTextureToCanvasTexture(
  palette: Record<string, string>,
  pixels: string[],
): PreparedAsciiTexture {
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
      const color = palette[row[x] ?? "."] ?? "#ff00ff";
      if (color === TRANSPARENT_PALETTE_COLOR) {
        context.clearRect(x, y, 1, 1);
      } else {
        context.fillStyle = color;
        context.fillRect(x, y, 1, 1);
      }
    }
  }

  const texture = new THREE.CanvasTexture(canvas);
  texture.magFilter = THREE.NearestFilter;
  texture.minFilter = THREE.NearestFilter;
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.flipY = true;
  return {
    hasTransparency: asciiTextureHasTransparency({ palette, pixels }),
    texture,
  };
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

export function disposeObjectTree(root: THREE.Object3D): void {
  disposeObjectResources(root, [], []);
}

function disposeObjectResources(
  root: THREE.Object3D,
  extraMaterials: Iterable<THREE.Material>,
  extraTextures: Iterable<THREE.Texture>,
): void {
  const geometries = new Set<THREE.BufferGeometry>();
  const materials = new Set<THREE.Material>(extraMaterials);
  const textures = new Set<THREE.Texture>(extraTextures);

  root.traverse((object) => {
    const renderable = object as THREE.Object3D & {
      geometry?: THREE.BufferGeometry;
      material?: THREE.Material | THREE.Material[];
    };
    if (renderable.geometry) {
      geometries.add(renderable.geometry);
    }
    if (Array.isArray(renderable.material)) {
      for (const material of renderable.material) {
        materials.add(material);
      }
    } else if (renderable.material) {
      materials.add(renderable.material);
    }
  });

  for (const material of materials) {
    collectMaterialTextures(material, textures);
  }
  for (const geometry of geometries) {
    geometry.dispose();
  }
  for (const material of materials) {
    material.dispose();
  }
  for (const texture of textures) {
    texture.dispose();
  }
}

function collectMaterialTextures(material: THREE.Material, textures: Set<THREE.Texture>): void {
  const materialWithMaps = material as THREE.Material & Record<string, unknown>;
  for (const property of [
    "alphaMap",
    "aoMap",
    "bumpMap",
    "displacementMap",
    "emissiveMap",
    "envMap",
    "lightMap",
    "map",
    "metalnessMap",
    "normalMap",
    "roughnessMap",
  ]) {
    const value = materialWithMaps[property];
    if (value instanceof THREE.Texture) {
      textures.add(value);
    }
  }
}

function prepareClip(clip: ClipSpec): PreparedClip {
  const keysByPart = new Map<string, ClipSpec["keys"]>();
  for (const key of clip.keys) {
    const keys = keysByPart.get(key[0]) ?? [];
    keys.push(key);
    keysByPart.set(key[0], keys);
  }
  for (const keys of keysByPart.values()) {
    keys.sort((left, right) => left[1] - right[1]);
  }
  return { source: clip, keysByPart };
}

function requiredClip(
  clips: Map<string, PreparedClip>,
  clipName: string,
  figureName: string,
): PreparedClip {
  const clip = clips.get(clipName);
  if (!clip) {
    throw new Error(`Figure '${figureName}' has no clip '${clipName}'`);
  }
  return clip;
}

function applyClip(parts: Map<string, PartObject>, prepared: PreparedClip, timeSeconds: number): void {
  if (!Number.isFinite(timeSeconds)) {
    throw new Error("Animation presentation time must be finite");
  }
  const clip = prepared.source;
  const duration = clipDuration(clip);
  const localTime = clip.loop && duration > 0
    ? ((timeSeconds % duration) + duration) % duration
    : THREE.MathUtils.clamp(timeSeconds, 0, duration);

  for (const [partName, keys] of prepared.keysByPart.entries()) {
    const part = parts.get(partName);
    if (!part) {
      continue;
    }
    const translation = sampleChannel(keys, localTime, (key) => key[2].at);
    if (translation) {
      part.group.position.copy(part.basePosition).add(toVector(translation));
    }

    const rotation = channelSpan(keys, localTime, (key) => key[2].rot);
    if (rotation) {
      const left = additiveRotation(part.baseRotation, rotation.left);
      const right = additiveRotation(part.baseRotation, rotation.right);
      part.group.quaternion.slerpQuaternions(left, right, rotation.alpha).normalize();
    }

    const scale = sampleChannel(keys, localTime, (key) => key[2].scale);
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
