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
  input: string;
  outDir: string;
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
      "Usage: tsx src/review.ts <figure.ts|figure.json> [--out-dir PATH] [--width PIXELS] [--height PIXELS]",
    );
  }
  let outDir = "/tmp/mclone-figure-compare/player";
  let width = 360;
  let height = 480;
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
    } else {
      throw new Error(`Unknown argument '${argument}'`);
    }
  }
  return { input, outDir, width, height };
}

function parseDimension(flag: string, value: string | undefined): number {
  const parsed = value === undefined ? Number.NaN : Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0 || parsed > 4096) {
    throw new Error(`${flag} must be an integer from 1 through 4096`);
  }
  return parsed;
}
