#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = path.resolve(SCRIPT_DIR, "..");
const DEFAULT_CAMERA = "0,96,0,180,20";
const DEFAULT_SEED = "12345";
const DEFAULT_DAY_TIME = "6000";
const DEFAULT_RENDER_DISTANCE = "8";
const DEFAULT_VANILLA_WIDTH = "854";
const DEFAULT_VANILLA_HEIGHT = "480";
const DEFAULT_SETTLE_FRAMES = "40";
const DEFAULT_TIMEOUT_SECONDS = "240";
const DEFAULT_NATIVE_EYE_Y_OFFSET = "1.62";
const DEFAULT_NATIVE_TARGET_DISTANCE = "64";
const DEFAULT_OUTPUT_ROOT = "/tmp";

try {
  main();
} catch (error) {
  console.error(`error: ${error.message}`);
  process.exit(1);
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const camera = parseCamera(options.camera);
  const chunk = {
    x: options.chunkX ?? Math.floor(camera.x / 16),
    z: options.chunkZ ?? Math.floor(camera.z / 16),
  };
  const outDir = path.resolve(options.outDir ?? path.join(DEFAULT_OUTPUT_ROOT, `mclone-visual-compare-seed-${options.seed}`));
  const paths = {
    vanillaPng: path.join(outDir, "vanilla.png"),
    nativePng: path.join(outDir, "native.png"),
    metadata: path.join(outDir, "metadata.json"),
    vanillaLog: path.join(outDir, "vanilla.log"),
    nativeLog: path.join(outDir, "native.log"),
  };
  fs.mkdirSync(outDir, { recursive: true });

  const vanillaCommand = vanillaCaptureCommand(options, paths.vanillaPng);
  console.log(`capturing vanilla screenshot: ${paths.vanillaPng}`);
  const vanillaRun = runCommand("vanilla capture", vanillaCommand, paths.vanillaLog);
  const vanillaDimensions = readPngDimensions(paths.vanillaPng);

  const nativePose = nativePoseFromVanillaCamera(
    camera,
    Number(options.nativeEyeYOffset),
    Number(options.nativeTargetDistance),
  );
  const nativeCommand = nativeCaptureCommand(options, paths.nativePng, vanillaDimensions, nativePose, chunk);
  console.log(`capturing native screenshot: ${paths.nativePng}`);
  const nativeRun = runCommand("native capture", nativeCommand, paths.nativeLog);
  const nativeDimensions = readPngDimensions(paths.nativePng);

  const metadata = {
    generatedAt: new Date().toISOString(),
    seed: options.seed,
    dayTime: options.dayTime,
    renderDistance: Number(options.renderDistance),
    chunk,
    camera: {
      vanillaPlayerPosition: [camera.x, camera.y, camera.z],
      yawDegrees: camera.yaw,
      pitchDegrees: camera.pitch,
      nativeEyeYOffset: Number(options.nativeEyeYOffset),
      nativeEye: nativePose.eye,
      nativeTarget: nativePose.target,
      nativeTargetDistance: Number(options.nativeTargetDistance),
    },
    vanilla: {
      png: paths.vanillaPng,
      log: paths.vanillaLog,
      windowSize: [Number(options.vanillaWidth), Number(options.vanillaHeight)],
      imageSize: [vanillaDimensions.width, vanillaDimensions.height],
      command: vanillaCommand,
      elapsedMs: vanillaRun.elapsedMs,
    },
    native: {
      png: paths.nativePng,
      log: paths.nativeLog,
      imageSize: [nativeDimensions.width, nativeDimensions.height],
      command: nativeCommand,
      elapsedMs: nativeRun.elapsedMs,
    },
    notes: [
      "Vanilla screenshot uses Minecraft's player/camera stack; native screenshot uses an explicit eye/target derived from the same player position, yaw, and pitch.",
      "This is for visual spot-checking. Terrain, features, lighting, fog, and renderer parity are still active workstreams, so pixel equality is not expected.",
    ],
  };
  fs.writeFileSync(paths.metadata, `${JSON.stringify(metadata, null, 2)}\n`);

  console.log(`metadata: ${paths.metadata}`);
  console.log(`vanilla: ${paths.vanillaPng} (${vanillaDimensions.width}x${vanillaDimensions.height})`);
  console.log(`native:  ${paths.nativePng} (${nativeDimensions.width}x${nativeDimensions.height})`);
}

