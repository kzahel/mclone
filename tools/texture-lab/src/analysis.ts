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
// - palette:   how many tones, how concentrated they are (top-8 share), whether
//              tones form connected planes or a speckle field (color run length)
// - color:     what hue/saturation/temperature the material sits at (not just value)
// - banding:   is structure horizontal, vertical, or isotropic (the row/col test)
// - grain:     dominant stroke direction and strength from gradients — catches
//              the diagonal streaks the axis-aligned banding test cannot see
// - scale:     is the variation large-scale or fine noise (does it survive shrink)
// - spread:    is the detail spread across the tile or bunched in one corner
//              (detail clustering — clustered detail stamps when tiled)
// - repeats:   strongest internal self-similarity at a non-trivial offset —
//              stamped motifs from masks/macro noise show up here
// - defects:   sparkle pixels and one-way lighting gradients (authoring mistakes)
// - blobs:     how big are the darkest/lightest clusters (catches oversized pits)
// - alpha:     cutout coverage, binary-alpha discipline (semi-alpha share),
//              silhouette structure (solid islands), and edge halo risk. These
//              are measured on the NATIVE tile, not the shared grid, because the
//              alpha-weighted area downsample manufactures semi-alpha edge
//              pixels that the authored art does not have.
// - seam:      does the tile wrap cleanly left-right and top-bottom
//
// Besides the scalar features, each texture also carries normalized luminance
// and hue histograms. Those never appear as rows on their own; they exist so a
// comparison can compute an Earth Mover's Distance between candidate and
// reference distributions (see PAIR_ROWS) — one scalar for "same tone/hue
// SHAPE", which mean/std/span all miss.

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
  semiAlphaShare: number;
  solidIslandCount: number;
  solidIslandMaxShare: number;
  edgeInteriorDelta: number;
  distinctColors: number;
  quantizedColors: number;
  top8ColorShare: number;
  distinctLevels: number;
  luminance: LuminanceStats;
  luminanceHistogram: number[];
  hueHistogram: number[];
  meanHueDeg: number;
  hueSpreadDeg: number;
  meanSaturation: number;
  warmCool: number;
  dominantColorShare: number;
  dominantBinShare: number;
  rowBandStd: number;
  colBandStd: number;
  anisotropy: number;
  grainDirectionality: number;
  grainAngleDeg: number;
  stdFull: number;
  stdHalf: number;
  stdQuarter: number;
  midRetention: number;
  coarseRetention: number;
  localContrast: number;
  detailClustering: number;
  colorRunLength: number;
  repetitionPeak: number;
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
// Palette concentration is measured as the pixel share covered by this many of
// the most common quantized colors. Vanilla terrain palettes are tiny (stone
// uses 4 exact colors; the 16x16 median is 9), so vanilla sits near 1.0 while
// continuous-tone output leaks share into a long color tail.
const TOP_COLOR_COVERAGE = 8;
// Self-correlation at shifts closer than this (torus Chebyshev distance) mostly
// measures smoothness — any coherent texture correlates with itself one pixel
// over. From this distance out, a high peak means a repeated motif: the stamped
// macro-noise/mask artifact the repeat panel shows.
const REPETITION_MIN_SHIFT = 3;
// Below this grain directionality the resultant angle is statistical noise (an
// isotropic 16x16 field lands near 0.055 by random walk alone), so the angle is
// reported as n/a instead of a random direction.
const GRAIN_ANGLE_MIN_DIRECTIONALITY = 0.1;
// The vanilla cutout shaders discard fragments with alpha < 0.1, so this is the
// render-visible silhouette threshold: at or above it a pixel draws at full
// color (cutout does not blend), below it the pixel vanishes.
const CUTOUT_SOLID_ALPHA = 26;
// Below this mean saturation a texture is effectively neutral. The vanilla
// split is bimodal: tint-driven grayscale (leaves 0.017, grass 0.005) and gray
// rock (andesite 0.015, gravel 0.043) sit at 0.00-0.04, while genuinely colored
// materials start at iron_ore's 0.082 (clay 0.10, sand 0.26, dirt 0.50). Hue
// direction, spread, and hue-distribution distance report n/a below this: the
// hue of a few faint pixels is noise, not a target to chase.
const NEUTRAL_SATURATION = 0.05;
// Block edge for the detail-clustering measure: local contrast is averaged per
// 4x4 block and the spread across blocks says whether detail covers the tile or
// bunches in one corner (which stamps when tiled).
const CLUSTERING_BLOCK = 4;
const LUMINANCE_HISTOGRAM_BINS = 32;
const HUE_HISTOGRAM_BINS = 36;

