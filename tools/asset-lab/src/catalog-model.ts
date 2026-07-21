import type { ClipRole, LocomotionKind } from "./dsl";

export interface AnimalCatalogClip {
  durationSeconds: number;
  fps?: number;
  label: string;
  locomotionKind?: LocomotionKind;
  loop: boolean;
  name: string;
  nextClip?: string;
  role: ClipRole;
}

export interface AnimalCatalogFigure {
  clipCount: number;
  clips: AnimalCatalogClip[];
  defaultClip: string;
  jsonPath: string;
  label: string;
  materialCount: number;
  name: string;
  partCount: number;
  runtimePromotion?: AnimalCatalogRuntimePromotion;
  semanticBytes: number;
  semanticSha256: string;
  textureCount: number;
  thumbnailPath: string;
}

export interface AnimalCatalogRuntimePromotion {
  figureId: string;
  jsonPath: string;
}

export interface AnimalCatalogDocument {
  catalogSha256: string;
  figures: AnimalCatalogFigure[];
  schemaVersion: 1;
  summary: {
    canonicalFigures: number;
    clips: number;
    parts: number;
    runtimePromotedFigures: number;
  };
}

export function parseAnimalCatalog(value: unknown, sourceLabel: string): AnimalCatalogDocument {
  if (!isRecord(value) || value.schemaVersion !== 1 || !Array.isArray(value.figures)) {
    throw new Error(`Expected '${sourceLabel}' to contain an animal catalogue schema-v1 document`);
  }
  if (!isSha256(value.catalogSha256) || !isCatalogSummary(value.summary)) {
    throw new Error(`Animal catalogue '${sourceLabel}' has invalid summary metadata`);
  }

  const figures = value.figures.map((figure, index) => parseCatalogFigure(figure, sourceLabel, index));
  if (value.summary.canonicalFigures !== figures.length) {
    throw new Error(
      `Animal catalogue '${sourceLabel}' expected ${value.summary.canonicalFigures} figures but contains ${figures.length}`,
    );
  }
  const names = new Set<string>();
  const runtimeFigureIds = new Set<string>();
  const runtimePaths = new Set<string>();
  for (const figure of figures) {
    if (names.has(figure.name)) {
      throw new Error(`Animal catalogue '${sourceLabel}' contains duplicate figure '${figure.name}'`);
    }
    names.add(figure.name);
    if (figure.runtimePromotion) {
      if (runtimeFigureIds.has(figure.runtimePromotion.figureId)) {
        throw new Error(
          `Animal catalogue '${sourceLabel}' contains duplicate runtime figure ID '${figure.runtimePromotion.figureId}'`,
        );
      }
      if (runtimePaths.has(figure.runtimePromotion.jsonPath)) {
        throw new Error(
          `Animal catalogue '${sourceLabel}' contains duplicate runtime path '${figure.runtimePromotion.jsonPath}'`,
        );
      }
      runtimeFigureIds.add(figure.runtimePromotion.figureId);
      runtimePaths.add(figure.runtimePromotion.jsonPath);
    }
  }
  if (value.summary.runtimePromotedFigures !== runtimeFigureIds.size) {
    throw new Error(
      `Animal catalogue '${sourceLabel}' expected ${value.summary.runtimePromotedFigures} runtime-promoted figures but contains ${runtimeFigureIds.size}`,
    );
  }

  return {
    catalogSha256: value.catalogSha256,
    figures,
    schemaVersion: 1,
    summary: value.summary,
  };
}

export function formatFigureLabel(name: string): string {
  return name
    .split(/[_-]+/u)
    .filter(Boolean)
    .map((part) => `${part.charAt(0).toUpperCase()}${part.slice(1)}`)
    .join(" ");
}

export function chooseDefaultClip(clipNames: readonly string[]): string {
  if (clipNames.length === 0) {
    throw new Error("Cannot choose a default from an empty clip list");
  }
  const priorities = ["walk", "fly", "swim", "run", "idle"];
  for (const priority of priorities) {
    if (clipNames.includes(priority)) {
      return priority;
    }
  }
  return [...clipNames].sort((left, right) => left.localeCompare(right))[0] as string;
}

