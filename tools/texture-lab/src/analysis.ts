import type { RgbaImage } from "./png";

// Objective structural features for a single texture tile. The same function is
// run on the local vanilla reference (to get target numbers) and on a candidate
// (to get actuals), so the two are directly comparable. These are deliberately
// AGGREGATE statistics — never a per-pixel comparison to the reference — so they
// guide and guard authoring without becoming a way to trace the original art.
//
// What each group answers, in plain terms:
// - palette:   how many tones, how wide the value range, is there a dominant mid
// - banding:   is structure horizontal, vertical, or isotropic (the row/col test)
// - scale:     is the variation large-scale or fine noise (does it survive shrink)
// - blobs:     how big are the darkest/lightest clusters (catches oversized pits)
// - seam:      does the tile wrap cleanly left-right and top-bottom

export interface LuminanceStats {
  min: number;
  max: number;
  span: number;
  mean: number;
  std: number;
  p10: number;
  p50: number;
  p90: number;
}

export interface TextureFeatures {
  width: number;
  height: number;
  opaquePixels: number;
  transparentShare: number;
  distinctColors: number;
  distinctLevels: number;
  luminance: LuminanceStats;
  dominantColorShare: number;
  dominantBinShare: number;
  rowBandStd: number;
  colBandStd: number;
  anisotropy: number;
  stdFull: number;
  stdHalf: number;
  stdQuarter: number;
  coarseRetention: number;
  localContrast: number;
  darkBlobMax: number;
  darkBlobMean: number;
  darkBlobCount: number;
  lightBlobMax: number;
  lightBlobMean: number;
  lightBlobCount: number;
  seamLeftRight: number;
  seamTopBottom: number;
}

const DARK_PERCENTILE = 12;
const LIGHT_PERCENTILE = 88;
const LUMINANCE_BINS = 8;

export function analyzeTexture(image: RgbaImage): TextureFeatures {
  const { width, height, data } = image;
  const pixelCount = width * height;

  // Per-pixel luminance grid; transparent pixels are filled with the opaque
  // mean afterwards so they do not invent structure in banding/scale measures.
  const grid = new Float64Array(pixelCount);
  const opaqueLum: number[] = [];
  const colorCounts = new Map<string, number>();
  let transparent = 0;

  for (let i = 0; i < pixelCount; i += 1) {
    const r = data[i * 4]!;
    const g = data[i * 4 + 1]!;
    const b = data[i * 4 + 2]!;
    const a = data[i * 4 + 3]!;
    const l = luminance(r, g, b);
    grid[i] = l;
    if (a === 0) {
      transparent += 1;
      continue;
    }
    opaqueLum.push(l);
    const key = `${r},${g},${b},${a}`;
    colorCounts.set(key, (colorCounts.get(key) ?? 0) + 1);
  }

  const opaqueCount = opaqueLum.length;
  const mean = opaqueCount > 0 ? average(opaqueLum) : 0;
  for (let i = 0; i < pixelCount; i += 1) {
    if (data[i * 4 + 3]! === 0) {
      grid[i] = mean;
    }
  }

  const sorted = [...opaqueLum].sort((a, b) => a - b);
  const levels = new Set(opaqueLum.map((l) => Math.round(l)));

  const rowMeans = axisMeans(grid, width, height, "row");
  const colMeans = axisMeans(grid, width, height, "col");
  const rowBandStd = std(rowMeans);
  const colBandStd = std(colMeans);

  const stdFull = std([...grid]);
  const half = boxDownsample(grid, width, height, 2);
  const quarter = boxDownsample(grid, width, height, 4);
  const stdHalf = half ? std(half.values) : Number.NaN;
  const stdQuarter = quarter ? std(quarter.values) : Number.NaN;

  const darkThreshold = percentile(sorted, DARK_PERCENTILE);
  const lightThreshold = percentile(sorted, LIGHT_PERCENTILE);
  const dark = blobStats(grid, width, height, (l) => l <= darkThreshold);
  const light = blobStats(grid, width, height, (l) => l >= lightThreshold);

  return {
    width,
    height,
    opaquePixels: opaqueCount,
    transparentShare: pixelCount > 0 ? transparent / pixelCount : 0,
    distinctColors: colorCounts.size,
    distinctLevels: levels.size,
    luminance: {
      min: sorted[0] ?? 0,
      max: sorted[sorted.length - 1] ?? 0,
      span: (sorted[sorted.length - 1] ?? 0) - (sorted[0] ?? 0),
      mean,
      std: stdFull,
      p10: percentile(sorted, 10),
      p50: percentile(sorted, 50),
      p90: percentile(sorted, 90),
    },
    dominantColorShare: opaqueCount > 0 ? maxCount(colorCounts) / opaqueCount : 0,
    dominantBinShare: dominantLuminanceBinShare(opaqueLum),
    rowBandStd,
    colBandStd,
    anisotropy: rowBandStd / Math.max(colBandStd, 0.01),
    stdFull,
    stdHalf,
    stdQuarter,
    coarseRetention: stdFull > 0 ? stdQuarter / stdFull : 0,
    localContrast: localContrast(grid, width, height),
    darkBlobMax: dark.max,
    darkBlobMean: dark.mean,
    darkBlobCount: dark.count,
    lightBlobMax: light.max,
    lightBlobMean: light.mean,
    lightBlobCount: light.count,
    seamLeftRight: edgeDistance(data, width, height, "leftRight"),
    seamTopBottom: edgeDistance(data, width, height, "topBottom"),
  };
}

