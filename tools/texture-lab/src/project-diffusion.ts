import fs from "node:fs/promises";
import path from "node:path";
import type { PaletteSpec } from "./dsl";
import { areaDownsample, resolveColor, type Rgba } from "./image";
import { loadTexturePack } from "./load";
import { decodePng, encodePng, type RgbaImage } from "./png";

interface ProjectArgs {
  input: string;
  manifestPath: string;
  texture: string;
  outDir: string;
  candidates: string[];
  resolutions: number[];
  paletteColors: string[];
  symbols: string[];
  limit: number | undefined;
}

interface DiffusionManifest {
  candidates?: DiffusionCandidate[];
  contact_sheet_png?: string;
  created_at?: string;
  input?: DiffusionInput;
  model_id?: string;
  negative_prompt?: string;
  prompt?: string;
  prompt_preset?: string;
  scheduler?: string;
  steps?: number;
}

interface DiffusionInput {
  path?: string;
  original_sha256?: string;
  original_size?: [number, number];
  prepared_size?: [number, number];
  preprocess?: Record<string, unknown>;
}

interface DiffusionCandidate {
  id: string;
  png: string;
  seed?: number;
  sha256?: string;
  strength?: number;
  seam?: unknown;
  tile3x3_png?: string;
}

interface PaletteEntry {
  name: string;
  symbol: string;
  color: Rgba;
  lab: Oklab;
}

interface Oklab {
  l: number;
  a: number;
  b: number;
}

interface QuantizedImage {
  image: RgbaImage;
  indices: Uint16Array;
  width: number;
  height: number;
  meanOklabError: number;
  maxOklabError: number;
  usage: Record<string, number>;
}

interface SeamAxisMetrics {
  meanAbsRgb: number;
  maxAbsRgb: number;
  symbolMismatchRate: number;
}

interface SeamMetrics {
  xWrap: SeamAxisMetrics;
  yWrap: SeamAxisMetrics;
}

interface DetailMetrics {
  horizontalTransitionRate: number;
  verticalTransitionRate: number;
  transitionRate: number;
  isolatedPixelRate: number;
  normalizedPaletteEntropy: number;
}

interface MacroMetrics {
  referenceSize: [number, number];
  cellSize: [number, number];
  cellCount: number;
  majorityMismatchCells: number;
  majorityMismatchRate: number;
  meanCellOklabError: number;
  maxCellOklabError: number;
}

interface ResolutionReport {
  resolution: number;
  png: string;
  mask: string;
  usage: Record<string, number>;
  correction?: MacroCorrectionMetrics;
  quantization: {
    meanOklabError: number;
    maxOklabError: number;
  };
  seam: SeamMetrics;
  detail: DetailMetrics;
  macro?: MacroMetrics;
  triage: {
    status: "keep" | "review" | "kill";
    score: number;
    reasons: string[];
  };
}

interface MacroCorrectionMetrics {
  referenceSize: [number, number];
  cellSize: [number, number];
  correctedCells: number;
  correctedPixels: number;
  correctedPixelRate: number;
  targetMajorityCount: number;
}

interface CandidateReport {
  candidate: Pick<DiffusionCandidate, "id" | "png" | "seed" | "sha256" | "strength" | "tile3x3_png">;
  sourceManifest: string;
  diffusion: {
    createdAt?: string;
    input?: DiffusionInput;
    modelId?: string;
    negativePrompt?: string;
    prompt?: string;
    promptPreset?: string;
    scheduler?: string;
    steps?: number;
  };
  texture: string;
  palette: {
    colors: string[];
    symbols: Record<string, string>;
  };
  resolutions: ResolutionReport[];
}

const DEFAULT_SYMBOLS = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!#$%&*+-/:;<=>?@^_|~";

const args = parseArgs(process.argv.slice(2));
await run(args);

