import { statSync, type Dirent } from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";
import type { TextureCandidateEntry, TextureCandidateSource, TextureImageRef } from "./index-model";

const LEGACY_TEXTURE_LAB_OUTPUT_ROOT = "/tmp/mclone-texture-lab";

interface CandidateDiscovery {
  candidates: TextureCandidateEntry[];
  warnings: string[];
}

export interface DiscoverTextureCandidatesOptions {
  textureNames?: string[];
}

interface DiffusionManifest {
  candidates?: DiffusionManifestCandidate[];
  contact_sheet_png?: string;
  input?: {
    path?: string;
    prepared_png?: string;
  };
  model_id?: string;
  negative_prompt?: string;
  prompt?: string;
  prompt_preset?: string;
  scheduler?: string;
  steps?: number;
}

interface DiffusionManifestCandidate {
  id?: string;
  png?: string;
  seed?: number;
  strength?: number;
  tile3x3_png?: string;
}

interface ProjectionSummary {
  sourceManifest?: string;
  texture?: string;
  paletteColors?: string[];
  candidates?: ProjectionSummaryCandidate[];
}

interface ProjectionSummaryCandidate {
  id?: string;
  codename?: string;
  reports?: ProjectionSummaryReport[];
}

interface ProjectionSummaryReport {
  resolution?: number;
  status?: string;
  score?: number;
  reasons?: string[];
  png?: string;
  mask?: string;
}

interface ArchiveManifest {
  texture?: string;
  palette?: {
    colors?: string[];
  };
  diffusion?: {
    manifest?: ArchiveFileRef;
    modelId?: string;
    promptPreset?: string;
    prompt?: string;
    negativePrompt?: string;
    scheduler?: string;
    steps?: number;
  };
  projectionSummary?: ArchiveFileRef;
  reviewSheet?: ArchiveFileRef;
  candidates?: ArchiveManifestCandidate[];
}

interface ArchiveManifestCandidate {
  id?: string;
  codename?: string;
  seed?: number;
  strength?: number;
  raw?: ArchiveFileRef;
  rawTile?: ArchiveFileRef;
  projectionReport?: ArchiveFileRef;
  resolutions?: ArchiveResolution[];
}

interface ArchiveResolution {
  resolution?: number;
  png?: ArchiveFileRef;
  triage?: {
    status?: string;
    score?: number;
    reasons?: string[];
  };
}

interface ArchiveFileRef {
  path?: string;
}

export async function discoverTextureCandidates(
  outputRoot: string,
  options: DiscoverTextureCandidatesOptions = {},
): Promise<CandidateDiscovery> {
  const warnings: string[] = [];
  const candidates: TextureCandidateEntry[] = [];
  const textureNames = [...(options.textureNames ?? [])].sort((left, right) => right.length - left.length);
  for (const manifestPath of await findNamedFiles(path.join(outputRoot, "diffusion"), "manifest.json")) {
    await pushDiscovered(warnings, candidates, () => candidatesFromDiffusionManifest(manifestPath, outputRoot, textureNames));
  }
  for (const summaryPath of await findNamedFiles(path.join(outputRoot, "diffusion-projection"), "projection-summary.json")) {
    await pushDiscovered(warnings, candidates, () => candidatesFromProjectionSummary(summaryPath, outputRoot, textureNames));
  }
  for (const archivePath of await findNamedFiles(path.join(outputRoot, "diffusion-archive"), "archive-manifest.json")) {
    await pushDiscovered(warnings, candidates, () => candidatesFromArchiveManifest(archivePath, outputRoot));
  }
  candidates.sort(compareCandidates);
  return { candidates, warnings };
}

async function pushDiscovered(
  warnings: string[],
  candidates: TextureCandidateEntry[],
  discover: () => Promise<TextureCandidateEntry[]>,
): Promise<void> {
  try {
    candidates.push(...await discover());
  } catch (error) {
    warnings.push(error instanceof Error ? error.message : String(error));
  }
}

