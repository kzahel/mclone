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
const readinessAttemptLimit = 3;

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

function positiveInteger(flag, value, maximum) {
  const parsed = Number.parseInt(value ?? "", 10);
  if (!Number.isInteger(parsed) || parsed < 1 || parsed > maximum) {
    fail(`${flag} must be an integer between 1 and ${maximum}`);
  }
  return parsed;
}

function parseArguments(argv) {
  const options = {
    output: "/tmp/mclone-direct-terrain-handoff-review",
    width: 1280,
    height: 720,
    settleFrames: 180,
    skipBuild: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--") {
      continue;
    } else if (argument === "--output") {
      options.output = argv[++index] ?? fail("--output requires a directory");
    } else if (argument === "--width") {
      options.width = positiveInteger("--width", argv[++index], 8192);
    } else if (argument === "--height") {
      options.height = positiveInteger("--height", argv[++index], 8192);
    } else if (argument === "--settle-frames") {
      options.settleFrames = positiveInteger(
        "--settle-frames",
        argv[++index],
        600,
      );
    } else if (argument === "--skip-build") {
      options.skipBuild = true;
    } else if (argument === "--help") {
      process.stdout.write(
        "Usage: node scripts/capture-direct-terrain-handoff-review.mjs "
        + "[--output /tmp/mclone-direct-terrain-handoff-review] "
        + "[--width 1280] [--height 720] [--settle-frames 180] "
        + "[--skip-build]\n",
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
    env: options.env ?? process.env,
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

const views = {
  low: {
    label: "walking-height exact/procedural frontier and open water",
    seed: 12345,
    chunk: [0, 0],
    eye: [8, 82, 8],
    target: [8, 67, -180],
  },
  overhead: {
    label: "near-vertical exact footprint and connector inspection",
    seed: 12345,
    chunk: [0, 0],
    eye: [8, 180, 8],
    target: [20, 60, 8],
  },
  snowSteep: {
    label: "steep snow frontier at a grazing angle",
    seed: 12345,
    chunk: [-130, -69],
    eye: [-2072, 165, -1096],
    target: [-2072, 100, -900],
  },
  lowSubcell: {
    label: "walking view translated by less than one block",
    seed: 12345,
    chunk: [0, 0],
    eye: [8.375, 82, 8.375],
    target: [8.375, 67, -179.625],
  },
  lowRebase: {
    label: "walking view translated across a spacing-one tile",
    seed: 12345,
    chunk: [4, 0],
    eye: [72, 82, 8],
    target: [72, 67, -180],
  },
  lowOrbit: {
    label: "frontier viewed from a second orbit angle",
    seed: 12345,
    chunk: [-3, 0],
    eye: [-36, 92, 6],
    target: [8, 67, -180],
  },
};

const diagnostics = [
  ["natural", 6000],
  ["ownership-level", 6000],
  ["topology", 6000],
  ["albedo", 6000],
  ["environmental-illumination", 18000],
  ["geometric-shade", 6000],
  ["water", 6000],
  ["texture", 6000],
];

function definition(name, group, view, topology, diagnostic = "natural", time = 6000,
  shape = null, expectedExact = 25) {
  return { name, group, view, topology, diagnostic, time, shape, expectedExact };
}

function campaign() {
  const captures = [];
  for (const [diagnostic, time] of diagnostics) {
    for (const topology of ["voxel-shell", "direct-smooth"]) {
      captures.push(definition(
        `${topology}-low-${diagnostic}`,
        `ab-low-${diagnostic}`,
        "low",
        topology,
        diagnostic,
        time,
      ));
    }
  }
  for (const diagnostic of ["natural", "topology"]) {
    for (const topology of ["voxel-shell", "direct-smooth"]) {
      captures.push(definition(
        `${topology}-steep-snow-${diagnostic}`,
        `ab-steep-snow-${diagnostic}`,
        "snowSteep",
        topology,
        diagnostic,
      ));
    }
  }
  for (const [shape, expectedExact] of [
    ["l-shape", 21],
    ["hole", 24],
  ]) {
    for (const diagnostic of ["natural", "topology"]) {
      captures.push(definition(
        `direct-${shape}-overhead-${diagnostic}`,
        `irregular-${shape}-${diagnostic}`,
        "overhead",
        "direct-smooth",
        diagnostic,
        6000,
        shape,
        expectedExact,
      ));
    }
  }
  captures.push(definition(
    "direct-disconnected-island-overhead-topology",
    "irregular-disconnected-island-topology",
    "overhead",
    "direct-smooth",
    "topology",
    6000,
    "island",
    9,
  ));
  for (const view of ["lowSubcell", "lowRebase", "lowOrbit"]) {
    captures.push(definition(
      `direct-stability-${view}`,
      `stability-${view}`,
      view,
      "direct-smooth",
    ));
  }
  return captures;
}

function parseState(output, name) {
  const prefix = "MCLONE_TERRAIN_SEAM_STATE ";
  const line = output.split(/\r?\n/u).find((candidate) => candidate.startsWith(prefix));
  if (!line) {
    fail(`${name} did not report terrain handoff readiness state`);
  }
  try {
    return JSON.parse(line.slice(prefix.length));
  } catch (error) {
    fail(`${name} reported invalid terrain handoff state: ${error.message}`);
  }
}

function stateProblem(capture, state) {
  if (!state.enabled || !state.targetReady || !state.exactCenterReady) {
    return `${capture.name} did not reach exact/procedural target readiness`;
  }
  if (state.exactColumnCount !== capture.expectedExact) {
    return `${capture.name} admitted ${state.exactColumnCount} exact chunks; `
      + `expected ${capture.expectedExact}`;
  }
  if (state.exactHandoffTopology !== capture.topology) {
    return `${capture.name} rendered ${state.exactHandoffTopology}; `
      + `expected ${capture.topology}`;
  }
  if (state.vertexCount <= 0 || state.fixedResidentBytes <= 0 || state.residentBytes <= 0) {
    return `${capture.name} did not report terrain vertex and memory diagnostics`;
  }
  if (state.exactTransitionPreparationMicros <= 0
      || state.exactTransitionPayloadBytes <= 0
      || state.exactTransitionPayloadBytes > 80 * 1024) {
    return `${capture.name} did not report a bounded prepared transition field`;
  }
  if (capture.topology === "direct-smooth"
      && (state.exactConnectorSegments <= 0
        || state.exactConnectorVertexCount !== state.exactConnectorSegments * 6
        || state.exactConnectorBytes !== state.exactConnectorSegments * 12)) {
    return `${capture.name} did not report a coherent direct connector perimeter`;
  }
  if (capture.topology === "voxel-shell"
      && (state.exactConnectorSegments !== 0
        || state.exactConnectorVertexCount !== 0)) {
    return `${capture.name} rendered a direct connector in voxel review mode`;
  }
  if (state.pendingVegetationTiles !== 0
      || state.vegetationSubmittedJobs !== state.vegetationCompletedJobs
      || state.vegetationTransportFailures !== 0
      || state.vegetationJobFailures !== 0) {
    return `${capture.name} did not reach drained, failure-free vegetation state`;
  }
  return null;
}

function comparisonState(state) {
  const {
    coverageGeneration: _coverageGeneration,
    exactHandoffTopology: _exactHandoffTopology,
    lastFrameRevision: _lastFrameRevision,
    vertexCount: _vertexCount,
    residentBytes: _residentBytes,
    exactConnectorSegments: _exactConnectorSegments,
    exactConnectorVertexCount: _exactConnectorVertexCount,
    exactConnectorBytes: _exactConnectorBytes,
    exactTransitionPreparationMicros: _exactTransitionPreparationMicros,
    vegetationSubmittedJobs: _vegetationSubmittedJobs,
    vegetationCompletedJobs: _vegetationCompletedJobs,
    ...stable
  } = state;
  return stable;
}

function validatePairs(results) {
  const signatures = new Map();
  for (const capture of results.filter((entry) => entry.group.startsWith("ab-"))) {
    const signature = JSON.stringify(capture.comparisonState);
    const prior = signatures.get(capture.group);
    if (prior !== undefined && prior !== signature) {
      fail(`${capture.group} changed source, coverage, or settled draw state`);
    }
    signatures.set(capture.group, signature);
  }
}

function pngFacts(path, width, height) {
  const bytes = readFileSync(path);
  if (bytes.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a") {
    fail(`${path} is not a PNG`);
  }
  if (bytes.readUInt32BE(16) !== width || bytes.readUInt32BE(20) !== height) {
    fail(`${path} has an unexpected extent`);
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
const definitions = campaign();
const results = [];

for (let index = 0; index < definitions.length; index += 1) {
  const capture = definitions[index];
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
    "--terrain-presentation", "composed",
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
    "--screenshot-eye", view.eye.join(","),
    "--screenshot-target", view.target.join(","),
    "--screenshot-terrain-horizon-diagnostic", capture.diagnostic,
  ];
  const environment = { ...process.env };
  delete environment.MCLONE_TERRAIN_HANDOFF_REVIEW;
  delete environment.MCLONE_TERRAIN_EXACT_REVIEW_SHAPE;
  if (capture.topology === "voxel-shell") {
    environment.MCLONE_TERRAIN_HANDOFF_REVIEW = "voxel";
  }
  if (capture.shape !== null) {
    environment.MCLONE_TERRAIN_EXACT_REVIEW_SHAPE = capture.shape;
  }

  let state;
  let acceptedAttempt = 0;
  for (let attempt = 1; attempt <= readinessAttemptLimit; attempt += 1) {
    process.stdout.write(
      `[${index + 1}/${definitions.length}] ${capture.name} `
      + `(attempt ${attempt}/${readinessAttemptLimit})\n`,
    );
    const output = run(clientPath, args, {
      capture: true,
      echo: true,
      env: environment,
    });
    const candidate = parseState(output, capture.name);
    const problem = stateProblem(capture, candidate);
    if (problem === null) {
      state = candidate;
      acceptedAttempt = attempt;
      break;
    }
    if (attempt === readinessAttemptLimit) {
      fail(`${problem} after ${readinessAttemptLimit} attempts`);
    }
    process.stderr.write(`Rejected incomplete capture: ${problem}; retrying\n`);
  }
  results.push({
    ...capture,
    acceptedAttempt,
    file: `${capture.name}.png`,
    camera: {
      label: view.label,
      chunk: view.chunk,
      eye: view.eye,
      target: view.target,
    },
    observedState: state,
    comparisonState: comparisonState(state),
    png: pngFacts(path, options.width, options.height),
  });
}

validatePairs(results);
const voxel = results.find((entry) => entry.name === "voxel-shell-low-natural");
const direct = results.find((entry) => entry.name === "direct-smooth-low-natural");
if (voxel === undefined || direct === undefined) {
  fail("review campaign did not produce its primary A/B pair");
}
const savedVertices = voxel.observedState.vertexCount - direct.observedState.vertexCount;
if (savedVertices <= 0) {
  fail(`direct smooth submitted ${savedVertices} fewer vertices; expected a positive saving`);
}

const receipt = {
  schema: "mclone-direct-terrain-handoff-review-v1",
  tactical: 313,
  revision,
  generatedAt: new Date().toISOString(),
  outputDirectory: options.output,
  captureCount: results.length,
  reviewGate: "Human Review 1",
  comparisonContract: {
    fixed: [
      "generation profile and seed within each scene",
      "chunk interest, camera, output extent, and render distance",
      "asset pack, color profile, lighting, and frozen world time",
      "target-ready exact/procedural coverage and drained vegetation",
    ],
    varied: [
      "temporary capture-only voxel-shell or direct-smooth topology",
      "named terrain diagnostic",
      "capture-only connected irregular exact footprint",
      "camera endpoint for motion/rebase evidence",
    ],
    appearanceBandBlocks: 32,
    geometryBlend: "binary ownership; no alpha overlap",
  },
  primaryVertexComparison: {
    scene: "low natural matched pair",
    voxelShell: voxel.observedState.vertexCount,
    directSmooth: direct.observedState.vertexCount,
    savedVertices,
    reductionPercent: savedVertices / voxel.observedState.vertexCount * 100,
    fixedResidentBytesVoxel: voxel.observedState.fixedResidentBytes,
    fixedResidentBytesDirect: direct.observedState.fixedResidentBytes,
    directConnectorSegments: direct.observedState.exactConnectorSegments,
    directConnectorVertices: direct.observedState.exactConnectorVertexCount,
    directConnectorBytes: direct.observedState.exactConnectorBytes,
    worstCaseSpacingOneModel: {
      voxelShellVertices: 5505024,
      directSmoothVertices: 3932160,
      savedVertices: 1572864,
    },
  },
  irregularCoverage: {
    lShapeAdmittedChunks: 21,
    singleChunkHoleAdmittedChunks: 24,
    disconnectedIslandRawReadyChunks: 10,
    disconnectedIslandAdmittedChunks: 9,
    connectorDiagnostic: "cyan",
  },
  motionEndpoints: [
    "direct-stability-lowSubcell",
    "direct-stability-lowRebase",
    "direct-stability-lowOrbit",
  ],
  diagnosticLegend: {
    topology: "Smooth procedural terrain is blue; the direct exact connector is cyan; proxy vegetation is pink; exact terrain remains naturally shaded.",
    texture: "Red is texture strength, green is logarithmic material footprint, and blue is exact-proximity presentation weight.",
  },
  captures: results,
};

const receiptPath = join(options.output, "receipt.json");
writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
process.stdout.write(`Wrote ${results.length} captures and ${receiptPath}\n`);