async function run(args: ProjectArgs): Promise<void> {
  const pack = await loadTexturePack(args.input);
  const texture = pack.textures[args.texture];
  if (!texture) {
    throw new Error(`Texture '${args.texture}' was not found in pack '${pack.name}'`);
  }
  const palette = pack.palettes[texture.palette];
  if (!palette) {
    throw new Error(`Texture '${args.texture}' references missing palette '${texture.palette}'`);
  }

  const entries = makePaletteEntries(palette, args.paletteColors, args.symbols);
  const manifest = await loadManifest(args.manifestPath);
  const candidates = selectCandidates(manifest, args);
  const manifestDir = path.dirname(args.manifestPath);
  const macroReference = await loadMacroReference(manifest, manifestDir, entries);

  await fs.mkdir(args.outDir, { recursive: true });

  const reports: CandidateReport[] = [];
  for (const candidate of candidates) {
    const report = await projectCandidate(candidate, {
      args,
      entries,
      macroReference,
      manifest,
      manifestDir,
    });
    reports.push(report);
  }

  const summary = {
    sourceManifest: args.manifestPath,
    texture: args.texture,
    candidateCount: reports.length,
    resolutions: args.resolutions,
    paletteColors: entries.map((entry) => entry.name),
    candidates: reports.map((report) => ({
      id: report.candidate.id,
      reports: report.resolutions.map((resolution) => ({
        resolution: resolution.resolution,
        status: resolution.triage.status,
        score: resolution.triage.score,
        reasons: resolution.triage.reasons,
        png: resolution.png,
        mask: resolution.mask,
      })),
    })),
  };
  const summaryPath = path.join(args.outDir, "projection-summary.json");
  await fs.writeFile(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);
  console.log(`Wrote ${summaryPath}`);
}

interface ProjectContext {
  args: ProjectArgs;
  entries: PaletteEntry[];
  macroReference: QuantizedImage | undefined;
  manifest: DiffusionManifest;
  manifestDir: string;
}

async function projectCandidate(candidate: DiffusionCandidate, context: ProjectContext): Promise<CandidateReport> {
  const candidatePng = resolvePath(context.manifestDir, candidate.png);
  const candidateImage = decodePng(await fs.readFile(candidatePng));
  const candidateDir = path.join(context.args.outDir, sanitizeFileStem(candidate.id));
  await fs.mkdir(candidateDir, { recursive: true });

  const reports: ResolutionReport[] = [];
  for (const resolution of context.args.resolutions) {
    const downsampled = areaDownsample(candidateImage, resolution, resolution);
    const quantized = quantizeImage(downsampled, context.entries);
    const correction = context.macroReference
      ? macroCorrect(quantized, downsampled, context.macroReference, context.entries)
      : undefined;
    const pngPath = path.join(candidateDir, `${candidate.id}-${resolution}.png`);
    const maskPath = path.join(candidateDir, `${candidate.id}-${resolution}.mask.txt`);
    await fs.writeFile(pngPath, encodePng(quantized.image));
    await fs.writeFile(maskPath, formatMask(quantized, context.entries));

    const macro = context.macroReference
      ? summarizeMacro(quantized, context.macroReference, context.entries)
      : undefined;
    const seam = summarizeSeams(quantized);
    const detail = summarizeDetail(quantized, context.entries.length);
    const triage = triageProjection(quantized, seam, detail, macro, correction);
    const resolutionReport: ResolutionReport = {
      resolution,
      png: pngPath,
      mask: maskPath,
      usage: quantized.usage,
      ...(correction ? { correction } : {}),
      quantization: {
        meanOklabError: round6(quantized.meanOklabError),
        maxOklabError: round6(quantized.maxOklabError),
      },
      seam,
      detail,
      triage,
      ...(macro ? { macro } : {}),
    };
    reports.push(resolutionReport);
    console.log(`Wrote ${pngPath}`);
    console.log(`Wrote ${maskPath}`);
  }

  const report: CandidateReport = {
    candidate: pickCandidateProvenance(candidate),
    sourceManifest: context.args.manifestPath,
    diffusion: {
      ...(context.manifest.created_at ? { createdAt: context.manifest.created_at } : {}),
      ...(context.manifest.input ? { input: context.manifest.input } : {}),
      ...(context.manifest.model_id ? { modelId: context.manifest.model_id } : {}),
      ...(context.manifest.negative_prompt ? { negativePrompt: context.manifest.negative_prompt } : {}),
      ...(context.manifest.prompt ? { prompt: context.manifest.prompt } : {}),
      ...(context.manifest.prompt_preset ? { promptPreset: context.manifest.prompt_preset } : {}),
      ...(context.manifest.scheduler ? { scheduler: context.manifest.scheduler } : {}),
      ...(context.manifest.steps !== undefined ? { steps: context.manifest.steps } : {}),
    },
    texture: context.args.texture,
    palette: {
      colors: context.entries.map((entry) => entry.name),
      symbols: Object.fromEntries(context.entries.map((entry) => [entry.name, entry.symbol])),
    },
    resolutions: reports,
  };
  const reportPath = path.join(candidateDir, `${candidate.id}-projection-report.json`);
  await fs.writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`Wrote ${reportPath}`);
  return report;
}

