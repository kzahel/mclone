import * as THREE from "three";
import { assertFigureGeometry } from "./geometry-analysis";

export type Vec3 = readonly [number, number, number];
export type EulerDeg = Vec3;

export interface FigureAsset {
  schemaVersion: 1;
  name: string;
  defaultClip?: string;
  geometryExceptions?: GeometryExceptionSpec[];
  materials: Record<string, MaterialSpec>;
  textures: Record<string, AsciiTextureSpec>;
  parts: PartSpec[];
  clips: Record<string, ClipSpec>;
}

export interface GeometryExceptionSpec {
  rule: "disconnected-component";
  parts: string[];
  reason: string;
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

/** @deprecated Legacy rounded-comparison input only. Use `box`. */
export interface SphereOptions extends PartOptions {
  radius: number;
  widthSegments?: number;
  heightSegments?: number;
}

/** @deprecated Legacy rounded-comparison input only. Use `box`. */
export interface CapsuleOptions extends PartOptions {
  radius: number;
  length: number;
  capSegments?: number;
  radialSegments?: number;
}

/** @deprecated Legacy rounded-comparison input only. Use `box`. */
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

export type LocomotionKind = "biped-walk" | "quadruped-walk" | "slither" | "swim" | "wing-flap";
export type ClipRole = "locomotion" | "idle" | "action";

export interface ClipPresentationSpec {
  label?: string;
  role?: ClipRole;
  nextClip?: string;
}

export interface LocomotionContactSpec {
  part: string;
  phaseStart: number;
  phaseEnd: number;
  role?: string;
  stanceRatio: number;
}

export interface ClipLocomotionSpec {
  kind: LocomotionKind;
  cycleDistance: number;
  contacts?: LocomotionContactSpec[];
  direction?: Vec3;
  speed?: number;
  units?: "figure";
}

export interface ClipSpec extends ClipPresentationSpec {
  fps?: number;
  loop?: boolean;
  locomotion?: ClipLocomotionSpec;
  keys: ClipKey[];
}

export type AxisName = "x" | "y" | "z";

export interface CycleValueConstraint {
  /** Lower bound applied after evaluating the procedural waveform. */
  min?: number;
  /** Upper bound applied after evaluating the procedural waveform. */
  max?: number;
}

export interface SwingOptions extends CycleValueConstraint {
  axis?: AxisName;
  degrees: number;
  center?: number;
  frequency?: number;
  phase?: number;
}

export interface BobOptions extends CycleValueConstraint {
  axis?: AxisName;
  amount: number;
  center?: number;
  frequency?: number;
  phase?: number;
}

export interface ContactSwingOptions extends SwingOptions {
  stanceRatio: number;
}

export interface FollowThroughOptions extends CycleValueConstraint {
  source: string;
  sourceChannel?: "rot" | "pos";
  sourceAxis?: AxisName;
  axis?: AxisName;
  degrees: number;
  lag?: number;
  overshoot?: number;
  center?: number;
}

export type CycleTrack =
  | ({ kind: "swing"; part: string } & SwingOptions)
  | ({ kind: "contactSwing"; part: string } & ContactSwingOptions)
  | ({ kind: "bob"; part: string } & BobOptions)
  | ({ kind: "followThrough"; part: string } & FollowThroughOptions);

export interface WalkCycleSpec extends ClipPresentationSpec {
  duration?: number;
  fps?: number;
  groundContacts?: GroundContactConstraint[];
  locomotion?: ClipLocomotionSpec;
  loop?: boolean;
  samples?: number;
  tracks: CycleTrack[];
}

export interface GroundContactConstraint {
  /** Box whose transformed lower surface may not pass below groundY. */
  contactPart: string;
  /** Ancestor hinge whose animated rotation is corrected after track sampling. */
  solvePart: string;
  axis?: AxisName;
  groundY?: number;
  /** Bounds on the correction added to the sampled solve-part angle. */
  minCorrectionDegrees?: number;
  maxCorrectionDegrees?: number;
  /** Counter-rotate the contact box so its sampled pad orientation is retained. */
  preserveContactRotation?: boolean;
}

export interface CycleTimingSpec extends ClipPresentationSpec {
  duration?: number;
  fps?: number;
  loop?: boolean;
  samples?: number;
}

export type QuadrupedGait = "trot" | "walk" | "pace" | "bound";

export interface QuadrupedLegs {
  frontLeft: string;
  frontRight: string;
  backLeft: string;
  backRight: string;
}

export interface QuadrupedWalkSpec extends CycleTimingSpec {
  body?: string;
  bodyBob?: number;
  bodyBobCenter?: number;
  bodyBobPhase?: number;
  contactParts?: Partial<QuadrupedLegs>;
  cycleDistance?: number;
  direction?: Vec3;
  gait?: QuadrupedGait;
  head?: string;
  headSwingDegrees?: number;
  legAxis?: AxisName;
  legs: QuadrupedLegs;
  stanceRatio?: number;
  swingDegrees?: number;
  tail?: string;
  tailSwingDegrees?: number;
  tracks?: CycleTrack[];
}

export interface BipedWalkSpec extends CycleTimingSpec {
  armAxis?: AxisName;
  armSwingDegrees?: number;
  body?: string;
  bodyBob?: number;
  bodyBobCenter?: number;
  bodyBobPhase?: number;
  cycleDistance?: number;
  direction?: Vec3;
  head?: string;
  headSwingDegrees?: number;
  leftArm?: string;
  leftContact?: string;
  leftLeg: string;
  legAxis?: AxisName;
  rightContact?: string;
  rightArm?: string;
  rightLeg: string;
  stanceRatio?: number;
  swingDegrees?: number;
  tracks?: CycleTrack[];
}

export interface WingFlapSpec extends CycleTimingSpec {
  axis?: AxisName;
  body?: string;
  bodyBob?: number;
  bodyBobCenter?: number;
  bodyBobPhase?: number;
  center?: number;
  cycleDistance?: number;
  direction?: Vec3;
  degrees?: number;
  frequency?: number;
  leftWing: string;
  mirror?: boolean;
  phase?: number;
  rightWing: string;
  tracks?: CycleTrack[];
}

export interface SwimSpec extends CycleTimingSpec {
  body?: string;
  bodyBob?: number;
  bodyBobCenter?: number;
  bodyBobPhase?: number;
  bodySwayDegrees?: number;
  cycleDistance?: number;
  direction?: Vec3;
  finAxis?: AxisName;
  finPhase?: number;
  finSwingDegrees?: number;
  frequency?: number;
  leftFin?: string;
  rightFin?: string;
  tail: string;
  tailAxis?: AxisName;
  tailSwingDegrees?: number;
  tailTip?: string;
  tailTipPhase?: number;
  tailTipSwingDegrees?: number;
  tracks?: CycleTrack[];
}

export interface SlitherSpec extends CycleTimingSpec {
  axis?: AxisName;
  body?: string;
  bodyBob?: number;
  bodyBobCenter?: number;
  bodyBobPhase?: number;
  cycleDistance?: number;
  degrees?: number;
  direction?: Vec3;
  frequency?: number;
  phaseStep?: number;
  segments: readonly string[];
  tracks?: CycleTrack[];
}

export interface FigureApi {
  defaultClip(name: string): void;
  geometryException(spec: GeometryExceptionSpec): void;
  mat(name: string, colorOrSpec: string | MaterialSpec): void;
  asciiTexture(name: string, texture: AsciiTextureSpec): void;
  part(name: string, draft: PartDraft): void;
  clip(name: string, spec: ClipSpec): void;
  walkCycle(name: string, spec: WalkCycleSpec): void;
  bipedWalk(name: string, spec: BipedWalkSpec): void;
  quadrupedWalk(name: string, spec: QuadrupedWalkSpec): void;
  slither(name: string, spec: SlitherSpec): void;
  swim(name: string, spec: SwimSpec): void;
  wingFlap(name: string, spec: WingFlapSpec): void;
  swing(part: string, options: SwingOptions): CycleTrack;
  contactSwing(part: string, options: ContactSwingOptions): CycleTrack;
  bob(part: string, options: BobOptions): CycleTrack;
  followThrough(part: string, options: FollowThroughOptions): CycleTrack;
  box(options: BoxOptions): PartDraft;
}

/**
 * Compatibility API for retained rounded A/B sources.
 *
 * @deprecated Canonical and promoted figures must use `figure()` and boxes.
 */
export interface LegacyFigureApi extends FigureApi {
  /** @deprecated Legacy rounded-comparison input only. Use `box`. */
  sphere(options: SphereOptions): PartDraft;
  /** @deprecated Legacy rounded-comparison input only. Use `box`. */
  capsule(options: CapsuleOptions): PartDraft;
  /** @deprecated Legacy rounded-comparison input only. Use `box`. */
  cylinder(options: CylinderOptions): PartDraft;
}

export function figure(name: string, build: (api: FigureApi) => void): FigureAsset {
  const asset = buildFigureAsset(name, build);
  assertBoxOnlyFigure(asset);
  assertFigureGeometry(asset);
  return asset;
}

/**
 * Builds a retained rounded comparison source without making it canonical.
 *
 * @deprecated Canonical and promoted figures must use `figure()` and boxes.
 */
export function legacyFigure(
  name: string,
  build: (api: LegacyFigureApi) => void,
): FigureAsset {
  return buildFigureAsset(name, build);
}

function buildFigureAsset(
  name: string,
  build: (api: LegacyFigureApi) => void,
): FigureAsset {
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

export function assertBoxOnlyFigure(asset: FigureAsset): void {
  const errors = validateBoxOnlyFigure(asset);
  if (errors.length > 0) {
    throw new Error(
      `Invalid canonical figure '${asset.name}':\n${errors.map((error) => `- ${error}`).join("\n")}`,
    );
  }
}

export function validateBoxOnlyFigure(asset: FigureAsset): string[] {
  return asset.parts
    .filter((part) => part.primitive.kind !== "box")
    .map(
      (part) =>
        `part '${part.name}' uses deprecated '${part.primitive.kind}'; canonical figures may use only box primitives`,
    );
}

export function validateFigure(asset: FigureAsset): string[] {
  const errors: string[] = [];
  const partNames = new Set(asset.parts.map((part) => part.name));
  const clipNames = new Set(Object.keys(asset.clips));

  if (!asset.name.trim()) {
    errors.push("figure name is required");
  }
  if (asset.defaultClip !== undefined && !clipNames.has(asset.defaultClip)) {
    errors.push(`default clip references missing clip '${asset.defaultClip}'`);
  }
  for (const [index, exception] of (asset.geometryExceptions ?? []).entries()) {
    if (exception.rule !== "disconnected-component") {
      errors.push(`geometry exception ${index} uses unknown rule '${String(exception.rule)}'`);
    }
    if (!Array.isArray(exception.parts) || exception.parts.length === 0) {
      errors.push(`geometry exception ${index} must name at least one part`);
    } else {
      const uniqueParts = new Set(exception.parts);
      if (uniqueParts.size !== exception.parts.length) {
        errors.push(`geometry exception ${index} contains duplicate part names`);
      }
      for (const partName of exception.parts) {
        if (!partNames.has(partName)) {
          errors.push(`geometry exception ${index} references missing part '${partName}'`);
        }
      }
    }
    if (typeof exception.reason !== "string" || !exception.reason.trim()) {
      errors.push(`geometry exception ${index} requires a nonempty reason`);
    }
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
    if (part.at !== undefined) {
      validateFiniteVec(`part '${part.name}' at`, part.at, errors);
    }
    if (part.rot !== undefined) {
      validateFiniteVec(`part '${part.name}' rot`, part.rot, errors);
    }
    if (part.pivot !== undefined) {
      validateFiniteVec(`part '${part.name}' pivot`, part.pivot, errors);
    }
    if (part.joint?.pivot !== undefined) {
      validateFiniteVec(`part '${part.name}' joint pivot`, part.joint.pivot, errors);
    }
    if (part.joint?.axis !== undefined) {
      validateFiniteVec(`part '${part.name}' joint axis`, part.joint.axis, errors);
    }
    validatePrimitive(part, errors);
  }

  for (const [clipName, clip] of Object.entries(asset.clips)) {
    if (clip.label !== undefined && (typeof clip.label !== "string" || !clip.label.trim())) {
      errors.push(`clip '${clipName}' label must be a nonempty string`);
    }
    if (clip.role !== undefined && !["locomotion", "idle", "action"].includes(clip.role)) {
      errors.push(`clip '${clipName}' role '${clip.role}' is invalid`);
    }
    if (clip.nextClip !== undefined) {
      if (!clipNames.has(clip.nextClip)) {
        errors.push(`clip '${clipName}' nextClip references missing clip '${clip.nextClip}'`);
      }
      if (clip.loop === true) {
        errors.push(`looping clip '${clipName}' cannot declare nextClip`);
      }
    }
    if (clip.role === "locomotion" && clip.locomotion === undefined) {
      errors.push(`locomotion clip '${clipName}' requires locomotion metadata`);
    }
    if (clip.role !== undefined && clip.role !== "locomotion" && clip.locomotion !== undefined) {
      errors.push(`clip '${clipName}' with locomotion metadata must use role 'locomotion'`);
    }
    for (const [partName, time, transform] of clip.keys) {
      if (!partNames.has(partName)) {
        errors.push(`clip '${clipName}' references missing part '${partName}'`);
      }
      if (!Number.isFinite(time) || time < 0) {
        errors.push(`clip '${clipName}' has invalid key time '${time}'`);
      }
      if (transform.at !== undefined) {
        validateFiniteVec(`clip '${clipName}' part '${partName}' at`, transform.at, errors);
      }
      if (transform.rot !== undefined) {
        validateFiniteVec(`clip '${clipName}' part '${partName}' rot`, transform.rot, errors);
      }
      if (transform.scale !== undefined) {
        validateFiniteVec(`clip '${clipName}' part '${partName}' scale`, transform.scale, errors);
      }
    }
    if (clip.locomotion) {
      validateLocomotion(clipName, clip.locomotion, partNames, errors);
    }
  }

  return errors;
}

class FigureBuilder {
  private readonly materials: Record<string, MaterialSpec> = {};
  private readonly textures: Record<string, AsciiTextureSpec> = {};
  private readonly parts: PartSpec[] = [];
  private readonly clips: Record<string, ClipSpec> = {};
  private readonly geometryExceptions: GeometryExceptionSpec[] = [];
  private defaultClipName: string | undefined;
  readonly api: LegacyFigureApi;

  constructor(private readonly name: string) {
    this.api = {
      defaultClip: (name) => this.defaultClip(name),
      geometryException: (spec) => this.geometryException(spec),
      mat: (name, colorOrSpec) => this.mat(name, colorOrSpec),
      asciiTexture: (name, texture) => this.asciiTexture(name, texture),
      part: (name, draft) => this.part(name, draft),
      clip: (name, spec) => this.clip(name, spec),
      walkCycle: (name, spec) => this.clip(name, buildWalkCycleClip(spec, this.parts)),
      bipedWalk: (name, spec) => this.clip(name, buildWalkCycleClip(buildBipedWalkCycle(spec), this.parts)),
      quadrupedWalk: (name, spec) => this.clip(name, buildWalkCycleClip(buildQuadrupedWalkCycle(spec), this.parts)),
      slither: (name, spec) => this.clip(name, buildWalkCycleClip(buildSlitherCycle(spec), this.parts)),
      swim: (name, spec) => this.clip(name, buildWalkCycleClip(buildSwimCycle(spec), this.parts)),
      wingFlap: (name, spec) => this.clip(name, buildWalkCycleClip(buildWingFlapCycle(spec), this.parts)),
      swing,
      contactSwing,
      bob,
      followThrough,
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
      ...(this.defaultClipName === undefined ? {} : { defaultClip: this.defaultClipName }),
      ...(this.geometryExceptions.length === 0
        ? {}
        : { geometryExceptions: this.geometryExceptions }),
      materials: this.materials,
      textures: this.textures,
      parts: this.parts,
      clips: this.clips,
    };
  }

  private defaultClip(name: string): void {
    if (this.defaultClipName !== undefined && this.defaultClipName !== name) {
      throw new Error(`Figure '${this.name}' already declares default clip '${this.defaultClipName}'`);
    }
    this.defaultClipName = name;
  }

  private geometryException(spec: GeometryExceptionSpec): void {
    this.geometryExceptions.push({
      rule: spec.rule,
      parts: [...spec.parts],
      reason: spec.reason,
    });
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

function swing(part: string, options: SwingOptions): CycleTrack {
  const track: Extract<CycleTrack, { kind: "swing" }> = { kind: "swing", part, degrees: options.degrees };
  if (options.axis !== undefined) {
    track.axis = options.axis;
  }
  if (options.center !== undefined) {
    track.center = options.center;
  }
  if (options.frequency !== undefined) {
    track.frequency = options.frequency;
  }
  if (options.phase !== undefined) {
    track.phase = options.phase;
  }
  copyCycleValueConstraint(track, options);
  return track;
}

function bob(part: string, options: BobOptions): CycleTrack {
  const track: Extract<CycleTrack, { kind: "bob" }> = { kind: "bob", part, amount: options.amount };
  if (options.axis !== undefined) {
    track.axis = options.axis;
  }
  if (options.center !== undefined) {
    track.center = options.center;
  }
  if (options.frequency !== undefined) {
    track.frequency = options.frequency;
  }
  if (options.phase !== undefined) {
    track.phase = options.phase;
  }
  copyCycleValueConstraint(track, options);
  return track;
}

function contactSwing(part: string, options: ContactSwingOptions): CycleTrack {
  const track: Extract<CycleTrack, { kind: "contactSwing" }> = {
    kind: "contactSwing",
    part,
    degrees: options.degrees,
    stanceRatio: options.stanceRatio,
  };
  if (options.axis !== undefined) {
    track.axis = options.axis;
  }
  if (options.center !== undefined) {
    track.center = options.center;
  }
  if (options.frequency !== undefined) {
    track.frequency = options.frequency;
  }
  if (options.phase !== undefined) {
    track.phase = options.phase;
  }
  copyCycleValueConstraint(track, options);
  return track;
}

function followThrough(part: string, options: FollowThroughOptions): CycleTrack {
  const track: Extract<CycleTrack, { kind: "followThrough" }> = {
    kind: "followThrough",
    part,
    source: options.source,
    degrees: options.degrees,
  };
  if (options.sourceChannel !== undefined) {
    track.sourceChannel = options.sourceChannel;
  }
  if (options.sourceAxis !== undefined) {
    track.sourceAxis = options.sourceAxis;
  }
  if (options.axis !== undefined) {
    track.axis = options.axis;
  }
  if (options.lag !== undefined) {
    track.lag = options.lag;
  }
  if (options.overshoot !== undefined) {
    track.overshoot = options.overshoot;
  }
  if (options.center !== undefined) {
    track.center = options.center;
  }
  copyCycleValueConstraint(track, options);
  return track;
}

function copyCycleValueConstraint(
  track: CycleValueConstraint,
  options: CycleValueConstraint,
): void {
  if (options.min !== undefined) {
    track.min = options.min;
  }
  if (options.max !== undefined) {
    track.max = options.max;
  }
}

function buildQuadrupedWalkCycle(spec: QuadrupedWalkSpec): WalkCycleSpec {
  const swingDegrees = spec.swingDegrees ?? 18;
  const legAxis = spec.legAxis ?? "x";
  const gait = spec.gait ?? "trot";
  const phases = quadrupedPhases(gait);
  const stanceRatio = spec.stanceRatio ?? defaultQuadrupedStanceRatio(gait);
  const tracks: CycleTrack[] = [
    contactSwing(spec.legs.frontLeft, {
      axis: legAxis,
      degrees: swingDegrees,
      phase: phases.frontLeft,
      stanceRatio,
    }),
    contactSwing(spec.legs.frontRight, {
      axis: legAxis,
      degrees: swingDegrees,
      phase: phases.frontRight,
      stanceRatio,
    }),
    contactSwing(spec.legs.backLeft, {
      axis: legAxis,
      degrees: swingDegrees,
      phase: phases.backLeft,
      stanceRatio,
    }),
    contactSwing(spec.legs.backRight, {
      axis: legAxis,
      degrees: swingDegrees,
      phase: phases.backRight,
      stanceRatio,
    }),
  ];

  if (spec.body && spec.bodyBob !== 0) {
    const amount = spec.bodyBob ?? 0.012;
    tracks.push(bob(spec.body, {
      axis: "y",
      amount,
      center: spec.bodyBobCenter ?? amount,
      phase: spec.bodyBobPhase ?? 0.5,
    }));
  }
  if (spec.head && spec.headSwingDegrees !== 0) {
    tracks.push(swing(spec.head, {
      axis: "y",
      degrees: spec.headSwingDegrees ?? 3,
      phase: 0.5,
    }));
  }
  if (spec.tail && spec.tailSwingDegrees !== 0) {
    tracks.push(swing(spec.tail, {
      axis: "z",
      degrees: spec.tailSwingDegrees ?? 10,
      phase: 0.5,
    }));
  }
  tracks.push(...(spec.tracks ?? []));

  return cycleSpecFromTiming(spec, tracks, {
    kind: "quadruped-walk",
    cycleDistance: spec.cycleDistance ?? 0.72,
    contacts: [
      contactSpec(
        spec.contactParts?.frontLeft ?? spec.legs.frontLeft,
        "front-left",
        phases.frontLeft,
        stanceRatio,
      ),
      contactSpec(
        spec.contactParts?.frontRight ?? spec.legs.frontRight,
        "front-right",
        phases.frontRight,
        stanceRatio,
      ),
      contactSpec(
        spec.contactParts?.backLeft ?? spec.legs.backLeft,
        "back-left",
        phases.backLeft,
        stanceRatio,
      ),
      contactSpec(
        spec.contactParts?.backRight ?? spec.legs.backRight,
        "back-right",
        phases.backRight,
        stanceRatio,
      ),
    ],
    direction: spec.direction ?? [0, 0, -1],
    units: "figure",
  });
}

function buildBipedWalkCycle(spec: BipedWalkSpec): WalkCycleSpec {
  const swingDegrees = spec.swingDegrees ?? 24;
  const armSwingDegrees = spec.armSwingDegrees ?? swingDegrees * 0.7;
  const legAxis = spec.legAxis ?? "x";
  const armAxis = spec.armAxis ?? legAxis;
  const stanceRatio = spec.stanceRatio ?? 0.62;
  const tracks: CycleTrack[] = [
    contactSwing(spec.leftLeg, { axis: legAxis, degrees: swingDegrees, phase: 0, stanceRatio }),
    contactSwing(spec.rightLeg, { axis: legAxis, degrees: swingDegrees, phase: 0.5, stanceRatio }),
  ];

  if (spec.leftArm && armSwingDegrees !== 0) {
    tracks.push(swing(spec.leftArm, { axis: armAxis, degrees: armSwingDegrees, phase: 0.5 }));
  }
  if (spec.rightArm && armSwingDegrees !== 0) {
    tracks.push(swing(spec.rightArm, { axis: armAxis, degrees: armSwingDegrees, phase: 0 }));
  }
  if (spec.body && spec.bodyBob !== 0) {
    const amount = spec.bodyBob ?? 0.016;
    tracks.push(bob(spec.body, {
      axis: "y",
      amount,
      center: spec.bodyBobCenter ?? amount,
      phase: spec.bodyBobPhase ?? 0.5,
    }));
  }
  if (spec.head && spec.headSwingDegrees !== 0) {
    tracks.push(swing(spec.head, {
      axis: "y",
      degrees: spec.headSwingDegrees ?? 2.5,
      phase: 0.5,
    }));
  }
  tracks.push(...(spec.tracks ?? []));

  return cycleSpecFromTiming(spec, tracks, {
    kind: "biped-walk",
    cycleDistance: spec.cycleDistance ?? 0.9,
    contacts: [
      contactSpec(spec.leftContact ?? spec.leftLeg, "left", 0, stanceRatio),
      contactSpec(spec.rightContact ?? spec.rightLeg, "right", 0.5, stanceRatio),
    ],
    direction: spec.direction ?? [0, 0, -1],
    units: "figure",
  });
}

function buildWingFlapCycle(spec: WingFlapSpec): WalkCycleSpec {
  const axis = spec.axis ?? "z";
  const degrees = spec.degrees ?? 38;
  const center = spec.center ?? 0;
  const frequency = spec.frequency ?? 2;
  const phase = spec.phase ?? 0;
  const mirror = spec.mirror ?? true;
  const tracks: CycleTrack[] = [
    swing(spec.leftWing, { axis, center, degrees, frequency, phase }),
    swing(spec.rightWing, {
      axis,
      center: mirror ? -center : center,
      degrees: mirror ? -degrees : degrees,
      frequency,
      phase,
    }),
  ];

  if (spec.body && spec.bodyBob !== 0) {
    const amount = spec.bodyBob ?? 0.03;
    tracks.push(bob(spec.body, {
      axis: "y",
      amount,
      center: spec.bodyBobCenter ?? 0,
      frequency,
      phase: spec.bodyBobPhase ?? 0.5,
    }));
  }
  tracks.push(...(spec.tracks ?? []));

  const locomotion = spec.cycleDistance === undefined
    ? undefined
    : {
      kind: "wing-flap" as const,
      cycleDistance: spec.cycleDistance,
      direction: spec.direction ?? [0, 0, -1] as const,
      units: "figure" as const,
    };

  return cycleSpecFromTiming(spec, tracks, locomotion);
}

function buildSwimCycle(spec: SwimSpec): WalkCycleSpec {
  const frequency = spec.frequency ?? 1;
  const tailAxis = spec.tailAxis ?? "y";
  const tailSwingDegrees = spec.tailSwingDegrees ?? 18;
  const tracks: CycleTrack[] = [];

  if (spec.body && spec.bodySwayDegrees !== 0) {
    tracks.push(swing(spec.body, {
      axis: tailAxis,
      degrees: spec.bodySwayDegrees ?? 3,
      frequency,
      phase: 0.5,
    }));
  }
  tracks.push(swing(spec.tail, {
    axis: tailAxis,
    degrees: tailSwingDegrees,
    frequency,
  }));
  if (spec.tailTip) {
    tracks.push(swing(spec.tailTip, {
      axis: tailAxis,
      degrees: spec.tailTipSwingDegrees ?? tailSwingDegrees * 1.4,
      frequency,
      phase: spec.tailTipPhase ?? 0.12,
    }));
  }

  const finAxis = spec.finAxis ?? "z";
  const finSwingDegrees = spec.finSwingDegrees ?? 8;
  const finPhase = spec.finPhase ?? 0.25;
  if (spec.leftFin) {
    tracks.push(swing(spec.leftFin, {
      axis: finAxis,
      degrees: finSwingDegrees,
      frequency,
      phase: finPhase,
    }));
  }
  if (spec.rightFin) {
    tracks.push(swing(spec.rightFin, {
      axis: finAxis,
      degrees: -finSwingDegrees,
      frequency,
      phase: finPhase,
    }));
  }
  if (spec.body && spec.bodyBob !== 0) {
    tracks.push(bob(spec.body, {
      axis: "y",
      amount: spec.bodyBob ?? 0.02,
      center: spec.bodyBobCenter ?? 0,
      frequency,
      phase: spec.bodyBobPhase ?? 0.25,
    }));
  }
  tracks.push(...(spec.tracks ?? []));

  const locomotion = spec.cycleDistance === undefined
    ? undefined
    : {
      kind: "swim" as const,
      cycleDistance: spec.cycleDistance,
      direction: spec.direction ?? [0, 0, -1] as const,
      units: "figure" as const,
    };

  return cycleSpecFromTiming(spec, tracks, locomotion);
}

function buildSlitherCycle(spec: SlitherSpec): WalkCycleSpec {
  if (spec.segments.length < 2) {
    throw new Error("slither requires at least two segments");
  }

  const axis = spec.axis ?? "y";
  const degrees = spec.degrees ?? 9;
  const frequency = spec.frequency ?? 1;
  const phaseStep = spec.phaseStep ?? 0.12;
  const lastIndex = spec.segments.length - 1;
  const tracks: CycleTrack[] = spec.segments.map((part, index) =>
    swing(part, {
      axis,
      degrees: degrees * (0.7 + (0.3 * index) / lastIndex),
      frequency,
      phase: index * phaseStep,
    })
  );

  if (spec.body && spec.bodyBob !== 0) {
    const amount = spec.bodyBob ?? 0.01;
    tracks.push(bob(spec.body, {
      axis: "y",
      amount,
      center: spec.bodyBobCenter ?? amount,
      frequency,
      phase: spec.bodyBobPhase ?? 0.25,
    }));
  }
  tracks.push(...(spec.tracks ?? []));

  const locomotion = spec.cycleDistance === undefined
    ? undefined
    : {
      kind: "slither" as const,
      cycleDistance: spec.cycleDistance,
      direction: spec.direction ?? [0, 0, -1] as const,
      units: "figure" as const,
    };

  return cycleSpecFromTiming(spec, tracks, locomotion);
}

function cycleSpecFromTiming(
  timing: CycleTimingSpec,
  tracks: CycleTrack[],
  locomotion?: ClipLocomotionSpec,
): WalkCycleSpec {
  const spec: WalkCycleSpec = { tracks };
  if (timing.duration !== undefined) {
    spec.duration = timing.duration;
  }
  if (timing.fps !== undefined) {
    spec.fps = timing.fps;
  }
  if (timing.loop !== undefined) {
    spec.loop = timing.loop;
  }
  if (timing.samples !== undefined) {
    spec.samples = timing.samples;
  }
  if (timing.label !== undefined) {
    spec.label = timing.label;
  }
  if (timing.role !== undefined) {
    spec.role = timing.role;
  }
  if (timing.nextClip !== undefined) {
    spec.nextClip = timing.nextClip;
  }
  if (locomotion) {
    spec.locomotion = locomotion;
  }
  return spec;
}

function quadrupedPhases(gait: QuadrupedGait): Record<keyof QuadrupedLegs, number> {
  if (gait === "walk") {
    return { frontLeft: 0, frontRight: 0.5, backLeft: 0.75, backRight: 0.25 };
  }
  if (gait === "pace") {
    return { frontLeft: 0, frontRight: 0.5, backLeft: 0, backRight: 0.5 };
  }
  if (gait === "bound") {
    return { frontLeft: 0, frontRight: 0, backLeft: 0.5, backRight: 0.5 };
  }
  return { frontLeft: 0, frontRight: 0.5, backLeft: 0.5, backRight: 0 };
}

function defaultQuadrupedStanceRatio(gait: QuadrupedGait): number {
  if (gait === "bound") {
    return 0.48;
  }
  if (gait === "walk") {
    return 0.65;
  }
  return 0.56;
}

function contactSpec(part: string, role: string, phaseStart: number, stanceRatio: number): LocomotionContactSpec {
  return {
    part,
    phaseStart: roundFloat(normalizePhase(phaseStart)),
    phaseEnd: roundFloat(normalizePhase(phaseStart + stanceRatio)),
    role,
    stanceRatio: roundFloat(stanceRatio),
  };
}

function buildWalkCycleClip(spec: WalkCycleSpec, parts: readonly PartSpec[]): ClipSpec {
  const duration = spec.duration ?? 1;
  const samples = spec.samples ?? 9;
  if (!Number.isFinite(duration) || duration <= 0) {
    throw new Error("walkCycle duration must be positive");
  }
  if (!Number.isInteger(samples) || samples < 2) {
    throw new Error("walkCycle samples must be an integer >= 2");
  }
  if (spec.fps !== undefined && (!Number.isFinite(spec.fps) || spec.fps <= 0)) {
    throw new Error("walkCycle fps must be positive");
  }
  if (spec.tracks.length === 0) {
    throw new Error("walkCycle requires at least one track");
  }

  for (const track of spec.tracks) {
    validateCycleTrack(track);
  }

  const frameParts = Array.from({ length: samples }, () => new Map<string, TransformKey>());
  for (const track of spec.tracks) {
    if (track.kind === "followThrough") {
      continue;
    }
    for (let index = 0; index < samples; index += 1) {
      const progress = index / (samples - 1);
      const proceduralValue = track.kind === "contactSwing"
        ? contactCycleValue(
          progress,
          track.phase ?? 0,
          track.center ?? 0,
          track.degrees,
          track.frequency ?? 1,
          track.stanceRatio,
        )
        : cycleValue(
          progress,
          track.phase ?? 0,
          track.center ?? 0,
          track.kind === "swing" ? track.degrees : track.amount,
          track.frequency ?? 1,
        );
      const value = constrainCycleValue(proceduralValue, track.min, track.max);
      const transform = ensureFrameTransform(frameParts[index], track.part);
      if (track.kind === "swing" || track.kind === "contactSwing") {
        const rot = mutableVec(transform.rot);
        setAxis(rot, track.axis ?? "x", value);
        transform.rot = rot;
      } else {
        const at = mutableVec(transform.at);
        setAxis(at, track.axis ?? "y", value);
        transform.at = at;
      }
    }
  }

  applyFollowThroughTracks(frameParts, spec.tracks, samples);
  applyGroundContactConstraints(frameParts, spec.groundContacts ?? [], parts);

  const keys: ClipKey[] = [];
  for (const [index, parts] of frameParts.entries()) {
    const time = (duration * index) / (samples - 1);
    for (const [part, transform] of parts.entries()) {
      keys.push([part, time, transform]);
    }
  }

  const clip: ClipSpec = {
    loop: spec.loop ?? true,
    keys,
  };
  if (spec.fps !== undefined) {
    clip.fps = spec.fps;
  }
  if (spec.locomotion) {
    clip.locomotion = completeLocomotion(spec.locomotion, duration);
  }
  if (spec.label !== undefined) {
    clip.label = spec.label;
  }
  if (spec.role !== undefined) {
    clip.role = spec.role;
  }
  if (spec.nextClip !== undefined) {
    clip.nextClip = spec.nextClip;
  }
  return clip;
}

function applyGroundContactConstraints(
  frames: Map<string, TransformKey>[],
  constraints: readonly GroundContactConstraint[],
  parts: readonly PartSpec[],
): void {
  if (constraints.length === 0) {
    return;
  }
  const partsByName = new Map(parts.map((part) => [part.name, part]));
  for (const constraint of constraints) {
    validateGroundContactConstraint(constraint, partsByName);
  }
  for (const [frameIndex, frame] of frames.entries()) {
    for (const constraint of constraints) {
      projectGroundContact(frame, constraint, partsByName, frameIndex);
    }
  }
}

function validateGroundContactConstraint(
  constraint: GroundContactConstraint,
  partsByName: ReadonlyMap<string, PartSpec>,
): void {
  const contactPart = partsByName.get(constraint.contactPart);
  if (!contactPart) {
    throw new Error(`ground contact references missing contact part '${constraint.contactPart}'`);
  }
  if (contactPart.primitive.kind !== "box") {
    throw new Error(`ground contact part '${constraint.contactPart}' must be a box`);
  }
  const solvePart = partsByName.get(constraint.solvePart);
  if (!solvePart) {
    throw new Error(`ground contact references missing solve part '${constraint.solvePart}'`);
  }
  if (!isAncestorPart(constraint.solvePart, contactPart, partsByName)) {
    throw new Error(
      `ground contact solve part '${constraint.solvePart}' must be an ancestor of '${constraint.contactPart}'`,
    );
  }
  if (constraint.axis !== undefined && !["x", "y", "z"].includes(constraint.axis)) {
    throw new Error(`ground contact for '${constraint.contactPart}' has invalid axis '${constraint.axis}'`);
  }
  const groundY = constraint.groundY ?? 0;
  const minCorrection = constraint.minCorrectionDegrees ?? -75;
  const maxCorrection = constraint.maxCorrectionDegrees ?? 75;
  if (!Number.isFinite(groundY)) {
    throw new Error(`ground contact for '${constraint.contactPart}' groundY must be finite`);
  }
  if (!Number.isFinite(minCorrection) || !Number.isFinite(maxCorrection)) {
    throw new Error(`ground contact for '${constraint.contactPart}' correction bounds must be finite`);
  }
  if (minCorrection > 0 || maxCorrection < 0 || minCorrection > maxCorrection) {
    throw new Error(
      `ground contact for '${constraint.contactPart}' correction bounds must contain zero`,
    );
  }
}

function isAncestorPart(
  ancestorName: string,
  part: PartSpec,
  partsByName: ReadonlyMap<string, PartSpec>,
): boolean {
  let parentName = part.parent;
  while (parentName !== undefined) {
    if (parentName === ancestorName) {
      return true;
    }
    parentName = partsByName.get(parentName)?.parent;
  }
  return false;
}

function projectGroundContact(
  frame: Map<string, TransformKey>,
  constraint: GroundContactConstraint,
  partsByName: ReadonlyMap<string, PartSpec>,
  frameIndex: number,
): void {
  const groundY = constraint.groundY ?? 0;
  const tolerance = 1e-6;
  const solveTransform = ensureFrameTransform(frame, constraint.solvePart);
  const rotation = mutableVec(solveTransform.rot);
  const axis = constraint.axis ?? "x";
  const axisIndex = axis === "x" ? 0 : axis === "y" ? 1 : 2;
  const originalAngle = rotation[axisIndex];
  const preserveContactRotation = constraint.preserveContactRotation ?? true;
  const contactTransform = preserveContactRotation
    ? ensureFrameTransform(frame, constraint.contactPart)
    : undefined;
  const contactRotation = contactTransform ? mutableVec(contactTransform.rot) : undefined;
  const originalContactAngle = contactRotation?.[axisIndex] ?? 0;
  // Emit the sampled hinge value on every frame so interpolation cannot hold
  // a previous contact correction through the unconstrained flight interval.
  solveTransform.rot = rotation;
  if (contactTransform && contactRotation) {
    contactTransform.rot = contactRotation;
  }
  if (boxBottomY(constraint.contactPart, frame, partsByName) >= groundY - tolerance) {
    return;
  }

  const evaluate = (correction: number): number => {
    rotation[axisIndex] = originalAngle + correction;
    solveTransform.rot = rotation;
    if (contactTransform && contactRotation) {
      contactRotation[axisIndex] = originalContactAngle - correction;
      contactTransform.rot = contactRotation;
    }
    return boxBottomY(constraint.contactPart, frame, partsByName) - groundY;
  };

  const roots = [
    findNearestGroundRoot(evaluate, constraint.minCorrectionDegrees ?? -75),
    findNearestGroundRoot(evaluate, constraint.maxCorrectionDegrees ?? 75),
  ].filter((value): value is number => value !== undefined);
  rotation[axisIndex] = originalAngle;
  if (contactTransform && contactRotation) {
    contactRotation[axisIndex] = originalContactAngle;
  }
  if (roots.length === 0) {
    throw new Error(
      `ground contact '${constraint.contactPart}' could not reach y=${groundY} by correcting ` +
      `'${constraint.solvePart}' at sample ${frameIndex}`,
    );
  }
  const correction = roots.reduce((best, value) =>
    Math.abs(value) < Math.abs(best) ? value : best
  );
  rotation[axisIndex] = originalAngle + correction;
  solveTransform.rot = rotation;
  if (contactTransform && contactRotation) {
    contactRotation[axisIndex] = originalContactAngle - correction;
    contactTransform.rot = contactRotation;
  }
}

function findNearestGroundRoot(
  evaluate: (correction: number) => number,
  correctionLimit: number,
): number | undefined {
  if (correctionLimit === 0) {
    return undefined;
  }
  const direction = Math.sign(correctionLimit);
  const distance = Math.abs(correctionLimit);
  const stepDegrees = 1;
  let previousCorrection = 0;
  let previousValue = evaluate(0);
  for (let step = 1; step <= Math.ceil(distance / stepDegrees); step += 1) {
    const correction = direction * Math.min(distance, step * stepDegrees);
    const value = evaluate(correction);
    if (value >= 0 && previousValue < 0) {
      let penetrating = previousCorrection;
      let clear = correction;
      for (let iteration = 0; iteration < 18; iteration += 1) {
        const middle = (penetrating + clear) / 2;
        if (evaluate(middle) >= 0) {
          clear = middle;
        } else {
          penetrating = middle;
        }
      }
      return clear;
    }
    previousCorrection = correction;
    previousValue = value;
  }
  return undefined;
}

function boxBottomY(
  partName: string,
  frame: ReadonlyMap<string, TransformKey>,
  partsByName: ReadonlyMap<string, PartSpec>,
): number {
  const part = partsByName.get(partName);
  if (!part || part.primitive.kind !== "box") {
    throw new Error(`Cannot evaluate box bottom for '${partName}'`);
  }
  const worldMatrix = partWorldMatrix(part, frame, partsByName);
  const [width, height, depth] = part.primitive.size;
  let bottom = Number.POSITIVE_INFINITY;
  for (const x of [-width / 2, width / 2]) {
    for (const y of [-height / 2, height / 2]) {
      for (const z of [-depth / 2, depth / 2]) {
        bottom = Math.min(bottom, new THREE.Vector3(x, y, z).applyMatrix4(worldMatrix).y);
      }
    }
  }
  return bottom;
}

function partWorldMatrix(
  part: PartSpec,
  frame: ReadonlyMap<string, TransformKey>,
  partsByName: ReadonlyMap<string, PartSpec>,
): THREE.Matrix4 {
  const chain: PartSpec[] = [];
  let current: PartSpec | undefined = part;
  while (current) {
    chain.push(current);
    current = current.parent ? partsByName.get(current.parent) : undefined;
  }
  chain.reverse();

  const world = new THREE.Matrix4();
  for (const chainPart of chain) {
    const transform = frame.get(chainPart.name);
    const pivot = new THREE.Vector3(...(chainPart.joint?.pivot ?? chainPart.pivot ?? [0, 0, 0]));
    const baseAt = chainPart.at ?? [0, 0, 0];
    const deltaAt = transform?.at ?? [0, 0, 0];
    const position = new THREE.Vector3(
      baseAt[0] + pivot.x + deltaAt[0],
      baseAt[1] + pivot.y + deltaAt[1],
      baseAt[2] + pivot.z + deltaAt[2],
    );
    const baseRotation = chainPart.rot ?? [0, 0, 0];
    const deltaRotation = transform?.rot ?? [0, 0, 0];
    const quaternion = new THREE.Quaternion().setFromEuler(new THREE.Euler(
      THREE.MathUtils.degToRad(baseRotation[0] + deltaRotation[0]),
      THREE.MathUtils.degToRad(baseRotation[1] + deltaRotation[1]),
      THREE.MathUtils.degToRad(baseRotation[2] + deltaRotation[2]),
      "XYZ",
    ));
    const scale = new THREE.Vector3(...(transform?.scale ?? [1, 1, 1]));
    const groupMatrix = new THREE.Matrix4().compose(position, quaternion, scale);
    const contentMatrix = new THREE.Matrix4().makeTranslation(-pivot.x, -pivot.y, -pivot.z);
    world.multiply(groupMatrix).multiply(contentMatrix);
  }
  return world;
}

function validateCycleTrack(track: CycleTrack): void {
  if (!track.part.trim()) {
    throw new Error("walkCycle track part is required");
  }
  if (track.axis !== undefined && !["x", "y", "z"].includes(track.axis)) {
    throw new Error(`walkCycle track '${track.part}' has invalid axis '${track.axis}'`);
  }
  validateCycleValueConstraint(track);
  if (track.kind === "followThrough") {
    if (!track.source.trim()) {
      throw new Error(`walkCycle track '${track.part}' followThrough source is required`);
    }
    if (track.sourceAxis !== undefined && !["x", "y", "z"].includes(track.sourceAxis)) {
      throw new Error(`walkCycle track '${track.part}' has invalid sourceAxis '${track.sourceAxis}'`);
    }
    if (!Number.isFinite(track.degrees)) {
      throw new Error(`walkCycle track '${track.part}' degrees must be finite`);
    }
    if (track.lag !== undefined && (!Number.isFinite(track.lag) || track.lag < 0 || track.lag >= 1)) {
      throw new Error(`walkCycle track '${track.part}' lag must be in [0, 1)`);
    }
    if (track.overshoot !== undefined && !Number.isFinite(track.overshoot)) {
      throw new Error(`walkCycle track '${track.part}' overshoot must be finite`);
    }
    if (track.center !== undefined && !Number.isFinite(track.center)) {
      throw new Error(`walkCycle track '${track.part}' center must be finite`);
    }
    return;
  }
  const amount = track.kind === "bob" ? track.amount : track.degrees;
  if (!Number.isFinite(amount)) {
    throw new Error(`walkCycle track '${track.part}' amount must be finite`);
  }
  if (track.center !== undefined && !Number.isFinite(track.center)) {
    throw new Error(`walkCycle track '${track.part}' center must be finite`);
  }
  if (track.frequency !== undefined && (!Number.isFinite(track.frequency) || track.frequency <= 0)) {
    throw new Error(`walkCycle track '${track.part}' frequency must be positive`);
  }
  if (track.phase !== undefined && !Number.isFinite(track.phase)) {
    throw new Error(`walkCycle track '${track.part}' phase must be finite`);
  }
  if (
    track.kind === "contactSwing" &&
    (!Number.isFinite(track.stanceRatio) || track.stanceRatio <= 0 || track.stanceRatio >= 1)
  ) {
    throw new Error(`walkCycle track '${track.part}' stanceRatio must be between 0 and 1`);
  }
}

function validateCycleValueConstraint(track: CycleTrack): void {
  if (track.min !== undefined && !Number.isFinite(track.min)) {
    throw new Error(`walkCycle track '${track.part}' min must be finite`);
  }
  if (track.max !== undefined && !Number.isFinite(track.max)) {
    throw new Error(`walkCycle track '${track.part}' max must be finite`);
  }
  if (track.min !== undefined && track.max !== undefined && track.min > track.max) {
    throw new Error(`walkCycle track '${track.part}' min must not exceed max`);
  }
}

function validateLocomotion(
  clipName: string,
  locomotion: ClipLocomotionSpec,
  partNames: Set<string>,
  errors: string[],
): void {
  if (!["biped-walk", "quadruped-walk", "slither", "swim", "wing-flap"].includes(locomotion.kind)) {
    errors.push(`clip '${clipName}' locomotion kind '${locomotion.kind}' is invalid`);
  }
  if (!Number.isFinite(locomotion.cycleDistance) || locomotion.cycleDistance <= 0) {
    errors.push(`clip '${clipName}' locomotion cycleDistance must be positive`);
  }
  if (locomotion.speed !== undefined && (!Number.isFinite(locomotion.speed) || locomotion.speed <= 0)) {
    errors.push(`clip '${clipName}' locomotion speed must be positive`);
  }
  if (locomotion.units !== undefined && locomotion.units !== "figure") {
    errors.push(`clip '${clipName}' locomotion units '${locomotion.units}' are invalid`);
  }
  if (locomotion.direction) {
    let lengthSq = 0;
    for (const [index, value] of locomotion.direction.entries()) {
      if (!Number.isFinite(value)) {
        errors.push(`clip '${clipName}' locomotion direction[${index}] must be finite`);
      }
      lengthSq += value * value;
    }
    if (lengthSq === 0) {
      errors.push(`clip '${clipName}' locomotion direction must be nonzero`);
    }
  }
  if (locomotion.contacts) {
    for (const contact of locomotion.contacts) {
      if (!partNames.has(contact.part)) {
        errors.push(`clip '${clipName}' locomotion contact references missing part '${contact.part}'`);
      }
      validatePhase(`clip '${clipName}' locomotion contact '${contact.part}' phaseStart`, contact.phaseStart, errors);
      validatePhase(`clip '${clipName}' locomotion contact '${contact.part}' phaseEnd`, contact.phaseEnd, errors);
      if (!Number.isFinite(contact.stanceRatio) || contact.stanceRatio <= 0 || contact.stanceRatio >= 1) {
        errors.push(`clip '${clipName}' locomotion contact '${contact.part}' stanceRatio must be between 0 and 1`);
      }
    }
  }
}

function validatePhase(label: string, value: number, errors: string[]): void {
  if (!Number.isFinite(value) || value < 0 || value >= 1) {
    errors.push(`${label} must be in [0, 1)`);
  }
}

function ensureFrameTransform(frame: Map<string, TransformKey> | undefined, part: string): TransformKey {
  if (!frame) {
    throw new Error("Missing walkCycle frame");
  }
  const existing = frame.get(part);
  if (existing) {
    return existing;
  }
  const transform: TransformKey = {};
  frame.set(part, transform);
  return transform;
}

function cycleValue(progress: number, phase: number, center: number, amount: number, frequency: number): number {
  return center + Math.cos((progress * frequency + phase) * Math.PI * 2) * amount;
}

function constrainCycleValue(value: number, min: number | undefined, max: number | undefined): number {
  return Math.min(max ?? Number.POSITIVE_INFINITY, Math.max(min ?? Number.NEGATIVE_INFINITY, value));
}

function contactCycleValue(
  progress: number,
  phaseStart: number,
  center: number,
  amount: number,
  frequency: number,
  stanceRatio: number,
): number {
  const cycleProgress = normalizePhase(progress * frequency - phaseStart);
  if (cycleProgress <= stanceRatio) {
    return center + lerp(amount, -amount, cycleProgress / stanceRatio);
  }
  const recoveryProgress = (cycleProgress - stanceRatio) / (1 - stanceRatio);
  return center + lerp(-amount, amount, smoothStep(recoveryProgress));
}

function applyFollowThroughTracks(
  frames: Map<string, TransformKey>[],
  tracks: CycleTrack[],
  samples: number,
): void {
  const outputs: { part: string; axis: AxisName; values: number[] }[] = [];
  for (const track of tracks) {
    if (track.kind !== "followThrough") {
      continue;
    }
    outputs.push({
      part: track.part,
      axis: track.axis ?? "x",
      values: computeFollowThroughValues(frames, track, samples),
    });
  }
  for (const output of outputs) {
    const axisIndex = output.axis === "x" ? 0 : output.axis === "y" ? 1 : 2;
    for (let index = 0; index < frames.length; index += 1) {
      const transform = ensureFrameTransform(frames[index], output.part);
      const rot = mutableVec(transform.rot);
      rot[axisIndex] += output.values[index] ?? 0;
      transform.rot = rot;
    }
  }
}

// A follow-through track makes a child part lag and overshoot a parent's motion
// instead of running on its own clock: it reads the driver part's already-baked
// curve, normalizes it, delays it by `lag`, and adds a velocity term for whip.
// The result is sampled into ordinary keyframes like every other cycle helper.
function computeFollowThroughValues(
  frames: Map<string, TransformKey>[],
  track: Extract<CycleTrack, { kind: "followThrough" }>,
  samples: number,
): number[] {
  const period = Math.max(1, samples - 1);
  const channel = track.sourceChannel ?? "pos";
  const sourceAxis = track.sourceAxis ?? "y";
  const lag = track.lag ?? 0.12;
  const overshoot = track.overshoot ?? 0.45;
  const center = track.center ?? 0;

  const raw: number[] = [];
  for (let index = 0; index < period; index += 1) {
    const transform = frames[index]?.get(track.source);
    const vec = channel === "rot" ? transform?.rot : transform?.at;
    raw.push(vec ? axisValue(vec, sourceAxis) : 0);
  }

  let mean = 0;
  for (const value of raw) {
    mean += value;
  }
  mean /= period;
  const centered = raw.map((value) => value - mean);

  let amplitude = 0;
  for (const value of centered) {
    amplitude = Math.max(amplitude, Math.abs(value));
  }

  const velocity: number[] = [];
  for (let index = 0; index < period; index += 1) {
    const next = centered[(index + 1) % period] ?? 0;
    const prev = centered[(index - 1 + period) % period] ?? 0;
    velocity.push((next - prev) / 2);
  }
  let velocityAmplitude = 0;
  for (const value of velocity) {
    velocityAmplitude = Math.max(velocityAmplitude, Math.abs(value));
  }

  const lagSamples = (((Math.round(lag * period) % period) + period) % period);
  const values: number[] = [];
  for (let index = 0; index <= period; index += 1) {
    if (amplitude <= 1e-6) {
      values.push(constrainCycleValue(center, track.min, track.max));
      continue;
    }
    const wrapped = (((index % period) - lagSamples) % period + period) % period;
    const normalized = (centered[wrapped] ?? 0) / amplitude;
    const normalizedVelocity = velocityAmplitude > 1e-6 ? (velocity[wrapped] ?? 0) / velocityAmplitude : 0;
    values.push(constrainCycleValue(
      center + track.degrees * (normalized + overshoot * normalizedVelocity),
      track.min,
      track.max,
    ));
  }
  return values;
}

function axisValue(vec: Vec3, axis: AxisName): number {
  if (axis === "x") {
    return vec[0];
  }
  if (axis === "y") {
    return vec[1];
  }
  return vec[2];
}

function completeLocomotion(locomotion: ClipLocomotionSpec, duration: number): ClipLocomotionSpec {
  const complete: ClipLocomotionSpec = { ...locomotion };
  if (complete.speed === undefined) {
    complete.speed = roundFloat(complete.cycleDistance / duration);
  }
  return complete;
}

function normalizePhase(value: number): number {
  const normalized = value - Math.floor(value);
  return normalized === 1 ? 0 : normalized;
}

function smoothStep(value: number): number {
  const t = Math.max(0, Math.min(1, value));
  return t * t * (3 - 2 * t);
}

function lerp(left: number, right: number, alpha: number): number {
  return left + (right - left) * alpha;
}

function roundFloat(value: number): number {
  return Math.round(value * 10_000) / 10_000;
}

function mutableVec(value: Vec3 | undefined): [number, number, number] {
  return value ? [value[0], value[1], value[2]] : [0, 0, 0];
}

function setAxis(value: [number, number, number], axis: AxisName, amount: number): void {
  if (axis === "x") {
    value[0] = amount;
  } else if (axis === "y") {
    value[1] = amount;
  } else {
    value[2] = amount;
  }
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

function validateFiniteVec(label: string, value: unknown, errors: string[]): void {
  if (!Array.isArray(value) || value.length !== 3) {
    errors.push(`${label} must contain exactly three numbers`);
    return;
  }
  for (const [index, item] of value.entries()) {
    if (!Number.isFinite(item)) {
      errors.push(`${label}[${index}] must be finite`);
    }
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