function luminance(r: number, g: number, b: number): number {
  return 0.299 * r + 0.587 * g + 0.114 * b;
}

function average(values: number[]): number {
  let sum = 0;
  for (const v of values) {
    sum += v;
  }
  return values.length > 0 ? sum / values.length : 0;
}

function std(values: number[] | Float64Array): number {
  const length = values.length;
  if (length === 0) {
    return 0;
  }
  let sum = 0;
  for (const v of values) {
    sum += v;
  }
  const mean = sum / length;
  let variance = 0;
  for (const v of values) {
    variance += (v - mean) * (v - mean);
  }
  return Math.sqrt(variance / length);
}

function percentile(sortedAscending: number[], p: number): number {
  if (sortedAscending.length === 0) {
    return 0;
  }
  const index = Math.min(sortedAscending.length - 1, Math.max(0, Math.round((p / 100) * (sortedAscending.length - 1))));
  return sortedAscending[index]!;
}

function maxCount(counts: Map<string, number>): number {
  let max = 0;
  for (const value of counts.values()) {
    if (value > max) {
      max = value;
    }
  }
  return max;
}

function dominantLuminanceBinShare(values: number[]): number {
  if (values.length === 0) {
    return 0;
  }
  const bins = new Array<number>(LUMINANCE_BINS).fill(0);
  for (const l of values) {
    const bin = Math.min(LUMINANCE_BINS - 1, Math.floor((l / 256) * LUMINANCE_BINS));
    bins[bin] = (bins[bin] ?? 0) + 1;
  }
  return Math.max(...bins) / values.length;
}

function axisMeans(grid: Float64Array, width: number, height: number, axis: "row" | "col"): number[] {
  if (axis === "row") {
    const means: number[] = [];
    for (let y = 0; y < height; y += 1) {
      let sum = 0;
      for (let x = 0; x < width; x += 1) {
        sum += grid[y * width + x]!;
      }
      means.push(sum / width);
    }
    return means;
  }
  const means: number[] = [];
  for (let x = 0; x < width; x += 1) {
    let sum = 0;
    for (let y = 0; y < height; y += 1) {
      sum += grid[y * width + x]!;
    }
    means.push(sum / height);
  }
  return means;
}

interface DownsampledGrid {
  values: number[];
  width: number;
  height: number;
}