async function candidatesFromDiffusionManifest(
  manifestPath: string,
  outputRoot: string,
  textureNames: string[],
): Promise<TextureCandidateEntry[]> {
  const manifest = await readJson<DiffusionManifest>(manifestPath);
  const manifestDir = path.dirname(manifestPath);
  const artifactRoot = manifestDir;
  const contactSheet = resolveArtifactPath(manifest.contact_sheet_png, manifestDir, outputRoot);
  const textureName = inferTextureName(manifest.input?.path, manifest.prompt_preset, textureNames);
  const records: TextureCandidateEntry[] = [];
  for (const candidate of manifest.candidates ?? []) {
    if (!candidate.id) {
      continue;
    }
    const codename = candidateCodenameFrom(candidate.id, candidate.seed ?? null, candidate.strength ?? null, manifest.prompt_preset);
    records.push({
      id: candidateKey("diffusion", manifestPath, candidate.id, null),
      candidateId: candidate.id,
      codename,
      source: "diffusion",
      textureName,
      artifactRoot,
      manifestPath,
      projectionReportPath: null,
      archivePath: null,
      archived: false,
      promptPreset: manifest.prompt_preset ?? null,
      prompt: manifest.prompt ?? null,
      negativePrompt: manifest.negative_prompt ?? null,
      modelId: manifest.model_id ?? null,
      scheduler: manifest.scheduler ?? null,
      steps: manifest.steps ?? null,
      seed: candidate.seed ?? seedFromCandidateId(candidate.id) ?? null,
      strength: candidate.strength ?? strengthFromCandidateId(candidate.id) ?? null,
      resolution: null,
      resolutions: [],
      paletteColors: [],
      status: null,
      score: null,
      reasons: [],
      images: {
        raw: await imageRef("Raw", resolveArtifactPath(candidate.png, manifestDir, outputRoot)),
        rawTile: await imageRef("Raw 3x3", resolveArtifactPath(candidate.tile3x3_png, manifestDir, outputRoot)),
        projected: await imageRef("Projected", null),
        contactSheet: await imageRef("Contact sheet", contactSheet),
        reviewSheet: await imageRef("Review sheet", null),
      },
    });
  }
  return records;
}

async function candidatesFromProjectionSummary(
  summaryPath: string,
  outputRoot: string,
  textureNames: string[],
): Promise<TextureCandidateEntry[]> {
  const summary = await readJson<ProjectionSummary>(summaryPath);
  const summaryDir = path.dirname(summaryPath);
  const sourceManifestPath = resolveArtifactPath(summary.sourceManifest, summaryDir, outputRoot);
  const sourceManifest = sourceManifestPath ? await readOptionalJson<DiffusionManifest>(sourceManifestPath) : null;
  const sourceById = new Map((sourceManifest?.candidates ?? []).filter((candidate) => candidate.id).map((candidate) => [candidate.id!, candidate]));
  const reviewSheet = await findReviewSheet(summaryDir);
  const records: TextureCandidateEntry[] = [];
  for (const candidate of summary.candidates ?? []) {
    if (!candidate.id) {
      continue;
    }
    const reports = (candidate.reports ?? []).filter((report) => Number.isFinite(report.resolution));
    const primary = [...reports].sort((left, right) => (right.resolution ?? 0) - (left.resolution ?? 0))[0] ?? null;
    const sourceCandidate = sourceById.get(candidate.id);
    const codename = candidate.codename ?? candidateCodenameFrom(
      candidate.id,
      sourceCandidate?.seed ?? null,
      sourceCandidate?.strength ?? null,
      sourceManifest?.prompt_preset,
    );
    records.push({
      id: candidateKey("projection", summaryPath, candidate.id, primary?.resolution ?? null),
      candidateId: candidate.id,
      codename,
      source: "projection",
      textureName: summary.texture ?? inferTextureName(sourceManifest?.input?.path, sourceManifest?.prompt_preset, textureNames),
      artifactRoot: summaryDir,
      manifestPath: sourceManifestPath,
      projectionReportPath: resolveArtifactPath(projectionReportPath(summaryDir, candidate.id), summaryDir, outputRoot),
      archivePath: null,
      archived: false,
      promptPreset: sourceManifest?.prompt_preset ?? null,
      prompt: sourceManifest?.prompt ?? null,
      negativePrompt: sourceManifest?.negative_prompt ?? null,
      modelId: sourceManifest?.model_id ?? null,
      scheduler: sourceManifest?.scheduler ?? null,
      steps: sourceManifest?.steps ?? null,
      seed: sourceCandidate?.seed ?? seedFromCandidateId(candidate.id) ?? null,
      strength: sourceCandidate?.strength ?? strengthFromCandidateId(candidate.id) ?? null,
      resolution: primary?.resolution ?? null,
      resolutions: reports.map((report) => report.resolution!).sort((left, right) => left - right),
      paletteColors: summary.paletteColors ?? [],
      status: primary?.status ?? null,
      score: primary?.score ?? null,
      reasons: primary?.reasons ?? [],
      images: {
        raw: await imageRef("Raw", resolveArtifactPath(sourceCandidate?.png, sourceManifestPath ? path.dirname(sourceManifestPath) : summaryDir, outputRoot)),
        rawTile: await imageRef("Raw 3x3", resolveArtifactPath(sourceCandidate?.tile3x3_png, sourceManifestPath ? path.dirname(sourceManifestPath) : summaryDir, outputRoot)),
        projected: await imageRef("Projected", resolveArtifactPath(primary?.png, summaryDir, outputRoot)),
        contactSheet: await imageRef("Contact sheet", resolveArtifactPath(sourceManifest?.contact_sheet_png, sourceManifestPath ? path.dirname(sourceManifestPath) : summaryDir, outputRoot)),
        reviewSheet: await imageRef("Review sheet", reviewSheet),
      },
    });
  }
  return records;
}

