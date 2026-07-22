import * as THREE from "three";
import type {
  BoxFaceName,
  FigureFaceName,
  FigureAsset,
  PartSpec,
  SurfaceExceptionSpec,
  Vec3,
} from "./dsl";
import {
  evaluateFigurePose,
  figureAnalysisPoses,
  type FigurePose,
} from "./figure-pose";
import { SURFACE_BASELINE } from "./surface-baseline";

export const COPLANAR_OVERLAP_RULE = "coplanar-overlap" as const;
export const ARTICULATED_SEAM_MARGIN_RULE = "articulated-seam-margin" as const;
export type FigureSurfaceRule =
  | typeof COPLANAR_OVERLAP_RULE
  | typeof ARTICULATED_SEAM_MARGIN_RULE;

export interface FigureFaceRef {
  part: string;
  face: FigureFaceName;
}

export interface FigureSurfaceIssue {
  rule: FigureSurfaceRule;
  faces: [FigureFaceRef, FigureFaceRef];
  pose: FigurePose;
  gap: number;
  requiredMargin?: number;
  overlapRatio: number;
  occurrences: number;
}

export interface FigureSurfaceAnalysisOptions {
  exactPlaneScale?: number;
  minimumOverlapRatio?: number;
  parallelDotThreshold?: number;
  poses?: FigurePose[];
}

export interface AcknowledgedFigureSurfaceIssue {
  issue: FigureSurfaceIssue;
  reason: string;
}

export interface FigureSurfaceEvaluation {
  acknowledged: AcknowledgedFigureSurfaceIssue[];
  baseline: FigureSurfaceIssue[];
  unacknowledged: FigureSurfaceIssue[];
  staleExceptions: SurfaceExceptionSpec[];
  staleBaseline: string[];
}

interface TransformedFace extends FigureFaceRef {
  appearance: string;
  center: THREE.Vector3;
  normal: THREE.Vector3;
  vertices: [THREE.Vector3, THREE.Vector3, THREE.Vector3, THREE.Vector3];
}

interface TransformedBox {
  part: string;
  inverse: THREE.Matrix4;
  halfSize: THREE.Vector3;
}

interface OverlapMeasurement {
  ratio: number;
  samplePoints: THREE.Vector3[];
}

interface Vec2 {
  x: number;
  y: number;
}

const DEFAULT_EXACT_PLANE_SCALE = 1e-5;
const DEFAULT_MINIMUM_OVERLAP_RATIO = 0.01;
const DEFAULT_PARALLEL_DOT_THRESHOLD = 0.99999;
const ARTICULATED_MARGIN_SCALE = 0.0075;
const MINIMUM_ARTICULATED_MARGIN = 0.02;

