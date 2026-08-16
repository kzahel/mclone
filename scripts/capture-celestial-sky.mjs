#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const repo = resolve(import.meta.dirname, "..");
const revision = run("git", ["rev-parse", "--short=12", "HEAD"], repo).trim();
const output = resolve(
  valueAfter("--output") ?? `/tmp/mclone-celestial-${revision}-${Date.now()}`,
);
const executable = resolve(repo, "native/target/debug/mclone-native-client");
mkdirSync(output, { recursive: true });

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

const OFF = {
  sun: false,
  halo: false,
  glow: false,
  moon: false,
  moonlight: false,
  stars: "off",
};
const matrix = [
  ["sky-only-a", OFF],
  ["sun", { ...OFF, sun: true }],
  ["sun-halo", { ...OFF, sun: true, halo: true }],
  ["horizon-glow", { ...OFF, sun: true, halo: true, glow: true }],
  ["moon", { ...OFF, sun: true, halo: true, glow: true, moon: true }],
  ["moonlight", { ...OFF, sun: true, halo: true, glow: true, moon: true, moonlight: true }],
  ["stars-quarter", { sun: true, halo: true, glow: true, moon: true, moonlight: true, stars: "quarter" }],
  ["stars-half", { sun: true, halo: true, glow: true, moon: true, moonlight: true, stars: "half" }],
  ["stars-full", { sun: true, halo: true, glow: true, moon: true, moonlight: true, stars: "full" }],
  ["sky-only-restored", OFF],
];
const captures = matrix.map(([id, settings]) => capture(id, settings));
captures.push(captureCenteredBody("square-sun", "sun", 0, 12));
for (const [index, label] of [
  "new",
  "new-wax-crescent-mid",
  "wax-crescent",
  "crescent-first-quarter-mid",
  "first-quarter",
  "quarter-wax-gibbous-mid",
  "wax-gibbous",
  "gibbous-full-mid",
  "full",
  "full-wane-gibbous-mid",
  "wane-gibbous",
  "gibbous-last-quarter-mid",
  "last-quarter",
  "quarter-wane-crescent-mid",
  "wane-crescent",
  "crescent-new-mid",
].entries()) {
  captures.push(captureCenteredBody(`phase-${String(index).padStart(2, "0")}-${label}`, "moon", index / 16, (12 + index * 1.5) % 24));
}
captures.push(captureMenu());
captures.push(captureStereo("stereo-full", matrix[8][1]));
captures.push(captureStereo("stereo-off", OFF));
captures.push(captureReference());

const baseline = captures.find((entry) => entry.id === "sky-only-a");
const restored = captures.find((entry) => entry.id === "sky-only-restored");
if (baseline.sha256 !== restored.sha256) {
  throw new Error("all-off baseline did not restore exact sky pixels");
}
for (const entry of [baseline, restored]) {
  const cost = entry.celestial.evaluated?.cost;
  if (!cost || cost.optionalDraws !== 0 || cost.featureBufferWrites !== 0) {
    throw new Error(`${entry.id} submitted optional celestial work`);
  }
}
for (const [id, expected] of [
  ["stars-quarter", 384],
  ["stars-half", 768],
  ["stars-full", 1536],
]) {
  const actual = captures.find((entry) => entry.id === id)?.celestial.evaluated?.stars
    ?.submittedCount;
  if (actual !== expected) throw new Error(`${id} submitted ${actual}, expected ${expected}`);
}

