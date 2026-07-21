import * as THREE from "three";
import { OBB } from "three/examples/jsm/math/OBB.js";
import type {
  FigureAsset,
  GeometryExceptionSpec,
  PartSpec,
  Vec3,
} from "./dsl";

export const DISCONNECTED_COMPONENT_RULE = "disconnected-component" as const;

export interface FigureGeometryIssue {
  rule: typeof DISCONNECTED_COMPONENT_RULE;
  parts: string[];
  nearestPart: string;
  gap: number;
  threshold: number;
}

export interface FigureGeometryAnalysisOptions {
  /** World-space AABB gap below which nearby authored boxes count as attached. */
  connectionGapThreshold?: number;
}

export interface AcknowledgedFigureGeometryIssue {
  issue: FigureGeometryIssue;
  reason: string;
}

export interface FigureGeometryEvaluation {
  acknowledged: AcknowledgedFigureGeometryIssue[];
  unacknowledged: FigureGeometryIssue[];
  staleExceptions: GeometryExceptionSpec[];
}

const DEFAULT_CONNECTION_GAP_THRESHOLD = 0.06;

/**
 * Finds rest-pose box components that do not connect to the figure's largest
 * component. This is intentionally a bounded authoring check: transformed
 * oriented boxes catch conspicuous floating pieces without treating the rig
 * parent as proof of attachment or pretending to provide general collision.
 */
export function analyzeFigureGeometry(
  asset: FigureAsset,
  options: FigureGeometryAnalysisOptions = {},
): FigureGeometryIssue[] {
  const threshold = options.connectionGapThreshold ?? DEFAULT_CONNECTION_GAP_THRESHOLD;
  const parts = new Map(asset.parts.map((part) => [part.name, part] as const));
  const contentMatrices = new Map<string, THREE.Matrix4>();
  const bounds = new Map<string, OBB>();
  const visiting = new Set<string>();

  const resolveContentMatrix = (part: PartSpec): THREE.Matrix4 => {
    const cached = contentMatrices.get(part.name);
    if (cached) {
      return cached;
    }
    if (visiting.has(part.name)) {
      return new THREE.Matrix4();
    }
    visiting.add(part.name);
    const parent = part.parent === undefined ? undefined : parts.get(part.parent);
    const parentMatrix = parent === undefined
      ? new THREE.Matrix4()
      : resolveContentMatrix(parent);
    const pivot = vector(part.joint?.pivot ?? part.pivot);
    const position = vector(part.at).add(pivot);
    const rotation = euler(part.rot);
    const groupMatrix = new THREE.Matrix4().compose(
      position,
      new THREE.Quaternion().setFromEuler(rotation),
      new THREE.Vector3(1, 1, 1),
    );
    const contentMatrix = parentMatrix.clone()
      .multiply(groupMatrix)
      .multiply(new THREE.Matrix4().makeTranslation(-pivot.x, -pivot.y, -pivot.z));
    contentMatrices.set(part.name, contentMatrix);
    visiting.delete(part.name);
    return contentMatrix;
  };

  for (const part of asset.parts) {
    if (part.primitive.kind !== "box") {
      continue;
    }
    bounds.set(
      part.name,
      transformedBox(part.primitive.size, resolveContentMatrix(part)),
    );
  }

  const names = [...bounds.keys()];
  if (names.length <= 1) {
    return [];
  }
  const adjacency = new Map(names.map((name) => [name, new Set<string>()] as const));
  for (let leftIndex = 0; leftIndex < names.length; leftIndex += 1) {
    for (let rightIndex = leftIndex + 1; rightIndex < names.length; rightIndex += 1) {
      const leftName = names[leftIndex]!;
      const rightName = names[rightIndex]!;
      const gap = boxGap(bounds.get(leftName)!, bounds.get(rightName)!);
      if (gap <= threshold) {
        adjacency.get(leftName)!.add(rightName);
        adjacency.get(rightName)!.add(leftName);
      }
    }
  }

  const components: string[][] = [];
  const remaining = new Set(names);
  while (remaining.size > 0) {
    const first = remaining.values().next().value as string;
    const component: string[] = [];
    const pending = [first];
    remaining.delete(first);
    while (pending.length > 0) {
      const name = pending.pop()!;
      component.push(name);
      for (const neighbor of adjacency.get(name) ?? []) {
        if (remaining.delete(neighbor)) {
          pending.push(neighbor);
        }
      }
    }
    component.sort();
    components.push(component);
  }
  if (components.length === 1) {
    return [];
  }

  components.sort((left, right) => componentVolume(right, bounds) - componentVolume(left, bounds));
  const main = new Set(components[0]!);
  return components.slice(1).map((component) => {
    let nearestPart = "";
    let nearestGap = Number.POSITIVE_INFINITY;
    for (const partName of component) {
      for (const mainPartName of main) {
        const gap = boxGap(bounds.get(partName)!, bounds.get(mainPartName)!);
        if (gap < nearestGap) {
          nearestGap = gap;
          nearestPart = mainPartName;
        }
      }
    }
    return {
      rule: DISCONNECTED_COMPONENT_RULE,
      parts: component,
      nearestPart,
      gap: nearestGap,
      threshold,
    };
  });
}

