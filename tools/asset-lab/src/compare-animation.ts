import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import { chromium } from "@playwright/test";
import { loadFigureJsonDocument } from "./load";
import type {
  FigureAnimationReviewContract,
  FigureReviewContract,
} from "./review-page";
import { assetLabRoot } from "./vite-figure-path";

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(assetLabRoot, "../..");
const args = parseArgs(process.argv.slice(2));
const input = path.isAbsolute(args.input)
  ? args.input
  : path.resolve(repoRoot, args.input);
const relativeInputPath = path.relative(repoRoot, input).split(path.sep).join("/");
if (
  relativeInputPath.startsWith("../")
  || (!relativeInputPath.endsWith(".json") && !relativeInputPath.endsWith(".ts"))
) {
  throw new Error(
    "Engine animation comparison requires a figure TypeScript or JSON source beneath the repository root",
  );
}
await assertFfmpeg();
await fs.mkdir(args.outDir, { recursive: true });
const document = await loadFigureJsonDocument(input);
const nativeInput = await prepareNativeInput(input, document.asset.name, document.json, args.outDir);

await run(
  pnpmCommand(),
  [
    "--dir",
    assetLabRoot,
    "exec",
    "tsx",
    "src/review.ts",
    input,
    "--out-dir",
    args.outDir,
    "--width",
    String(args.width),
    "--height",
    String(args.height),
    "--animation",
    "--clip",
    args.clip,
    "--sample-times",
    args.sampleTimes.join(","),
    "--capture-fps",
    String(args.captureFps),
    "--capture-cycles",
    String(args.captureCycles),
  ],
  repoRoot,
);
await run(
  cargoCommand(),
  [
    "run",
    "--manifest-path",
    "native/Cargo.toml",
    "-p",
    "mclone-figure-review",
    "--",
    "--asset-root",
    nativeInput.assetRoot,
    "--figure",
    nativeInput.assetPath,
    "--out-dir",
    args.outDir,
    "--review-contract",
    path.join(args.outDir, "review-contract.json"),
    "--animation-proof",
  ],
  repoRoot,
);

const threeReceipt = JSON.parse(
  await fs.readFile(path.join(args.outDir, "three-receipt.json"), "utf8"),
) as ThreeReceipt;
const engineReceipt = JSON.parse(
  await fs.readFile(path.join(args.outDir, "animation-receipt.json"), "utf8"),
) as EngineAnimationReceipt;
const preparedReceipt = JSON.parse(
  await fs.readFile(path.join(args.outDir, "engine-receipt.json"), "utf8"),
) as EngineFigureReceipt;
const animation = validateReceipts(threeReceipt, engineReceipt, preparedReceipt);
const keyTimes = (document.asset.clips[args.clip]?.keys ?? []).map((key) => key[1]);

const sheetPath = path.join(args.outDir, `${args.clip}-comparison-sheet.png`);
await writeComparisonSheet(
  sheetPath,
  args.outDir,
  threeReceipt.figure,
  animation,
  engineReceipt,
  keyTimes,
);
const videoPath = path.join(args.outDir, `${args.clip}-comparison.mp4`);
await encodeComparisonVideo(args.outDir, videoPath, animation);

const receipt = {
  schemaVersion: 1,
  figure: threeReceipt.figure,
  clip: animation.clip,
  view: animation.view,
  semanticSha256: threeReceipt.semanticSha256,
  engineImageAlignment: "horizontal-reflection",
  sheet: path.basename(sheetPath),
  video: path.basename(videoPath),
  sampleTimesSeconds: animation.sampleTimesSeconds,
  captureFramesPerSecond: animation.captureFramesPerSecond,
  captureCycleCount: animation.captureCycleCount,
  captureFrameCount: animation.captureFrameCount,
  captureCadenceOnly: true,
  engine: {
    partCount: preparedReceipt.partCount,
    immutableUploadCount: engineReceipt.immutableUploadCount,
    paletteWriteCount: engineReceipt.paletteWriteCount,
    paletteWrittenBytes: engineReceipt.paletteWrittenBytes,
    paletteBytesPerWrite: engineReceipt.paletteBytesPerWrite,
    viewUniformWriteCount: engineReceipt.viewUniformWriteCount,
    changedSequenceFrameCount: engineReceipt.changedSequenceFrameCount,
  },
  checks: {
    sharedAnimationContract: true,
    exactSampleTimes: true,
    continuouslyChangingCapture: true,
    residentTopology: true,
    productionSelectorUnchanged: true,
  },
};
await fs.writeFile(
  path.join(args.outDir, "animation-comparison-receipt.json"),
  `${JSON.stringify(receipt, null, 2)}\n`,
);
console.log(`Animation comparison wrote ${sheetPath} and ${videoPath}`);

