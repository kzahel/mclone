import type { RgbaImage } from "./png";

// Objective structural features for a single texture tile. The same function is
// run on the local vanilla reference (to get target numbers) and on a candidate
// (to get actuals), so the two are directly comparable. These are deliberately
// AGGREGATE statistics — never a per-pixel comparison to the reference — so they
// guide and guard authoring without becoming a way to trace the original art.
//
// IMPORTANT: comparison happens at a shared resolution. The candidate is area-
// downsampled to the vanilla grid (usually 16x16) before this runs, so scale-
// sensitive measures (banding, local contrast, blob sizes) are apples-to-apples
// instead of being skewed by a 32x32 tile simply having 4x the pixels. See
// matchResolution in analyze.ts.
//
// What each group answers, in plain terms:
// - palette:   how many tones, how wide the value range, is there a dominant mid
// - color:     what hue/saturation/temperature the material sits at (not just value)
// - banding:   is structure horizontal, vertical, or isotropic (the row/col test)
// - scale:     is the variation large-scale or fine noise (does it survive shrink)
// - defects:   sparkle pixels and one-way lighting gradients (authoring mistakes)
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
  quantizedColors: number;
  distinctLevels: number;
  luminance: LuminanceStats;
  meanHueDeg: number;
  hueSpreadDeg: number;
  meanSaturation: number;
  warmCool: number;
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
  sparklePixels: number;
  lightingBiasLR: number;
  lightingBiasTB: number;
  darkBlobMax: number;
  darkBlobMaxShare: number;
  darkBlobMean: number;
  darkBlobCount: number;
  lightBlobMax: number;
  lightBlobMaxShare: number;
  lightBlobMean: number;
  lightBlobCount: number;
  seamLeftRight: number;
  seamTopBottom: number;
}

const DARK_PERCENTILE = 12;
const LIGHT_PERCENTILE = 88;
const LUMINANCE_BINS = 8;
// A pixel brighter (or darker) than all four neighbours by more than this many
// luminance steps is a "sparkle" — the kind of lone high-contrast pixel that
// twinkles at distance. Measured on the shared comparison grid.
const SPARKLE_THRESHOLD = 40;
// Channel rounding for the perceptual-ish color count. Exact RGBA counts explode
// after opacity blending and noise; bucketing to 16-level channels collapses the
// near-duplicates so the number tracks "how many real tones" more honestly.
const COLOR_QUANTIZE_STEP = 16;

