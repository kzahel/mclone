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

function parsePositiveInteger(flag, value, maximum) {
  const parsed = Number.parseInt(value ?? "", 10);
  if (!Number.isInteger(parsed) || parsed < 1 || parsed > maximum) {
    fail(`${flag} must be an integer between 1 and ${maximum}`);
  }
  return parsed;
}

function parseArguments(argv) {
  const options = {
    output: "/tmp/mclone-exact-frontier-hybrid-review",
    width: 1024,
    height: 576,
    settleFrames: 240,
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
        1200,
      );
    } else if (argument === "--skip-build") {
      options.skipBuild = true;
    } else if (argument === "--help") {
      process.stdout.write(
        "Usage: node scripts/capture-exact-frontier-review.mjs "
        + "[--output /tmp/mclone-exact-frontier-hybrid-review] "
        + "[--width 1024] [--height 576] [--settle-frames 240] "
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

function parseTerrainState(output, name) {
  const prefix = "MCLONE_TERRAIN_SEAM_STATE ";
  const line = output.split(/\r?\n/u).find((candidate) => candidate.startsWith(prefix));
  if (!line) {
    fail(`${name} did not report terrain frontier state`);
  }
  try {
    return JSON.parse(line.slice(prefix.length));
  } catch (error) {
    fail(`${name} reported invalid terrain frontier JSON: ${error.message}`);
  }
}

function validateState(capture, state) {
  const expectedColumns = (capture.renderDistance * 2 + 1) ** 2;
  const expectedSegments = 64 * (capture.renderDistance * 2 + 1);
  if (!state.enabled || !state.targetReady || !state.exactCenterReady) {
    return "terrain view did not reach target readiness";
  }
  if (state.exactColumnCount !== expectedColumns) {
    return `prepared ${state.exactColumnCount} exact columns; expected ${expectedColumns}`;
  }
  if (state.frontier?.enabled !== true
      || state.frontier?.planFailures !== 0
      || state.frontier?.state === "invalid"
      || state.frontier?.format?.allValid !== true) {
    return `frontier plan is invalid: ${JSON.stringify(state.frontier)}`;
  }
  if (state.frontier.exposedSegments !== expectedSegments) {
    return `classified ${state.frontier.exposedSegments} edges; expected ${expectedSegments}`;
  }
  if (state.frontierTopology?.planFailures !== 0
      || state.frontierTopology?.state !== "complete"
      || state.frontierTopology?.exposedSegments !== expectedSegments
      || state.frontierTopology?.certifiedSegments !== expectedSegments
      || state.frontierTopology?.unresolvedSegments !== 0) {
    return `frontier topology is incomplete: ${JSON.stringify(state.frontierTopology)}`;
  }
  if (capture.renderDistance === 2
      && (state.frontier.maximumAdjacentSpacing !== 1
        || state.frontier.unsupportedSpacingSegments !== 0)) {
    return "RD2 did not retain spacing-one support around every edge";
  }
  if (capture.renderDistance === 8
      && (state.frontier.maximumAdjacentSpacing < 2
        || state.frontier.unsupportedSpacingSegments === 0)) {
    return "RD8 did not expose the expected coarser unsupported edge";
  }
  if (state.pendingVegetationTiles !== 0
      || state.vegetationSubmittedJobs !== state.vegetationCompletedJobs
      || state.vegetationTransportFailures !== 0
      || state.vegetationJobFailures !== 0) {
    return "vegetation did not settle without failures";
  }
  const gpu = state.frontierProofGpu;
  if (capture.diagnostic === "natural") {
    if (gpu?.allocatedSupportTiles !== 0
        || gpu?.readySupportTiles !== 0
        || gpu?.connectorSegments !== 0
        || gpu?.supportResourceBytes !== 0
        || gpu?.connectorBytes !== 0) {
      return `natural path retained proof resources: ${JSON.stringify(gpu)}`;
    }
  } else {
    if (gpu?.allocatedSupportTiles !== state.frontierTopology.selectedSupportTiles
        || gpu?.readySupportTiles !== gpu?.allocatedSupportTiles
        || gpu?.pendingSupportTiles !== 0
        || gpu?.supportResourceBytes !== state.frontierTopology.activeSupportResourceBytes
        || gpu?.connectorSegments === 0
        || gpu?.connectorBytes === 0) {
      return `hybrid proof GPU receipt is incomplete: ${JSON.stringify(gpu)}`;
    }
    if (capture.renderDistance === 2 && gpu.allocatedSupportTiles !== 0) {
      return "RD2 unexpectedly allocated sparse support outside the resident fine ring";
    }
    if (capture.renderDistance === 8
        && capture.diagnostic === "frontier-hybrid-proof"
        && (gpu.allocatedSupportTiles !== 20
          || gpu.supportDispatchesTotal < 20
          || gpu.drawnSupportTiles === 0)) {
      return `RD8 did not commit and draw its 20-tile support belt: ${JSON.stringify(gpu)}`;
    }
    if (capture.diagnostic === "frontier-hybrid-fallback-proof"
        && (gpu.allocatedSupportTiles !== 1
          || gpu.supportDispatchesTotal < 1
          || state.frontierTopology.supportPoolCapacity !== 1
          || state.frontierTopology.rejectedSupportTiles === 0
          || (state.frontierTopology.fallbackSolidSegments
            + state.frontierTopology.fallbackWaterSegments) === 0)) {
      return `forced proof did not exercise the bounded fallback: ${JSON.stringify(state.frontierTopology)}`;
    }
  }
  return null;
}

function stableState(state) {
  const {
    coverageGeneration: _coverageGeneration,
    exactTransitionPreparationMicros: _transitionMicros,
    exactBoundaryPreparationMicros: _boundaryMicros,
    vegetationSubmittedJobs: _submittedJobs,
    vegetationCompletedJobs: _completedJobs,
    residentBytes: _residentBytes,
    vertexCount: _vertexCount,
    exactConnectorSegments: _exactConnectorSegments,
    exactConnectorVertexCount: _exactConnectorVertexCount,
    exactConnectorBytes: _exactConnectorBytes,
    frontierProofGpu: _frontierProofGpu,
    frontier,
    frontierTopology: _frontierTopology,
    ...stable
  } = state;
  return {
    ...stable,
    frontier: {
      ...frontier,
      exactGeneration: 0,
      preparationMicros: 0,
    },
  };
}

function pngFacts(path, width, height) {
  const bytes = readFileSync(path);
  if (bytes.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a") {
    fail(`${path} is not a PNG`);
  }
  if (bytes.readUInt32BE(16) !== width || bytes.readUInt32BE(20) !== height) {
    fail(`${path} has the wrong dimensions`);
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

const views = {
  low: {
    eye: [8, 82, 8],
    target: [8, 67, -180],
  },
  elevated: {
    eye: [8, 105, 8],
    target: [8, 65, -300],
  },
};
const campaign = [
  { name: "rd2-low-natural", renderDistance: 2, view: "low", diagnostic: "natural" },
  { name: "rd2-low-hybrid", renderDistance: 2, view: "low", diagnostic: "frontier-hybrid-proof" },
  { name: "rd8-elevated-natural", renderDistance: 8, view: "elevated", diagnostic: "natural" },
  { name: "rd8-elevated-hybrid", renderDistance: 8, view: "elevated", diagnostic: "frontier-hybrid-proof" },
  { name: "rd8-elevated-fallback", renderDistance: 8, view: "elevated", diagnostic: "frontier-hybrid-fallback-proof" },
];
const results = [];

for (const [index, capture] of campaign.entries()) {
  const view = views[capture.view];
  const path = join(options.output, `${capture.name}.png`);
  const args = [
    "--screenshot", path,
    "--width", String(options.width),
    "--height", String(options.height),
    "--seed", "12345",
    "--generation-profile", "mclone-overworld-v1",
    "--starter-content", "wild",
    "--chunk-x", "0",
    "--chunk-z", "0",
    "--render-distance", String(capture.renderDistance),
    "--terrain-presentation", "composed",
    "--day-time", "6000",
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
  let state;
  let acceptedAttempt = 0;
  for (let attempt = 1; attempt <= readinessAttemptLimit; attempt += 1) {
    process.stdout.write(
      `[${index + 1}/${campaign.length}] ${capture.name} `
      + `(attempt ${attempt}/${readinessAttemptLimit})\n`,
    );
    const output = run(clientPath, args, { capture: true, echo: true });
    const candidate = parseTerrainState(output, capture.name);
    const problem = validateState(capture, candidate);
    if (problem === null) {
      state = candidate;
      acceptedAttempt = attempt;
      break;
    }
    if (attempt === readinessAttemptLimit) {
      fail(`${capture.name}: ${problem}`);
    }
    process.stderr.write(`Rejected incomplete capture: ${problem}; retrying\n`);
  }
  results.push({
    ...capture,
    acceptedAttempt,
    file: `${capture.name}.png`,
    state,
    stableState: stableState(state),
    png: pngFacts(path, options.width, options.height),
    command: [clientPath, ...args],
  });
}

for (const renderDistance of [2, 8]) {
  const pair = results.filter((capture) => capture.renderDistance === renderDistance);
  const natural = pair.find((capture) => capture.diagnostic === "natural");
  if (!natural || pair.length < 2) {
    fail(`RD${renderDistance} is missing its natural comparison`);
  }
  for (const proof of pair.filter((capture) => capture !== natural)) {
    if (JSON.stringify(natural.stableState) !== JSON.stringify(proof.stableState)) {
      fail(`RD${renderDistance} ${proof.diagnostic} changed invariant settled state`);
    }
    if (natural.png.sha256 === proof.png.sha256) {
      fail(`RD${renderDistance} ${proof.diagnostic} did not change pixels`);
    }
  }
}

const receipt = {
  schema: "mclone-exact-frontier-review-v2",
  tactical: 321,
  tacticalPhase: 2,
  revision: run("git", ["rev-parse", "HEAD"], { capture: true }),
  generatedAt: new Date().toISOString(),
  outputDirectory: options.output,
  comparisonContract: {
    invariantAxes: [
      "seed 12345",
      "focus chunk 0,0",
      "noon frozen time",
      "original asset pack",
      "vanilla color profile",
      "view-settled startup",
      `${options.width}x${options.height} output`,
    ],
    variedAxes: ["exact render distance 2 versus 8", "natural, full hybrid, and forced fallback proof"],
    diagnosticLegend: {
      fineSupport: "selected spacing-one procedural tiles outside the exact frontier",
      coarseSuppression: "base clipmap horizontal fragments beneath committed support are discarded",
      exactClosure: "typed solid or water curtain owned by one fine or fallback procedural tile",
      outerClosure: "vertical support skirt closes the handoff back to the base clipmap",
    },
  },
  captures: results,
};
const receiptPath = join(options.output, "receipt.json");
writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
process.stdout.write(`Wrote ${results.length} captures and ${receiptPath}\n`);