interface CompareArgs {
  captureCycles: number;
  captureFps: number;
  clip: string;
  height: number;
  input: string;
  outDir: string;
  sampleTimes: number[];
  width: number;
}

interface ThreeReceipt {
  schemaVersion: number;
  figure: string;
  semanticSha256: string;
  contract: FigureReviewContract;
}

interface EngineAnimationReceipt {
  schemaVersion: number;
  figure: string;
  clip: string;
  view: string;
  durationSeconds: number;
  sampleFrameCount: number;
  captureFramesPerSecond: number;
  captureCycleCount: number;
  sequenceFrameCount: number;
  changedSequenceFrameCount: number;
  sequenceImagePattern: string;
  immutableUploadCount: number;
  paletteWriteCount: number;
  paletteWrittenBytes: number;
  paletteBytesPerWrite: number;
  viewUniformWriteCount: number;
  sampleFrames: Array<{
    requestedTimeSeconds: number;
    localTimeSeconds: number;
    image: string;
    differentFromPrevious?: number;
  }>;
}

interface EngineFigureReceipt {
  schemaVersion: number;
  figure: string;
  partCount: number;
}

function parseArgs(argv: string[]): CompareArgs {
  let input = path.join(repoRoot, "assets/mclone/figures/player.figure.json");
  let outDir = "/tmp/mclone-prepared-animation/player-comparison";
  let clip = "walk";
  let width = 360;
  let height = 480;
  let captureFps = 60;
  let captureCycles = 2;
  let sampleTimes = [0, 0.045, 0.09, 0.123, 0.125, 0.45, 0.899, 0.901];
  let positionalSeen = false;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--") {
      continue;
    } else if (argument === "--out-dir") {
      outDir = requiredValue(argument, argv[index + 1]);
      index += 1;
    } else if (argument === "--clip") {
      clip = requiredValue(argument, argv[index + 1]);
      index += 1;
    } else if (argument === "--width") {
      width = parseDimension(argument, argv[index + 1]);
      index += 1;
    } else if (argument === "--height") {
      height = parseDimension(argument, argv[index + 1]);
      index += 1;
    } else if (argument === "--capture-fps") {
      captureFps = parsePositiveNumber(argument, argv[index + 1]);
      index += 1;
    } else if (argument === "--capture-cycles") {
      captureCycles = parsePositiveNumber(argument, argv[index + 1]);
      index += 1;
    } else if (argument === "--sample-times") {
      sampleTimes = parseSampleTimes(requiredValue(argument, argv[index + 1]));
      index += 1;
    } else if (argument?.startsWith("-")) {
      throw new Error(`Unknown argument '${argument}'`);
    } else if (argument && !positionalSeen) {
      input = argument;
      positionalSeen = true;
    } else {
      throw new Error(`Unexpected argument '${argument}'`);
    }
  }
  return {
    captureCycles,
    captureFps,
    clip,
    height,
    input,
    outDir: path.resolve(outDir),
    sampleTimes,
    width,
  };
}

async function prepareNativeInput(
  inputPath: string,
  figureName: string,
  json: string,
  outDir: string,
): Promise<{ assetRoot: string; assetPath: string }> {
  if (inputPath.endsWith(".json")) {
    return {
      assetRoot: repoRoot,
      assetPath: path.relative(repoRoot, inputPath).split(path.sep).join("/"),
    };
  }
  const assetRoot = path.join(outDir, "native-asset-root");
  const assetPath = `assets/mclone/figures/${figureName}.figure.json`;
  const stagedPath = path.join(assetRoot, ...assetPath.split("/"));
  await fs.mkdir(path.dirname(stagedPath), { recursive: true });
  await fs.writeFile(stagedPath, json, "utf8");
  return { assetRoot, assetPath };
}