export function analyzeTexture(image: RgbaImage): TextureFeatures {
  const { width, height, data } = image;
  const pixelCount = width * height;

  // Per-pixel luminance grid; transparent pixels are filled with the opaque
  // mean afterwards so they do not invent structure in banding/scale measures.
  const grid = new Float64Array(pixelCount);
  const opaqueAlpha = new Uint8Array(pixelCount);
  const opaqueLum: number[] = [];
  const colorCounts = new Map<string, number>();
  const quantizedColors = new Set<string>();
  let transparent = 0;
  let sumSaturation = 0;
  let sumWarmCool = 0;
  let hueSin = 0;
  let hueCos = 0;
  let hueWeight = 0;

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
    opaqueAlpha[i] = 1;
    opaqueLum.push(l);
    const key = `${r},${g},${b},${a}`;
    colorCounts.set(key, (colorCounts.get(key) ?? 0) + 1);
    quantizedColors.add(`${quantize(r)},${quantize(g)},${quantize(b)}`);

    const max = Math.max(r, g, b);
    const min = Math.min(r, g, b);
    const chroma = max - min;
    sumSaturation += max > 0 ? chroma / max : 0;
    sumWarmCool += (r - b) / 255;
    if (chroma > 0) {
      const hueRad = (hueDegrees(r, g, b, max, chroma) * Math.PI) / 180;
      hueSin += chroma * Math.sin(hueRad);
      hueCos += chroma * Math.cos(hueRad);
      hueWeight += chroma;
    }
  }

  const opaqueCount = opaqueLum.length;
  const mean = opaqueCount > 0 ? average(opaqueLum) : 0;
  for (let i = 0; i < pixelCount; i += 1) {
    if (opaqueAlpha[i] === 0) {
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

  const bias = lightingBias(grid, width, height);

  return {
    width,
    height,
    opaquePixels: opaqueCount,
    transparentShare: pixelCount > 0 ? transparent / pixelCount : 0,
    distinctColors: colorCounts.size,
    quantizedColors: quantizedColors.size,
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
    meanHueDeg: hueWeight > 0 ? wrapDegrees((Math.atan2(hueSin, hueCos) * 180) / Math.PI) : Number.NaN,
    hueSpreadDeg: hueWeight > 0 ? hueSpread(hueSin, hueCos, hueWeight) : Number.NaN,
    meanSaturation: opaqueCount > 0 ? sumSaturation / opaqueCount : 0,
    warmCool: opaqueCount > 0 ? sumWarmCool / opaqueCount : 0,
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
    sparklePixels: sparkleCount(grid, opaqueAlpha, width, height),
    lightingBiasLR: bias.leftRight,
    lightingBiasTB: bias.topBottom,
    darkBlobMax: dark.max,
    darkBlobMaxShare: pixelCount > 0 ? dark.max / pixelCount : 0,
    darkBlobMean: dark.mean,
    darkBlobCount: dark.count,
    lightBlobMax: light.max,
    lightBlobMaxShare: pixelCount > 0 ? light.max / pixelCount : 0,
    lightBlobMean: light.mean,
    lightBlobCount: light.count,
    seamLeftRight: edgeDistance(data, width, height, "leftRight"),
    seamTopBottom: edgeDistance(data, width, height, "topBottom"),
  };
}

function luminance(r: number, g: number, b: number): number {
  return 0.299 * r + 0.587 * g + 0.114 * b;
}

function quantize(value: number): number {
  return Math.round(value / COLOR_QUANTIZE_STEP) * COLOR_QUANTIZE_STEP;
}

// Standard HSV hue in degrees [0,360) given the precomputed max and chroma.
function hueDegrees(r: number, g: number, b: number, max: number, chroma: number): number {
  let hue: number;
  if (max === r) {
    hue = ((g - b) / chroma) % 6;
  } else if (max === g) {
    hue = (b - r) / chroma + 2;
  } else {
    hue = (r - g) / chroma + 4;
  }
  return wrapDegrees(hue * 60);
}

function wrapDegrees(value: number): number {
  return ((value % 360) + 360) % 360;
}

// Circular standard deviation (degrees) from accumulated chroma-weighted sin/cos.
// Near-zero resultant length means the hue is spread all the way around the
// wheel; a resultant length near 1 means a tight, well-defined hue.
function hueSpread(sin: number, cos: number, weight: number): number {
  const resultant = Math.min(1, Math.sqrt(sin * sin + cos * cos) / weight);
  if (resultant <= 1e-9) {
    return 180;
  }
  return (Math.sqrt(-2 * Math.log(resultant)) * 180) / Math.PI;
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

// Lone high-contrast pixels: opaque cells that sit further than SPARKLE_THRESHOLD
// from every one of their four neighbours, in the same direction. These are the
// single pixels that sparkle/twinkle as the texture mips down at distance.
function sparkleCount(grid: Float64Array, opaqueAlpha: Uint8Array, width: number, height: number): number {
  let count = 0;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const index = y * width + x;
      if (opaqueAlpha[index] === 0) {
        continue;
      }
      const here = grid[index]!;
      const right = grid[y * width + ((x + 1) % width)]!;
      const left = grid[y * width + ((x - 1 + width) % width)]!;
      const down = grid[((y + 1) % height) * width + x]!;
      const up = grid[((y - 1 + height) % height) * width + x]!;
      const maxNeighbor = Math.max(right, left, down, up);
      const minNeighbor = Math.min(right, left, down, up);
      if (here - maxNeighbor > SPARKLE_THRESHOLD || minNeighbor - here > SPARKLE_THRESHOLD) {
        count += 1;
      }
    }
  }
  return count;
}

