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

export type LocomotionKind = "biped-walk" | "quadruped-walk" | "wing-flap";

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

export interface ClipSpec {
  fps?: number;
  loop?: boolean;
  locomotion?: ClipLocomotionSpec;
  keys: ClipKey[];
}

export type AxisName = "x" | "y" | "z";

export interface SwingOptions {
  axis?: AxisName;
  degrees: number;
  center?: number;
  frequency?: number;
  phase?: number;
}

export interface BobOptions {
  axis?: AxisName;
  amount: number;
  center?: number;
  frequency?: number;
  phase?: number;
}

export interface ContactSwingOptions extends SwingOptions {
  stanceRatio: number;
}

export type CycleTrack =
  | ({ kind: "swing"; part: string } & SwingOptions)
  | ({ kind: "contactSwing"; part: string } & ContactSwingOptions)
  | ({ kind: "bob"; part: string } & BobOptions);

export interface WalkCycleSpec {
  duration?: number;
  fps?: number;
  locomotion?: ClipLocomotionSpec;
  loop?: boolean;
  samples?: number;
  tracks: CycleTrack[];
}

export interface CycleTimingSpec {
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

export interface FigureApi {
  mat(name: string, colorOrSpec: string | MaterialSpec): void;
  asciiTexture(name: string, texture: AsciiTextureSpec): void;
  part(name: string, draft: PartDraft): void;
  clip(name: string, spec: ClipSpec): void;
  walkCycle(name: string, spec: WalkCycleSpec): void;
  bipedWalk(name: string, spec: BipedWalkSpec): void;
  quadrupedWalk(name: string, spec: QuadrupedWalkSpec): void;
  wingFlap(name: string, spec: WingFlapSpec): void;
  swing(part: string, options: SwingOptions): CycleTrack;
  contactSwing(part: string, options: ContactSwingOptions): CycleTrack;
  bob(part: string, options: BobOptions): CycleTrack;
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
  readonly api: FigureApi;

  constructor(private readonly name: string) {
    this.api = {
      mat: (name, colorOrSpec) => this.mat(name, colorOrSpec),
      asciiTexture: (name, texture) => this.asciiTexture(name, texture),
      part: (name, draft) => this.part(name, draft),
      clip: (name, spec) => this.clip(name, spec),
      walkCycle: (name, spec) => this.clip(name, buildWalkCycleClip(spec)),
      bipedWalk: (name, spec) => this.clip(name, buildWalkCycleClip(buildBipedWalkCycle(spec))),
      quadrupedWalk: (name, spec) => this.clip(name, buildWalkCycleClip(buildQuadrupedWalkCycle(spec))),
      wingFlap: (name, spec) => this.clip(name, buildWalkCycleClip(buildWingFlapCycle(spec))),
      swing,
      contactSwing,
      bob,
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

function swing(part: string, options: SwingOptions): CycleTrack {
  const track: CycleTrack = { kind: "swing", part, degrees: options.degrees };
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
  return track;
}

function bob(part: string, options: BobOptions): CycleTrack {
  const track: CycleTrack = { kind: "bob", part, amount: options.amount };
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
  return track;
}

function contactSwing(part: string, options: ContactSwingOptions): CycleTrack {
  const track: CycleTrack = {
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
  return track;
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

function buildWalkCycleClip(spec: WalkCycleSpec): ClipSpec {
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

  const frameParts = Array.from({ length: samples }, () => new Map<string, TransformKey>());
  for (const track of spec.tracks) {
    validateCycleTrack(track);
    for (let index = 0; index < samples; index += 1) {
      const progress = index / (samples - 1);
      const value = track.kind === "contactSwing"
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
  return clip;
}

function validateCycleTrack(track: CycleTrack): void {
  if (!track.part.trim()) {
    throw new Error("walkCycle track part is required");
  }
  if (track.axis !== undefined && !["x", "y", "z"].includes(track.axis)) {
    throw new Error(`walkCycle track '${track.part}' has invalid axis '${track.axis}'`);
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

function validateLocomotion(
  clipName: string,
  locomotion: ClipLocomotionSpec,
  partNames: Set<string>,
  errors: string[],
): void {
  if (!["biped-walk", "quadruped-walk", "wing-flap"].includes(locomotion.kind)) {
    errors.push(`clip '${clipName}' locomotion kind '${locomotion.kind}' is invalid`);
  }
  if (!Number.isFinite(locomotion.cycleDistance) || locomotion.cycleDistance <= 0) {
    errors.push(`clip '${clipName}' locomotion cycleDistance must be positive`);
  }
  if (locomotion.speed !== undefined && (!Number.isFinite(locomotion.speed) || locomotion.speed <= 0)) {
    errors.push(`clip '${clipName}' locomotion speed must be positive`);
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

function validatePositive(label: string, value: number, errors: string[]): void {
  if (!Number.isFinite(value) || value <= 0) {
    errors.push(`${label} must be positive`);
  }
}

function isHexColor(value: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(value);
}