function validateReceipts(
  three: ThreeReceipt,
  engine: EngineAnimationReceipt,
  prepared: EngineFigureReceipt,
): FigureAnimationReviewContract {
  const animation = three.contract.animation;
  if (three.schemaVersion !== 1
    || engine.schemaVersion !== 2
    || prepared.schemaVersion !== 1
    || !animation) {
    throw new Error("Animation comparison receipt schema mismatch");
  }
  if (three.figure !== engine.figure
    || engine.figure !== prepared.figure
    || animation.clip !== engine.clip
    || animation.view !== engine.view) {
    throw new Error("Animation comparison figure or clip identity mismatch");
  }
  if (!Number.isInteger(prepared.partCount) || prepared.partCount <= 0) {
    throw new Error("Prepared figure receipt has an invalid part count");
  }
  if (!close(animation.durationSeconds, engine.durationSeconds)
    || !close(animation.captureFramesPerSecond, engine.captureFramesPerSecond)
    || !close(animation.captureCycleCount, engine.captureCycleCount)
    || animation.captureFrameCount !== engine.sequenceFrameCount
    || animation.sampleTimesSeconds.length !== engine.sampleFrameCount) {
    throw new Error("Three.js and native animation contracts differ");
  }
  for (let index = 0; index < animation.sampleTimesSeconds.length; index += 1) {
    if (!close(
      animation.sampleTimesSeconds[index] ?? Number.NaN,
      engine.sampleFrames[index]?.requestedTimeSeconds ?? Number.NaN,
    )) {
      throw new Error(`Animation sample ${index} differs between renderers`);
    }
  }
  const totalFrames = animation.sampleTimesSeconds.length + animation.captureFrameCount;
  const paletteBytesPerWrite = prepared.partCount * 16 * 4;
  if (engine.immutableUploadCount !== 4
    || engine.paletteBytesPerWrite !== paletteBytesPerWrite
    || engine.paletteWriteCount !== totalFrames
    || engine.paletteWrittenBytes !== engine.paletteBytesPerWrite * totalFrames
    || engine.viewUniformWriteCount !== totalFrames) {
    throw new Error("Native animation did not retain topology while updating palettes");
  }
  if (engine.changedSequenceFrameCount < animation.captureFrameCount - 2) {
    throw new Error("Native animation sequence contains unexpected held frames");
  }
  return animation;
}

async function writeComparisonSheet(
  outPath: string,
  outDir: string,
  figureName: string,
  animation: FigureAnimationReviewContract,
  engine: EngineAnimationReceipt,
  keyTimes: number[],
): Promise<void> {
  const panels = await Promise.all(animation.sampleTimesSeconds.map(async (_time, index) => {
    const three = await imageDataUrl(
      path.join(outDir, `three-${animation.clip}-sample-${pad(index, 3)}.png`),
    );
    const native = await imageDataUrl(path.join(outDir, engine.sampleFrames[index]!.image));
    return { native, three };
  }));
  const displayWidth = 240;
  const displayHeight = Math.round(displayWidth * 4 / 3);
  const groupWidth = displayWidth * 2 + 10;
  const columns = 2;
  const rows = Math.ceil(panels.length / columns);
  const cards = panels.map((panel, index) => {
    const requested = animation.sampleTimesSeconds[index] ?? 0;
    const local = engine.sampleFrames[index]?.localTimeSeconds ?? requested;
    return `<article>
      <h2>${escapeHtml(sampleTitle(requested, local, animation.durationSeconds, keyTimes))}</h2>
      <div class="pair">
        <figure><figcaption>Three.js semantic</figcaption><img src="${panel.three}"></figure>
        <figure class="engine"><figcaption>Mclone prepared (aligned)</figcaption><img src="${panel.native}"></figure>
      </div>
    </article>`;
  }).join("");
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({
      viewport: {
        width: groupWidth * columns + 78,
        height: rows * (displayHeight + 68) + 150,
      },
      deviceScaleFactor: 1,
    });
    await page.setContent(`<!doctype html>
      <html><head><style>
        * { box-sizing: border-box; }
        html, body { margin: 0; background: #dfe7ed; color: #17202a; font-family: system-ui, sans-serif; }
        #sheet { width: ${groupWidth * columns + 48}px; margin: 12px; padding: 16px; background: #f8fafc; }
        header { margin-bottom: 14px; }
        h1 { margin: 0 0 4px; font-size: 22px; }
        header p { margin: 0; color: #475569; font: 12px ui-monospace, monospace; }
        .grid { display: grid; grid-template-columns: repeat(${columns}, ${groupWidth}px); gap: 14px 16px; }
        article { min-width: 0; }
        h2 { height: 30px; margin: 0; padding: 5px 7px; background: #cbd8e2; font-size: 13px; }
        .pair { display: grid; grid-template-columns: ${displayWidth}px ${displayWidth}px; gap: 8px; }
        figure { margin: 0; border: 1px solid #9aaab8; background: #edf1f4; }
        figcaption { height: 25px; padding: 4px 6px; background: #e1e9ef; font-size: 11px; font-weight: 700; }
        img { display: block; width: ${displayWidth}px; height: ${displayHeight}px; }
        .engine img { transform: scaleX(-1); }
        footer { margin-top: 14px; color: #475569; font-size: 11px; line-height: 1.4; }
      </style></head><body>
        <main id="sheet">
          <header>
            <h1>${escapeHtml(figureName)} — synchronized ${escapeHtml(animation.clip)} samples</h1>
            <p>${animation.durationSeconds.toFixed(3)}s loop · exact presentation times · 0.123/0.125s are 2ms apart</p>
          </header>
          <section class="grid">${cards}</section>
          <footer>Raw panels remain unscaled beside this sheet. Native panels are reflected only for the established handedness alignment. Capture cadence is not an evaluator or display-rate limit.</footer>
        </main>
      </body></html>`);
    await page.locator("#sheet").screenshot({ path: outPath });
  } finally {
    await browser.close();
  }
}