export function analyzeTexture(image: RgbaImage, alphaSource: RgbaImage = image): TextureFeatures {
  const { width, height, data } = image;
  const pixelCount = width * height;

  // Per-pixel luminance grid; transparent pixels are filled with the opaque
  // mean afterwards so they do not invent structure in banding/scale measures.
  const grid = new Float64Array(pixelCount);
  const opaqueAlpha = new Uint8Array(pixelCount);
  const opaqueLum: number[] = [];
  const colorCounts = new Map<string, number>();
  const quantizedCounts = new Map<string, number>();
  const quantIds = new Map<string, number>();
  // Per-pixel quantized-color id (-1 = transparent) for the run-length measure.
  const quantId = new Int32Array(pixelCount).fill(-1);
  let sumSaturation = 0;
  let sumWarmCool = 0;
  let hueSin = 0;
  let hueCos = 0;
  let hueWeight = 0;
  const hueHistogram = new Array<number>(HUE_HISTOGRAM_BINS).fill(0);

  for (let i = 0; i < pixelCount; i += 1) {
    const r = data[i * 4]!;
    const g = data[i * 4 + 1]!;
    const b = data[i * 4 + 2]!;
    const a = data[i * 4 + 3]!;
    const l = luminance(r, g, b);
    grid[i] = l;
    if (a === 0) {
      continue;
    }
    opaqueAlpha[i] = 1;
    opaqueLum.push(l);
    const key = `${r},${g},${b},${a}`;
    colorCounts.set(key, (colorCounts.get(key) ?? 0) + 1);
    const quantKey = `${quantize(r)},${quantize(g)},${quantize(b)}`;
    quantizedCounts.set(quantKey, (quantizedCounts.get(quantKey) ?? 0) + 1);
    let id = quantIds.get(quantKey);
    if (id === undefined) {
      id = quantIds.size;
      quantIds.set(quantKey, id);
    }
    quantId[i] = id;

    const max = Math.max(r, g, b);
    const min = Math.min(r, g, b);
    const chroma = max - min;
    sumSaturation += max > 0 ? chroma / max : 0;
    sumWarmCool += (r - b) / 255;
    if (chroma > 0) {
      const hueDeg = hueDegrees(r, g, b, max, chroma);
      const hueRad = (hueDeg * Math.PI) / 180;
      hueSin += chroma * Math.sin(hueRad);
      hueCos += chroma * Math.cos(hueRad);
      hueWeight += chroma;
      const bin = Math.min(HUE_HISTOGRAM_BINS - 1, Math.floor((hueDeg / 360) * HUE_HISTOGRAM_BINS));
      hueHistogram[bin] = hueHistogram[bin]! + chroma;
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
  const grain = grainDirection(grid, width, height);
  const alpha = alphaFeatures(alphaSource);
  const meanSaturation = opaqueCount > 0 ? sumSaturation / opaqueCount : 0;
  const hueDefined = hueWeight > 0 && meanSaturation >= NEUTRAL_SATURATION;
  if (hueWeight > 0) {
    for (let bin = 0; bin < HUE_HISTOGRAM_BINS; bin += 1) {
      hueHistogram[bin] = hueHistogram[bin]! / hueWeight;
    }
  }

  return {
    width,
    height,
    opaquePixels: opaqueCount,
    transparentShare: alpha.transparentShare,
    semiAlphaShare: alpha.semiAlphaShare,
    solidIslandCount: alpha.solidIslandCount,
    solidIslandMaxShare: alpha.solidIslandMaxShare,
    edgeInteriorDelta: alpha.edgeInteriorDelta,
    distinctColors: colorCounts.size,
    quantizedColors: quantizedCounts.size,
    top8ColorShare: topColorShare(quantizedCounts, opaqueCount),
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
    luminanceHistogram: luminanceHistogram(opaqueLum),
    hueHistogram,
    meanHueDeg: hueDefined ? wrapDegrees((Math.atan2(hueSin, hueCos) * 180) / Math.PI) : Number.NaN,
    hueSpreadDeg: hueDefined ? hueSpread(hueSin, hueCos, hueWeight) : Number.NaN,
    meanSaturation,
    warmCool: opaqueCount > 0 ? sumWarmCool / opaqueCount : 0,
    dominantColorShare: opaqueCount > 0 ? maxCount(colorCounts) / opaqueCount : 0,
    dominantBinShare: dominantLuminanceBinShare(opaqueLum),
    rowBandStd,
    colBandStd,
    anisotropy: rowBandStd / Math.max(colBandStd, 0.01),
    grainDirectionality: grain.directionality,
    grainAngleDeg: grain.angleDeg,
    stdFull,
    stdHalf,
    stdQuarter,
    midRetention: stdFull > 0 ? stdHalf / stdFull : 0,
    coarseRetention: stdFull > 0 ? stdQuarter / stdFull : 0,
    localContrast: localContrast(grid, width, height),
    detailClustering: detailClustering(grid, width, height),
    colorRunLength: colorRunLength(quantId, width, height),
    repetitionPeak: repetitionPeak(grid, width, height),
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

// Coefficient of variation of mean local contrast across CLUSTERING_BLOCK-sized
// blocks. localContrast says how much grain there is overall; this says whether
// it is spread across the tile (low) or bunched in one region (high). Bunched
// detail reads fine on the single tile but stamps visibly when tiled. NaN when
// the tile is smaller than two blocks (nothing to compare).
function detailClustering(grid: Float64Array, width: number, height: number): number {
  const blocksX = Math.ceil(width / CLUSTERING_BLOCK);
  const blocksY = Math.ceil(height / CLUSTERING_BLOCK);
  if (blocksX * blocksY < 2) {
    return Number.NaN;
  }
  const sums = new Float64Array(blocksX * blocksY);
  const counts = new Float64Array(blocksX * blocksY);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const here = grid[y * width + x]!;
      const right = grid[y * width + ((x + 1) % width)]!;
      const down = grid[((y + 1) % height) * width + x]!;
      const block = Math.floor(y / CLUSTERING_BLOCK) * blocksX + Math.floor(x / CLUSTERING_BLOCK);
      sums[block] = sums[block]! + (Math.abs(here - right) + Math.abs(here - down)) / 2;
      counts[block] = counts[block]! + 1;
    }
  }
  const blockMeans: number[] = [];
  for (let block = 0; block < sums.length; block += 1) {
    blockMeans.push(counts[block]! > 0 ? sums[block]! / counts[block]! : 0);
  }
  const mean = average(blockMeans);
  if (mean <= 1e-9) {
    return 0;
  }
  return std(blockMeans) / mean;
}

interface AlphaFeatures {
  transparentShare: number;
  semiAlphaShare: number;
  solidIslandCount: number;
  solidIslandMaxShare: number;
  edgeInteriorDelta: number;
}

// Alpha/cutout measures, computed on the NATIVE tile (see analyzeTexture's
// alphaSource): the alpha-weighted area downsample used to match resolutions
// manufactures semi-alpha edge pixels, so measuring these on the shared grid
// would flag clean binary-alpha art. "Solid" means at or above the vanilla
// cutout discard threshold — the render-visible silhouette. Island stats and
// the edge/interior split are NaN for a tile with no cutout (fully solid),
// where a silhouette does not exist.
function alphaFeatures(image: RgbaImage): AlphaFeatures {
  const { width, height, data } = image;
  const pixelCount = width * height;
  const solid = new Float64Array(pixelCount);
  let transparent = 0;
  let semi = 0;
  let solidCount = 0;
  for (let i = 0; i < pixelCount; i += 1) {
    const a = data[i * 4 + 3]!;
    if (a === 0) {
      transparent += 1;
    } else if (a < 255) {
      semi += 1;
    }
    if (a >= CUTOUT_SOLID_ALPHA) {
      solid[i] = 1;
      solidCount += 1;
    }
  }
  if (solidCount === pixelCount || solidCount === 0) {
    return {
      transparentShare: pixelCount > 0 ? transparent / pixelCount : 0,
      semiAlphaShare: pixelCount > 0 ? semi / pixelCount : 0,
      solidIslandCount: Number.NaN,
      solidIslandMaxShare: Number.NaN,
      edgeInteriorDelta: Number.NaN,
    };
  }
  const islands = blobStats(solid, width, height, (v) => v > 0.5);

  // Silhouette edge = solid pixel with a non-solid 4-neighbour (torus, so a
  // sprite surrounded by transparent margin behaves the same as a tiled leaf
  // texture with wrapping holes). A strongly negative delta is the dark fringe
  // left by compositing on black / anti-aliasing — the classic cutout halo.
  let edgeSum = 0;
  let edgeCount = 0;
  let interiorSum = 0;
  let interiorCount = 0;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const index = y * width + x;
      if (solid[index] === 0) {
        continue;
      }
      const l = luminance(data[index * 4]!, data[index * 4 + 1]!, data[index * 4 + 2]!);
      const isEdge =
        solid[y * width + ((x + 1) % width)] === 0 ||
        solid[y * width + ((x - 1 + width) % width)] === 0 ||
        solid[((y + 1) % height) * width + x] === 0 ||
        solid[((y - 1 + height) % height) * width + x] === 0;
      if (isEdge) {
        edgeSum += l;
        edgeCount += 1;
      } else {
        interiorSum += l;
        interiorCount += 1;
      }
    }
  }
  return {
    transparentShare: transparent / pixelCount,
    semiAlphaShare: semi / pixelCount,
    solidIslandCount: islands.count,
    solidIslandMaxShare: solidCount > 0 ? islands.max / solidCount : Number.NaN,
    edgeInteriorDelta:
      edgeCount > 0 && interiorCount > 0 ? edgeSum / edgeCount - interiorSum / interiorCount : Number.NaN,
  };
}