/** Finds same-facing, coplanar box and plane faces with meaningful projected overlap. */
export function analyzeFigureSurfaces(
  asset: FigureAsset,
  options: FigureSurfaceAnalysisOptions = {},
): FigureSurfaceIssue[] {
  const poses = options.poses ?? figureAnalysisPoses(asset);
  const exactPlaneScale = options.exactPlaneScale ?? DEFAULT_EXACT_PLANE_SCALE;
  const minimumOverlapRatio = options.minimumOverlapRatio ?? DEFAULT_MINIMUM_OVERLAP_RATIO;
  const parallelDotThreshold = options.parallelDotThreshold ?? DEFAULT_PARALLEL_DOT_THRESHOLD;
  const articulatedPartPairs = animatedHeadSocketPairs(asset);
  const grouped = new Map<string, FigureSurfaceIssue>();
  const articulatedGroups = new Map<string, Map<BoxFaceName, FigureSurfaceIssue>>();

  for (const pose of poses) {
    const matrices = evaluateFigurePose(asset, pose);
    const faces = asset.parts.flatMap((part) =>
      transformedFaces(asset, part, matrices.get(part.name)!)
    );
    const boxes = asset.parts.flatMap((part) =>
      part.primitive.kind === "box"
        ? [transformedBox(part, matrices.get(part.name)!)]
        : []
    );
    const diagonal = figureDiagonal(faces);
    const exactPlaneEpsilon = Math.max(1e-7, diagonal * exactPlaneScale);
    const articulatedMargin = Math.max(
      MINIMUM_ARTICULATED_MARGIN,
      diagonal * ARTICULATED_MARGIN_SCALE,
    );

    for (let leftIndex = 0; leftIndex < faces.length; leftIndex += 1) {
      const left = faces[leftIndex]!;
      for (let rightIndex = leftIndex + 1; rightIndex < faces.length; rightIndex += 1) {
        const right = faces[rightIndex]!;
        if (
          left.part === right.part
          || left.appearance === right.appearance
          || left.normal.dot(right.normal) < parallelDotThreshold
        ) {
          continue;
        }
        const gap = Math.abs(right.center.clone().sub(left.center).dot(left.normal));
        const exactCandidate = gap <= exactPlaneEpsilon;
        const sharedAxis = left.face === right.face ? faceAxis(left.face) : undefined;
        const articulatedAxis = sharedAxis === "x"
          && articulatedPartPairs.has(partPairKey(left.part, right.part))
          && gap < articulatedMargin
          ? sharedAxis
          : undefined;
        if (!exactCandidate && articulatedAxis === undefined) {
          continue;
        }
        const overlap = projectedOverlap(left, right);
        if (
          overlap.ratio < minimumOverlapRatio
          || overlapIsCovered(left, right, overlap.samplePoints, boxes, exactPlaneEpsilon)
        ) {
          continue;
        }
        const pair = normalizeFacePair(left, right);
        if (exactCandidate) {
          recordIssue(grouped, facePairKey(pair), {
            rule: COPLANAR_OVERLAP_RULE,
            faces: pair,
            pose,
            gap,
            overlapRatio: overlap.ratio,
            occurrences: 1,
          });
        }
        if (articulatedAxis !== undefined) {
          const groupKey = `${partPairKey(left.part, right.part)}:${articulatedAxis}`;
          const issuesByFace = articulatedGroups.get(groupKey) ?? new Map();
          articulatedGroups.set(groupKey, issuesByFace);
          recordIssue(issuesByFace, left.face as BoxFaceName, {
            rule: ARTICULATED_SEAM_MARGIN_RULE,
            faces: pair,
            pose,
            gap,
            requiredMargin: articulatedMargin,
            overlapRatio: overlap.ratio,
            occurrences: 1,
          });
        }
      }
    }
  }

  const articulatedIssues: FigureSurfaceIssue[] = [];
  for (const issuesByFace of articulatedGroups.values()) {
    const eastIssue = issuesByFace.get("east");
    const westIssue = issuesByFace.get("west");
    if (eastIssue && westIssue) {
      articulatedIssues.push(eastIssue, westIssue);
    }
  }
  const articulatedFacePairs = new Set(
    articulatedIssues.map((issue) => facePairKey(issue.faces)),
  );
  return [
    ...[...grouped.values()].filter((issue) =>
      !articulatedFacePairs.has(facePairKey(issue.faces))
    ),
    ...articulatedIssues,
  ].sort((left, right) => surfaceIssueKey(left).localeCompare(surfaceIssueKey(right)));
}

function recordIssue<Key extends string>(
  issues: Map<Key, FigureSurfaceIssue>,
  key: Key,
  issue: FigureSurfaceIssue,
): void {
  const current = issues.get(key);
  if (!current) {
    issues.set(key, issue);
    return;
  }
  current.occurrences += 1;
  const currentMarginSlack = current.requiredMargin === undefined
    ? Number.POSITIVE_INFINITY
    : current.gap - current.requiredMargin;
  const nextMarginSlack = issue.requiredMargin === undefined
    ? Number.POSITIVE_INFINITY
    : issue.gap - issue.requiredMargin;
  if (
    nextMarginSlack < currentMarginSlack
    || (nextMarginSlack === currentMarginSlack && issue.overlapRatio > current.overlapRatio)
  ) {
    current.pose = issue.pose;
    current.gap = issue.gap;
    if (issue.requiredMargin === undefined) {
      delete current.requiredMargin;
    } else {
      current.requiredMargin = issue.requiredMargin;
    }
    current.overlapRatio = issue.overlapRatio;
  } else if (
    current.requiredMargin === undefined
    && issue.requiredMargin === undefined
    && issue.overlapRatio > current.overlapRatio
  ) {
    current.pose = issue.pose;
    current.gap = issue.gap;
    current.overlapRatio = issue.overlapRatio;
  }
}