// Average-pool by an integer factor. Returns undefined when the texture is not
// evenly divisible (the scale measure is then reported as NaN rather than faked).
function boxDownsample(grid: Float64Array, width: number, height: number, factor: number): DownsampledGrid | undefined {
  if (width % factor !== 0 || height % factor !== 0) {
    return undefined;
  }
  const outWidth = width / factor;
  const outHeight = height / factor;
  const values: number[] = [];
  for (let oy = 0; oy < outHeight; oy += 1) {
    for (let ox = 0; ox < outWidth; ox += 1) {
      let sum = 0;
      for (let dy = 0; dy < factor; dy += 1) {
        for (let dx = 0; dx < factor; dx += 1) {
          sum += grid[(oy * factor + dy) * width + (ox * factor + dx)]!;
        }
      }
      values.push(sum / (factor * factor));
    }
  }
  return { values, width: outWidth, height: outHeight };
}

function localContrast(grid: Float64Array, width: number, height: number): number {
  let sum = 0;
  let samples = 0;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const here = grid[y * width + x]!;
      const right = grid[y * width + ((x + 1) % width)]!;
      const down = grid[((y + 1) % height) * width + x]!;
      sum += Math.abs(here - right) + Math.abs(here - down);
      samples += 2;
    }
  }
  return samples > 0 ? sum / samples : 0;
}

interface BlobStats {
  max: number;
  mean: number;
  count: number;
}

// Connected-component sizes of the pixels passing `select` (e.g. the darkest
// tones), on a torus so tile-wrapping clusters count as one. This is what tells
// "1px scattered pits" (vanilla) apart from "one big pit" (oversized feature).
function blobStats(grid: Float64Array, width: number, height: number, select: (l: number) => boolean): BlobStats {
  const visited = new Uint8Array(width * height);
  const sizes: number[] = [];
  for (let start = 0; start < width * height; start += 1) {
    if (visited[start] || !select(grid[start]!)) {
      continue;
    }
    let size = 0;
    const stack = [start];
    visited[start] = 1;
    while (stack.length > 0) {
      const index = stack.pop()!;
      size += 1;
      const x = index % width;
      const y = Math.floor(index / width);
      const neighbors = [
        ((x + 1) % width) + y * width,
        ((x - 1 + width) % width) + y * width,
        x + ((y + 1) % height) * width,
        x + ((y - 1 + height) % height) * width,
      ];
      for (const n of neighbors) {
        if (!visited[n] && select(grid[n]!)) {
          visited[n] = 1;
          stack.push(n);
        }
      }
    }
    sizes.push(size);
  }
  return {
    max: sizes.length > 0 ? Math.max(...sizes) : 0,
    mean: sizes.length > 0 ? average(sizes) : 0,
    count: sizes.length,
  };
}

// Mean RGBA distance (0..1) across the wrap edge. Lower means a quieter seam.
function edgeDistance(data: Uint8Array, width: number, height: number, axis: "leftRight" | "topBottom"): number {
  let sum = 0;
  let samples = 0;
  if (axis === "leftRight") {
    for (let y = 0; y < height; y += 1) {
      sum += rgbaDistance(data, (y * width + (width - 1)) * 4, (y * width + 0) * 4);
      samples += 1;
    }
  } else {
    for (let x = 0; x < width; x += 1) {
      sum += rgbaDistance(data, ((height - 1) * width + x) * 4, (0 * width + x) * 4);
      samples += 1;
    }
  }
  return samples > 0 ? sum / samples : 0;
}

function rgbaDistance(data: Uint8Array, offsetA: number, offsetB: number): number {
  let sum = 0;
  for (let c = 0; c < 4; c += 1) {
    const d = (data[offsetA + c]! - data[offsetB + c]!) / 255;
    sum += d * d;
  }
  return Math.sqrt(sum / 4);
}

// --- Reporting -------------------------------------------------------------

interface FeatureRow {
  label: string;
  value: (f: TextureFeatures) => number;
  digits?: number;
  hint?: string;
}