// Normalized luminance histogram over opaque pixels. Not reported as a row —
// it feeds the Earth Mover's Distance in PAIR_ROWS.
function luminanceHistogram(values: number[]): number[] {
  const bins = new Array<number>(LUMINANCE_HISTOGRAM_BINS).fill(0);
  if (values.length === 0) {
    return bins;
  }
  for (const l of values) {
    const bin = Math.min(LUMINANCE_HISTOGRAM_BINS - 1, Math.floor((l / 256) * LUMINANCE_HISTOGRAM_BINS));
    bins[bin] = bins[bin]! + 1;
  }
  return bins.map((count) => count / values.length);
}

// Earth Mover's Distance between two normalized linear histograms, reported in
// value units (0..255): the mean distance each unit of tone mass must travel to
// turn one distribution into the other. Unlike mean/std deltas it also sees
// shape differences — bimodal vs unimodal at the same mean scores > 0.
function linearEmd(a: number[], b: number[], valueRange: number): number {
  const bins = Math.max(a.length, b.length);
  let cumulative = 0;
  let total = 0;
  for (let bin = 0; bin < bins; bin += 1) {
    cumulative += (a[bin] ?? 0) - (b[bin] ?? 0);
    total += Math.abs(cumulative);
  }
  return total * (valueRange / bins);
}