function faceAxis(face: FigureFaceName): "x" | "y" | "z" | undefined {
  if (face === "east" || face === "west") {
    return "x";
  }
  if (face === "up" || face === "down") {
    return "y";
  }
  if (face === "north" || face === "south" || face === "front" || face === "back") {
    return "z";
  }
  return undefined;
}

function animatedHeadSocketPairs(asset: FigureAsset): Set<string> {
  const parts = new Map(asset.parts.map((part) => [part.name, part] as const));
  const animatedParts = new Set(
    Object.values(asset.clips).flatMap((clip) => clip.keys.map(([part]) => part)),
  );
  const pairs = new Set<string>();
  for (const descendant of asset.parts) {
    const relativePath: string[] = [];
    let current: PartSpec | undefined = descendant;
    while (current?.parent !== undefined) {
      relativePath.push(current.name);
      const ancestor: PartSpec | undefined = parts.get(current.parent);
      if (!ancestor) {
        break;
      }
      if (
        isHeadSocketPair(descendant.name, ancestor.name)
        && relativePath.some((part) => animatedParts.has(part))
      ) {
        pairs.add(partPairKey(descendant.name, ancestor.name));
      }
      current = ancestor;
    }
  }
  return pairs;
}

function isHeadSocketPair(left: string, right: string): boolean {
  if (left !== "head" && right !== "head") {
    return false;
  }
  const socket = left === "head" ? right : left;
  return socket === "body"
    || socket === "throat"
    || socket === "neck"
    || socket.startsWith("neck_");
}

function partPairKey(left: string, right: string): string {
  return left.localeCompare(right) <= 0 ? `${left}|${right}` : `${right}|${left}`;
}

export function formatSurfaceIssue(issue: FigureSurfaceIssue): string {
  const [left, right] = issue.faces;
  const pose = issue.pose.clipName === undefined
    ? "rest pose"
    : `clip '${issue.pose.clipName}' at ${issue.pose.time.toFixed(3)}s`;
  const measurement = issue.rule === ARTICULATED_SEAM_MARGIN_RULE
    ? `margin ${issue.gap.toFixed(3)}, required ${(issue.requiredMargin ?? 0).toFixed(3)}`
    : `gap ${issue.gap.toExponential(2)}`;
  return `${issue.rule} ${left.part}.${left.face} / ${right.part}.${right.face} in ${pose}`
    + ` (${measurement}, overlap ${(issue.overlapRatio * 100).toFixed(1)}%, ${issue.occurrences} sampled pose${issue.occurrences === 1 ? "" : "s"})`;
}

export function surfaceIssueKey(issue: FigureSurfaceIssue): string {
  return `${issue.rule}:${facePairKey(issue.faces)}`;
}

export function evaluateFigureSurfaces(asset: FigureAsset): FigureSurfaceEvaluation {
  const remainingExceptions = [...(asset.surfaceExceptions ?? [])];
  const remainingBaseline = [...(SURFACE_BASELINE[asset.name] ?? [])];
  const acknowledged: AcknowledgedFigureSurfaceIssue[] = [];
  const baseline: FigureSurfaceIssue[] = [];
  const unacknowledged: FigureSurfaceIssue[] = [];
  for (const issue of analyzeFigureSurfaces(asset)) {
    const key = surfaceIssueKey(issue);
    const exceptionIndex = remainingExceptions.findIndex((exception) =>
      surfaceExceptionKey(exception) === key
    );
    if (exceptionIndex >= 0) {
      const [exception] = remainingExceptions.splice(exceptionIndex, 1);
      acknowledged.push({ issue, reason: exception!.reason.trim() });
      continue;
    }
    const baselineIndex = remainingBaseline.indexOf(key);
    if (baselineIndex >= 0) {
      remainingBaseline.splice(baselineIndex, 1);
      baseline.push(issue);
      continue;
    }
    unacknowledged.push(issue);
  }
  return {
    acknowledged,
    baseline,
    unacknowledged,
    staleExceptions: remainingExceptions,
    staleBaseline: remainingBaseline,
  };
}