// Mean luminance difference across the tile: left edge vs right edge, top vs
// bottom. A value far from zero is a one-way lighting gradient baked into the
// art, which tiles into visible stripes and breaks rotation-safety.
function lightingBias(grid: Float64Array, width: number, height: number): { leftRight: number; topBottom: number } {
  let left = 0;
  let right = 0;
  for (let y = 0; y < height; y += 1) {
    left += grid[y * width]!;
    right += grid[y * width + (width - 1)]!;
  }
  let top = 0;
  let bottom = 0;
  for (let x = 0; x < width; x += 1) {
    top += grid[x]!;
    bottom += grid[(height - 1) * width + x]!;
  }
  return {
    leftRight: height > 0 ? (left - right) / height : 0,
    topBottom: width > 0 ? (top - bottom) / width : 0,
  };
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
  // Soft step size: roughly one "notable" difference in this feature's units.
  // Used only to draw the ↑/↓/• hint and to rank the biggest gaps. It is NOT a
  // pass/fail threshold — a strong texture can legitimately sit outside it.
  scale?: number;
  // Hue lives on a circle, so its candidate-vs-reference delta wraps at 360.
  circular?: boolean;
  // Display as a percentage rather than a raw fraction.
  percent?: boolean;
  // Counted/inspected but excluded from the ↑/↓ hint and gap ranking because the
  // raw number is known to be noisy (exact post-blend color count).
  informational?: boolean;
}

const FEATURE_ROWS: FeatureRow[] = [
  { label: "distinct colors", value: (f) => f.distinctColors, digits: 0, informational: true, hint: "exact RGBA; noisy after blending" },
  { label: "quantized colors", value: (f) => f.quantizedColors, digits: 0, scale: 4, hint: "tones at 16-level buckets" },
  { label: "distinct levels", value: (f) => f.distinctLevels, digits: 0, informational: true, hint: "exact levels; inflated by resample" },
  { label: "luminance span", value: (f) => f.luminance.span, digits: 0, scale: 15, hint: "value range; narrow = subtle" },
  { label: "luminance mean", value: (f) => f.luminance.mean, digits: 0, scale: 15 },
  { label: "luminance std", value: (f) => f.luminance.std, digits: 1, scale: 4, hint: "overall contrast" },
  { label: "dominant hue (deg)", value: (f) => f.meanHueDeg, digits: 0, scale: 8, circular: true, hint: "n/a if near-neutral" },
  { label: "hue spread (deg)", value: (f) => f.hueSpreadDeg, digits: 0, scale: 10, hint: "how varied the hue is" },
  { label: "mean saturation", value: (f) => f.meanSaturation, digits: 3, scale: 0.04, hint: "chroma; rock is low" },
  { label: "warm-cool (R-B)", value: (f) => f.warmCool, digits: 3, scale: 0.05, hint: ">0 warm, <0 cool" },
  { label: "dominant bin share", value: (f) => f.dominantBinShare, digits: 2, scale: 0.1, hint: "is there a main tone" },
  { label: "row-band std", value: (f) => f.rowBandStd, digits: 2, scale: 1.5, hint: "horizontal structure" },
  { label: "col-band std", value: (f) => f.colBandStd, digits: 2, scale: 1.5, hint: "vertical structure" },
  { label: "anisotropy (row/col)", value: (f) => f.anisotropy, digits: 2, scale: 0.3, hint: ">1 horizontal, <1 vertical" },
  { label: "coarse retention", value: (f) => f.coarseRetention, digits: 2, scale: 0.08, hint: "high = large-scale, low = fine noise" },
  { label: "local contrast", value: (f) => f.localContrast, digits: 1, scale: 4, hint: "grain busyness" },
  { label: "sparkle pixels", value: (f) => f.sparklePixels, digits: 0, scale: 2, hint: "lone twinkly pixels; lower is safer" },
  { label: "lighting bias L-R", value: (f) => f.lightingBiasLR, digits: 1, scale: 6, hint: "one-way gradient; near 0 is flat" },
  { label: "lighting bias T-B", value: (f) => f.lightingBiasTB, digits: 1, scale: 6 },
  { label: "dark blob max %", value: (f) => f.darkBlobMaxShare, digits: 2, scale: 0.02, percent: true, hint: "biggest dark cluster (tile %)" },
  { label: "light blob max %", value: (f) => f.lightBlobMaxShare, digits: 2, scale: 0.02, percent: true, hint: "biggest light cluster (tile %)" },
  { label: "seam left-right", value: (f) => f.seamLeftRight, digits: 3, scale: 0.04, hint: "0 = perfect wrap" },
  { label: "seam top-bottom", value: (f) => f.seamTopBottom, digits: 3, scale: 0.04 },
];

