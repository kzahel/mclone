#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

const repo = resolve(import.meta.dirname, "..");
const revision = run("git", ["rev-parse", "--short=12", "HEAD"], repo).trim();
const output = resolve(
  valueAfter("--output") ?? `/tmp/mclone-seasonal-appearance-${revision}-${Date.now()}`,
);
const quick = process.argv.includes("--quick");
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

const fixtures = {
  temperate: fixture("temperate", 1032, 101, 1032),
  warmDry: fixture("warm-dry", 8, 80, 8),
  coolWet: fixture("cool-wet", -632, 100, 136),
  coldHigh: fixture("cold-high", -1720, 153, -776),
};
const cases = buildCases();
const selected = quick
  ? cases.filter((entry) =>
      [
        "temperate-off",
        "north-spring",
        "north-summer",
        "north-autumn",
        "north-winter",
        "recent-snow-full",
        "recent-snow-stereo",
        "seasonal-debug-menu",
      ].includes(entry.id),
    )
  : cases;
const captures = selected.map(capture);

const off = captures.find((entry) => entry.id === "temperate-off");
const configuredOff = captures.find((entry) => entry.id === "temperate-off-configured");
if (configuredOff && off.sha256 !== configuredOff.sha256) {
  throw new Error("preview-disabled terrain pixels changed under configured seasonal inputs");
}
// Summer deliberately preserves the present baseline family; the other
// quarter-year landmarks must produce visible exact-terrain responses.
for (const id of ["north-spring", "north-autumn", "north-winter"]) {
  const active = captures.find((entry) => entry.id === id);
  if (active && active.sha256 === off.sha256) {
    throw new Error(`${id} did not change the exact-terrain frame`);
  }
}
const recentFull = captures.find((entry) => entry.id === "recent-snow-full");
const lateWinter = captures.find((entry) => entry.id === "north-late-winter");
if (recentFull && lateWinter && recentFull.sha256 === lateWinter.sha256) {
  throw new Error("full recent snow did not change the late-winter frame");
}

const contactSheet = resolve(output, "seasonal-appearance-contact-sheet.png");
const montageFont = run("fc-match", ["-f", "%{file}", "sans"], repo).trim();
run(
  "magick",
  [
    "montage",
    "-font",
    montageFont,
    ...captures.map((entry) => entry.path),
    "-thumbnail",
    "320x200",
    "-tile",
    "4x",
    "-geometry",
    "+6+6",
    contactSheet,
  ],
  repo,
);