function makePaletteEntries(palette: PaletteSpec, selectedNames: string[], selectedSymbols: string[]): PaletteEntry[] {
  const names = selectedNames.length > 0 ? selectedNames : Object.keys(palette);
  if (names.length === 0) {
    throw new Error("Projection palette must contain at least one color");
  }
  if (selectedSymbols.length > 0 && selectedSymbols.length !== names.length) {
    throw new Error(
      `--symbols count (${selectedSymbols.length}) must match --palette-colors count (${names.length})`,
    );
  }
  if (selectedSymbols.length === 0 && names.length > DEFAULT_SYMBOLS.length) {
    throw new Error(`Projection palette has ${names.length} colors, but only ${DEFAULT_SYMBOLS.length} default symbols`);
  }

  return names.map((name, index) => {
    if (!palette[name]) {
      throw new Error(`Palette color '${name}' was not found`);
    }
    const color = resolveColor(palette, name);
    const symbol = selectedSymbols[index] ?? DEFAULT_SYMBOLS[index];
    if (!symbol) {
      throw new Error(`No ASCII symbol is available for palette color '${name}'`);
    }
    if (symbol.length !== 1) {
      throw new Error(`Palette symbol '${symbol}' for '${name}' must be exactly one character`);
    }
    return {
      name,
      symbol,
      color,
      lab: rgbToOklab(color),
    };
  });
}

async function loadManifest(manifestPath: string): Promise<DiffusionManifest> {
  const manifest = JSON.parse(await fs.readFile(manifestPath, "utf8")) as DiffusionManifest;
  if (!Array.isArray(manifest.candidates)) {
    throw new Error(`Diffusion manifest '${manifestPath}' does not contain a candidates array`);
  }
  for (const [index, candidate] of manifest.candidates.entries()) {
    if (!candidate.id || !candidate.png) {
      throw new Error(`Diffusion manifest candidate ${index} is missing id/png`);
    }
  }
  return manifest;
}

function selectCandidates(manifest: DiffusionManifest, args: ProjectArgs): DiffusionCandidate[] {
  const allCandidates = manifest.candidates ?? [];
  const selected =
    args.candidates.length > 0
      ? args.candidates.map((id) => {
          const candidate = allCandidates.find((item) => item.id === id);
          if (!candidate) {
            throw new Error(`Candidate '${id}' was not found in ${args.manifestPath}`);
          }
          return candidate;
        })
      : allCandidates;
  const limited = args.limit === undefined ? selected : selected.slice(0, args.limit);
  if (limited.length === 0) {
    throw new Error("No diffusion candidates selected");
  }
  return limited;
}

async function loadMacroReference(
  manifest: DiffusionManifest,
  manifestDir: string,
  entries: PaletteEntry[],
): Promise<QuantizedImage | undefined> {
  if (!manifest.input?.path) {
    return undefined;
  }
  const inputPath = resolvePath(manifestDir, manifest.input.path);
  try {
    const input = decodePng(await fs.readFile(inputPath));
    return quantizeImage(input, entries);
  } catch (error) {
    console.warn(`Skipping macro reference '${inputPath}': ${(error as Error).message}`);
    return undefined;
  }
}