// Circular Earth Mover's Distance for hue histograms, in degrees. The optimal
// transport on a circle shifts the linear cumulative-difference curve by its
// median (a standard result), so mass may move either way around the wheel;
// red-vs-magenta scores small even though they sit at opposite ends linearly.
function circularEmd(a: number[], b: number[], periodDegrees: number): number {
  const bins = Math.max(a.length, b.length);
  const cumulative: number[] = [];
  let running = 0;
  for (let bin = 0; bin < bins; bin += 1) {
    running += (a[bin] ?? 0) - (b[bin] ?? 0);
    cumulative.push(running);
  }
  const sorted = [...cumulative].sort((x, y) => x - y);
  const median = sorted[Math.floor(bins / 2)] ?? 0;
  let total = 0;
  for (const c of cumulative) {
    total += Math.abs(c - median);
  }
  return total * (periodDegrees / bins);
}

// Pixel share covered by the TOP_COLOR_COVERAGE most common quantized colors.
// Robust to blending noise (unlike the exact color count) and separates a
// compact authored palette from continuous-tone output with a long color tail.
function topColorShare(counts: Map<string, number>, opaqueCount: number): number {
  if (opaqueCount <= 0) {
    return 0;
  }
  const sorted = [...counts.values()].sort((a, b) => b - a);
  let covered = 0;
  for (let i = 0; i < Math.min(TOP_COLOR_COVERAGE, sorted.length); i += 1) {
    covered += sorted[i]!;
  }
  return covered / opaqueCount;
}

