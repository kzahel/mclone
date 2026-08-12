import {
  FIGURE_ALPHA_MODES,
  type ClipRole,
  type FigureAlphaMode,
  type LocomotionKind,
} from "./dsl";
import {
  cloneCreatureMetadata,
  type CreatureMetadata,
  validateCreatureMetadata,
} from "./creature-metadata";
import {
  SEMANTIC_ASSET_ANCHORS,
  SEMANTIC_ASSET_USES,
  SEMANTIC_INSTANTIATION_STATUSES,
  semanticAssetAnchorForUse,
  type SemanticAssetAnchor,
  type SemanticAssetUse,
  type SemanticInstantiationStatus,
} from "./semantic-assets";

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
  alphaModes: FigureAlphaMode[];
  anchor: SemanticAssetAnchor;
  clipCount: number;
  clips: AnimalCatalogClip[];
  defaultClip?: string;
  jsonPath: string;
  label: string;
  materialCount: number;
  metadata?: CreatureMetadata;
  name: string;
  partCount: number;
  runtimePromotion?: AnimalCatalogRuntimePromotion;
  semanticBytes: number;
  semanticSha256: string;
  textureCount: number;
  thumbnailPath: string;
  use: SemanticAssetUse;
}

export interface AnimalCatalogRuntimePromotion {
  anchor: SemanticAssetAnchor;
  assetId: string;
  instantiation: SemanticInstantiationStatus;
  jsonPath: string;
  use: SemanticAssetUse;
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
    runtimePromotedProps: number;
    liveInstantiatedProps: number;
    semanticPropParts: number;
    semanticProps: number;
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
  const actorFigures = figures.filter((figure) => figure.use === "actor");
  const semanticProps = figures.filter((figure) => figure.use !== "actor");
  if (
    value.summary.canonicalFigures !== actorFigures.length
    || value.summary.semanticProps !== semanticProps.length
  ) {
    throw new Error(
      `Animal catalogue '${sourceLabel}' figure/prop summary does not match its ${figures.length} entries`,
    );
  }
  const names = new Set<string>();
  const runtimeAssetIds = new Set<string>();
  const runtimePaths = new Set<string>();
  for (const figure of figures) {
    if (names.has(figure.name)) {
      throw new Error(`Animal catalogue '${sourceLabel}' contains duplicate figure '${figure.name}'`);
    }
    names.add(figure.name);
    if (figure.runtimePromotion) {
      if (runtimeAssetIds.has(figure.runtimePromotion.assetId)) {
        throw new Error(
          `Animal catalogue '${sourceLabel}' contains duplicate runtime asset ID '${figure.runtimePromotion.assetId}'`,
        );
      }
      if (runtimePaths.has(figure.runtimePromotion.jsonPath)) {
        throw new Error(
          `Animal catalogue '${sourceLabel}' contains duplicate runtime path '${figure.runtimePromotion.jsonPath}'`,
        );
      }
      runtimeAssetIds.add(figure.runtimePromotion.assetId);
      runtimePaths.add(figure.runtimePromotion.jsonPath);
    }
  }
  const runtimePromotedFigures = actorFigures.filter((figure) => figure.runtimePromotion !== undefined).length;
  const runtimePromotedProps = semanticProps.filter((figure) => figure.runtimePromotion !== undefined).length;
  const liveInstantiatedProps = semanticProps.filter(
    (figure) => figure.runtimePromotion?.instantiation === "live_gameplay",
  ).length;
  if (
    value.summary.runtimePromotedFigures !== runtimePromotedFigures
    || value.summary.runtimePromotedProps !== runtimePromotedProps
    || value.summary.liveInstantiatedProps !== liveInstantiatedProps
  ) {
    throw new Error(
      `Animal catalogue '${sourceLabel}' runtime-promotion summary does not match its entries`,
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
    || !Array.isArray(value.alphaModes)
    || value.alphaModes.length === 0
    || !value.alphaModes.every(isFigureAlphaMode)
    || !isSemanticAssetUse(value.use)
    || !isSemanticAssetAnchor(value.anchor)
    || semanticAssetAnchorForUse(value.use) !== value.anchor
    || (value.defaultClip !== undefined && !isSafeName(value.defaultClip))
  ) {
    throw new Error(`Animal catalogue '${sourceLabel}' has an invalid figure at index ${index}`);
  }
  const clips = value.clips.map((clip, clipIndex) => parseCatalogClip(clip, sourceLabel, index, clipIndex));
  if (
    value.clipCount !== clips.length
    || (clips.length === 0 && value.defaultClip !== undefined)
    || (clips.length > 0 && !clips.some((clip) => clip.name === value.defaultClip))
  ) {
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
  if (
    runtimePromotion !== undefined
    && (runtimePromotion.use !== value.use || runtimePromotion.anchor !== value.anchor)
  ) {
    throw new Error(
      `Animal catalogue '${sourceLabel}' promotion use/anchor disagrees with '${value.name}'`,
    );
  }
  const metadataErrors = value.use === "actor"
    ? validateCreatureMetadata(value.metadata, `figure ${index} metadata`)
    : value.metadata === undefined
      ? []
      : [`figure ${index} metadata must be absent for semantic props`];
  if (metadataErrors.length > 0) {
    throw new Error(
      `Animal catalogue '${sourceLabel}' has invalid creature metadata for '${value.name}':\n`
        + metadataErrors.map((error) => `- ${error}`).join("\n"),
    );
  }
  return {
    alphaModes: [...value.alphaModes] as FigureAlphaMode[],
    anchor: value.anchor,
    clipCount: value.clipCount,
    clips,
    ...(value.defaultClip === undefined ? {} : { defaultClip: value.defaultClip }),
    jsonPath: value.jsonPath,
    label: value.label,
    materialCount: value.materialCount,
    ...(value.use === "actor"
      ? { metadata: cloneCreatureMetadata(value.metadata as CreatureMetadata) }
      : {}),
    name: value.name,
    partCount: value.partCount,
    ...(runtimePromotion === undefined ? {} : { runtimePromotion }),
    semanticBytes: value.semanticBytes,
    semanticSha256: value.semanticSha256,
    textureCount: value.textureCount,
    thumbnailPath: value.thumbnailPath,
    use: value.use,
  };
}

function parseRuntimePromotion(
  value: unknown,
  sourceLabel: string,
  figureIndex: number,
): AnimalCatalogRuntimePromotion {
  if (
    !isRecord(value)
    || typeof value.assetId !== "string"
    || !/^[a-z0-9._-]+:[a-z0-9._/-]+$/u.test(value.assetId)
    || typeof value.jsonPath !== "string"
    || !value.jsonPath.startsWith("assets/mclone/figures/")
    || !isRelativeArtifactPath(value.jsonPath, ".figure.json")
    || !isSemanticAssetUse(value.use)
    || !isSemanticAssetAnchor(value.anchor)
    || semanticAssetAnchorForUse(value.use) !== value.anchor
    || !isSemanticInstantiationStatus(value.instantiation)
  ) {
    throw new Error(
      `Animal catalogue '${sourceLabel}' has invalid runtime promotion metadata at figure ${figureIndex}`,
    );
  }
  return {
    anchor: value.anchor,
    assetId: value.assetId,
    instantiation: value.instantiation,
    jsonPath: value.jsonPath,
    use: value.use,
  };
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
    && isNonnegativeInteger(value.runtimePromotedFigures)
    && isNonnegativeInteger(value.runtimePromotedProps)
    && isNonnegativeInteger(value.liveInstantiatedProps)
    && isNonnegativeInteger(value.semanticPropParts)
    && isNonnegativeInteger(value.semanticProps);
}

function isSemanticAssetUse(value: unknown): value is SemanticAssetUse {
  return typeof value === "string" && (SEMANTIC_ASSET_USES as readonly string[]).includes(value);
}

function isSemanticAssetAnchor(value: unknown): value is SemanticAssetAnchor {
  return typeof value === "string" && (SEMANTIC_ASSET_ANCHORS as readonly string[]).includes(value);
}

function isSemanticInstantiationStatus(value: unknown): value is SemanticInstantiationStatus {
  return typeof value === "string"
    && (SEMANTIC_INSTANTIATION_STATUSES as readonly string[]).includes(value);
}

function isLocomotionKind(value: unknown): value is LocomotionKind {
  return value === "biped-walk"
    || value === "quadruped-walk"
    || value === "slither"
    || value === "swim"
    || value === "wing-flap";
}

function isFigureAlphaMode(value: unknown): value is FigureAlphaMode {
  return typeof value === "string"
    && (FIGURE_ALPHA_MODES as readonly string[]).includes(value);
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
