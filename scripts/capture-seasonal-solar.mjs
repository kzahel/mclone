#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const repo = resolve(import.meta.dirname, "..");
const revision = run("git", ["rev-parse", "--short=12", "HEAD"], repo).trim();
const requestedOutput = valueAfter("--output");
const output = resolve(
  requestedOutput ?? `/tmp/mclone-seasonal-solar-${revision}-${Date.now()}`,
);
const quick = process.argv.includes("--quick");
mkdirSync(output, { recursive: true });

run(
  "cargo",
  [
    "run",
    "--manifest-path",
    "native/Cargo.toml",
    "-p",
    "mclone-season-lab",
    "--",
    "--output",
    output,
    "--seed",
    "12345",
  ],
  repo,
);
run(
  "cargo",
  [
    "build",
    "--manifest-path",
    "native/Cargo.toml",
    "-p",
    "mclone-native-client",
    "--bin",
    "mclone-native-client",
  ],
  repo,
);

const review = JSON.parse(readFileSync(resolve(output, "solar-review.json"), "utf8"));
const executable = resolve(repo, "native/target/debug/mclone-native-client");
const selected = quick
  ? review.solarCases.filter((entry) =>
      ["north45-north-solstice-09", "north75-polar-night-noon"].includes(entry.id),
    )
  : review.solarCases;
const captures = [];

for (const sample of selected) {
  captures.push(capture(sample, "sun", false));
  captures.push(capture(sample, "terrain", false));
}

if (!quick) {
  for (const id of ["north45-north-solstice-09", "south45-south-solstice-15"]) {
    captures.push(capture(review.solarCases.find((entry) => entry.id === id), "terrain", true));
  }
  captures.push(captureDebugMenu(review.solarCases[0]));
  captures.push(...captureNoOpPairs(review.solarCases[0]));
  captures.push(...captureCylinderPair(review.cylinderCases));
}