function parseCatalogFigure(value: unknown, sourceLabel: string, index: number): AnimalCatalogFigure {
  if (
    !isRecord(value)
    || !isSafeName(value.name)
    || typeof value.label !== "string"
    || !isNonnegativeInteger(value.partCount)
    || !isNonnegativeInteger(value.materialCount)
    || !isNonnegativeInteger(value.textureCount)
    || !isNonnegativeInteger(value.clipCount)
    || !isPositiveInteger(value.semanticBytes)
    || !isSha256(value.semanticSha256)
    || !isRelativeArtifactPath(value.jsonPath, ".json")
    || !isRelativeArtifactPath(value.thumbnailPath, ".png")
    || !Array.isArray(value.clips)
    || !isSafeName(value.defaultClip)
  ) {
    throw new Error(`Animal catalogue '${sourceLabel}' has an invalid figure at index ${index}`);
  }
  const clips = value.clips.map((clip, clipIndex) => parseCatalogClip(clip, sourceLabel, index, clipIndex));
  if (value.clipCount !== clips.length || !clips.some((clip) => clip.name === value.defaultClip)) {
    throw new Error(`Animal catalogue '${sourceLabel}' has inconsistent clips for '${value.name}'`);
  }
  const clipNames = new Set(clips.map((clip) => clip.name));
  for (const clip of clips) {
    if (clip.nextClip !== undefined && !clipNames.has(clip.nextClip)) {
      throw new Error(
        `Animal catalogue '${sourceLabel}' clip '${clip.name}' references missing next clip '${clip.nextClip}'`,
      );
    }
    if (clip.nextClip !== undefined && clip.loop) {
      throw new Error(`Animal catalogue '${sourceLabel}' looping clip '${clip.name}' declares nextClip`);
    }
  }
  const runtimePromotion = value.runtimePromotion === undefined
    ? undefined
    : parseRuntimePromotion(value.runtimePromotion, sourceLabel, index);
  return {
    clipCount: value.clipCount,
    clips,
    defaultClip: value.defaultClip,
    jsonPath: value.jsonPath,
    label: value.label,
    materialCount: value.materialCount,
    name: value.name,
    partCount: value.partCount,
    ...(runtimePromotion === undefined ? {} : { runtimePromotion }),
    semanticBytes: value.semanticBytes,
    semanticSha256: value.semanticSha256,
    textureCount: value.textureCount,
    thumbnailPath: value.thumbnailPath,
  };
}

function parseRuntimePromotion(
  value: unknown,
  sourceLabel: string,
  figureIndex: number,
): AnimalCatalogRuntimePromotion {
  if (
    !isRecord(value)
    || typeof value.figureId !== "string"
    || !/^[a-z0-9._-]+:[a-z0-9._/-]+$/u.test(value.figureId)
    || typeof value.jsonPath !== "string"
    || !value.jsonPath.startsWith("assets/mclone/figures/")
    || !isRelativeArtifactPath(value.jsonPath, ".figure.json")
  ) {
    throw new Error(
      `Animal catalogue '${sourceLabel}' has invalid runtime promotion metadata at figure ${figureIndex}`,
    );
  }
  return { figureId: value.figureId, jsonPath: value.jsonPath };
}

function parseCatalogClip(
  value: unknown,
  sourceLabel: string,
  figureIndex: number,
  clipIndex: number,
): AnimalCatalogClip {
  if (
    !isRecord(value)
    || !isSafeName(value.name)
    || typeof value.loop !== "boolean"
    || typeof value.durationSeconds !== "number"
    || !Number.isFinite(value.durationSeconds)
    || value.durationSeconds <= 0
    || (value.fps !== undefined && (typeof value.fps !== "number" || !Number.isFinite(value.fps) || value.fps <= 0))
    || (value.locomotionKind !== undefined && !isLocomotionKind(value.locomotionKind))
    || (value.label !== undefined && (typeof value.label !== "string" || value.label.trim() === ""))
    || (value.role !== undefined && !isClipRole(value.role))
    || (value.nextClip !== undefined && !isSafeName(value.nextClip))
  ) {
    throw new Error(
      `Animal catalogue '${sourceLabel}' has an invalid clip at figure ${figureIndex}, clip ${clipIndex}`,
    );
  }
  const locomotionKind = value.locomotionKind as LocomotionKind | undefined;
  const role = isClipRole(value.role)
    ? value.role
    : inferClipRole(value.name, locomotionKind);
  const label = typeof value.label === "string" && value.label.trim()
    ? value.label
    : formatFigureLabel(value.name);
  return {
    durationSeconds: value.durationSeconds,
    ...(value.fps === undefined ? {} : { fps: value.fps }),
    label,
    ...(locomotionKind === undefined ? {} : { locomotionKind }),
    loop: value.loop,
    name: value.name,
    ...(value.nextClip === undefined ? {} : { nextClip: value.nextClip as string }),
    role,
  };
}

export function inferClipRole(name: string, locomotionKind?: LocomotionKind): ClipRole {
  if (locomotionKind !== undefined) {
    return "locomotion";
  }
  return name === "idle" || name.startsWith("idle_") ? "idle" : "action";
}

function isCatalogSummary(value: unknown): value is AnimalCatalogDocument["summary"] {
  return isRecord(value)
    && isNonnegativeInteger(value.canonicalFigures)
    && isNonnegativeInteger(value.clips)
    && isNonnegativeInteger(value.parts)
    && isNonnegativeInteger(value.runtimePromotedFigures);
}

function isLocomotionKind(value: unknown): value is LocomotionKind {
  return value === "biped-walk"
    || value === "quadruped-walk"
    || value === "slither"
    || value === "swim"
    || value === "wing-flap";
}

function isClipRole(value: unknown): value is ClipRole {
  return value === "locomotion" || value === "idle" || value === "action";
}

function isSafeName(value: unknown): value is string {
  return typeof value === "string" && /^[a-z0-9][a-z0-9_-]*$/u.test(value);
}

function isRelativeArtifactPath(value: unknown, extension: string): value is string {
  return typeof value === "string"
    && !value.startsWith("/")
    && !value.includes("..")
    && value.endsWith(extension);
}

function isSha256(value: unknown): value is string {
  return typeof value === "string" && /^[a-f0-9]{64}$/u.test(value);
}

function isNonnegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0;
}

function isPositiveInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value > 0;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