// Mean run length of same-quantized-color pixels along rows and columns, on the
// torus. Vanilla materials lay their few tones out in connected planes (long
// runs); a speckle field alternates tone almost every pixel (runs near 1). This
// is the "structure vs speckle" number for the flat-gray-plus-dots failure mode.
// Pairs involving transparent pixels are skipped rather than counted as breaks.
function colorRunLength(quantId: Int32Array, width: number, height: number): number {
  let pairs = 0;
  let transitions = 0;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const here = quantId[y * width + x]!;
      if (here < 0) {
        continue;
      }
      const right = quantId[y * width + ((x + 1) % width)]!;
      if (right >= 0) {
        pairs += 1;
        if (right !== here) {
          transitions += 1;
        }
      }
      const down = quantId[((y + 1) % height) * width + x]!;
      if (down >= 0) {
        pairs += 1;
        if (down !== here) {
          transitions += 1;
        }
      }
    }
  }
  return pairs > 0 ? pairs / Math.max(1, transitions) : 0;
}

// Strongest normalized luminance self-correlation over all torus shifts at
// least REPETITION_MIN_SHIFT away from the origin. A stamped motif (repeated
// mask, periodic macro noise) correlates strongly with itself at the stamp
// period; organic vanilla tiles stay low everywhere. 0..1, higher = more
// visible internal repetition.
function repetitionPeak(grid: Float64Array, width: number, height: number): number {
  const pixelCount = width * height;
  let mean = 0;
  for (let i = 0; i < pixelCount; i += 1) {
    mean += grid[i]!;
  }
  mean /= Math.max(1, pixelCount);
  const centered = new Float64Array(pixelCount);
  let denom = 0;
  for (let i = 0; i < pixelCount; i += 1) {
    centered[i] = grid[i]! - mean;
    denom += centered[i]! * centered[i]!;
  }
  if (denom <= 1e-9) {
    return 0;
  }
  let best = 0;
  for (let shiftY = 0; shiftY < height; shiftY += 1) {
    const torusY = Math.min(shiftY, height - shiftY);
    for (let shiftX = 0; shiftX < width; shiftX += 1) {
      const torusX = Math.min(shiftX, width - shiftX);
      if (Math.max(torusX, torusY) < REPETITION_MIN_SHIFT) {
        continue;
      }
      let sum = 0;
      for (let y = 0; y < height; y += 1) {
        const shiftedRow = ((y + shiftY) % height) * width;
        const row = y * width;
        for (let x = 0; x < width; x += 1) {
          sum += centered[row + x]! * centered[shiftedRow + ((x + shiftX) % width)]!;
        }
      }
      const r = sum / denom;
      if (r > best) {
        best = r;
      }
    }
  }
  return best;
}