export interface FeatureGap {
  label: string;
  direction: "higher" | "lower";
  candidate: number;
  reference: number;
  deviation: number;
}

export interface ComparisonSizes {
  candidateNative?: string | undefined;
  referenceNative?: string | undefined;
  comparedAt?: string | undefined;
}

// Signed deviation in "scale" units: how many notable steps the candidate sits
// from the reference. Sign gives the ↑/↓ direction; magnitude ranks the gaps.
function deviation(row: FeatureRow, candidate: TextureFeatures, reference: TextureFeatures): number {
  const cand = row.value(candidate);
  const ref = row.value(reference);
  if (!Number.isFinite(cand) || !Number.isFinite(ref) || !row.scale) {
    return Number.NaN;
  }
  const delta = row.circular ? circularDelta(cand, ref) : cand - ref;
  return delta / row.scale;
}

function circularDelta(a: number, b: number): number {
  return ((a - b + 540) % 360) - 180;
}

function readHint(dev: number): string {
  if (!Number.isFinite(dev)) {
    return " ";
  }
  if (Math.abs(dev) <= 1) {
    return "•";
  }
  return dev > 0 ? "↑" : "↓";
}

// Rank the features the candidate is furthest from vanilla on. This is the
// worst-offender list an iteration loop attacks first — work the top item, then
// re-measure. Informational and undefined-delta rows are excluded.
export function featureGaps(candidate: TextureFeatures, reference: TextureFeatures, limit = 3): FeatureGap[] {
  const gaps: FeatureGap[] = [];
  for (const row of FEATURE_ROWS) {
    if (row.informational) {
      continue;
    }
    const dev = deviation(row, candidate, reference);
    if (!Number.isFinite(dev) || Math.abs(dev) <= 1) {
      continue;
    }
    gaps.push({
      label: row.label,
      direction: dev > 0 ? "higher" : "lower",
      candidate: row.value(candidate),
      reference: row.value(reference),
      deviation: dev,
    });
  }
  gaps.sort((a, b) => Math.abs(b.deviation) - Math.abs(a.deviation));
  return gaps.slice(0, limit);
}

export function formatComparison(
  name: string,
  candidate: TextureFeatures,
  reference?: TextureFeatures,
  sizes: ComparisonSizes = {},
): string {
  const lines: string[] = [];
  const candidateSize = sizes.candidateNative ?? `${candidate.width}x${candidate.height}`;
  lines.push(`# ${name}  (candidate ${candidateSize})`);
  if (reference) {
    const referenceSize = sizes.referenceNative ?? `${reference.width}x${reference.height}`;
    const comparedAt = sizes.comparedAt ?? `${candidate.width}x${candidate.height}`;
    lines.push(`reference: ${referenceSize} vanilla   ·   compared at ${comparedAt} (apples-to-apples)`);
  } else {
    lines.push("reference: none found (showing candidate only)");
  }
  lines.push("");
  const labelWidth = Math.max(...FEATURE_ROWS.map((row) => row.label.length));
  const header = `${"feature".padEnd(labelWidth)}  ${"reference".padStart(10)}  ${"candidate".padStart(10)}  ${"vs".padStart(3)}   notes`;
  lines.push(header);
  lines.push("-".repeat(header.length));
  for (const row of FEATURE_ROWS) {
    const candText = formatValue(row, row.value(candidate));
    if (reference) {
      const refText = formatValue(row, row.value(reference));
      const read = row.informational ? " " : readHint(deviation(row, candidate, reference));
      lines.push(
        `${row.label.padEnd(labelWidth)}  ${refText.padStart(10)}  ${candText.padStart(10)}  ${read.padStart(3)}   ${row.hint ?? ""}`,
      );
    } else {
      lines.push(`${row.label.padEnd(labelWidth)}  ${"—".padStart(10)}  ${candText.padStart(10)}  ${"—".padStart(3)}   ${row.hint ?? ""}`);
    }
  }

  lines.push("");
  if (reference) {
    const gaps = featureGaps(candidate, reference);
    if (gaps.length === 0) {
      lines.push("biggest gaps vs vanilla: none stand out — candidate sits close on every measure.");
    } else {
      lines.push("biggest gaps vs vanilla (work these first):");
      for (const [index, gap] of gaps.entries()) {
        const arrow = gap.direction === "higher" ? "↑" : "↓";
        const candText = formatGapValue(gap.label, gap.candidate);
        const refText = formatGapValue(gap.label, gap.reference);
        lines.push(`  ${index + 1}. ${gap.label.padEnd(labelWidth)} ${arrow}  ${candText} vs ${refText}`);
      }
    }
    lines.push("(directional guidance, not a pass/fail — a strong texture can sit outside these.)");
  } else {
    lines.push("biggest gaps: no vanilla reference found — candidate values only.");
  }
  return lines.join("\n");
}

