export type Vec3 = readonly [number, number, number];
export type EulerDeg = Vec3;

export interface FigureAsset {
  schemaVersion: 1;
  name: string;
  materials: Record<string, MaterialSpec>;
  textures: Record<string, AsciiTextureSpec>;
  parts: PartSpec[];
  clips: Record<string, ClipSpec>;
}

export interface MaterialSpec {
  color: string;
  roughness?: number;
  metalness?: number;
}

export interface AsciiTextureSpec {
  palette: Record<string, string>;
  pixels: string[];
}

export type BoxFaceName = "north" | "south" | "east" | "west" | "up" | "down";

export const BOX_FACE_NAMES: readonly BoxFaceName[] = ["north", "south", "east", "west", "up", "down"];

export interface FaceSpec {
  material?: string;
  texture?: string;
}

export type BoxFaceMap = Partial<Record<BoxFaceName, FaceSpec>>;

export type PrimitiveSpec =
  | { kind: "box"; size: Vec3; faces?: BoxFaceMap }
  | { kind: "sphere"; radius: number; widthSegments?: number; heightSegments?: number }
  | { kind: "capsule"; radius: number; length: number; capSegments?: number; radialSegments?: number }
  | { kind: "cylinder"; radiusTop: number; radiusBottom: number; length: number; radialSegments?: number };

export interface JointSpec {
  pivot?: Vec3;
  axis?: Vec3;
}

export interface PartSpec {
  name: string;
  parent?: string;
  at?: Vec3;
  rot?: EulerDeg;
  pivot?: Vec3;
  material?: string;
  texture?: string;
  joint?: JointSpec;
  primitive: PrimitiveSpec;
}

export interface PartOptions {
  parent?: string;
  at?: Vec3;
  rot?: EulerDeg;
  pivot?: Vec3;
  material?: string;
  texture?: string;
  joint?: JointSpec;
}

export interface BoxOptions extends PartOptions {
  size: Vec3;
  faces?: BoxFaceMap;
}

export interface SphereOptions extends PartOptions {
  radius: number;
  widthSegments?: number;
  heightSegments?: number;
}

export interface CapsuleOptions extends PartOptions {
  radius: number;
  length: number;
  capSegments?: number;
  radialSegments?: number;
}

export interface CylinderOptions extends PartOptions {
  radiusTop?: number;
  radiusBottom?: number;
  radius?: number;
  length: number;
  radialSegments?: number;
}

export type PartDraft = Omit<PartSpec, "name">;

export interface TransformKey {
  at?: Vec3;
  rot?: EulerDeg;
  scale?: Vec3;
}

export type ClipKey = readonly [part: string, time: number, transform: TransformKey];

export interface ClipSpec {
  fps?: number;
  loop?: boolean;
  keys: ClipKey[];
}

export interface FigureApi {
  mat(name: string, colorOrSpec: string | MaterialSpec): void;
  asciiTexture(name: string, texture: AsciiTextureSpec): void;
  part(name: string, draft: PartDraft): void;
  clip(name: string, spec: ClipSpec): void;
  box(options: BoxOptions): PartDraft;
  sphere(options: SphereOptions): PartDraft;
  capsule(options: CapsuleOptions): PartDraft;
  cylinder(options: CylinderOptions): PartDraft;
}

export function figure(name: string, build: (api: FigureApi) => void): FigureAsset {
  const builder = new FigureBuilder(name);
  build(builder.api);
  const asset = builder.finish();
  assertValidFigure(asset);
  return asset;
}

export function assertValidFigure(asset: FigureAsset): void {
  const errors = validateFigure(asset);
  if (errors.length > 0) {
    throw new Error(`Invalid figure '${asset.name}':\n${errors.map((error) => `- ${error}`).join("\n")}`);
  }
}

