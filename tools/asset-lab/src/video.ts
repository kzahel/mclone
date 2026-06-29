import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { createServer } from "vite";

interface VideoArgs {
  clip: string;
  cycles: number;
  debug: boolean;
  fps: number;
  input: string;
  outPath: string;
  seconds: number | undefined;
}

const execFileAsync = promisify(execFile);
const assetLabRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const args = parseArgs(process.argv.slice(2));
const figurePath = toVitePath(args.input);
await assertFfmpeg();
await fs.mkdir(path.dirname(args.outPath), { recursive: true });

const frameDir = await fs.mkdtemp(path.join(os.tmpdir(), "mclone-asset-lab-video-"));
const server = await createServer({
  root: assetLabRoot,
  logLevel: "error",
  server: {
    host: "127.0.0.1",
    port: 0,
    hmr: false,
    strictPort: false,
  },
});

await server.listen();

const url = server.resolvedUrls?.local[0];
if (!url) {
  throw new Error("Vite did not report a local URL");
}

const browser = await chromium.launch();
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 720 },
    deviceScaleFactor: 1,
  });
  const browserErrors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      browserErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => {
    browserErrors.push(error.stack ?? error.message);
  });

  const params = new URLSearchParams({
    figure: figurePath,
    clip: args.clip,
    debug: args.debug ? "1" : "0",
  });
  await page.goto(`${url}video.html?${params.toString()}`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.assetLabVideoReady === true, undefined, { timeout: 15_000 });
  const previewError = await page.locator(".error").textContent().catch(() => null);
  if (previewError) {
    throw new Error(`Video render failed:\n${previewError}`);
  }
  if (browserErrors.length > 0) {
    throw new Error(`Browser errors:\n${browserErrors.join("\n")}`);
  }

  const clipDuration = await page.evaluate(() => window.assetLabVideoDuration ?? 1);
  const seconds = args.seconds ?? Math.max(clipDuration * args.cycles, 2);
  const frameCount = Math.max(2, Math.ceil(seconds * args.fps));
  const target = page.locator("#video");
  for (let frame = 0; frame < frameCount; frame += 1) {
    const time = frame / args.fps;
    await page.evaluate((timeSeconds) => window.assetLabSetVideoTime?.(timeSeconds), time);
    await target.screenshot({ path: path.join(frameDir, `frame-${frame.toString().padStart(5, "0")}.png`) });
  }

  await encodeMp4(frameDir, args.outPath, args.fps);
  console.log(`Wrote ${args.outPath}`);
} finally {
  await browser.close();
  await server.close();
  await fs.rm(frameDir, { force: true, recursive: true });
}

function parseArgs(argv: string[]): VideoArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error("Usage: tsx src/video.ts <figure.ts> [--out <mp4>] [--clip <name>] [--fps <n>] [--seconds <n>] [--cycles <n>] [--clean]");
  }

  let clip = "walk";
  let cycles = 4;
  let debug = true;
  let fps = 24;
  let outPath = path.join("/tmp", "mclone-asset-lab", "walk.mp4");
  let seconds: number | undefined;

  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--out") {
      outPath = argv[index + 1] ?? outPath;
      index += 1;
    } else if (arg === "--clip") {
      clip = argv[index + 1] ?? clip;
      index += 1;
    } else if (arg === "--fps") {
      fps = parsePositiveNumber(argv[index + 1], "--fps");
      index += 1;
    } else if (arg === "--seconds") {
      seconds = parsePositiveNumber(argv[index + 1], "--seconds");
      index += 1;
    } else if (arg === "--cycles") {
      cycles = parsePositiveNumber(argv[index + 1], "--cycles");
      index += 1;
    } else if (arg === "--clean") {
      debug = false;
    } else {
      throw new Error(`Unknown argument '${arg}'`);
    }
  }

  return { clip, cycles, debug, fps, input, outPath, seconds };
}

function parsePositiveNumber(value: string | undefined, flag: string): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${flag} must be a positive number`);
  }
  return parsed;
}

async function assertFfmpeg(): Promise<void> {
  try {
    await execFileAsync("ffmpeg", ["-version"], { timeout: 10_000 });
  } catch {
    throw new Error("ffmpeg is required for asset-lab video output");
  }
}

async function encodeMp4(frameDir: string, outPath: string, fps: number): Promise<void> {
  await execFileAsync(
    "ffmpeg",
    [
      "-hide_banner",
      "-loglevel",
      "error",
      "-y",
      "-framerate",
      String(fps),
      "-i",
      path.join(frameDir, "frame-%05d.png"),
      "-vf",
      "format=yuv420p",
      "-movflags",
      "+faststart",
      "-c:v",
      "libx264",
      outPath,
    ],
    { timeout: 120_000 },
  );
}

function toVitePath(input: string): string {
  const absolute = path.resolve(input);
  const relative = path.relative(assetLabRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`Figure '${input}' must live under ${assetLabRoot}`);
  }
  return `/${relative.replaceAll(path.sep, "/")}`;
}