function quantizeImage(image: RgbaImage, entries: PaletteEntry[]): QuantizedImage {
  const data = new Uint8Array(image.width * image.height * 4);
  const indices = new Uint16Array(image.width * image.height);
  const usage = Object.fromEntries(entries.map((entry) => [entry.name, 0]));
  let errorSum = 0;
  let maxError = 0;

  for (let y = 0; y < image.height; y += 1) {
    for (let x = 0; x < image.width; x += 1) {
      const offset = (y * image.width + x) * 4;
      const source: Rgba = [
        image.data[offset]!,
        image.data[offset + 1]!,
        image.data[offset + 2]!,
        image.data[offset + 3]!,
      ];
      const sourceLab = rgbToOklab(source);
      let bestIndex = 0;
      let bestDistance = Number.POSITIVE_INFINITY;
      for (const [entryIndex, entry] of entries.entries()) {
        const distance = oklabDistance(sourceLab, entry.lab);
        if (distance < bestDistance) {
          bestDistance = distance;
          bestIndex = entryIndex;
        }
      }
      const entry = entries[bestIndex]!;
      data[offset] = entry.color[0];
      data[offset + 1] = entry.color[1];
      data[offset + 2] = entry.color[2];
      data[offset + 3] = entry.color[3];
      indices[y * image.width + x] = bestIndex;
      usage[entry.name] = (usage[entry.name] ?? 0) + 1;
      errorSum += bestDistance;
      maxError = Math.max(maxError, bestDistance);
    }
  }

  return {
    image: { width: image.width, height: image.height, data },
    indices,
    width: image.width,
    height: image.height,
    meanOklabError: errorSum / (image.width * image.height),
    maxOklabError: maxError,
    usage,
  };
}

function macroCorrect(
  quantized: QuantizedImage,
  source: RgbaImage,
  reference: QuantizedImage,
  entries: PaletteEntry[],
): MacroCorrectionMetrics | undefined {
  if (quantized.width % reference.width !== 0 || quantized.height % reference.height !== 0) {
    return undefined;
  }

  const cellWidth = quantized.width / reference.width;
  const cellHeight = quantized.height / reference.height;
  const cellPixels = cellWidth * cellHeight;
  const targetMajorityCount = Math.floor(cellPixels / 2) + 1;
  let correctedCells = 0;
  let correctedPixels = 0;

  for (let cellY = 0; cellY < reference.height; cellY += 1) {
    for (let cellX = 0; cellX < reference.width; cellX += 1) {
      const referenceIndex = reference.indices[cellY * reference.width + cellX]!;
      let referenceCount = 0;
      const swaps: MacroSwapCandidate[] = [];

      for (let dy = 0; dy < cellHeight; dy += 1) {
        for (let dx = 0; dx < cellWidth; dx += 1) {
          const pixelX = cellX * cellWidth + dx;
          const pixelY = cellY * cellHeight + dy;
          const pixelIndex = pixelY * quantized.width + pixelX;
          const currentIndex = quantized.indices[pixelIndex]!;
          if (currentIndex === referenceIndex) {
            referenceCount += 1;
            continue;
          }

          const sourceLab = pixelOklab(source, pixelIndex);
          swaps.push({
            pixelIndex,
            pixelX,
            pixelY,
            penalty:
              oklabDistance(sourceLab, entries[referenceIndex]!.lab) -
              oklabDistance(sourceLab, entries[currentIndex]!.lab),
          });
        }
      }

      if (referenceCount >= targetMajorityCount) {
        continue;
      }

      swaps.sort((a, b) => a.penalty - b.penalty || a.pixelY - b.pixelY || a.pixelX - b.pixelX);
      const needed = Math.min(targetMajorityCount - referenceCount, swaps.length);
      if (needed > 0) {
        correctedCells += 1;
      }
      for (let index = 0; index < needed; index += 1) {
        const swap = swaps[index]!;
        setQuantizedPixel(quantized, swap.pixelIndex, referenceIndex, entries);
        correctedPixels += 1;
      }
    }
  }

  refreshQuantizedStats(quantized, source, entries);
  return {
    referenceSize: [reference.width, reference.height],
    cellSize: [cellWidth, cellHeight],
    correctedCells,
    correctedPixels,
    correctedPixelRate: round6(correctedPixels / (quantized.width * quantized.height)),
    targetMajorityCount,
  };
}