export function validateFigure(asset: FigureAsset): string[] {
  const errors: string[] = [];
  const partNames = new Set(asset.parts.map((part) => part.name));

  if (!asset.name.trim()) {
    errors.push("figure name is required");
  }

  for (const [name, material] of Object.entries(asset.materials)) {
    if (!isHexColor(material.color)) {
      errors.push(`material '${name}' has invalid color '${material.color}'`);
    }
  }

  for (const [name, texture] of Object.entries(asset.textures)) {
    validateAsciiTexture(name, texture, errors);
  }

  for (const part of asset.parts) {
    if (!part.name.trim()) {
      errors.push("part name is required");
    }
    if (part.parent && !partNames.has(part.parent)) {
      errors.push(`part '${part.name}' references missing parent '${part.parent}'`);
    }
    if (part.parent === part.name) {
      errors.push(`part '${part.name}' cannot parent itself`);
    }
    if (part.material && !asset.materials[part.material]) {
      errors.push(`part '${part.name}' references missing material '${part.material}'`);
    }
    if (part.texture && !asset.textures[part.texture]) {
      errors.push(`part '${part.name}' references missing texture '${part.texture}'`);
    }
    if (part.primitive.kind === "box" && part.primitive.faces) {
      validateBoxFaces(part, asset, errors);
    }
    validatePrimitive(part, errors);
  }

  for (const [clipName, clip] of Object.entries(asset.clips)) {
    for (const [partName, time] of clip.keys) {
      if (!partNames.has(partName)) {
        errors.push(`clip '${clipName}' references missing part '${partName}'`);
      }
      if (!Number.isFinite(time) || time < 0) {
        errors.push(`clip '${clipName}' has invalid key time '${time}'`);
      }
    }
  }

  return errors;
}

class FigureBuilder {
  private readonly materials: Record<string, MaterialSpec> = {};
  private readonly textures: Record<string, AsciiTextureSpec> = {};
  private readonly parts: PartSpec[] = [];
  private readonly clips: Record<string, ClipSpec> = {};
  readonly api: FigureApi;

  constructor(private readonly name: string) {
    this.api = {
      mat: (name, colorOrSpec) => this.mat(name, colorOrSpec),
      asciiTexture: (name, texture) => this.asciiTexture(name, texture),
      part: (name, draft) => this.part(name, draft),
      clip: (name, spec) => this.clip(name, spec),
      box,
      sphere,
      capsule,
      cylinder,
    };
  }

  finish(): FigureAsset {
    return {
      schemaVersion: 1,
      name: this.name,
      materials: this.materials,
      textures: this.textures,
      parts: this.parts,
      clips: this.clips,
    };
  }

  private mat(name: string, colorOrSpec: string | MaterialSpec): void {
    this.materials[name] = typeof colorOrSpec === "string" ? { color: colorOrSpec } : colorOrSpec;
  }

  private asciiTexture(name: string, texture: AsciiTextureSpec): void {
    this.textures[name] = texture;
  }

  private part(name: string, draft: PartDraft): void {
    if (this.parts.some((part) => part.name === name)) {
      throw new Error(`Duplicate part '${name}'`);
    }
    this.parts.push({ name, ...draft });
  }

  private clip(name: string, spec: ClipSpec): void {
    this.clips[name] = spec;
  }
}

function box(options: BoxOptions): PartDraft {
  const { size, faces, ...rest } = options;
  const primitive: PrimitiveSpec = { kind: "box", size };
  if (faces) {
    primitive.faces = faces;
  }
  return { ...rest, primitive };
}

function sphere(options: SphereOptions): PartDraft {
  const { radius, widthSegments, heightSegments, ...rest } = options;
  const primitive: PrimitiveSpec = { kind: "sphere", radius };
  if (widthSegments !== undefined) {
    primitive.widthSegments = widthSegments;
  }
  if (heightSegments !== undefined) {
    primitive.heightSegments = heightSegments;
  }
  return { ...rest, primitive };
}