function parseArgs(args) {
  const options = {
    seed: DEFAULT_SEED,
    camera: DEFAULT_CAMERA,
    dayTime: DEFAULT_DAY_TIME,
    renderDistance: DEFAULT_RENDER_DISTANCE,
    vanillaWidth: DEFAULT_VANILLA_WIDTH,
    vanillaHeight: DEFAULT_VANILLA_HEIGHT,
    settleFrames: DEFAULT_SETTLE_FRAMES,
    timeoutSeconds: DEFAULT_TIMEOUT_SECONDS,
    macosArm64Lwjgl: process.platform === "darwin" && process.arch === "arm64" ? "prism" : "vanilla",
    nativeEyeYOffset: DEFAULT_NATIVE_EYE_Y_OFFSET,
    nativeTargetDistance: DEFAULT_NATIVE_TARGET_DISTANCE,
    nativeRelease: false,
    noBuild: false,
    noDownload: false,
    outDir: null,
    chunkX: null,
    chunkZ: null,
  };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    switch (arg) {
      case "--":
        break;
      case "--help":
      case "-h":
        printUsage();
        process.exit(0);
        break;
      case "--seed":
        options.seed = requireInteger(requireValue(args, ++index, arg), "seed");
        break;
      case "--camera":
        options.camera = requireCamera(requireValue(args, ++index, arg));
        break;
      case "--day-time":
        options.dayTime = requireInteger(requireValue(args, ++index, arg), "day time");
        break;
      case "--render-distance":
        options.renderDistance = requireIntegerInRange(requireValue(args, ++index, arg), "render distance", 2, 16);
        break;
      case "--vanilla-width":
        options.vanillaWidth = requirePositiveInteger(requireValue(args, ++index, arg), "vanilla width");
        break;
      case "--vanilla-height":
        options.vanillaHeight = requirePositiveInteger(requireValue(args, ++index, arg), "vanilla height");
        break;
      case "--settle-frames":
        options.settleFrames = requirePositiveInteger(requireValue(args, ++index, arg), "settle frames");
        break;
      case "--timeout-seconds":
        options.timeoutSeconds = requirePositiveInteger(requireValue(args, ++index, arg), "timeout seconds");
        break;
      case "--macos-arm64-lwjgl":
        options.macosArm64Lwjgl = requireEnum(requireValue(args, ++index, arg), arg, ["vanilla", "prism"]);
        break;
      case "--native-eye-y-offset":
        options.nativeEyeYOffset = requireFiniteNumber(requireValue(args, ++index, arg), "native eye Y offset");
        break;
      case "--native-target-distance":
        options.nativeTargetDistance = requirePositiveFiniteNumber(requireValue(args, ++index, arg), "native target distance");
        break;
      case "--chunk-x":
        options.chunkX = Number(requireInteger(requireValue(args, ++index, arg), "chunk x"));
        break;
      case "--chunk-z":
        options.chunkZ = Number(requireInteger(requireValue(args, ++index, arg), "chunk z"));
        break;
      case "--out-dir":
        options.outDir = requireValue(args, ++index, arg);
        break;
      case "--native-release":
        options.nativeRelease = true;
        break;
      case "--no-build":
        options.noBuild = true;
        break;
      case "--no-download":
        options.noDownload = true;
        break;
      default:
        throw new Error(`unsupported option '${arg}'`);
    }
  }

  return options;
}

function printUsage() {
  console.log(`usage: node oracle/visual-compare.mjs [options]

Captures a vanilla 1.17.1 screenshot and a native offscreen screenshot for the
same seed/camera probe. Outputs are written under /tmp by default.

options:
  --seed <seed>                 default: ${DEFAULT_SEED}
  --camera <x,y,z,yaw,pitch>    default: ${DEFAULT_CAMERA}
  --day-time <ticks>            default: ${DEFAULT_DAY_TIME}
  --render-distance <chunks>    default: ${DEFAULT_RENDER_DISTANCE}; passed to both clients
  --vanilla-width <points>      default: ${DEFAULT_VANILLA_WIDTH}
  --vanilla-height <points>     default: ${DEFAULT_VANILLA_HEIGHT}
  --settle-frames <n>           default: ${DEFAULT_SETTLE_FRAMES}
  --timeout-seconds <n>         default: ${DEFAULT_TIMEOUT_SECONDS}
  --chunk-x <x>                 native interest chunk; default: floor(camera x / 16)
  --chunk-z <z>                 native interest chunk; default: floor(camera z / 16)
  --out-dir <path>              default: /tmp/mclone-visual-compare-seed-<seed>
  --macos-arm64-lwjgl <mode>    vanilla|prism; default: prism on Apple Silicon, otherwise vanilla
  --native-eye-y-offset <n>     default: ${DEFAULT_NATIVE_EYE_Y_OFFSET}
  --native-target-distance <n>  default: ${DEFAULT_NATIVE_TARGET_DISTANCE}
  --native-release              run native capture with cargo run --release
  --no-build                    skip oracle/build.sh for the vanilla launcher
  --no-download                 fail instead of downloading missing vanilla assets/natives`);
}

function vanillaCaptureCommand(options, outputPath) {
  const args = [
    "--silent",
    "oracle:client",
    "--",
    "--macos-arm64-lwjgl",
    options.macosArm64Lwjgl,
    "--screenshot",
    outputPath,
    "--seed",
    options.seed,
    "--camera",
    options.camera,
    "--day-time",
    options.dayTime,
    "--settle-frames",
    options.settleFrames,
    "--timeout-seconds",
    options.timeoutSeconds,
    "--render-distance",
    options.renderDistance,
    "--width",
    options.vanillaWidth,
    "--height",
    options.vanillaHeight,
  ];
  if (options.noBuild) {
    args.push("--no-build");
  }
  if (options.noDownload) {
    args.push("--no-download");
  }
  return ["pnpm", ...args];
}