export function evaluateFigureGeometry(asset: FigureAsset): FigureGeometryEvaluation {
  const issues = analyzeFigureGeometry(asset);
  const remainingExceptions = [...(asset.geometryExceptions ?? [])];
  const acknowledged: AcknowledgedFigureGeometryIssue[] = [];
  const unacknowledged: FigureGeometryIssue[] = [];
  for (const issue of issues) {
    const exceptionIndex = remainingExceptions.findIndex((exception) =>
      exception.rule === issue.rule && samePartSet(exception.parts, issue.parts)
    );
    if (exceptionIndex < 0) {
      unacknowledged.push(issue);
      continue;
    }
    const [exception] = remainingExceptions.splice(exceptionIndex, 1);
    acknowledged.push({ issue, reason: exception!.reason.trim() });
  }
  return {
    acknowledged,
    unacknowledged,
    staleExceptions: remainingExceptions,
  };
}

export function assertFigureGeometry(asset: FigureAsset): void {
  const evaluation = evaluateFigureGeometry(asset);
  if (evaluation.unacknowledged.length === 0 && evaluation.staleExceptions.length === 0) {
    return;
  }
  const errors = evaluation.unacknowledged.map((issue) =>
    `${formatIssue(issue)}; connect the geometry or add geometryException({ rule: "${issue.rule}", parts: ${JSON.stringify(issue.parts)}, reason: "..." })`
  );
  errors.push(...evaluation.staleExceptions.map((exception) =>
    `stale geometry exception for ${exception.rule} parts ${JSON.stringify([...exception.parts].sort())}; remove it or update it to the current component`
  ));
  throw new Error(
    `Figure '${asset.name}' failed geometry analysis:\n${errors.map((error) => `- ${error}`).join("\n")}`,
  );
}

export function formatIssue(issue: FigureGeometryIssue): string {
  return `${issue.rule} parts [${issue.parts.join(", ")}] are ${issue.gap.toFixed(3)} units from nearest main part '${issue.nearestPart}' (allowed ${issue.threshold.toFixed(3)})`;
}

function componentVolume(component: readonly string[], bounds: ReadonlyMap<string, OBB>): number {
  return component.reduce((total, name) => {
    const size = bounds.get(name)!.getSize(new THREE.Vector3());
    return total + size.x * size.y * size.z;
  }, 0);
}

function samePartSet(left: readonly string[], right: readonly string[]): boolean {
  if (left.length !== right.length) {
    return false;
  }
  const rightParts = new Set(right);
  return left.every((part) => rightParts.has(part));
}

function transformedBox(size: Vec3, matrix: THREE.Matrix4): OBB {
  const half = new THREE.Vector3(size[0] / 2, size[1] / 2, size[2] / 2);
  return new OBB(new THREE.Vector3(), half).applyMatrix4(matrix);
}

function boxGap(left: OBB, right: OBB): number {
  if (left.intersectsOBB(right)) {
    return 0;
  }
  let lower = 0;
  let upper = 0.01;
  while (upper < 16 && !expandedBoxesIntersect(left, right, upper)) {
    upper *= 2;
  }
  for (let iteration = 0; iteration < 18; iteration += 1) {
    const middle = (lower + upper) / 2;
    if (expandedBoxesIntersect(left, right, middle)) {
      upper = middle;
    } else {
      lower = middle;
    }
  }
  return upper;
}

function expandedBoxesIntersect(left: OBB, right: OBB, totalMargin: number): boolean {
  const perBoxMargin = totalMargin / 2;
  const expandedLeft = left.clone();
  const expandedRight = right.clone();
  expandedLeft.halfSize.addScalar(perBoxMargin);
  expandedRight.halfSize.addScalar(perBoxMargin);
  return expandedLeft.intersectsOBB(expandedRight);
}

function vector(value: Vec3 | undefined): THREE.Vector3 {
  return new THREE.Vector3(...(value ?? [0, 0, 0]));
}

function euler(value: Vec3 | undefined): THREE.Euler {
  const [x, y, z] = value ?? [0, 0, 0];
  return new THREE.Euler(
    THREE.MathUtils.degToRad(x),
    THREE.MathUtils.degToRad(y),
    THREE.MathUtils.degToRad(z),
    "XYZ",
  );
}