interface MacroSwapCandidate {
  pixelIndex: number;
  pixelX: number;
  pixelY: number;
  penalty: number;
}

function setQuantizedPixel(
  quantized: QuantizedImage,
  pixelIndex: number,
  entryIndex: number,
  entries: PaletteEntry[],
): void {
  const entry = entries[entryIndex]!;
  const offset = pixelIndex * 4;
  quantized.indices[pixelIndex] = entryIndex;
  quantized.image.data[offset] = entry.color[0];
  quantized.image.data[offset + 1] = entry.color[1];
  quantized.image.data[offset + 2] = entry.color[2];
  quantized.image.data[offset + 3] = entry.color[3];
}

function refreshQuantizedStats(quantized: QuantizedImage, source: RgbaImage, entries: PaletteEntry[]): void {
  const usage = Object.fromEntries(entries.map((entry) => [entry.name, 0]));
  let errorSum = 0;
  let maxError = 0;
  for (let pixelIndex = 0; pixelIndex < quantized.indices.length; pixelIndex += 1) {
    const entry = entries[quantized.indices[pixelIndex]!]!;
    usage[entry.name] = (usage[entry.name] ?? 0) + 1;
    const distance = oklabDistance(pixelOklab(source, pixelIndex), entry.lab);
    errorSum += distance;
    maxError = Math.max(maxError, distance);
  }
  quantized.usage = usage;
  quantized.meanOklabError = errorSum / quantized.indices.length;
  quantized.maxOklabError = maxError;
}

function pixelOklab(image: RgbaImage, pixelIndex: number): Oklab {
  const offset = pixelIndex * 4;
  return rgbToOklab([
    image.data[offset]!,
    image.data[offset + 1]!,
    image.data[offset + 2]!,
    image.data[offset + 3]!,
  ]);
}

function formatMask(quantized: QuantizedImage, entries: PaletteEntry[]): string {
  const lines: string[] = [];
  for (let y = 0; y < quantized.height; y += 1) {
    let line = "";
    for (let x = 0; x < quantized.width; x += 1) {
      const index = quantized.indices[y * quantized.width + x]!;
      line += entries[index]?.symbol ?? "?";
    }
    lines.push(line);
  }
  return `${lines.join("\n")}\n`;
}

function summarizeMacro(
  quantized: QuantizedImage,
  reference: QuantizedImage,
  entries: PaletteEntry[],
): MacroMetrics | undefined {
  if (quantized.width % reference.width !== 0 || quantized.height % reference.height !== 0) {
    return undefined;
  }
  const cellWidth = quantized.width / reference.width;
  const cellHeight = quantized.height / reference.height;
  let mismatchCells = 0;
  let errorSum = 0;
  let maxError = 0;

  for (let cellY = 0; cellY < reference.height; cellY += 1) {
    for (let cellX = 0; cellX < reference.width; cellX += 1) {
      const referenceIndex = reference.indices[cellY * reference.width + cellX]!;
      const counts = new Array<number>(entries.length).fill(0);
      let labL = 0;
      let labA = 0;
      let labB = 0;
      for (let dy = 0; dy < cellHeight; dy += 1) {
        for (let dx = 0; dx < cellWidth; dx += 1) {
          const pixelX = cellX * cellWidth + dx;
          const pixelY = cellY * cellHeight + dy;
          const index = quantized.indices[pixelY * quantized.width + pixelX]!;
          counts[index] = (counts[index] ?? 0) + 1;
          const lab = entries[index]!.lab;
          labL += lab.l;
          labA += lab.a;
          labB += lab.b;
        }
      }
      const majorityIndex = counts.reduce(
        (best, count, index) => (count > counts[best]! ? index : best),
        0,
      );
      if (majorityIndex !== referenceIndex) {
        mismatchCells += 1;
      }
      const cellPixels = cellWidth * cellHeight;
      const cellLab = { l: labL / cellPixels, a: labA / cellPixels, b: labB / cellPixels };
      const error = oklabDistance(cellLab, entries[referenceIndex]!.lab);
      errorSum += error;
      maxError = Math.max(maxError, error);
    }
  }

  const cellCount = reference.width * reference.height;
  return {
    referenceSize: [reference.width, reference.height],
    cellSize: [cellWidth, cellHeight],
    cellCount,
    majorityMismatchCells: mismatchCells,
    majorityMismatchRate: round6(mismatchCells / cellCount),
    meanCellOklabError: round6(errorSum / cellCount),
    maxCellOklabError: round6(maxError),
  };
}

