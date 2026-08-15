#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(scriptDirectory, "..");
const nativeManifest = join(repositoryRoot, "native", "Cargo.toml");
const executableName = process.platform === "win32"
  ? "mclone-native-client.exe"
  : "mclone-native-client";
const clientPath = join(repositoryRoot, "native", "target", "debug", executableName);

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

function parsePositiveInteger(flag, value, maximum) {
  const parsed = Number.parseInt(value ?? "", 10);
  if (!Number.isInteger(parsed) || parsed < 1 || parsed > maximum) {
    fail(`${flag} must be an integer between 1 and ${maximum}`);
  }
  return parsed;
}

function parseArguments(argv) {
  const options = {
    output: "/tmp/mclone-terrain-seam-review",
    width: 1280,
    height: 720,
    settleFrames: 90,
    skipBuild: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--") {
      continue;
    } else if (argument === "--output") {
      options.output = argv[++index] ?? fail("--output requires a directory");
    } else if (argument === "--width") {
      options.width = parsePositiveInteger("--width", argv[++index], 8192);
    } else if (argument === "--height") {
      options.height = parsePositiveInteger("--height", argv[++index], 8192);
    } else if (argument === "--settle-frames") {
      options.settleFrames = parsePositiveInteger(
        "--settle-frames",
        argv[++index],
        600,
      );
    } else if (argument === "--skip-build") {
      options.skipBuild = true;
    } else if (argument === "--help") {
      process.stdout.write(
        "Usage: node scripts/capture-terrain-seam-review.mjs "
        + "[--output /tmp/mclone-terrain-seam-review] [--width 1280] "
        + "[--height 720] [--settle-frames 90] [--skip-build]\n",
      );
      process.exit(0);
    } else {
      fail(`unknown argument: ${argument}`);
    }
  }
  options.output = resolve(options.output);
  return options;
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repositoryRoot,
    encoding: "utf8",
    stdio: options.capture ? "pipe" : "inherit",
  });
  if (result.error) {
    fail(`${command} failed to start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    if (options.capture) {
      process.stderr.write(result.stdout ?? "");
      process.stderr.write(result.stderr ?? "");
    }
    fail(`${command} exited with status ${result.status}`);
  }
  if (options.echo) {
    process.stdout.write(result.stdout ?? "");
    process.stderr.write(result.stderr ?? "");
  }
  return options.capture ? (result.stdout ?? "").trim() : "";
}

function vectorArgument(vector) {
  return vector.join(",");
}

const views = {
  elevated: {
    label: "accepted broad coast and horizon view",
    seed: 12345,
    chunk: [0, 0],
    eye: [8, 105, 8],
    target: [8, 65, -300],
  },
  low: {
    label: "accepted exact, voxel, smooth, water, and tree seam view",
    seed: 12345,
    chunk: [0, 0],
    eye: [8, 82, 8],
    target: [8, 67, -180],
  },
  coast: {
    label: "focused coast and open-water view",
    seed: 12345,
    chunk: [0, 0],
    eye: [8, 105, 8],
    target: [8, 65, -300],
  },
  forest: {
    label: "focused conifer forest view",
    seed: 12345,
    chunk: [-43, 6],
    eye: [-680, 132, 104],
    target: [-680, 78, -140],
  },
  stone: {
    label: "focused second-seed exposed-stone mountain view",
    seed: -98765,
    chunk: [-96, 72],
    eye: [-1528, 160, 1160],
    target: [-1528, 92, 900],
  },
  snow: {
    label: "focused alpine snow and exposed-rib view",
    seed: 12345,
    chunk: [-130, -69],
    eye: [-2072, 165, -1096],
    target: [-2072, 100, -1350],
  },
};

const times = [
  ["dawn", 0],
  ["noon", 6000],
  ["dusk", 12000],
  ["midnight", 18000],
];

const diagnostics = [
  ["ownership-level", 6000],
  ["topology", 6000],
  ["albedo", 6000],
  ["environmental-illumination", 18000],
  ["geometric-shade", 6000],
  ["local-occlusion", 6000],
  ["water", 6000],
  ["texture", 6000],
];

function captureDefinition({
  name,
  group,
  view,
  time,
  presentation = "composed",
  diagnostic = "natural",
}) {
  return { name, group, view, time, presentation, diagnostic };
}

function buildCampaign() {
  const captures = [];
  for (const view of ["elevated", "low"]) {
    for (const [timeLabel, time] of times) {
      captures.push(captureDefinition({
        name: `baseline-${view}-${timeLabel}`,
        group: `baseline-${view}-time-sweep`,
        view,
        time,
      }));
    }
  }
  for (const view of ["elevated", "low"]) {
    for (const [timeLabel, time] of [["noon", 6000], ["midnight", 18000]]) {
      captures.push(captureDefinition({
        name: `control-${view}-exact-only-${timeLabel}`,
        group: `control-${view}-exact-only`,
        view,
        time,
        presentation: "exact-only",
      }));
    }
  }
  // The accepted elevated baseline is itself the focused coast view. Avoid
  // writing duplicate PNGs while retaining that role in the receipt.
  for (const view of ["forest", "stone", "snow"]) {
    for (const [timeLabel, time] of [["noon", 6000], ["midnight", 18000]]) {
      captures.push(captureDefinition({
        name: `focus-${view}-${timeLabel}`,
        group: `focus-${view}`,
        view,
        time,
      }));
    }
  }
  for (const [diagnostic, time] of diagnostics) {
    captures.push(captureDefinition({
      name: `diagnostic-low-${diagnostic}`,
      group: "diagnostic-low",
      view: "low",
      time,
      diagnostic,
    }));
  }
  return captures;
}

function parseTerrainViewState(output, captureName) {
  const prefix = "MCLONE_TERRAIN_SEAM_STATE ";
  const line = output.split(/\r?\n/u).find((candidate) => candidate.startsWith(prefix));
  if (!line) {
    fail(`${captureName} did not report terrain seam readiness state`);
  }
  try {
    return JSON.parse(line.slice(prefix.length));
  } catch (error) {
    fail(`${captureName} reported invalid terrain seam state: ${error.message}`);
  }
}

function validateObservedState(capture, state) {
  if (capture.presentation === "exact-only") {
    if (state.enabled !== false) {
      fail(`${capture.name} allocated a terrain horizon in Exact Only mode`);
    }
    return;
  }
  if (!state.enabled || !state.targetReady || !state.exactCenterReady) {
    fail(`${capture.name} did not reach exact/procedural target readiness`);
  }
  if (state.pendingVegetationTiles !== 0
      || state.vegetationSubmittedJobs !== state.vegetationCompletedJobs
      || state.vegetationTransportFailures !== 0
      || state.vegetationJobFailures !== 0) {
    fail(`${capture.name} did not reach a drained, failure-free vegetation state`);
  }
}

function validateComparisonGroups(results) {
  const signatures = new Map();
  for (const capture of results) {
    const signature = JSON.stringify(capture.observedState);
    const prior = signatures.get(capture.group);
    if (prior !== undefined && prior !== signature) {
      fail(`${capture.group} changed observed source/coverage/vegetation state`);
    }
    signatures.set(capture.group, signature);
  }
}

function pngFacts(path, expectedWidth, expectedHeight) {
  const bytes = readFileSync(path);
  const signature = bytes.subarray(0, 8).toString("hex");
  if (signature !== "89504e470d0a1a0a") {
    fail(`${path} is not a PNG`);
  }
  const width = bytes.readUInt32BE(16);
  const height = bytes.readUInt32BE(20);
  if (width !== expectedWidth || height !== expectedHeight) {
    fail(`${path} is ${width}x${height}; expected ${expectedWidth}x${expectedHeight}`);
  }
  if (bytes.length < 4096) {
    fail(`${path} is suspiciously small (${bytes.length} bytes)`);
  }
  return {
    width,
    height,
    bytes: bytes.length,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  };
}

const options = parseArguments(process.argv.slice(2));
mkdirSync(options.output, { recursive: true });

if (!options.skipBuild || !existsSync(clientPath)) {
  run("cargo", [
    "build",
    "--manifest-path",
    nativeManifest,
    "-p",
    "mclone-native-client",
    "--bin",
    "mclone-native-client",
  ]);
}
if (!existsSync(clientPath)) {
  fail(`native client binary is missing after build: ${clientPath}`);
}

const revision = run("git", ["rev-parse", "HEAD"], { capture: true });
const campaign = buildCampaign();
const results = [];

for (let index = 0; index < campaign.length; index += 1) {
  const capture = campaign[index];
  const view = views[capture.view];
  const path = join(options.output, `${capture.name}.png`);
  const args = [
    "--screenshot", path,
    "--width", String(options.width),
    "--height", String(options.height),
    "--seed", String(view.seed),
    "--generation-profile", "mclone-overworld-v1",
    "--starter-content", "wild",
    "--chunk-x", String(view.chunk[0]),
    "--chunk-z", String(view.chunk[1]),
    "--render-distance", "2",
    "--terrain-presentation", capture.presentation,
    "--day-time", String(capture.time),
    "--freeze-time",
    "--transient",
    "--asset-pack", "original",
    "--render-color-profile", "vanilla",
    "--debug-passive-showcase", "false",
    "--adaptive-render-admission-budget", "false",
    "--lighting", "true",
    "--fullbright", "false",
    "--startup-wait", "view-settled",
    "--screenshot-settle-frames", String(options.settleFrames),
    "--screenshot-eye", vectorArgument(view.eye),
    "--screenshot-target", vectorArgument(view.target),
    "--screenshot-terrain-horizon-diagnostic", capture.diagnostic,
  ];
  process.stdout.write(
    `[${index + 1}/${campaign.length}] ${capture.name} `
    + `(seed ${view.seed}, ${capture.presentation}, tick ${capture.time}, `
    + `${capture.diagnostic})\n`,
  );
  const output = run(clientPath, args, { capture: true, echo: true });
  const observedState = parseTerrainViewState(output, capture.name);
  validateObservedState(capture, observedState);
  results.push({
    ...capture,
    file: `${capture.name}.png`,
    source: {
      generationProfile: "mclone-overworld-v1",
      seed: view.seed,
      chunk: view.chunk,
      starterContent: "wild",
      transient: true,
    },
    camera: {
      label: view.label,
      eye: view.eye,
      target: view.target,
    },
    render: {
      extent: [options.width, options.height],
      renderDistance: 2,
      assetPack: "original",
      colorProfile: "vanilla",
      lighting: true,
      fullbright: false,
      frozenTime: true,
      passiveShowcase: false,
      adaptiveRenderAdmission: false,
    },
    settledState: {
      startupWait: "view-settled",
      postSettleFrames: options.settleFrames,
      pacedFrameMilliseconds: 16,
    },
    command: [clientPath, ...args],
    observedState,
    png: pngFacts(path, options.width, options.height),
  });
}

validateComparisonGroups(results);

const receipt = {
  schema: "mclone-terrain-seam-review-v1",
  tactical: 309,
  revision,
  generatedAt: new Date().toISOString(),
  outputDirectory: options.output,
  captureCount: results.length,
  comparisonContract: {
    invariantAxes: [
      "generation profile",
      "seed within each named scene",
      "chunk interest",
      "camera eye and target",
      "render distance",
      "asset pack",
      "color profile",
      "lighting/fullbright",
      "startup readiness",
      "post-settle frames",
      "output extent",
    ],
    variedAxes: ["frozen time", "terrain presentation", "diagnostic channel"],
    observedStateRule: "Every composed capture is target-ready with drained, failure-free vegetation; observed state is identical within each comparison group. Exact Only reports the horizon disabled.",
  },
  sceneCoverage: {
    coast: ["baseline-elevated-dawn", "baseline-elevated-noon", "baseline-elevated-dusk", "baseline-elevated-midnight"],
    forest: ["focus-forest-noon", "focus-forest-midnight"],
    exposedStone: ["focus-stone-noon", "focus-stone-midnight"],
    snow: ["focus-snow-noon", "focus-snow-midnight"],
  },
  diagnosticLegend: {
    "ownership-level": "Exact pixels remain natural; procedural levels use a spacing palette from red (spacing one) through successively cooler rings.",
    topology: "Smooth is blue, voxel tops green, ordinary risers amber, frontier curtains magenta, and proxy vegetation pink.",
    albedo: "Material, biome tint, active-pack texture, and water color before environmental and geometric multipliers.",
    "environmental-illumination": "The environmental multiplier actually consumed by each procedural branch; identity is white.",
    "geometric-shade": "Face or slope geometric multiplier in grayscale; proxy height shade uses the same convention.",
    "local-occlusion": "Procedural local-AO multiplier; currently identity-white for terrain and vegetation.",
    water: "Land is charcoal, sampled water is depth-coded blue, analytic rivers cyan, and pools magenta.",
    texture: "Red is texture strength, green is logarithmic material footprint, and blue is the near-material transition weight.",
  },
  captures: results,
};

const receiptPath = join(options.output, "receipt.json");
writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
process.stdout.write(
  `Wrote ${results.length} verified captures and ${receiptPath}\n`,
);