function nativeCaptureCommand(options, outputPath, dimensions, pose, chunk) {
  const args = ["run"];
  if (options.nativeRelease) {
    args.push("--release");
  }
  args.push(
    "--manifest-path",
    "native/Cargo.toml",
    "-p",
    "mclone-native-client",
    "--",
    "--screenshot",
    outputPath,
    "--width",
    String(dimensions.width),
    "--height",
    String(dimensions.height),
    "--seed",
    options.seed,
    "--chunk-x",
    String(chunk.x),
    "--chunk-z",
    String(chunk.z),
    "--render-distance",
    options.renderDistance,
    "--day-time",
    options.dayTime,
    "--freeze-time",
    "--screenshot-eye",
    formatVec3(pose.eye),
    "--screenshot-target",
    formatVec3(pose.target),
    "--screenshot-camera-view",
    "first-person",
    "--screenshot-hud",
    "false",
    "--debug-passive-showcase",
    "false",
    "--lighting",
    "true",
    "--fullbright",
    "false",
    "--render-color-profile",
    "vanilla",
  );
  return ["cargo", ...args];
}

function runCommand(label, command, logPath) {
  const [executable, ...args] = command;
  const startedAt = Date.now();
  const fd = fs.openSync(logPath, "w");
  try {
    fs.writeSync(fd, `$ ${command.map(shellQuote).join(" ")}\n\n`);
    const result = spawnSync(executable, args, {
      cwd: ROOT_DIR,
      stdio: ["ignore", fd, fd],
      env: process.env,
    });
    if (result.error) {
      throw result.error;
    }
    if (result.status !== 0) {
      throw new Error(`${label} exited with status ${result.status}; see ${logPath}`);
    }
  } finally {
    fs.closeSync(fd);
  }
  return { elapsedMs: Date.now() - startedAt };
}

function parseCamera(value) {
  const [x, y, z, yaw, pitch] = value.split(",").map((part) => Number(part));
  return { x, y, z, yaw, pitch };
}

function nativePoseFromVanillaCamera(camera, eyeYOffset, targetDistance) {
  const yaw = degreesToRadians(camera.yaw);
  const pitch = degreesToRadians(camera.pitch);
  const cosPitch = Math.cos(pitch);
  const direction = [
    -Math.sin(yaw) * cosPitch,
    -Math.sin(pitch),
    Math.cos(yaw) * cosPitch,
  ];
  const eye = [camera.x, camera.y + eyeYOffset, camera.z];
  const target = [
    eye[0] + direction[0] * targetDistance,
    eye[1] + direction[1] * targetDistance,
    eye[2] + direction[2] * targetDistance,
  ];
  return { eye, target };
}

function readPngDimensions(filePath) {
  const header = fs.readFileSync(filePath, { encoding: null, flag: "r" }).subarray(0, 24);
  const signature = "89504e470d0a1a0a";
  if (header.length < 24 || header.subarray(0, 8).toString("hex") !== signature) {
    throw new Error(`${filePath} is not a PNG`);
  }
  return {
    width: header.readUInt32BE(16),
    height: header.readUInt32BE(20),
  };
}

function requireValue(args, index, option) {
  if (index >= args.length || args[index].startsWith("--")) {
    throw new Error(`${option} requires a value`);
  }
  return args[index];
}

function requireCamera(value) {
  const parts = value.split(",");
  if (parts.length !== 5 || parts.some((part) => part.trim() === "" || Number.isNaN(Number(part)))) {
    throw new Error("camera must use numeric x,y,z,yaw,pitch");
  }
  return value;
}

function requireInteger(value, name) {
  if (!/^-?[0-9]+$/.test(value)) {
    throw new Error(`${name} must be an integer`);
  }
  return value;
}

function requirePositiveInteger(value, name) {
  if (!/^[1-9][0-9]*$/.test(value)) {
    throw new Error(`${name} must be a positive integer`);
  }
  return value;
}

function requireIntegerInRange(value, name, min, max) {
  const integer = requireInteger(value, name);
  const numeric = Number(integer);
  if (numeric < min || numeric > max) {
    throw new Error(`${name} must be between ${min} and ${max}`);
  }
  return integer;
}

function requireFiniteNumber(value, name) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    throw new Error(`${name} must be numeric`);
  }
  return value;
}

function requirePositiveFiniteNumber(value, name) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric) || numeric <= 0) {
    throw new Error(`${name} must be a positive number`);
  }
  return value;
}

function requireEnum(value, option, allowed) {
  if (!allowed.includes(value)) {
    throw new Error(`${option} must be one of: ${allowed.join(", ")}`);
  }
  return value;
}

function formatVec3(values) {
  return values.map(formatNumber).join(",");
}

function formatNumber(value) {
  return Number(value.toFixed(6)).toString();
}

function degreesToRadians(value) {
  return (value * Math.PI) / 180;
}

function shellQuote(value) {
  if (/^[A-Za-z0-9_/:=.,+-]+$/.test(value)) {
    return value;
  }
  return `'${value.replace(/'/g, "'\\''")}'`;
}