export function assertFigureSurfaces(asset: FigureAsset): void {
  const evaluation = evaluateFigureSurfaces(asset);
  if (
    evaluation.unacknowledged.length === 0
    && evaluation.staleExceptions.length === 0
    && evaluation.staleBaseline.length === 0
  ) {
    return;
  }
  const errors = evaluation.unacknowledged.map((issue) =>
    `${formatSurfaceIssue(issue)}; ${issue.rule === ARTICULATED_SEAM_MARGIN_RULE ? "widen the outer part or narrow the inner part across this articulated seam" : "separate the surfaces"}, or add surfaceException({ rule: "${issue.rule}", faces: ${JSON.stringify(issue.faces)}, reason: "..." })`
  );
  errors.push(...evaluation.staleExceptions.map((exception) =>
    `stale surface exception ${surfaceExceptionKey(exception)}; remove it or update it to the current face pair`
  ));
  errors.push(...evaluation.staleBaseline.map((key) =>
    `stale surface baseline '${key}'; remove it from surface-baseline.ts to ratchet the catalog`
  ));
  throw new Error(
    `Figure '${asset.name}' failed surface analysis:\n${errors.map((error) => `- ${error}`).join("\n")}`,
  );
}

function transformedFaces(
  asset: FigureAsset,
  part: PartSpec,
  matrix: THREE.Matrix4,
): TransformedFace[] {
  if (part.primitive.kind === "plane") {
    return transformedPlaneFaces(asset, part, matrix);
  }
  if (part.primitive.kind !== "box") {
    return [];
  }
  const [x, y, z] = part.primitive.size.map((value) => value / 2) as [number, number, number];
  const definitions: Array<[BoxFaceName, Vec3, Vec3, Vec3]> = [
    ["east", [x, 0, 0], [0, y, 0], [0, 0, z]],
    ["west", [-x, 0, 0], [0, y, 0], [0, 0, -z]],
    ["up", [0, y, 0], [x, 0, 0], [0, 0, -z]],
    ["down", [0, -y, 0], [x, 0, 0], [0, 0, z]],
    ["south", [0, 0, z], [x, 0, 0], [0, y, 0]],
    ["north", [0, 0, -z], [-x, 0, 0], [0, y, 0]],
  ];
  return definitions.map(([face, centerValue, uValue, vValue]) => {
    const center = vector(centerValue).applyMatrix4(matrix);
    const u = vector(centerValue).add(vector(uValue)).applyMatrix4(matrix).sub(center);
    const v = vector(centerValue).add(vector(vValue)).applyMatrix4(matrix).sub(center);
    const normal = u.clone().cross(v).normalize();
    const vertices = [
      center.clone().sub(u).sub(v),
      center.clone().add(u).sub(v),
      center.clone().add(u).add(v),
      center.clone().sub(u).add(v),
    ] as TransformedFace["vertices"];
    return {
      part: part.name,
      face,
      appearance: faceAppearance(asset, part, face),
      center,
      normal,
      vertices,
    };
  });
}