const receipt = {
  schemaVersion: 1,
  revision,
  generatedAt: new Date().toISOString(),
  output,
  quick,
  sourceReview: "solar-review.json",
  reviewArtifacts: {
    latitudeMaps: review.candidates.map((entry) => entry.image),
    tiltComparison: review.tiltComparisonImage,
    cylinderMap: review.cylinderMapImage,
  },
  captures,
};
writeFileSync(resolve(output, "capture-receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);
process.stdout.write(`${output}\n`);

function capture(sample, view, stereo) {
  if (!sample) throw new Error("missing requested seasonal solar sample");
  const latitudeSource = view === "sun" ? "world" : "manual";
  const eye = latitudeSource === "world" ? [sample.worldX, 112, sample.worldZ] : [0, 112, 0];
  const target =
    view === "sun"
      ? eye.map((value, index) => value + sample.direction[index] * 100)
      : [0, 66, -52];
  const suffix = stereo ? "stereo" : "mono";
  const path = resolve(output, `${sample.id}-${view}-${suffix}.png`);
  const mode = stereo ? "--xr-emulation-screenshot" : "--screenshot";
  const args = [mode, path, "--width", stereo ? "480" : "800", "--height", stereo ? "480" : "500"];
  args.push(...sceneArgs(sample, eye, target, latitudeSource));
  const stdout = run(executable, args, repo);
  return fileReceipt(sample, view, suffix, path, args, stdout);
}

function captureDebugMenu(sample) {
  const eye = [0, 112, 0];
  const target = [0, 66, -52];
  const path = resolve(output, "season-debug-menu.png");
  const args = [
    "--screenshot",
    path,
    "--width",
    "960",
    "--height",
    "540",
    "--screenshot-ui",
    "options-debug",
    ...sceneArgs(sample, eye, target, "manual"),
  ];
  const stdout = run(executable, args, repo);
  return fileReceipt(sample, "debug-menu", "mono", path, args, stdout);
}

function captureCylinderPair(samples) {
  if (!Array.isArray(samples) || samples.length !== 2) {
    throw new Error("season lab must provide exactly two cylinder seam samples");
  }
  const receipts = samples.map((sample) => {
    const eye = [sample.worldX, 112, sample.worldZ];
    const target = eye.map((value, index) => value + sample.direction[index] * 100);
    const path = resolve(output, `${sample.id}-sun-mono.png`);
    const args = [
      "--screenshot",
      path,
      "--width",
      "800",
      "--height",
      "500",
      ...sceneArgs(sample, eye, target, "world"),
      "--world-topology",
      sample.topology,
    ];
    return fileReceipt(sample, "sun", "mono", path, args, run(executable, args, repo));
  });
  if (receipts[0].sha256 !== receipts[1].sha256) {
    throw new Error("secondary cylinder X-seam pixels differ");
  }
  receipts[0].identicalTo = receipts[1].path;
  receipts[1].identicalTo = receipts[0].path;
  return receipts;
}

function captureNoOpPairs(sample) {
  const eye = [0, 112, 0];
  // Keep whole-frame equality focused on solar-owned sky pixels. Natural
  // wildlife advances during separate process startup and is intentionally
  // outside this client-local presentation proof.
  const target = [0, 212, 0];
  const pairs = [];
  for (const profile of ["mclone-overworld-v1", "overworld"]) {
    const paths = [
      resolve(output, `${profile}-fixed-a.png`),
      resolve(output, `${profile}-fixed-b.png`),
    ];
    const common = [
      "--width",
      "800",
      "--height",
      "500",
      "--transient",
      "--asset-pack",
      "original",
      "--seed",
      "12345",
      "--generation-profile",
      profile,
      "--render-distance",
      "3",
      "--day-time",
      "6000",
      "--freeze-time",
      "--debug-passive-showcase",
      "false",
      "--terrain-presentation",
      "exact-only",
      "--screenshot-eye",
      csv(eye),
      "--screenshot-target",
      csv(target),
    ];
    const argsA = ["--screenshot", paths[0], ...common];
    const argsB = [
      "--screenshot",
      paths[1],
      ...common,
      "--season-preview",
      profile === "overworld" ? "true" : "false",
      "--season-orbital-phase",
      "0.75",
      "--season-latitude-source",
      "manual",
      "--season-latitude",
      "75",
      "--season-solar-time-source",
      "manual",
      "--season-solar-time",
      "1",
    ];
    const first = fileReceipt(sample, `${profile}-fixed-a`, "mono", paths[0], argsA, run(executable, argsA, repo));
    const second = fileReceipt(sample, `${profile}-fixed-b`, "mono", paths[1], argsB, run(executable, argsB, repo));
    if (first.sha256 !== second.sha256) {
      throw new Error(`${profile} fixed-path A/B pixels differ`);
    }
    first.identicalTo = second.path;
    second.identicalTo = first.path;
    pairs.push(first, second);
  }
  return pairs;
}

function sceneArgs(sample, eye, target, latitudeSource) {
  const args = [
    "--transient",
    "--asset-pack",
    "original",
    "--seed",
    "12345",
    "--generation-profile",
    "mclone-overworld-v1",
    "--render-distance",
    "3",
    "--day-time",
    "6000",
    "--freeze-time",
    "--debug-passive-showcase",
    "false",
    "--terrain-presentation",
    "exact-only",
    "--screenshot-eye",
    csv(eye),
    "--screenshot-target",
    csv(target),
    "--season-preview",
    "true",
    "--season-orbital-phase",
    String(sample.orbitalPhase),
    "--season-latitude-source",
    latitudeSource,
    "--season-solar-time-source",
    "manual",
    "--season-solar-time",
    String(sample.solarTimeHours),
  ];
  if (latitudeSource === "manual") {
    args.push("--season-latitude", String(sample.latitudeDegrees));
  }
  return args;
}

function fileReceipt(sample, view, renderPath, path, args, stdout) {
  const bytes = readFileSync(path);
  return {
    case: sample.id,
    view,
    renderPath,
    latitudeSource: optionValue(args, "--season-latitude-source"),
    path,
    byteLength: statSync(path).size,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    solarSample: sample,
    argv: args,
    stdout: stdout.trim(),
  };
}

function optionValue(args, flag) {
  const index = args.indexOf(flag);
  return index >= 0 ? args[index + 1] : undefined;
}

function csv(values) {
  return values.map((value) => Number(value).toFixed(6)).join(",");
}

function valueAfter(flag) {
  const index = process.argv.indexOf(flag);
  if (index < 0) return undefined;
  if (!process.argv[index + 1]) throw new Error(`${flag} requires a value`);
  return process.argv[index + 1];
}

function run(command, args, cwd) {
  process.stderr.write(`+ ${command} ${args.join(" ")}\n`);
  return execFileSync(command, args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
}