function summarizeSeams(quantized: QuantizedImage): SeamMetrics {
  return {
    xWrap: summarizeSeamAxis(quantized, "x"),
    yWrap: summarizeSeamAxis(quantized, "y"),
  };
}

function summarizeSeamAxis(quantized: QuantizedImage, axis: "x" | "y"): SeamAxisMetrics {
  const count = axis === "x" ? quantized.height : quantized.width;
  let sum = 0;
  let max = 0;
  let mismatches = 0;
  for (let index = 0; index < count; index += 1) {
    const aIndex = axis === "x" ? index * quantized.width : index;
    const bIndex = axis === "x" ? index * quantized.width + quantized.width - 1 : (quantized.height - 1) * quantized.width + index;
    const diff = pixelMeanAbsRgb(quantized.image, aIndex, bIndex);
    sum += diff;
    max = Math.max(max, diff);
    if (quantized.indices[aIndex] !== quantized.indices[bIndex]) {
      mismatches += 1;
    }
  }
  return {
    meanAbsRgb: round3(sum / count),
    maxAbsRgb: round3(max),
    symbolMismatchRate: round6(mismatches / count),
  };
}

function summarizeDetail(quantized: QuantizedImage, paletteSize: number): DetailMetrics {
  let horizontalTransitions = 0;
  let verticalTransitions = 0;
  for (let y = 0; y < quantized.height; y += 1) {
    for (let x = 0; x < quantized.width; x += 1) {
      const current = quantized.indices[y * quantized.width + x]!;
      if (x + 1 < quantized.width && current !== quantized.indices[y * quantized.width + x + 1]) {
        horizontalTransitions += 1;
      }
      if (y + 1 < quantized.height && current !== quantized.indices[(y + 1) * quantized.width + x]) {
        verticalTransitions += 1;
      }
    }
  }

  let isolated = 0;
  for (let y = 0; y < quantized.height; y += 1) {
    for (let x = 0; x < quantized.width; x += 1) {
      const current = quantized.indices[y * quantized.width + x]!;
      const left = quantized.indices[y * quantized.width + wrap(x - 1, quantized.width)]!;
      const right = quantized.indices[y * quantized.width + wrap(x + 1, quantized.width)]!;
      const up = quantized.indices[wrap(y - 1, quantized.height) * quantized.width + x]!;
      const down = quantized.indices[wrap(y + 1, quantized.height) * quantized.width + x]!;
      if (current !== left && current !== right && current !== up && current !== down) {
        isolated += 1;
      }
    }
  }

  const horizontalEdges = quantized.height * Math.max(0, quantized.width - 1);
  const verticalEdges = Math.max(0, quantized.height - 1) * quantized.width;
  const totalEdges = horizontalEdges + verticalEdges;
  const totalPixels = quantized.width * quantized.height;

  return {
    horizontalTransitionRate: round6(horizontalEdges === 0 ? 0 : horizontalTransitions / horizontalEdges),
    verticalTransitionRate: round6(verticalEdges === 0 ? 0 : verticalTransitions / verticalEdges),
    transitionRate: round6(totalEdges === 0 ? 0 : (horizontalTransitions + verticalTransitions) / totalEdges),
    isolatedPixelRate: round6(isolated / totalPixels),
    normalizedPaletteEntropy: round6(normalizedEntropy(quantized.usage, totalPixels, paletteSize)),
  };
}