// --- A/B candidate comparison ----------------------------------------------

// Two candidates count as tied on a feature unless their distances to vanilla
// differ by more than this many scale units, so noise does not decide a winner.
const CLOSENESS_MARGIN = 0.25;

export type Closer = "a" | "b" | "tie" | "n/a";

export interface FeatureClosenessRow {
  label: string;
  reference: number;
  a: number;
  b: number;
  devA: number;
  devB: number;
  closer: Closer;
}

export interface CandidateComparison {
  hasReference: boolean;
  rows: FeatureClosenessRow[];
  aWins: number;
  bWins: number;
  ties: number;
  totalDistanceA: number;
  totalDistanceB: number;
  winner: Closer;
}

// Rank two candidates by how close each sits to the same vanilla reference,
// feature by feature. The winner is decided by how many scored features each is
// closer on, tie-broken by total distance to vanilla. This is the mechanical
// tiebreaker for the tournament step of the iteration loop: pick the closer
// candidate, then keep iterating from it. With no reference it still reports the
// raw a-vs-b differences but declares no winner.
export function compareCandidates(
  a: TextureFeatures,
  b: TextureFeatures,
  reference?: TextureFeatures,
): CandidateComparison {
  const rows: FeatureClosenessRow[] = [];
  let aWins = 0;
  let bWins = 0;
  let ties = 0;
  let totalDistanceA = 0;
  let totalDistanceB = 0;

  for (const row of FEATURE_ROWS) {
    if (row.informational || !row.scale) {
      continue;
    }
    const aValue = row.value(a);
    const bValue = row.value(b);
    if (!reference) {
      rows.push({ label: row.label, reference: Number.NaN, a: aValue, b: bValue, devA: Number.NaN, devB: Number.NaN, closer: "n/a" });
      continue;
    }
    const devA = deviation(row, a, reference);
    const devB = deviation(row, b, reference);
    if (!Number.isFinite(devA) || !Number.isFinite(devB)) {
      rows.push({ label: row.label, reference: row.value(reference), a: aValue, b: bValue, devA, devB, closer: "n/a" });
      continue;
    }
    const absA = Math.abs(devA);
    const absB = Math.abs(devB);
    totalDistanceA += absA;
    totalDistanceB += absB;
    let closer: Closer;
    if (Math.abs(absA - absB) <= CLOSENESS_MARGIN) {
      closer = "tie";
      ties += 1;
    } else if (absA < absB) {
      closer = "a";
      aWins += 1;
    } else {
      closer = "b";
      bWins += 1;
    }
    rows.push({ label: row.label, reference: row.value(reference), a: aValue, b: bValue, devA, devB, closer });
  }

  let winner: Closer = "tie";
  if (!reference) {
    winner = "n/a";
  } else if (aWins > bWins) {
    winner = "a";
  } else if (bWins > aWins) {
    winner = "b";
  } else if (totalDistanceA < totalDistanceB) {
    winner = "a";
  } else if (totalDistanceB < totalDistanceA) {
    winner = "b";
  }

  return { hasReference: Boolean(reference), rows, aWins, bWins, ties, totalDistanceA, totalDistanceB, winner };
}