// Dominant stroke/grain direction from Sobel gradients on the torus. Gradient
// orientations are axial (a stroke at 30° and 210° is the same grain), so they
// are accumulated as doubled angles weighted by gradient magnitude; the
// resultant length is the directionality (0 = isotropic, 1 = every edge points
// one way). The angle is converted from gradient direction to stroke direction
// (0 = horizontal strokes, 90 = vertical) and reported only when the
// directionality is strong enough to be meaningful. This is the diagonal-aware
// complement to the axis-only row/col banding test, and the number to watch for
// rotation-safety.
function grainDirection(grid: Float64Array, width: number, height: number): { directionality: number; angleDeg: number } {
  let sumSin = 0;
  let sumCos = 0;
  let sumMagnitude = 0;
  for (let y = 0; y < height; y += 1) {
    const up = ((y - 1 + height) % height) * width;
    const down = ((y + 1) % height) * width;
    const row = y * width;
    for (let x = 0; x < width; x += 1) {
      const left = (x - 1 + width) % width;
      const right = (x + 1) % width;
      const gx =
        2 * grid[row + right]! + grid[up + right]! + grid[down + right]!
        - 2 * grid[row + left]! - grid[up + left]! - grid[down + left]!;
      const gy =
        2 * grid[down + x]! + grid[down + left]! + grid[down + right]!
        - 2 * grid[up + x]! - grid[up + left]! - grid[up + right]!;
      const magnitude = Math.sqrt(gx * gx + gy * gy);
      if (magnitude <= 0) {
        continue;
      }
      const doubled = 2 * Math.atan2(gy, gx);
      sumSin += magnitude * Math.sin(doubled);
      sumCos += magnitude * Math.cos(doubled);
      sumMagnitude += magnitude;
    }
  }
  if (sumMagnitude <= 1e-9) {
    return { directionality: 0, angleDeg: Number.NaN };
  }
  const directionality = Math.sqrt(sumSin * sumSin + sumCos * sumCos) / sumMagnitude;
  if (directionality < GRAIN_ANGLE_MIN_DIRECTIONALITY) {
    return { directionality, angleDeg: Number.NaN };
  }
  const gradientAngle = ((Math.atan2(sumSin, sumCos) / 2) * 180) / Math.PI;
  // Strokes run perpendicular to their luminance gradient.
  const angleDeg = (((gradientAngle + 90) % 180) + 180) % 180;
  return { directionality, angleDeg };
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
  // Period for circular quantities: hue wraps at 360, axial grain angle at 180.
  // The candidate-vs-reference delta wraps at this period.
  circularPeriod?: number;
  // Display as a percentage rather than a raw fraction.
  percent?: boolean;
  // Counted/inspected but excluded from the ↑/↓ hint and gap ranking because the
  // raw number is known to be noisy (exact post-blend color count).
  informational?: boolean;
}