const receipt = {
  schemaVersion: 1,
  revision,
  generatedAt: new Date().toISOString(),
  output,
  quick,
  fixedWorld: {
    profile: "mclone-overworld-v1",
    seed: 12345,
    topology: "plane",
    terrainPresentation: "exact-only",
    dayTime: 6000,
    authoritativeWeather: "default-clear",
    lighting: true,
    renderDistance: 3,
  },
  contactSheet,
  captures,
};
writeFileSync(resolve(output, "capture-receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);
process.stdout.write(`${output}\n`);

function buildCases() {
  const entries = [
    captureCase("temperate-off", fixtures.temperate, { enabled: false }),
    captureCase("temperate-off-configured", fixtures.temperate, {
      enabled: false,
      phase: 0.75,
      latitude: 47.5,
      snow: 1,
      snowCenter: [1032, 1032],
    }),
  ];
  for (const [id, phase] of [
    ["north-spring", 0],
    ["north-spring-summer", 0.125],
    ["north-summer", 0.25],
    ["north-summer-autumn", 0.375],
    ["north-autumn", 0.5],
    ["north-autumn-winter", 0.625],
    ["north-winter", 0.75],
    ["north-late-winter", 0.875],
  ]) {
    entries.push(captureCase(id, fixtures.temperate, { enabled: true, phase, latitude: 47.5 }));
  }
  entries.push(
    captureCase("south-at-north-summer", fixtures.temperate, {
      enabled: true,
      phase: 0.25,
      latitude: -47.5,
    }),
    captureCase("south-at-north-winter", fixtures.temperate, {
      enabled: true,
      phase: 0.75,
      latitude: -47.5,
    }),
    captureCase("equatorial-weak-cycle", fixtures.temperate, {
      enabled: true,
      phase: 0.875,
      latitude: 0,
    }),
    captureCase("warm-dry-late-winter", fixtures.warmDry, {
      enabled: true,
      phase: 0.875,
      latitude: 47.5,
      snow: 1,
      snowCenter: [8, 8],
    }),
    captureCase("cool-wet-late-winter", fixtures.coolWet, {
      enabled: true,
      phase: 0.875,
      latitude: 47.5,
      snow: 1,
      snowCenter: [-632, 136],
    }),
    captureCase("cold-high-late-winter", fixtures.coldHigh, {
      enabled: true,
      phase: 0.875,
      latitude: 47.5,
      snow: 1,
      snowCenter: [-1720, -776],
    }),
    captureCase("recent-snow-half", fixtures.temperate, {
      enabled: true,
      phase: 0.875,
      latitude: 47.5,
      snow: 0.5,
      snowCenter: [1032, 1032],
    }),
    captureCase("recent-snow-full", fixtures.temperate, {
      enabled: true,
      phase: 0.875,
      latitude: 47.5,
      snow: 1,
      snowCenter: [1032, 1032],
    }),
    captureCase("recent-snow-falloff", fixture("falloff", 1098, 101, 1032), {
      enabled: true,
      phase: 0.875,
      latitude: 47.5,
      snow: 1,
      snowCenter: [1032, 1032],
    }),
    captureCase("recent-snow-outside", fixture("outside", 1140, 101, 1032), {
      enabled: true,
      phase: 0.875,
      latitude: 47.5,
      snow: 1,
      snowCenter: [1032, 1032],
    }),
    captureCase("recent-snow-stereo", fixtures.temperate, {
      enabled: true,
      phase: 0.875,
      latitude: 47.5,
      snow: 1,
      snowCenter: [1032, 1032],
      stereo: true,
    }),
    captureCase("seasonal-debug-menu", fixtures.temperate, {
      enabled: true,
      phase: 0.875,
      latitude: 47.5,
      snow: 0.5,
      snowCenter: [1032, 1032],
      menu: true,
    }),
  );
  return entries;
}

function captureCase(id, location, options) {
  return { id, location, ...options };
}

function fixture(id, x, y, z) {
  return {
    id,
    eye: [x, y, z],
    target: [x, y - 13, z - 32],
    chunk: [Math.floor(x / 16), Math.floor(z / 16)],
  };
}

function capture(entry) {
  const stereo = entry.stereo === true;
  const path = resolve(output, `${entry.id}.png`);
  const args = [
    stereo ? "--xr-emulation-screenshot" : "--screenshot",
    path,
    "--width",
    stereo ? "480" : entry.menu ? "960" : "800",
    "--height",
    stereo ? "480" : entry.menu ? "540" : "500",
    "--transient",
    "--seed",
    "12345",
    "--generation-profile",
    "mclone-overworld-v1",
    "--chunk-x",
    String(entry.location.chunk[0]),
    "--chunk-z",
    String(entry.location.chunk[1]),
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
    csv(entry.location.eye),
    "--screenshot-target",
    csv(entry.location.target),
  ];
  if (stereo) {
    args.push("--xr-emulation-pause-panel", "false");
  } else {
    args.push("--startup-wait", "view-settled");
  }
  if (entry.menu) args.push("--screenshot-ui", "options-seasonal-debug");
  if (entry.id !== "temperate-off") {
    args.push(
      "--season-preview",
      String(entry.enabled),
      "--season-orbital-phase",
      String(entry.phase ?? 0),
      "--season-latitude-source",
      "manual",
      "--season-latitude",
      String(entry.latitude ?? 0),
      "--season-solar-time-source",
      "manual",
      "--season-solar-time",
      "12",
    );
    if (entry.snow !== undefined) {
      args.push(
        "--season-recent-snow",
        String(entry.snow),
        "--season-recent-snow-center",
        entry.snowCenter.join(","),
      );
    }
  }
  const stdout = run(executable, args, repo);
  const seasonal = parseReceipt(stdout);
  if (seasonal.enabled !== entry.enabled) {
    throw new Error(`${entry.id} receipt disagrees with requested preview state`);
  }
  if (entry.enabled && (!seasonal.evaluated || seasonal.renderScope.seasonalLodDeferred !== true)) {
    throw new Error(`${entry.id} did not publish evaluated exact-only seasonal diagnostics`);
  }
  if (entry.snow > 0) {
    const expectedRaw = Math.round(entry.snow * 65535);
    if (
      seasonal.recentSnow?.intensityRaw !== expectedRaw ||
      seasonal.recentSnow?.radiusBlocks !== 96 ||
      seasonal.recentSnow?.centerX !== entry.snowCenter[0] ||
      seasonal.recentSnow?.centerZ !== entry.snowCenter[1]
    ) {
      throw new Error(`${entry.id} receipt lost exact recent-snow state`);
    }
  }
  const bytes = readFileSync(path);
  return {
    id: entry.id,
    fixture: entry.location,
    path,
    file: basename(path),
    renderPath: stereo ? "synthetic-stereo" : "mono",
    byteLength: statSync(path).size,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    seasonal,
    argv: args,
    stdout: stdout.trim(),
  };
}

function parseReceipt(stdout) {
  const prefix = "MCLONE_SEASONAL_APPEARANCE_STATE ";
  const line = stdout.split(/\r?\n/).find((entry) => entry.startsWith(prefix));
  if (!line) throw new Error("capture did not emit a seasonal appearance receipt");
  return JSON.parse(line.slice(prefix.length));
}

function csv(values) {
  return values.join(",");
}

function valueAfter(flag) {
  const index = process.argv.indexOf(flag);
  if (index < 0) return undefined;
  if (!process.argv[index + 1]) throw new Error(`${flag} requires a value`);
  return process.argv[index + 1];
}

function run(command, args, cwd) {
  process.stderr.write(`+ ${command} ${args.join(" ")}\n`);
  return execFileSync(command, args, {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });
}
