import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { chromium } from "@playwright/test";
import { createServer } from "vite";
import { loadFigureJsonDocument } from "./load";
import type { FigureReviewContract } from "./review-page";
import { assetLabRoot, toViteFigurePath } from "./vite-figure-path";

const VIEW_NAMES = ["front", "right", "three-quarter"] as const;

interface ReviewArgs {
  animation: boolean;
  captureCycles: number;
  captureFps: number;
  clip: string;
  input: string;
  outDir: string;
  sampleTimes: number[];
  width: number;
  height: number;
}

const args = parseArgs(process.argv.slice(2));
const figurePath = toViteFigurePath(args.input);
const document = await loadFigureJsonDocument(args.input);
await fs.mkdir(args.outDir, { recursive: true });

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
    viewport: {
      width: args.width * VIEW_NAMES.length + 64,
      height: args.height + 32,
    },
    deviceScaleFactor: 1,
  });
  const browserErrors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      browserErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => browserErrors.push(error.stack ?? error.message));
  const params = new URLSearchParams({
    figure: figurePath,
    width: String(args.width),
    height: String(args.height),
  });
  if (args.animation) {
    params.set("clip", args.clip);
    params.set("sampleTimes", args.sampleTimes.join(","));
    params.set("captureFps", String(args.captureFps));
    params.set("captureCycles", String(args.captureCycles));
  }
  await page.goto(`${url}review.html?${params.toString()}`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.assetLabReviewReady === true, undefined, { timeout: 15_000 });
  const previewError = await page.locator(".error").textContent().catch(() => null);
  if (previewError) {
    throw new Error(`Review render failed:\n${previewError}`);
  }
  if (browserErrors.length > 0) {
    throw new Error(`Browser errors:\n${browserErrors.join("\n")}`);
  }
  const contract = await page.evaluate(() => window.assetLabReviewContract);
  const figureName = await page.evaluate(() => window.assetLabReviewFigureName);
  if (!contract || !figureName) {
    throw new Error("Review page did not expose its contract and figure name");
  }
  for (const viewName of VIEW_NAMES) {
    await page
      .locator(`[data-view='${viewName}'] canvas`)
      .screenshot({ path: path.join(args.outDir, `three-${viewName}.png`) });
  }
  if (args.animation) {
    const animation = contract.animation;
    if (!animation || animation.clip !== args.clip) {
      throw new Error("Review page did not expose the requested animation contract");
    }
    const animationPanel = page.locator(`[data-view='${animation.view}'] canvas`);
    for (let index = 0; index < animation.sampleTimesSeconds.length; index += 1) {
      const timeSeconds = animation.sampleTimesSeconds[index];
      if (timeSeconds === undefined) {
        throw new Error("Animation sample time index out of range");
      }
      await page.evaluate((time) => window.assetLabSetReviewTime?.(time), timeSeconds);
      await animationPanel.screenshot({
        path: path.join(args.outDir, `three-${animation.clip}-sample-${pad(index, 3)}.png`),
      });
    }
    for (let frame = 0; frame < animation.captureFrameCount; frame += 1) {
      const timeSeconds = frame / animation.captureFramesPerSecond;
      await page.evaluate((time) => window.assetLabSetReviewTime?.(time), timeSeconds);
      await animationPanel.screenshot({
        path: path.join(args.outDir, `three-${animation.clip}-frame-${pad(frame, 5)}.png`),
      });
    }
  }
  await fs.writeFile(
    path.join(args.outDir, "review-contract.json"),
    `${JSON.stringify(contract, null, 2)}\n`,
  );
  const receipt = {
    schemaVersion: 1,
    figure: figureName,
    semanticSha256: createHash("sha256").update(document.json).digest("hex"),
    semanticJsonBytes: Buffer.byteLength(document.json),
    contract: contract satisfies FigureReviewContract,
    views: VIEW_NAMES,
  };
  await fs.writeFile(
    path.join(args.outDir, "three-receipt.json"),
    `${JSON.stringify(receipt, null, 2)}\n`,
  );
  console.log(`Asset Lab review wrote ${VIEW_NAMES.length} views to ${args.outDir}`);
} finally {
  await browser.close();
  await server.close();
}

function parseArgs(argv: string[]): ReviewArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error(
      "Usage: tsx src/review.ts <figure.ts|figure.json> [--out-dir PATH] [--width PIXELS] [--height PIXELS] [--animation] [--clip NAME] [--sample-times CSV] [--capture-fps N] [--capture-cycles N]",
    );
  }
  let outDir = "/tmp/mclone-figure-compare/player";
  let width = 360;
  let height = 480;
  let animation = false;
  let clip = "walk";
  let sampleTimes = [0, 0.045, 0.09, 0.123, 0.125, 0.45, 0.899, 0.901];
  let captureFps = 60;
  let captureCycles = 2;
  for (let index = 1; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--out-dir") {
      outDir = argv[index + 1] ?? outDir;
      index += 1;
    } else if (argument === "--width") {
      width = parseDimension("--width", argv[index + 1]);
      index += 1;
    } else if (argument === "--height") {
      height = parseDimension("--height", argv[index + 1]);
      index += 1;
    } else if (argument === "--animation") {
      animation = true;
    } else if (argument === "--clip") {
      clip = requiredValue(argument, argv[index + 1]);
      index += 1;
    } else if (argument === "--sample-times") {
      sampleTimes = parseSampleTimes(requiredValue(argument, argv[index + 1]));
      index += 1;
    } else if (argument === "--capture-fps") {
      captureFps = parsePositiveNumber(argument, argv[index + 1]);
      index += 1;
    } else if (argument === "--capture-cycles") {
      captureCycles = parsePositiveNumber(argument, argv[index + 1]);
      index += 1;
    } else {
      throw new Error(`Unknown argument '${argument}'`);
    }
  }
  return {
    animation,
    captureCycles,
    captureFps,
    clip,
    input,
    outDir,
    sampleTimes,
    width,
    height,
  };
}

function parseDimension(flag: string, value: string | undefined): number {
  const parsed = value === undefined ? Number.NaN : Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0 || parsed > 4096) {
    throw new Error(`${flag} must be an integer from 1 through 4096`);
  }
  return parsed;
}

function requiredValue(flag: string, value: string | undefined): string {
  if (!value) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
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

function pad(value: number, width: number): string {
  return value.toString().padStart(width, "0");
}