const FEATURE_ROWS: FeatureRow[] = [
  { label: "distinct colors", value: (f) => f.distinctColors, digits: 0, informational: true, hint: "exact RGBA; noisy after blending" },
  { label: "quantized colors", value: (f) => f.quantizedColors, digits: 0, scale: 4, hint: "tones at 16-level buckets" },
  { label: "top-8 color share", value: (f) => f.top8ColorShare, digits: 0, scale: 0.06, percent: true, hint: "palette concentration; vanilla is compact" },
  { label: "dominant color share", value: (f) => f.dominantColorShare, digits: 0, percent: true, informational: true, hint: "exact top color; noisy after blending" },
  { label: "distinct levels", value: (f) => f.distinctLevels, digits: 0, informational: true, hint: "exact levels; inflated by resample" },
  { label: "luminance span", value: (f) => f.luminance.span, digits: 0, scale: 15, hint: "value range; narrow = subtle" },
  { label: "luminance mean", value: (f) => f.luminance.mean, digits: 0, scale: 15 },
  { label: "luminance std", value: (f) => f.luminance.std, digits: 1, scale: 4, hint: "overall contrast" },
  { label: "value skew", value: (f) => f.luminance.p90 - f.luminance.p50 - (f.luminance.p50 - f.luminance.p10), digits: 0, scale: 12, hint: ">0 light-tailed, <0 heavy shadows" },
  { label: "dominant hue (deg)", value: (f) => f.meanHueDeg, digits: 0, scale: 8, circularPeriod: 360, hint: "n/a if near-neutral" },
  { label: "hue spread (deg)", value: (f) => f.hueSpreadDeg, digits: 0, scale: 10, hint: "how varied the hue is" },
  { label: "mean saturation", value: (f) => f.meanSaturation, digits: 3, scale: 0.04, hint: "chroma; rock is low" },
  { label: "warm-cool (R-B)", value: (f) => f.warmCool, digits: 3, scale: 0.05, hint: ">0 warm, <0 cool" },
  { label: "dominant bin share", value: (f) => f.dominantBinShare, digits: 2, scale: 0.1, hint: "is there a main tone" },
  { label: "row-band std", value: (f) => f.rowBandStd, digits: 2, scale: 1.5, hint: "horizontal structure" },
  { label: "col-band std", value: (f) => f.colBandStd, digits: 2, scale: 1.5, hint: "vertical structure" },
  { label: "anisotropy (row/col)", value: (f) => f.anisotropy, digits: 2, scale: 0.3, hint: ">1 horizontal, <1 vertical" },
  { label: "grain strength", value: (f) => f.grainDirectionality, digits: 2, scale: 0.08, hint: "0 isotropic; high = one-way strokes" },
  { label: "grain angle (deg)", value: (f) => f.grainAngleDeg, digits: 0, scale: 20, circularPeriod: 180, hint: "0 horiz, 90 vert; n/a if isotropic" },
  { label: "mid retention", value: (f) => f.midRetention, digits: 2, scale: 0.08, hint: "half-scale structure; the mid layer" },
  { label: "coarse retention", value: (f) => f.coarseRetention, digits: 2, scale: 0.08, hint: "high = large-scale, low = fine noise" },
  { label: "local contrast", value: (f) => f.localContrast, digits: 1, scale: 4, hint: "grain busyness" },
  { label: "detail clustering", value: (f) => f.detailClustering, digits: 2, scale: 0.2, hint: "high = detail bunched in one region" },
  { label: "color run length", value: (f) => f.colorRunLength, digits: 1, scale: 0.5, hint: "same-tone runs; near 1 = speckle field" },
  { label: "repetition peak", value: (f) => f.repetitionPeak, digits: 2, scale: 0.08, hint: "internal motif repeats; stamping is high" },
  { label: "sparkle pixels", value: (f) => f.sparklePixels, digits: 0, scale: 2, hint: "lone twinkly pixels; lower is safer" },
  { label: "lighting bias L-R", value: (f) => f.lightingBiasLR, digits: 1, scale: 6, hint: "one-way gradient; near 0 is flat" },
  { label: "lighting bias T-B", value: (f) => f.lightingBiasTB, digits: 1, scale: 6 },
  { label: "dark blob max %", value: (f) => f.darkBlobMaxShare, digits: 2, scale: 0.02, percent: true, hint: "biggest dark cluster (tile %)" },
  { label: "light blob max %", value: (f) => f.lightBlobMaxShare, digits: 2, scale: 0.02, percent: true, hint: "biggest light cluster (tile %)" },
  { label: "dark blob count", value: (f) => f.darkBlobCount, digits: 0, scale: 4, hint: "scattered pits vs one clump" },
  { label: "light blob count", value: (f) => f.lightBlobCount, digits: 0, scale: 4 },
  { label: "transparent share", value: (f) => f.transparentShare, digits: 0, scale: 0.05, percent: true, hint: "cutout coverage; measured at native res" },
  { label: "semi-alpha share", value: (f) => f.semiAlphaShare, digits: 1, scale: 0.01, percent: true, hint: "0<a<255 pixels; cutout wants ~0" },
  { label: "solid islands", value: (f) => f.solidIslandCount, digits: 0, scale: 3, hint: "silhouette pieces; n/a if no cutout" },
  { label: "island max %", value: (f) => f.solidIslandMaxShare, digits: 0, scale: 0.15, percent: true, hint: "biggest piece's share of solid pixels" },
  { label: "edge vs interior lum", value: (f) => f.edgeInteriorDelta, digits: 1, scale: 8, hint: "silhouette fringe; strongly <0 = dark halo" },
  { label: "seam left-right", value: (f) => f.seamLeftRight, digits: 3, scale: 0.04, hint: "0 = perfect wrap" },
  { label: "seam top-bottom", value: (f) => f.seamTopBottom, digits: 3, scale: 0.04 },
];

