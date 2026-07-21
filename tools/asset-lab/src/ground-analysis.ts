import * as THREE from "three";
import type { FigureAsset, GeometryExceptionSpec, PartSpec } from "./dsl";
import {
  evaluateFigurePose,
  figureAnalysisPoses,
  type FigurePose,
} from "./figure-pose";
import { GROUND_BASELINE } from "./ground-baseline";

export const GROUND_PENETRATION_RULE = "ground-penetration" as const;

export interface FigureGroundIssue {
  rule: typeof GROUND_PENETRATION_RULE;
  part: string;
  pose: FigurePose;
  bottomY: number;
  groundY: number;
  penetration: number;
  tolerance: number;
  occurrences: number;
}

export interface FigureGroundAnalysisOptions {
  groundY?: number;
  poses?: FigurePose[];
  tolerance?: number;
}

export interface AcknowledgedFigureGroundIssue {
  issue: FigureGroundIssue;
  reason: string;
}

export interface FigureGroundEvaluation {
  acknowledged: AcknowledgedFigureGroundIssue[];
  baseline: FigureGroundIssue[];
  unacknowledged: FigureGroundIssue[];
  staleExceptions: GeometryExceptionSpec[];
  staleBaseline: string[];
}

const DEFAULT_GROUND_Y = 0;
const DEFAULT_TOLERANCE = 0.02;
const LAND_LOCOMOTION_KINDS = new Set(["biped-walk", "quadruped-walk", "slither"]);

/**
 * Finds non-contact box geometry that passes below the authoring ground plane
 * in rest, land-locomotion, idle, or action poses. Swim and flight clips are
 * excluded even when the same figure also has a land clip.
 */
export function analyzeFigureGrounding(
  asset: FigureAsset,
  options: FigureGroundAnalysisOptions = {},
): FigureGroundIssue[] {
  if (!hasLandLocomotion(asset)) {
    return [];
  }
  const groundY = options.groundY ?? DEFAULT_GROUND_Y;
  const tolerance = options.tolerance ?? DEFAULT_TOLERANCE;
  const poses = options.poses ?? groundAnalysisPoses(asset);
  const supportParts = landContactParts(asset);
  const issues = new Map<string, FigureGroundIssue>();

  for (const pose of poses) {
    const matrices = evaluateFigurePose(asset, pose);
    for (const part of asset.parts) {
      if (part.primitive.kind !== "box" || supportParts.has(part.name)) {
        continue;
      }
      const bottomY = transformedBoxBottom(part, matrices.get(part.name)!);
      if (bottomY >= groundY - tolerance) {
        continue;
      }
      const current = issues.get(part.name);
      if (!current) {
        issues.set(part.name, {
          rule: GROUND_PENETRATION_RULE,
          part: part.name,
          pose,
          bottomY,
          groundY,
          penetration: groundY - bottomY,
          tolerance,
          occurrences: 1,
        });
        continue;
      }
      current.occurrences += 1;
      if (bottomY < current.bottomY) {
        current.pose = pose;
        current.bottomY = bottomY;
        current.penetration = groundY - bottomY;
      }
    }
  }
  return [...issues.values()].sort((left, right) =>
    groundIssueKey(left).localeCompare(groundIssueKey(right))
  );
}

export function evaluateFigureGrounding(asset: FigureAsset): FigureGroundEvaluation {
  const remainingExceptions = (asset.geometryExceptions ?? [])
    .filter((exception) => exception.rule === GROUND_PENETRATION_RULE);
  const remainingBaseline = [...(GROUND_BASELINE[asset.name] ?? [])];
  const acknowledged: AcknowledgedFigureGroundIssue[] = [];
  const baseline: FigureGroundIssue[] = [];
  const unacknowledged: FigureGroundIssue[] = [];
  for (const issue of analyzeFigureGrounding(asset)) {
    const key = groundIssueKey(issue);
    const exceptionIndex = remainingExceptions.findIndex((exception) =>
      exception.parts.length === 1 && exception.parts[0] === issue.part
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

export function assertFigureGrounding(asset: FigureAsset): void {
  const evaluation = evaluateFigureGrounding(asset);
  if (
    evaluation.unacknowledged.length === 0
    && evaluation.staleExceptions.length === 0
    && evaluation.staleBaseline.length === 0
  ) {
    return;
  }
  const errors = evaluation.unacknowledged.map((issue) =>
    `${formatGroundIssue(issue)}; raise or re-pose the part, declare it as a locomotion contact when it is genuinely load-bearing, or add geometryException({ rule: "${GROUND_PENETRATION_RULE}", parts: ["${issue.part}"], reason: "..." })`
  );
  errors.push(...evaluation.staleExceptions.map((exception) =>
    `stale ground exception for parts ${JSON.stringify(exception.parts)}; remove it or update it to the current penetrating part`
  ));
  errors.push(...evaluation.staleBaseline.map((key) =>
    `stale ground baseline '${key}'; remove it from ground-baseline.ts to ratchet the catalog`
  ));
  throw new Error(
    `Figure '${asset.name}' failed ground analysis:\n${errors.map((error) => `- ${error}`).join("\n")}`,
  );
}

export function formatGroundIssue(issue: FigureGroundIssue): string {
  const pose = issue.pose.clipName === undefined
    ? "rest pose"
    : `clip '${issue.pose.clipName}' at ${issue.pose.time.toFixed(3)}s`;
  return `${issue.rule} part '${issue.part}' reaches y=${issue.bottomY.toFixed(3)} in ${pose}`
    + ` (${issue.penetration.toFixed(3)} below ground y=${issue.groundY.toFixed(3)}, allowed ${issue.tolerance.toFixed(3)}, ${issue.occurrences} sampled pose${issue.occurrences === 1 ? "" : "s"})`;
}

export function groundIssueKey(issue: FigureGroundIssue): string {
  return `${issue.rule}:${issue.part}`;
}

function hasLandLocomotion(asset: FigureAsset): boolean {
  return Object.values(asset.clips).some((clip) =>
    clip.locomotion !== undefined && LAND_LOCOMOTION_KINDS.has(clip.locomotion.kind)
  );
}

function groundAnalysisPoses(asset: FigureAsset): FigurePose[] {
  return figureAnalysisPoses(asset).filter((pose) => {
    if (pose.clipName === undefined) {
      return true;
    }
    const locomotion = asset.clips[pose.clipName]?.locomotion;
    return locomotion === undefined || LAND_LOCOMOTION_KINDS.has(locomotion.kind);
  });
}

function landContactParts(asset: FigureAsset): Set<string> {
  return new Set(
    Object.values(asset.clips).flatMap((clip) =>
      clip.locomotion !== undefined && LAND_LOCOMOTION_KINDS.has(clip.locomotion.kind)
        ? (clip.locomotion.contacts ?? []).map((contact) => contact.part)
        : []
    ),
  );
}

function transformedBoxBottom(part: PartSpec, matrix: THREE.Matrix4): number {
  if (part.primitive.kind !== "box") {
    return Number.POSITIVE_INFINITY;
  }
  const half = part.primitive.size.map((value) => value / 2);
  let bottomY = Number.POSITIVE_INFINITY;
  for (const x of [-half[0]!, half[0]!]) {
    for (const y of [-half[1]!, half[1]!]) {
      for (const z of [-half[2]!, half[2]!]) {
        bottomY = Math.min(bottomY, new THREE.Vector3(x, y, z).applyMatrix4(matrix).y);
      }
    }
  }
  return bottomY;
}