const receipt = {
  schemaVersion: 1,
  revision,
  generatedAt: new Date().toISOString(),
  output,
  fixedScene: {
    profile: "mclone-overworld-v1",
    seed: 12345,
    topology: "infinite-plane",
    resolution: [800, 500],
    camera: { eye: [0, 112, 0], target: [0, 212, 0] },
    dayTime: 13000,
    orbitalPhase: 0,
    latitudeDegrees: 45,
    solarTimeHours: 18.5,
    lunarPhase: 0.5,
  },
  exactAllOffRestoration: true,
  captures,
};
writeFileSync(resolve(output, "receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);
process.stdout.write(`${output}\n`);

function capture(id, settings) {
  const path = resolve(output, `${id}.png`);
  const args = ["--screenshot", path, ...commonArgs(), ...celestialArgs(settings)];
  return fileReceipt(id, "mono", path, args, run(executable, args, repo));
}

function captureCenteredBody(id, body, phase, solarTimeHours) {
  const probePath = resolve(output, `.${id}-probe.png`);
  const settings = {
    sun: true,
    halo: body === "sun",
    glow: false,
    moon: body === "moon",
    moonlight: false,
    stars: "off",
  };
  const probeArgs = [
    "--screenshot",
    probePath,
    "--width",
    "320",
    "--height",
    "200",
    ...commonSceneArgs(String(solarTimeHours)),
    ...celestialArgs(settings, phase),
  ];
  const probeStdout = run(executable, probeArgs, repo);
  const probe = parseCelestial(id, probeStdout);
  unlinkSync(probePath);
  const direction = body === "sun"
    ? probe.evaluated.solar.direction
    : probe.evaluated.lunar.direction;
  const eye = [0, 112, 0];
  const target = eye.map((value, index) => value + direction[index] * 100);
  const path = resolve(output, `${id}.png`);
  const args = [
    "--screenshot",
    path,
    "--width",
    "960",
    "--height",
    "540",
    ...commonSceneArgs(String(solarTimeHours), target.join(",")),
    ...celestialArgs(settings, phase),
  ];
  return fileReceipt(id, `mono-${body}-centered`, path, args, run(executable, args, repo));
}

function captureStereo(id, settings) {
  const path = resolve(output, `${id}.png`);
  const args = [
    "--xr-emulation-screenshot",
    path,
    "--width",
    "480",
    "--height",
    "480",
    ...commonSceneArgs(),
    ...celestialArgs(settings),
    "--xr-emulation-pause-panel",
    "false",
  ];
  return fileReceipt(id, "synthetic-stereo", path, args, run(executable, args, repo));
}

function captureMenu() {
  const path = resolve(output, "celestial-debug-menu.png");
  const settings = matrix[8][1];
  const args = [
    "--screenshot",
    path,
    "--width",
    "960",
    "--height",
    "540",
    ...commonSceneArgs(),
    ...celestialArgs(settings),
    "--screenshot-eye",
    "0,112,0",
    "--screenshot-target",
    "0,80,-52",
    "--screenshot-ui",
    "options-celestial-debug",
  ];
  return fileReceipt("celestial-debug-menu", "mono-ui", path, args, run(executable, args, repo));
}

function captureReference() {
  const path = resolve(output, "retained-java-night.png");
  const args = [
    "--screenshot",
    path,
    "--width",
    "800",
    "--height",
    "500",
    "--transient",
    "--asset-pack",
    "saved",
    "--seed",
    "12345",
    "--generation-profile",
    "overworld",
    "--render-distance",
    "3",
    "--day-time",
    "18000",
    "--freeze-time",
    "--debug-passive-showcase",
    "false",
    "--terrain-presentation",
    "exact-only",
    "--screenshot-eye",
    "0,112,0",
    "--screenshot-target",
    "0,212,0",
  ];
  return fileReceipt("retained-java-night", "mono-reference", path, args, run(executable, args, repo));
}

function commonArgs() {
  return ["--width", "800", "--height", "500", ...commonSceneArgs()];
}

function commonSceneArgs(solarTimeHours = "18.5", target = "0,212,0") {
  return [
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
    "13000",
    "--freeze-time",
    "--debug-passive-showcase",
    "false",
    "--terrain-presentation",
    "exact-only",
    "--season-preview",
    "true",
    "--season-appearance",
    "false",
    "--season-orbital-phase",
    "0",
    "--season-latitude-source",
    "manual",
    "--season-latitude",
    "45",
    "--season-solar-time-source",
    "manual",
    "--season-solar-time",
    solarTimeHours,
    "--screenshot-eye",
    "0,112,0",
    "--screenshot-target",
    target,
  ];
}

function celestialArgs(settings, phase = 0.5) {
  return [
    "--celestial-sun",
    String(settings.sun),
    "--celestial-sun-halo",
    String(settings.halo),
    "--celestial-horizon-glow",
    String(settings.glow),
    "--celestial-moon",
    String(settings.moon),
    "--celestial-moonlight",
    String(settings.moonlight),
    "--celestial-stars",
    settings.stars,
    "--moon-phase-source",
    "manual-preview",
    "--moon-phase",
    String(phase),
  ];
}

function fileReceipt(id, renderPath, path, args, stdout) {
  const celestial = parseCelestial(id, stdout);
  const seasonalLine = stdout
    .split(/\r?\n/)
    .find((line) => line.startsWith("MCLONE_SEASONAL_APPEARANCE_STATE "));
  const summary = stdout.match(/\((?:\d+x\d+[^)]*?, )?(\d+) sections, (\d+) drawn sections, (\d+) GUI commands/);
  return {
    id,
    renderPath,
    path,
    sha256: createHash("sha256").update(readFileSync(path)).digest("hex"),
    bytes: statSync(path).size,
    args,
    frame: summary
      ? { sections: Number(summary[1]), drawnSections: Number(summary[2]), guiCommands: Number(summary[3]) }
      : null,
    seasonal: seasonalLine ? JSON.parse(seasonalLine.slice("MCLONE_SEASONAL_APPEARANCE_STATE ".length)) : null,
    celestial,
  };
}

function parseCelestial(id, stdout) {
  const line = stdout
    .split(/\r?\n/)
    .find((entry) => entry.startsWith("MCLONE_CELESTIAL_STATE "));
  if (!line) throw new Error(`${id} omitted MCLONE_CELESTIAL_STATE`);
  return JSON.parse(line.slice("MCLONE_CELESTIAL_STATE ".length));
}

function valueAfter(flag) {
  const index = process.argv.indexOf(flag);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function run(command, args, cwd) {
  return execFileSync(command, args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
}