async function encodeComparisonVideo(
  outDir: string,
  outPath: string,
  animation: FigureAnimationReviewContract,
): Promise<void> {
  await execFileAsync(
    "ffmpeg",
    [
      "-hide_banner",
      "-loglevel",
      "error",
      "-y",
      "-framerate",
      String(animation.captureFramesPerSecond),
      "-i",
      path.join(outDir, `three-${animation.clip}-frame-%05d.png`),
      "-framerate",
      String(animation.captureFramesPerSecond),
      "-i",
      path.join(outDir, `engine-${animation.clip}-frame-%05d.png`),
      "-filter_complex",
      "[1:v]hflip[engine];[0:v][engine]hstack=inputs=2,format=yuv420p[video]",
      "-map",
      "[video]",
      "-frames:v",
      String(animation.captureFrameCount),
      "-movflags",
      "+faststart",
      "-c:v",
      "libx264",
      outPath,
    ],
    { timeout: 120_000, maxBuffer: 16 * 1024 * 1024 },
  );
}

function sampleTitle(
  requested: number,
  local: number,
  duration: number,
  keyTimes: number[],
): string {
  const authoredKey = keyTimes.some((time) => close(time, local));
  const kind = requested >= duration ? "wrapped non-key" : authoredKey ? "authored key" : "interpolated non-key";
  return `t=${requested.toFixed(3)}s · local=${local.toFixed(3)}s · ${kind}`;
}

async function imageDataUrl(imagePath: string): Promise<string> {
  const bytes = await fs.readFile(imagePath);
  return `data:image/png;base64,${bytes.toString("base64")}`;
}

async function run(command: string, commandArgs: string[], cwd: string): Promise<void> {
  const { stdout, stderr } = await execFileAsync(command, commandArgs, {
    cwd,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (stdout.trim()) {
    process.stdout.write(stdout);
  }
  if (stderr.trim()) {
    process.stderr.write(stderr);
  }
}

async function assertFfmpeg(): Promise<void> {
  try {
    await execFileAsync("ffmpeg", ["-version"], { timeout: 10_000 });
  } catch {
    throw new Error("ffmpeg is required for synchronized animation comparison output");
  }
}

function requiredValue(flag: string, value: string | undefined): string {
  if (!value) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function parseDimension(flag: string, value: string | undefined): number {
  const parsed = Number.parseInt(requiredValue(flag, value), 10);
  if (!Number.isInteger(parsed) || parsed <= 0 || parsed > 4096) {
    throw new Error(`${flag} must be an integer from 1 through 4096`);
  }
  return parsed;
}

function parsePositiveNumber(flag: string, value: string | undefined): number {
  const parsed = Number(requiredValue(flag, value));
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${flag} must be a positive number`);
  }
  return parsed;
}

function parseSampleTimes(value: string): number[] {
  const parsed = value.split(",").map(Number);
  if (parsed.length === 0 || parsed.some((time) => !Number.isFinite(time) || time < 0)) {
    throw new Error("--sample-times must contain finite nonnegative seconds");
  }
  return parsed;
}

function close(left: number, right: number): boolean {
  return Number.isFinite(left) && Number.isFinite(right) && Math.abs(left - right) <= 1e-5;
}

function pad(value: number, width: number): string {
  return value.toString().padStart(width, "0");
}

function pnpmCommand(): string {
  return process.platform === "win32" ? "pnpm.cmd" : "pnpm";
}

function cargoCommand(): string {
  return process.platform === "win32" ? "cargo.exe" : "cargo";
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}