// Rows that only exist as a candidate-vs-reference distance (no single-texture
// value): Earth Mover's Distances between the stored histograms. The reference
// value is definitionally 0 ("vanilla is zero away from itself"), so deviation
// is always >= 0 and direction is always "higher". They join the gap ranking
// and the compare-mode tournament exactly like scalar rows.
interface PairRow {
  label: string;
  distance: (candidate: TextureFeatures, reference: TextureFeatures) => number;
  digits?: number;
  scale: number;
  hint?: string;
}

const PAIR_ROWS: PairRow[] = [
  {
    label: "luminance emd",
    distance: (c, r) => linearEmd(c.luminanceHistogram, r.luminanceHistogram, 256),
    digits: 1,
    scale: 8,
    hint: "tone distribution shape distance; 0 = same",
  },
  {
    label: "hue emd (deg)",
    distance: (c, r) =>
      c.meanSaturation >= NEUTRAL_SATURATION && r.meanSaturation >= NEUTRAL_SATURATION
        ? circularEmd(c.hueHistogram, r.hueHistogram, 360)
        : Number.NaN,
    digits: 0,
    scale: 10,
    hint: "hue distribution distance; n/a if neutral",
  },
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
  const delta = row.circularPeriod ? circularDelta(cand, ref, row.circularPeriod) : cand - ref;
  return delta / row.scale;
}

function circularDelta(a: number, b: number, period: number): number {
  return ((((a - b) % period) + period * 1.5) % period) - period / 2;
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
  for (const row of PAIR_ROWS) {
    const dist = row.distance(candidate, reference);
    const dev = dist / row.scale;
    if (!Number.isFinite(dev) || dev <= 1) {
      continue;
    }
    gaps.push({ label: row.label, direction: "higher", candidate: dist, reference: 0, deviation: dev });
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
  if (reference) {
    for (const row of PAIR_ROWS) {
      const dist = row.distance(candidate, reference);
      const distText = Number.isNaN(dist) ? "n/a" : dist.toFixed(row.digits ?? 1);
      const read = Number.isFinite(dist) ? readHint(dist / row.scale) : " ";
      lines.push(
        `${row.label.padEnd(labelWidth)}  ${"0".padStart(10)}  ${distText.padStart(10)}  ${read.padStart(3)}   ${row.hint ?? ""}`,
      );
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

  // Distribution distances join the tournament like scalar rows: each
  // candidate's value is its EMD to vanilla, and vanilla's own column is the
  // definitional 0. Without a reference there is no distance to score, so the
  // rows are omitted rather than shown as raw a-vs-b values.
  if (reference) {
    for (const row of PAIR_ROWS) {
      const distA = row.distance(a, reference);
      const distB = row.distance(b, reference);
      const devA = distA / row.scale;
      const devB = distB / row.scale;
      if (!Number.isFinite(devA) || !Number.isFinite(devB)) {
        rows.push({ label: row.label, reference: 0, a: distA, b: distB, devA, devB, closer: "n/a" });
        continue;
      }
      totalDistanceA += devA;
      totalDistanceB += devB;
      let closer: Closer;
      if (Math.abs(devA - devB) <= CLOSENESS_MARGIN) {
        closer = "tie";
        ties += 1;
      } else if (devA < devB) {
        closer = "a";
        aWins += 1;
      } else {
        closer = "b";
        bWins += 1;
      }
      rows.push({ label: row.label, reference: 0, a: distA, b: distB, devA, devB, closer });
    }
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

function formatValue(row: { digits?: number | undefined; percent?: boolean | undefined }, value: number): string {
  if (Number.isNaN(value)) {
    return "n/a";
  }
  if (row.percent) {
    return `${(value * 100).toFixed(row.digits ?? 1)}%`;
  }
  return value.toFixed(row.digits ?? 1);
}

function formatGapValue(label: string, value: number): string {
  const row =
    FEATURE_ROWS.find((candidate) => candidate.label === label) ??
    PAIR_ROWS.find((candidate) => candidate.label === label);
  return row ? formatValue(row, value) : value.toFixed(1);
}