function transformedPlaneFaces(
  asset: FigureAsset,
  part: PartSpec,
  matrix: THREE.Matrix4,
): TransformedFace[] {
  if (part.primitive.kind !== "plane") {
    return [];
  }
  const halfWidth = part.primitive.width / 2;
  const halfHeight = part.primitive.height / 2;
  const definitions: Array<["front" | "back", Vec3, Vec3]> = [
    ["front", [halfWidth, 0, 0], [0, halfHeight, 0]],
  ];
  if (part.primitive.sidedness === "double") {
    definitions.push(["back", [-halfWidth, 0, 0], [0, halfHeight, 0]]);
  }
  return definitions.map(([face, uValue, vValue]) => {
    const center = new THREE.Vector3().applyMatrix4(matrix);
    const u = vector(uValue).applyMatrix4(matrix).sub(center);
    const v = vector(vValue).applyMatrix4(matrix).sub(center);
    const normal = u.clone().cross(v).normalize();
    const vertices = [
      center.clone().sub(u).sub(v),
      center.clone().add(u).sub(v),
      center.clone().add(u).add(v),
      center.clone().sub(u).add(v),
    ] as TransformedFace["vertices"];
    return {
      part: part.name,
      face,
      appearance: faceAppearance(asset, part, face),
      center,
      normal,
      vertices,
    };
  });
}

function faceAppearance(
  asset: FigureAsset,
  part: PartSpec,
  face: FigureFaceName,
): string {
  const faceSpec = part.primitive.kind === "box" && face !== "front" && face !== "back"
    ? part.primitive.faces?.[face]
    : undefined;
  const materialName = faceSpec?.material ?? part.material;
  const textureName = faceSpec?.texture ?? part.texture;
  return JSON.stringify({
    material: materialName === undefined ? undefined : asset.materials[materialName],
    texture: textureName === undefined ? undefined : asset.textures[textureName],
  });
}

function projectedOverlap(
  left: TransformedFace,
  right: TransformedFace,
): OverlapMeasurement {
  const tangent = left.vertices[1].clone().sub(left.vertices[0]).normalize();
  const bitangent = left.normal.clone().cross(tangent).normalize();
  const project = (point: THREE.Vector3): Vec2 => {
    const delta = point.clone().sub(left.center);
    return { x: delta.dot(tangent), y: delta.dot(bitangent) };
  };
  const leftPolygon = left.vertices.map(project);
  const rightPolygon = right.vertices.map(project);
  const intersection = clipConvexPolygon(rightPolygon, leftPolygon);
  const overlapArea = polygonArea(intersection);
  const smallerArea = Math.min(polygonArea(leftPolygon), polygonArea(rightPolygon));
  if (smallerArea <= 1e-12 || intersection.length < 3) {
    return { ratio: 0, samplePoints: [] };
  }
  const samples2d = [polygonCentroid(intersection)];
  for (let index = 0; index < intersection.length; index += 1) {
    const current = intersection[index]!;
    const next = intersection[(index + 1) % intersection.length]!;
    samples2d.push({ x: (current.x + next.x) / 2, y: (current.y + next.y) / 2 });
  }
  return {
    ratio: overlapArea / smallerArea,
    samplePoints: samples2d.map((point) => left.center.clone()
      .addScaledVector(tangent, point.x)
      .addScaledVector(bitangent, point.y)),
  };
}

function overlapIsCovered(
  left: TransformedFace,
  right: TransformedFace,
  samplePoints: THREE.Vector3[],
  boxes: TransformedBox[],
  epsilon: number,
): boolean {
  if (samplePoints.length === 0) {
    return true;
  }
  const outwardStep = Math.max(epsilon * 4, 1e-6);
  return samplePoints.every((point) => {
    const outside = point.clone().addScaledVector(left.normal, outwardStep);
    return boxes.some((box) =>
      box.part !== left.part
      && box.part !== right.part
      && boxContainsPoint(box, outside, outwardStep)
    );
  });
}

function transformedBox(part: PartSpec, matrix: THREE.Matrix4): TransformedBox {
  if (part.primitive.kind !== "box") {
    throw new Error(`Part '${part.name}' is not a box`);
  }
  return {
    part: part.name,
    inverse: matrix.clone().invert(),
    halfSize: vector(part.primitive.size).multiplyScalar(0.5),
  };
}

function boxContainsPoint(
  box: TransformedBox,
  point: THREE.Vector3,
  epsilon: number,
): boolean {
  const local = point.clone().applyMatrix4(box.inverse);
  return Math.abs(local.x) < box.halfSize.x - epsilon
    && Math.abs(local.y) < box.halfSize.y - epsilon
    && Math.abs(local.z) < box.halfSize.z - epsilon;
}