function capsule(options: CapsuleOptions): PartDraft {
  const { radius, length, capSegments, radialSegments, ...rest } = options;
  const primitive: PrimitiveSpec = { kind: "capsule", radius, length };
  if (capSegments !== undefined) {
    primitive.capSegments = capSegments;
  }
  if (radialSegments !== undefined) {
    primitive.radialSegments = radialSegments;
  }
  return { ...rest, primitive };
}

function cylinder(options: CylinderOptions): PartDraft {
  const { radiusTop, radiusBottom, radius, length, radialSegments, ...rest } = options;
  const primitive: PrimitiveSpec = {
    kind: "cylinder",
    radiusTop: radiusTop ?? radius ?? 0.5,
    radiusBottom: radiusBottom ?? radius ?? 0.5,
    length,
  };
  if (radialSegments !== undefined) {
    primitive.radialSegments = radialSegments;
  }
  return { ...rest, primitive };
}

function validateAsciiTexture(name: string, texture: AsciiTextureSpec, errors: string[]): void {
  if (texture.pixels.length === 0) {
    errors.push(`texture '${name}' has no pixels`);
    return;
  }
  const width = texture.pixels[0]?.length ?? 0;
  if (width === 0) {
    errors.push(`texture '${name}' has an empty first row`);
  }
  for (const [rowIndex, row] of texture.pixels.entries()) {
    if (row.length !== width) {
      errors.push(`texture '${name}' row ${rowIndex} has width ${row.length}, expected ${width}`);
    }
    for (const char of row) {
      if (!texture.palette[char]) {
        errors.push(`texture '${name}' uses palette character '${char}' without a color`);
      }
    }
  }
  for (const [char, color] of Object.entries(texture.palette)) {
    if (char.length !== 1) {
      errors.push(`texture '${name}' palette key '${char}' must be one character`);
    }
    if (!isHexColor(color)) {
      errors.push(`texture '${name}' palette '${char}' has invalid color '${color}'`);
    }
  }
}

function validatePrimitive(part: PartSpec, errors: string[]): void {
  const primitive = part.primitive;
  if (primitive.kind === "box") {
    validatePositiveVec(`part '${part.name}' box size`, primitive.size, errors);
  } else if (primitive.kind === "sphere") {
    validatePositive(`part '${part.name}' sphere radius`, primitive.radius, errors);
  } else if (primitive.kind === "capsule") {
    validatePositive(`part '${part.name}' capsule radius`, primitive.radius, errors);
    validatePositive(`part '${part.name}' capsule length`, primitive.length, errors);
  } else if (primitive.kind === "cylinder") {
    validatePositive(`part '${part.name}' cylinder radiusTop`, primitive.radiusTop, errors);
    validatePositive(`part '${part.name}' cylinder radiusBottom`, primitive.radiusBottom, errors);
    validatePositive(`part '${part.name}' cylinder length`, primitive.length, errors);
  }
}

function validateBoxFaces(part: PartSpec, asset: FigureAsset, errors: string[]): void {
  if (part.primitive.kind !== "box" || !part.primitive.faces) {
    return;
  }
  for (const [faceName, face] of Object.entries(part.primitive.faces)) {
    if (!BOX_FACE_NAMES.includes(faceName as BoxFaceName)) {
      errors.push(`part '${part.name}' references unknown box face '${faceName}'`);
    }
    if (face.material && !asset.materials[face.material]) {
      errors.push(`part '${part.name}' face '${faceName}' references missing material '${face.material}'`);
    }
    if (face.texture && !asset.textures[face.texture]) {
      errors.push(`part '${part.name}' face '${faceName}' references missing texture '${face.texture}'`);
    }
  }
}

function validatePositiveVec(label: string, value: Vec3, errors: string[]): void {
  for (const [index, item] of value.entries()) {
    validatePositive(`${label}[${index}]`, item, errors);
  }
}

function validatePositive(label: string, value: number, errors: string[]): void {
  if (!Number.isFinite(value) || value <= 0) {
    errors.push(`${label} must be positive`);
  }
}

function isHexColor(value: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(value);
}