async function candidatesFromArchiveManifest(archivePath: string, outputRoot: string): Promise<TextureCandidateEntry[]> {
  const archive = await readJson<ArchiveManifest>(archivePath);
  const archiveDir = path.dirname(archivePath);
  const reviewSheet = resolveArchiveFile(archive.reviewSheet, archiveDir, outputRoot);
  const records: TextureCandidateEntry[] = [];
  for (const candidate of archive.candidates ?? []) {
    if (!candidate.id) {
      continue;
    }
    const resolutions = (candidate.resolutions ?? []).filter((resolution) => Number.isFinite(resolution.resolution));
    const primary = [...resolutions].sort((left, right) => (right.resolution ?? 0) - (left.resolution ?? 0))[0] ?? null;
    const codename = candidate.codename ?? candidateCodenameFrom(
      candidate.id,
      candidate.seed ?? null,
      candidate.strength ?? null,
      archive.diffusion?.promptPreset,
    );
    records.push({
      id: candidateKey("archive", archivePath, candidate.id, primary?.resolution ?? null),
      candidateId: candidate.id,
      codename,
      source: "archive",
      textureName: archive.texture ?? null,
      artifactRoot: archiveDir,
      manifestPath: resolveArchiveFile(archive.diffusion?.manifest, archiveDir, outputRoot),
      projectionReportPath: resolveArchiveFile(candidate.projectionReport, archiveDir, outputRoot),
      archivePath,
      archived: true,
      promptPreset: archive.diffusion?.promptPreset ?? null,
      prompt: archive.diffusion?.prompt ?? null,
      negativePrompt: archive.diffusion?.negativePrompt ?? null,
      modelId: archive.diffusion?.modelId ?? null,
      scheduler: archive.diffusion?.scheduler ?? null,
      steps: archive.diffusion?.steps ?? null,
      seed: candidate.seed ?? seedFromCandidateId(candidate.id) ?? null,
      strength: candidate.strength ?? strengthFromCandidateId(candidate.id) ?? null,
      resolution: primary?.resolution ?? null,
      resolutions: resolutions.map((resolution) => resolution.resolution!).sort((left, right) => left - right),
      paletteColors: archive.palette?.colors ?? [],
      status: primary?.triage?.status ?? null,
      score: primary?.triage?.score ?? null,
      reasons: primary?.triage?.reasons ?? [],
      images: {
        raw: await imageRef("Raw", resolveArchiveFile(candidate.raw, archiveDir, outputRoot)),
        rawTile: await imageRef("Raw 3x3", resolveArchiveFile(candidate.rawTile, archiveDir, outputRoot)),
        projected: await imageRef("Projected", resolveArchiveFile(primary?.png, archiveDir, outputRoot)),
        contactSheet: await imageRef("Contact sheet", null),
        reviewSheet: await imageRef("Review sheet", reviewSheet),
      },
    });
  }
  return records;
}

async function findNamedFiles(root: string, filename: string): Promise<string[]> {
  try {
    const stat = await fs.stat(root);
    if (!stat.isDirectory()) {
      return [];
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return [];
    }
    throw error;
  }
  const files: string[] = [];
  await walk(root, async (filePath, entry) => {
    if (entry.isFile() && entry.name === filename) {
      files.push(filePath);
    }
  });
  return files.sort((left, right) => left.localeCompare(right));
}

async function walk(root: string, visit: (filePath: string, entry: Dirent) => Promise<void>): Promise<void> {
  const entries = await fs.readdir(root, { withFileTypes: true });
  for (const entry of entries) {
    const filePath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      await walk(filePath, visit);
    } else {
      await visit(filePath, entry);
    }
  }
}

async function readJson<T>(filePath: string): Promise<T> {
  try {
    return JSON.parse(await fs.readFile(filePath, "utf8")) as T;
  } catch (error) {
    throw new Error(`Failed to read candidate artifact '${filePath}': ${(error as Error).message}`);
  }
}

async function readOptionalJson<T>(filePath: string): Promise<T | null> {
  try {
    return await readJson<T>(filePath);
  } catch {
    return null;
  }
}