function triageProjection(
  quantized: QuantizedImage,
  seam: SeamMetrics,
  detail: DetailMetrics,
  macro: MacroMetrics | undefined,
  correction: MacroCorrectionMetrics | undefined,
): ResolutionReport["triage"] {
  const reasons: string[] = [];
  const seamMean = Math.max(seam.xWrap.meanAbsRgb, seam.yWrap.meanAbsRgb);
  const seamMismatch = Math.max(seam.xWrap.symbolMismatchRate, seam.yWrap.symbolMismatchRate);
  if (seamMean > 24 || (seamMean > 16 && seamMismatch > 0.55)) {
    reasons.push("kill: high projected wrap seam");
  } else if (seamMean > 12 || seamMismatch > 0.35) {
    reasons.push("review: projected wrap seam risk");
  }

  if (macro && macro.majorityMismatchRate > 0.6) {
    reasons.push("kill: macro majority drift");
  } else if (macro && macro.majorityMismatchRate > 0.25) {
    reasons.push("review: macro majority drift");
  }

  if (quantized.meanOklabError > 0.08) {
    reasons.push("review: high palette projection error");
  }
  if (correction && correction.correctedPixelRate > 0.45) {
    reasons.push("kill: high macro correction cost");
  } else if (correction && correction.correctedPixelRate > 0.25) {
    reasons.push("review: high macro correction cost");
  }
  if (detail.isolatedPixelRate > 0.08) {
    reasons.push("review: high isolated-pixel rate");
  }

  const score =
    quantized.meanOklabError * 8 +
    seamMean / 64 +
    seamMismatch +
    detail.isolatedPixelRate * 2 +
    (macro ? macro.majorityMismatchRate * 2 + macro.meanCellOklabError * 8 : 0) +
    (correction ? correction.correctedPixelRate * 2 : 0);
  const status = reasons.some((reason) => reason.startsWith("kill:"))
    ? "kill"
    : reasons.length > 0
      ? "review"
      : "keep";
  return {
    status,
    score: round6(score),
    reasons,
  };
}

function pixelMeanAbsRgb(image: RgbaImage, pixelA: number, pixelB: number): number {
  const offsetA = pixelA * 4;
  const offsetB = pixelB * 4;
  return (
    Math.abs(image.data[offsetA]! - image.data[offsetB]!) +
    Math.abs(image.data[offsetA + 1]! - image.data[offsetB + 1]!) +
    Math.abs(image.data[offsetA + 2]! - image.data[offsetB + 2]!)
  ) / 3;
}

function normalizedEntropy(usage: Record<string, number>, totalPixels: number, paletteSize: number): number {
  if (totalPixels === 0 || paletteSize <= 1) {
    return 0;
  }
  let entropy = 0;
  for (const count of Object.values(usage)) {
    if (count <= 0) {
      continue;
    }
    const p = count / totalPixels;
    entropy -= p * Math.log2(p);
  }
  return entropy / Math.log2(paletteSize);
}

function pickCandidateProvenance(
  candidate: DiffusionCandidate,
): Pick<DiffusionCandidate, "id" | "png" | "seed" | "sha256" | "strength" | "tile3x3_png"> {
  return {
    id: candidate.id,
    png: candidate.png,
    ...(candidate.seed !== undefined ? { seed: candidate.seed } : {}),
    ...(candidate.sha256 ? { sha256: candidate.sha256 } : {}),
    ...(candidate.strength !== undefined ? { strength: candidate.strength } : {}),
    ...(candidate.tile3x3_png ? { tile3x3_png: candidate.tile3x3_png } : {}),
  };
}