const FEATURE_ROWS: FeatureRow[] = [
  { label: "distinct colors", value: (f) => f.distinctColors, digits: 0, hint: "vanilla rock uses few" },
  { label: "distinct levels", value: (f) => f.distinctLevels, digits: 0 },
  { label: "luminance span", value: (f) => f.luminance.span, digits: 0, hint: "value range; narrow = subtle" },
  { label: "luminance mean", value: (f) => f.luminance.mean, digits: 0 },
  { label: "luminance std", value: (f) => f.luminance.std, digits: 1, hint: "overall contrast" },
  { label: "dominant color share", value: (f) => f.dominantColorShare, digits: 2, hint: "is there a main tone" },
  { label: "dominant bin share", value: (f) => f.dominantBinShare, digits: 2 },
  { label: "row-band std", value: (f) => f.rowBandStd, digits: 2, hint: "horizontal structure" },
  { label: "col-band std", value: (f) => f.colBandStd, digits: 2, hint: "vertical structure" },
  { label: "anisotropy (row/col)", value: (f) => f.anisotropy, digits: 2, hint: ">1 horizontal, <1 vertical" },
  { label: "std full", value: (f) => f.stdFull, digits: 1 },
  { label: "std quarter", value: (f) => f.stdQuarter, digits: 1 },
  { label: "coarse retention", value: (f) => f.coarseRetention, digits: 2, hint: "high = large-scale, low = fine noise" },
  { label: "local contrast", value: (f) => f.localContrast, digits: 1, hint: "grain busyness" },
  { label: "dark blob max", value: (f) => f.darkBlobMax, digits: 0, hint: "biggest dark cluster (px)" },
  { label: "dark blob mean", value: (f) => f.darkBlobMean, digits: 1 },
  { label: "light blob max", value: (f) => f.lightBlobMax, digits: 0, hint: "biggest light cluster (px)" },
  { label: "light blob mean", value: (f) => f.lightBlobMean, digits: 1 },
  { label: "seam left-right", value: (f) => f.seamLeftRight, digits: 3, hint: "0 = perfect wrap" },
  { label: "seam top-bottom", value: (f) => f.seamTopBottom, digits: 3 },
];

export function formatComparison(name: string, candidate: TextureFeatures, reference?: TextureFeatures): string {
  const lines: string[] = [];
  lines.push(`# ${name}  (${candidate.width}x${candidate.height})`);
  if (reference) {
    lines.push(`reference: ${reference.width}x${reference.height} vanilla`);
  } else {
    lines.push("reference: none found (showing candidate only)");
  }
  lines.push("");
  const labelWidth = Math.max(...FEATURE_ROWS.map((row) => row.label.length));
  const header = `${"feature".padEnd(labelWidth)}  ${"reference".padStart(10)}  ${"candidate".padStart(10)}  ${"Δ".padStart(9)}   notes`;
  lines.push(header);
  lines.push("-".repeat(header.length));
  for (const row of FEATURE_ROWS) {
    const cand = row.value(candidate);
    const candText = fmt(cand, row.digits ?? 1);
    if (reference) {
      const ref = row.value(reference);
      const refText = fmt(ref, row.digits ?? 1);
      const delta = cand - ref;
      const deltaText = (delta >= 0 ? "+" : "") + fmt(delta, row.digits ?? 1);
      lines.push(
        `${row.label.padEnd(labelWidth)}  ${refText.padStart(10)}  ${candText.padStart(10)}  ${deltaText.padStart(9)}   ${row.hint ?? ""}`,
      );
    } else {
      lines.push(`${row.label.padEnd(labelWidth)}  ${"—".padStart(10)}  ${candText.padStart(10)}  ${"—".padStart(9)}   ${row.hint ?? ""}`);
    }
  }
  return lines.join("\n");
}

function fmt(value: number, digits: number): string {
  if (Number.isNaN(value)) {
    return "n/a";
  }
  return value.toFixed(digits);
}