function resolveArchiveFile(ref: ArchiveFileRef | undefined, baseDir: string, outputRoot: string): string | null {
  return resolveArtifactPath(ref?.path, baseDir, outputRoot);
}

function resolveArtifactPath(value: string | undefined, baseDir: string, outputRoot: string): string | null {
  if (!value) {
    return null;
  }
  const candidates: string[] = [];
  if (path.isAbsolute(value)) {
    if (value.startsWith(`${LEGACY_TEXTURE_LAB_OUTPUT_ROOT}/`)) {
      candidates.push(path.join(outputRoot, path.relative(LEGACY_TEXTURE_LAB_OUTPUT_ROOT, value)));
    }
    candidates.push(value);
  } else {
    candidates.push(path.resolve(baseDir, value));
  }
  return candidates.find(fileExistsSync) ?? candidates[0] ?? null;
}

async function imageRef(label: string, imagePath: string | null): Promise<TextureImageRef> {
  return {
    label,
    path: imagePath,
    exists: imagePath ? await fileExists(imagePath) : false,
    missingCommand: null,
  };
}

async function fileExists(filePath: string): Promise<boolean> {
  try {
    const stat = await fs.stat(filePath);
    return stat.isFile();
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return false;
    }
    throw error;
  }
}

function fileExistsSync(filePath: string): boolean {
  try {
    return statSync(filePath).isFile();
  } catch {
    return false;
  }
}

async function findReviewSheet(directory: string): Promise<string | null> {
  try {
    const entries = await fs.readdir(directory, { withFileTypes: true });
    const match = entries
      .filter((entry) => entry.isFile() && /^projection-review.*\.png$/i.test(entry.name))
      .map((entry) => path.join(directory, entry.name))
      .sort((left, right) => left.localeCompare(right))[0];
    return match ?? null;
  } catch {
    return null;
  }
}

function projectionReportPath(summaryDir: string, candidateId: string): string {
  return path.join(summaryDir, sanitizeFileStem(candidateId), `${candidateId}-projection-report.json`);
}

function sanitizeFileStem(value: string): string {
  return value.replace(/[^a-zA-Z0-9._-]+/g, "-");
}

function candidateKey(source: TextureCandidateSource, artifactPath: string, candidateId: string, resolution: number | null): string {
  const resolutionPart = resolution === null ? "" : `:${resolution}`;
  return `${source}:${path.basename(path.dirname(artifactPath))}:${candidateId}${resolutionPart}`;
}

function inferTextureName(inputPath: string | undefined, promptPreset: string | undefined, textureNames: string[]): string | null {
  const value = `${inputPath ?? ""} ${promptPreset ?? ""}`.toLowerCase();
  for (const textureName of textureNames) {
    const normalizedTextureName = textureName.toLowerCase();
    if (value.includes(normalizedTextureName) || value.includes(normalizedTextureName.replace(/_/g, "-"))) {
      return textureName;
    }
  }
  if (value.includes("grass_block_top") || value.includes("grass-top")) {
    return "grass_block_top";
  }
  if (value.includes("stone")) {
    return "stone";
  }
  return null;
}

function candidateCodenameFrom(candidateId: string, seed: number | null, strength: number | null, promptPreset: string | undefined): string {
  const prefix = promptPresetPrefix(promptPreset);
  const seedText = seed?.toString() ?? seedFromCandidateId(candidateId)?.toString() ?? "0000";
  const strengthText = Math.round((strength ?? strengthFromCandidateId(candidateId) ?? 0) * 100)
    .toString()
    .padStart(2, "0");
  return `${prefix}${seedText}S${strengthText}`;
}

function promptPresetPrefix(promptPreset: string | undefined): string {
  if (promptPreset?.includes("grass")) {
    return "G";
  }
  if (promptPreset?.includes("dressed")) {
    return "D";
  }
  if (promptPreset?.includes("hewn")) {
    return "H";
  }
  if (promptPreset?.includes("stone")) {
    return "S";
  }
  return "X";
}

function seedFromCandidateId(id: string): number | null {
  const match = /seed(\d+)/.exec(id);
  return match ? Number.parseInt(match[1]!, 10) : null;
}

function strengthFromCandidateId(id: string): number | null {
  const match = /strength(\d+)p(\d+)/.exec(id);
  return match ? Number.parseFloat(`${match[1]}.${match[2]}`) : null;
}

function compareCandidates(left: TextureCandidateEntry, right: TextureCandidateEntry): number {
  return (
    (left.textureName ?? "~").localeCompare(right.textureName ?? "~") ||
    left.codename.localeCompare(right.codename) ||
    left.source.localeCompare(right.source) ||
    left.id.localeCompare(right.id)
  );
}