function rgbToOklab(color: Rgba): Oklab {
  const r = srgbToLinear(color[0] / 255);
  const g = srgbToLinear(color[1] / 255);
  const b = srgbToLinear(color[2] / 255);

  const l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
  const m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
  const s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

  const lRoot = Math.cbrt(l);
  const mRoot = Math.cbrt(m);
  const sRoot = Math.cbrt(s);

  return {
    l: 0.2104542553 * lRoot + 0.793617785 * mRoot - 0.0040720468 * sRoot,
    a: 1.9779984951 * lRoot - 2.428592205 * mRoot + 0.4505937099 * sRoot,
    b: 0.0259040371 * lRoot + 0.7827717662 * mRoot - 0.808675766 * sRoot,
  };
}

function srgbToLinear(value: number): number {
  return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
}

function oklabDistance(a: Oklab, b: Oklab): number {
  const dl = a.l - b.l;
  const da = a.a - b.a;
  const db = a.b - b.b;
  return Math.sqrt(dl * dl + da * da + db * db);
}

function resolvePath(baseDir: string, maybeRelative: string): string {
  return path.isAbsolute(maybeRelative) ? maybeRelative : path.join(baseDir, maybeRelative);
}

function sanitizeFileStem(value: string): string {
  return value.replace(/[^a-zA-Z0-9._-]/g, "_");
}

function wrap(value: number, size: number): number {
  return ((value % size) + size) % size;
}

function round3(value: number): number {
  return Math.round(value * 1000) / 1000;
}

function round6(value: number): number {
  return Math.round(value * 1_000_000) / 1_000_000;
}

function parseArgs(argv: string[]): ProjectArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error(
      "Usage: tsx src/project-diffusion.ts <texture.ts> --manifest <manifest.json> --texture <name> [--candidate <id>]... [--resolutions 32,64,128] [--palette-colors a,b,c] [--symbols abc] [--out <dir>] [--limit <n>]",
    );
  }

  let manifestPath = "";
  let texture = "";
  let outDir = path.join("/tmp", "mclone-texture-lab", "diffusion-projection");
  const candidates: string[] = [];
  let resolutions = [32];
  let paletteColors: string[] = [];
  let symbols: string[] = [];
  let limit: number | undefined;

  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index]!;
    if (arg === "--") {
      continue;
    } else if (arg === "--manifest") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--manifest requires a path");
      }
      manifestPath = next;
      index += 1;
    } else if (arg === "--texture") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--texture requires a name");
      }
      texture = next;
      index += 1;
    } else if (arg === "--candidate") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--candidate requires an id");
      }
      candidates.push(next);
      index += 1;
    } else if (arg === "--resolutions") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--resolutions requires a comma-separated list");
      }
      resolutions = parsePositiveIntegerList(next, "--resolutions");
      index += 1;
    } else if (arg === "--palette-colors") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--palette-colors requires a comma-separated list");
      }
      paletteColors = parseStringList(next);
      index += 1;
    } else if (arg === "--symbols") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--symbols requires a character list");
      }
      symbols = next.split("");
      index += 1;
    } else if (arg === "--out") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--out requires a directory");
      }
      outDir = next;
      index += 1;
    } else if (arg === "--limit") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--limit requires a positive integer");
      }
      const parsed = Number.parseInt(next, 10);
      if (!Number.isInteger(parsed) || parsed <= 0) {
        throw new Error(`--limit must be a positive integer, got '${next}'`);
      }
      limit = parsed;
      index += 1;
    } else {
      throw new Error(`Unknown argument '${arg}'`);
    }
  }

  if (!manifestPath) {
    throw new Error("--manifest is required");
  }
  if (!texture) {
    throw new Error("--texture is required");
  }

  return {
    input,
    manifestPath,
    texture,
    outDir,
    candidates,
    resolutions,
    paletteColors,
    symbols,
    limit,
  };
}

function parsePositiveIntegerList(value: string, flag: string): number[] {
  const parsed = parseStringList(value).map((item) => Number.parseInt(item, 10));
  if (parsed.length === 0 || parsed.some((item) => !Number.isInteger(item) || item <= 0)) {
    throw new Error(`${flag} must be a comma-separated list of positive integers`);
  }
  return parsed;
}

function parseStringList(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}