export function formatCandidateComparison(
  labelA: string,
  labelB: string,
  comparison: CandidateComparison,
  sizes: ComparisonSizes & { referenceName?: string | undefined } = {},
): string {
  const lines: string[] = [];
  lines.push("# compare candidates");
  lines.push(`  a: ${labelA}`);
  lines.push(`  b: ${labelB}`);
  if (comparison.hasReference) {
    const name = sizes.referenceName ? `${sizes.referenceName} ` : "";
    const refSize = sizes.referenceNative ? `${sizes.referenceNative} ` : "";
    const comparedAt = sizes.comparedAt ? `   ·   compared at ${sizes.comparedAt}` : "";
    lines.push(`reference: ${name}${refSize}vanilla${comparedAt}`);
  } else {
    lines.push("reference: none — showing a vs b only (pass --reference-name <block> or --reference-png <path> to rank vs vanilla)");
  }
  lines.push("");

  const labelWidth = Math.max(...comparison.rows.map((row) => row.label.length), "feature".length);
  if (comparison.hasReference) {
    const header = `${"feature".padEnd(labelWidth)}  ${"vanilla".padStart(9)}  ${"a".padStart(9)}  ${"b".padStart(9)}  closer`;
    lines.push(header);
    lines.push("-".repeat(header.length));
    for (const row of comparison.rows) {
      lines.push(
        `${row.label.padEnd(labelWidth)}  ${formatGapValue(row.label, row.reference).padStart(9)}  ${formatGapValue(row.label, row.a).padStart(9)}  ${formatGapValue(row.label, row.b).padStart(9)}  ${row.closer}`,
      );
    }
  } else {
    const header = `${"feature".padEnd(labelWidth)}  ${"a".padStart(9)}  ${"b".padStart(9)}  ${"Δ(b-a)".padStart(9)}`;
    lines.push(header);
    lines.push("-".repeat(header.length));
    for (const row of comparison.rows) {
      const delta = row.b - row.a;
      const deltaText = (delta >= 0 ? "+" : "") + formatGapValue(row.label, Math.abs(delta));
      lines.push(
        `${row.label.padEnd(labelWidth)}  ${formatGapValue(row.label, row.a).padStart(9)}  ${formatGapValue(row.label, row.b).padStart(9)}  ${(delta >= 0 ? deltaText : `-${formatGapValue(row.label, Math.abs(delta))}`).padStart(9)}`,
      );
    }
  }

  lines.push("");
  if (!comparison.hasReference) {
    lines.push("verdict: no vanilla reference, so no winner — differences only.");
    return lines.join("\n");
  }
  if (comparison.winner === "a" || comparison.winner === "b") {
    const winLetter = comparison.winner;
    const loseLetter = winLetter === "a" ? "b" : "a";
    const winLabel = winLetter === "a" ? labelA : labelB;
    const winCount = winLetter === "a" ? comparison.aWins : comparison.bWins;
    const loseCount = winLetter === "a" ? comparison.bWins : comparison.aWins;
    lines.push(
      `verdict: ${winLetter} (${winLabel}) is closer to vanilla — closer on ${winCount} features, ${loseLetter} on ${loseCount}, ${comparison.ties} tie(s).`,
    );
  } else {
    lines.push(`verdict: too close to call — a closer on ${comparison.aWins}, b on ${comparison.bWins}, ${comparison.ties} tie(s).`);
  }
  lines.push(
    `         total distance to vanilla: a ${comparison.totalDistanceA.toFixed(1)}, b ${comparison.totalDistanceB.toFixed(1)} (lower is closer)`,
  );
  lines.push("(tiebreaker guidance, not a pass/fail — use it to pick which candidate to keep iterating from.)");
  return lines.join("\n");
}

function formatValue(row: FeatureRow, value: number): string {
  if (Number.isNaN(value)) {
    return "n/a";
  }
  if (row.percent) {
    return `${(value * 100).toFixed(row.digits ?? 1)}%`;
  }
  return value.toFixed(row.digits ?? 1);
}

function formatGapValue(label: string, value: number): string {
  const row = FEATURE_ROWS.find((candidate) => candidate.label === label);
  return row ? formatValue(row, value) : value.toFixed(1);
}