function clipConvexPolygon(subject: Vec2[], clip: Vec2[]): Vec2[] {
  let output = subject;
  for (let index = 0; index < clip.length; index += 1) {
    const edgeStart = clip[index]!;
    const edgeEnd = clip[(index + 1) % clip.length]!;
    const input = output;
    output = [];
    if (input.length === 0) {
      break;
    }
    let previous = input[input.length - 1]!;
    for (const current of input) {
      const currentInside = inside(current, edgeStart, edgeEnd);
      const previousInside = inside(previous, edgeStart, edgeEnd);
      if (currentInside !== previousInside) {
        output.push(lineIntersection(previous, current, edgeStart, edgeEnd));
      }
      if (currentInside) {
        output.push(current);
      }
      previous = current;
    }
  }
  return output;
}

function inside(point: Vec2, edgeStart: Vec2, edgeEnd: Vec2): boolean {
  return cross2(
    edgeEnd.x - edgeStart.x,
    edgeEnd.y - edgeStart.y,
    point.x - edgeStart.x,
    point.y - edgeStart.y,
  ) >= -1e-10;
}

function lineIntersection(
  lineStart: Vec2,
  lineEnd: Vec2,
  edgeStart: Vec2,
  edgeEnd: Vec2,
): Vec2 {
  const lineX = lineEnd.x - lineStart.x;
  const lineY = lineEnd.y - lineStart.y;
  const edgeX = edgeEnd.x - edgeStart.x;
  const edgeY = edgeEnd.y - edgeStart.y;
  const denominator = cross2(lineX, lineY, edgeX, edgeY);
  if (Math.abs(denominator) <= 1e-12) {
    return lineEnd;
  }
  const amount = cross2(
    edgeStart.x - lineStart.x,
    edgeStart.y - lineStart.y,
    edgeX,
    edgeY,
  ) / denominator;
  return { x: lineStart.x + amount * lineX, y: lineStart.y + amount * lineY };
}

function polygonArea(polygon: Vec2[]): number {
  let doubledArea = 0;
  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index]!;
    const next = polygon[(index + 1) % polygon.length]!;
    doubledArea += current.x * next.y - next.x * current.y;
  }
  return Math.abs(doubledArea) / 2;
}

function polygonCentroid(polygon: Vec2[]): Vec2 {
  let x = 0;
  let y = 0;
  for (const point of polygon) {
    x += point.x;
    y += point.y;
  }
  return { x: x / polygon.length, y: y / polygon.length };
}

function cross2(leftX: number, leftY: number, rightX: number, rightY: number): number {
  return leftX * rightY - leftY * rightX;
}

function figureDiagonal(faces: TransformedFace[]): number {
  const bounds = new THREE.Box3();
  for (const face of faces) {
    for (const vertex of face.vertices) {
      bounds.expandByPoint(vertex);
    }
  }
  return bounds.getSize(new THREE.Vector3()).length();
}

function normalizeFacePair(
  left: FigureFaceRef,
  right: FigureFaceRef,
): [FigureFaceRef, FigureFaceRef] {
  return faceRefKey(left).localeCompare(faceRefKey(right)) <= 0
    ? [{ part: left.part, face: left.face }, { part: right.part, face: right.face }]
    : [{ part: right.part, face: right.face }, { part: left.part, face: left.face }];
}

function surfaceExceptionKey(exception: SurfaceExceptionSpec): string {
  return `${exception.rule}:${facePairKey(normalizeFacePair(...exception.faces))}`;
}

function facePairKey([left, right]: readonly [FigureFaceRef, FigureFaceRef]): string {
  return `${faceRefKey(left)}|${faceRefKey(right)}`;
}

function faceRefKey(face: FigureFaceRef): string {
  return `${face.part}.${face.face}`;
}

function vector(value: Vec3): THREE.Vector3 {
  return new THREE.Vector3(...value);
}
